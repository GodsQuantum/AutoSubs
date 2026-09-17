use crate::{
    domain::{Encoder, Settings},
    error::{AppError, AppResult},
    media::transcribe::models_endpoint,
    state::AppState,
};
use axum::{Json, extract::State};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    transcription_url: String,
    transcription_model: String,
    transcription_api_key_set: bool,
    language: String,
    local_transcription_enabled: bool,
    local_fallback_enabled: bool,
    local_transcription_url: String,
    local_transcription_model: String,
    local_transcription_api_key_set: bool,
    llm_enabled: bool,
    llm_endpoint: String,
    llm_model: String,
    llm_prompt: String,
    llm_api_key_set: bool,
    encoder: Encoder,
}
impl From<&Settings> for SettingsView {
    fn from(s: &Settings) -> Self {
        Self {
            transcription_url: s.transcription_url.clone(),
            transcription_model: s.transcription_model.clone(),
            transcription_api_key_set: !s.transcription_api_key.is_empty(),
            language: s.language.clone(),
            local_transcription_enabled: s.local_transcription_enabled,
            local_fallback_enabled: s.local_fallback_enabled,
            local_transcription_url: s.local_transcription_url.clone(),
            local_transcription_model: s.local_transcription_model.clone(),
            local_transcription_api_key_set: !s.local_transcription_api_key.is_empty(),
            llm_enabled: s.llm_enabled,
            llm_endpoint: s.llm_endpoint.clone(),
            llm_model: s.llm_model.clone(),
            llm_prompt: s.llm_prompt.clone(),
            llm_api_key_set: !s.llm_api_key.is_empty(),
            encoder: s.encoder.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SecretAction {
    Keep,
    Replace,
    Clear,
}
#[derive(Debug, Deserialize)]
struct SecretPatch {
    action: SecretAction,
    #[serde(default)]
    value: String,
}
fn apply_secret(target: &mut String, patch: Option<SecretPatch>) -> AppResult<()> {
    match patch.map(|p| (p.action, p.value)) {
        None | Some((SecretAction::Keep, _)) => {}
        Some((SecretAction::Clear, _)) => target.clear(),
        Some((SecretAction::Replace, value)) if value.is_empty() => {
            return Err(AppError::BadRequest(
                "replacement API key cannot be empty".into(),
            ));
        }
        Some((SecretAction::Replace, value)) => *target = value,
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    transcription_url: String,
    transcription_model: String,
    #[serde(default)]
    transcription_api_key: Option<SecretPatch>,
    language: String,
    local_transcription_enabled: bool,
    local_fallback_enabled: bool,
    local_transcription_url: String,
    local_transcription_model: String,
    #[serde(default)]
    local_transcription_api_key: Option<SecretPatch>,
    llm_enabled: bool,
    llm_endpoint: String,
    llm_model: String,
    llm_prompt: String,
    #[serde(default)]
    llm_api_key: Option<SecretPatch>,
    encoder: Encoder,
}

pub async fn get_settings(State(state): State<AppState>) -> Json<SettingsView> {
    Json(SettingsView::from(&*state.settings.read().await))
}
pub async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<SettingsUpdate>,
) -> AppResult<Json<SettingsView>> {
    let mut settings = state.settings.write().await;
    settings.transcription_url = body.transcription_url;
    settings.transcription_model = body.transcription_model;
    settings.language = body.language;
    settings.local_transcription_enabled = body.local_transcription_enabled;
    settings.local_fallback_enabled = body.local_fallback_enabled;
    settings.local_transcription_url = body.local_transcription_url;
    settings.local_transcription_model = body.local_transcription_model;
    settings.llm_enabled = body.llm_enabled;
    settings.llm_endpoint = body.llm_endpoint;
    settings.llm_model = body.llm_model;
    settings.llm_prompt = body.llm_prompt;
    settings.encoder = body.encoder;
    apply_secret(
        &mut settings.transcription_api_key,
        body.transcription_api_key,
    )?;
    apply_secret(
        &mut settings.local_transcription_api_key,
        body.local_transcription_api_key,
    )?;
    apply_secret(&mut settings.llm_api_key, body.llm_api_key)?;
    state
        .db
        .set_singleton("settings", &*settings)
        .map_err(AppError::Internal)?;
    Ok(Json(SettingsView::from(&*settings)))
}

pub async fn update_settings_legacy(
    State(state): State<AppState>,
    Json(mut incoming): Json<Settings>,
) -> AppResult<Json<SettingsView>> {
    // Legacy clients often send empty keys because GET used to echo secrets. Empty now means keep.
    let mut current = state.settings.write().await;
    if incoming.transcription_api_key.is_empty() {
        incoming.transcription_api_key = current.transcription_api_key.clone();
    }
    if incoming.local_transcription_api_key.is_empty() {
        incoming.local_transcription_api_key = current.local_transcription_api_key.clone();
    }
    if incoming.llm_api_key.is_empty() {
        incoming.llm_api_key = current.llm_api_key.clone();
    }
    *current = incoming;
    state
        .db
        .set_singleton("settings", &*current)
        .map_err(AppError::Internal)?;
    Ok(Json(SettingsView::from(&*current)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRequest {
    endpoint: String,
    #[serde(default)]
    api_key: String,
}
#[derive(Serialize)]
pub struct Models {
    models: Vec<String>,
}
pub async fn list_models(
    State(state): State<AppState>,
    Json(body): Json<ModelRequest>,
) -> AppResult<Json<Models>> {
    let url = models_endpoint(&body.endpoint);
    let mut request = state.http.get(url);
    if !body.api_key.is_empty() {
        request = request.bearer_auth(body.api_key);
    }
    let value: serde_json::Value = request
        .send()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::BadRequest(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let mut models: Vec<String> = value
        .get("data")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|v| v.get("id").and_then(|v| v.as_str()).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    models.sort();
    Ok(Json(Models { models }))
}

#[cfg(test)]
mod model_endpoint_regression_tests {
    use super::*;
    use crate::config::Config;
    use axum::{Json as AxumJson, Router, routing::get};
    use serde_json::json;
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
    async fn full_transcription_endpoint_still_discovers_v1_models() {
        let app = Router::new().route(
            "/v1/models",
            get(|| async { AxumJson(json!({"data": [{"id": "demo-model"}]})) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let dir = tempfile::tempdir().unwrap();
        let state = AppState::load(config(dir.path())).await.unwrap();
        let response = list_models(
            State(state),
            Json(ModelRequest {
                endpoint: format!("http://{addr}/v1/audio/transcriptions"),
                api_key: String::new(),
            }),
        )
        .await
        .unwrap();
        assert_eq!(response.0.models, vec!["demo-model"]);
    }
}
