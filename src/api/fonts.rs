use crate::{
    error::{AppError, AppResult},
    fonts::{self, FontFace, FontSource},
    state::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, State},
    http::{Response, header},
};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

pub async fn list(State(state): State<AppState>) -> AppResult<Json<Vec<FontFace>>> {
    Ok(Json(
        fonts::scan_catalog(&state.config.fonts_dir).map_err(AppError::Internal)?,
    ))
}

pub async fn stylesheet(State(state): State<AppState>) -> AppResult<Response<Body>> {
    let catalog = fonts::scan_catalog(&state.config.fonts_dir).map_err(AppError::Internal)?;
    Response::builder()
        .header(header::CONTENT_TYPE, "text/css; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(fonts::css(&catalog)))
        .map_err(|error| AppError::Internal(error.into()))
}

pub async fn content(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response<Body>> {
    let path = fonts::resolve_app_font_content(&state.config.fonts_dir, &id)
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

pub async fn upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> AppResult<Json<FontFace>> {
    const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::BadRequest(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let original = field.file_name().unwrap_or("font.ttf").to_owned();
        let extension = std::path::Path::new(&original)
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .filter(|value| matches!(value.as_str(), "ttf" | "otf" | "ttc" | "otc"))
            .ok_or_else(|| AppError::BadRequest("unsupported font format".into()))?;
        let stored = format!("{}.{}", Uuid::new_v4(), extension);
        let path = crate::config::Config::safe_child(&state.config.fonts_dir, &stored)
            .map_err(AppError::Internal)?;
        let mut file = tokio::fs::File::create(&path).await?;
        let mut size = 0_u64;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|error| AppError::BadRequest(error.to_string()))?
        {
            size = size.saturating_add(chunk.len() as u64);
            if size > MAX_FONT_BYTES.min(state.config.max_upload_bytes) {
                let _ = tokio::fs::remove_file(&path).await;
                return Err(AppError::BadRequest("font file is too large".into()));
            }
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);

        if !fonts::valid_font_file(&path) {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(AppError::BadRequest(
                "invalid or unreadable font file".into(),
            ));
        }

        let face = fonts::scan_app_fonts(&state.config.fonts_dir)
            .map_err(AppError::Internal)?
            .into_iter()
            .find(|face| face.source == FontSource::App && face.file_name == stored)
            .ok_or_else(|| {
                AppError::Internal(anyhow::anyhow!("imported font missing from catalog"))
            })?;
        return Ok(Json(face));
    }
    Err(AppError::BadRequest("missing font file".into()))
}
