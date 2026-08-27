use anyhow::{Context, Result, bail};
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Parser)]
#[command(name = "autosubs", version, about)]
pub struct Config {
    #[arg(long, env = "AUTOSUBS_HOST", default_value = "0.0.0.0")]
    pub host: String,
    #[arg(long, env = "AUTOSUBS_PORT", default_value_t = 3000)]
    pub port: u16,
    #[arg(long, env = "AUTOSUBS_CONFIG_DIR", default_value = "/config")]
    pub config_dir: PathBuf,
    #[arg(long, env = "AUTOSUBS_DATA_DIR", default_value = "/data")]
    pub data_dir: PathBuf,
    #[arg(long, env = "AUTOSUBS_FONTS_DIR", default_value = "/fonts")]
    pub fonts_dir: PathBuf,
    #[arg(long, env = "AUTOSUBS_DIST_DIR", default_value = "/app/frontend")]
    pub dist_dir: PathBuf,
    #[arg(long, env = "AUTOSUBS_ALLOWED_ROOTS", value_delimiter = ':')]
    pub allowed_roots: Vec<PathBuf>,
    #[arg(long, env = "AUTOSUBS_MAX_RENDER_JOBS", default_value_t = 2)]
    pub max_render_jobs: usize,
    #[arg(long, env = "AUTOSUBS_MAX_TRANSCRIPTION_JOBS", default_value_t = 2)]
    pub max_transcription_jobs: usize,
    #[arg(long, env = "AUTOSUBS_MAX_QUEUED_JOBS", default_value_t = 256)]
    pub max_queued_jobs: usize,
    #[arg(long, env = "AUTOSUBS_WORKFLOW_SCAN_SECONDS", default_value_t = 5)]
    pub workflow_scan_seconds: u64,
    #[arg(long, env = "AUTOSUBS_FILE_STABILITY_MS", default_value_t = 2000)]
    pub file_stability_ms: u64,
    #[arg(long, env = "AUTOSUBS_MAX_UPLOAD_BYTES", default_value_t = 53_687_091_200u64)]
    pub max_upload_bytes: u64,
}

impl Config {
    pub fn init_dirs(&mut self) -> Result<()> {
        if let Ok(value) = std::env::var("DATA_DIR") {
            if std::env::var_os("AUTOSUBS_DATA_DIR").is_none() { self.data_dir = PathBuf::from(value); }
        }
        if let Ok(value) = std::env::var("FONTS_DIR") {
            if std::env::var_os("AUTOSUBS_FONTS_DIR").is_none() { self.fonts_dir = PathBuf::from(value); }
        }
        if let Ok(value) = std::env::var("MAX_ENCODE_JOBS") {
            if std::env::var_os("AUTOSUBS_MAX_RENDER_JOBS").is_none() {
                if let Ok(parsed) = value.parse() { self.max_render_jobs = parsed; }
            }
        }
        for path in [&self.config_dir, &self.data_dir] { std::fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?; }
        for path in [self.uploads_dir(), self.outputs_dir(), self.assets_dir(), self.work_dir()] {
            std::fs::create_dir_all(&path).with_context(|| format!("create {}", path.display()))?;
        }
        if self.allowed_roots.is_empty() { self.allowed_roots = vec![self.data_dir.clone()]; }
        self.allowed_roots = self.allowed_roots.iter().filter_map(|p| std::fs::canonicalize(p).ok()).collect();
        if self.allowed_roots.is_empty() { bail!("AUTOSUBS_ALLOWED_ROOTS does not contain an existing directory"); }
        ensure_sqlite_local(&self.config_dir)?;
        Ok(())
    }

    pub fn db_file(&self) -> PathBuf { self.config_dir.join("autosubs.sqlite3") }
    pub fn uploads_dir(&self) -> PathBuf { self.data_dir.join("uploads") }
    pub fn outputs_dir(&self) -> PathBuf { self.data_dir.join("outputs") }
    pub fn assets_dir(&self) -> PathBuf { self.data_dir.join("assets") }
    pub fn work_dir(&self) -> PathBuf { self.data_dir.join("work") }

    pub fn path_is_allowed(&self, path: &Path) -> bool {
        let Ok(canon) = std::fs::canonicalize(path) else { return false; };
        self.allowed_roots.iter().any(|root| canon.starts_with(root))
    }
}

fn unescape_mount(value: &str) -> String {
    value.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\134", "\\")
}

pub fn ensure_sqlite_local(config_dir: &Path) -> Result<()> {
    #[cfg(not(target_os = "linux"))]
    { let _ = config_dir; return Ok(()); }
    #[cfg(target_os = "linux")]
    {
        let canon = std::fs::canonicalize(config_dir).with_context(|| format!("canonicalize {}", config_dir.display()))?;
        let mounts = std::fs::read_to_string("/proc/self/mountinfo").context("read mount table")?;
        let mut best: Option<(usize, String, String)> = None;
        for line in mounts.lines() {
            let Some((left, right)) = line.split_once(" - ") else { continue; };
            let fields: Vec<&str> = left.split_whitespace().collect();
            let rfields: Vec<&str> = right.split_whitespace().collect();
            if fields.len() < 5 || rfields.is_empty() { continue; }
            let mount_point = PathBuf::from(unescape_mount(fields[4]));
            if canon.starts_with(&mount_point) {
                let len = mount_point.as_os_str().len();
                if best.as_ref().is_none_or(|(best_len, _, _)| len > *best_len) {
                    best = Some((len, mount_point.display().to_string(), rfields[0].to_string()));
                }
            }
        }
        if let Some((_, mount, fs)) = best {
            let lower = fs.to_ascii_lowercase();
            let network = ["nfs", "cifs", "smb", "fuse.sshfs", "sshfs", "9p", "ceph", "glusterfs"];
            if network.iter().any(|name| lower == *name || lower.starts_with(&format!("{name}."))) {
                bail!("AUTOSUBS_CONFIG_DIR={} is on network filesystem {} mounted at {}. SQLite WAL must use local storage", canon.display(), fs, mount);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mount_escape_decoder_handles_space() { assert_eq!(unescape_mount("/mnt/My\\040Disk"), "/mnt/My Disk"); }
}
