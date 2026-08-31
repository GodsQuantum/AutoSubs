use crate::{
    config::Config,
    error::{AppError, AppResult},
    jobs,
    state::AppState,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{HeaderMap, HeaderValue, StatusCode},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

const TUS: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UploadRecord {
    id: String,
    filename: String,
    staging: PathBuf,
    final_path: Option<PathBuf>,
    length: u64,
    offset: u64,
    job_id: Option<String>,
}

fn tus_headers(headers: &mut HeaderMap) {
    headers.insert("Tus-Resumable", HeaderValue::from_static(TUS));
    headers.insert("Tus-Version", HeaderValue::from_static(TUS));
}

pub async fn options_uploads() -> (StatusCode, HeaderMap) {
    let mut headers = HeaderMap::new();
    tus_headers(&mut headers);
    headers.insert("Tus-Extension", HeaderValue::from_static("creation"));
    headers.insert("Tus-Max-Size", HeaderValue::from_static("53687091200"));
    (StatusCode::NO_CONTENT, headers)
}

pub async fn create_upload(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<(StatusCode, HeaderMap)> {
    require_tus(&headers)?;
    let length = header_u64(&headers, "Upload-Length")?;
    if length > state.config.max_upload_bytes {
        return Err(AppError::BadRequest(
            "upload exceeds AUTOSUBS_MAX_UPLOAD_BYTES".into(),
        ));
    }
    let filename = parse_filename(headers.get("Upload-Metadata").and_then(|v| v.to_str().ok()))
        .map(|value| safe_filename(&value))
        .unwrap_or_else(|| "video".into());
    let id = Uuid::new_v4().to_string();
    let staging_name = format!(".{id}.uploading");
    let staging = Config::safe_child(&state.config.uploads_dir(), &staging_name)
        .map_err(AppError::Internal)?;
    tokio::fs::File::create(&staging).await?;
    let record = UploadRecord {
        id: id.clone(),
        filename,
        staging,
        final_path: None,
        length,
        offset: 0,
        job_id: None,
    };
    state
        .db
        .upsert("upload", &id, &record)
        .map_err(AppError::Internal)?;
    let mut response = HeaderMap::new();
    tus_headers(&mut response);
    response.insert(
        "Location",
        format!("/api/v1/uploads/{id}")
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid upload location")))?,
    );
    response.insert("Upload-Offset", HeaderValue::from_static("0"));
    Ok((StatusCode::CREATED, response))
}

pub async fn head_upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> AppResult<(StatusCode, HeaderMap)> {
    require_tus(&headers)?;
    let record = load(&state, &id)?;
    let mut response = HeaderMap::new();
    tus_headers(&mut response);
    response.insert(
        "Upload-Offset",
        record
            .offset
            .to_string()
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad offset")))?,
    );
    response.insert(
        "Upload-Length",
        record
            .length
            .to_string()
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad length")))?,
    );
    Ok((StatusCode::OK, response))
}

pub async fn patch_upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> AppResult<(StatusCode, HeaderMap)> {
    require_tus(&headers)?;
    if headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        != Some("application/offset+octet-stream")
    {
        return Err(AppError::BadRequest(
            "PATCH Content-Type must be application/offset+octet-stream".into(),
        ));
    }
    let client_offset = header_u64(&headers, "Upload-Offset")?;
    let mut record = load(&state, &id)?;

    if record.id != id {
        return Err(AppError::Internal(anyhow::anyhow!(
            "upload record id mismatch"
        )));
    }

    let upload_id = Uuid::parse_str(&record.id)
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid persisted upload id")))?;
    let staging_name = format!(".{upload_id}.uploading");
    let staging = Config::safe_child(&state.config.uploads_dir(), &staging_name)
        .map_err(AppError::Internal)?;

    if record.offset != client_offset {
        return Err(AppError::Conflict(format!(
            "upload offset mismatch: server={}, client={client_offset}",
            record.offset
        )));
    }
    if record.offset >= record.length {
        return Err(AppError::Conflict("upload is already complete".into()));
    }
    let mut file = tokio::fs::OpenOptions::new()
        .append(true)
        .open(&staging)
        .await?;
    let mut stream = body.into_data_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AppError::BadRequest(e.to_string()))?;
        let new_offset = record.offset.saturating_add(chunk.len() as u64);
        if new_offset > record.length {
            return Err(AppError::BadRequest("PATCH exceeds Upload-Length".into()));
        }
        file.write_all(&chunk).await?;
        record.offset = new_offset;
    }
    file.flush().await?;
    if record.offset == record.length {
        let final_name = format!("{upload_id}.media");
        let final_path = Config::safe_child(&state.config.uploads_dir(), &final_name)
            .map_err(AppError::Internal)?;
        tokio::fs::rename(&staging, &final_path).await?;
        record.staging = staging;
        record.final_path = Some(final_path.clone());
        let job = jobs::create_job(
            &state,
            record.filename.clone(),
            final_path,
            None,
            None,
            None,
        )
        .map_err(AppError::Internal)?;
        record.job_id = Some(job.id.clone());
        state
            .db
            .upsert("upload", &id, &record)
            .map_err(AppError::Internal)?;
    } else {
        state
            .db
            .upsert("upload", &id, &record)
            .map_err(AppError::Internal)?;
    }
    let mut response = HeaderMap::new();
    tus_headers(&mut response);
    response.insert(
        "Upload-Offset",
        record
            .offset
            .to_string()
            .parse()
            .map_err(|_| AppError::Internal(anyhow::anyhow!("bad offset")))?,
    );
    if let Some(job) = record.job_id {
        response.insert(
            "Upload-Final-Job",
            job.parse()
                .map_err(|_| AppError::Internal(anyhow::anyhow!("bad job id")))?,
        );
    }
    Ok((StatusCode::NO_CONTENT, response))
}

fn load(state: &AppState, id: &str) -> AppResult<UploadRecord> {
    state
        .db
        .get("upload", id)
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("upload not found".into()))
}
fn require_tus(headers: &HeaderMap) -> AppResult<()> {
    if headers.get("Tus-Resumable").and_then(|v| v.to_str().ok()) != Some(TUS) {
        return Err(AppError::BadRequest(
            "Tus-Resumable: 1.0.0 is required".into(),
        ));
    }
    Ok(())
}
fn header_u64(headers: &HeaderMap, name: &str) -> AppResult<u64> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| AppError::BadRequest(format!("missing or invalid {name}")))
}
fn parse_filename(metadata: Option<&str>) -> Option<String> {
    for pair in metadata?.split(',') {
        let mut parts = pair.trim().splitn(2, ' ');
        if parts.next()? != "filename" {
            continue;
        }
        let decoded = STANDARD.decode(parts.next()?).ok()?;
        return String::from_utf8(decoded).ok();
    }
    None
}
fn safe_filename(value: &str) -> String {
    let name = std::path::Path::new(value)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("video");
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_' | ' ') {
                c
            } else {
                '_'
            }
        })
        .take(180)
        .collect();
    if cleaned.trim().is_empty() {
        "video".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_tus_filename() {
        let encoded = STANDARD.encode("clip été.mov");
        assert_eq!(
            parse_filename(Some(&format!("filename {encoded}"))).as_deref(),
            Some("clip été.mov")
        );
    }
    #[test]
    fn filename_drops_path_components() {
        assert_eq!(safe_filename("../../evil.mp4"), "evil.mp4");
    }
}
