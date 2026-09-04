use crate::domain::{
    Asset, Brand, EncoderKind, Job, JobOutro, JobStatus, Preset, RawWord, SubtitleLine,
    TimingQuality, TranscriptTimeline, TranscriptionResponse, Workflow,
};
use crate::media::process::ProcessError;
use crate::media::transcribe::{TranscriptionError, extract_audio, transcribe_audio};
use crate::media::{build_render_plan, probe_media, render_video};
use crate::state::AppState;
use crate::subtitle::{
    NormalizeOptions,
    ass::{generate_ass_content, scale_ass_metric},
    group_transcription_into_lines,
    llm::correct_lines,
    normalize_subtitles,
    segment::LayoutOptions,
    segment::transcript_timeline,
    srt::{generate_srt_content, parse_ass_to_lines, parse_srt_to_lines},
};
use anyhow::{Context, Result, anyhow, bail};
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub fn persist_job(state: &AppState, job: &Job) -> Result<()> {
    state.db.upsert("job", &job.id, job)
}

pub fn create_job(
    state: &AppState,
    original_name: String,
    input_path: PathBuf,
    sidecar: Option<PathBuf>,
    preset_id: Option<String>,
    workflow: Option<&Workflow>,
) -> Result<Job> {
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
        preset_id,
        outro: crate::domain::JobOutro::Inherit,
        format: workflow.map(|w| w.format.clone()).unwrap_or_default(),
        workflow_id: workflow.map(|w| w.id.clone()),
        archive_after_success: workflow.is_some(),
        attached_sidecar: sidecar,
        created_at_ms: now,
        updated_at_ms: now,
    };
    state.jobs.insert(job.id.clone(), job.clone());
    persist_job(state, &job)?;
    state.emit_job(&job);
    Ok(job)
}

pub fn update_job<F>(state: &AppState, id: &str, update: F) -> Result<Job>
where
    F: FnOnce(&mut Job),
{
    let job = {
        let mut entry = state
            .jobs
            .get_mut(id)
            .ok_or_else(|| anyhow!("job not found: {id}"))?;
        update(&mut entry);
        entry.updated_at_ms = now_ms();
        entry.clone()
    };
    persist_job(state, &job)?;
    state.emit_job(&job);
    Ok(job)
}

pub fn get_job(state: &AppState, id: &str) -> Result<Job> {
    state
        .jobs
        .get(id)
        .map(|v| v.clone())
        .ok_or_else(|| anyhow!("job not found: {id}"))
}

pub fn delete_job(state: &AppState, id: &str) -> Result<()> {
    let job = get_job(state, id)?;
    if job.status.is_active() {
        bail!("cannot delete an active job; cancel it first");
    }

    let safe_work_id = Uuid::parse_str(&job.id)
        .context("stored job id is not a UUID")?
        .to_string();
    let work_dir = state.config.work_dir();
    let work_files = [".wav", "_words.json", ".ass"]
        .into_iter()
        .map(|suffix| {
            crate::config::Config::safe_child(&work_dir, &format!("{safe_work_id}{suffix}"))
        })
        .collect::<Result<Vec<_>>>()?;

    if let Some((_, token)) = state.job_tokens.remove(id) {
        token.cancel();
    }
    state.jobs.remove(id);
    state.db.delete("job", id)?;
    state.db.delete("job_transcript", id)?;

    for path in work_files {
        let _ = std::fs::remove_file(path);
    }

    Ok(())
}

pub fn cancel_job(state: &AppState, id: &str) -> Result<Job> {
    if let Some(token) = state.job_tokens.get(id) {
        token.cancel();
    }
    update_job(state, id, |job| {
        job.status = JobStatus::Cancelled;
        job.progress = None;
        job.error = None;
    })
}

fn fresh_token(state: &AppState, id: &str) -> CancellationToken {
    if let Some((_, old)) = state.job_tokens.remove(id) {
        old.cancel();
    }
    let token = CancellationToken::new();
    state.job_tokens.insert(id.to_owned(), token.clone());
    token
}

pub fn enqueue_prepare(state: AppState, id: String) -> Result<()> {
    let permit = state
        .active_job_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| anyhow!("job queue is full"))?;
    let token = fresh_token(&state, &id);
    tokio::spawn(async move {
        let _permit = permit;
        if let Err(error) = prepare_job(&state, &id, &token, false).await {
            finish_error(&state, &id, &token, error);
        }
        state.job_tokens.remove(&id);
    });
    Ok(())
}

pub fn enqueue_retranscribe(state: AppState, id: String) -> Result<()> {
    let job = get_job(&state, &id)?;
    if job.status.is_active() {
        bail!("cannot retranscribe an active job");
    }
    let permit = state
        .active_job_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| anyhow!("job queue is full"))?;
    let token = fresh_token(&state, &id);
    tokio::spawn(async move {
        let _permit = permit;
        if let Err(error) = prepare_job(&state, &id, &token, true).await {
            finish_error(&state, &id, &token, error);
        }
        state.job_tokens.remove(&id);
    });
    Ok(())
}

async fn cancellable_permit(
    semaphore: std::sync::Arc<tokio::sync::Semaphore>,
    token: &CancellationToken,
) -> Result<tokio::sync::OwnedSemaphorePermit> {
    tokio::select! {
        permit = semaphore.acquire_owned() => permit.map_err(|_| anyhow!("worker pool closed")),
        _ = token.cancelled() => bail!("cancelled"),
    }
}

async fn prepare_job(
    state: &AppState,
    id: &str,
    token: &CancellationToken,
    force_audio: bool,
) -> Result<()> {
    let job = get_job(state, id)?;
    let input = job
        .input_path
        .clone()
        .ok_or_else(|| anyhow!("job has no input"))?;
    update_job(state, id, |job| {
        job.status = JobStatus::Probing;
        job.progress = None;
        job.error = None;
    })?;
    let probe = probe_media(&input, token).await.context("probe video")?;
    if token.is_cancelled() {
        bail!("cancelled");
    }
    let workflow = if let Some(wid) = job.workflow_id.as_deref() {
        state
            .workflows
            .read()
            .await
            .iter()
            .find(|w| w.id == wid)
            .cloned()
    } else {
        None
    };
    let preset = resolve_preset(
        state,
        &job.original_name,
        workflow.as_ref(),
        job.preset_id.as_deref(),
    )
    .await;
    if workflow.is_some() {
        let preset_format = preset.format.clone();
        update_job(state, id, move |job| job.format = preset_format)?;
    }
    let lines = if !force_audio && let Some(sidecar) = job.attached_sidecar.clone() {
        load_sidecar(&sidecar, &preset).await?
    } else {
        update_job(state, id, |job| {
            job.status = JobStatus::Transcribing;
            job.progress = None;
        })?;
        let _slot = cancellable_permit(state.transcription_slots.clone(), token).await?;
        let audio = state.config.work_dir().join(format!("{id}.wav"));
        extract_audio(&input, &audio, token)
            .await
            .context("extract transcription audio")?;
        let settings = state.settings.read().await.clone();
        let result = transcribe_audio(&audio, &settings, &state.http, token).await;
        let _ = tokio::fs::remove_file(&audio).await;
        let transcription = match result {
            Ok(v) => v,
            Err(TranscriptionError::Cancelled) => bail!("cancelled"),
            Err(e) => return Err(e.into()),
        };
        persist_transcript(state, id, &transcript_timeline(&transcription))?;
        let raw_path = state.config.work_dir().join(format!("{id}_words.json"));
        tokio::fs::write(&raw_path, serde_json::to_vec_pretty(&transcription)?).await?;
        let (output_width, output_height) = preset
            .format
            .resolution(Some((probe.width, probe.height)))
            .unwrap_or((probe.width, probe.height));
        let lines = crate::subtitle::group_transcription_into_lines_with_layout(
            &transcription,
            LayoutOptions {
                max_chars: preset.max_chars,
                max_lines: preset.max_lines,
                output_width,
                font_size: scale_ass_metric(preset.size, output_height),
            },
        );
        if settings.llm_enabled {
            update_job(state, id, |job| job.status = JobStatus::Correcting)?;
            correct_lines(lines, &settings, &state.http, token).await
        } else {
            lines
        }
    };
    if token.is_cancelled() {
        bail!("cancelled");
    }
    let report = normalize_subtitles(&lines, NormalizeOptions::default());
    update_job(state, id, move |job| {
        job.lines = Some(report.lines);
        job.status = JobStatus::Ready;
        job.progress = Some(100);
        job.error = None;
    })?;
    Ok(())
}

fn finish_error(state: &AppState, id: &str, token: &CancellationToken, error: anyhow::Error) {
    let cancelled = token.is_cancelled() || error.to_string() == "cancelled";
    let message = error.to_string();
    let _ = update_job(state, id, |job| {
        job.status = if cancelled {
            JobStatus::Cancelled
        } else {
            JobStatus::Error
        };
        job.progress = None;
        job.error = if cancelled { None } else { Some(message) };
    });
}

pub fn enqueue_render(state: AppState, id: String) -> Result<()> {
    let job = get_job(&state, &id)?;
    if !matches!(
        job.status,
        JobStatus::Ready | JobStatus::Done | JobStatus::Error | JobStatus::Interrupted
    ) {
        bail!("job is not ready to render");
    }
    if job.lines.as_ref().is_none_or(Vec::is_empty) {
        bail!("job has no subtitles");
    }
    let permit = state
        .active_job_slots
        .clone()
        .try_acquire_owned()
        .map_err(|_| anyhow!("job queue is full"))?;
    let token = fresh_token(&state, &id);
    tokio::spawn(async move {
        let _permit = permit;
        if let Err(error) = render_job(&state, &id, &token).await {
            finish_error(&state, &id, &token, error);
        }
        state.job_tokens.remove(&id);
    });
    Ok(())
}

async fn render_job(state: &AppState, id: &str, token: &CancellationToken) -> Result<()> {
    let _slot = cancellable_permit(state.render_slots.clone(), token).await?;
    let job = get_job(state, id)?;
    let input = job
        .input_path
        .clone()
        .ok_or_else(|| anyhow!("job has no input"))?;
    let lines = job
        .lines
        .clone()
        .ok_or_else(|| anyhow!("job has no subtitles"))?;
    let workflow = if let Some(wid) = &job.workflow_id {
        state
            .workflows
            .read()
            .await
            .iter()
            .find(|w| &w.id == wid)
            .cloned()
    } else {
        None
    };
    let mut preset = resolve_preset(
        state,
        &job.original_name,
        workflow.as_ref(),
        job.preset_id.as_deref(),
    )
    .await;
    // The job owns output geometry. A preset supplies styling; it must never silently override a job format.
    preset.format = job.format.clone();
    let source = probe_media(&input, token)
        .await
        .context("probe render input")?;
    let output_dir = match workflow.as_ref() {
        Some(workflow) => state
            .config
            .resolve_allowed_dir(Path::new(&workflow.output_dir))
            .context("validate workflow output directory")?,
        None => state.config.outputs_dir(),
    };
    tokio::fs::create_dir_all(&output_dir).await?;
    let output_reservation = reserve_output_path(&output_dir, &job.original_name).await?;
    let final_video = output_reservation.path.clone();
    let staging_video = partial_path(&final_video);
    let work_ass = state.config.work_dir().join(format!("{id}.ass"));
    let mut cleanup = CleanupFiles::default();
    cleanup.track(staging_video.clone());
    cleanup.track(work_ass.clone());
    let ass = generate_ass_content(&lines, &preset, Some((source.width, source.height)));
    tokio::fs::write(&work_ass, ass.as_bytes()).await?;
    let outro = resolve_outro(state, &preset, &job.outro).await;
    let outro_probe = if let Some(path) = &outro {
        Some(probe_media(path, token).await.context("probe outro")?)
    } else {
        None
    };
    let caps = state.encoders.read().await.clone();
    if !caps.libass {
        bail!("FFmpeg was built without the ass/libass filter");
    }
    update_job(state, id, |job| {
        job.status = JobStatus::Rendering;
        job.progress = Some(0);
        job.error = None;
    })?;
    let settings = state.settings.read().await.clone();
    let mut plan = build_render_plan(
        &input,
        &staging_video,
        &work_ass,
        &preset,
        &settings.encoder,
        &caps,
        &source,
        outro.as_deref().zip(outro_probe.as_ref()),
        Some(&state.config.fonts_dir),
    )?;
    let (progress_tx, mut progress_rx) = mpsc::channel(16);
    let progress_state = state.clone();
    let progress_id = id.to_owned();
    let progress_task = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = update_job(&progress_state, &progress_id, |job| {
                job.progress = Some(progress)
            });
        }
    });
    let duration = source.duration + outro_probe.as_ref().map(|p| p.duration).unwrap_or(0.0);
    let first = render_video(&plan, duration, token, Some(progress_tx.clone())).await;
    let result = match first {
        Ok(()) => Ok(()),
        Err(error)
            if settings.encoder.kind == EncoderKind::Auto
                && plan.encoder != EncoderKind::Libx264
                && !token.is_cancelled() =>
        {
            tracing::warn!(job_id = id, encoder = ?plan.encoder, error = %error, "hardware render failed; retrying once with libx264");
            let mut fallback = settings.encoder.clone();
            fallback.kind = EncoderKind::Libx264;
            plan = build_render_plan(
                &input,
                &staging_video,
                &work_ass,
                &preset,
                &fallback,
                &caps,
                &source,
                outro.as_deref().zip(outro_probe.as_ref()),
                Some(&state.config.fonts_dir),
            )?;
            render_video(&plan, duration, token, Some(progress_tx)).await
        }
        Err(error) => Err(error),
    };
    progress_task.abort();
    result.map_err(|e| anyhow!(e)).context("render video")?;
    if token.is_cancelled() {
        bail!("cancelled");
    }
    let rendered_probe = probe_media(&staging_video, token)
        .await
        .context("validate rendered staging video")?;
    if rendered_probe.width == 0
        || rendered_probe.height == 0
        || !rendered_probe.duration.is_finite()
        || rendered_probe.duration <= 0.0
    {
        bail!("rendered staging video failed validation");
    }

    let stem = final_video
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("video");
    let side_srt = final_video.with_file_name(format!("{stem}.srt"));
    let side_ass = final_video.with_file_name(format!("{stem}.ass"));
    let side_json = final_video.with_file_name(format!("{stem}.json"));
    let staged_srt = temp_peer(&side_srt);
    let staged_ass = temp_peer(&side_ass);
    let staged_json = temp_peer(&side_json);
    cleanup.track(staged_srt.clone());
    cleanup.track(staged_ass.clone());
    cleanup.track(staged_json.clone());
    tokio::fs::write(&staged_srt, generate_srt_content(&lines)).await?;
    tokio::fs::write(
        &staged_ass,
        generate_ass_content(&lines, &preset, Some((source.width, source.height))),
    )
    .await?;
    tokio::fs::write(&staged_json, serde_json::to_vec_pretty(&lines)?).await?;
    publish_transaction(&[
        (staging_video.clone(), final_video.clone()),
        (staged_srt, side_srt),
        (staged_ass, side_ass),
        (staged_json, side_json),
    ])
    .await?;

    let mut effective_input = input.clone();
    if job.archive_after_success
        && let Some(workflow) = workflow.as_ref()
    {
        let archive_dir = state
            .config
            .resolve_allowed_dir(Path::new(&workflow.archive_dir))
            .context("validate workflow archive directory")?;
        let source_name = input
            .file_name()
            .ok_or_else(|| anyhow!("source has no filename"))?;
        let archive_reservation = reserve_archive_path(&archive_dir, source_name).await?;
        let archived = archive_reservation.path.clone();
        move_file(&input, &archived)
            .await
            .context("archive source after successful render")?;
        effective_input = archived;
        if let Some(sidecar) = &job.attached_sidecar
            && sidecar.exists()
            && let Some(name) = sidecar.file_name()
        {
            match reserve_archive_path(&archive_dir, name).await {
                Ok(sidecar_reservation) => {
                    if let Err(error) = move_file(sidecar, &sidecar_reservation.path).await {
                        tracing::warn!(%error, "could not archive subtitle sidecar");
                    }
                }
                Err(error) => {
                    tracing::warn!(%error, "could not reserve archive name for subtitle sidecar")
                }
            }
        }
    }
    update_job(state, id, move |job| {
        job.status = JobStatus::Done;
        job.progress = Some(100);
        job.error = None;
        job.output_path = Some(final_video);
        job.input_path = Some(effective_input);
        job.archive_after_success = false;
    })?;
    Ok(())
}

pub fn save_subtitles(
    state: &AppState,
    id: &str,
    lines: Vec<SubtitleLine>,
) -> Result<crate::subtitle::NormalizationReport> {
    let job = get_job(state, id)?;
    if job.status.is_active() {
        bail!("cannot edit subtitles while the job is active");
    }
    let report = normalize_subtitles(&lines, NormalizeOptions::default());
    let saved = report.lines.clone();
    let timing_quality = state
        .db
        .get::<TranscriptTimeline>("job_transcript", id)?
        .map(|timeline| timeline.timing_quality)
        .unwrap_or(TimingQuality::Inferred);
    let timeline = TranscriptTimeline {
        words: saved
            .iter()
            .flat_map(|line| line.words.clone().unwrap_or_default())
            .collect(),
        timing_quality,
    };
    persist_transcript(state, id, &timeline)?;
    update_job(state, id, move |job| {
        job.lines = Some(saved);
        job.status = JobStatus::Ready;
        job.progress = Some(100);
        job.error = None;
    })?;
    Ok(report)
}

pub fn persist_transcript(state: &AppState, id: &str, timeline: &TranscriptTimeline) -> Result<()> {
    state.db.upsert("job_transcript", id, timeline)
}

pub fn regroup_subtitles(
    state: &AppState,
    id: &str,
    max_chars: u32,
    max_lines: u32,
) -> Result<Vec<SubtitleLine>> {
    let job = get_job(state, id)?;
    let timeline =
        if let Some(timeline) = state.db.get::<TranscriptTimeline>("job_transcript", id)? {
            timeline
        } else {
            let words = job
                .lines
                .as_ref()
                .into_iter()
                .flatten()
                .flat_map(|line| line.words.clone().unwrap_or_default())
                .collect::<Vec<_>>();
            if words.is_empty() {
                bail!("job has no word timing to regroup");
            }
            let timeline = TranscriptTimeline {
                words,
                timing_quality: TimingQuality::Inferred,
            };
            persist_transcript(state, id, &timeline)?;
            timeline
        };
    if timeline.words.is_empty() {
        bail!("job has no word timing to regroup");
    }
    let lines = group_transcription_into_lines(
        &TranscriptionResponse {
            text: None,
            words: Some(
                timeline
                    .words
                    .into_iter()
                    .map(|w| RawWord {
                        word: Some(w.word),
                        start: Some(w.start),
                        end: Some(w.end),
                    })
                    .collect(),
            ),
            segments: None,
        },
        max_chars,
        max_lines,
    );
    save_subtitles(state, id, lines.clone())?;
    Ok(lines)
}

pub fn attach_sidecar(state: &AppState, id: &str, path: Option<PathBuf>) -> Result<Job> {
    let job = get_job(state, id)?;
    if job.status.is_active() {
        bail!("cannot change sidecar while job is active");
    }
    update_job(state, id, move |job| {
        job.attached_sidecar = path;
        job.lines = None;
        job.status = JobStatus::Pending;
        job.progress = None;
        job.error = None;
    })
}

fn resegment_sidecar(lines: Vec<SubtitleLine>, preset: &Preset) -> Vec<SubtitleLine> {
    let mut words = Vec::new();
    for line in lines {
        let tokens = line.text.split_whitespace().collect::<Vec<_>>();
        let units = tokens
            .iter()
            .map(|token| token.chars().count().max(1))
            .sum::<usize>()
            .max(1);
        let duration = (line.end - line.start).max(0.02);
        let mut cursor = line.start;
        for (index, token) in tokens.iter().enumerate() {
            let end = if index + 1 == tokens.len() {
                line.end
            } else {
                cursor + duration * (token.chars().count().max(1) as f64 / units as f64)
            };
            words.push(RawWord {
                word: Some((*token).to_owned()),
                start: Some(cursor),
                end: Some(end.max(cursor + 0.02)),
            });
            cursor = end;
        }
    }
    group_transcription_into_lines(
        &TranscriptionResponse {
            text: None,
            segments: None,
            words: Some(words),
        },
        preset.max_chars,
        preset.max_lines,
    )
}

async fn load_sidecar(path: &Path, preset: &Preset) -> Result<Vec<SubtitleLine>> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "srt" => Ok(resegment_sidecar(
            parse_srt_to_lines(&tokio::fs::read_to_string(path).await?),
            preset,
        )),
        "ass" | "ssa" => Ok(resegment_sidecar(
            parse_ass_to_lines(&tokio::fs::read_to_string(path).await?),
            preset,
        )),
        "json" => parse_json_companion(
            &tokio::fs::read(path).await?,
            preset.max_chars,
            preset.max_lines,
        ),
        _ => bail!("unsupported sidecar; use .srt, .ass, .ssa or .json"),
    }
}

pub fn parse_json_companion(
    bytes: &[u8],
    max_chars: u32,
    max_lines: u32,
) -> Result<Vec<SubtitleLine>> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(lines) = value.get("lines")
        && let Ok(lines) = serde_json::from_value::<Vec<SubtitleLine>>(lines.clone())
    {
        return Ok(normalize_subtitles(&lines, NormalizeOptions::default()).lines);
    }
    if let Ok(lines) = serde_json::from_value::<Vec<SubtitleLine>>(value.clone()) {
        return Ok(normalize_subtitles(&lines, NormalizeOptions::default()).lines);
    }
    if let Ok(transcription) = serde_json::from_value::<TranscriptionResponse>(value) {
        return Ok(group_transcription_into_lines(
            &transcription,
            max_chars,
            max_lines,
        ));
    }
    bail!("unsupported subtitle JSON structure")
}

pub async fn discover_companion(video: &Path, preset: &Preset) -> Result<Option<PathBuf>> {
    for ext in ["ass", "ssa", "srt", "json"] {
        let path = video.with_extension(ext);
        if path.exists() {
            let _ = load_sidecar(&path, preset).await?;
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn extract_parenthesized_preset(filename: &str) -> Option<&str> {
    let start = filename.find('(')? + 1;
    let end = filename[start..].find(')')? + start;
    let value = filename[start..end].trim();
    (!value.is_empty()).then_some(value)
}

pub async fn resolve_preset(
    state: &AppState,
    filename: &str,
    workflow: Option<&Workflow>,
    requested: Option<&str>,
) -> Preset {
    let presets = state.presets.read().await.clone();
    let brands = state.brands.read().await.clone();
    if let Some(id) = requested.or_else(|| workflow.and_then(|w| w.preset_id.as_deref()))
        && let Some(p) = presets.iter().find(|p| p.id == id)
    {
        return p.clone();
    }
    if let Some(name) = extract_parenthesized_preset(filename)
        && let Some(p) = presets.iter().find(|p| p.name.eq_ignore_ascii_case(name))
    {
        return p.clone();
    }
    let workflow_brand = workflow.and_then(|w| w.brand_id.as_deref());
    let lower = filename.to_lowercase();
    if let Some(p) = presets.iter().find(|p| {
        (workflow_brand.is_none() || p.brand_id.as_deref() == workflow_brand)
            && p.match_keywords.as_deref().is_some_and(|keywords| {
                keywords
                    .split(',')
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                    .any(|k| lower.contains(&k.to_lowercase()))
            })
    }) {
        return p.clone();
    }
    if let Some(workflow) = workflow
        && let Some(brand_id) = workflow.brand_id.as_deref()
        && let Some(brand) = brands.iter().find(|b| b.id == brand_id)
        && let Some(id) = brand.default_preset_by_format.get(&workflow.format.key)
        && let Some(p) = presets.iter().find(|p| p.id == *id)
    {
        return p.clone();
    }
    presets
        .iter()
        .find(|p| p.name == "Default")
        .cloned()
        .or_else(|| presets.first().cloned())
        .unwrap_or_default()
}

async fn resolve_outro(state: &AppState, preset: &Preset, selection: &JobOutro) -> Option<PathBuf> {
    let brands = state.brands.read().await;
    let brand: Option<&Brand> = preset
        .brand_id
        .as_deref()
        .and_then(|id| brands.iter().find(|b| b.id == id));
    let value = match selection {
        JobOutro::None => return None,
        JobOutro::Asset(asset_id) => asset_id.clone(),
        JobOutro::Inherit => preset
            .outro_video
            .clone()
            .or_else(|| brand.and_then(|b| b.assets.default_outro.clone()))?,
    };
    let direct = PathBuf::from(&value);
    if direct.is_absolute()
        && let Ok(path) = state.config.resolve_allowed_file(&direct)
    {
        return Some(path);
    }

    if let Ok(Some(asset)) = state.db.get::<Asset>("asset", &value)
        && let Ok(path) =
            crate::config::Config::safe_child(&state.config.assets_dir(), &asset.stored_file)
        && path.is_file()
    {
        return Some(path);
    }

    crate::config::Config::safe_child(&state.config.assets_dir(), &value)
        .ok()
        .filter(|path| path.is_file())
}

#[derive(Default)]
struct CleanupFiles {
    paths: Vec<PathBuf>,
}
impl CleanupFiles {
    fn track(&mut self, path: PathBuf) {
        self.paths.push(path);
    }
}
impl Drop for CleanupFiles {
    fn drop(&mut self) {
        for path in &self.paths {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => {
                    tracing::warn!(path = %path.display(), %error, "could not remove temporary file")
                }
            }
        }
    }
}

struct PathReservation {
    path: PathBuf,
    marker: PathBuf,
}
impl Drop for PathReservation {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_file(&self.marker)
            && error.kind() != ErrorKind::NotFound
        {
            tracing::warn!(path = %self.marker.display(), %error, "could not remove path reservation");
        }
    }
}

async fn reserve_candidate(path: PathBuf) -> Result<Option<PathReservation>> {
    if path.exists() {
        return Ok(None);
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.file_name().hash(&mut hasher);
    let marker = path.with_file_name(format!(".autosubs-reserve-{:016x}", hasher.finish()));
    match tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&marker)
        .await
    {
        Ok(_) => {
            if path.exists() {
                let _ = tokio::fs::remove_file(&marker).await;
                Ok(None)
            } else {
                Ok(Some(PathReservation { path, marker }))
            }
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Ok(None),
        Err(error) => Err(error.into()),
    }
}

async fn reserve_output_path(dir: &Path, original_name: &str) -> Result<PathReservation> {
    let stem = Path::new(original_name)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("video");
    for n in 0usize..10_000 {
        let suffix = if n == 0 {
            String::new()
        } else {
            format!(" ({n})")
        };
        if let Some(reservation) =
            reserve_candidate(dir.join(format!("{stem} - ST{suffix}.mp4"))).await?
        {
            return Ok(reservation);
        }
    }
    bail!("could not reserve a unique output filename")
}

async fn reserve_archive_path(dir: &Path, filename: &std::ffi::OsStr) -> Result<PathReservation> {
    let original = Path::new(filename);
    let stem = original
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("source");
    let extension = original.extension().and_then(|v| v.to_str());
    for n in 0usize..10_000 {
        let suffix = if n == 0 {
            String::new()
        } else {
            format!(" ({n})")
        };
        let name = match extension {
            Some(ext) => format!("{stem}{suffix}.{ext}"),
            None => format!("{stem}{suffix}"),
        };
        if let Some(reservation) = reserve_candidate(dir.join(name)).await? {
            return Ok(reservation);
        }
    }
    bail!("could not reserve a unique archive filename")
}

fn partial_path(final_path: &Path) -> PathBuf {
    final_path.with_file_name(format!(".autosubs.partial-{}.mp4", Uuid::new_v4()))
}
fn temp_peer(final_path: &Path) -> PathBuf {
    final_path.with_file_name(format!(".autosubs.partial-{}", Uuid::new_v4()))
}

async fn restore_backups(backups: &[(PathBuf, PathBuf)]) {
    for (backup, original) in backups.iter().rev() {
        if let Err(error) = tokio::fs::rename(backup, original).await {
            tracing::error!(backup = %backup.display(), original = %original.display(), %error, "could not restore output backup");
        }
    }
}

async fn publish_transaction(pairs: &[(PathBuf, PathBuf)]) -> Result<()> {
    let mut backups = Vec::new();
    let mut published: Vec<PathBuf> = Vec::new();
    for (_, final_path) in pairs {
        if final_path.exists() {
            let backup = temp_peer(final_path);
            if let Err(error) = tokio::fs::rename(final_path, &backup).await {
                restore_backups(&backups).await;
                return Err(error)
                    .with_context(|| format!("backup existing output {}", final_path.display()));
            }
            backups.push((backup, final_path.clone()));
        }
    }
    for (staged, final_path) in pairs {
        if let Err(error) = tokio::fs::rename(staged, final_path).await {
            for path in published.iter().rev() {
                if let Err(remove_error) = tokio::fs::remove_file(path).await
                    && remove_error.kind() != ErrorKind::NotFound
                {
                    tracing::error!(path = %path.display(), %remove_error, "could not roll back newly published output");
                }
            }
            restore_backups(&backups).await;
            return Err(error).with_context(|| format!("publish {}", final_path.display()));
        }
        published.push(final_path.clone());
    }
    for (backup, _) in backups {
        if let Err(error) = tokio::fs::remove_file(&backup).await
            && error.kind() != ErrorKind::NotFound
        {
            tracing::warn!(path = %backup.display(), %error, "could not remove obsolete output backup");
        }
    }
    Ok(())
}

async fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        bail!("destination already exists: {}", destination.display());
    }
    match tokio::fs::rename(source, destination).await {
        Ok(()) => Ok(()),
        Err(error) if error.raw_os_error() == Some(18) => {
            let staged = temp_peer(destination);
            let mut cleanup = CleanupFiles::default();
            cleanup.track(staged.clone());
            tokio::fs::copy(source, &staged).await?;
            if destination.exists() {
                bail!(
                    "destination appeared while archiving: {}",
                    destination.display()
                );
            }
            tokio::fs::rename(&staged, destination).await?;
            tokio::fs::remove_file(source).await?;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn error_is_cancelled(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<ProcessError>()
        .is_some_and(|e| matches!(e, ProcessError::Cancelled))
        || error.to_string() == "cancelled"
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_state() -> (tempfile::TempDir, AppState) {
        let root = tempfile::tempdir().unwrap();
        let config = crate::config::Config {
            host: "127.0.0.1".into(),
            port: 0,
            config_dir: root.path().join("config"),
            data_dir: root.path().join("data"),
            fonts_dir: root.path().join("fonts"),
            dist_dir: root.path().join("frontend"),
            allowed_roots: Vec::new(),
            max_render_jobs: 1,
            max_transcription_jobs: 1,
            max_queued_jobs: 2,
            workflow_scan_seconds: 5,
            file_stability_ms: 10,
            max_upload_bytes: 1024,
        };
        (root, AppState::load(config).await.unwrap())
    }

    #[tokio::test]
    async fn saved_corrections_become_the_canonical_regroup_timeline() {
        let (_root, state) = test_state().await;
        let job = create_job(
            &state,
            "clip.mp4".into(),
            PathBuf::from("clip.mp4"),
            None,
            None,
            None,
        )
        .unwrap();
        update_job(&state, &job.id, |job| job.status = JobStatus::Ready).unwrap();
        let mut edited = SubtitleLine {
            id: 0,
            start: 0.0,
            end: 2.0,
            text: "edited layout".into(),
            words: None,
        };
        edited.words = Some(vec![
            crate::domain::SubtitleWord {
                word: "edited".into(),
                start: 0.0,
                end: 1.0,
            },
            crate::domain::SubtitleWord {
                word: "layout".into(),
                start: 1.0,
                end: 2.0,
            },
        ]);
        persist_transcript(
            &state,
            &job.id,
            &TranscriptTimeline {
                words: vec![crate::domain::SubtitleWord {
                    word: "canonical".into(),
                    start: 4.0,
                    end: 5.0,
                }],
                timing_quality: TimingQuality::Exact,
            },
        )
        .unwrap();
        save_subtitles(&state, &job.id, vec![edited]).unwrap();
        let regrouped = regroup_subtitles(&state, &job.id, 25, 2).unwrap();
        assert_eq!(regrouped[0].text, "edited layout");
        assert_eq!((regrouped[0].start, regrouped[0].end), (0.0, 2.0));
        let stored: TranscriptTimeline = state.db.get("job_transcript", &job.id).unwrap().unwrap();
        assert_eq!(stored.words[0].word, "edited");
        assert_eq!(stored.words[1].word, "layout");
    }

    #[tokio::test]
    async fn regroup_legacy_lines_are_migrated_to_canonical_timeline() {
        let (_root, state) = test_state().await;
        let job = create_job(
            &state,
            "clip.mp4".into(),
            PathBuf::from("clip.mp4"),
            None,
            None,
            None,
        )
        .unwrap();
        update_job(&state, &job.id, |job| job.status = JobStatus::Ready).unwrap();
        let line = SubtitleLine {
            id: 0,
            start: 2.0,
            end: 3.0,
            text: "legacy".into(),
            words: Some(vec![crate::domain::SubtitleWord {
                word: "legacy".into(),
                start: 2.0,
                end: 3.0,
            }]),
        };
        update_job(&state, &job.id, |job| job.lines = Some(vec![line])).unwrap();
        regroup_subtitles(&state, &job.id, 25, 2).unwrap();
        let stored: TranscriptTimeline = state.db.get("job_transcript", &job.id).unwrap().unwrap();
        assert_eq!(stored.timing_quality, TimingQuality::Inferred);
        assert_eq!(stored.words[0].word, "legacy");
    }

    #[tokio::test]
    async fn deleting_non_active_job_keeps_media_and_removes_records() {
        let (root, state) = test_state().await;
        let input = root.path().join("source.mp4");
        let output = root.path().join("final.mp4");
        std::fs::write(&input, b"source").unwrap();
        std::fs::write(&output, b"output").unwrap();
        let job = create_job(&state, "source.mp4".into(), input.clone(), None, None, None).unwrap();
        update_job(&state, &job.id, |job| {
            job.status = JobStatus::Ready;
            job.output_path = Some(output.clone());
        })
        .unwrap();
        persist_transcript(
            &state,
            &job.id,
            &TranscriptTimeline {
                words: vec![],
                timing_quality: TimingQuality::Inferred,
            },
        )
        .unwrap();

        delete_job(&state, &job.id).unwrap();

        assert!(get_job(&state, &job.id).is_err());
        assert!(state.db.get::<Job>("job", &job.id).unwrap().is_none());
        assert!(
            state
                .db
                .get::<TranscriptTimeline>("job_transcript", &job.id)
                .unwrap()
                .is_none()
        );
        assert!(input.exists());
        assert!(output.exists());
    }

    #[tokio::test]
    async fn deleting_active_job_conflicts_until_cancelled() {
        let (_root, state) = test_state().await;
        let job = create_job(
            &state,
            "source.mp4".into(),
            PathBuf::from("source.mp4"),
            None,
            None,
            None,
        )
        .unwrap();
        update_job(&state, &job.id, |job| job.status = JobStatus::Transcribing).unwrap();
        assert!(delete_job(&state, &job.id).is_err());
        cancel_job(&state, &job.id).unwrap();
        delete_job(&state, &job.id).unwrap();
    }

    #[tokio::test]
    async fn deleting_job_rejects_non_uuid_stored_id_before_touching_work_files() {
        let (_root, state) = test_state().await;
        let job = create_job(
            &state,
            "source.mp4".into(),
            PathBuf::from("source.mp4"),
            None,
            None,
            None,
        )
        .unwrap();

        let mut poisoned = job.clone();
        state.jobs.remove(&job.id);
        poisoned.id = "../escape".into();
        poisoned.status = JobStatus::Ready;
        state.jobs.insert(poisoned.id.clone(), poisoned);

        let sentinel = state.config.data_dir.join("escape.wav");
        std::fs::write(&sentinel, b"must survive").unwrap();

        assert!(delete_job(&state, "../escape").is_err());
        assert!(sentinel.exists());
    }

    #[tokio::test]
    async fn explicitly_requested_preset_wins_over_filename_rules() {
        let (_root, state) = test_state().await;
        let keyword = Preset {
            id: "keyword".into(),
            name: "Keyword".into(),
            match_keywords: Some("clip".into()),
            max_chars: 42,
            ..Preset::default()
        };
        let selected = Preset {
            id: "selected".into(),
            name: "Selected".into(),
            max_chars: 12,
            max_lines: 1,
            ..Preset::default()
        };
        *state.presets.write().await = vec![keyword, selected];

        let resolved = resolve_preset(&state, "clip.mp4", None, Some("selected")).await;

        assert_eq!(
            (resolved.id.as_str(), resolved.max_chars, resolved.max_lines),
            ("selected", 12, 1)
        );
    }

    #[test]
    fn json_companion_accepts_line_array() {
        let data = br#"[{"id":0,"start":0.0,"end":1.0,"text":"hello"}]"#;
        assert_eq!(parse_json_companion(data, 25, 2).unwrap()[0].text, "hello");
    }
    #[test]
    fn partial_is_not_a_subtitle_extension() {
        let path = Path::new("video.partial");
        let ext = path.extension().and_then(|v| v.to_str()).unwrap();
        assert_ne!(ext, "srt");
        assert_ne!(ext, "ass");
        assert_ne!(ext, "ssa");
        assert_ne!(ext, "json");
    }
}
