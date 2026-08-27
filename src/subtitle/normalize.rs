use crate::domain::{SubtitleLine, SubtitleWord};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizeOptions {
    pub min_line_duration: f64,
    pub gap: f64,
    pub min_word_duration: f64,
}

impl Default for NormalizeOptions {
    fn default() -> Self {
        Self { min_line_duration: 0.080, gap: 0.010, min_word_duration: 0.020 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizationReport {
    pub lines: Vec<SubtitleLine>,
    pub repaired_line_overlaps: usize,
    pub retimed_word_lines: usize,
    pub dropped_empty_lines: usize,
}

pub fn normalize_subtitles(lines: &[SubtitleLine], options: NormalizeOptions) -> NormalizationReport {
    let min_line = options.min_line_duration.max(0.001);
    let gap = options.gap.max(0.0);
    let mut dropped = 0usize;

    let mut out: Vec<SubtitleLine> = lines.iter().filter_map(|line| {
        let text = line.text.trim().to_string();
        if text.is_empty() { dropped += 1; return None; }
        let start = finite_non_negative(line.start);
        let mut end = finite_non_negative(line.end);
        if end < start + min_line { end = start + min_line; }
        Some(SubtitleLine { id: line.id, start, end, text, words: line.words.clone() })
    }).collect();

    out.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.end.total_cmp(&b.end)));

    let mut repaired = 0usize;
    if out.len() > 1 {
        for i in 0..out.len() - 1 {
            if out[i].end + gap <= out[i + 1].start { continue; }
            repaired += 1;

            let left_min_end = out[i].start + min_line;
            let right_max_start = out[i + 1].end - min_line;

            if left_min_end + gap <= right_max_start {
                let desired_seam = (out[i].end + out[i + 1].start) / 2.0;
                let seam_min = left_min_end + gap / 2.0;
                let seam_max = right_max_start - gap / 2.0;
                let seam = desired_seam.clamp(seam_min, seam_max);
                out[i].end = seam - gap / 2.0;
                out[i + 1].start = seam + gap / 2.0;
            } else {
                out[i].end = left_min_end;
                out[i + 1].start = out[i].end + gap;
                if out[i + 1].end < out[i + 1].start + min_line {
                    out[i + 1].end = out[i + 1].start + min_line;
                }
            }
        }
    }

    let mut retimed_word_lines = 0usize;
    for (idx, line) in out.iter_mut().enumerate() {
        line.id = idx as u32;
        let (words, retimed) = normalize_words(line, options.min_word_duration);
        if retimed { retimed_word_lines += 1; }
        line.words = Some(words);
    }

    NormalizationReport {
        lines: out,
        repaired_line_overlaps: repaired,
        retimed_word_lines,
        dropped_empty_lines: dropped,
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() { value.max(0.0) } else { 0.0 }
}

fn normalize_words(line: &SubtitleLine, configured_min_word: f64) -> (Vec<SubtitleWord>, bool) {
    let tokens: Vec<&str> = line.text.split_whitespace().collect();
    if tokens.is_empty() { return (Vec::new(), false); }

    let duration = (line.end - line.start).max(0.001);
    let n = tokens.len();
    let fit_min = (duration / (n as f64 * 1.25)).max(0.001);
    let min_word = configured_min_word.max(0.001).min(fit_min);

    if let Some(existing) = &line.words {
        if existing.len() == n && existing.iter().all(|word| word.start.is_finite() && word.end.is_finite()) {
            let mut result = Vec::with_capacity(n);
            let mut previous_end = line.start;
            let mut changed = false;

            for (i, (token, old)) in tokens.iter().zip(existing.iter()).enumerate() {
                let remaining = n - i - 1;
                let latest_start = (line.end - min_word * (remaining as f64 + 1.0)).max(previous_end);
                let desired_start = if i == 0 { line.start } else { old.start };
                let start = desired_start.clamp(previous_end, latest_start);
                let latest_end = (line.end - min_word * remaining as f64).max(start + min_word);
                let end = if i == n - 1 {
                    line.end
                } else {
                    old.end.clamp(start + min_word, latest_end)
                };
                if (start - old.start).abs() > 1e-6 || (end - old.end).abs() > 1e-6 || old.word != *token {
                    changed = true;
                }
                result.push(SubtitleWord { word: (*token).to_string(), start, end });
                previous_end = end;
            }
            return (result, changed);
        }
    }

    let weights: Vec<usize> = tokens.iter().map(|token| token.graphemes(true).count().max(1)).collect();
    let total_weight: usize = weights.iter().sum();
    let mut result = Vec::with_capacity(n);
    let mut cursor = line.start;

    for (i, (token, weight)) in tokens.iter().zip(weights.iter()).enumerate() {
        let remaining = n - i - 1;
        let proportional = duration * (*weight as f64 / total_weight as f64);
        let target = proportional.max(min_word);
        let latest_end = line.end - min_word * remaining as f64;
        let end = if i == n - 1 { line.end } else { (cursor + target).min(latest_end) };
        result.push(SubtitleWord { word: (*token).to_string(), start: cursor, end: end.max(cursor + min_word).min(line.end) });
        cursor = result.last().map(|w| w.end).unwrap_or(cursor);
    }
    (result, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(id: u32, start: f64, end: f64, text: &str) -> SubtitleLine {
        SubtitleLine { id, start, end, text: text.into(), words: None }
    }

    fn assert_invariants(lines: &[SubtitleLine], options: NormalizeOptions) {
        for (i, line) in lines.iter().enumerate() {
            assert!(line.start.is_finite() && line.end.is_finite());
            assert!(line.start >= 0.0);
            assert!(line.end - line.start + 1e-9 >= options.min_line_duration);
            if let Some(words) = &line.words {
                let mut previous = line.start;
                for word in words {
                    assert!(word.start + 1e-9 >= line.start);
                    assert!(word.start + 1e-9 >= previous);
                    assert!(word.end > word.start);
                    assert!(word.end <= line.end + 1e-9, "word {:?} escaped line {:?}", word, line);
                    previous = word.end;
                }
            }
            if let Some(next) = lines.get(i + 1) {
                assert!(line.end + options.gap <= next.start + 1e-9,
                    "overlap: {}..{} then {}..{}", line.start, line.end, next.start, next.end);
            }
        }
    }

    #[test]
    fn repairs_overlap_by_sharing_the_seam() {
        let options = NormalizeOptions::default();
        let result = normalize_subtitles(&[
            line(0, 0.0, 2.0, "première ligne"),
            line(1, 1.5, 3.0, "deuxième ligne"),
        ], options);
        assert_eq!(result.repaired_line_overlaps, 1);
        assert!(result.lines[0].end < 2.0);
        assert!(result.lines[1].start > 1.5);
        assert_invariants(&result.lines, options);
    }

    #[test]
    fn word_end_never_escapes_parent_after_edit() {
        let options = NormalizeOptions::default();
        let mut edited = line(0, 1.0, 1.8, "bonjour le monde");
        edited.words = Some(vec![
            SubtitleWord { word: "bonjour".into(), start: 0.8, end: 1.4 },
            SubtitleWord { word: "le".into(), start: 1.35, end: 2.1 },
            SubtitleWord { word: "monde".into(), start: 2.0, end: 2.4 },
        ]);
        let result = normalize_subtitles(&[edited], options);
        assert_invariants(&result.lines, options);
        assert_eq!(result.lines[0].words.as_ref().unwrap().last().unwrap().end, 1.8);
    }

    #[test]
    fn token_count_change_retimes_words_inside_line() {
        let options = NormalizeOptions::default();
        let mut edited = line(0, 0.0, 1.0, "un texte beaucoup plus long");
        edited.words = Some(vec![
            SubtitleWord { word: "un".into(), start: 0.0, end: 0.5 },
            SubtitleWord { word: "texte".into(), start: 0.5, end: 1.0 },
        ]);
        let result = normalize_subtitles(&[edited], options);
        assert_eq!(result.retimed_word_lines, 1);
        assert_eq!(result.lines[0].words.as_ref().unwrap().len(), 5);
        assert_invariants(&result.lines, options);
    }

    #[test]
    fn invalid_and_negative_timestamps_are_sanitized() {
        let options = NormalizeOptions::default();
        let result = normalize_subtitles(&[
            line(0, f64::NAN, f64::INFINITY, "hello"),
            line(1, -2.0, -1.0, "world"),
        ], options);
        assert_invariants(&result.lines, options);
    }

    #[test]
    fn cascading_tiny_windows_remain_non_overlapping() {
        let options = NormalizeOptions::default();
        let result = normalize_subtitles(&[
            line(0, 0.0, 0.02, "a"),
            line(1, 0.01, 0.03, "b"),
            line(2, 0.02, 0.04, "c"),
        ], options);
        assert_invariants(&result.lines, options);
    }
}
