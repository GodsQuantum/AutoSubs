use std::path::PathBuf;
use clap::Parser;

/// AutoSubs — automated animated subtitle server
#[derive(Debug, Clone, Parser)]
#[command(version, about)]
pub struct Config {
    /// Port to listen on
    #[arg(long, env = "PORT", default_value = "3000")]
    pub port: u16,

    /// Persistent data directory (presets, settings, jobs, uploads)
    #[arg(long, env = "DATA_DIR", default_value = "./data")]
    pub data_dir: PathBuf,

    /// Custom fonts directory
    #[arg(long, env = "FONTS_DIR", default_value = "/usr/share/fonts/custom")]
    pub fonts_dir: PathBuf,

    /// Static frontend dist directory (production)
    #[arg(long, env = "DIST_DIR", default_value = "./dist")]
    pub dist_dir: PathBuf,

    /// Max concurrent FFmpeg encode jobs
    #[arg(long, env = "MAX_ENCODE_JOBS", default_value = "2")]
    pub max_encode_jobs: usize,

    /// FFmpeg video CRF (quality; lower = bigger file)
    #[arg(long, env = "VIDEO_CRF", default_value = "20")]
    pub video_crf: u8,

    /// FFmpeg video preset (ultrafast..veryslow)
    #[arg(long, env = "VIDEO_PRESET", default_value = "medium")]
    pub video_preset: String,

    /// Force video codec (e.g. libx264, h264_nvenc)
    #[arg(long, env = "VIDEO_CODEC")]
    pub video_codec: Option<String>,
}

impl Config {
    pub fn uploads_dir(&self) -> PathBuf {
        self.data_dir.join("uploads")
    }
    pub fn jobs_dir(&self) -> PathBuf {
        self.data_dir.join("jobs")
    }
    pub fn outros_dir(&self) -> PathBuf {
        self.data_dir.join("outros")
    }
    pub fn presets_file(&self) -> PathBuf {
        self.data_dir.join("presets.json")
    }
    pub fn settings_file(&self) -> PathBuf {
        self.data_dir.join("settings.json")
    }
    pub fn workflows_file(&self) -> PathBuf {
        self.data_dir.join("workflows.json")
    }

    /// Ensure all required directories exist
    pub fn init_dirs(&self) -> anyhow::Result<()> {
        for dir in [
            &self.data_dir,
            &self.fonts_dir,
            &self.uploads_dir(),
            &self.jobs_dir(),
            &self.outros_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}
