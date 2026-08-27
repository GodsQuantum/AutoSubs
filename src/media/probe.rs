use crate::media::process::{ProcessError, run_capture};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Deserialize)]
struct ProbeRoot {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    format: Option<ProbeFormat>,
}
#[derive(Debug, Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
    format_name: Option<String>,
}
#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    duration: Option<String>,
}

fn parse_ratio(value: Option<&str>) -> f64 {
    let Some(value) = value else {
        return 0.0;
    };
    let Some((n, d)) = value.split_once('/') else {
        return value.parse().unwrap_or(0.0);
    };
    let n: f64 = n.parse().unwrap_or(0.0);
    let d: f64 = d.parse().unwrap_or(0.0);
    if d == 0.0 { 0.0 } else { n / d }
}

pub fn parse_probe_json(bytes: &[u8]) -> anyhow::Result<MediaProbe> {
    let root: ProbeRoot = serde_json::from_slice(bytes)?;
    let video = root
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"))
        .ok_or_else(|| anyhow::anyhow!("no video stream"))?;
    let audio = root
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("audio"));
    let duration = root
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|v| v.parse().ok())
        .or_else(|| video.duration.as_deref().and_then(|v| v.parse().ok()))
        .unwrap_or(0.0);
    Ok(MediaProbe {
        duration,
        width: video.width.unwrap_or(0),
        height: video.height.unwrap_or(0),
        fps: parse_ratio(video.avg_frame_rate.as_deref())
            .max(parse_ratio(video.r_frame_rate.as_deref())),
        video_codec: video.codec_name.clone().unwrap_or_default(),
        audio_codec: audio.and_then(|a| a.codec_name.clone()),
        has_audio: audio.is_some(),
        format_name: root.format.and_then(|f| f.format_name),
    })
}

pub async fn probe_media(
    path: &std::path::Path,
    token: &CancellationToken,
) -> Result<MediaProbe, ProcessError> {
    let mut command = Command::new("ffprobe");
    command
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path);
    let output = run_capture(command, token).await?;
    parse_probe_json(&output.stdout).map_err(|error| {
        ProcessError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        ))
    })
}

pub async fn is_video(path: &std::path::Path, token: &CancellationToken) -> bool {
    probe_media(path, token)
        .await
        .is_ok_and(|probe| probe.width > 0 && probe.height > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_ffprobe_fractional_fps() {
        let data = br#"{"streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":1080,"avg_frame_rate":"30000/1001"},{"codec_type":"audio","codec_name":"aac"}],"format":{"duration":"12.5","format_name":"mov,mp4"}}"#;
        let p = parse_probe_json(data).unwrap();
        assert!((p.fps - 29.970).abs() < 0.01);
        assert!(p.has_audio);
        assert_eq!(p.duration, 12.5);
    }
}
