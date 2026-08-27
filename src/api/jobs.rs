use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;
use crate::state::AppState;
use crate::subtitle::types::{Job, JobStatus, JobEvent, SubtitleLine};
use crate::subtitle::srt::{parse_srt_to_lines, parse_ass_to_lines};
use crate::subtitle::normalize::normalize_and_fix_overlaps;

pub async fn list_active_jobs(State(state): State<AppState>) -> impl IntoResponse {
    let mut jobs: Vec<Job> = state.jobs.iter().map(|kv| kv.value().clone()).collect();
    jobs.sort_by_key(|j| j.id.clone());
    Json(jobs)
}

pub async fn cancel_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if let Some(mut job) = state.jobs.get_mut(&id) {
        job.status = JobStatus::Cancelled;
        let _ = state.tx.send(JobEvent {
            id: id.clone(),
            status: JobStatus::Cancelled,
            progress: None,
            error: None,
        });
    }
    Json(serde_json::json!({ "success": true }))
}

pub async fn upload_and_transcribe(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut video_bytes = None;
    let mut video_name = String::new();
    let mut subtitle_bytes = None;
    let mut subtitle_name = String::new();
    let mut lines_json = None;
    let mut max_chars = 25u32;
    let mut max_lines = 2u32;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or_default().to_string();
        if name == "video" {
            video_name = field.file_name().unwrap_or("video.mp4").to_string();
            video_bytes = field.bytes().await.ok();
        } else if name == "subtitle" {
            subtitle_name = field.file_name().unwrap_or("sub.srt").to_string();
            subtitle_bytes = field.bytes().await.ok();
        } else if name == "maxChars" {
            if let Ok(text) = field.text().await {
                max_chars = text.parse().unwrap_or(25);
            }
        } else if name == "maxLines" {
            if let Ok(text) = field.text().await {
                max_lines = text.parse().unwrap_or(2);
            }
        } else if name == "lines" {
            lines_json = field.text().await.ok();
        }
    }

    let video_bytes = match video_bytes {
        Some(b) => b,
        None => return (StatusCode::BAD_REQUEST, "No video provided").into_response(),
    };

    let ext = std::path::Path::new(&video_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4");
    
    let id = format!("{}.{}", Uuid::new_v4(), ext);
    let input_path = state.config.uploads_dir().join(&id);

    if let Err(e) = tokio::fs::write(&input_path, video_bytes).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, format!("Write err: {}", e)).into_response();
    }

    let mut job = Job {
        id: id.clone(),
        original_name: video_name.clone(),
        status: JobStatus::Uploading,
        progress: None,
        lines: None,
        error: None,
        input_path: Some(input_path.to_string_lossy().to_string()),
    };
    state.jobs.insert(id.clone(), job.clone());
    let _ = state.tx.send(JobEvent { id: id.clone(), status: JobStatus::Uploading, progress: None, error: None });

    let mut lines: Vec<SubtitleLine> = vec![];
    let mut has_lines = false;

    if let Some(lj) = lines_json {
        if let Ok(parsed) = serde_json::from_str::<Vec<SubtitleLine>>(&lj) {
            lines = normalize_and_fix_overlaps(&parsed);
            has_lines = true;
        }
    } else if let Some(sub_bytes) = subtitle_bytes {
        let content = String::from_utf8_lossy(&sub_bytes);
        let sub_ext = std::path::Path::new(&subtitle_name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if sub_ext == "srt" {
            lines = parse_srt_to_lines(&content);
            has_lines = true;
        } else if sub_ext == "ass" {
            lines = parse_ass_to_lines(&content);
            has_lines = true;
        } else if sub_ext == "json" {
            // Very simplified JSON check
            if let Ok(raw) = serde_json::from_str::<crate::subtitle::types::TranscriptionResponse>(&content) {
                lines = crate::subtitle::group::group_transcription_into_lines(&raw, max_chars, max_lines);
                has_lines = true;
            }
        }
    }

    if has_lines {
        job.status = JobStatus::Ready;
        job.lines = Some(lines.clone());
        state.jobs.insert(id.clone(), job);
        let _ = state.tx.send(JobEvent { id: id.clone(), status: JobStatus::Ready, progress: None, error: None });
        
        return Json(serde_json::json!({
            "id": id,
            "originalName": video_name,
            "lines": lines
        })).into_response();
    }

    // Spawn async transcription worker
    let s = state.clone();
    let job_id = id.clone();
    let orig = video_name.clone();
    tokio::spawn(async move {
        crate::pipeline::worker::run_transcribe_job(job_id, input_path, orig, max_chars, max_lines, s).await;
    });

    Json(serde_json::json!({
        "id": id,
        "originalName": video_name,
        "status": "transcribing"
    })).into_response()
}
