use clap::Parser;
use std::path::PathBuf;

#[derive(Debug, Clone, Parser)]
#[command(version, about = "AutoSubs API-first subtitle production server")]
pub struct Config {
    #[arg(long, env = "HOST", default_value = "0.0.0.0")]
    pub host: String,
    #[arg(long, env = "PORT", default_value_t = 3000)]
    pub port: u16,
    #[arg(long, env = "DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,
    #[arg(long, env = "DIST_DIR", default_value = "./frontend/build")]
    pub dist_dir: PathBuf,
    #[arg(long, env = "OUTPUT_DIR", default_value = "./output")]
    pub output_dir: PathBuf,
    #[arg(long, env = "FONTS_DIR", default_value = "/usr/share/fonts")]
    pub fonts_dir: PathBuf,
    #[arg(long, env = "MAX_ENCODE_JOBS", default_value_t = 2)]
    pub max_encode_jobs: usize,
    #[arg(long, env = "MAX_UPLOAD_BYTES", default_value_t = 34_359_738_368u64)]
    pub max_upload_bytes: u64,
    #[arg(long, env = "BROWSE_ROOTS", value_delimiter = ',')]
    pub browse_roots: Vec<PathBuf>,
    #[arg(long, env = "WATCH_POLL_SECONDS", default_value_t = 10)]
    pub watch_poll_seconds: u64,
    #[arg(long, env = "WATCH_STABLE_MILLIS", default_value_t = 2000)]
    pub watch_stable_millis: u64,
}

impl Config {
    pub fn uploads_dir(&self) -> PathBuf { self.data_dir.join("uploads") }
    pub fn jobs_dir(&self) -> PathBuf { self.data_dir.join("jobs") }
    pub fn outros_dir(&self) -> PathBuf { self.data_dir.join("outros") }
    pub fn presets_file(&self) -> PathBuf { self.data_dir.join("presets.json") }
    pub fn brands_file(&self) -> PathBuf { self.data_dir.join("brands.json") }
    pub fn workflows_file(&self) -> PathBuf { self.data_dir.join("workflows.json") }
    pub fn settings_file(&self) -> PathBuf { self.data_dir.join("settings.json") }

    pub fn effective_browse_roots(&self) -> Vec<PathBuf> {
        if !self.browse_roots.is_empty() { return self.browse_roots.clone(); }
        vec![
            self.data_dir.clone(),
            PathBuf::from("./watch"),
            self.output_dir.clone(),
            PathBuf::from("./archives"),
        ]
    }

    pub fn init_dirs(&self) -> anyhow::Result<()> {
        for path in [self.data_dir.clone(), self.uploads_dir(), self.jobs_dir(), self.outros_dir(), self.output_dir.clone()] {
            std::fs::create_dir_all(path)?;
        }
        Ok(())
    }
}
