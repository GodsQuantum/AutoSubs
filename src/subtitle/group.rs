use crate::subtitle::normalize::{normalize_and_fix_overlaps, normalize_line_words};
use crate::subtitle::types::{SubtitleLine, SubtitleWord, TranscriptionResponse};

// ─── Tokenization fix ────────────────────────────────────────────────────────

fn fix_tokenization(words: Vec<SubtitleWord>) -> Vec<SubtitleWord> {
    if words.is_empty() {
        return words;
    }
    let mut fixed: Vec<SubtitleWord> = Vec::with_capacity(words.len());
    let mut current = words[0].clone();

    for next in words.into_iter().skip(1) {
        let curr_trim = current.word.trim();
        let next_trim = next.word.trim();
        // Merge apostrophe-adjacent tokens
        if curr_trim.ends_with('\'') || curr_trim.ends_with('\u{2019}')
            || next_trim.starts_with('\'') || next_trim.starts_with('\u{2019}')
        {
            current.word = format!(
                "{}{}",
                current.word.trim_end(),
                next.word.trim_start()
            );
            current.end = next.end;
        } else {
            fixed.push(current);
            current = next;
        }
    }
    fixed.push(current);
    fixed
}

// ─── Bad-break detection (French) ────────────────────────────────────────────

fn is_bad_break(word1: &str, word2: &str) -> bool {
    let w1 = word1
        .to_lowercase()
        .trim_end_matches(['.', ',', '!', '?', ';', ':', '\u{2026}'])
        .trim()
        .to_string();
    let w2 = word2
        .to_lowercase()
        .trim_start_matches(['.', ',', '!', '?', ';', ':', '\u{2026}'])
        .trim()
        .to_string();

    if w1.is_empty() || w2.is_empty() {
        return false;
    }
    if word2.trim().starts_with(['.', ',', '!', '?', ';', ':']) {
        return true;
    }
    if word1.trim().ends_with('\'') || word1.trim().ends_with('\u{2019}') {
        return true;
    }
    if word2.trim().starts_with('\'') || word2.trim().starts_with('\u{2019}') {
        return true;
    }
    if word1.trim().ends_with(['.', '!', '?', ';', ':']) {
        return false;
    }

    const FORWARD: &[&str] = &[
        "le", "la", "les", "l", "un", "une", "des", "au", "aux", "du", "de", "d",
        "à", "en", "pour", "sur", "sous", "dans", "par", "avec", "sans", "chez",
        "vers", "avant", "après", "pendant", "depuis", "lors",
        "je", "j", "tu", "il", "elle", "on", "nous", "vous", "ils", "elles",
        "me", "m", "te", "t", "se", "s", "lui", "leur", "y",
        "ce", "cet", "cette", "ces", "c", "mon", "ton", "son", "ma", "ta", "sa",
        "mes", "tes", "ses", "notre", "votre", "nos", "vos", "leurs",
        "ne", "n", "très", "trop", "plus", "moins", "bien", "si",
        "est", "sont", "a", "ont", "suis", "es", "sommes", "êtes",
        "qui", "que", "qu", "quoi", "dont", "où", "quand", "comment", "pourquoi",
        "et", "ou", "mais", "donc", "or", "ni", "car",
        "parce", "tandis", "jusqu", "jusque", "afin",
        "tel", "telle", "tels", "telles",
        "quel", "quelle", "quels", "quelles",
    ];
    const BACKWARD: &[&str] = &["pas", "plus", "jamais", "rien", "personne"];

    if FORWARD.contains(&w1.as_str()) {
        return true;
    }
    if BACKWARD.contains(&w2.as_str()) {
        return true;
    }
    if (w1 == "parce" || w1 == "bien" || w1 == "alors" || w1 == "ainsi"
        || w1 == "avant" || w1 == "afin" || w1 == "y")
        && w2 == "que"
    {
        return true;
    }
    if w1 == "y" && w2 == "a" {
        return true;
    }
    false
}

// ─── Optimal line-split ───────────────────────────────────────────────────────

fn find_optimal_split(words: &[SubtitleWord], max_chars: u32) -> Option<usize> {
    if words.len() <= 1 {
        return None;
    }
    let mut best_split: Option<usize> = None;
    let mut best_score = f64::INFINITY;

    for i in 1..words.len() {
        let line1: String = words[..i].iter().map(|w| w.word.trim()).collect::<Vec<_>>().join(" ");
        let line2: String = words[i..].iter().map(|w| w.word.trim()).collect::<Vec<_>>().join(" ");

        let mut score = 0.0_f64;
        let mc = max_chars as i64;
        if line1.len() as i64 > mc {
            score += (line1.len() as i64 - mc) as f64 * 100.0;
        }
        if line2.len() as i64 > mc {
            score += (line2.len() as i64 - mc) as f64 * 100.0;
        }
        let diff = line2.len() as i64 - line1.len() as i64;
        if diff < 0 {
            score += (-diff) as f64 * 2.0;
        } else {
            score += diff as f64 * 0.5;
        }

        let prev_word = words[i - 1].word.trim();
        let next_word = words[i].word.trim();
        if is_bad_break(prev_word, next_word) {
            score += 10000.0;
        }
        if prev_word.ends_with(['.', ',', '!', '?', ';', ':']) {
            score -= 50.0;
        }

        if score < best_score {
            best_score = score;
            best_split = Some(i);
        }
    }
    best_split
}

// ─── Block creation ───────────────────────────────────────────────────────────

fn create_block(words: &[SubtitleWord], id: u32, max_chars: u32, max_lines: u32) -> SubtitleLine {
    let joined: String = words.iter().map(|w| w.word.trim()).collect::<Vec<_>>().join(" ");

    let text = if max_lines == 1 {
        joined
    } else if max_lines == 2 {
        if joined.len() > max_chars as usize {
            if let Some(split) = find_optimal_split(words, max_chars) {
                let l1: String = words[..split].iter().map(|w| w.word.trim()).collect::<Vec<_>>().join(" ");
                let l2: String = words[split..].iter().map(|w| w.word.trim()).collect::<Vec<_>>().join(" ");
                format!("{}\n{}", l1, l2)
            } else {
                joined
            }
        } else {
            joined
        }
    } else {
        // N lines: greedy wrap
        let mut lines_out: Vec<String> = Vec::new();
        let mut current_line: Vec<&str> = Vec::new();
        let mut current_len = 0usize;

        for w in words {
            let wt = w.word.trim();
            if !current_line.is_empty() && current_len + wt.len() > max_chars as usize {
                let prev = current_line.last().copied().unwrap_or("");
                if !is_bad_break(prev, wt) {
                    lines_out.push(current_line.join(" "));
                    current_line = vec![wt];
                    current_len = wt.len() + 1;
                    if lines_out.len() >= max_lines as usize {
                        break;
                    }
                    continue;
                }
            }
            current_len += wt.len() + 1;
            current_line.push(wt);
        }
        if !current_line.is_empty() && lines_out.len() < max_lines as usize {
            lines_out.push(current_line.join(" "));
        }
        lines_out.join("\n")
    };

    let start = words[0].start;
    let end = words[words.len() - 1].end;

    SubtitleLine {
        id,
        start,
        end: end.max(start + 0.1),
        text,
        words: Some(words.to_vec()),
    }
}

// ─── Main grouping function ───────────────────────────────────────────────────

pub fn group_transcription_into_lines(
    transcription: &TranscriptionResponse,
    max_chars: u32,
    max_lines: u32,
) -> Vec<SubtitleLine> {
    let mut raw_words: Vec<SubtitleWord> = Vec::new();

    if let Some(words) = &transcription.words {
        if !words.is_empty() {
            for w in words {
                let word = w.word.as_deref().unwrap_or("").trim().to_string();
                if word.is_empty() { continue; }
                raw_words.push(SubtitleWord {
                    word,
                    start: w.start.unwrap_or(0.0),
                    end: w.end.unwrap_or(0.0),
                });
            }
        }
    }

    if raw_words.is_empty() {
        if let Some(segments) = &transcription.segments {
            for seg in segments {
                if let Some(seg_words) = &seg.words {
                    if !seg_words.is_empty() {
                        for w in seg_words {
                            let word = w.word.as_deref().unwrap_or("").trim().to_string();
                            if word.is_empty() { continue; }
                            raw_words.push(SubtitleWord {
                                word,
                                start: w.start.unwrap_or(0.0),
                                end: w.end.unwrap_or(0.0),
                            });
                        }
                        continue;
                    }
                }
                // Fallback: distribute segment text
                if let Some(text) = &seg.text {
                    let seg_start = seg.start.unwrap_or(0.0);
                    let seg_end = seg.end.unwrap_or(seg_start + 1.0).max(seg_start + 0.1);
                    let tokens: Vec<&str> = text.split_whitespace().collect();
                    let total_chars: usize = tokens.iter().map(|t| t.len()).sum::<usize>().max(1);
                    let seg_dur = seg_end - seg_start;
                    let mut offset = 0.0;
                    for t in &tokens {
                        let dur = (t.len() as f64 / total_chars as f64) * seg_dur;
                        let w_start = seg_start + offset;
                        offset += dur;
                        raw_words.push(SubtitleWord {
                            word: t.to_string(),
                            start: w_start,
                            end: w_start + dur,
                        });
                    }
                }
            }
        }
    }

    if raw_words.is_empty() {
        if let Some(text) = &transcription.text {
            let mut offset = 0.0_f64;
            for t in text.split_whitespace() {
                raw_words.push(SubtitleWord {
                    word: t.to_string(),
                    start: offset,
                    end: offset + 0.4,
                });
                offset += 0.4;
            }
        }
    }

    if raw_words.is_empty() {
        return vec![];
    }

    let fixed = fix_tokenization(raw_words);
    let max_block_chars = (max_chars * max_lines) as usize;
    let mut blocks: Vec<SubtitleLine> = Vec::new();
    let mut current_block: Vec<SubtitleWord> = Vec::new();
    let mut current_len = 0usize;
    let mut block_index = 0u32;

    for word_obj in fixed {
        let wt = word_obj.word.trim().to_string();
        if wt.is_empty() { continue; }

        if !current_block.is_empty() && current_len + wt.len() > max_block_chars {
            let prev = current_block.last().map(|w| w.word.trim()).unwrap_or("");
            if !is_bad_break(prev, &wt) {
                blocks.push(create_block(&current_block, block_index, max_chars, max_lines));
                block_index += 1;
                current_block = Vec::new();
                current_len = 0;
            }
        }

        current_len += wt.len() + 1;
        let ends_sentence = wt.ends_with(['.', '!', '?']);
        current_block.push(word_obj);

        if ends_sentence {
            blocks.push(create_block(&current_block, block_index, max_chars, max_lines));
            block_index += 1;
            current_block = Vec::new();
            current_len = 0;
        }
    }

    if !current_block.is_empty() {
        blocks.push(create_block(&current_block, block_index, max_chars, max_lines));
    }

    normalize_and_fix_overlaps(&blocks)
}
