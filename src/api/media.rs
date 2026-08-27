use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::IntoResponse,
    Json,
};
use tokio::fs::File;
use tokio_util::io::ReaderStream;
use std::path::PathBuf;
use crate::state::AppState;

// ─── Font listing ─────────────────────────────────────────────────────────────

pub async fn list_fonts(State(state): State<AppState>) -> impl IntoResponse {
    let mut fonts = vec![];
    if let Ok(mut entries) = tokio::fs::read_dir(&state.config.fonts_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let e = ext.to_lowercase();
                if e == "ttf" || e == "otf" || e == "woff" || e == "woff2" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        fonts.push(name.to_string());
                    }
                }
            }
        }
    }
    Json(fonts)
}

// ─── Outro handling ───────────────────────────────────────────────────────────

pub async fn list_outros(State(state): State<AppState>) -> impl IntoResponse {
    let mut outros = vec![];
    if let Ok(mut entries) = tokio::fs::read_dir(state.config.outros_dir()).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let e = ext.to_lowercase();
                if e == "mp4" || e == "mov" || e == "webm" {
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        outros.push(name.to_string());
                    }
                }
            }
        }
    }
    Json(outros)
}

pub async fn upload_outro(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let orig_name = field.file_name().unwrap_or("outro.mp4").to_string();
            let ext = std::path::Path::new(&orig_name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("mp4");
            
            // Clean up name
            let base = orig_name.replace(&format!(".{}", ext), "")
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect::<String>();
                
            let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis();
            let new_name = format!("{}_{}.{}", base, timestamp, ext);
            let out_path = state.config.outros_dir().join(&new_name);

            if let Ok(bytes) = field.bytes().await {
                if tokio::fs::write(&out_path, bytes).await.is_ok() {
                    return Json(serde_json::json!({ "filename": new_name })).into_response();
                }
            }
        }
    }
    (StatusCode::BAD_REQUEST, "No valid file found").into_response()
}

pub async fn delete_outro(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let path = state.config.outros_dir().join(name);
    let _ = tokio::fs::remove_file(path).await;
    Json(serde_json::json!({ "success": true }))
}

// ─── Video streaming (HTTP 206) ───────────────────────────────────────────────

pub async fn video_stream(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    // Basic path traversal prevention
    let safe_id = std::path::Path::new(&id)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
        
    let file_path = state.config.uploads_dir().join(&safe_id);

    let mut file = match File::open(&file_path).await {
        Ok(f) => f,
        Err(_) => return (StatusCode::NOT_FOUND, "Video not found").into_response(),
    };

    let metadata = match file.metadata().await {
        Ok(m) => m,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Error reading metadata").into_response(),
    };
    let file_size = metadata.len();

    let content_type = match file_path.extension().and_then(|e| e.to_str()) {
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        _ => "video/mp4",
    };

    if let Some(range) = headers.get(header::RANGE) {
        if let Ok(range_str) = range.to_str() {
            let range = range_str.replace("bytes=", "");
            let parts: Vec<&str> = range.split('-').collect();
            if parts.len() == 2 {
                let start: u64 = parts[0].parse().unwrap_or(0);
                let end: u64 = if parts[1].is_empty() {
                    file_size - 1
                } else {
                    parts[1].parse().unwrap_or(file_size - 1)
                };

                let end = std::cmp::min(end, file_size - 1);
                let chunk_size = end - start + 1;

                use tokio::io::AsyncSeekExt;
                if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Seek failed").into_response();
                }

                use tokio::io::AsyncReadExt;
                let stream = ReaderStream::new(file.take(chunk_size));
                let body = Body::from_stream(stream);

                let mut res = body.into_response();
                *res.status_mut() = StatusCode::PARTIAL_CONTENT;
                res.headers_mut().insert(header::CONTENT_TYPE, content_type.parse().unwrap());
                res.headers_mut().insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
                res.headers_mut().insert(
                    header::CONTENT_LENGTH,
                    chunk_size.to_string().parse().unwrap(),
                );
                res.headers_mut().insert(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, file_size).parse().unwrap(),
                );
                return res;
            }
        }
    }

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut res = body.into_response();
    *res.status_mut() = StatusCode::OK;
    res.headers_mut().insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    res.headers_mut().insert(header::CONTENT_LENGTH, file_size.to_string().parse().unwrap());
    res
}
