use crate::domain::{
    RawWord, SubtitleLine, SubtitleWord, TimingQuality, TranscriptTimeline, TranscriptionResponse,
};
use regex::Regex;
use std::sync::OnceLock;
use unicode_linebreak::{BreakOpportunity, linebreaks};
use unicode_segmentation::UnicodeSegmentation;

static PROTECTED_TOKEN_RE: OnceLock<Regex> = OnceLock::new();
static ABBREVIATION_RE: OnceLock<Regex> = OnceLock::new();

#[derive(Debug, Clone, Copy)]
pub struct LayoutOptions {
    pub max_chars: u32,
    pub max_lines: u32,
    pub output_width: u32,
    pub font_size: f64,
}

fn effective_max_chars(options: LayoutOptions) -> usize {
    let metric_limit = if options.output_width > 0 && options.font_size > 0.0 {
        // Conservative unresolved-font estimate with one-character tolerance at caption sizes.
        options.output_width as f64 * 0.9 / (options.font_size * 0.425)
            + if options.font_size <= 32.0 { 1.0 } else { 0.0 }
    } else {
        f64::INFINITY
    };
    (options.max_chars.clamp(4, 42) as f64)
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
                ',' | ';'
                    | ':'
                    | '!'
                    | '?'
                    | '.'
                    | '…'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | '"'
                    | '«'
                    | '»'
                    | '“'
                    | '”'
                    | '‘'
                    | '’'
            )
        })
        .to_lowercase()
}

fn pair_is_bound(left: &str, right: &str) -> bool {
    let left_trimmed = left.trim_end();
    let right_trimmed = right.trim_start();
    if right_trimmed.starts_with([',', '.', ';', ':', '!', '?', '…', ')', ']', '}', '»', '”'])
        || left_trimmed.ends_with(['(', '[', '{', '«', '“'])
    {
        return true;
    }
    let l = cleaned_word(left);
    let r = cleaned_word(right);
    if l.is_empty() || r.is_empty() {
        return false;
    }

    if left_trimmed.ends_with(['\'', '’']) || right_trimmed.starts_with(['\'', '’']) {
        return true;
    }
    if protected_token_re().is_match(left.trim()) || protected_token_re().is_match(right.trim()) {
        return left.contains("http") || right.contains("http");
    }

    const FORWARD: &[&str] = &[
        "le", "la", "les", "l", "un", "une", "des", "du", "de", "d", "au", "aux", "à", "en",
        "pour", "sur", "sous", "dans", "par", "avec", "sans", "chez", "vers", "je", "j", "tu",
        "il", "elle", "on", "nous", "ils", "elles", "me", "m", "te", "t", "se", "s", "lui", "leur",
        "y", "ce", "cet", "cette", "ces", "c", "mon", "ton", "son", "ma", "ta", "sa", "mes", "tes",
        "ses", "notre", "votre", "nos", "vos", "leurs", "ne", "n", "très", "trop", "plus", "moins",
        "bien", "si", "qui", "que", "qu", "quoi", "dont", "où", "quand", "comment", "pourquoi",
        "et", "ou", "mais", "donc", "or", "ni", "car", "parce", "tandis", "jusqu", "jusque",
        "afin", "tel", "telle", "tels", "telles", "quel", "quelle", "quels", "quelles", "avant",
        "alors", "ainsi",
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
        .starts_with([',', '.', ';', ':', '!', '?', '…', ')', ']', '}', '»', '”'])
    {
        return 10_000.0;
    }
    if left.trim_end().ends_with(['(', '[', '{', '«', '“']) {
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

fn ends_sentence(word: &str) -> bool {
    word.trim_end()
        .trim_end_matches(['\"', '”', '’', '»', ')', ']', '}'])
        .ends_with(['.', '!', '?', '…'])
}

fn fix_tokenization(words: Vec<SubtitleWord>) -> Vec<SubtitleWord> {
    let mut fixed: Vec<SubtitleWord> = Vec::with_capacity(words.len());
    let mut pending_prefix: Option<SubtitleWord> = None;
    let mut double_quote_open = false;

    for mut word in words {
        let token = word.word.trim().to_owned();
        let token = token.as_str();
        let symmetric_quote = token == "\"";
        let opening = matches!(token, "(" | "[" | "{" | "«" | "“" | "‘")
            || (symmetric_quote && !double_quote_open);
        let closing =
            matches!(token, ")" | "]" | "}" | "”") || (symmetric_quote && double_quote_open);
        if symmetric_quote {
            double_quote_open = !double_quote_open;
        }
        if opening {
            pending_prefix = Some(match pending_prefix.take() {
                Some(mut prefix) => {
                    prefix.word.push_str(token);
                    prefix.end = word.end;
                    prefix
                }
                None => word,
            });
            continue;
        }
        if let Some(prefix) = pending_prefix.take() {
            let spacing = if prefix.word.trim_end().ends_with('«') {
                "\u{202f}"
            } else {
                ""
            };
            word.word = format!("{}{spacing}{}", prefix.word.trim(), word.word.trim());
            word.start = prefix.start;
        }
        if (closing || matches!(token, "," | "." | "…" | ")" | "]" | "}" | "»"))
            && let Some(last) = fixed.last_mut()
        {
            if token == "»" {
                last.word.push('\u{202f}');
            }
            last.word.push_str(token);
            last.end = word.end;
            continue;
        }
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
    if let Some(prefix) = pending_prefix {
        fixed.push(prefix);
    }
    fixed
}

fn raw_words(transcription: &TranscriptionResponse) -> Vec<SubtitleWord> {
    fix_tokenization(raw_words_with_quality(transcription).0)
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
        + words
            .windows(2)
            .filter(|pair| needs_space(&pair[0].word, &pair[1].word))
            .count()
}

fn needs_space(left: &str, right: &str) -> bool {
    !left.trim_end().ends_with(['\'', '’', '(', '[', '{', '“'])
        && !right
            .trim_start()
            .starts_with(['\'', '’', ',', '.', '…', ')', ']', '}', '»', '”'])
}

fn display_words(words: &[SubtitleWord]) -> String {
    let mut text = String::new();
    for word in words {
        if !text.is_empty() && needs_space(&text, &word.word) {
            text.push(' ');
        }
        text.push_str(word.word.trim());
    }
    text
}

fn line_cost(words: &[SubtitleWord], max_chars: usize, is_final_line: bool) -> f64 {
    let len = segment_text_len(words) as f64;
    let max = max_chars.max(1) as f64;
    let overflow = (len - max).max(0.0);
    let under = (max - len).max(0.0);
    let mut cost =
        overflow * overflow * 120.0 + under * under * if is_final_line { 0.35 } else { 0.10 };
    if len < max * 0.32 && !is_final_line {
        cost += 120.0;
    }
    cost
}

fn best_layout(words: &[SubtitleWord], max_chars: usize, max_lines: usize) -> Vec<usize> {
    if words.is_empty() {
        return Vec::new();
    }
    if segment_text_len(words) <= max_chars || max_lines <= 1 {
        return vec![words.len()];
    }

    let mut best: Option<(f64, usize)> = None;
    for split in 1..words.len() {
        let top = &words[..split];
        let bottom = &words[split..];
        let top_len = segment_text_len(top);
        let bottom_len = segment_text_len(bottom);
        if top_len > max_chars
            || bottom_len > max_chars
            || top_len > bottom_len
            || pair_is_bound(&words[split - 1].word, &words[split].word)
        {
            continue;
        }
        let cost = line_cost(top, max_chars, false)
            + line_cost(bottom, max_chars, true)
            + boundary_penalty(&words[split - 1].word, &words[split].word);
        if best.is_none_or(|(best_cost, _)| cost < best_cost) {
            best = Some((cost, split));
        }
    }
    best.map(|(_, split)| vec![split, words.len()])
        .unwrap_or_else(|| vec![words.len()])
}

fn can_layout(words: &[SubtitleWord], max_chars: usize, max_lines: usize) -> bool {
    if words.is_empty() {
        return true;
    }
    let breaks = best_layout(words, max_chars, max_lines);
    let mut start = 0;
    for end in breaks {
        if end <= start || end > words.len() {
            return false;
        }
        let visual = &words[start..end];
        if visual.len() > 1 && segment_text_len(visual) > max_chars {
            return false;
        }
        start = end;
    }
    start == words.len()
}

fn safe_event_boundary(words: &[SubtitleWord], index: usize) -> bool {
    index > 0
        && index < words.len()
        && !pair_is_bound(&words[index - 1].word, &words[index].word)
        && unicode_allows_boundary(&words[index - 1].word, &words[index].word)
}

fn event_boundary_strength(words: &[SubtitleWord], index: usize) -> i32 {
    let left = &words[index - 1];
    let right = &words[index];
    let gap = (right.start - left.end).max(0.0);
    if ends_sentence(&left.word) && !abbreviation_re().is_match(left.word.trim()) {
        500
    } else if gap >= 0.35 {
        400
    } else if gap >= 0.20 {
        300
    } else if left.word.trim_end().ends_with([';', ':', ',']) {
        200
    } else {
        100
    }
}

fn best_event_boundary(
    words: &[SubtitleWord],
    max_chars: usize,
    max_lines: usize,
) -> Option<usize> {
    (1..words.len())
        .filter(|&index| {
            safe_event_boundary(words, index) && can_layout(&words[..index], max_chars, max_lines)
        })
        .max_by_key(|&index| (event_boundary_strength(words, index), index))
        .or_else(|| {
            (1..words.len()).rev().find(|&index| {
                !pair_is_bound(&words[index - 1].word, &words[index].word)
                    && can_layout(&words[..index], max_chars, max_lines)
            })
        })
}

fn make_block(words: &[SubtitleWord], id: u32, max_chars: usize, max_lines: usize) -> SubtitleLine {
    let breaks = best_layout(words, max_chars, max_lines);
    let mut text_lines = Vec::new();
    let mut start_idx = 0;
    for end_idx in breaks {
        if end_idx <= start_idx || end_idx > words.len() {
            continue;
        }
        text_lines.push(display_words(&words[start_idx..end_idx]));
        start_idx = end_idx;
    }
    if start_idx < words.len() {
        text_lines.push(display_words(&words[start_idx..]));
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
    let max_lines = options.max_lines.clamp(1, 2) as usize;
    let mut blocks = Vec::new();
    let mut current: Vec<SubtitleWord> = Vec::new();
    let mut id = 0u32;

    for word in words {
        if let Some(previous) = current.last() {
            let gap = (word.start - previous.end).max(0.0);
            let sentence_end =
                ends_sentence(&previous.word) && !abbreviation_re().is_match(previous.word.trim());
            if !pair_is_bound(&previous.word, &word.word)
                && (sentence_end
                    || gap >= 0.35
                    || (gap >= 0.20 && segment_text_len(&current) >= max_chars / 2))
            {
                blocks.push(make_block(&current, id, max_chars, max_lines));
                id += 1;
                current.clear();
            }
        }

        current.push(word);
        while !can_layout(&current, max_chars, max_lines) && current.len() > 1 {
            let boundary = best_event_boundary(&current, max_chars, max_lines)
                .or_else(|| {
                    (1..current.len())
                        .rev()
                        .find(|&index| can_layout(&current[..index], max_chars, max_lines))
                })
                .unwrap_or(1);
            let remainder = current.split_off(boundary);
            blocks.push(make_block(&current, id, max_chars, max_lines));
            id += 1;
            current = remainder;
        }
    }

    if !current.is_empty() {
        blocks.push(make_block(&current, id, max_chars, max_lines));
    }
    blocks
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
    fn french_delivery_caps_layout_at_two_lines_and_42_characters() {
        let options = LayoutOptions {
            max_chars: 80,
            max_lines: 4,
            output_width: 3840,
            font_size: 20.0,
        };
        assert_eq!(effective_max_chars(options), 42);
        let input = transcription(&[("mot", 0.0, 0.2); 40]);
        let lines = group_transcription_into_lines_with_layout(&input, options);
        assert!(lines.iter().all(|line| line.text.lines().count() <= 2));
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
        let emitted = lines
            .into_iter()
            .flat_map(|line| line.words.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(emitted[0].word, "l'homme");
        assert_eq!(emitted[0].start, 0.0);
        assert_eq!(emitted[0].end, 0.4);
        assert_eq!(emitted[1].word, "arrive");
    }

    #[test]
    fn standalone_closing_punctuation_stays_with_the_previous_word() {
        let input = transcription(&[
            ("Bonjour", 0.0, 0.3),
            (".", 0.3, 0.35),
            ("Nouvelle", 0.35, 0.7),
            ("phrase", 0.7, 0.9),
            ("!", 0.9, 1.0),
        ]);
        let lines = group_transcription_into_lines(&input, 42, 2);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0].text, "Bonjour.");
        assert_eq!(lines[1].text, "Nouvelle phrase !");
        assert!(
            lines
                .iter()
                .all(|line| !line.text.starts_with(['.', ',', ')', ']', '»']))
        );
    }

    #[test]
    fn standalone_quotes_never_form_orphan_boundaries() {
        let input = transcription(&[
            ("Il", 0.0, 0.1),
            ("dit", 0.1, 0.2),
            (":", 0.2, 0.25),
            ("\"", 0.25, 0.3),
            ("Bonjour", 0.3, 0.6),
            (".", 0.6, 0.65),
            ("\"", 0.65, 0.7),
        ]);
        let lines = group_transcription_into_lines(&input, 12, 2);
        assert!(
            lines
                .iter()
                .flat_map(|line| line.words.as_deref().unwrap_or_default())
                .all(|word| word.word != "\""),
            "{lines:?}"
        );
        assert!(
            lines.iter().all(|line| !line.text.ends_with(": \"")),
            "{lines:?}"
        );
        assert!(
            lines.iter().all(|line| !line.text.starts_with(".\"")),
            "{lines:?}"
        );
        assert_eq!(
            lines
                .iter()
                .map(|line| line.text.replace('\n', " "))
                .collect::<Vec<_>>()
                .join(" "),
            "Il dit : \"Bonjour.\""
        );
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
            "Je vais vraiment vous\nmontrer comment ça fonctionne."
        );
    }

    #[test]
    fn significant_pause_starts_a_new_event_before_text_capacity() {
        let input = transcription(&[
            ("On", 0.123456789, 0.3),
            ("commence", 0.31, 0.8),
            ("maintenant", 1.21, 1.7),
            ("ensemble.", 1.71, 2.2),
        ]);
        let lines = group_transcription_into_lines(&input, 30, 2);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert_eq!(lines[0].text, "On commence");
        assert_eq!(lines[1].text, "maintenant ensemble.");
        assert_eq!(lines[0].start, 0.123456789);
        assert_eq!(lines[0].end, 0.8);
        assert_eq!(lines[1].start, 1.21);
        assert_eq!(lines[1].end, 2.2);
    }

    #[test]
    fn real_french_text_becomes_valid_visual_events_without_breaking_connectors() {
        let input = transcription(&[
            ("Nous", 0.0, 0.18),
            ("allons", 0.18, 0.36),
            ("prendre", 0.36, 0.54),
            ("le", 0.54, 0.62),
            ("temps", 0.62, 0.82),
            ("avant", 0.82, 1.0),
            ("de", 1.0, 1.08),
            ("publier,", 1.08, 1.35),
            ("parce", 1.36, 1.55),
            ("que", 1.55, 1.65),
            ("chaque", 1.65, 1.84),
            ("détail", 1.84, 2.05),
            ("compte", 2.05, 2.25),
            ("vraiment.", 2.25, 2.55),
        ]);
        let original = raw_words(&input);
        let lines = group_transcription_into_lines(&input, 14, 2);
        assert!(lines.len() > 1, "{lines:?}");
        for line in &lines {
            assert!(line.text.lines().count() <= 2, "{}", line.text);
            for visual in line.text.lines() {
                assert!(
                    grapheme_len(visual) <= 14 || !visual.contains(' '),
                    "{visual}"
                );
            }
            assert!(!line.text.contains("avant\nde"));
            assert!(!line.text.contains("parce\nque"));
            let words = line.words.as_ref().unwrap();
            assert_eq!(line.start, words.first().unwrap().start);
            assert_eq!(line.end, words.last().unwrap().end);
        }
        let emitted: Vec<_> = lines
            .into_iter()
            .flat_map(|line| line.words.unwrap())
            .collect();
        assert_eq!(emitted, original);
    }
    #[test]
    fn one_line_limit_always_spills_into_following_events() {
        let input = transcription(&[
            ("je", 0.0, 0.1),
            ("vais", 0.1, 0.2),
            ("dans", 0.2, 0.3),
            ("la", 0.3, 0.4),
            ("grande", 0.4, 0.5),
            ("maison", 0.5, 0.6),
        ]);
        let lines = group_transcription_into_lines(&input, 9, 1);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|line| line.text.lines().count() == 1));
        assert!(
            lines
                .iter()
                .all(|line| grapheme_len(&line.text) <= 9 || !line.text.contains(' '))
        );
    }

    #[test]
    fn two_line_layout_is_bottom_heavy() {
        let input = transcription(&[
            ("Une", 0.0, 0.1),
            ("segmentation", 0.1, 0.2),
            ("professionnelle", 0.2, 0.3),
            ("reste", 0.3, 0.4),
            ("lisible", 0.4, 0.5),
        ]);
        let lines = group_transcription_into_lines(&input, 24, 2);
        for line in lines {
            let visual = line.text.lines().collect::<Vec<_>>();
            if visual.len() == 2 {
                assert!(
                    grapheme_len(visual[0]) <= grapheme_len(visual[1]),
                    "{}",
                    line.text
                );
            }
        }
    }
}
