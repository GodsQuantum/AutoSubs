use autosubs::{
    api, config::Config, media::detect_encoder_capabilities, state::AppState, workflows,
};
use axum::{Router, http::StatusCode, routing::any};
use clap::Parser;
use tokio_util::sync::CancellationToken;
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "autosubs=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let config = Config::parse();
    let state = AppState::load(config).await?;
    let probe_token = CancellationToken::new();
    let caps = detect_encoder_capabilities(&probe_token).await;
    if !caps.ffmpeg {
        tracing::warn!(
            "FFmpeg is unavailable; the UI/API will run but media jobs will fail until FFmpeg is installed"
        );
    } else if !caps.libass {
        tracing::warn!(
            "FFmpeg is available without the ass/libass filter; subtitle burn is disabled"
        );
    }
    *state.encoders.write().await = caps;

    let shutdown = CancellationToken::new();
    tokio::spawn(workflows::run_supervisor(state.clone(), shutdown.clone()));

    let mut app = Router::new()
        .merge(api::router())
        // API paths must never fall through to the SPA index.
        .route("/api", any(api_not_found))
        .route("/api/", any(api_not_found))
        .route("/api/{*path}", any(api_not_found))
        .nest_service("/fonts", ServeDir::new(&state.config.fonts_dir))
        .layer(TraceLayer::new_for_http());
    if state.config.dist_dir.is_dir() {
        let index = state.config.dist_dir.join("index.html");
        app = app.fallback_service(
            ServeDir::new(&state.config.dist_dir)
                .append_index_html_on_directories(true)
                .not_found_service(ServeFile::new(index)),
        );
    }
    let app = app.with_state(state.clone());
    let address = format!("{}:{}", state.config.host, state.config.port);
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!(%address, "AutoSubs started");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shutdown))
        .await?;
    Ok(())
}

async fn api_not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}

async fn shutdown_signal(token: CancellationToken) {
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = async { if let Some(signal) = term.as_mut() { signal.recv().await; } } => {} }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
    token.cancel();
}
