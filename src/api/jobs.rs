use crate::{
    domain::{FormatProfile, Job, SubtitleLine},
    error::{AppError, AppResult},
    format::normalize_format_profile,
    jobs,
    media::probe_media,
    state::AppState,
};
use axum::{
    Json,
    body::Bytes,
    extract::{Multipart, Path, State},
};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use tokio_util::sync::CancellationToken;

pub async fn list_jobs(State(state): State<AppState>) -> Json<Vec<Job>> {
    let mut jobs: Vec<Job> = state
        .jobs
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    jobs.sort_by_key(|job| std::cmp::Reverse(job.created_at_ms));
    Json(jobs)
}

pub async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Job>> {
    jobs::get_job(&state, &id)
        .map(Json)
        .map_err(|_| AppError::NotFound("job not found".into()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobOptions {
    #[serde(default)]
    preset_id: Option<String>,
    #[serde(default)]
    format: Option<FormatProfile>,
}
pub async fn update_job_options(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<JobOptions>,
) -> AppResult<Json<Job>> {
    let current =
        jobs::get_job(&state, &id).map_err(|_| AppError::NotFound("job not found".into()))?;
    if current.status.is_active() {
        return Err(AppError::Conflict(
            "cannot change job options while it is active".into(),
        ));
    }
    if let Some(preset_id) = body.preset_id.as_deref()
        && !state.presets.read().await.iter().any(|p| p.id == preset_id)
    {
        return Err(AppError::BadRequest("unknown presetId".into()));
    }
    let preset_id = body.preset_id;
    let mut format = body.format;
    if let Some(profile) = format.as_mut() {
        normalize_format_profile(profile).map_err(AppError::BadRequest)?;
    }
    jobs::update_job(&state, &id, move |job| {
        job.preset_id = preset_id;
        if let Some(format) = format {
            job.format = format;
        }
    })
    .map(Json)
    .map_err(AppError::Internal)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FromPath {
    path: String,
    sidecar_path: Option<String>,
    preset_id: Option<String>,
}

pub async fn create_from_path(
    State(state): State<AppState>,
    Json(body): Json<FromPath>,
) -> AppResult<Json<Job>> {
    let input = PathBuf::from(&body.path);
    if !input.is_file() || !state.config.path_is_allowed(&input) {
        return Err(AppError::Forbidden(
            "video path is outside allowed roots or not a file".into(),
        ));
    }
    let sidecar = body.sidecar_path.map(PathBuf::from);
    if let Some(path) = &sidecar {
        if !path.is_file() || !state.config.path_is_allowed(path) {
            return Err(AppError::Forbidden(
                "sidecar path is outside allowed roots or not a file".into(),
            ));
        }
        validate_sidecar_extension(path)?;
    }
    let original = input
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| AppError::BadRequest("invalid filename".into()))?
        .to_owned();
    let requested_preset = body.preset_id.clone();
    let requested_format = if let Some(preset_id) = requested_preset.as_deref() {
        Some(
            state
                .presets
                .read()
                .await
                .iter()
                .find(|preset| preset.id == preset_id)
                .map(|preset| preset.format.clone())
                .ok_or_else(|| AppError::BadRequest("unknown presetId".into()))?,
        )
    } else {
        None
    };
    let mut job = jobs::create_job(&state, original, input, sidecar, requested_preset, None)
        .map_err(AppError::Internal)?;
    if let Some(format) = requested_format {
        job = jobs::update_job(&state, &job.id, move |job| job.format = format)
            .map_err(AppError::Internal)?;
    }
    jobs::enqueue_prepare(state.clone(), job.id.clone())
        .map_err(|e| AppError::Conflict(e.to_string()))?;
    Ok(Json(job))
}

pub async fn prepare(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let _ = jobs::get_job(&state, &id).map_err(|_| AppError::NotFound("job not found".into()))?;
    jobs::enqueue_prepare(state, id).map_err(|e| AppError::Conflict(e.to_string()))?;
    Ok(Json(json!({"accepted":true})))
}
pub async fn render(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    jobs::enqueue_render(state, id).map_err(|e| AppError::Conflict(e.to_string()))?;
    Ok(Json(json!({"accepted":true})))
}
pub async fn cancel(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<Json<Job>> {
    jobs::cancel_job(&state, &id)
        .map(Json)
        .map_err(|_| AppError::NotFound("job not found".into()))
}

pub async fn get_subtitles(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<SubtitleLine>>> {
    let job = jobs::get_job(&state, &id).map_err(|_| AppError::NotFound("job not found".into()))?;
    Ok(Json(job.lines.unwrap_or_default()))
}
pub async fn save_subtitles(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(lines): Json<Vec<SubtitleLine>>,
) -> AppResult<Json<crate::subtitle::NormalizationReport>> {
    jobs::save_subtitles(&state, &id, lines)
        .map(Json)
        .map_err(|e| AppError::Conflict(e.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Regroup {
    max_chars: u32,
    max_lines: u32,
}
pub async fn regroup(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<Regroup>,
) -> AppResult<Json<Vec<SubtitleLine>>> {
    jobs::regroup_subtitles(&state, &id, body.max_chars, body.max_lines)
        .map(Json)
        .map_err(|e| AppError::Conflict(e.to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidecarPath {
    path: String,
}
pub async fn set_sidecar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SidecarPath>,
) -> AppResult<Json<Job>> {
    let path = PathBuf::from(body.path);
    if !path.is_file() || !state.config.path_is_allowed(&path) {
        return Err(AppError::Forbidden(
            "sidecar path is outside allowed roots".into(),
        ));
    }
    validate_sidecar_extension(&path)?;
    let job = jobs::attach_sidecar(&state, &id, Some(path))
        .map_err(|e| AppError::Conflict(e.to_string()))?;
    jobs::enqueue_prepare(state, id).map_err(|e| AppError::Conflict(e.to_string()))?;
    Ok(Json(job))
}
pub async fn remove_sidecar(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Job>> {
    let job =
        jobs::attach_sidecar(&state, &id, None).map_err(|e| AppError::Conflict(e.to_string()))?;
    jobs::enqueue_prepare(state, id).map_err(|e| AppError::Conflict(e.to_string()))?;
    Ok(Json(job))
}

pub async fn upload_sidecar(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> AppResult<Json<Job>> {
    let mut uploaded: Option<PathBuf> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let name = field.file_name().unwrap_or("subtitles.srt").to_owned();
        let ext = std::path::Path::new(&name)
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(ext.as_str(), "srt" | "ass" | "ssa" | "json") {
            return Err(AppError::BadRequest(
                "sidecar must be .srt, .ass, .ssa or .json".into(),
            ));
        }
        let bytes = field
            .bytes()
            .await
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        if bytes.len() > 32 * 1024 * 1024 {
            return Err(AppError::BadRequest("sidecar is too large".into()));
        }
        let path = state
            .config
            .uploads_dir()
            .join(format!("{}.{}", uuid::Uuid::new_v4(), ext));
        tokio::fs::write(&path, bytes).await?;
        uploaded = Some(path);
        break;
    }
    let path = uploaded.ok_or_else(|| AppError::BadRequest("missing file".into()))?;
    let job = jobs::attach_sidecar(&state, &id, Some(path))
        .map_err(|e| AppError::Conflict(e.to_string()))?;
    jobs::enqueue_prepare(state, id).map_err(|e| AppError::Conflict(e.to_string()))?;
    Ok(Json(job))
}

pub async fn export_subtitles(
    State(state): State<AppState>,
    Path((id, format)): Path<(String, String)>,
) -> AppResult<(axum::http::HeaderMap, Bytes)> {
    let job = jobs::get_job(&state, &id).map_err(|_| AppError::NotFound("job not found".into()))?;
    let lines = job.lines.clone().unwrap_or_default();
    let mut headers = axum::http::HeaderMap::new();
    let (mime, data) = match format.as_str() {
        "srt" => (
            "application/x-subrip",
            crate::subtitle::srt::generate_srt_content(&lines).into_bytes(),
        ),
        "json" => ("application/json", serde_json::to_vec_pretty(&lines)?),
        "ass" => {
            let mut preset =
                jobs::resolve_preset(&state, &job.original_name, None, job.preset_id.as_deref())
                    .await;
            preset.format = job.format.clone();
            let source_resolution = if let Some(input) = job.input_path.as_ref() {
                let token = CancellationToken::new();
                let probe = probe_media(input, &token)
                    .await
                    .map_err(|error| AppError::Internal(anyhow::anyhow!(error)))?;
                Some((probe.width, probe.height))
            } else {
                None
            };
            (
                "text/x-ssa",
                crate::subtitle::ass::generate_ass_content(&lines, &preset, source_resolution)
                    .into_bytes(),
            )
        }
        _ => {
            return Err(AppError::BadRequest(
                "format must be srt, ass or json".into(),
            ));
        }
    };
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        mime.parse().unwrap_or(axum::http::HeaderValue::from_static(
            "application/octet-stream",
        )),
    );
    headers.insert(
        axum::http::header::CONTENT_DISPOSITION,
        format!(
            "attachment; filename=\"{}.{}\"",
            safe_stem(&job.original_name),
            format
        )
        .parse()
        .map_err(|_| AppError::BadRequest("invalid filename".into()))?,
    );
    Ok((headers, Bytes::from(data)))
}

fn validate_sidecar_extension(path: &std::path::Path) -> AppResult<()> {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "srt" | "ass" | "ssa" | "json") {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "sidecar must be .srt, .ass, .ssa or .json".into(),
        ))
    }
}

fn safe_stem(name: &str) -> String {
    std::path::Path::new(name)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("subtitles")
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .collect()
}
