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
    // Auto-language requests must not ask Speaches to force-align inside ASR:
    // the GGUF backend rejects word timestamps without a language (HTTP 503).
    // Autosubs always runs its own strict acoustic word aligner afterwards,
    // using a separately autodetected spoken-language code.
    let mut form = multipart::Form::new()
        .part("file", part)
        .text("response_format", "verbose_json");
    if !language.trim().is_empty() {
        form = form.text("timestamp_granularities[]", "word");
    }
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

async fn transcribe_once(
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
            Ok(v) => {
                tracing::info!(
                    model = %settings.local_transcription_model,
                    "local ASR transcription completed"
                );
                return Ok(v);
            }
            Err(TranscriptionError::Cancelled) => return Err(TranscriptionError::Cancelled),
            Err(e) if !settings.local_fallback_enabled => return Err(e),
            Err(_) => {
                tracing::warn!(
                    model = %settings.local_transcription_model,
                    "local ASR failed; using configured fallback"
                );
            }
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


// Long audio on some ASR backends returns one text blob with no timestamped
// segments, which the strict CTC aligner must reject (HTTP 422). Transcribe
// bounded, real audio excerpts and preserve their absolute segment boundaries.
// Do not guess per-word timestamps: the native acoustic aligner still owns them.
const MAX_ASR_CHUNK_SECS: f64 = 24.0;
const CHUNKING_THRESHOLD_SECS: f64 = 45.0;

fn combine_transcription_chunks(
    parts: Vec<(f64, f64, TranscriptionResponse)>,
) -> Result<TranscriptionResponse, TranscriptionError> {
    let mut text_parts = Vec::new();
    let mut segments = Vec::new();
    let mut words = Vec::new();
    let mut all_have_words = true;
    for (offset, length, mut result) in parts {
        let partial_text = result.text.take().filter(|text| !text.trim().is_empty())
            .or_else(|| result.segments.as_ref().map(|rows| rows.iter()
                .filter_map(|row| row.text.as_deref())
                .collect::<Vec<_>>().join(" ")));
        let text = partial_text.filter(|text| !text.trim().is_empty())
            .ok_or_else(|| TranscriptionError::Invalid(format!(
                "chunk at {offset:.1}s produced no words"
            )))?;
        text_parts.push(text.clone());
        let mut emitted = false;
        for segment in result.segments.take().unwrap_or_default() {
            let (Some(start), Some(end), Some(content)) =
                (segment.start, segment.end, segment.text) else { continue };
            if start < 0.0 || end <= start || end > length + 1.0 || content.trim().is_empty() {
                continue;
            }
            emitted = true;
            segments.push(RawSegment {
                start: Some(offset + start),
                end: Some(offset + end),
                text: Some(content),
                words: None,
            });
        }
        if !emitted {
            // This is the exact slice boundary supplied to ffmpeg. The
            // acoustic aligner computes the real word timings inside it.
            segments.push(RawSegment {
                start: Some(offset),
                end: Some(offset + length),
                text: Some(text),
                words: None,
            });
        }
        if let Some(local_words) = result.words.take().filter(|rows| !rows.is_empty()) {
            for mut word in local_words {
                word.start = word.start.map(|value| value + offset);
                word.end = word.end.map(|value| value + offset);
                words.push(word);
            }
        } else {
            all_have_words = false;
        }
    }
    if text_parts.is_empty() || segments.is_empty() {
        return Err(TranscriptionError::Invalid("no chunk transcripts".into()));
    }
    Ok(TranscriptionResponse {
        text: Some(text_parts.join(" ")),
        words: if all_have_words { Some(words) } else { None },
        segments: Some(segments),
    })
}

pub async fn transcribe_audio(
    audio: &Path,
    settings: &Settings,
    client: &Client,
    token: &CancellationToken,
) -> Result<TranscriptionResponse, TranscriptionError> {
    if !settings.alignment_enabled || !settings.alignment_required {
        return transcribe_once(audio, settings, client, token).await;
    }
    let mut probe = Command::new("ffprobe");
    probe.args(["-v", "error", "-show_entries", "format=duration",
        "-of", "default=noprint_wrappers=1:nokey=1"]).arg(audio);
    let probe_output = tokio::select! {
        output = probe.output() => output?,
        _ = token.cancelled() => return Err(TranscriptionError::Cancelled),
    };
    let duration: f64 = String::from_utf8_lossy(&probe_output.stdout).trim().parse()
        .map_err(|_| TranscriptionError::Invalid("unable to determine audio duration".into()))?;
    if !duration.is_finite() || duration < 0.0 {
        return Err(TranscriptionError::Invalid("invalid audio duration".into()));
    }
    if duration <= CHUNKING_THRESHOLD_SECS {
        return transcribe_once(audio, settings, client, token).await;
    }
    let mut parts = Vec::new();
    let mut offset = 0.0;
    let mut index = 0usize;
    while offset < duration - 0.01 {
        if token.is_cancelled() { return Err(TranscriptionError::Cancelled); }
        let length = (duration - offset).min(MAX_ASR_CHUNK_SECS);
        let chunk = audio.with_file_name(format!(
            "{}-asr-{index:03}.wav",
            audio.file_stem().and_then(|name| name.to_str()).unwrap_or("audio")
        ));
        let mut command = Command::new("ffmpeg");
        command.args(["-y", "-v", "error", "-ss", &format!("{offset:.3}"),
            "-t", &format!("{length:.3}"), "-i"]).arg(audio)
            .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"]).arg(&chunk);
        let transcript = async {
            run_capture(command, token).await?;
            transcribe_once(&chunk, settings, client, token).await
        }.await;
        let _ = tokio::fs::remove_file(&chunk).await;
        parts.push((offset, length, transcript?));
        offset += length;
        index += 1;
    }
    combine_transcription_chunks(parts)
}

#[cfg(test)]
mod endpoint_regression_tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use serde_json::json;


    #[test]
    fn long_audio_chunks_preserve_exact_boundaries_without_inventing_word_times() {
        let merged = combine_transcription_chunks(vec![
            (0.0, 24.0, TranscriptionResponse {
                text: Some("Teddy Riner".into()), words: None, segments: None,
            }),
            (24.0, 24.0, TranscriptionResponse {
                text: Some("est un judoka".into()), words: None,
                segments: Some(vec![RawSegment {
                    start: Some(2.0), end: Some(18.0),
                    text: Some("est un judoka".into()), words: None,
                }]),
            }),
        ]).unwrap();
        assert_eq!(merged.text.as_deref(), Some("Teddy Riner est un judoka"));
        assert!(merged.words.is_none());
        let segments = merged.segments.unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].start, segments[0].end), (Some(0.0), Some(24.0)));
        assert_eq!((segments[1].start, segments[1].end), (Some(26.0), Some(42.0)));
    }

    #[tokio::test]
    async fn base_v1_endpoint_transcribes_via_audio_transcriptions() {
        let app = Router::new().route(
            "/v1/audio/transcriptions",
            post(|_body: axum::body::Bytes| async { Json(json!({"text": "ok"})) }),
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
