use axum::{extract::Query, Json};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
pub struct BrowseQuery {
    path: Option<String>,
}

#[derive(Serialize)]
pub struct FileEntry {
    name: String,
    path: String,
    is_dir: bool,
}

#[derive(Serialize)]
pub struct BrowseResponse {
    current_path: String,
    parent_path: Option<String>,
    entries: Vec<FileEntry>,
}

pub async fn browse_directory(Query(query): Query<BrowseQuery>) -> Json<BrowseResponse> {
    let current_path = query.path.unwrap_or_else(|| "/".to_string());
    let path = Path::new(&current_path);

    let mut entries = Vec::new();
    let parent_path = path.parent().map(|p| p.to_string_lossy().to_string());

    if let Ok(read_dir) = fs::read_dir(path) {
        for entry in read_dir.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if metadata.is_dir() {
                    entries.push(FileEntry {
                        name: entry.file_name().to_string_lossy().to_string(),
                        path: entry.path().to_string_lossy().to_string(),
                        is_dir: true,
                    });
                }
            }
        }
    }

    entries.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Json(BrowseResponse {
        current_path,
        parent_path,
        entries,
    })
}
