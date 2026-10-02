use crate::{
    error::{AppError, AppResult},
    state::AppState,
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const FAVORITES_KEY: &str = "browse_favorites";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseQuery {
    #[serde(default)]
    path: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    extensions: String,
}
fn default_mode() -> String {
    "any".into()
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    name: String,
    path: String,
    is_dir: bool,
    size: Option<u64>,
    modified_ms: Option<u128>,
    selectable: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowseResponse {
    current_path: String,
    parent_path: Option<String>,
    entries: Vec<Entry>,
    roots: Vec<String>,
    favorites: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteRequest {
    path: String,
    favorite: bool,
}

fn canonical_favorite(config: &crate::config::Config, path: &str) -> anyhow::Result<String> {
    config
        .resolve_allowed_dir(Path::new(path))?
        .to_str()
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("favorite path is not valid UTF-8"))
}

fn valid_favorites(state: &AppState) -> AppResult<Vec<String>> {
    let stored = state
        .db
        .get_singleton::<Vec<String>>(FAVORITES_KEY)
        .map_err(AppError::Internal)?
        .unwrap_or_default();
    let mut valid = Vec::new();
    for path in &stored {
        if let Ok(canonical) = canonical_favorite(&state.config, path)
            && !valid.contains(&canonical)
        {
            valid.push(canonical);
        }
    }
    if valid != stored {
        state
            .db
            .set_singleton(FAVORITES_KEY, &valid)
            .map_err(AppError::Internal)?;
    }
    Ok(valid)
}

pub async fn favorites(State(state): State<AppState>) -> AppResult<Json<Vec<String>>> {
    Ok(Json(valid_favorites(&state)?))
}

pub async fn set_favorite(
    State(state): State<AppState>,
    Json(request): Json<FavoriteRequest>,
) -> AppResult<Json<Vec<String>>> {
    let mut favorites = valid_favorites(&state)?;
    if request.favorite {
        let canonical = canonical_favorite(&state.config, &request.path).map_err(|_| {
            AppError::BadRequest(
                "favorite must be an existing directory inside AUTOSUBS_ALLOWED_ROOTS".into(),
            )
        })?;
        if !favorites.contains(&canonical) {
            favorites.push(canonical);
        }
    } else {
        let canonical =
            canonical_favorite(&state.config, &request.path).unwrap_or(request.path.clone());
        favorites.retain(|path| path != &canonical && path != &request.path);
    }
    favorites.sort();
    favorites.dedup();
    state
        .db
        .set_singleton(FAVORITES_KEY, &favorites)
        .map_err(AppError::Internal)?;
    Ok(Json(favorites))
}

pub async fn browse(
    State(state): State<AppState>,
    Query(query): Query<BrowseQuery>,
) -> AppResult<Json<BrowseResponse>> {
    if !matches!(query.mode.as_str(), "directory" | "file" | "any") {
        return Err(AppError::BadRequest(
            "mode must be directory, file or any".into(),
        ));
    }
    let roots: Vec<String> = state
        .config
        .allowed_roots
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let favorites = valid_favorites(&state)?;
    let current = if query.path.trim().is_empty() {
        state
            .config
            .allowed_roots
            .first()
            .cloned()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("no browse roots")))?
    } else {
        PathBuf::from(&query.path)
    };
    if !current.is_dir() {
        return Err(AppError::BadRequest("path is not a directory".into()));
    }
    let current = std::fs::canonicalize(&current)?;
    let root = state
        .config
        .allowed_roots
        .iter()
        .filter(|r| current.starts_with(r))
        .max_by_key(|r| r.as_os_str().len())
        .ok_or_else(|| AppError::Forbidden("path is outside allowed roots".into()))?;
    let extensions = query
        .extensions
        .split(',')
        .map(|v| v.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    let mut dir = tokio::fs::read_dir(&current).await?;
    let mut entries = Vec::new();
    while let Some(item) = dir.next_entry().await? {
        let path = item.path();
        let Ok(meta) = item.metadata().await else {
            continue;
        };
        let is_dir = meta.is_dir();
        if !is_dir && !meta.is_file() {
            continue;
        }
        if !is_dir && !extensions.is_empty() {
            let ext = path
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            if !extensions.contains(&ext) {
                continue;
            }
        }
        let selectable = match query.mode.as_str() {
            "directory" => is_dir,
            "file" => !is_dir,
            _ => true,
        };
        entries.push(Entry {
            name: item.file_name().to_string_lossy().into_owned(),
            path: path.to_string_lossy().into_owned(),
            is_dir,
            size: (!is_dir).then_some(meta.len()),
            modified_ms: meta
                .modified()
                .ok()
                .and_then(|v| v.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|v| v.as_millis()),
            selectable,
        });
    }
    entries.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    let parent_path = bounded_parent(&current, root).map(|p| p.to_string_lossy().into_owned());
    Ok(Json(BrowseResponse {
        current_path: current.to_string_lossy().into_owned(),
        parent_path,
        entries,
        roots,
        favorites,
    }))
}

fn bounded_parent<'a>(current: &'a Path, root: &Path) -> Option<&'a Path> {
    if current == root {
        None
    } else {
        current.parent().filter(|parent| parent.starts_with(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parent_never_escapes_root() {
        let root = Path::new("/mnt/media");
        assert!(bounded_parent(root, root).is_none());
        assert_eq!(bounded_parent(Path::new("/mnt/media/a"), root), Some(root));
    }

    #[test]
    fn favorite_folder_must_be_an_existing_allowed_directory() {
        let temp = tempfile::tempdir().unwrap();
        let allowed = temp.path().join("allowed");
        let favorite = allowed.join("nested");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&favorite).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let mut config = crate::config::Config {
            host: "127.0.0.1".into(),
            port: 3000,
            config_dir: temp.path().join("config"),
            data_dir: temp.path().join("data"),
            fonts_dir: temp.path().join("fonts"),
            dist_dir: temp.path().join("dist"),
            allowed_roots: vec![allowed],
            max_render_jobs: 1,
            max_transcription_jobs: 1,
            max_queued_jobs: 1,
            workflow_scan_seconds: 5,
            file_stability_ms: 10,
            max_upload_bytes: 1024,
        };
        config.init_dirs().unwrap();
        assert_eq!(
            canonical_favorite(&config, favorite.to_str().unwrap()).unwrap(),
            std::fs::canonicalize(favorite).unwrap().to_string_lossy()
        );
        assert!(canonical_favorite(&config, outside.to_str().unwrap()).is_err());
    }
}
