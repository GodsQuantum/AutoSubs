use crate::domain::{Encoder, EncoderKind, FitMode, FormatKey, Preset};
use crate::media::probe::MediaProbe;
use crate::media::process::ProcessError;
use std::{collections::BTreeMap, path::Path, time::Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EncoderCapabilities {
    pub ffmpeg: bool,
    pub h264_nvenc: bool,
    pub hevc_nvenc: bool,
    pub h264_qsv: bool,
    pub h264_vaapi: bool,
    pub h264_vulkan: bool,
    pub h264_amf: bool,
    pub vaapi_device: Option<String>,
    pub vulkan_device: Option<String>,
    pub h264_benchmarks_ms: BTreeMap<String, u64>,
    pub auto_encoder_order: Vec<EncoderKind>,
    pub libass: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderPlan {
    pub args: Vec<String>,
    pub target_resolution: (u32, u32),
    pub encoder: EncoderKind,
}

fn ffmpeg_filter_escape(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .replace(':', "\\:")
        .replace('\'', "'\\''")
}

pub fn auto_encoder_order(caps: &EncoderCapabilities) -> Vec<EncoderKind> {
    if !caps.auto_encoder_order.is_empty() {
        return caps.auto_encoder_order.clone();
    }
    [
        (caps.h264_nvenc, EncoderKind::NvencH264),
        (caps.h264_qsv, EncoderKind::QsvH264),
        (caps.h264_vaapi, EncoderKind::VaapiH264),
        (caps.h264_vulkan, EncoderKind::VulkanH264),
        (caps.h264_amf, EncoderKind::AmfH264),
    ]
    .into_iter()
    .filter_map(|(available, encoder)| available.then_some(encoder))
    .collect()
}

fn resolved_encoder(settings: &Encoder, caps: &EncoderCapabilities) -> EncoderKind {
    match settings.kind {
        EncoderKind::Auto => auto_encoder_order(caps)
            .into_iter()
            .next()
            .unwrap_or(EncoderKind::Libx264),
        ref explicit => explicit.clone(),
    }
}

fn encoder_args(encoder: &EncoderKind, quality: u8, preset: &str) -> Vec<String> {
    let quality = quality.clamp(0, 51).to_string();
    match encoder {
        EncoderKind::Libx264 => vec![
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            quality,
            "-preset".into(),
            preset.into(),
        ],
        EncoderKind::Libx265 => vec![
            "-c:v".into(),
            "libx265".into(),
            "-crf".into(),
            quality,
            "-preset".into(),
            preset.into(),
        ],
        EncoderKind::NvencH264 => vec![
            "-c:v".into(),
            "h264_nvenc".into(),
            "-cq".into(),
            quality,
            "-preset".into(),
            nvenc_preset(preset).into(),
        ],
        EncoderKind::NvencHevc => vec![
            "-c:v".into(),
            "hevc_nvenc".into(),
            "-cq".into(),
            quality,
            "-preset".into(),
            nvenc_preset(preset).into(),
        ],
        EncoderKind::QsvH264 => vec![
            "-c:v".into(),
            "h264_qsv".into(),
            "-global_quality".into(),
            quality,
        ],
        EncoderKind::VaapiH264 => vec!["-c:v".into(), "h264_vaapi".into(), "-qp".into(), quality],
        EncoderKind::VulkanH264 => vec![
            "-c:v".into(),
            "h264_vulkan".into(),
            "-qp".into(),
            quality,
            "-usage".into(),
            "transcode".into(),
        ],
        EncoderKind::AmfH264 => vec![
            "-c:v".into(),
            "h264_amf".into(),
            "-qp_i".into(),
            quality.clone(),
            "-qp_p".into(),
            quality,
        ],
        EncoderKind::Auto => unreachable!("encoder must be resolved first"),
    }
}

fn uses_hardware_upload(encoder: &EncoderKind) -> bool {
    matches!(encoder, EncoderKind::VaapiH264 | EncoderKind::VulkanH264)
}

fn nvenc_preset(value: &str) -> &'static str {
    match value.to_ascii_lowercase().as_str() {
        "ultrafast" | "superfast" | "veryfast" | "faster" | "fast" => "p3",
        "slow" | "slower" | "veryslow" => "p7",
        _ => "p5",
    }
}

fn target_resolution(preset: &Preset, source: &MediaProbe) -> anyhow::Result<(u32, u32)> {
    let source_resolution = (source.width, source.height);
    let target = preset
        .format
        .resolution(Some(source_resolution))
        .unwrap_or(source_resolution);
    if target.0 == 0 || target.1 == 0 {
        anyhow::bail!("invalid target resolution");
    }
    if preset.format.key != FormatKey::Source && preset.format.fit == FitMode::Preserve {
        anyhow::bail!("fit=preserve is only valid with source format");
    }
    Ok(target)
}

fn geometry_chain(preset: &Preset, source: &MediaProbe) -> anyhow::Result<String> {
    if preset.format.key == FormatKey::Source || preset.format.fit == FitMode::Preserve {
        return Ok(String::new());
    }
    let (w, h) = target_resolution(preset, source)?;
    if (w, h) == (source.width, source.height) {
        return Ok(String::new());
    }
    Ok(match preset.format.fit {
        FitMode::Contain => format!(
            "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1"
        ),
        FitMode::Cover => {
            format!("scale={w}:{h}:force_original_aspect_ratio=increase,crop={w}:{h},setsar=1")
        }
        FitMode::Stretch => format!("scale={w}:{h},setsar=1"),
        FitMode::Preserve => String::new(),
    })
}

fn chain(parts: impl IntoIterator<Item = String>) -> String {
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(",")
}

#[expect(
    clippy::too_many_arguments,
    reason = "render-plan construction keeps independent FFmpeg resources explicit"
)]
pub fn build_render_plan(
    input: &Path,
    output: &Path,
    ass: &Path,
    preset: &Preset,
    encoder_settings: &Encoder,
    caps: &EncoderCapabilities,
    source: &MediaProbe,
    outro: Option<(&Path, &MediaProbe)>,
    fonts_dir: Option<&Path>,
) -> anyhow::Result<RenderPlan> {
    let target = target_resolution(preset, source)?;
    let encoder = resolved_encoder(encoder_settings, caps);
    let ass_filter = match fonts_dir {
        Some(fonts) => format!(
            "ass='{}':fontsdir='{}'",
            ffmpeg_filter_escape(ass),
            ffmpeg_filter_escape(fonts)
        ),
        None => format!("ass='{}'", ffmpeg_filter_escape(ass)),
    };
    let hardware_upload = uses_hardware_upload(&encoder).then(|| "format=nv12,hwupload".to_owned());
    let main_video = chain([
        geometry_chain(preset, source)?,
        ass_filter,
        hardware_upload.clone().unwrap_or_default(),
    ]);
    let mut args = vec!["-y".into()];
    match &encoder {
        EncoderKind::VaapiH264 => {
            let device = caps.vaapi_device.as_deref().ok_or_else(|| {
                anyhow::anyhow!("VA-API encoder selected but no usable render device was detected")
            })?;
            args.extend(["-vaapi_device".into(), device.into()]);
        }
        EncoderKind::VulkanH264 => {
            let device = caps.vulkan_device.as_deref().ok_or_else(|| {
                anyhow::anyhow!("Vulkan encoder selected but no usable Vulkan device was detected")
            })?;
            args.extend([
                "-init_hw_device".into(),
                format!("vulkan=vk:{device}"),
                "-filter_hw_device".into(),
                "vk".into(),
            ]);
        }
        _ => {}
    }
    args.extend(["-i".into(), input.to_string_lossy().into_owned()]);

    match outro {
        None => {
            args.extend(["-vf".into(), main_video]);
            args.extend(encoder_args(
                &encoder,
                encoder_settings.quality,
                &encoder_settings.preset,
            ));
            if source.has_audio {
                args.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "192k".into()]);
            } else {
                args.push("-an".into());
            }
            args.extend(["-movflags".into(), "+faststart".into()]);
            if !uses_hardware_upload(&encoder) {
                args.extend(["-pix_fmt".into(), "yuv420p".into()]);
            }
        }
        Some((outro_path, outro_probe)) => {
            args.extend(["-i".into(), outro_path.to_string_lossy().into_owned()]);
            let (w, h) = target;
            let fps = if source.fps > 0.0 { source.fps } else { 30.0 };
            let outro_video = format!(
                "scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,fps={fps:.6},setsar=1"
            );
            let main_audio = if source.has_audio {
                format!(
                    "[0:a]aresample=48000,aformat=channel_layouts=stereo,apad,atrim=duration={:.6}[maina]",
                    source.duration.max(0.01)
                )
            } else {
                format!(
                    "anullsrc=r=48000:cl=stereo:d={:.6}[maina]",
                    source.duration.max(0.01)
                )
            };
            let outro_audio = if outro_probe.has_audio {
                format!(
                    "[1:a]aresample=48000,aformat=channel_layouts=stereo,apad,atrim=duration={:.6}[outa]",
                    outro_probe.duration.max(0.01)
                )
            } else {
                format!(
                    "anullsrc=r=48000:cl=stereo:d={:.6}[outa]",
                    outro_probe.duration.max(0.01)
                )
            };
            let complex = if uses_hardware_upload(&encoder) {
                let main_video_cpu = chain([
                    geometry_chain(preset, source)?,
                    match fonts_dir {
                        Some(fonts) => format!(
                            "ass='{}':fontsdir='{}'",
                            ffmpeg_filter_escape(ass),
                            ffmpeg_filter_escape(fonts)
                        ),
                        None => format!("ass='{}'", ffmpeg_filter_escape(ass)),
                    },
                ]);
                format!(
                    "[0:v]{main_video_cpu}[mainv];[1:v]{outro_video}[outv];{main_audio};{outro_audio};[mainv][maina][outv][outa]concat=n=2:v=1:a=1[vjoin][aout];[vjoin]format=nv12,hwupload[vout]"
                )
            } else {
                format!(
                    "[0:v]{main_video}[mainv];[1:v]{outro_video}[outv];{main_audio};{outro_audio};[mainv][maina][outv][outa]concat=n=2:v=1:a=1[vout][aout]"
                )
            };
            args.extend([
                "-filter_complex".into(),
                complex,
                "-map".into(),
                "[vout]".into(),
                "-map".into(),
                "[aout]".into(),
            ]);
            args.extend(encoder_args(
                &encoder,
                encoder_settings.quality,
                &encoder_settings.preset,
            ));
            args.extend([
                "-c:a".into(),
                "aac".into(),
                "-b:a".into(),
                "192k".into(),
                "-movflags".into(),
                "+faststart".into(),
            ]);
            if !uses_hardware_upload(&encoder) {
                args.extend(["-pix_fmt".into(), "yuv420p".into()]);
            }
        }
    }
    args.extend([
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
        output.to_string_lossy().into_owned(),
    ]);
    Ok(RenderPlan {
        args,
        target_resolution: target,
        encoder,
    })
}

pub async fn render_video(
    plan: &RenderPlan,
    duration: f64,
    token: &CancellationToken,
    progress_tx: Option<mpsc::Sender<u8>>,
) -> Result<(), ProcessError> {
    let mut command = Command::new("ffmpeg");
    command
        .args(&plan.args)
        .kill_on_drop(true)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
        program: "ffmpeg".into(),
        source,
    })?;
    let stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    let progress_task = tokio::spawn(async move {
        let mut reader = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if let Some(value) = line.strip_prefix("out_time_ms=")
                && let (Ok(micros), Some(tx)) = (value.parse::<f64>(), progress_tx.as_ref())
                && duration > 0.0
            {
                let pct = ((micros / 1_000_000.0) / duration * 100.0)
                    .round()
                    .clamp(0.0, 99.0) as u8;
                let _ = tx.send(pct).await;
            }
        }
    });
    let stderr_task = tokio::spawn(async move {
        let mut data = Vec::new();
        stderr.read_to_end(&mut data).await.map(|_| data)
    });

    let status = tokio::select! {
        result = child.wait() => result?,
        _ = token.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            progress_task.abort();
            stderr_task.abort();
            return Err(ProcessError::Cancelled);
        }
    };
    let _ = progress_task.await;
    let stderr = stderr_task
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))??;
    if !status.success() {
        return Err(ProcessError::Failed {
            status,
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    Ok(())
}

fn encoder_is_listed(text: &str, name: &str) -> bool {
    text.lines()
        .any(|line| line.split_whitespace().any(|field| field == name))
}

async fn probe_command(command: Command, token: &CancellationToken) -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(4),
            crate::media::process::run_capture(command, token),
        )
        .await,
        Ok(Ok(_))
    )
}

async fn timed_probe(command: Command, token: &CancellationToken) -> Option<u64> {
    let started = Instant::now();
    match tokio::time::timeout(
        std::time::Duration::from_secs(8),
        crate::media::process::run_capture(command, token),
    )
    .await
    {
        Ok(Ok(_)) => Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
        _ => None,
    }
}

async fn probe_quick_encoder(name: &str, token: &CancellationToken) -> bool {
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=128x128:r=1",
        "-frames:v",
        "1",
        "-an",
        "-c:v",
        name,
        "-f",
        "null",
        "-",
    ]);
    probe_command(command, token).await
}

async fn benchmark_software_encoder(
    name: &str,
    quality_args: &[&str],
    token: &CancellationToken,
) -> Option<u64> {
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=2160x3840:r=30",
        "-frames:v",
        "180",
        "-an",
        "-c:v",
        name,
    ]);
    command.args(quality_args);
    command.args(["-f", "null", "-"]);
    timed_probe(command, token).await
}

async fn benchmark_vaapi(token: &CancellationToken) -> Option<(String, u64)> {
    let Ok(entries) = std::fs::read_dir("/dev/dri") else {
        return None;
    };
    let mut devices = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
        .collect::<Vec<_>>();
    devices.sort();

    let mut best: Option<(String, u64)> = None;
    for device in devices {
        let Some(device_text) = device.to_str() else {
            continue;
        };
        let mut command = Command::new("ffmpeg");
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-vaapi_device",
            device_text,
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=2160x3840:r=30",
            "-vf",
            "format=nv12,hwupload",
            "-frames:v",
            "180",
            "-an",
            "-c:v",
            "h264_vaapi",
            "-qp",
            "23",
            "-f",
            "null",
            "-",
        ]);
        if let Some(ms) = timed_probe(command, token).await
            && best.as_ref().is_none_or(|(_, current)| ms < *current)
        {
            best = Some((device_text.to_owned(), ms));
        }
    }
    best
}

async fn benchmark_vulkan(token: &CancellationToken) -> Option<(String, u64)> {
    let mut best: Option<(String, u64)> = None;
    for index in 0..4 {
        let selector = index.to_string();
        let init = format!("vulkan=vk:{selector}");
        let mut command = Command::new("ffmpeg");
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-init_hw_device",
            &init,
            "-filter_hw_device",
            "vk",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=2160x3840:r=30",
            "-vf",
            "format=nv12,hwupload",
            "-frames:v",
            "180",
            "-an",
            "-c:v",
            "h264_vulkan",
            "-qp",
            "23",
            "-usage",
            "transcode",
            "-f",
            "null",
            "-",
        ]);
        if let Some(ms) = timed_probe(command, token).await
            && best.as_ref().is_none_or(|(_, current)| ms < *current)
        {
            best = Some((selector, ms));
        }
    }
    best
}

fn encoder_stability_priority(encoder: &EncoderKind) -> u8 {
    match encoder {
        EncoderKind::NvencH264 => 0,
        EncoderKind::QsvH264 => 1,
        EncoderKind::VaapiH264 => 2,
        EncoderKind::VulkanH264 => 3,
        EncoderKind::AmfH264 => 4,
        _ => 10,
    }
}

pub async fn detect_encoder_capabilities(token: &CancellationToken) -> EncoderCapabilities {
    let mut enc = Command::new("ffmpeg");
    enc.args(["-hide_banner", "-encoders"]);
    let encoder_result = crate::media::process::run_capture(enc, token).await;
    let ffmpeg = encoder_result.is_ok();
    let encoder_text = encoder_result
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    let nvenc_ms = if encoder_is_listed(&encoder_text, "h264_nvenc") {
        benchmark_software_encoder("h264_nvenc", &["-cq", "23"], token).await
    } else {
        None
    };
    let qsv_ms = if encoder_is_listed(&encoder_text, "h264_qsv") {
        benchmark_software_encoder("h264_qsv", &["-global_quality", "23"], token).await
    } else {
        None
    };
    let amf_ms = if encoder_is_listed(&encoder_text, "h264_amf") {
        benchmark_software_encoder("h264_amf", &["-qp_i", "23", "-qp_p", "23"], token).await
    } else {
        None
    };
    let vaapi = if encoder_is_listed(&encoder_text, "h264_vaapi") {
        benchmark_vaapi(token).await
    } else {
        None
    };
    let vulkan = if encoder_is_listed(&encoder_text, "h264_vulkan") {
        benchmark_vulkan(token).await
    } else {
        None
    };
    let hevc_nvenc = encoder_is_listed(&encoder_text, "hevc_nvenc")
        && probe_quick_encoder("hevc_nvenc", token).await;

    let mut ranked = Vec::new();
    let mut h264_benchmarks_ms = BTreeMap::new();
    for (name, encoder, score) in [
        ("nvenc_h264", EncoderKind::NvencH264, nvenc_ms),
        ("qsv_h264", EncoderKind::QsvH264, qsv_ms),
        (
            "vaapi_h264",
            EncoderKind::VaapiH264,
            vaapi.as_ref().map(|(_, ms)| *ms),
        ),
        (
            "vulkan_h264",
            EncoderKind::VulkanH264,
            vulkan.as_ref().map(|(_, ms)| *ms),
        ),
        ("amf_h264", EncoderKind::AmfH264, amf_ms),
    ] {
        if let Some(ms) = score {
            h264_benchmarks_ms.insert(name.to_owned(), ms);
            ranked.push((ms, encoder));
        }
    }
    ranked.sort_by(|(a_ms, a_encoder), (b_ms, b_encoder)| {
        let fastest = (*a_ms).min(*b_ms).max(1);
        let delta = a_ms.abs_diff(*b_ms);
        if delta.saturating_mul(100) <= fastest.saturating_mul(5) {
            encoder_stability_priority(a_encoder).cmp(&encoder_stability_priority(b_encoder))
        } else {
            a_ms.cmp(b_ms)
        }
    });
    let auto_encoder_order = ranked.into_iter().map(|(_, encoder)| encoder).collect();

    let mut filters = Command::new("ffmpeg");
    filters.args(["-hide_banner", "-filters"]);
    let filter_text = crate::media::process::run_capture(filters, token)
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();

    EncoderCapabilities {
        ffmpeg,
        h264_nvenc: nvenc_ms.is_some(),
        hevc_nvenc,
        h264_qsv: qsv_ms.is_some(),
        h264_vaapi: vaapi.is_some(),
        h264_vulkan: vulkan.is_some(),
        h264_amf: amf_ms.is_some(),
        vaapi_device: vaapi.map(|(device, _)| device),
        vulkan_device: vulkan.map(|(device, _)| device),
        h264_benchmarks_ms,
        auto_encoder_order,
        libass: filter_text
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some("ass")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FitMode, FormatKey, FormatProfile, Preset};

    fn source() -> MediaProbe {
        MediaProbe {
            duration: 10.0,
            width: 1920,
            height: 1080,
            fps: 25.0,
            video_codec: "h264".into(),
            audio_codec: Some("aac".into()),
            has_audio: true,
            format_name: Some("mp4".into()),
        }
    }

    #[test]
    fn source_preserve_does_not_crop_or_scale() {
        let preset = Preset::default();
        let plan = build_render_plan(
            Path::new("in.mkv"),
            Path::new("out.mkv"),
            Path::new("sub.ass"),
            &preset,
            &Encoder::default(),
            &EncoderCapabilities::default(),
            &source(),
            None,
            None,
        )
        .unwrap();
        let joined = plan.args.join(" ");
        assert!(!joined.contains("crop="));
        assert!(!joined.contains("scale="));
        assert!(joined.contains("ass='sub.ass'"));
        assert_eq!(plan.target_resolution, (1920, 1080));
    }

    #[test]
    fn portrait_cover_builds_scale_and_crop() {
        let preset = Preset {
            format: FormatProfile {
                key: FormatKey::Portrait916,
                fit: FitMode::Cover,
                width: None,
                height: None,
            },
            ..Preset::default()
        };
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &preset,
            &Encoder::default(),
            &EncoderCapabilities::default(),
            &source(),
            None,
            None,
        )
        .unwrap();
        let joined = plan.args.join(" ");
        assert!(
            joined.contains("scale=594:1056:force_original_aspect_ratio=increase,crop=594:1056")
        );
        assert_eq!(plan.target_resolution, (594, 1056));
    }

    #[test]
    fn matching_portrait_ratio_keeps_4k_source_resolution() {
        let mut src = source();
        src.width = 2160;
        src.height = 3840;
        let preset = Preset {
            format: FormatProfile {
                key: FormatKey::Portrait916,
                fit: FitMode::Cover,
                width: None,
                height: None,
            },
            ..Preset::default()
        };
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &preset,
            &Encoder::default(),
            &EncoderCapabilities::default(),
            &src,
            None,
            None,
        )
        .unwrap();
        assert_eq!(plan.target_resolution, (2160, 3840));
        let joined = plan.args.join(" ");
        assert!(!joined.contains("scale="));
        assert!(!joined.contains("crop="));
    }

    #[test]
    fn encoder_listing_matches_whole_fields_only() {
        let listing = " V....D h264_nvenc NVIDIA NVENC H.264 encoder\n V..... h264_qsv H.264 QSV";
        assert!(encoder_is_listed(listing, "h264_nvenc"));
        assert!(encoder_is_listed(listing, "h264_qsv"));
        assert!(!encoder_is_listed(listing, "nvenc"));
    }

    #[test]
    fn nvenc_uses_cq_not_x264_crf() {
        let settings = Encoder {
            kind: EncoderKind::NvencH264,
            quality: 19,
            preset: "medium".into(),
        };
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &Preset::default(),
            &settings,
            &EncoderCapabilities {
                h264_nvenc: true,
                ..Default::default()
            },
            &source(),
            None,
            None,
        )
        .unwrap();
        let joined = plan.args.join(" ");
        assert!(joined.contains("h264_nvenc -cq 19"));
        assert!(!joined.contains(" -crf "));
    }

    #[test]
    fn vaapi_plan_uses_detected_device_and_hwupload() {
        let settings = Encoder {
            kind: EncoderKind::VaapiH264,
            quality: 20,
            preset: "medium".into(),
        };
        let caps = EncoderCapabilities {
            h264_vaapi: true,
            vaapi_device: Some("/dev/dri/renderD128".into()),
            ..Default::default()
        };
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &Preset::default(),
            &settings,
            &caps,
            &source(),
            None,
            None,
        )
        .unwrap();
        let joined = plan.args.join(" ");
        assert!(joined.contains("-vaapi_device /dev/dri/renderD128"));
        assert!(joined.contains("format=nv12,hwupload"));
        assert!(joined.contains("-c:v h264_vaapi"));
        assert!(!joined.contains("-pix_fmt yuv420p"));
    }

    #[test]
    fn vulkan_plan_uses_detected_device_and_hwupload() {
        let settings = Encoder {
            kind: EncoderKind::VulkanH264,
            quality: 18,
            preset: "medium".into(),
        };
        let caps = EncoderCapabilities {
            h264_vulkan: true,
            vulkan_device: Some("0".into()),
            ..Default::default()
        };
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &Preset::default(),
            &settings,
            &caps,
            &source(),
            None,
            None,
        )
        .unwrap();
        let joined = plan.args.join(" ");
        assert!(joined.contains("-init_hw_device vulkan=vk:0 -filter_hw_device vk"));
        assert!(joined.contains("format=nv12,hwupload"));
        assert!(joined.contains("-c:v h264_vulkan -qp 18 -usage transcode"));
        assert!(!joined.contains("-pix_fmt yuv420p"));
    }

    #[test]
    fn auto_uses_runtime_benchmark_order() {
        let caps = EncoderCapabilities {
            h264_vaapi: true,
            h264_vulkan: true,
            vaapi_device: Some("/dev/dri/renderD128".into()),
            vulkan_device: Some("0".into()),
            auto_encoder_order: vec![EncoderKind::VulkanH264, EncoderKind::VaapiH264],
            ..Default::default()
        };
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &Preset::default(),
            &Encoder::default(),
            &caps,
            &source(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(plan.encoder, EncoderKind::VulkanH264);
        assert_eq!(
            auto_encoder_order(&caps),
            vec![EncoderKind::VulkanH264, EncoderKind::VaapiH264]
        );
    }

    #[test]
    fn explicit_profile_rejects_preserve_fit() {
        let preset = Preset {
            format: FormatProfile {
                key: FormatKey::Square11,
                fit: FitMode::Preserve,
                width: None,
                height: None,
            },
            ..Preset::default()
        };
        assert!(
            build_render_plan(
                Path::new("in.mp4"),
                Path::new("out.mp4"),
                Path::new("sub.ass"),
                &preset,
                &Encoder::default(),
                &EncoderCapabilities::default(),
                &source(),
                None,
                None
            )
            .is_err()
        );
    }

    #[test]
    fn outro_plan_synthesizes_missing_audio() {
        let mut outro = source();
        outro.has_audio = false;
        outro.audio_codec = None;
        outro.duration = 2.0;
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &Preset::default(),
            &Encoder::default(),
            &EncoderCapabilities::default(),
            &source(),
            Some((Path::new("outro.mp4"), &outro)),
            None,
        )
        .unwrap();
        assert!(
            plan.args
                .join(" ")
                .contains("anullsrc=r=48000:cl=stereo:d=2.000000[outa]")
        );
    }

    #[test]
    fn source_preserve_never_geometrically_transforms_primary_video() {
        for (width, height) in [(1920, 1080), (1080, 1920), (1080, 1080), (1237, 517)] {
            let mut source = source();
            source.width = width;
            source.height = height;
            let plan = build_render_plan(
                Path::new("in.mp4"),
                Path::new("out.mp4"),
                Path::new("sub.ass"),
                &Preset::default(),
                &Encoder::default(),
                &EncoderCapabilities::default(),
                &source,
                None,
                None,
            )
            .unwrap();
            let joined = plan.args.join(" ");
            assert!(!joined.contains("scale="), "{width}x{height}: {joined}");
            assert!(!joined.contains("pad="), "{width}x{height}: {joined}");
            assert!(!joined.contains("crop="), "{width}x{height}: {joined}");
            assert_eq!(plan.target_resolution, (width, height));
        }
    }

    #[test]
    fn source_preserve_outro_adapts_only_outro_geometry() {
        let mut main = source();
        main.width = 1237;
        main.height = 517;
        let plan = build_render_plan(
            Path::new("in.mp4"),
            Path::new("out.mp4"),
            Path::new("sub.ass"),
            &Preset::default(),
            &Encoder::default(),
            &EncoderCapabilities::default(),
            &main,
            Some((Path::new("outro.mp4"), &source())),
            None,
        )
        .unwrap();
        let filter = plan.args.iter().find(|arg| arg.contains("[0:v]")).unwrap();
        assert!(filter.contains("[0:v]ass='sub.ass'[mainv]"));
    }
}
