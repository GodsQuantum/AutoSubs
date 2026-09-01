use crate::domain::{Encoder, EncoderKind, FitMode, FormatKey, Preset};
use crate::media::probe::MediaProbe;
use crate::media::process::ProcessError;
use std::path::Path;
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
    pub h264_amf: bool,
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

fn resolved_encoder(settings: &Encoder, caps: &EncoderCapabilities) -> EncoderKind {
    match settings.kind {
        EncoderKind::Auto if caps.h264_nvenc => EncoderKind::NvencH264,
        EncoderKind::Auto if caps.h264_qsv => EncoderKind::QsvH264,
        EncoderKind::Auto if caps.h264_vaapi => EncoderKind::VaapiH264,
        EncoderKind::Auto if caps.h264_amf => EncoderKind::AmfH264,
        EncoderKind::Auto => EncoderKind::Libx264,
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
    let main_video = chain([geometry_chain(preset, source)?, ass_filter]);
    let mut args = vec![
        "-y".into(),
        "-i".into(),
        input.to_string_lossy().into_owned(),
    ];

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
            args.extend([
                "-movflags".into(),
                "+faststart".into(),
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
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
            let complex = format!(
                "[0:v]{main_video}[mainv];[1:v]{outro_video}[outv];{main_audio};{outro_audio};[mainv][maina][outv][outa]concat=n=2:v=1:a=1[vout][aout]"
            );
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
                "-pix_fmt".into(),
                "yuv420p".into(),
            ]);
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

pub async fn detect_encoder_capabilities(token: &CancellationToken) -> EncoderCapabilities {
    let mut enc = Command::new("ffmpeg");
    enc.args(["-hide_banner", "-encoders"]);
    let encoder_result = crate::media::process::run_capture(enc, token).await;
    let ffmpeg = encoder_result.is_ok();
    let encoder_text = encoder_result
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    let mut filters = Command::new("ffmpeg");
    filters.args(["-hide_banner", "-filters"]);
    let filter_text = crate::media::process::run_capture(filters, token)
        .await
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    EncoderCapabilities {
        ffmpeg,
        h264_nvenc: encoder_text.contains("h264_nvenc"),
        hevc_nvenc: encoder_text.contains("hevc_nvenc"),
        h264_qsv: encoder_text.contains("h264_qsv"),
        h264_vaapi: encoder_text.contains("h264_vaapi"),
        h264_amf: encoder_text.contains("h264_amf"),
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
            joined.contains("scale=1080:1920:force_original_aspect_ratio=increase,crop=1080:1920")
        );
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
