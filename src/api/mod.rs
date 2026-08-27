pub mod events;
pub mod presets;
pub mod settings;
pub mod workflows;
pub mod media;
pub mod jobs;
pub mod burn;
pub mod browse;

use axum::{Router, routing::{get, post, delete}};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/upload-and-transcribe", post(jobs::upload_and_transcribe))
        .route("/api/active-jobs", get(jobs::list_active_jobs))
        .route("/api/jobs/{id}/cancel", post(jobs::cancel_job))
        .route("/api/burn", post(burn::burn))
        .route("/api/batch-burn", post(burn::batch_burn))
        .route("/api/regroup", post(burn::regroup))
        .route("/api/presets", get(presets::list).post(presets::upsert))
        .route("/api/presets/import", post(presets::import))
        .route("/api/presets/{name}", delete(presets::delete))
        .route("/api/settings", get(settings::get).post(settings::update))
        .route("/api/models", post(settings::list_models))
        .route("/api/browse", get(browse::browse_directory))
        .route("/api/workflows", get(workflows::list).post(workflows::upsert))
        .route("/api/workflows/{id}", delete(workflows::delete))
        .route("/api/video-stream/{id}", get(media::video_stream))
        .route("/api/fonts", get(media::list_fonts))
        .route("/api/outros", get(media::list_outros).post(media::upload_outro))
        .route("/api/outros/{name}", delete(media::delete_outro))
        .route("/api/events", get(events::sse_handler))
}
