use std::process::Stdio;
use tokio::process::Command;
use tokio::io::{AsyncBufReadExt, BufReader};
use anyhow::{Context, Result};
use crate::config::Config;
use crate::subtitle::types::Settings;

use crate::subtitle::types::Preset;

pub async fn generate_ass_and_burn(
    input_path: &str,
    output_path: &str,
    ass_path: &str,
    config: &Config,
    settings: &Settings, preset: &Preset,
    progress_tx: Option<tokio::sync::mpsc::Sender<u8>>,
) -> Result<()> {
    // Escape ASS path for FFmpeg filter
    let escaped_ass = ass_path
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "'\\''");

    let video_codec = if settings.hardware_accel == "nvenc" {
        "h264_nvenc"
    } else {
        config.video_codec.as_deref().unwrap_or("libx264")
    };

    // Probe to get duration for progress (simplified: assuming duration is known or ignoring exact percentage if ffprobe fails)
    let mut total_duration_secs = 0.0;
    if let Ok(probe) = Command::new("ffprobe")
        .args(["-v", "quiet", "-print_format", "json", "-show_format", input_path])
        .output().await 
    {
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&probe.stdout) {
            if let Some(dur_str) = json.pointer("/format/duration").and_then(|v| v.as_str()) {
                total_duration_secs = dur_str.parse::<f64>().unwrap_or(0.0);
            }
        }
    }

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-i", input_path,
            "-vf", &format!("{},ass='{}'", match preset.aspect_ratio { crate::subtitle::types::AspectRatio::Portrait => "crop='min(iw, ih*9/16)':'min(ih, iw*16/9)'", crate::subtitle::types::AspectRatio::Landscape => "crop='min(iw, ih*16/9)':'min(ih, iw*9/16)'", crate::subtitle::types::AspectRatio::Square => "crop='min(iw, ih)':'min(iw, ih)'", crate::subtitle::types::AspectRatio::Instagram => "crop='min(iw, ih*4/5)':'min(ih, iw*5/4)'" }, escaped_ass),
            "-c:v", video_codec,
            "-crf", &config.video_crf.to_string(),
            "-preset", &config.video_preset,
            "-c:a", "copy",
            "-movflags", "+faststart",
            "-pix_fmt", "yuv420p",
            output_path
        ])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .context("Failed to spawn ffmpeg for burning")?;

    if let Some(tx) = progress_tx {
        if let Some(stderr) = child.stderr.take() {
            let mut reader = BufReader::new(stderr).lines();
            let mut last_percent = 0;
            tokio::spawn(async move {
                while let Ok(Some(line)) = reader.next_line().await {
                    // Extract time=HH:MM:SS.ms
                    if let Some(time_idx) = line.find("time=") {
                        let time_str = &line[time_idx + 5..];
                        if let Some(end_idx) = time_str.find(' ') {
                            let ts = &time_str[..end_idx];
                            // Parse HH:MM:SS.ms
                            let parts: Vec<&str> = ts.split(':').collect();
                            if parts.len() == 3 {
                                let h: f64 = parts[0].parse().unwrap_or(0.0);
                                let m: f64 = parts[1].parse().unwrap_or(0.0);
                                let s: f64 = parts[2].parse().unwrap_or(0.0);
                                let current_secs = h * 3600.0 + m * 60.0 + s;
                                if total_duration_secs > 0.0 {
                                    let percent = ((current_secs / total_duration_secs) * 100.0) as u8;
                                    let percent = percent.min(100);
                                    if percent > last_percent {
                                        let _ = tx.send(percent).await;
                                        last_percent = percent;
                                    }
                                }
                            }
                        }
                    }
                }
            });
        }
    }

    let status = child.wait().await.context("Failed to wait on ffmpeg")?;
    if !status.success() {
        anyhow::bail!("FFmpeg burn process failed with status: {}", status);
    }

    Ok(())
}
