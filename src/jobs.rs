use crate::domain::{Brand, FormatKey, Job, JobEvent, JobStatus, Preset, SubtitleLine, TranscriptionResponse, Workflow};
use crate::media::{build_render_plan, probe_media, render_video};
use crate::media::transcribe::{extract_audio, transcribe_audio, TranscriptionError};
use crate::persistence::atomic_write_json;
use crate::state::AppState;
use crate::subtitle::{ass::generate_ass_content, llm::correct_lines, normalize_subtitles, srt::{generate_srt_content, parse_ass_to_lines, parse_srt_to_lines}, NormalizeOptions};
use crate::subtitle::segment::group_transcription_into_lines;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis()
}

pub async fn persist_job(state: &AppState, job: &Job) -> Result<()> {
    let path = state.config.jobs_dir().join(format!("{}.json", job.id));
    let job = job.clone();
    tokio::task::spawn_blocking(move || atomic_write_json(&path, &job)).await??;
    Ok(())
}

pub async fn create_job(state: &AppState, original_name: String, input_path: PathBuf) -> Result<Job> {
    let now = now_ms();
    let job = Job {
        id: Uuid::new_v4().to_string(),
        original_name,
        status: JobStatus::Pending,
        progress: None,
        lines: None,
        error: None,
        input_path: Some(input_path),
        output_path: None,
        preset_id: None,
        created_at_ms: now,
        updated_at_ms: now,
    };
    state.jobs.insert(job.id.clone(), job.clone());
    state.job_tokens.insert(job.id.clone(), CancellationToken::new());
    persist_job(state, &job).await?;
    emit(state, &job);
    Ok(job)
}

pub async fn update_job<F>(state: &AppState, id: &str, update: F) -> Result<Job>
where
    F: FnOnce(&mut Job),
{
    let job = {
        let mut entry = state.jobs.get_mut(id).ok_or_else(|| anyhow::anyhow!("job not found: {id}"))?;
        update(&mut entry);
        entry.updated_at_ms = now_ms();
        entry.clone()
    };
    persist_job(state, &job).await?;
    emit(state, &job);
    Ok(job)
}

fn emit(state: &AppState, job: &Job) {
    let _ = state.events.send(JobEvent { id: job.id.clone(), status: job.status.clone(), progress: job.progress, error: job.error.clone() });
}

pub async fn cancel_job(state: &AppState, id: &str) -> Result<Job> {
    if let Some(token) = state.job_tokens.get(id) { token.cancel(); }
    update_job(state, id, |job| {
        job.status = JobStatus::Cancelled;
        job.error = None;
    }).await
}

pub async fn set_job_lines_ready(state: &AppState, id: &str, lines: Vec<SubtitleLine>) -> Result<Job> {
    let report = normalize_subtitles(&lines, NormalizeOptions::default());
    update_job(state, id, move |job| {
        job.lines = Some(report.lines);
        job.status = JobStatus::Ready;
        job.progress = Some(100);
        job.error = None;
    }).await
}

pub async fn start_transcription(state: AppState, id: String, max_chars: u32, max_lines: u32) {
    tokio::spawn(async move {
        if let Err(error) = run_transcription_job(&state, &id, max_chars, max_lines).await {
            let cancelled = state.job_tokens.get(&id).is_some_and(|token| token.is_cancelled());
            let message = error.to_string();
            let _ = update_job(&state, &id, |job| {
                if cancelled {
                    job.status = JobStatus::Cancelled;
                    job.error = None;
                } else {
                    job.status = JobStatus::Error;
                    job.error = Some(message);
                }
                job.progress = None;
            }).await;
        }
    });
}

async fn run_transcription_job(state: &AppState, id: &str, max_chars: u32, max_lines: u32) -> Result<()> {
    let job = state.jobs.get(id).map(|j| j.clone()).ok_or_else(|| anyhow::anyhow!("job not found"))?;
    let input = job.input_path.clone().ok_or_else(|| anyhow::anyhow!("job has no input"))?;
    let token = state.job_tokens.get(id).map(|v| v.clone()).unwrap_or_else(CancellationToken::new);
    update_job(state, id, |job| { job.status = JobStatus::Transcribing; job.progress = None; job.error = None; }).await?;

    let audio = state.config.uploads_dir().join(format!("{id}.wav"));
    extract_audio(&input, &audio, &token).await.context("extract audio")?;
    let settings = state.settings.read().await.clone();
    let transcription = transcribe_audio(&audio, &settings, &state.http, &token).await;
    let _ = tokio::fs::remove_file(&audio).await;
    let transcription = match transcription {
        Ok(value) => value,
        Err(TranscriptionError::Cancelled) => anyhow::bail!("cancelled"),
        Err(error) => return Err(error.into()),
    };
    if token.is_cancelled() { anyhow::bail!("cancelled"); }

    let words_path = state.config.uploads_dir().join(format!("{id}_words.json"));
    let raw = transcription.clone();
    tokio::task::spawn_blocking(move || atomic_write_json(&words_path, &raw)).await??;

    let lines = group_transcription_into_lines(&transcription, max_chars, max_lines);
    let lines = correct_lines(lines, &settings, &state.http, &token).await;
    if token.is_cancelled() { anyhow::bail!("cancelled"); }
    set_job_lines_ready(state, id, lines).await?;
    Ok(())
}

pub fn parse_json_companion(bytes: &[u8], max_chars: u32, max_lines: u32) -> Result<Vec<SubtitleLine>> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(lines) = value.get("lines") {
        if let Ok(lines) = serde_json::from_value::<Vec<SubtitleLine>>(lines.clone()) {
            return Ok(normalize_subtitles(&lines, NormalizeOptions::default()).lines);
        }
    }
    if let Ok(lines) = serde_json::from_value::<Vec<SubtitleLine>>(value.clone()) {
        return Ok(normalize_subtitles(&lines, NormalizeOptions::default()).lines);
    }
    if let Ok(transcription) = serde_json::from_value::<TranscriptionResponse>(value) {
        return Ok(group_transcription_into_lines(&transcription, max_chars, max_lines));
    }
    anyhow::bail!("unsupported subtitle JSON structure")
}

pub async fn load_companion(video: &Path, preset: &Preset) -> Result<Option<(Vec<SubtitleLine>, Vec<PathBuf>)>> {
    let ass = video.with_extension("ass");
    let srt = video.with_extension("srt");
    let json = video.with_extension("json");
    if ass.exists() {
        let text = tokio::fs::read_to_string(&ass).await?;
        return Ok(Some((parse_ass_to_lines(&text), vec![ass])));
    }
    if srt.exists() {
        let text = tokio::fs::read_to_string(&srt).await?;
        return Ok(Some((parse_srt_to_lines(&text), vec![srt])));
    }
    if json.exists() {
        let bytes = tokio::fs::read(&json).await?;
        return Ok(Some((parse_json_companion(&bytes, preset.max_chars, preset.max_lines)?, vec![json])));
    }
    Ok(None)
}

fn extract_parenthesized_preset(filename: &str) -> Option<&str> {
    let start = filename.find('(')? + 1;
    let end = filename[start..].find(')')? + start;
    let value = filename[start..end].trim();
    (!value.is_empty()).then_some(value)
}

pub async fn resolve_preset(state: &AppState, filename: &str, workflow: Option<&Workflow>, requested_preset_id: Option<&str>) -> Preset {
    let presets = state.presets.read().await.clone();
    let brands = state.brands.read().await.clone();

    // V1 behavior: an explicit `(Preset Name)` in the filename wins.
    if let Some(name) = extract_parenthesized_preset(filename) {
        if let Some(preset) = presets.iter().find(|p| p.name.eq_ignore_ascii_case(name)) { return preset.clone(); }
    }

    // V1 keyword behavior, constrained to workflow Brand when one is selected.
    let workflow_brand = workflow.and_then(|w| w.brand_id.as_deref());
    let lower = filename.to_lowercase();
    if let Some(preset) = presets.iter().find(|p| {
        (workflow_brand.is_none() || p.brand_id.as_deref() == workflow_brand) && p.match_keywords.as_deref().is_some_and(|keywords| {
            keywords.split(',').map(str::trim).filter(|v| !v.is_empty()).any(|keyword| lower.contains(&keyword.to_lowercase()))
        })
    }) { return preset.clone(); }

    let explicit_id = requested_preset_id.or_else(|| workflow.and_then(|w| w.preset_id.as_deref()));
    if let Some(id) = explicit_id {
        if let Some(preset) = presets.iter().find(|p| p.id == id) { return preset.clone(); }
    }

    if let Some(workflow) = workflow {
        if let Some(brand_id) = workflow.brand_id.as_deref() {
            if let Some(brand) = brands.iter().find(|b| b.id == brand_id) {
                if let Some(preset_id) = brand.default_preset_by_format.get(&workflow.format.key) {
                    if let Some(preset) = presets.iter().find(|p| p.id == *preset_id) { return preset.clone(); }
                }
            }
        }
    }

    presets.iter().find(|p| p.name == "Défaut").cloned().or_else(|| presets.first().cloned()).unwrap_or_default()
}

fn brand_for_preset<'a>(brands: &'a [Brand], preset: &Preset) -> Option<&'a Brand> {
    preset.brand_id.as_deref().and_then(|id| brands.iter().find(|brand| brand.id == id))
}

fn resolve_outro_name(preset: &Preset, brand: Option<&Brand>, override_value: Option<&str>) -> Option<String> {
    match override_value {
        Some(value) if value.is_empty() => None,
        Some(value) => Some(value.to_string()),
        None => preset.outro_video.clone().or_else(|| brand.and_then(|b| b.assets.default_outro.clone())),
    }
}

fn output_path_for(dir: &Path, original_name: &str) -> PathBuf {
    let original = Path::new(original_name);
    let stem = original.file_stem().and_then(|v| v.to_str()).unwrap_or("video");
    let ext = original.extension().and_then(|v| v.to_str()).unwrap_or("mp4");
    let mut candidate = dir.join(format!("{stem} - ST.{ext}"));
    let mut n = 1usize;
    while candidate.exists() {
        candidate = dir.join(format!("{stem} - ST({n}).{ext}"));
        n += 1;
    }
    candidate
}

fn partial_path(final_path: &Path) -> PathBuf {
    let parent = final_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = final_path.file_stem().and_then(|v| v.to_str()).unwrap_or("video");
    let ext = final_path.extension().and_then(|v| v.to_str()).unwrap_or("mp4");
    parent.join(format!(".{stem}.partial-{}.{}", Uuid::new_v4(), ext))
}

pub async fn start_burn(state: AppState, id: String, lines: Option<Vec<SubtitleLine>>, preset_id: Option<String>, outro_override: Option<String>) {
    tokio::spawn(async move {
        if let Some(lines) = lines {
            let normalized = normalize_subtitles(&lines, NormalizeOptions::default()).lines;
            let _ = update_job(&state, &id, |job| job.lines = Some(normalized)).await;
        }
        if let Some(preset_id) = preset_id {
            let _ = update_job(&state, &id, |job| job.preset_id = Some(preset_id)).await;
        }
        if let Err(error) = run_burn_job(&state, &id, outro_override.as_deref()).await {
            let cancelled = state.job_tokens.get(&id).is_some_and(|token| token.is_cancelled());
            let message = error.to_string();
            let _ = update_job(&state, &id, |job| {
                if cancelled { job.status = JobStatus::Cancelled; job.error = None; }
                else { job.status = JobStatus::Error; job.error = Some(message); }
                job.progress = None;
            }).await;
        }
    });
}

async fn run_burn_job(state: &AppState, id: &str, outro_override: Option<&str>) -> Result<()> {
    let job = state.jobs.get(id).map(|j| j.clone()).ok_or_else(|| anyhow::anyhow!("job not found"))?;
    let input = job.input_path.clone().ok_or_else(|| anyhow::anyhow!("job has no input"))?;
    let lines = job.lines.clone().ok_or_else(|| anyhow::anyhow!("job has no subtitle lines"))?;
    let token = state.job_tokens.get(id).map(|t| t.clone()).unwrap_or_else(CancellationToken::new);
    let preset = resolve_preset(state, &job.original_name, None, job.preset_id.as_deref()).await;
    let source = probe_media(&input, &token).await.context("probe input")?;
    let output = output_path_for(&state.config.output_dir, &job.original_name);
    let temporary_video = partial_path(&output);
    let stem = output.file_stem().and_then(|v| v.to_str()).unwrap_or("video");
    let final_ass = state.config.output_dir.join(format!("{stem}.ass"));
    let final_srt = state.config.output_dir.join(format!("{stem}.srt"));
    let final_json = state.config.output_dir.join(format!("{stem}.json"));
    let temp_ass = state.config.output_dir.join(format!(".{stem}.{}.ass", Uuid::new_v4()));
    tokio::fs::write(&temp_ass, generate_ass_content(&lines, &preset, Some((source.width, source.height)))).await?;

    let brands = state.brands.read().await.clone();
    let brand = brand_for_preset(&brands, &preset);
    let outro_name = resolve_outro_name(&preset, brand, outro_override);
    let outro_path = outro_name.as_deref().map(|name| state.config.outros_dir().join(Path::new(name).file_name().unwrap_or_default()));
    let outro_probe = if let Some(path) = outro_path.as_deref() {
        if path.exists() { Some(probe_media(path, &token).await.context("probe outro")?) } else { None }
    } else { None };
    let outro = outro_path.as_deref().zip(outro_probe.as_ref());
    let settings = state.settings.read().await.clone();
    let caps = state.encoders.read().await.clone();
    let plan = build_render_plan(&input, &temporary_video, &temp_ass, &preset, &settings.encoder, &caps, &source, outro)?;

    update_job(state, id, |job| { job.status = JobStatus::Burning; job.progress = Some(0); job.error = None; }).await?;
    let _permit = state.encode_slots.acquire().await.context("encode semaphore closed")?;
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(32);
    let progress_state = state.clone();
    let progress_id = id.to_string();
    let progress_forwarder = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = update_job(&progress_state, &progress_id, |job| {
                if job.status == JobStatus::Burning { job.progress = Some(progress); }
            }).await;
        }
    });
    let render_result = render_video(&plan, source.duration + outro_probe.as_ref().map(|p| p.duration).unwrap_or(0.0), &token, Some(progress_tx)).await;
    progress_forwarder.abort();
    if let Err(error) = render_result {
        let _ = tokio::fs::remove_file(&temporary_video).await;
        let _ = tokio::fs::remove_file(&temp_ass).await;
        return Err(error.into());
    }
    if token.is_cancelled() {
        let _ = tokio::fs::remove_file(&temporary_video).await;
        let _ = tokio::fs::remove_file(&temp_ass).await;
        anyhow::bail!("cancelled");
    }

    tokio::fs::rename(&temporary_video, &output).await?;
    tokio::fs::rename(&temp_ass, &final_ass).await?;
    let srt = generate_srt_content(&lines);
    tokio::fs::write(&final_srt, srt).await?;
    let json = serde_json::json!({"lines": normalize_subtitles(&lines, NormalizeOptions::default()).lines});
    let json_path = final_json.clone();
    tokio::task::spawn_blocking(move || atomic_write_json(&json_path, &json)).await??;

    let words = state.config.uploads_dir().join(format!("{id}_words.json"));
    if words.exists() {
        let destination = state.config.output_dir.join(format!("{stem}_words.json"));
        tokio::fs::copy(&words, destination).await?;
        let _ = tokio::fs::remove_file(words).await;
    }
    let _ = tokio::fs::remove_file(&input).await;
    update_job(state, id, |job| {
        if job.status != JobStatus::Cancelled {
            job.status = JobStatus::Done;
            job.progress = Some(100);
            job.output_path = Some(output.clone());
            job.error = None;
        }
    }).await?;
    Ok(())
}

pub async fn transcribe_for_workflow(state: &AppState, video: &Path, preset: &Preset, token: &CancellationToken) -> Result<(Vec<SubtitleLine>, Option<PathBuf>)> {
    if let Some((lines, _companions)) = load_companion(video, preset).await? { return Ok((lines, None)); }
    let audio = video.with_extension(format!("{}.autosubs.wav", video.extension().and_then(|v| v.to_str()).unwrap_or("video")));
    extract_audio(video, &audio, token).await?;
    let settings = state.settings.read().await.clone();
    let transcription = transcribe_audio(&audio, &settings, &state.http, token).await;
    let _ = tokio::fs::remove_file(&audio).await;
    let transcription = transcription?;
    let words_path = video.with_file_name(format!("{}_words.json", video.file_stem().and_then(|v| v.to_str()).unwrap_or("video")));
    let raw = transcription.clone();
    let raw_path = words_path.clone();
    tokio::task::spawn_blocking(move || atomic_write_json(&raw_path, &raw)).await??;
    let lines = group_transcription_into_lines(&transcription, preset.max_chars, preset.max_lines);
    let lines = correct_lines(lines, &settings, &state.http, token).await;
    Ok((lines, Some(words_path)))
}

pub async fn render_workflow_file(state: &AppState, workflow: &Workflow, video: &Path, lines: &[SubtitleLine], preset: &Preset, token: &CancellationToken) -> Result<PathBuf> {
    let source = probe_media(video, token).await?;
    let output_dir = PathBuf::from(&workflow.output_dir);
    tokio::fs::create_dir_all(&output_dir).await?;
    let original_name = video.file_name().and_then(|v| v.to_str()).unwrap_or("video.mp4");
    let output = output_path_for(&output_dir, original_name);
    let temporary_video = partial_path(&output);
    let stem = output.file_stem().and_then(|v| v.to_str()).unwrap_or("video");
    let temp_ass = output_dir.join(format!(".{stem}.{}.ass", Uuid::new_v4()));
    tokio::fs::write(&temp_ass, generate_ass_content(lines, preset, Some((source.width, source.height)))).await?;
    let brands = state.brands.read().await.clone();
    let outro_name = resolve_outro_name(preset, brand_for_preset(&brands, preset), None);
    let outro_path = outro_name.as_deref().map(|name| state.config.outros_dir().join(Path::new(name).file_name().unwrap_or_default()));
    let outro_probe = if let Some(path) = outro_path.as_deref() { if path.exists() { Some(probe_media(path, token).await?) } else { None } } else { None };
    let settings = state.settings.read().await.clone();
    let caps = state.encoders.read().await.clone();
    let plan = build_render_plan(video, &temporary_video, &temp_ass, preset, &settings.encoder, &caps, &source, outro_path.as_deref().zip(outro_probe.as_ref()))?;
    let _permit = state.encode_slots.acquire().await?;
    if let Err(error) = render_video(&plan, source.duration + outro_probe.as_ref().map(|v| v.duration).unwrap_or(0.0), token, None).await {
        let _ = tokio::fs::remove_file(&temporary_video).await;
        let _ = tokio::fs::remove_file(&temp_ass).await;
        return Err(error.into());
    }
    if token.is_cancelled() { anyhow::bail!("cancelled"); }

    tokio::fs::rename(&temporary_video, &output).await?;
    let final_ass = output_dir.join(format!("{stem}.ass"));
    tokio::fs::rename(&temp_ass, &final_ass).await?;
    tokio::fs::write(output_dir.join(format!("{stem}.srt")), generate_srt_content(lines)).await?;
    let json = serde_json::json!({"lines": normalize_subtitles(lines, NormalizeOptions::default()).lines});
    let json_path = output_dir.join(format!("{stem}.json"));
    tokio::task::spawn_blocking(move || atomic_write_json(&json_path, &json)).await??;
    let source_words = video.with_file_name(format!("{}_words.json", video.file_stem().and_then(|v| v.to_str()).unwrap_or("video")));
    if source_words.exists() { tokio::fs::copy(source_words, output_dir.join(format!("{stem}_words.json"))).await?; }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_companion_accepts_lines_wrapper_array_and_transcription() {
        let wrapper = br#"{"lines":[{"id":0,"start":0.0,"end":1.0,"text":"hello"}]}"#;
        assert_eq!(parse_json_companion(wrapper, 25, 2).unwrap().len(), 1);
        let array = br#"[{"id":0,"start":0.0,"end":1.0,"text":"hello"}]"#;
        assert_eq!(parse_json_companion(array, 25, 2).unwrap().len(), 1);
        let raw = br#"{"words":[{"word":"hello","start":0.0,"end":0.5},{"word":"world","start":0.5,"end":1.0}]}"#;
        assert!(!parse_json_companion(raw, 25, 2).unwrap().is_empty());
    }

    #[test]
    fn empty_outro_override_means_no_outro() {
        let preset = Preset { outro_video: Some("preset.mp4".into()), ..Preset::default() };
        assert_eq!(resolve_outro_name(&preset, None, Some("")), None);
        assert_eq!(resolve_outro_name(&preset, None, None), Some("preset.mp4".into()));
    }

    #[test]
    fn parenthesized_name_parser_is_safe() {
        assert_eq!(extract_parenthesized_preset("clip (Hormozi).mp4"), Some("Hormozi"));
        assert_eq!(extract_parenthesized_preset("clip.mp4"), None);
    }
}
