use crate::{
    domain::{Preset, SubtitleLine, SubtitleWord},
    error::{AppError, AppResult},
    jobs,
    media::{probe_media, process::run_capture, render::visual_filter_chain},
    state::AppState,
    subtitle::ass::generate_ass_content,
};
use axum::{
    Json,
    body::Body,
    extract::State,
    http::{Response, header},
};
use serde::Deserialize;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFrameRequest {
    pub preset: Preset,
    pub text: String,
    #[serde(default)]
    pub words: Option<Vec<SubtitleWord>>,
    #[serde(default)]
    pub timestamp: f64,
    #[serde(default)]
    pub job_id: Option<String>,
}

pub async fn frame(
    State(state): State<AppState>,
    Json(mut body): Json<PreviewFrameRequest>,
) -> AppResult<Response<Body>> {
    let _slot = state
        .preview_slots
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::Conflict("preview worker pool is unavailable".into()))?;

    body.preset.migrate();
    let token = CancellationToken::new();
    let timestamp = body.timestamp.max(0.0);
    let (input, source) = if let Some(id) = body.job_id.as_deref().filter(|id| !id.is_empty()) {
        let job =
            jobs::get_job(&state, id).map_err(|_| AppError::NotFound("job not found".into()))?;
        let input = job
            .input_path
            .filter(|path| path.is_file())
            .ok_or_else(|| AppError::NotFound("job source video is unavailable".into()))?;
        let probe = probe_media(&input, &token)
            .await
            .map_err(|error| AppError::Conflict(format!("preview probe failed: {error}")))?;
        (Some(input), probe)
    } else {
        let (width, height) = body
            .preset
            .format
            .resolution(Some((1920, 1080)))
            .unwrap_or((1920, 1080));
        (
            None,
            crate::media::probe::MediaProbe {
                duration: 4.0,
                width,
                height,
                fps: 30.0,
                video_codec: "rawvideo".into(),
                audio_codec: None,
                has_audio: false,
                format_name: Some("lavfi".into()),
            },
        )
    };

    let words = body.words.take().unwrap_or_default();
    let line_start = words
        .first()
        .map(|word| word.start)
        .unwrap_or(0.0)
        .min(timestamp);
    let line_end = words
        .last()
        .map(|word| word.end)
        .unwrap_or_else(|| timestamp.max(2.0) + 1.0)
        .max(timestamp + 0.05);
    let line = SubtitleLine {
        id: 0,
        start: line_start.max(0.0),
        end: line_end,
        text: body.text,
        words: (!words.is_empty()).then_some(words),
    };

    let ass_path = state
        .config
        .work_dir()
        .join(format!("preview-{}.ass", Uuid::new_v4()));
    let ass = generate_ass_content(&[line], &body.preset, Some((source.width, source.height)));
    tokio::fs::write(&ass_path, ass)
        .await
        .map_err(AppError::from)?;

    let chain = match visual_filter_chain(
        &body.preset,
        &source,
        &ass_path,
        Some(&state.config.fonts_dir),
        false,
    ) {
        Ok(chain) => chain,
        Err(error) => {
            let _ = tokio::fs::remove_file(&ass_path).await;
            return Err(AppError::Conflict(format!(
                "preview filter construction failed: {error}"
            )));
        }
    };

    let mut command = Command::new("ffmpeg");
    command.args(["-v", "error"]);
    if let Some(input) = input {
        command.arg("-i").arg(input);
    } else {
        command.args(["-f", "lavfi", "-i"]).arg(format!(
            "color=c=0x182329:s={}x{}:r=30:d=4",
            source.width, source.height
        ));
    }
    command
        .args(["-ss", &format!("{timestamp:.6}"), "-vf"])
        .arg(chain)
        .args([
            "-frames:v",
            "1",
            "-an",
            "-f",
            "image2pipe",
            "-vcodec",
            "png",
            "pipe:1",
        ]);

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(12),
        run_capture(command, &token),
    )
    .await;
    let _ = tokio::fs::remove_file(&ass_path).await;
    let output = match result {
        Ok(Ok(output)) if !output.stdout.is_empty() => output,
        Ok(Ok(_)) => {
            return Err(AppError::Conflict(
                "preview renderer returned no frame".into(),
            ));
        }
        Ok(Err(error)) => {
            return Err(AppError::Conflict(format!(
                "authoritative preview render failed: {error}"
            )));
        }
        Err(_) => {
            token.cancel();
            return Err(AppError::Conflict("authoritative preview timed out".into()));
        }
    };

    Response::builder()
        .header(header::CONTENT_TYPE, "image/png")
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(output.stdout))
        .map_err(|error| AppError::Internal(error.into()))
}
