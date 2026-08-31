use crate::domain::{
    Asset, Brand, EncoderKind, Job, JobStatus, Preset, RawWord, SubtitleLine,
    TranscriptionResponse, Workflow,
};
use crate::media::process::ProcessError;
use crate::media::transcribe::{TranscriptionError, extract_audio, transcribe_audio};
use crate::media::{build_render_plan, probe_media, render_video};
use crate::state::AppState;
use crate::subtitle::{
    NormalizeOptions,
    ass::generate_ass_content,
    group_transcription_into_lines,
    llm::correct_lines,
    normalize_subtitles,
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
        if let Err(error) = prepare_job(&state, &id, &token).await {
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

async fn prepare_job(state: &AppState, id: &str, token: &CancellationToken) -> Result<()> {
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
    let _probe = probe_media(&input, token).await.context("probe video")?;
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
    let lines = if let Some(sidecar) = job.attached_sidecar.clone() {
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
        let raw_path = state.config.work_dir().join(format!("{id}_words.json"));
        tokio::fs::write(&raw_path, serde_json::to_vec_pretty(&transcription)?).await?;
        let lines =
            group_transcription_into_lines(&transcription, preset.max_chars, preset.max_lines);
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
    let outro = resolve_outro(state, &preset).await;
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
    update_job(state, id, move |job| {
        job.lines = Some(saved);
        job.status = JobStatus::Ready;
        job.progress = Some(100);
        job.error = None;
    })?;
    Ok(report)
}

pub fn regroup_subtitles(
    state: &AppState,
    id: &str,
    max_chars: u32,
    max_lines: u32,
) -> Result<Vec<SubtitleLine>> {
    let job = get_job(state, id)?;
    let words = job
        .lines
        .as_ref()
        .into_iter()
        .flatten()
        .flat_map(|line| line.words.clone().unwrap_or_default())
        .map(|w| RawWord {
            word: Some(w.word),
            start: Some(w.start),
            end: Some(w.end),
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        bail!("job has no word timing to regroup");
    }
    let lines = group_transcription_into_lines(
        &TranscriptionResponse {
            text: None,
            words: Some(words),
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

async fn load_sidecar(path: &Path, preset: &Preset) -> Result<Vec<SubtitleLine>> {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "srt" => Ok(parse_srt_to_lines(&tokio::fs::read_to_string(path).await?)),
        "ass" | "ssa" => Ok(parse_ass_to_lines(&tokio::fs::read_to_string(path).await?)),
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
    if let Some(id) = requested.or_else(|| workflow.and_then(|w| w.preset_id.as_deref()))
        && let Some(p) = presets.iter().find(|p| p.id == id)
    {
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

async fn resolve_outro(state: &AppState, preset: &Preset) -> Option<PathBuf> {
    let brands = state.brands.read().await;
    let brand: Option<&Brand> = preset
        .brand_id
        .as_deref()
        .and_then(|id| brands.iter().find(|b| b.id == id));
    let value = preset
        .outro_video
        .clone()
        .or_else(|| brand.and_then(|b| b.assets.default_outro.clone()))?;
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
