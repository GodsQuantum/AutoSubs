//! Optional one-shot forced alignment for ASR engines without word timings.
//! Owned by AutoSubs, never by the general-purpose Speaches transcriber.
use crate::domain::{RawWord, TranscriptionResponse};
use crate::media::process::run_capture;
use anyhow::{Context, Result, anyhow, bail};
use regex::Regex;
use std::path::Path;
use tokio::{io::AsyncWriteExt, process::Command};
use tokio_util::sync::CancellationToken;

fn read_timestamp(value: &str) -> Option<f64> {
    let (time, ms) = value.split_once(',')?;
    let mut parts = time.split(':');
    let (hours, minutes, seconds) = (
        parts.next()?.parse::<u32>().ok()?,
        parts.next()?.parse::<u32>().ok()?,
        parts.next()?.parse::<u32>().ok()?,
    );
    if parts.next().is_some() || minutes > 59 || seconds > 59 || ms.len() != 3 {
        return None;
    }
    let milliseconds = ms.parse::<u32>().ok()?;
    Some((hours * 3600 + minutes * 60 + seconds) as f64 + f64::from(milliseconds) / 1000.0)
}

fn normalized_lexemes(text: &str) -> String {
    text.chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Parse only the word-level SRT cues; diagnostic log lines are ignored.
/// Forbid rewriting the user's ASR transcript even if the aligner emits
/// different punctuation, names, omitted words, or hallucinations.
pub fn parse_aligned_words(output: &str, exact_asr_text: &str) -> Result<Vec<RawWord>> {
    let pattern = Regex::new(
        r"(?m)^(\d+)\r?\n(\d{2}:\d{2}:\d{2},\d{3}) --> (\d{2}:\d{2}:\d{2},\d{3})\r?\n([^\r\n]+)",
    )?;
    let mut words = Vec::new();
    for (next_index, captures) in (1_usize..).zip(pattern.captures_iter(output)) {
        let number = captures[1].parse::<usize>()?;
        if number != next_index {
            bail!("forced aligner cue index is not continuous: {number} != {next_index}");
        }
        let start = read_timestamp(&captures[2]).ok_or_else(|| anyhow!("invalid start time"))?;
        let end = read_timestamp(&captures[3]).ok_or_else(|| anyhow!("invalid end time"))?;
        let text = captures[4].trim();
        if text.is_empty() || text.contains('\n') || !start.is_finite() || end < start {
            bail!("invalid forced-alignment cue {number}");
        }
        words.push(RawWord {
            word: Some(text.to_owned()),
            start: Some(start),
            end: Some(end),
        });
        if words.len() > 30_000 {
            bail!("forced alignment contains too many words");
        }
    }
    if words.is_empty() {
        bail!("forced aligner returned no word-level SRT cues");
    }
    let joined = words
        .iter()
        .filter_map(|word| word.word.as_deref())
        .collect::<Vec<_>>()
        .join(" ");
    if normalized_lexemes(&joined) != normalized_lexemes(exact_asr_text) {
        bail!("forced aligner changed or omitted words from the original ASR transcript");
    }
    Ok(words)
}

/// Prefer the small language-specific native CrispASR registry model.
/// Aliases are resolved and downloaded once by CrispASR itself, rather than
/// running a new service or maintaining a custom model downloader.
fn resolve_aligner_model(
    configured: &str,
    language: &str,
    model_root: &Path,
) -> Result<(String, bool)> {
    let requested = configured.trim();
    let name = if requested.is_empty() || requested.eq_ignore_ascii_case("auto") {
        let language = language.to_ascii_lowercase();
        if language.starts_with("fr") {
            "fastconformer-aligner-fr"
        } else if language.starts_with("en") {
            "fastconformer-aligner-en-pc"
        } else {
            "canary-ctc-aligner"
        }
    } else {
        requested
    };
    if matches!(
        name,
        "fastconformer-aligner-fr"
            | "fastconformer-aligner-en-pc"
            | "qwen3-forced-aligner"
            | "canary-ctc-aligner"
    ) {
        return Ok((name.to_owned(), true));
    }
    let file = Path::new(name);
    if !file.is_absolute() || file.extension().is_none_or(|ext| ext != "gguf") {
        bail!("optional alignment model must be a supported registry alias or a local GGUF path");
    }
    let canonical_root = model_root
        .canonicalize()
        .context("local aligner model root is missing")?;
    let canonical_file = file
        .canonicalize()
        .context("local aligner GGUF does not exist")?;
    if !canonical_file.starts_with(&canonical_root) || !canonical_file.is_file() {
        bail!("local aligner GGUF must reside under the AutoSubs data/models directory");
    }
    Ok((canonical_file.to_string_lossy().into_owned(), false))
}

/// A standalone native binary is spawned only when a user has selected an
/// optional model and the ASR model did not supply native word timings.
/// No persistent alignment daemon and no requests to a Speaches alignment API.
pub async fn align_transcript_on_demand(
    audio: &Path,
    transcript: &TranscriptionResponse,
    model_path: &str,
    language: &str,
    token: &CancellationToken,
) -> Result<TranscriptionResponse> {
    let text = transcript
        .text
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow!("ASR did not return text to align"))?;
    // Jobs are always extracted into the application's own <data>/work
    // directory. Keep both model/cache paths relative to that data root,
    // never hardcoded to Linux /data on Windows/macOS native builds.
    let data_root = audio
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| anyhow!("cannot derive AutoSubs data directory from work audio"))?;
    let (model, auto_download) =
        resolve_aligner_model(model_path, language, &data_root.join("models"))?;
    let gpu = std::env::var("AUTOSUBS_ALIGNMENT_GPU_BACKEND").unwrap_or_else(|_| "auto".into());
    if !matches!(gpu.as_str(), "auto" | "vulkan" | "cpu" | "cuda" | "metal") {
        bail!("unsupported optional alignment GPU backend");
    }
    let text_file = audio.with_extension("autosubs-asr-text.txt");
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options
        .open(&text_file)
        .await
        .context("open private ASR transcript file for optional alignment")?;
    file.write_all(text.as_bytes()).await?;
    file.flush().await?;
    drop(file);
    let binary = std::env::var("AUTOSUBS_ALIGNMENT_EXECUTABLE").unwrap_or_else(|_| {
        if cfg!(target_os = "windows") {
            "crispasr.exe".into()
        } else if Path::new("/opt/crispasr/crispasr").is_file() {
            "/opt/crispasr/crispasr".into()
        } else {
            "crispasr".into()
        }
    });
    let mut command = Command::new(binary);
    command
        .args(["--align-only", "--align-granularity", "word", "-am"])
        .arg(&model)
        .arg("-f")
        .arg(audio)
        .arg("--text-file")
        .arg(&text_file)
        .args(["--gpu-backend", &gpu]);
    if auto_download {
        let cache = data_root.join("model-cache").join("crispasr");
        if let Err(err) = tokio::fs::create_dir_all(&cache).await {
            let _ = tokio::fs::remove_file(&text_file).await;
            return Err(err).context("prepare optional aligner model cache");
        }
        command
            .arg("--auto-download")
            .arg("--cache-dir")
            .arg(&cache);
    }
    let captured = run_capture(command, token).await;
    let _ = tokio::fs::remove_file(&text_file).await;
    let captured = captured.context("optional forced aligner failed")?;
    let output =
        String::from_utf8(captured.stdout).context("forced aligner returned non-UTF8 output")?;
    let words = parse_aligned_words(&output, text)?;
    Ok(TranscriptionResponse {
        text: transcript.text.clone(),
        words: Some(words),
        segments: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const VALID: &str = "crispasr 0.8.53\n1\n00:00:00,000 --> 00:00:00,400\nTeddy\n\n2\n00:00:00,400 --> 00:00:00,480\nRiner\n\n3\n00:00:00,640 --> 00:00:00,640\nqui\n\n4\n00:00:00,640 --> 00:00:00,800\na\n";
    #[test]
    fn aligner_output_preserves_exact_transcript_and_detects_zero_spans() {
        let words = parse_aligned_words(VALID, "Teddy Riner qui a").unwrap();
        assert_eq!(words.len(), 4);
        assert_eq!(words[0].word.as_deref(), Some("Teddy"));
        assert_eq!(words[1].start, Some(0.4));
        assert_eq!(words[2].start, words[2].end);
        assert!(parse_aligned_words(VALID, "Tedéryner qui a").is_err());
        assert!(parse_aligned_words(VALID, "Teddy Riner qui").is_err());
    }
    #[test]
    fn native_model_registry_defaults_require_no_manual_models_or_daemons() {
        assert_eq!(
            resolve_aligner_model("auto", "fr-FR", Path::new("/tmp")).unwrap(),
            ("fastconformer-aligner-fr".to_owned(), true)
        );
        assert_eq!(
            resolve_aligner_model("", "en-US", Path::new("/tmp")).unwrap(),
            ("fastconformer-aligner-en-pc".to_owned(), true)
        );
        assert_eq!(
            resolve_aligner_model("auto", "es", Path::new("/tmp")).unwrap(),
            ("canary-ctc-aligner".to_owned(), true)
        );
        assert_eq!(
            resolve_aligner_model("qwen3-forced-aligner", "fr", Path::new("/tmp")).unwrap(),
            ("qwen3-forced-aligner".to_owned(), true)
        );
        assert!(resolve_aligner_model("fake-model", "fr", Path::new("/tmp")).is_err());
        assert!(resolve_aligner_model("/etc/passwd", "fr", Path::new("/tmp")).is_err());
    }
    #[test]
    fn rejects_disordered_cues_and_invalid_timestamps() {
        assert!(parse_aligned_words(&VALID.replace("3\n", "5\n"), "Teddy Riner qui a").is_err());
        assert!(
            parse_aligned_words(
                &VALID.replace("00:00:00,800", "00:00:00,300"),
                "Teddy Riner qui a"
            )
            .is_err()
        );
        assert!(parse_aligned_words("no timestamps", "Teddy").is_err());
    }
}
