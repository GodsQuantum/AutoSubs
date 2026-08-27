use axum::Router;
use clap::Parser;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod pipeline;
mod state;
mod subtitle;
mod watchdog;

use config::Config;
use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "autosubs=info,tower_http=warn".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::parse();
    config.init_dirs()?;
    tracing::info!("Data dir: {:?}", config.data_dir);
    tracing::info!("Fonts dir: {:?}", config.fonts_dir);

    let state = AppState::new(config.clone());

    // Start file watchers for all enabled workflows
    {
        let s = state.clone();
        tokio::spawn(async move {
            watchdog::start_all_watchers(s).await;
        });
    }

    // Job cleanup task (janitor)
    {
        let s = state.clone();
        tokio::spawn(async move {
            use crate::subtitle::types::JobStatus;
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
                s.jobs.retain(|_, job| {
                    match job.status {
                        JobStatus::Done | JobStatus::Error | JobStatus::Cancelled => false, // removes them
                        _ => true,
                    }
                });
            }
        });
    }

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let mut app = Router::new()
        .merge(api::router())
        // Serve fonts directory statically
        .nest_service("/fonts", ServeDir::new(&config.fonts_dir))
        .layer(cors)
        .with_state(state);


    // Serve static frontend in production
    let dist = config.dist_dir.clone();
    if dist.exists() {
        use tower_http::set_header::SetResponseHeaderLayer;
        use axum::http::header::CACHE_CONTROL;
        
        let no_cache_layer = SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store, no-cache, must-revalidate, proxy-revalidate, max-age=0")
        );

        let serve = tower_http::services::ServeDir::new(&dist)
            .append_index_html_on_directories(true)
            .not_found_service(tower_http::services::ServeFile::new(dist.join("index.html")));
            
        let fallback_router = axum::Router::new().fallback_service(serve).layer(no_cache_layer);
        app = app.fallback_service(fallback_router);
    }

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("🚀 AutoSubs listening on http://{}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
