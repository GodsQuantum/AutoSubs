pub mod assets;
pub mod browse;
pub mod events;
pub mod fonts;
pub mod jobs;
pub mod media;
pub mod resources;
pub mod settings;
pub mod uploads;

use crate::{error::AppResult, state::AppState};
use axum::{
    Json, Router,
    extract::DefaultBodyLimit,
    routing::{delete, get, options, post, put},
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    status: &'static str,
    version: &'static str,
    ffmpeg_ready: bool,
    libass: bool,
}

async fn health(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> AppResult<Json<Health>> {
    let caps = state.encoders.read().await.clone();
    Ok(Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        ffmpeg_ready: caps.ffmpeg,
        libass: caps.libass,
    }))
}
async fn capabilities(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<crate::media::render::EncoderCapabilities> {
    Json(state.encoders.read().await.clone())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/capabilities", get(capabilities))
        .route("/api/v1/fonts", get(fonts::list))
        .route("/api/v1/fonts/css", get(fonts::stylesheet))
        .route("/api/v1/fonts/{id}/content", get(fonts::content))
        .route("/api/v1/events", get(events::events))
        .route("/api/v1/jobs", get(jobs::list_jobs))
        .route("/api/v1/jobs/from-path", post(jobs::create_from_path))
        .route(
            "/api/v1/jobs/{id}",
            get(jobs::get_job)
                .put(jobs::update_job_options)
                .delete(jobs::delete),
        )
        .route("/api/v1/jobs/{id}/prepare", post(jobs::prepare))
        .route("/api/v1/jobs/{id}/render", post(jobs::render))
        .route("/api/v1/jobs/{id}/cancel", post(jobs::cancel))
        .route("/api/v1/jobs/{id}/retranscribe", post(jobs::retranscribe))
        .route(
            "/api/v1/jobs/{id}/subtitles",
            get(jobs::get_subtitles).put(jobs::save_subtitles),
        )
        .route(
            "/api/v1/jobs/{id}/subtitles/{format}",
            get(jobs::export_subtitles),
        )
        .route("/api/v1/jobs/{id}/regroup", post(jobs::regroup))
        .route(
            "/api/v1/jobs/{id}/sidecar",
            put(jobs::set_sidecar).delete(jobs::remove_sidecar),
        )
        .route(
            "/api/v1/jobs/{id}/sidecar/upload",
            post(jobs::upload_sidecar),
        )
        .route(
            "/api/v1/jobs/{id}/video",
            get(media::stream_job_video).head(media::head_job_video),
        )
        .route(
            "/api/v1/jobs/{id}/video/source",
            get(media::stream_job_source_video).head(media::head_job_source_video),
        )
        .route(
            "/api/v1/jobs/{id}/video/output",
            get(media::stream_job_output_video).head(media::head_job_output_video),
        )
        .route(
            "/api/v1/presets",
            get(resources::list_presets).post(resources::upsert_preset),
        )
        .route("/api/v1/presets/{id}", delete(resources::delete_preset))
        .route(
            "/api/v1/brands",
            get(resources::list_brands).post(resources::upsert_brand),
        )
        .route("/api/v1/brands/{id}", delete(resources::delete_brand))
        .route(
            "/api/v1/workflows",
            get(resources::list_workflows).post(resources::upsert_workflow),
        )
        .route("/api/v1/workflows/{id}", delete(resources::delete_workflow))
        .route(
            "/api/v1/settings",
            get(settings::get_settings).put(settings::update_settings),
        )
        .route("/api/v1/models", post(settings::list_models))
        .route("/api/v1/browse", get(browse::browse))
        .route(
            "/api/v1/assets",
            get(assets::list_assets)
                .post(assets::upload_asset)
                // Multipart is streamed and capped by AUTOSUBS_MAX_UPLOAD_BYTES in the handler.
                .layer(DefaultBodyLimit::disable()),
        )
        .route("/api/v1/assets/import", post(assets::import_asset))
        .route("/api/v1/assets/{id}", delete(assets::delete_asset))
        .route("/api/v1/assets/{id}/content", get(assets::stream_asset))
        .route(
            "/api/v1/uploads",
            options(uploads::options_uploads).post(uploads::create_upload),
        )
        .route(
            "/api/v1/uploads/{id}",
            options(uploads::options_uploads)
                .head(uploads::head_upload)
                .patch(uploads::patch_upload),
        )
        // Narrow compatibility aliases for old health/settings/event clients.
        .route(
            "/api/settings",
            get(settings::get_settings).post(settings::update_settings_legacy),
        )
        .route("/api/events", get(events::events))
}

#[cfg(test)]
mod multipart_regression_tests {
    use super::*;
    use crate::config::Config;
    use std::path::PathBuf;

    fn config(root: &std::path::Path) -> Config {
        Config {
            host: "127.0.0.1".into(),
            port: 0,
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            fonts_dir: root.join("fonts"),
            dist_dir: PathBuf::new(),
            allowed_roots: vec![root.join("data")],
            max_render_jobs: 1,
            max_transcription_jobs: 1,
            max_queued_jobs: 8,
            workflow_scan_seconds: 5,
            file_stability_ms: 0,
            max_upload_bytes: 10 * 1024 * 1024,
        }
    }

    #[tokio::test]
    async fn asset_multipart_upload_can_exceed_axum_default_body_limit() {
        let dir = tempfile::tempdir().unwrap();
        let state = AppState::load(config(dir.path())).await.unwrap();
        let app = router().with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let payload = vec![b'x'; 3 * 1024 * 1024];
        let part = reqwest::multipart::Part::bytes(payload)
            .file_name("outro.mp4")
            .mime_str("video/mp4")
            .unwrap();
        let response = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/assets"))
            .multipart(reqwest::multipart::Form::new().part("file", part))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "status={}",
            response.status()
        );
    }

    #[tokio::test]
    async fn asset_multipart_upload_still_respects_configured_maximum() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = config(dir.path());
        cfg.max_upload_bytes = 1024 * 1024;
        let state = AppState::load(cfg).await.unwrap();
        let app = router().with_state(state);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let payload = vec![b'x'; 2 * 1024 * 1024];
        let response = reqwest::Client::new()
            .post(format!("http://{addr}/api/v1/assets"))
            .multipart(reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(payload).file_name("too-large.mp4"),
            ))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        assert_eq!(
            std::fs::read_dir(dir.path().join("data/assets"))
                .unwrap()
                .count(),
            0
        );
    }
}
