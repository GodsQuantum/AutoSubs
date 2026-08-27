use std::process::Stdio;
use tokio::process::Command;
use anyhow::{Context, Result};

pub async fn extract_audio(input_path: &str, output_wav: &str) -> Result<()> {
    let output = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", input_path,
            "-vn",
            "-ac", "1",
            "-ar", "16000",
            "-acodec", "pcm_s16le",
            output_wav
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to execute ffmpeg for audio extraction")?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("FFmpeg audio extraction failed: {}", err);
    }

    Ok(())
}
