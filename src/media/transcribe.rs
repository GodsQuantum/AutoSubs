use crate::domain::{RawSegment, RawWord, Settings, TranscriptionResponse};
use crate::media::process::{ProcessError, run_capture};
use futures_util::TryStreamExt;
use reqwest::{Client, multipart};
use serde_json::Value;
use std::path::Path;
use thiserror::Error;
use tokio::{fs::File, process::Command};
use tokio_util::{io::ReaderStream, sync::CancellationToken};

#[derive(Debug, Error)]
pub enum TranscriptionError {
    #[error("operation cancelled")]
    Cancelled,
    #[error("no transcription endpoint configured")]
    NotConfigured,
    #[error("transcription request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("transcription response is invalid: {0}")]
    Invalid(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Process(#[from] ProcessError),
}

pub async fn extract_audio(
    input: &Path,
    output: &Path,
    token: &CancellationToken,
) -> Result<(), ProcessError> {
    let mut command = Command::new("ffmpeg");
    command
        .args(["-y", "-v", "error", "-i"])
        .arg(input)
        .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
        .arg(output);
    run_capture(command, token).await.map(|_| ())
}

pub(crate) fn endpoint_base(url: &str) -> String {
    let mut endpoint = url.trim().trim_end_matches('/').to_owned();
    for suffix in ["/audio/transcriptions", "/models"] {
        if endpoint.ends_with(suffix) {
            endpoint.truncate(endpoint.len() - suffix.len());
            break;
        }
    }
    endpoint
}

pub(crate) fn transcription_endpoint(url: &str) -> String {
    let endpoint = url.trim().trim_end_matches('/');
    if endpoint.ends_with("/audio/transcriptions") {
        endpoint.to_owned()
    } else {
        format!("{}/audio/transcriptions", endpoint_base(endpoint))
    }
}

pub(crate) fn models_endpoint(url: &str) -> String {
    format!("{}/models", endpoint_base(url))
}

async fn request_endpoint(
    url: &str,
    api_key: &str,
    model: &str,
    language: &str,
    audio: &Path,
    client: &Client,
    token: &CancellationToken,
) -> Result<TranscriptionResponse, TranscriptionError> {
    if url.trim().is_empty() {
        return Err(TranscriptionError::NotConfigured);
    }
    let url = transcription_endpoint(url);
    let file = File::open(audio).await?;
    let stream = ReaderStream::new(file).map_err(std::io::Error::other);
    let body = reqwest::Body::wrap_stream(stream);
    let part = multipart::Part::stream(body)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(TranscriptionError::Http)?;
    let mut form = multipart::Form::new()
        .part("file", part)
        .text("response_format", "verbose_json")
        .text("timestamp_granularities[]", "word");
    if !model.trim().is_empty() {
        form = form.text("model", model.to_owned());
    }
    if !language.trim().is_empty() {
        form = form.text("language", language.to_owned());
    }
    let mut request = client.post(url).multipart(form);
    if !api_key.trim().is_empty() {
        request = request.bearer_auth(api_key);
    }
    let response = tokio::select! { r = request.send() => r?, _ = token.cancelled() => return Err(TranscriptionError::Cancelled) };
    let status = response.status();
    let value: Value = tokio::select! { r = response.json() => r?, _ = token.cancelled() => return Err(TranscriptionError::Cancelled) };
    if !status.is_success() {
        return Err(TranscriptionError::Invalid(format!(
            "HTTP {status}: {value}"
        )));
    }
    parse_transcription(value)
}

fn parse_word(value: &Value) -> Option<RawWord> {
    Some(RawWord {
        word: value
            .get("word")
            .or_else(|| value.get("text"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        start: value.get("start").and_then(Value::as_f64),
        end: value.get("end").and_then(Value::as_f64),
    })
}

pub fn parse_transcription(value: Value) -> Result<TranscriptionResponse, TranscriptionError> {
    if let Ok(response) = serde_json::from_value::<TranscriptionResponse>(value.clone())
        && (response.words.as_ref().is_some_and(|v| !v.is_empty())
            || response.segments.as_ref().is_some_and(|v| !v.is_empty())
            || response.text.is_some())
    {
        return Ok(response);
    }
    // WhisperX-style nested result/chunks compatibility.
    let root = value.get("result").unwrap_or(&value);
    let words = root
        .get("word_segments")
        .or_else(|| root.get("words"))
        .or_else(|| root.get("chunks"))
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(parse_word).collect::<Vec<_>>());
    let segments = root
        .get("segments")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .map(|segment| RawSegment {
                    start: segment.get("start").and_then(Value::as_f64),
                    end: segment.get("end").and_then(Value::as_f64),
                    text: segment
                        .get("text")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    words: segment
                        .get("words")
                        .and_then(Value::as_array)
                        .map(|v| v.iter().filter_map(parse_word).collect()),
                })
                .collect()
        });
    let text = root.get("text").and_then(Value::as_str).map(str::to_owned);
    if words.as_ref().is_some_and(|v| !v.is_empty())
        || segments
            .as_ref()
            .is_some_and(|v: &Vec<RawSegment>| !v.is_empty())
        || text.is_some()
    {
        Ok(TranscriptionResponse {
            text,
            words,
            segments,
        })
    } else {
        Err(TranscriptionError::Invalid(
            "no text, words or segments in response".into(),
        ))
    }
}

pub async fn transcribe_audio(
    audio: &Path,
    settings: &Settings,
    client: &Client,
    token: &CancellationToken,
) -> Result<TranscriptionResponse, TranscriptionError> {
    let primary = || {
        request_endpoint(
            &settings.transcription_url,
            &settings.transcription_api_key,
            &settings.transcription_model,
            &settings.language,
            audio,
            client,
            token,
        )
    };
    let local = || {
        request_endpoint(
            &settings.local_transcription_url,
            &settings.local_transcription_api_key,
            &settings.local_transcription_model,
            &settings.language,
            audio,
            client,
            token,
        )
    };
    if settings.local_transcription_enabled {
        match local().await {
            Ok(v) => return Ok(v),
            Err(TranscriptionError::Cancelled) => return Err(TranscriptionError::Cancelled),
            Err(e) if !settings.local_fallback_enabled => return Err(e),
            Err(_) => {}
        }
    }
    if !settings.transcription_url.trim().is_empty() {
        return primary().await;
    }
    if !settings.local_transcription_url.trim().is_empty() {
        return local().await;
    }
    Err(TranscriptionError::NotConfigured)
}

#[cfg(test)]
mod endpoint_regression_tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use serde_json::json;

    #[tokio::test]
    async fn base_v1_endpoint_transcribes_via_audio_transcriptions() {
        let app = Router::new().route(
            "/v1/audio/transcriptions",
            post(|| async { Json(json!({"text": "ok"})) }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("sample.wav");
        tokio::fs::write(&audio, b"RIFFtest").await.unwrap();
        let settings = Settings {
            local_transcription_enabled: false,
            transcription_url: format!("http://{addr}/v1"),
            transcription_model: "demo".into(),
            language: "fr".into(),
            ..Settings::default()
        };

        let result = transcribe_audio(&audio, &settings, &Client::new(), &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(result.text.as_deref(), Some("ok"));
    }
}
