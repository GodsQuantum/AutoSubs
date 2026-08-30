use crate::{
    domain::{Brand, Preset, Workflow},
    error::{AppError, AppResult},
    state::AppState,
    workflows,
};
use axum::{
    Json,
    extract::{Path, State},
};
use uuid::Uuid;

pub async fn list_presets(State(state): State<AppState>) -> Json<Vec<Preset>> {
    Json(state.presets.read().await.clone())
}
pub async fn upsert_preset(
    State(state): State<AppState>,
    Json(mut preset): Json<Preset>,
) -> AppResult<Json<Preset>> {
    preset.migrate();
    if preset.name.trim().is_empty() {
        return Err(AppError::BadRequest("preset name is required".into()));
    }
    if let Some(brand_id) = &preset.brand_id
        && !state.brands.read().await.iter().any(|b| &b.id == brand_id)
    {
        return Err(AppError::BadRequest("unknown brandId".into()));
    }
    state
        .db
        .upsert("preset", &preset.id, &preset)
        .map_err(AppError::Internal)?;
    {
        let mut list = state.presets.write().await;
        if let Some(existing) = list.iter_mut().find(|p| p.id == preset.id) {
            *existing = preset.clone();
        } else {
            list.push(preset.clone());
        }
    }
    rebuild_brand_membership(&state).await?;
    Ok(Json(preset))
}
pub async fn delete_preset(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if !state.db.delete("preset", &id).map_err(AppError::Internal)? {
        return Err(AppError::NotFound("preset not found".into()));
    }
    state.presets.write().await.retain(|p| p.id != id);
    {
        let mut workflows = state.workflows.write().await;
        for w in workflows
            .iter_mut()
            .filter(|w| w.preset_id.as_deref() == Some(id.as_str()))
        {
            w.preset_id = None;
            state
                .db
                .upsert("workflow", &w.id, w)
                .map_err(AppError::Internal)?;
        }
    }
    rebuild_brand_membership(&state).await?;
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn list_brands(State(state): State<AppState>) -> Json<Vec<Brand>> {
    Json(state.brands.read().await.clone())
}
pub async fn upsert_brand(
    State(state): State<AppState>,
    Json(mut brand): Json<Brand>,
) -> AppResult<Json<Brand>> {
    if brand.id.trim().is_empty() {
        brand.id = Uuid::new_v4().to_string();
    }
    if brand.name.trim().is_empty() {
        return Err(AppError::BadRequest("brand name is required".into()));
    }
    let preset_ids = state
        .presets
        .read()
        .await
        .iter()
        .map(|p| p.id.clone())
        .collect::<std::collections::HashSet<_>>();
    if brand
        .default_preset_by_format
        .values()
        .any(|id| !preset_ids.contains(id))
    {
        return Err(AppError::BadRequest(
            "brand references an unknown preset".into(),
        ));
    }
    state
        .db
        .upsert("brand", &brand.id, &brand)
        .map_err(AppError::Internal)?;
    {
        let mut list = state.brands.write().await;
        if let Some(existing) = list.iter_mut().find(|b| b.id == brand.id) {
            *existing = brand.clone();
        } else {
            list.push(brand.clone());
        }
    }
    Ok(Json(brand))
}
pub async fn delete_brand(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if !state.db.delete("brand", &id).map_err(AppError::Internal)? {
        return Err(AppError::NotFound("brand not found".into()));
    }
    state.brands.write().await.retain(|b| b.id != id);
    {
        let mut presets = state.presets.write().await;
        for p in presets
            .iter_mut()
            .filter(|p| p.brand_id.as_deref() == Some(id.as_str()))
        {
            p.brand_id = None;
            state
                .db
                .upsert("preset", &p.id, p)
                .map_err(AppError::Internal)?;
        }
    }
    {
        let mut workflows = state.workflows.write().await;
        for w in workflows
            .iter_mut()
            .filter(|w| w.brand_id.as_deref() == Some(id.as_str()))
        {
            w.brand_id = None;
            state
                .db
                .upsert("workflow", &w.id, w)
                .map_err(AppError::Internal)?;
        }
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

pub async fn list_workflows(State(state): State<AppState>) -> Json<Vec<Workflow>> {
    Json(state.workflows.read().await.clone())
}
pub async fn upsert_workflow(
    State(state): State<AppState>,
    Json(mut workflow): Json<Workflow>,
) -> AppResult<Json<Workflow>> {
    if workflow.id.trim().is_empty() {
        workflow.id = Uuid::new_v4().to_string();
    }
    if workflow.name.trim().is_empty() {
        return Err(AppError::BadRequest("workflow name is required".into()));
    }
    for (field, value) in [
        ("watchDir", &workflow.watch_dir),
        ("outputDir", &workflow.output_dir),
        ("archiveDir", &workflow.archive_dir),
    ] {
        let path = std::path::Path::new(value);
        if !path.is_dir() || !state.config.path_is_allowed(path) {
            return Err(AppError::BadRequest(format!(
                "{field} must be an existing directory inside AUTOSUBS_ALLOWED_ROOTS"
            )));
        }
    }
    if let Some(id) = &workflow.preset_id
        && !state.presets.read().await.iter().any(|p| &p.id == id)
    {
        return Err(AppError::BadRequest("unknown presetId".into()));
    }
    if let Some(id) = &workflow.brand_id
        && !state.brands.read().await.iter().any(|b| &b.id == id)
    {
        return Err(AppError::BadRequest("unknown brandId".into()));
    }
    state
        .db
        .upsert("workflow", &workflow.id, &workflow)
        .map_err(AppError::Internal)?;
    {
        let mut list = state.workflows.write().await;
        if let Some(existing) = list.iter_mut().find(|w| w.id == workflow.id) {
            *existing = workflow.clone();
        } else {
            list.push(workflow.clone());
        }
    }
    workflows::reconcile(&state)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(workflow))
}
pub async fn delete_workflow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<axum::http::StatusCode> {
    if !state
        .db
        .delete("workflow", &id)
        .map_err(AppError::Internal)?
    {
        return Err(AppError::NotFound("workflow not found".into()));
    }
    state.workflows.write().await.retain(|w| w.id != id);
    if let Some((_, (_, token))) = state.watcher_tokens.remove(&id) {
        token.cancel();
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

async fn rebuild_brand_membership(state: &AppState) -> AppResult<()> {
    let presets = state.presets.read().await.clone();
    let mut brands = state.brands.write().await;
    for brand in brands.iter_mut() {
        brand.preset_ids = presets
            .iter()
            .filter(|p| p.brand_id.as_deref() == Some(brand.id.as_str()))
            .map(|p| p.id.clone())
            .collect();
        brand
            .default_preset_by_format
            .retain(|_, id| brand.preset_ids.contains(id));
        state
            .db
            .upsert("brand", &brand.id, brand)
            .map_err(AppError::Internal)?;
    }
    Ok(())
}
