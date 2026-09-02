use crate::domain::{SubtitleLine, SubtitleWord};
use crate::subtitle::normalize::{NormalizeOptions, normalize_subtitles};

fn parse_time(value: &str) -> Option<f64> {
    let value = value.trim().replace('.', ",");
    let (hms, ms) = value.split_once(',')?;
    let mut parts = hms.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    let ms: f64 = ms.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s + ms / 1000.0)
}

fn srt_time(seconds: f64) -> String {
    let total = (seconds.max(0.0) * 1000.0).round() as u64;
    let h = total / 3_600_000;
    let m = (total / 60_000) % 60;
    let s = (total / 1000) % 60;
    let ms = total % 1000;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

pub fn parse_srt_to_lines(text: &str) -> Vec<SubtitleLine> {
    let normalized = text.replace("\r\n", "\n");
    let mut lines = Vec::new();
    for block in normalized.split("\n\n") {
        let mut rows = block.lines().filter(|row| !row.trim().is_empty());
        let first = rows.next().unwrap_or_default();
        let timing = if first.contains("-->") {
            first
        } else {
            rows.next().unwrap_or_default()
        };
        let Some((start, end)) = timing.split_once("-->") else {
            continue;
        };
        let (Some(start), Some(end)) = (
            parse_time(start),
            parse_time(end.split_whitespace().next().unwrap_or_default()),
        ) else {
            continue;
        };
        let body = rows.collect::<Vec<_>>().join("\n").trim().to_string();
        if body.is_empty() {
            continue;
        }
        lines.push(SubtitleLine {
            id: lines.len() as u32,
            start,
            end,
            text: body,
            words: None,
        });
    }
    normalize_subtitles(&lines, NormalizeOptions::default()).lines
}

pub fn generate_srt_content(lines: &[SubtitleLine]) -> String {
    let lines = normalize_subtitles(lines, NormalizeOptions::default()).lines;
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            idx + 1,
            srt_time(line.start),
            srt_time(line.end),
            line.text
        ));
    }
    out
}

fn ass_time(value: &str) -> Option<f64> {
    let mut parts = value.trim().split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

fn strip_ass_tags(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '{' => in_tag = true,
            '}' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.replace("\\N", "\n")
        .replace("\\n", "\n")
        .replace("\\h", " ")
}

pub fn parse_ass_to_lines(text: &str) -> Vec<SubtitleLine> {
    let mut out: Vec<SubtitleLine> = Vec::new();
    for raw in text
        .lines()
        .filter_map(|line| line.strip_prefix("Dialogue:"))
    {
        let parts: Vec<&str> = raw.splitn(10, ',').collect();
        if parts.len() < 10 {
            continue;
        }
        let (Some(start), Some(end)) = (ass_time(parts[1]), ass_time(parts[2])) else {
            continue;
        };
        let text = strip_ass_tags(parts[9]).trim().to_string();
        if text.is_empty() {
            continue;
        }
        // Pop styles may emit several Dialogue rows for the same logical line. Coalesce identical timing neighborhoods/text.
        if let Some(existing) = out.iter_mut().find(|line| {
            line.text == text && (line.start - start).abs() < 0.08 && end <= line.end + 0.25
        }) {
            existing.start = existing.start.min(start);
            existing.end = existing.end.max(end);
            continue;
        }
        out.push(SubtitleLine {
            id: out.len() as u32,
            start,
            end,
            text,
            words: None,
        });
    }
    normalize_subtitles(&out, NormalizeOptions::default()).lines
}

pub fn lines_from_words(words: Vec<SubtitleWord>) -> Vec<SubtitleLine> {
    words
        .into_iter()
        .enumerate()
        .map(|(id, word)| SubtitleLine {
            id: id as u32,
            start: word.start,
            end: word.end,
            text: word.word.clone(),
            words: Some(vec![word]),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn srt_round_trip_preserves_text() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nBonjour\nmonde\n\n";
        let lines = parse_srt_to_lines(input);
        assert_eq!(lines[0].text, "Bonjour\nmonde");
        assert!(generate_srt_content(&lines).contains("Bonjour\nmonde"));
    }
    #[test]
    fn ass_parser_keeps_commas_in_text() {
        let input = "Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Bonjour, le monde";
        assert_eq!(parse_ass_to_lines(input)[0].text, "Bonjour, le monde");
    }
}
