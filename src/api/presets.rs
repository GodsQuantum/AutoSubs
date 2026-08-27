use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use crate::subtitle::types::Preset;
use crate::state::AppState;
use std::fs;

pub async fn list(State(state): State<AppState>) -> impl IntoResponse {
    let presets = state.presets.read().await.clone();
    Json(presets)
}

pub async fn upsert(
    State(state): State<AppState>,
    Json(preset): Json<Preset>,
) -> impl IntoResponse {
    let mut presets = state.presets.write().await;
    if let Some(p) = presets.iter_mut().find(|p| p.name == preset.name) {
        *p = preset.clone();
    } else {
        presets.push(preset.clone());
    }
    
    if let Ok(json) = serde_json::to_string_pretty(&*presets) {
        let _ = fs::write(state.config.presets_file(), json);
    }
    
    Json(preset)
}

pub async fn import(
    State(state): State<AppState>,
    Json(preset): Json<Preset>,
) -> impl IntoResponse {
    upsert(State(state), Json(preset)).await
}

pub async fn delete(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    if name == "Défaut" {
        return (StatusCode::BAD_REQUEST, "Cannot delete Défaut preset").into_response();
    }
    
    let mut presets = state.presets.write().await;
    presets.retain(|p| p.name != name);
    
    if let Ok(json) = serde_json::to_string_pretty(&*presets) {
        let _ = fs::write(state.config.presets_file(), json);
    }
    
    StatusCode::OK.into_response()
}
