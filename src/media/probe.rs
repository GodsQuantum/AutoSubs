use crate::media::process::{run_capture, ProcessError};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaProbe {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub video_codec: String,
    pub audio_codec: Option<String>,
    pub has_audio: bool,
    pub format_name: Option<String>,
}

fn parse_fraction(value: &str) -> f64 {
    if let Some((a, b)) = value.split_once('/') {
        let numerator: f64 = a.parse().unwrap_or(0.0);
        let denominator: f64 = b.parse().unwrap_or(1.0);
        if denominator.abs() > f64::EPSILON { numerator / denominator } else { 0.0 }
    } else { value.parse().unwrap_or(0.0) }
}

pub fn parse_probe_json(bytes: &[u8]) -> anyhow::Result<MediaProbe> {
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let streams = value.get("streams").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let video = streams.iter().find(|stream| stream.get("codec_type").and_then(|v| v.as_str()) == Some("video"))
        .ok_or_else(|| anyhow::anyhow!("no video stream found"))?;
    let audio = streams.iter().find(|stream| stream.get("codec_type").and_then(|v| v.as_str()) == Some("audio"));
    let format = value.get("format");
    let duration = format.and_then(|v| v.get("duration")).and_then(|v| v.as_str()).and_then(|v| v.parse().ok())
        .or_else(|| video.get("duration").and_then(|v| v.as_str()).and_then(|v| v.parse().ok()))
        .unwrap_or(0.0);
    let fps = video.get("avg_frame_rate").and_then(|v| v.as_str()).map(parse_fraction)
        .filter(|v| *v > 0.0)
        .or_else(|| video.get("r_frame_rate").and_then(|v| v.as_str()).map(parse_fraction))
        .unwrap_or(30.0);

    Ok(MediaProbe {
        duration,
        width: video.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        height: video.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        fps,
        video_codec: video.get("codec_name").and_then(|v| v.as_str()).unwrap_or("unknown").into(),
        audio_codec: audio.and_then(|v| v.get("codec_name")).and_then(|v| v.as_str()).map(str::to_string),
        has_audio: audio.is_some(),
        format_name: format.and_then(|v| v.get("format_name")).and_then(|v| v.as_str()).map(str::to_string),
    })
}

pub async fn probe_media(path: &Path, token: &CancellationToken) -> Result<MediaProbe, ProcessError> {
    let mut cmd = Command::new("ffprobe");
    cmd.args(["-v", "error", "-print_format", "json", "-show_streams", "-show_format"])
        .arg(path);
    let output = run_capture(cmd, token).await?;
    parse_probe_json(&output.stdout).map_err(|error| ProcessError::Io(std::io::Error::other(error.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_video_audio_and_fractional_fps() {
        let json = br#"{
          "streams": [
            {"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001"},
            {"codec_type":"audio","codec_name":"aac"}
          ],
          "format":{"duration":"12.5","format_name":"mov,mp4"}
        }"#;
        let probe = parse_probe_json(json).unwrap();
        assert_eq!(probe.width, 1920);
        assert_eq!(probe.height, 1080);
        assert!(probe.has_audio);
        assert!((probe.fps - 29.970).abs() < 0.01);
    }
}
