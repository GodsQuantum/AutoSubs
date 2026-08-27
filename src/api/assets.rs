use crate::{domain::Asset, error::{AppError, AppResult}, jobs::now_ms, state::AppState};
use axum::{Json, extract::{Multipart, Path, State}};
use serde::Deserialize;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub async fn list_assets(State(state): State<AppState>) -> AppResult<Json<Vec<Asset>>> { Ok(Json(state.db.list("asset").map_err(AppError::Internal)?)) }

pub async fn upload_asset(State(state): State<AppState>, mut multipart: Multipart) -> AppResult<Json<Asset>> {
    while let Some(mut field) = multipart.next_field().await.map_err(|e| AppError::BadRequest(e.to_string()))? {
        if field.name() != Some("file") { continue; }
        let name = field.file_name().unwrap_or("asset.bin").to_owned(); let mime = field.content_type().unwrap_or("application/octet-stream").to_owned();
        let ext = std::path::Path::new(&name).extension().and_then(|v| v.to_str()).map(safe_ext).unwrap_or_default();
        let id = Uuid::new_v4().to_string(); let stored = if ext.is_empty() { id.clone() } else { format!("{id}.{ext}") }; let path = state.config.assets_dir().join(&stored);
        let mut file = tokio::fs::File::create(&path).await?; let mut size = 0u64;
        while let Some(chunk) = field.chunk().await.map_err(|e| AppError::BadRequest(e.to_string()))? { size = size.saturating_add(chunk.len() as u64); if size > state.config.max_upload_bytes { let _ = tokio::fs::remove_file(&path).await; return Err(AppError::BadRequest("asset exceeds AUTOSUBS_MAX_UPLOAD_BYTES".into())); } file.write_all(&chunk).await?; }
        file.flush().await?; let asset = Asset { id: id.clone(), name, stored_file: stored, mime, size, created_at_ms: now_ms() };
        state.db.upsert("asset", &id, &asset).map_err(AppError::Internal)?; return Ok(Json(asset));
    }
    Err(AppError::BadRequest("missing file".into()))
}

#[derive(Deserialize)]
pub struct ImportAsset { path: String }
pub async fn import_asset(State(state): State<AppState>, Json(body): Json<ImportAsset>) -> AppResult<Json<Asset>> {
    let source = PathBuf::from(&body.path); if !source.is_file() || !state.config.path_is_allowed(&source) { return Err(AppError::Forbidden("asset path is outside allowed roots".into())); }
    let name = source.file_name().and_then(|v| v.to_str()).unwrap_or("asset.bin").to_owned(); let ext = source.extension().and_then(|v| v.to_str()).map(safe_ext).unwrap_or_default();
    let id = Uuid::new_v4().to_string(); let stored = if ext.is_empty() { id.clone() } else { format!("{id}.{ext}") }; let destination = state.config.assets_dir().join(&stored);
    let size = tokio::fs::copy(&source, &destination).await?; let mime = mime_guess::from_path(&source).first_or_octet_stream().to_string();
    let asset = Asset { id: id.clone(), name, stored_file: stored, mime, size, created_at_ms: now_ms() }; state.db.upsert("asset", &id, &asset).map_err(AppError::Internal)?; Ok(Json(asset))
}

pub async fn delete_asset(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<axum::http::StatusCode> {
    let asset = state.db.get::<Asset>("asset", &id).map_err(AppError::Internal)?.ok_or_else(|| AppError::NotFound("asset not found".into()))?;
    let _ = tokio::fs::remove_file(state.config.assets_dir().join(asset.stored_file)).await;
    state.db.delete("asset", &id).map_err(AppError::Internal)?; Ok(axum::http::StatusCode::NO_CONTENT)
}
fn safe_ext(value: &str) -> String { value.chars().filter(|c| c.is_ascii_alphanumeric()).take(12).collect::<String>().to_ascii_lowercase() }

pub async fn stream_asset(State(state): State<AppState>, Path(id): Path<String>) -> AppResult<axum::response::Response> {
    use axum::{body::Body, http::{Response, header}};
    use tokio_util::io::ReaderStream;
    let asset = state.db.get::<Asset>("asset", &id).map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("asset not found".into()))?;
    let path = state.config.assets_dir().join(&asset.stored_file);
    if !path.is_file() { return Err(AppError::NotFound("asset file is missing".into())); }
    let meta = tokio::fs::metadata(&path).await?;
    let file = tokio::fs::File::open(path).await?;
    Response::builder()
        .header(header::CONTENT_TYPE, if asset.mime.is_empty() { "application/octet-stream" } else { asset.mime.as_str() })
        .header(header::CONTENT_LENGTH, meta.len().to_string())
        .header(header::CACHE_CONTROL, "private, max-age=3600")
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(|error| AppError::Internal(error.into()))
}
