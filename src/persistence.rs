use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn load_json_or_default<T>(path: &Path) -> Result<T>
where
    T: DeserializeOwned + Default,
{
    if !path.exists() { return Ok(T::default()); }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.is_empty() { return Ok(T::default()); }
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

pub fn load_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

pub fn atomic_write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let file_name = path.file_name().and_then(|v| v.to_str()).unwrap_or("autosubs.json");
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    let tmp: PathBuf = parent.join(format!(".{file_name}.{}.{}.tmp", std::process::id(), nonce));
    let bytes = serde_json::to_vec_pretty(value).context("serialize json")?;

    let result = (|| -> Result<()> {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)
            .with_context(|| format!("create {}", tmp.display()))?;
        file.write_all(&bytes).with_context(|| format!("write {}", tmp.display()))?;
        file.write_all(b"\n")?;
        file.sync_all().with_context(|| format!("sync {}", tmp.display()))?;
        drop(file);
        fs::rename(&tmp, path).with_context(|| format!("rename {} -> {}", tmp.display(), path.display()))?;
        sync_directory(parent)?;
        Ok(())
    })();

    if result.is_err() { let _ = fs::remove_file(&tmp); }
    result
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let dir = File::open(path).with_context(|| format!("open dir {}", path.display()))?;
        dir.sync_all().with_context(|| format!("sync dir {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn atomic_write_replaces_complete_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        atomic_write_json(&path, &serde_json::json!({"a": 1})).unwrap();
        atomic_write_json(&path, &serde_json::json!({"a": 2, "b": true})).unwrap();
        let value: serde_json::Value = load_json(&path).unwrap();
        assert_eq!(value["a"], 2);
        assert_eq!(value["b"], true);
    }
}
