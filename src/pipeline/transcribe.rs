use anyhow::{Context, Result};
use reqwest::multipart;
use crate::subtitle::types::{Settings, TranscriptionResponse};

pub async fn transcribe_audio(
    wav_path: &str,
    settings: &Settings,
    http_client: &reqwest::Client,
) -> Result<TranscriptionResponse> {
    if settings.local_transcription_enabled {
        tracing::info!("Attempting local transcription via {}", settings.local_transcription_url);
        match do_transcribe(wav_path, &settings.local_transcription_url, &settings.local_transcription_api_key, &settings.local_transcription_model, &settings.language, http_client).await {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                tracing::error!("Local transcription failed: {}", e);
                if settings.local_fallback_enabled {
                    tracing::info!("Falling back to main transcription URL: {}", settings.transcription_url);
                } else {
                    return Err(e);
                }
            }
        }
    }

    do_transcribe(wav_path, &settings.transcription_url, &settings.transcription_api_key, &settings.transcription_model, &settings.language, http_client).await
}

async fn do_transcribe(
    wav_path: &str,
    url: &str,
    api_key: &str,
    model: &str,
    language: &str,
    http_client: &reqwest::Client,
) -> Result<TranscriptionResponse> {
    let audio_bytes = tokio::fs::read(wav_path).await.context("Failed to read wav file")?;
    let part = multipart::Part::bytes(audio_bytes).file_name("audio.wav");

    let is_whisperx = url.contains("/asr");
    
    let mut req = http_client.post(url);
    if !api_key.is_empty() {
        req = req.bearer_auth(api_key);
    }

    let form = if is_whisperx {
        req = req.query(&[
            ("task", "transcribe"),
            ("language", language),
            ("output", "json"),
            ("word_timestamps", "true"),
        ]);
        multipart::Form::new().part("audio_file", part)
    } else {
        multipart::Form::new()
            .part("file", part)
            .text("model", model.to_string())
            .text("response_format", "verbose_json")
            .text("timestamp_granularities[]", "word")
            .text("language", language.to_string())
    };

    let response = req.multipart(form).send().await.context("Transcription API request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        anyhow::bail!("Transcription API error {}: {}", status, error_text);
    }

    let resp_data = response.json::<TranscriptionResponse>().await.context("Failed to parse transcription JSON")?;
    Ok(resp_data)
}
