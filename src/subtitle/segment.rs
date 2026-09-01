use crate::domain::{
    RawWord, SubtitleLine, SubtitleWord, TimingQuality, TranscriptTimeline, TranscriptionResponse,
};
use crate::subtitle::normalize::{NormalizeOptions, normalize_subtitles};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation;

static PROTECTED_TOKEN_RE: OnceLock<Regex> = OnceLock::new();
static ABBREVIATION_RE: OnceLock<Regex> = OnceLock::new();

type LayoutMemo = HashMap<(usize, usize), Option<(f64, Vec<usize>)>>;

#[derive(Debug, Clone, Copy)]
pub struct LayoutOptions {
    pub max_chars: u32,
    pub max_lines: u32,
    pub output_width: u32,
    pub font_size: f64,
}

fn effective_max_chars(options: LayoutOptions) -> usize {
    let metric_limit = if options.output_width > 0 && options.font_size > 0.0 {
        // Conservative fallback for an unresolved font: average Latin glyph ≈ .46em.
        options.output_width as f64 / (options.font_size * 0.46)
    } else {
        f64::INFINITY
    };
    (options.max_chars.max(4) as f64)
        .min(metric_limit.floor())
        .max(1.0) as usize
}

fn protected_token_re() -> &'static Regex {
    PROTECTED_TOKEN_RE.get_or_init(|| Regex::new(
        r"(?i)^(?:https?://\S+|www\.\S+|[\w.+-]+@[\w.-]+\.[a-z]{2,}|[@#][\p{L}\p{N}_]+|\d+(?:[.,:/-]\d+)+|[\p{L}]+(?:['’][\p{L}]+)+)$"
    ).expect("valid protected token regex"))
}

fn abbreviation_re() -> &'static Regex {
    ABBREVIATION_RE.get_or_init(|| {
        Regex::new(r"(?xi)^(?:m|mme|mlle|dr|pr|etc|ex|env|approx|st|ste|vs|cf|n°|no)\.$")
            .expect("valid abbreviation regex")
    })
}

pub fn grapheme_len(text: &str) -> usize {
    text.graphemes(true).count()
}

fn cleaned_word(word: &str) -> String {
    word.trim()
        .trim_matches(|c: char| {
            matches!(
                c,
                ',' | ';' | ':' | '!' | '?' | '.' | '…' | '(' | ')' | '[' | ']' | '"' | '«' | '»'
            )
        })
        .to_lowercase()
}

fn pair_is_bound(left: &str, right: &str) -> bool {
    let l = cleaned_word(left);
    let r = cleaned_word(right);
    if l.is_empty() || r.is_empty() {
        return false;
    }

    if left.trim_end().ends_with(['\'', '’']) || right.trim_start().starts_with(['\'', '’']) {
        return true;
    }
    if protected_token_re().is_match(left.trim()) || protected_token_re().is_match(right.trim()) {
        return left.contains("http") || right.contains("http");
    }

    const FORWARD: &[&str] = &[
        "le", "la", "les", "l", "un", "une", "des", "du", "de", "d", "au", "aux", "à", "en",
        "pour", "sur", "sous", "dans", "par", "avec", "sans", "chez", "vers", "je", "j", "tu",
        "il", "elle", "on", "nous", "vous", "ils", "elles", "me", "m", "te", "t", "se", "s", "lui",
        "leur", "y", "ce", "cet", "cette", "ces", "c", "mon", "ton", "son", "ma", "ta", "sa",
        "mes", "tes", "ses", "notre", "votre", "nos", "vos", "leurs", "ne", "n", "très", "trop",
        "plus", "moins", "bien", "si", "qui", "que", "qu", "quoi", "dont", "où", "quand",
        "comment", "pourquoi", "et", "ou", "mais", "donc", "or", "ni", "car", "parce", "tandis",
        "jusqu", "jusque", "afin", "tel", "telle", "tels", "telles", "quel", "quelle", "quels",
        "quelles", "avant", "alors", "ainsi",
    ];
    const BACKWARD: &[&str] = &["pas", "plus", "jamais", "rien", "personne"];
    if FORWARD.contains(&l.as_str()) || BACKWARD.contains(&r.as_str()) {
        return true;
    }

    matches!(
        (l.as_str(), r.as_str()),
        ("parce", "que")
            | ("bien", "que")
            | ("alors", "que")
            | ("ainsi", "que")
            | ("avant", "de")
            | ("afin", "de")
            | ("tel", "que")
            | ("telle", "que")
            | ("tels", "que")
            | ("telles", "que")
            | ("y", "a")
            | ("il", "y")
    )
}

fn unicode_allows_boundary(left: &str, right: &str) -> bool {
    let sample = format!("{} {}", left.trim(), right.trim());
    let boundary = left.trim().len() + 1;
    linebreaks(&sample).any(|(idx, opportunity)| {
        idx == boundary
            && matches!(
                opportunity,
                BreakOpportunity::Allowed | BreakOpportunity::Mandatory
            )
    })
}

fn boundary_penalty(left: &str, right: &str) -> f64 {
    if pair_is_bound(left, right) {
        return 10_000.0;
    }
    if !unicode_allows_boundary(left, right) {
        return 5_000.0;
    }
    if right
        .trim_start()
        .starts_with([',', '.', ';', ':', '!', '?', '…', ')', ']', '»'])
    {
        return 10_000.0;
    }
    if left.trim_end().ends_with(['(', '[', '«']) {
        return 10_000.0;
    }
    if abbreviation_re().is_match(left.trim()) {
        return 500.0;
    }
    if left.trim_end().ends_with(['.', '!', '?', '…']) {
        return -120.0;
    }
    if left.trim_end().ends_with([',', ';', ':']) {
        return -45.0;
    }
    0.0
}

fn fix_tokenization(words: Vec<SubtitleWord>) -> Vec<SubtitleWord> {
    let mut fixed: Vec<SubtitleWord> = Vec::with_capacity(words.len());
    for word in words {
        if let Some(last) = fixed.last_mut() {
            let left = last.word.trim_end();
            let right = word.word.trim_start();
            if left.ends_with(['\'', '’']) || right.starts_with(['\'', '’']) {
                last.word = format!("{}{}", left, right);
                last.end = word.end;
                continue;
            }
        }
        fixed.push(word);
    }
    fixed
}

fn raw_words(transcription: &TranscriptionResponse) -> Vec<SubtitleWord> {
    let (words, _) = raw_words_with_quality(transcription);
    fix_tokenization(words)
}

fn raw_words_with_quality(
    transcription: &TranscriptionResponse,
) -> (Vec<SubtitleWord>, TimingQuality) {
    let mut result = Vec::new();
    let mut exact = true;
    if let Some(words) = &transcription.words {
        append_raw_words(&mut result, words, &mut exact);
    }
    if result.is_empty()
        && let Some(segments) = &transcription.segments
    {
        for segment in segments {
            if let Some(words) = &segment.words {
                append_raw_words(&mut result, words, &mut exact);
                continue;
            }
            if let Some(text) = &segment.text {
                let tokens: Vec<&str> = text.split_whitespace().collect();
                if tokens.is_empty() {
                    continue;
                }
                let start = segment.start.unwrap_or(0.0).max(0.0);
                let end = segment.end.unwrap_or(start + 1.0).max(start + 0.1);
                let total: usize = tokens.iter().map(|v| grapheme_len(v).max(1)).sum();
                let mut cursor = start;
                for (idx, token) in tokens.iter().enumerate() {
                    exact = false;
                    let duration =
                        (end - start) * (grapheme_len(token).max(1) as f64 / total as f64);
                    let token_end = if idx == tokens.len() - 1 {
                        end
                    } else {
                        cursor + duration
                    };
                    result.push(SubtitleWord {
                        word: (*token).into(),
                        start: cursor,
                        end: token_end,
                    });
                    cursor = token_end;
                }
            }
        }
    }
    if result.is_empty()
        && let Some(text) = &transcription.text
    {
        let mut cursor = 0.0;
        for token in text.split_whitespace() {
            exact = false;
            result.push(SubtitleWord {
                word: token.into(),
                start: cursor,
                end: cursor + 0.4,
            });
            cursor += 0.4;
        }
    }
    let quality = if exact && !result.is_empty() {
        TimingQuality::Exact
    } else {
        TimingQuality::Inferred
    };
    (result, quality)
}

fn append_raw_words(out: &mut Vec<SubtitleWord>, words: &[RawWord], exact: &mut bool) {
    for word in words {
        let text = word.word.as_deref().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let (start, end) = match (word.start, word.end) {
            (Some(start), Some(end)) if start.is_finite() && end.is_finite() && end >= start => {
                (start, end)
            }
            _ => {
                *exact = false;
                let start = word.start.unwrap_or(0.0).max(0.0);
                (start, word.end.unwrap_or(start + 0.04).max(start + 0.02))
            }
        };
        out.push(SubtitleWord {
            word: text.into(),
            start,
            end,
        });
    }
}

fn segment_text_len(words: &[SubtitleWord]) -> usize {
    if words.is_empty() {
        return 0;
    }
    words
        .iter()
        .map(|w| grapheme_len(w.word.trim()))
        .sum::<usize>()
        + words.len().saturating_sub(1)
}

fn line_cost(words: &[SubtitleWord], max_chars: usize, is_final_line: bool) -> f64 {
    let len = segment_text_len(words) as f64;
    let max = max_chars.max(1) as f64;
    let overflow = (len - max).max(0.0);
    let under = (max - len).max(0.0);
    let mut cost =
        overflow * overflow * 120.0 + under * under * if is_final_line { 0.10 } else { 0.35 };
    if len < max * 0.32 && !is_final_line {
        cost += 120.0;
    }
    cost
}

fn best_layout(words: &[SubtitleWord], max_chars: usize, max_lines: usize) -> Vec<usize> {
    fn solve(
        words: &[SubtitleWord],
        start: usize,
        remaining: usize,
        max_chars: usize,
        memo: &mut LayoutMemo,
    ) -> Option<(f64, Vec<usize>)> {
        if start == words.len() {
            return Some((0.0, Vec::new()));
        }
        if remaining == 0 {
            return None;
        }
        if let Some(cached) = memo.get(&(start, remaining)) {
            return cached.clone();
        }

        let mut best: Option<(f64, Vec<usize>)> = None;
        for end in start + 1..=words.len() {
            let words_left = words.len() - end;
            if remaining == 1 && words_left > 0 {
                continue;
            }
            if words_left > 0 && remaining <= 1 {
                continue;
            }

            let segment = &words[start..end];
            if segment_text_len(segment) > max_chars && segment.len() > 1 {
                continue;
            }
            let mut cost = line_cost(segment, max_chars, end == words.len());
            if end < words.len() {
                cost += boundary_penalty(&words[end - 1].word, &words[end].word);
            }
            if let Some((rest_cost, mut breaks)) = solve(words, end, remaining - 1, max_chars, memo)
            {
                cost += rest_cost;
                let mut candidate = vec![end];
                candidate.append(&mut breaks);
                if best.as_ref().is_none_or(|(best_cost, _)| cost < *best_cost) {
                    best = Some((cost, candidate));
                }
            }
        }
        memo.insert((start, remaining), best.clone());
        best
    }

    let mut memo = HashMap::new();
    solve(words, 0, max_lines.max(1), max_chars.max(1), &mut memo)
        .map(|(_, breaks)| breaks)
        .unwrap_or_else(|| vec![words.len()])
}

fn make_block(words: &[SubtitleWord], id: u32, max_chars: usize, max_lines: usize) -> SubtitleLine {
    let breaks = best_layout(words, max_chars, max_lines);
    let mut text_lines = Vec::new();
    let mut start_idx = 0;
    for end_idx in breaks {
        if end_idx <= start_idx || end_idx > words.len() {
            continue;
        }
        text_lines.push(
            words[start_idx..end_idx]
                .iter()
                .map(|w| w.word.trim())
                .collect::<Vec<_>>()
                .join(" "),
        );
        start_idx = end_idx;
    }
    if start_idx < words.len() {
        text_lines.push(
            words[start_idx..]
                .iter()
                .map(|w| w.word.trim())
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    SubtitleLine {
        id,
        start: words.first().map(|w| w.start).unwrap_or(0.0),
        end: words.last().map(|w| w.end).unwrap_or(0.1),
        text: text_lines.join("\n"),
        words: Some(words.to_vec()),
    }
}

pub fn group_transcription_into_lines(
    transcription: &TranscriptionResponse,
    max_chars: u32,
    max_lines: u32,
) -> Vec<SubtitleLine> {
    group_transcription_into_lines_with_layout(
        transcription,
        LayoutOptions {
            max_chars,
            max_lines,
            output_width: 0,
            font_size: 0.0,
        },
    )
}

pub fn group_transcription_into_lines_with_layout(
    transcription: &TranscriptionResponse,
    options: LayoutOptions,
) -> Vec<SubtitleLine> {
    let words = raw_words(transcription);
    if words.is_empty() {
        return Vec::new();
    }
    let max_chars = effective_max_chars(options);
    let max_lines = options.max_lines.clamp(1, 4) as usize;
    let target_block = max_chars * max_lines;
    let hard_block = target_block;

    let mut blocks = Vec::new();
    let mut current: Vec<SubtitleWord> = Vec::new();
    let mut id = 0u32;

    for word in words {
        let prospective_len = if current.is_empty() {
            grapheme_len(&word.word)
        } else {
            segment_text_len(&current) + 1 + grapheme_len(&word.word)
        };
        if !current.is_empty() && prospective_len > hard_block {
            let prev = current.last().unwrap();
            if !pair_is_bound(&prev.word, &word.word) {
                blocks.push(make_block(&current, id, max_chars, max_lines));
                id += 1;
                current.clear();
            }
        }

        let sentence_end = word.word.trim_end().ends_with(['.', '!', '?', '…'])
            && !abbreviation_re().is_match(word.word.trim());
        current.push(word);
        let current_len = segment_text_len(&current);
        if sentence_end
            || (current_len >= target_block
                && current.len() > 1
                && current
                    .last()
                    .is_some_and(|last| last.word.ends_with([',', ';', ':'])))
        {
            blocks.push(make_block(&current, id, max_chars, max_lines));
            id += 1;
            current.clear();
        }
    }

    if !current.is_empty() {
        blocks.push(make_block(&current, id, max_chars, max_lines));
    }
    normalize_subtitles(&blocks, NormalizeOptions::default()).lines
}

pub fn transcript_timeline(transcription: &TranscriptionResponse) -> TranscriptTimeline {
    let (words, timing_quality) = raw_words_with_quality(transcription);
    TranscriptTimeline {
        words,
        timing_quality,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcription(words: &[(&str, f64, f64)]) -> TranscriptionResponse {
        TranscriptionResponse {
            text: None,
            segments: None,
            words: Some(
                words
                    .iter()
                    .map(|(word, start, end)| RawWord {
                        word: Some((*word).into()),
                        start: Some(*start),
                        end: Some(*end),
                    })
                    .collect(),
            ),
        }
    }

    #[test]
    fn grapheme_count_is_not_utf8_byte_count() {
        assert_eq!(grapheme_len("été"), 3);
        assert_eq!(grapheme_len("👨‍👩‍👧‍👦"), 1);
    }

    #[test]
    fn protected_token_regex_covers_real_world_tokens() {
        for token in [
            "https://example.com/a-b",
            "test@example.com",
            "@creator",
            "#AutoSubs",
            "3.14",
            "l'homme",
        ] {
            assert!(
                protected_token_re().is_match(token),
                "not protected: {token}"
            );
        }
    }

    #[test]
    fn exact_transcription_timeline_round_trips_without_changing_timestamps() {
        let input = transcription(&[("hello", 0.123456789, 0.234567891)]);
        let timeline = transcript_timeline(&input);
        assert_eq!(timeline.timing_quality, TimingQuality::Exact);
        assert_eq!(timeline.words[0].start, 0.123456789);
        assert_eq!(timeline.words[0].end, 0.234567891);
        assert_eq!(
            serde_json::to_value(&timeline).unwrap()["words"][0]["start"],
            serde_json::json!(0.123456789)
        );
    }

    #[test]
    fn french_connectors_are_strong_no_break_pairs() {
        for (left, right) in [
            ("avant", "de"),
            ("afin", "de"),
            ("parce", "que"),
            ("bien", "que"),
            ("alors", "que"),
            ("ainsi", "que"),
            ("tel", "que"),
            ("y", "a"),
        ] {
            assert!(
                boundary_penalty(left, right) >= 5_000.0,
                "{left} {right} was breakable"
            );
        }
    }

    #[test]
    fn apostrophe_tokens_merge_before_layout() {
        let input = transcription(&[("l'", 0.0, 0.1), ("homme", 0.1, 0.4), ("arrive", 0.4, 0.8)]);
        let lines = group_transcription_into_lines(&input, 8, 2);
        assert!(lines.iter().any(|line| line.text.contains("l'homme")));
    }

    #[test]
    fn balanced_layout_avoids_breaking_avant_de() {
        let input = transcription(&[
            ("Je", 0.0, 0.1),
            ("pars", 0.1, 0.2),
            ("avant", 0.2, 0.4),
            ("de", 0.4, 0.5),
            ("manger", 0.5, 0.8),
            ("ici.", 0.8, 1.0),
        ]);
        let lines = group_transcription_into_lines(&input, 10, 2);
        assert!(
            lines.iter().all(|line| !line.text.contains("avant\nde")),
            "{:?}",
            lines
        );
    }

    #[test]
    fn larger_font_size_splits_events_before_visual_lines_overflow() {
        let input = transcription(&[
            ("abcdefgh", 0.0, 0.2),
            ("ijklmnop", 0.2, 0.4),
            ("qrstuvwx", 0.4, 0.6),
            ("yzabcdef", 0.6, 0.8),
        ]);
        let small = group_transcription_into_lines_with_layout(
            &input,
            LayoutOptions {
                max_chars: 40,
                max_lines: 2,
                output_width: 640,
                font_size: 20.0,
            },
        );
        let large = group_transcription_into_lines_with_layout(
            &input,
            LayoutOptions {
                max_chars: 40,
                max_lines: 2,
                output_width: 640,
                font_size: 80.0,
            },
        );
        assert_eq!(small.len(), 1);
        assert!(large.len() > 1);
        assert!(large.iter().all(|line| line.text.lines().count() <= 2));
    }

    #[test]
    fn french_sentence_prefers_natural_bottom_heavy_break() {
        let input = transcription(&[
            ("Je", 0.0, 0.1),
            ("vais", 0.1, 0.2),
            ("vraiment", 0.2, 0.3),
            ("vous", 0.3, 0.4),
            ("montrer", 0.4, 0.5),
            ("comment", 0.5, 0.6),
            ("ça", 0.6, 0.7),
            ("fonctionne.", 0.7, 0.8),
        ]);
        let lines = group_transcription_into_lines_with_layout(
            &input,
            LayoutOptions {
                max_chars: 40,
                max_lines: 2,
                output_width: 330,
                font_size: 24.0,
            },
        );
        assert_eq!(lines.len(), 1);
        assert_eq!(
            lines[0].text,
            "Je vais vraiment vous montrer\ncomment ça fonctionne."
        );
    }
}
