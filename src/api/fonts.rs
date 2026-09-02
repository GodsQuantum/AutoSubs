use crate::{
    error::{AppError, AppResult},
    fonts::{self, FontFace},
};
use axum::{
    Json,
    body::Body,
    extract::Path,
    http::{Response, header},
};
use tokio_util::io::ReaderStream;

pub async fn list() -> AppResult<Json<Vec<FontFace>>> {
    Ok(Json(fonts::scan_fonts().map_err(AppError::Internal)?))
}

pub async fn stylesheet() -> AppResult<Response<Body>> {
    let catalog = fonts::scan_fonts().map_err(AppError::Internal)?;
    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(fonts::css(&catalog)))
        .map_err(|error| AppError::Internal(error.into()))
}

pub async fn content(Path(id): Path<String>) -> AppResult<Response<Body>> {
    let path = fonts::resolve_font_content(&id)
        .map_err(|_| AppError::NotFound("font not found".into()))?;
    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|_| AppError::NotFound("font not found".into()))?;
    let mime = match path
        .extension()
        .and_then(|v| v.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("otf" | "otc") => "font/otf",
        _ => "font/ttf",
    };
    Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=3600")
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from_stream(ReaderStream::new(file)))
        .map_err(|error| AppError::Internal(error.into()))
}
