use crate::domain::{
    Asset, Brand, Job, JobOutro, JobStatus, Preset, RawWord, RenderProfile, SubtitleLine,
    TimingQuality, TranscriptTimeline, TranscriptionResponse, Workflow, WorkflowOutput,
};
use crate::media::align::{align_timeline, timeline_as_transcription};
use crate::media::process::ProcessError;
use crate::media::transcribe::{TranscriptionError, extract_audio, transcribe_audio};
use crate::media::{build_render_plan, probe_media, render_video, resolve_render_policy};
use crate::render_history::{
    RenderHistory, RenderHistorySample, RenderOptions, RenderProfileOption, estimate_render,
    push_sample,
};
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
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SidecarKind {
    Srt,
    Ass,
    Json,
}

fn workflow_sidecars(output: Option<WorkflowOutput>) -> &'static [SidecarKind] {
    match output {
        None => &[SidecarKind::Srt, SidecarKind::Ass, SidecarKind::Json],
        Some(WorkflowOutput::VideoOnly) => &[],
        Some(WorkflowOutput::VideoSrt) => &[SidecarKind::Srt],
    }
}

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
        effective_preset: None,
        resolved_brand_id: None,
        timing_quality: None,
        timing_fallback: None,
        render_profile: crate::domain::RenderProfile::Auto,
        last_render_encoder: None,
        last_render_elapsed_ms: None,
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
    let job = resolve_and_snapshot_job_preset(state, id, workflow.as_ref()).await?;
    let preset = job
        .effective_preset
        .clone()
        .ok_or_else(|| anyhow!("job has no effective preset after prepare"))?;
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
        let transcription = match result {
            Ok(v) => v,
            Err(TranscriptionError::Cancelled) => {
                let _ = tokio::fs::remove_file(&audio).await;
                bail!("cancelled")
            }
            Err(e) => {
                let _ = tokio::fs::remove_file(&audio).await;
                return Err(e.into());
            }
        };
        let native_timeline = transcript_timeline(&transcription);
        let alignment =
            align_timeline(&audio, &native_timeline, &settings, &state.http, token).await;
        let _ = tokio::fs::remove_file(&audio).await;
        if token.is_cancelled() {
            bail!("cancelled");
        }
        let timing_fallback = alignment.fallback_reason.clone();
        persist_transcript(state, id, &alignment.timeline)?;
        update_job(state, id, move |job| {
            job.timing_fallback = timing_fallback;
        })?;
        let effective_transcription = timeline_as_transcription(&alignment.timeline);
        let raw_path = state.config.work_dir().join(format!("{id}_words.json"));
        tokio::fs::write(
            &raw_path,
            serde_json::to_vec_pretty(&effective_transcription)?,
        )
        .await?;
        let (output_width, output_height) = preset
            .format
            .resolution(Some((probe.width, probe.height)))
            .unwrap_or((probe.width, probe.height));
        let lines = crate::subtitle::group_transcription_into_lines_with_layout(
            &effective_transcription,
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
    let mut message = format!("{error:#}");
    if message.len() > 4_000 {
        let tail_start = message
            .char_indices()
            .rev()
            .nth(3_199)
            .map(|(index, _)| index)
            .unwrap_or(0);
        message = format!("{} … {}", error, &message[tail_start..]);
    }
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

pub async fn render_options(state: &AppState, id: &str) -> Result<RenderOptions> {
    let job = get_job(state, id)?;
    let input = job
        .input_path
        .clone()
        .ok_or_else(|| anyhow!("job has no input"))?;
    let preset = job
        .effective_preset
        .clone()
        .ok_or_else(|| anyhow!("job has no effective preset"))?;
    let token = CancellationToken::new();
    let source = probe_media(&input, &token)
        .await
        .context("probe render options input")?;
    let outro = resolve_outro(state, &preset, &job.outro).await;
    let outro_duration = if let Some(path) = outro {
        probe_media(&path, &token)
            .await
            .map(|probe| probe.duration)
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let media_duration = (source.duration + outro_duration).max(0.1);
    let target = preset
        .format
        .resolution(Some((source.width, source.height)))
        .unwrap_or((source.width, source.height));
    let target_pixels = u64::from(target.0.max(1)) * u64::from(target.1.max(1));
    let caps = state.encoders.read().await.clone();
    let settings = state.settings.read().await.clone();
    let history = state
        .db
        .get_singleton::<RenderHistory>("render_history")?
        .unwrap_or_default();
    let options = [
        RenderProfile::Auto,
        RenderProfile::Fast,
        RenderProfile::Quality,
        RenderProfile::Compact,
    ]
    .into_iter()
    .map(|profile| {
        let policy = resolve_render_policy(profile, &settings.encoder, &caps);
        let encoder = policy.encoder.kind;
        let estimate = estimate_render(
            profile,
            &encoder,
            media_duration,
            target_pixels,
            &caps,
            &history,
        );
        RenderProfileOption {
            profile,
            encoder,
            estimate,
        }
    })
    .collect();

    Ok(RenderOptions {
        options,
        actual_encoder: job.last_render_encoder,
        last_elapsed_ms: job.last_render_elapsed_ms,
    })
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
    let mut job = get_job(state, id)?;
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
    let sidecar_kinds = workflow_sidecars(workflow.as_ref().map(|w| w.output_mode));
    let mut archive_candidates = if job.archive_after_success && workflow.is_some() {
        collect_source_bundle(&input)
            .await
            .context("collect source bundle before render")?
    } else {
        Vec::new()
    };
    if let Some(sidecar) = job.attached_sidecar.as_ref()
        && sidecar.exists()
        && !archive_candidates.contains(sidecar)
    {
        archive_candidates.push(sidecar.clone());
    }
    if job.effective_preset.is_none() {
        job = resolve_and_snapshot_job_preset(state, id, workflow.as_ref()).await?;
    }
    let preset = job
        .effective_preset
        .clone()
        .ok_or_else(|| anyhow!("job has no effective preset to render"))?;
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
    let policy = resolve_render_policy(job.render_profile, &settings.encoder, &caps);
    let render_order = policy.fallback_order.clone();
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
    let render_started = Instant::now();
    let mut result = Ok(());
    let mut used_encoder = None;
    let mut successful_elapsed_ms = 0_u64;
    let mut target_pixels = u64::from(source.width) * u64::from(source.height);
    let mut fallback_count = 0_u32;
    let mut fallback_overhead_ms = 0_u64;
    for (attempt, encoder_kind) in render_order.iter().cloned().enumerate() {
        let mut encoder = policy.encoder.clone();
        encoder.kind = encoder_kind.clone();
        let plan = build_render_plan(
            &input,
            &staging_video,
            &work_ass,
            &preset,
            &encoder,
            &caps,
            &source,
            outro.as_deref().zip(outro_probe.as_ref()),
            Some(&state.config.fonts_dir),
        )?;
        target_pixels = u64::from(plan.target_resolution.0) * u64::from(plan.target_resolution.1);
        let attempt_started = Instant::now();
        match render_video(&plan, duration, token, Some(progress_tx.clone())).await {
            Ok(()) => {
                successful_elapsed_ms =
                    u64::try_from(attempt_started.elapsed().as_millis()).unwrap_or(u64::MAX);
                used_encoder = Some(plan.encoder.clone());
                result = Ok(());
                break;
            }
            Err(error) => {
                fallback_count = fallback_count.saturating_add(1);
                fallback_overhead_ms = fallback_overhead_ms.saturating_add(
                    u64::try_from(attempt_started.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
                let is_last = attempt + 1 == render_order.len();
                if token.is_cancelled() || is_last {
                    result = Err(error);
                    break;
                }
                tracing::warn!(
                    job_id = id,
                    encoder = ?encoder_kind,
                    next_encoder = ?render_order[attempt + 1],
                    error = %error,
                    "render backend failed; trying next validated encoder"
                );
            }
        }
    }
    progress_task.abort();
    result.map_err(|e| anyhow!(e)).context("render video")?;
    let used_encoder = used_encoder.ok_or_else(|| anyhow!("render completed without encoder"))?;
    let total_render_elapsed_ms =
        u64::try_from(render_started.elapsed().as_millis()).unwrap_or(u64::MAX);
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
    let mut publish_pairs = vec![(staging_video.clone(), final_video.clone())];
    for kind in sidecar_kinds {
        let final_sidecar = match kind {
            SidecarKind::Srt => final_video.with_file_name(format!("{stem}.srt")),
            SidecarKind::Ass => final_video.with_file_name(format!("{stem}.ass")),
            SidecarKind::Json => final_video.with_file_name(format!("{stem}.json")),
        };
        let staged = temp_peer(&final_sidecar);
        cleanup.track(staged.clone());
        match kind {
            SidecarKind::Srt => {
                tokio::fs::write(&staged, generate_srt_content(&lines)).await?;
            }
            SidecarKind::Ass => {
                tokio::fs::write(&staged, ass.as_bytes()).await?;
            }
            SidecarKind::Json => {
                tokio::fs::write(&staged, serde_json::to_vec_pretty(&lines)?).await?;
            }
        }
        publish_pairs.push((staged, final_sidecar));
    }
    publish_transaction(&publish_pairs).await?;

    let mut effective_input = input.clone();
    if job.archive_after_success
        && let Some(workflow) = workflow.as_ref()
    {
        let archive_dir = state
            .config
            .resolve_allowed_dir(Path::new(&workflow.archive_dir))
            .context("validate workflow archive directory")?;
        effective_input = archive_source_bundle(&archive_dir, &input, &archive_candidates)
            .await
            .context("archive source bundle after successful render")?;
    }
    let mut history = state
        .db
        .get_singleton::<RenderHistory>("render_history")?
        .unwrap_or_default();
    push_sample(
        &mut history,
        RenderHistorySample {
            id: Uuid::new_v4().to_string(),
            profile: job.render_profile,
            encoder: used_encoder.clone(),
            media_duration_seconds: duration,
            target_pixels,
            elapsed_ms: successful_elapsed_ms,
            successful: true,
            fallback_count,
            fallback_overhead_ms,
            created_at_ms: now_ms(),
        },
    );
    state.db.set_singleton("render_history", &history)?;

    update_job(state, id, move |job| {
        job.status = JobStatus::Done;
        job.progress = Some(100);
        job.error = None;
        job.output_path = Some(final_video);
        job.input_path = Some(effective_input);
        job.archive_after_success = false;
        job.last_render_encoder = Some(used_encoder);
        job.last_render_elapsed_ms = Some(total_render_elapsed_ms);
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
    state.db.upsert("job_transcript", id, timeline)?;
    if state.jobs.contains_key(id) {
        let quality = timeline.timing_quality;
        update_job(state, id, move |job| {
            job.timing_quality = Some(quality);
        })?;
    }
    Ok(())
}

pub async fn apply_preset_to_job(state: &AppState, id: &str, preset_id: &str) -> Result<Job> {
    let current = get_job(state, id)?;
    if current.status.is_active() {
        bail!("cannot apply a preset while the job is active");
    }
    if !state
        .presets
        .read()
        .await
        .iter()
        .any(|preset| preset.id == preset_id)
    {
        bail!("unknown preset: {preset_id}");
    }
    let workflow = if let Some(workflow_id) = current.workflow_id.as_deref() {
        state
            .workflows
            .read()
            .await
            .iter()
            .find(|workflow| workflow.id == workflow_id)
            .cloned()
    } else {
        None
    };
    let preset_id = preset_id.to_owned();
    update_job(state, id, move |job| {
        job.preset_id = Some(preset_id);
    })?;
    let snapshotted = resolve_and_snapshot_job_preset(state, id, workflow.as_ref()).await?;
    let preset = snapshotted
        .effective_preset
        .clone()
        .ok_or_else(|| anyhow!("job has no effective preset after applying preset"))?;
    let timeline = state
        .db
        .get::<TranscriptTimeline>("job_transcript", id)?
        .ok_or_else(|| anyhow!("job has no canonical word timing to resegment"))?;
    if timeline.words.is_empty() {
        bail!("job has no canonical word timing to resegment");
    }
    let transcription = TranscriptionResponse {
        text: None,
        words: Some(
            timeline
                .words
                .iter()
                .map(|word| RawWord {
                    word: Some(word.word.clone()),
                    start: Some(word.start),
                    end: Some(word.end),
                })
                .collect(),
        ),
        segments: None,
    };
    let lines = if let Some(input) = snapshotted
        .input_path
        .as_ref()
        .filter(|path| path.is_file())
    {
        let token = CancellationToken::new();
        match probe_media(input, &token).await {
            Ok(probe) => {
                let (output_width, output_height) = preset
                    .format
                    .resolution(Some((probe.width, probe.height)))
                    .unwrap_or((probe.width, probe.height));
                crate::subtitle::group_transcription_into_lines_with_layout(
                    &transcription,
                    LayoutOptions {
                        max_chars: preset.max_chars,
                        max_lines: preset.max_lines,
                        output_width,
                        font_size: scale_ass_metric(preset.size, output_height),
                    },
                )
            }
            Err(_) => {
                group_transcription_into_lines(&transcription, preset.max_chars, preset.max_lines)
            }
        }
    } else {
        group_transcription_into_lines(&transcription, preset.max_chars, preset.max_lines)
    };
    save_subtitles(state, id, lines)?;
    get_job(state, id)
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
    update_job(state, id, |job| {
        if let Some(preset) = job.effective_preset.as_mut() {
            preset.max_chars = max_chars;
            preset.max_lines = max_lines;
        }
    })?;
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

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPreset {
    pub preset: Preset,
    pub brand_id: Option<String>,
}

fn keyword_match_score(filename: &str, keywords: Option<&str>) -> Option<usize> {
    let lower = filename.to_lowercase();
    keywords?
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter_map(|keyword| {
            let normalized = keyword.to_lowercase();
            lower
                .contains(&normalized)
                .then_some(normalized.chars().count())
        })
        .max()
}

fn resolve_brand(
    brands: &[Brand],
    filename: &str,
    workflow: Option<&Workflow>,
) -> Result<Option<Brand>> {
    if let Some(id) = workflow.and_then(|value| value.brand_id.as_deref()) {
        return brands
            .iter()
            .find(|brand| brand.id == id)
            .cloned()
            .map(Some)
            .ok_or_else(|| anyhow!("unknown workflow brand: {id}"));
    }

    let mut matches = brands
        .iter()
        .filter_map(|brand| {
            keyword_match_score(filename, brand.match_keywords.as_deref())
                .map(|score| (score, brand))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| left.id.cmp(&right.id))
    });
    let Some((best_score, best)) = matches.first().copied() else {
        return Ok(None);
    };
    if matches
        .iter()
        .skip(1)
        .any(|(score, brand)| *score == best_score && brand.id != best.id)
    {
        bail!("ambiguous brand keyword match for {filename}");
    }
    Ok(Some(best.clone()))
}

pub async fn resolve_effective_preset(
    state: &AppState,
    filename: &str,
    workflow: Option<&Workflow>,
    requested: Option<&str>,
) -> Result<ResolvedPreset> {
    let presets = state.presets.read().await.clone();
    let brands = state.brands.read().await.clone();
    let brand = resolve_brand(&brands, filename, workflow)?;

    let explicit_id = requested.or_else(|| workflow.and_then(|value| value.preset_id.as_deref()));
    let mut preset =
        explicit_id.and_then(|id| presets.iter().find(|preset| preset.id == id).cloned());

    if preset.is_none()
        && let Some(name) = extract_parenthesized_preset(filename)
    {
        preset = presets
            .iter()
            .find(|candidate| candidate.name.eq_ignore_ascii_case(name))
            .cloned();
    }

    if preset.is_none() {
        preset = presets
            .iter()
            .filter_map(|candidate| {
                keyword_match_score(filename, candidate.match_keywords.as_deref())
                    .map(|score| (score, candidate))
            })
            .max_by_key(|(score, _)| *score)
            .map(|(_, candidate)| candidate.clone());
    }

    if preset.is_none()
        && let Some(brand) = brand.as_ref()
    {
        let format_key = workflow
            .map(|value| value.format.key)
            .unwrap_or(crate::domain::FormatKey::Source);
        preset = brand
            .default_preset_by_format
            .get(&format_key)
            .and_then(|id| presets.iter().find(|candidate| candidate.id == *id))
            .cloned();
    }

    let mut preset = preset
        .or_else(|| {
            presets
                .iter()
                .find(|candidate| candidate.name == "Default")
                .cloned()
        })
        .or_else(|| presets.first().cloned())
        .unwrap_or_default();

    let brand_id = brand.as_ref().map(|value| value.id.clone());
    if let Some(brand) = brand {
        preset.brand_id = Some(brand.id);
        if let Some(color) = brand
            .highlight_color
            .filter(|value| !value.trim().is_empty())
        {
            preset.highlight_color = color;
        }
    }

    Ok(ResolvedPreset { preset, brand_id })
}

pub async fn resolve_preset(
    state: &AppState,
    filename: &str,
    workflow: Option<&Workflow>,
    requested: Option<&str>,
) -> Preset {
    resolve_effective_preset(state, filename, workflow, requested)
        .await
        .map(|resolved| resolved.preset)
        .unwrap_or_default()
}

pub async fn resolve_and_snapshot_job_preset(
    state: &AppState,
    id: &str,
    workflow: Option<&Workflow>,
) -> Result<Job> {
    let job = get_job(state, id)?;
    let resolved = resolve_effective_preset(
        state,
        &job.original_name,
        workflow,
        job.preset_id.as_deref(),
    )
    .await?;
    let preset = resolved.preset;
    let format = preset.format.clone();
    let brand_id = resolved.brand_id;
    update_job(state, id, move |job| {
        job.format = format;
        job.resolved_brand_id = brand_id;
        job.effective_preset = Some(preset);
    })
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

fn bundle_name_matches(stem: &str, name: &str) -> bool {
    let Some(rest) = name.strip_prefix(stem) else {
        return false;
    };
    rest.is_empty()
        || rest
            .chars()
            .next()
            .is_some_and(|c| matches!(c, '.' | '_' | '-' | ' ' | '(' | '['))
}

fn internal_source_bundle_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.contains(".partial-")
        || lower.ends_with(".uploading")
        || lower.ends_with("_words.json")
        || lower.starts_with(".autosubs-reserve-")
}

async fn collect_source_bundle(input: &Path) -> Result<Vec<PathBuf>> {
    let parent = input
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("source has no parent directory"))?;
    let stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("source stem is not valid UTF-8"))?;
    let mut dir = tokio::fs::read_dir(parent).await?;
    let mut paths = Vec::new();
    while let Some(entry) = dir.next_entry().await? {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !bundle_name_matches(stem, name) || internal_source_bundle_name(name) {
            continue;
        }
        if entry.file_type().await?.is_file() {
            paths.push(entry.path());
        }
    }
    if input.exists() && !paths.iter().any(|path| path == input) {
        paths.push(input.to_path_buf());
    }
    paths.sort();
    Ok(paths)
}

async fn rollback_archived_files(moved: &[(PathBuf, PathBuf)]) {
    for (source, archived) in moved.iter().rev() {
        if let Err(error) = move_file(archived, source).await {
            tracing::error!(
                source = %source.display(),
                archived = %archived.display(),
                %error,
                "could not roll back archived source companion"
            );
        }
    }
}

async fn archive_source_bundle(
    archive_dir: &Path,
    input: &Path,
    candidates: &[PathBuf],
) -> Result<PathBuf> {
    let mut ordered = candidates.to_vec();
    ordered.sort();
    ordered.dedup();
    ordered.sort_by_key(|path| path == input);

    let mut moved = Vec::new();
    let mut archived_input = None;
    for source in ordered {
        if !source.exists() {
            if source == input {
                rollback_archived_files(&moved).await;
                bail!("source disappeared before archive: {}", source.display());
            }
            continue;
        }
        let name = source
            .file_name()
            .ok_or_else(|| anyhow!("archive candidate has no filename"))?;
        let reservation = reserve_archive_path(archive_dir, name).await?;
        let destination = reservation.path.clone();
        if let Err(error) = move_file(&source, &destination).await {
            rollback_archived_files(&moved).await;
            return Err(error).with_context(|| format!("archive {}", source.display()));
        }
        if source == input {
            archived_input = Some(destination.clone());
        }
        moved.push((source, destination));
    }
    archived_input.ok_or_else(|| anyhow!("source was not present in archive bundle"))
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
    use crate::domain::{FormatKey, FormatProfile};

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
    async fn persisted_timeline_updates_job_timing_provenance() {
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

        persist_transcript(
            &state,
            &job.id,
            &TranscriptTimeline {
                words: vec![crate::domain::SubtitleWord {
                    word: "test".into(),
                    start: 0.0,
                    end: 0.5,
                }],
                timing_quality: TimingQuality::Aligned,
            },
        )
        .unwrap();

        let stored = get_job(&state, &job.id).unwrap();
        assert_eq!(stored.timing_quality, Some(TimingQuality::Aligned));
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

    #[tokio::test]
    async fn explicit_generic_preset_keeps_layout_while_brand_keyword_overrides_highlight() {
        let (_root, state) = test_state().await;
        let generic = Preset {
            id: "generic".into(),
            name: "Generic 9:16".into(),
            max_chars: 18,
            max_lines: 1,
            highlight_color: "#112233".into(),
            ..Preset::default()
        };
        let brand_default = Preset {
            id: "brand-default".into(),
            name: "Brand default".into(),
            max_chars: 40,
            ..Preset::default()
        };
        let brand = Brand {
            id: "chougar".into(),
            name: "Chougar".into(),
            description: String::new(),
            assets: Default::default(),
            preset_ids: vec!["brand-default".into()],
            default_preset_by_format: [(FormatKey::Source, "brand-default".into())]
                .into_iter()
                .collect(),
            match_keywords: Some("chougar,cf".into()),
            highlight_color: Some("#ff00aa".into()),
        };
        *state.presets.write().await = vec![generic, brand_default];
        *state.brands.write().await = vec![brand];

        let resolved =
            resolve_effective_preset(&state, "CHougar_episode.mp4", None, Some("generic"))
                .await
                .unwrap();

        assert_eq!(resolved.preset.id, "generic");
        assert_eq!(resolved.preset.max_chars, 18);
        assert_eq!(resolved.preset.max_lines, 1);
        assert_eq!(resolved.preset.highlight_color, "#ff00aa");
        assert_eq!(resolved.brand_id.as_deref(), Some("chougar"));
    }

    #[tokio::test]
    async fn equal_specificity_brand_keyword_matches_are_rejected() {
        let (_root, state) = test_state().await;
        *state.presets.write().await = vec![Preset {
            id: "generic".into(),
            name: "Generic".into(),
            ..Preset::default()
        }];
        *state.brands.write().await = vec![
            Brand {
                id: "a".into(),
                name: "A".into(),
                description: String::new(),
                assets: Default::default(),
                preset_ids: Vec::new(),
                default_preset_by_format: Default::default(),
                match_keywords: Some("show".into()),
                highlight_color: None,
            },
            Brand {
                id: "b".into(),
                name: "B".into(),
                description: String::new(),
                assets: Default::default(),
                preset_ids: Vec::new(),
                default_preset_by_format: Default::default(),
                match_keywords: Some("show".into()),
                highlight_color: None,
            },
        ];

        let error = resolve_effective_preset(&state, "show-42.mp4", None, Some("generic"))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("ambiguous brand"));
    }

    #[tokio::test]
    async fn snapshotting_explicit_manual_preset_persists_complete_render_and_layout_config() {
        let (_root, state) = test_state().await;
        let preset = Preset {
            id: "selected".into(),
            name: "Selected".into(),
            format: FormatProfile {
                key: FormatKey::Portrait916,
                fit: crate::domain::FitMode::Cover,
                width: None,
                height: None,
            },
            max_chars: 14,
            max_lines: 1,
            size: 37.0,
            animation_style: crate::domain::AnimationStyle::WordByWord,
            ..Preset::default()
        };
        *state.presets.write().await = vec![preset];

        let job = create_job(
            &state,
            "manual.mp4".into(),
            PathBuf::from("manual.mp4"),
            None,
            Some("selected".into()),
            None,
        )
        .unwrap();

        let snapshotted = resolve_and_snapshot_job_preset(&state, &job.id, None)
            .await
            .unwrap();
        let effective = snapshotted.effective_preset.expect("effective preset");

        assert_eq!(effective.id, "selected");
        assert_eq!(effective.max_chars, 14);
        assert_eq!(effective.max_lines, 1);
        assert_eq!(effective.size, 37.0);
        assert_eq!(
            effective.animation_style,
            crate::domain::AnimationStyle::WordByWord
        );
        assert_eq!(effective.format.key, FormatKey::Portrait916);
        assert_eq!(snapshotted.format.key, FormatKey::Portrait916);
    }

    #[tokio::test]
    async fn regroup_updates_the_effective_preset_segmentation_limits() {
        let (_root, state) = test_state().await;
        let effective = Preset {
            id: "snapshot".into(),
            ..Preset::default()
        };
        let job = create_job(
            &state,
            "manual.mp4".into(),
            PathBuf::from("manual.mp4"),
            None,
            None,
            None,
        )
        .unwrap();
        update_job(&state, &job.id, |job| {
            job.status = JobStatus::Ready;
            job.effective_preset = Some(effective);
        })
        .unwrap();
        persist_transcript(
            &state,
            &job.id,
            &TranscriptTimeline {
                words: vec![
                    crate::domain::SubtitleWord {
                        word: "un".into(),
                        start: 0.0,
                        end: 0.4,
                    },
                    crate::domain::SubtitleWord {
                        word: "deux".into(),
                        start: 0.5,
                        end: 1.0,
                    },
                ],
                timing_quality: TimingQuality::Exact,
            },
        )
        .unwrap();

        regroup_subtitles(&state, &job.id, 4, 1).unwrap();

        let updated = get_job(&state, &job.id).unwrap();
        let effective = updated.effective_preset.unwrap();
        assert_eq!(effective.max_chars, 4);
        assert_eq!(effective.max_lines, 1);
    }

    #[tokio::test]
    async fn applying_a_preset_to_a_ready_job_resegments_from_the_canonical_timeline() {
        let (_root, state) = test_state().await;
        *state.presets.write().await = vec![Preset {
            id: "tight".into(),
            name: "Tight".into(),
            max_chars: 4,
            max_lines: 1,
            ..Preset::default()
        }];
        let job = create_job(
            &state,
            "manual.mp4".into(),
            PathBuf::from("manual.mp4"),
            None,
            None,
            None,
        )
        .unwrap();
        update_job(&state, &job.id, |job| job.status = JobStatus::Ready).unwrap();
        persist_transcript(
            &state,
            &job.id,
            &TranscriptTimeline {
                words: vec![
                    crate::domain::SubtitleWord {
                        word: "un".into(),
                        start: 0.0,
                        end: 0.4,
                    },
                    crate::domain::SubtitleWord {
                        word: "deux".into(),
                        start: 0.5,
                        end: 1.0,
                    },
                ],
                timing_quality: TimingQuality::Exact,
            },
        )
        .unwrap();

        let updated = apply_preset_to_job(&state, &job.id, "tight").await.unwrap();

        assert_eq!(updated.effective_preset.as_ref().unwrap().id, "tight");
        assert_eq!(updated.effective_preset.as_ref().unwrap().max_chars, 4);
        assert_eq!(updated.lines.as_ref().unwrap().len(), 2);
        assert_eq!(updated.lines.as_ref().unwrap()[0].text, "un");
        assert_eq!(updated.lines.as_ref().unwrap()[1].text, "deux");
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

    #[test]
    fn workflow_output_policy_is_explicit_and_manual_keeps_full_exports() {
        assert_eq!(
            workflow_sidecars(None),
            &[SidecarKind::Srt, SidecarKind::Ass, SidecarKind::Json]
        );
        assert_eq!(workflow_sidecars(Some(WorkflowOutput::VideoOnly)), &[]);
        assert_eq!(
            workflow_sidecars(Some(WorkflowOutput::VideoSrt)),
            &[SidecarKind::Srt]
        );
    }

    #[test]
    fn source_bundle_matching_uses_stem_boundaries_not_raw_prefixes() {
        for name in [
            "clip.mp4",
            "clip.srt",
            "clip.ass",
            "clip - cover.jpg",
            "clip_notes.txt",
            "clip(backup).mov",
        ] {
            assert!(bundle_name_matches("clip", name), "{name}");
        }
        for name in ["clip2.mp4", "clipping.mov", "clipper.srt"] {
            assert!(!bundle_name_matches("clip", name), "{name}");
        }
    }

    #[tokio::test]
    async fn source_bundle_archive_moves_related_files_and_keeps_prefix_collisions() {
        let root = tempfile::tempdir().unwrap();
        let source_dir = root.path().join("watch");
        let archive_dir = root.path().join("archive");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&archive_dir).unwrap();
        for name in [
            "clip.mp4",
            "clip.srt",
            "clip - cover.jpg",
            "clip_notes.txt",
            "clip2.mp4",
        ] {
            std::fs::write(source_dir.join(name), name.as_bytes()).unwrap();
        }
        let input = source_dir.join("clip.mp4");
        let candidates = collect_source_bundle(&input).await.unwrap();
        assert_eq!(candidates.len(), 4);
        let archived = archive_source_bundle(&archive_dir, &input, &candidates)
            .await
            .unwrap();
        assert_eq!(archived, archive_dir.join("clip.mp4"));
        for name in ["clip.mp4", "clip.srt", "clip - cover.jpg", "clip_notes.txt"] {
            assert!(archive_dir.join(name).is_file(), "{name}");
            assert!(!source_dir.join(name).exists(), "{name}");
        }
        assert!(source_dir.join("clip2.mp4").is_file());
    }
}
