use std::path::Path;

use anyhow::{Context, Result, bail};
use futures_util::TryStreamExt;
use reqwest::{Client, multipart};
use tokio::fs::File;
use tokio_util::{io::ReaderStream, sync::CancellationToken};

use crate::domain::{
    RawWord, Settings, SubtitleWord, TimingQuality, TranscriptTimeline, TranscriptionResponse,
};

#[derive(Debug, Clone, PartialEq)]
pub struct AlignmentOutcome {
    pub timeline: TranscriptTimeline,
    pub attempted: bool,
    pub fallback_reason: Option<String>,
}

impl AlignmentOutcome {
    fn native(timeline: &TranscriptTimeline, attempted: bool, reason: Option<String>) -> Self {
        Self {
            timeline: timeline.clone(),
            attempted,
            fallback_reason: reason,
        }
    }
}

fn lexical_key(value: &str) -> String {
    value
        .trim()
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'' && c != '’' && c != '-')
        .to_lowercase()
}

fn timeline_bounds(timeline: &TranscriptTimeline) -> Option<(f64, f64)> {
    let start = timeline.words.first()?.start;
    let end = timeline.words.last()?.end;
    (start.is_finite() && end.is_finite() && end > start).then_some((start, end))
}

pub fn parse_alignment_response(value: serde_json::Value) -> Result<Vec<SubtitleWord>> {
    let root = value.get("result").unwrap_or(&value);
    let items = root
        .get("word_segments")
        .or_else(|| root.get("words"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("alignment response has no word segments"))?;

    let mut out = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let word = item
            .get("word")
            .or_else(|| item.get("text"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("alignment word {index} has no text"))?;
        let start = item
            .get("start")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| anyhow::anyhow!("alignment word {index} has no start"))?;
        let end = item
            .get("end")
            .and_then(serde_json::Value::as_f64)
            .ok_or_else(|| anyhow::anyhow!("alignment word {index} has no end"))?;
        out.push(SubtitleWord {
            word: word.to_owned(),
            start,
            end,
        });
    }
    if out.is_empty() {
        bail!("alignment response contains no words");
    }
    Ok(out)
}

pub fn timeline_as_transcription(timeline: &TranscriptTimeline) -> TranscriptionResponse {
    TranscriptionResponse {
        text: None,
        words: Some(
            timeline
                .words
                .iter()
                .map(|word| RawWord {
                    word: Some(word.word.clone()),
                    start: Some(word.start),
                    end: Some(word.end),
                })
                .collect(),
        ),
        segments: None,
    }
}

pub fn validate_aligned_words(
    native: &TranscriptTimeline,
    aligned: Vec<SubtitleWord>,
) -> Result<TranscriptTimeline> {
    if native.words.len() != aligned.len() {
        bail!(
            "alignment word count mismatch: native={} aligned={}",
            native.words.len(),
            aligned.len()
        );
    }

    let mut previous_end = 0.0_f64;
    let mut out = Vec::with_capacity(native.words.len());
    for (index, (native_word, aligned_word)) in native.words.iter().zip(aligned).enumerate() {
        if lexical_key(&native_word.word) != lexical_key(&aligned_word.word) {
            bail!(
                "alignment lexical mismatch at word {}: {:?} vs {:?}",
                index,
                native_word.word,
                aligned_word.word
            );
        }
        if !aligned_word.start.is_finite()
            || !aligned_word.end.is_finite()
            || aligned_word.start < 0.0
            || aligned_word.end <= aligned_word.start
        {
            bail!("invalid alignment boundary at word {index}");
        }
        if index > 0 && aligned_word.start < previous_end {
            bail!("non-monotonic alignment at word {index}");
        }
        previous_end = aligned_word.end;
        out.push(SubtitleWord {
            word: native_word.word.clone(),
            start: aligned_word.start,
            end: aligned_word.end,
        });
    }

    Ok(TranscriptTimeline {
        words: out,
        timing_quality: TimingQuality::Aligned,
    })
}

async fn request_http_alignment(
    audio: &Path,
    native: &TranscriptTimeline,
    settings: &Settings,
    client: &Client,
    token: &CancellationToken,
) -> Result<Vec<SubtitleWord>> {
    let file = File::open(audio)
        .await
        .with_context(|| format!("open alignment audio {}", audio.display()))?;
    let stream = ReaderStream::new(file).map_err(std::io::Error::other);
    let part = multipart::Part::stream(reqwest::Body::wrap_stream(stream))
        .file_name("audio.wav")
        .mime_str("audio/wav")?;

    let transcript = native
        .words
        .iter()
        .map(|word| word.word.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    let mut form = multipart::Form::new()
        .part("file", part)
        .text("transcript", transcript)
        .text("language", settings.language.clone());
    if let Some((start, end)) = timeline_bounds(native) {
        form = form
            .text("start", format!("{start:.6}"))
            .text("end", format!("{end:.6}"));
    }
    if !settings.alignment_model.trim().is_empty() {
        form = form.text("model", settings.alignment_model.clone());
    }

    let mut request = client.post(settings.alignment_url.trim()).multipart(form);
    if !settings.alignment_api_key.trim().is_empty() {
        request = request.bearer_auth(&settings.alignment_api_key);
    }

    let response = tokio::select! {
        response = request.send() => response?,
        _ = token.cancelled() => bail!("alignment cancelled"),
    };
    let status = response.status();
    let value: serde_json::Value = tokio::select! {
        value = response.json() => value?,
        _ = token.cancelled() => bail!("alignment cancelled"),
    };
    if !status.is_success() {
        bail!("alignment HTTP {status}: {value}");
    }
    parse_alignment_response(value)
}

pub async fn align_timeline(
    audio: &Path,
    native: &TranscriptTimeline,
    settings: &Settings,
    client: &Client,
    token: &CancellationToken,
) -> AlignmentOutcome {
    if !settings.alignment_enabled
        || settings.alignment_url.trim().is_empty()
        || native.words.is_empty()
    {
        return AlignmentOutcome::native(native, false, None);
    }

    if token.is_cancelled() {
        return AlignmentOutcome::native(native, true, Some("alignment cancelled".into()));
    }

    match request_http_alignment(audio, native, settings, client, token).await {
        Ok(words) => match validate_aligned_words(native, words) {
            Ok(timeline) => AlignmentOutcome {
                timeline,
                attempted: true,
                fallback_reason: None,
            },
            Err(error) => AlignmentOutcome::native(
                native,
                true,
                Some(format!("alignment rejected: {error:#}")),
            ),
        },
        Err(error) => AlignmentOutcome::native(
            native,
            true,
            Some(format!("alignment unavailable: {error:#}")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn native() -> TranscriptTimeline {
        TranscriptTimeline {
            words: vec![
                SubtitleWord {
                    word: "Bonjour,".into(),
                    start: 0.10,
                    end: 0.40,
                },
                SubtitleWord {
                    word: "Jordan".into(),
                    start: 0.45,
                    end: 0.80,
                },
            ],
            timing_quality: crate::domain::TimingQuality::Exact,
        }
    }

    #[test]
    fn canonical_timeline_bounds_cover_first_to_last_word() {
        assert_eq!(timeline_bounds(&native()), Some((0.10, 0.80)));
    }

    #[test]
    fn valid_alignment_replaces_only_boundaries_and_marks_aligned() {
        let aligned = vec![
            SubtitleWord {
                word: "Bonjour".into(),
                start: 0.12,
                end: 0.43,
            },
            SubtitleWord {
                word: "Jordan".into(),
                start: 0.49,
                end: 0.86,
            },
        ];
        let result = validate_aligned_words(&native(), aligned).unwrap();
        assert_eq!(result.words[0].word, "Bonjour,");
        assert_eq!(result.words[0].start, 0.12);
        assert_eq!(result.words[1].end, 0.86);
        assert_eq!(result.timing_quality, crate::domain::TimingQuality::Aligned);
    }

    #[test]
    fn non_monotonic_alignment_is_rejected() {
        let aligned = vec![
            SubtitleWord {
                word: "Bonjour".into(),
                start: 0.30,
                end: 0.60,
            },
            SubtitleWord {
                word: "Jordan".into(),
                start: 0.50,
                end: 0.90,
            },
        ];
        assert!(validate_aligned_words(&native(), aligned).is_err());
    }

    #[test]
    fn lexical_mismatch_is_rejected() {
        let aligned = vec![
            SubtitleWord {
                word: "Bonsoir".into(),
                start: 0.12,
                end: 0.43,
            },
            SubtitleWord {
                word: "Jordan".into(),
                start: 0.49,
                end: 0.86,
            },
        ];
        assert!(validate_aligned_words(&native(), aligned).is_err());
    }

    #[test]
    fn parses_whisperx_word_segments_and_plain_words() {
        let nested = serde_json::json!({
            "result": {
                "word_segments": [
                    {"word": "Bonjour", "start": 0.12, "end": 0.43, "score": 0.98},
                    {"word": "Jordan", "start": 0.49, "end": 0.86}
                ]
            }
        });
        let words = parse_alignment_response(nested).unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].word, "Bonjour");
        assert_eq!((words[1].start, words[1].end), (0.49, 0.86));

        let plain = serde_json::json!({
            "words": [{"word": "Bonjour", "start": 0.12, "end": 0.43}]
        });
        assert_eq!(parse_alignment_response(plain).unwrap().len(), 1);
    }

    #[test]
    fn aligned_timeline_converts_back_to_transcription_without_retiming() {
        let timeline = TranscriptTimeline {
            words: vec![
                SubtitleWord {
                    word: "Bonjour,".into(),
                    start: 0.12,
                    end: 0.43,
                },
                SubtitleWord {
                    word: "Jordan".into(),
                    start: 0.49,
                    end: 0.86,
                },
            ],
            timing_quality: crate::domain::TimingQuality::Aligned,
        };
        let transcription = timeline_as_transcription(&timeline);
        let words = transcription.words.unwrap();
        assert_eq!(words[0].word.as_deref(), Some("Bonjour,"));
        assert_eq!(words[0].start, Some(0.12));
        assert_eq!(words[1].end, Some(0.86));
    }

    #[tokio::test]
    async fn configured_http_aligner_replaces_native_boundaries() {
        use axum::{Json, Router, routing::post};
        use reqwest::Client;
        use tokio_util::sync::CancellationToken;

        let app = Router::new().route(
            "/align",
            post(|_body: axum::body::Bytes| async {
                Json(serde_json::json!({
                    "word_segments": [
                        {"word": "Bonjour", "start": 0.12, "end": 0.43},
                        {"word": "Jordan", "start": 0.49, "end": 0.86}
                    ]
                }))
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let dir = tempfile::tempdir().unwrap();
        let audio = dir.path().join("sample.wav");
        tokio::fs::write(&audio, b"RIFFtest").await.unwrap();
        let settings = crate::domain::Settings {
            alignment_enabled: true,
            alignment_url: format!("http://{addr}/align"),
            alignment_model: "facebook/wav2vec2-large-xlsr-53-french".into(),
            ..crate::domain::Settings::default()
        };

        let result = align_timeline(
            &audio,
            &native(),
            &settings,
            &Client::new(),
            &CancellationToken::new(),
        )
        .await;

        assert!(result.attempted);
        assert!(result.fallback_reason.is_none());
        assert_eq!(
            result.timeline.timing_quality,
            crate::domain::TimingQuality::Aligned
        );
        assert_eq!(result.timeline.words[0].start, 0.12);
    }

    #[tokio::test]
    async fn unavailable_aligner_falls_back_to_native_timing() {
        use reqwest::Client;
        use tokio_util::sync::CancellationToken;

        let settings = crate::domain::Settings {
            alignment_enabled: true,
            alignment_url: "http://127.0.0.1:9/align".into(),
            ..crate::domain::Settings::default()
        };
        let result = align_timeline(
            std::path::Path::new("/missing.wav"),
            &native(),
            &settings,
            &Client::new(),
            &CancellationToken::new(),
        )
        .await;

        assert!(result.attempted);
        assert!(result.fallback_reason.is_some());
        assert_eq!(result.timeline, native());
    }
}
