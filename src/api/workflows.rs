use axum::{
    extract::{State, Path},
    response::IntoResponse,
    Json,
};
use std::fs;
use uuid::Uuid;
use crate::subtitle::types::Workflow;
use crate::state::AppState;

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let workflows = state.workflows.read().await.clone();
    Json(workflows)
}

pub async fn upsert(
    State(state): State<AppState>,
    Json(mut workflow): Json<Workflow>,
) -> impl IntoResponse {
    let mut is_new = false;
    if workflow.id.is_empty() {
        workflow.id = Uuid::new_v4().to_string();
        is_new = true;
    }

    let mut workflows = state.workflows.write().await;
    if let Some(w) = workflows.iter_mut().find(|w| w.id == workflow.id) {
        *w = workflow.clone();
    } else {
        workflows.push(workflow.clone());
    }

    if let Ok(json) = serde_json::to_string_pretty(&*workflows) {
        let _ = fs::write(state.config.workflows_file(), json);
    }

    // If new and enabled, spawn watcher
    if is_new && workflow.enabled {
        let s = state.clone();
        let w_clone = workflow.clone();
        tokio::spawn(async move {
            let _ = crate::watchdog::start_workflow_watcher(w_clone, s).await;
        });
    }

    Json(workflow)
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut workflows = state.workflows.write().await;
    workflows.retain(|w| w.id != id);

    if let Ok(json) = serde_json::to_string_pretty(&*workflows) {
        let _ = fs::write(state.config.workflows_file(), json);
    }

    axum::http::StatusCode::OK
}
