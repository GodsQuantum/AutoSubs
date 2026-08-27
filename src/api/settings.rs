use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use crate::subtitle::types::Settings;
use crate::state::AppState;
use serde::Deserialize;
use std::fs;

pub async fn get(State(state): State<AppState>) -> impl IntoResponse {
    let settings = state.settings.read().await.clone();
    Json(settings)
}

pub async fn update(
    State(state): State<AppState>,
    Json(new_settings): Json<Settings>,
) -> impl IntoResponse {
    let mut settings = state.settings.write().await;
    *settings = new_settings.clone();
    
    if let Ok(json) = serde_json::to_string_pretty(&*settings) {
        let _ = fs::write(state.config.settings_file(), json);
    }
    
    Json(new_settings)
}

#[derive(Deserialize)]
pub struct ModelsRequest {
    #[serde(rename = "transcriptionUrl")]
    transcription_url: String,
    #[serde(rename = "transcriptionApiKey")]
    transcription_api_key: String,
}

#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelData>,
}

#[derive(Deserialize)]
struct ModelData {
    id: String,
}

pub async fn list_models(
    State(state): State<AppState>,
    Json(req): Json<ModelsRequest>,
) -> impl IntoResponse {
    let base = req.transcription_url.trim_end_matches('/');
    
    let mut url = format!("{}/v1/models", base);
    let mut res = state.http_client.get(&url).bearer_auth(&req.transcription_api_key).send().await;
    
    if res.is_err() || res.as_ref().unwrap().status() != 200 {
        url = format!("{}/models", base);
        res = state.http_client.get(&url).bearer_auth(&req.transcription_api_key).send().await;
    }
    
    if let Ok(res) = res {
        if let Ok(models) = res.json::<ModelsResponse>().await {
            let ids: Vec<String> = models.data.into_iter().map(|m| m.id).collect();
            return Json(ids).into_response();
        }
    }
    
    Json(Vec::<String>::new()).into_response()
}
