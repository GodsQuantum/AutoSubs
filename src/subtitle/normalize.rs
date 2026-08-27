use crate::subtitle::types::{SubtitleLine, SubtitleWord};

const MIN_LINE_DURATION: f64 = 0.08; // 80ms
const GAP: f64 = 0.010;             // 10ms

/// Cascading overlap-prevention engine.
/// Sort → enforce min duration → enforce gap between consecutive lines (cascade forward).
/// Also normalises word-level timestamps within each line.
pub fn normalize_and_fix_overlaps(lines: &[SubtitleLine]) -> Vec<SubtitleLine> {
    if lines.is_empty() {
        return vec![];
    }

    let mut sorted: Vec<SubtitleLine> = lines
        .iter()
        .filter(|l| !l.text.trim().is_empty())
        .enumerate()
        .map(|(_i, l)| SubtitleLine {
            id: l.id,
            start: l.start.max(0.0),
            end: l.end.max(0.0),
            text: l.text.trim().to_string(),
            words: l.words.clone(),
        })
        .collect();

    sorted.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.end.partial_cmp(&b.end).unwrap_or(std::cmp::Ordering::Equal))
    });

    if sorted.is_empty() {
        return vec![];
    }

    // Pass 1: enforce minimum duration
    for line in sorted.iter_mut() {
        if line.end < line.start + MIN_LINE_DURATION {
            line.end = line.start + MIN_LINE_DURATION;
        }
    }

    // Pass 2: enforce inter-line gap (cascade forward)
    for i in 0..sorted.len() - 1 {
        let current_end = sorted[i].end;
        let next_start   = sorted[i + 1].start;
        if current_end > next_start - GAP {
            let target_end = next_start - GAP;
            if target_end >= sorted[i].start + MIN_LINE_DURATION {
                sorted[i].end = target_end;
            } else {
                sorted[i].end = sorted[i].start + MIN_LINE_DURATION;
                sorted[i + 1].start = sorted[i].end + GAP;
                if sorted[i + 1].end < sorted[i + 1].start + MIN_LINE_DURATION {
                    sorted[i + 1].end = sorted[i + 1].start + MIN_LINE_DURATION;
                }
            }
        }
    }

    // Pass 3: normalize word timestamps per line
    sorted
        .into_iter()
        .enumerate()
        .map(|(idx, mut line)| {
            line.id = idx as u32;
            let words = normalize_line_words(&line);
            line.words = Some(words);
            line
        })
        .collect()
}

/// Rescales word-level timestamps to strictly lie within [line.start, line.end].
/// If word data is missing or corrupt, distributes proportionally by character count.
pub fn normalize_line_words(line: &SubtitleLine) -> Vec<SubtitleWord> {
    let line_start = line.start.max(0.0);
    let line_end = (line.end).max(line_start + 0.05);
    let total_dur = line_end - line_start;

    let raw_tokens: Vec<&str> = line
        .text
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .collect();

    if raw_tokens.is_empty() {
        return vec![];
    }

    if let Some(existing) = &line.words {
        if existing.len() == raw_tokens.len() && !existing.is_empty() {
            let first_start = existing[0].start;
            let last_end = existing[existing.len() - 1].end;
            let existing_span = last_end - first_start;

            let needs_rescale = existing_span <= 0.0
                || first_start < line_start - 0.5
                || last_end > line_end + 0.5
                || (existing_span - total_dur).abs() > 1.0;

            if !needs_rescale {
                // Clamp each word into the line window
                let mut fixed: Vec<SubtitleWord> = Vec::with_capacity(raw_tokens.len());
                let mut current_start = line_start;
                for (i, token) in raw_tokens.iter().enumerate() {
                    let orig_start = current_start.max(existing[i].start);
                    let orig_end = if i == raw_tokens.len() - 1 {
                        line_end.max(orig_start + 0.04)
                    } else {
                        existing[i].end.max(orig_start + 0.04)
                    };
                    fixed.push(SubtitleWord {
                        word: token.to_string(),
                        start: orig_start,
                        end: orig_end,
                    });
                    current_start = orig_end;
                }
                return fixed;
            }
        }
    }

    // Proportional distribution by character count
    distribute_by_chars(&raw_tokens, line_start, line_end, total_dur)
}

fn distribute_by_chars(
    tokens: &[&str],
    line_start: f64,
    line_end: f64,
    total_dur: f64,
) -> Vec<SubtitleWord> {
    let total_chars: usize = tokens.iter().map(|t| t.len().max(1)).sum();
    let mut offset = 0.0;
    let n = tokens.len();

    tokens
        .iter()
        .enumerate()
        .map(|(i, token)| {
            let char_frac = token.len().max(1) as f64 / total_chars as f64;
            let word_dur = (char_frac * total_dur).max(0.03);
            let w_start = line_start + offset;
            let w_end = if i == n - 1 {
                line_end
            } else {
                (w_start + word_dur).min(line_end - 0.02)
            };
            offset += word_dur;
            SubtitleWord {
                word: token.to_string(),
                start: w_start,
                end: (w_start + 0.02).max(w_end),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(id: u32, start: f64, end: f64, text: &str) -> SubtitleLine {
        SubtitleLine { id, start, end, text: text.into(), words: None }
    }

    #[test]
    fn no_overlap_after_fix() {
        let lines = vec![
            make_line(0, 0.0, 1.5, "hello"),
            make_line(1, 1.0, 2.5, "world"), // overlaps with first
        ];
        let fixed = normalize_and_fix_overlaps(&lines);
        assert_eq!(fixed.len(), 2);
        assert!(fixed[0].end + GAP <= fixed[1].start + 1e-9,
            "gap violated: {} > {}", fixed[0].end, fixed[1].start);
    }

    #[test]
    fn min_duration_enforced() {
        let lines = vec![make_line(0, 0.0, 0.01, "tiny")];
        let fixed = normalize_and_fix_overlaps(&lines);
        assert!(fixed[0].end - fixed[0].start >= MIN_LINE_DURATION);
    }

    #[test]
    fn empty_input() {
        assert!(normalize_and_fix_overlaps(&[]).is_empty());
    }
}
