use crate::domain::{Settings, TranscriptionResponse};
use crate::media::process::{run_capture, ProcessError};
use futures_util::TryStreamExt;
use reqwest::multipart::{Form, Part};
use std::path::Path;
use tokio::process::Command;
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum TranscriptionError {
    #[error("cancelled")]
    Cancelled,
    #[error("audio extraction failed: {0}")]
    Audio(#[from] ProcessError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("transcription API returned {status}: {body}")]
    Api { status: reqwest::StatusCode, body: String },
}

pub async fn extract_audio(input: &Path, output: &Path, token: &CancellationToken) -> Result<(), ProcessError> {
    let mut command = Command::new("ffmpeg");
    command.args(["-y", "-i"]).arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-acodec", "pcm_s16le"])
        .arg(output);
    run_capture(command, token).await.map(|_| ())
}

async fn streaming_audio_part(path: &Path) -> Result<Part, std::io::Error> {
    let file = tokio::fs::File::open(path).await?;
    let stream = ReaderStream::new(file).map_err(std::io::Error::other);
    let body = reqwest::Body::wrap_stream(stream);
    Ok(Part::stream(body).file_name("audio.wav").mime_str("audio/wav").expect("static mime is valid"))
}

async fn do_transcribe(
    audio_path: &Path,
    url: &str,
    api_key: &str,
    model: &str,
    language: &str,
    client: &reqwest::Client,
    token: &CancellationToken,
) -> Result<TranscriptionResponse, TranscriptionError> {
    let part = streaming_audio_part(audio_path).await?;
    let is_whisperx = url.contains("/asr");
    let mut request = client.post(url);
    if !api_key.trim().is_empty() { request = request.bearer_auth(api_key); }

    let form = if is_whisperx {
        request = request.query(&[
            ("task", "transcribe"),
            ("language", language),
            ("output", "json"),
            ("word_timestamps", "true"),
        ]);
        Form::new().part("audio_file", part)
    } else {
        Form::new().part("file", part)
            .text("model", model.to_string())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "word")
            .text("language", language.to_string())
    };

    let response = tokio::select! {
        response = request.multipart(form).send() => response?,
        _ = token.cancelled() => return Err(TranscriptionError::Cancelled),
    };
    let status = response.status();
    if !status.is_success() {
        let body = tokio::select! {
            body = response.text() => body.unwrap_or_default(),
            _ = token.cancelled() => return Err(TranscriptionError::Cancelled),
        };
        return Err(TranscriptionError::Api { status, body });
    }
    tokio::select! {
        parsed = response.json::<TranscriptionResponse>() => Ok(parsed?),
        _ = token.cancelled() => Err(TranscriptionError::Cancelled),
    }
}

pub async fn transcribe_audio(
    audio_path: &Path,
    settings: &Settings,
    client: &reqwest::Client,
    token: &CancellationToken,
) -> Result<TranscriptionResponse, TranscriptionError> {
    if settings.local_transcription_enabled && !settings.local_transcription_url.trim().is_empty() {
        match do_transcribe(
            audio_path,
            &settings.local_transcription_url,
            &settings.local_transcription_api_key,
            &settings.local_transcription_model,
            &settings.language,
            client,
            token,
        ).await {
            Ok(value) => return Ok(value),
            Err(TranscriptionError::Cancelled) => return Err(TranscriptionError::Cancelled),
            Err(error) if settings.local_fallback_enabled => {
                tracing::warn!(%error, "local transcription failed; using primary endpoint");
            }
            Err(error) => return Err(error),
        }
    }

    do_transcribe(
        audio_path,
        &settings.transcription_url,
        &settings.transcription_api_key,
        &settings.transcription_model,
        &settings.language,
        client,
        token,
    ).await
}
