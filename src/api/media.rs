use crate::{error::{AppError, AppResult}, jobs, state::AppState};
use axum::{body::Body, extract::{Path, State}, http::{HeaderMap, HeaderValue, Response, StatusCode, header}};
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

pub async fn head_job_video(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<Response<Body>> {
    let path = job_video(&state, &id)?; response_for(path, HeaderMap::new(), true).await
}
pub async fn stream_job_video(State(state): State<AppState>, Path(id): Path<String>, headers: HeaderMap) -> AppResult<Response<Body>> {
    let path = job_video(&state, &id)?; response_for(path, headers, false).await
}
fn job_video(state: &AppState, id: &str) -> AppResult<PathBuf> {
    let job = jobs::get_job(state, id).map_err(|_| AppError::NotFound("job not found".into()))?;
    let path = job.output_path.or(job.input_path).ok_or_else(|| AppError::NotFound("job has no video".into()))?;
    if !path.is_file() { return Err(AppError::NotFound("video file no longer exists".into())); } Ok(path)
}

async fn response_for(path: PathBuf, request_headers: HeaderMap, head_only: bool) -> AppResult<Response<Body>> {
    let meta = tokio::fs::metadata(&path).await?; let total = meta.len(); if total == 0 { return Err(AppError::NotFound("video is empty".into())); }
    let mime = mime_guess::from_path(&path).first_or_octet_stream().to_string();
    let range = request_headers.get(header::RANGE).and_then(|v| v.to_str().ok()).and_then(|v| parse_range(v, total));
    let (start,end,status) = range.map(|(s,e)| (s,e,StatusCode::PARTIAL_CONTENT)).unwrap_or((0,total-1,StatusCode::OK)); let length = end - start + 1;
    let mut builder = Response::builder().status(status).header(header::ACCEPT_RANGES, "bytes").header(header::CONTENT_TYPE, mime).header(header::CONTENT_LENGTH, length.to_string());
    if status == StatusCode::PARTIAL_CONTENT { builder = builder.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{total}")); }
    if head_only { return builder.body(Body::empty()).map_err(|e| AppError::Internal(e.into())); }
    let mut file = tokio::fs::File::open(path).await?; file.seek(std::io::SeekFrom::Start(start)).await?; let stream = ReaderStream::new(file.take(length));
    builder.body(Body::from_stream(stream)).map_err(|e| AppError::Internal(e.into()))
}

fn parse_range(value: &str, total: u64) -> Option<(u64,u64)> {
    let raw = value.strip_prefix("bytes=")?; if raw.contains(',') { return None; } let (left,right) = raw.split_once('-')?;
    if left.is_empty() { let suffix: u64 = right.parse().ok()?; if suffix == 0 { return None; } let start = total.saturating_sub(suffix.min(total)); return Some((start,total-1)); }
    let start: u64 = left.parse().ok()?; if start >= total { return None; }
    let end = if right.is_empty() { total-1 } else { right.parse::<u64>().ok()?.min(total-1) }; (start <= end).then_some((start,end))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn supports_suffix_and_open_ranges() { assert_eq!(parse_range("bytes=-100", 1000), Some((900,999))); assert_eq!(parse_range("bytes=500-",1000), Some((500,999))); assert_eq!(parse_range("bytes=0-99",1000), Some((0,99))); }
}
