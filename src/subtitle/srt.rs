use crate::domain::{SubtitleLine, SubtitleWord};
use crate::subtitle::normalize::{normalize_subtitles, NormalizeOptions};
use regex::Regex;
use std::collections::BTreeMap;
use std::sync::OnceLock;

static SRT_TIMECODE_RE: OnceLock<Regex> = OnceLock::new();
fn timecode_re() -> &'static Regex {
    SRT_TIMECODE_RE.get_or_init(|| Regex::new(
        r"(?m)^\s*(\d{1,3}:\d{2}:\d{2}[,.]\d{1,3})\s*-->\s*(\d{1,3}:\d{2}:\d{2}[,.]\d{1,3})(?:\s+.*)?$"
    ).unwrap())
}

pub fn parse_srt_time(value: &str) -> Option<f64> {
    let normalized = value.trim().replace('.', ",");
    let (hms, ms) = normalized.split_once(',')?;
    let mut parts = hms.split(':');
    let h: u64 = parts.next()?.parse().ok()?;
    let m: u64 = parts.next()?.parse().ok()?;
    let s: u64 = parts.next()?.parse().ok()?;
    let ms_digits = ms.trim();
    let millis: u64 = match ms_digits.len() {
        0 => 0,
        1 => ms_digits.parse::<u64>().ok()? * 100,
        2 => ms_digits.parse::<u64>().ok()? * 10,
        _ => ms_digits[..3.min(ms_digits.len())].parse().ok()?,
    };
    Some((h * 3600 + m * 60 + s) as f64 + millis as f64 / 1000.0)
}

pub fn format_srt_time(seconds: f64) -> String {
    let total_ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let h = total_ms / 3_600_000;
    let m = (total_ms / 60_000) % 60;
    let s = (total_ms / 1000) % 60;
    let ms = total_ms % 1000;
    format!("{h:02}:{m:02}:{s:02},{ms:03}")
}

pub fn parse_srt_to_lines(input: &str) -> Vec<SubtitleLine> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = Vec::new();
    let chunks: Vec<&str> = normalized.split("\n\n").collect();
    for chunk in chunks {
        let rows: Vec<&str> = chunk.lines().collect();
        if rows.len() < 2 { continue; }
        let time_idx = rows.iter().position(|row| timecode_re().is_match(row));
        let Some(time_idx) = time_idx else { continue; };
        let caps = timecode_re().captures(rows[time_idx]).unwrap();
        let (Some(start), Some(end)) = (parse_srt_time(&caps[1]), parse_srt_time(&caps[2])) else { continue; };
        let text = rows.iter().skip(time_idx + 1).copied().collect::<Vec<_>>().join("\n").trim().to_string();
        if text.is_empty() { continue; }
        lines.push(SubtitleLine { id: lines.len() as u32, start, end, text, words: None });
    }
    normalize_subtitles(&lines, NormalizeOptions::default()).lines
}

pub fn generate_srt_content(lines: &[SubtitleLine]) -> String {
    let normalized = normalize_subtitles(lines, NormalizeOptions::default()).lines;
    normalized.iter().enumerate().map(|(idx, line)| {
        format!("{}\n{} --> {}\n{}\n", idx + 1, format_srt_time(line.start), format_srt_time(line.end), line.text)
    }).collect::<Vec<_>>().join("\n")
}

pub fn parse_ass_time(value: &str) -> Option<f64> {
    let mut parts = value.trim().split(':');
    let h: u64 = parts.next()?.parse().ok()?;
    let m: u64 = parts.next()?.parse().ok()?;
    let sec: f64 = parts.next()?.parse().ok()?;
    Some(h as f64 * 3600.0 + m as f64 * 60.0 + sec)
}

fn strip_ass_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut depth = 0usize;
    for ch in input.chars() {
        match ch {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.replace("\\N", "\n").replace("\\n", "\n").trim().to_string()
}

#[derive(Debug)]
struct AutoSubsGroup {
    start: f64,
    end: f64,
    text: String,
}

pub fn parse_ass_to_lines(input: &str) -> Vec<SubtitleLine> {
    let Some(events_index) = input.find("[Events]") else { return Vec::new(); };
    let mut normal = Vec::new();
    let mut autosubs: BTreeMap<String, AutoSubsGroup> = BTreeMap::new();

    for row in input[events_index..].lines() {
        if !row.trim_start().starts_with("Dialogue:") { continue; }
        let payload = row.trim_start().trim_start_matches("Dialogue:").trim_start();
        let parts: Vec<&str> = payload.splitn(10, ',').collect();
        if parts.len() < 10 { continue; }
        let (Some(start), Some(end)) = (parse_ass_time(parts[1]), parse_ass_time(parts[2])) else { continue; };
        let name = parts[4].trim();
        let text = strip_ass_tags(parts[9]);
        if text.is_empty() { continue; }

        if name.starts_with("autosubs:") {
            autosubs.entry(name.to_string())
                .and_modify(|group| {
                    group.start = group.start.min(start);
                    group.end = group.end.max(end);
                    if group.text.is_empty() { group.text = text.clone(); }
                })
                .or_insert(AutoSubsGroup { start, end, text });
        } else {
            normal.push(SubtitleLine { id: 0, start, end, text, words: None });
        }
    }

    for group in autosubs.into_values() {
        normal.push(SubtitleLine { id: 0, start: group.start, end: group.end, text: group.text, words: None });
    }
    normalize_subtitles(&normal, NormalizeOptions::default()).lines
}

#[allow(dead_code)]
fn _keep_word_type_used(_: SubtitleWord) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srt_time_rounding_carries_into_next_second() {
        assert_eq!(format_srt_time(59.9996), "00:01:00,000");
    }

    #[test]
    fn parses_multiline_srt_without_requiring_numeric_index() {
        let srt = "00:00:01,000 --> 00:00:02,000\nBonjour\nle monde\n";
        let lines = parse_srt_to_lines(srt);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "Bonjour\nle monde");
    }

    #[test]
    fn groups_autosubs_pop_dialogues_by_name() {
        let ass = r#"[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:01.40,Default,autosubs:7,0,0,0,,{\c&H00FF00&}BONJOUR MONDE
Dialogue: 0,0:00:01.40,0:00:02.00,Default,autosubs:7,0,0,0,,BONJOUR {\c&H00FF00&}MONDE
"#;
        let lines = parse_ass_to_lines(ass);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text, "BONJOUR MONDE");
        assert!((lines[0].start - 1.0).abs() < 0.001);
        assert!((lines[0].end - 2.0).abs() < 0.001);
    }

    #[test]
    fn ass_text_with_commas_survives_splitn() {
        let ass = r#"[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:01.00,Default,,0,0,0,,Salut, monde, ça va ?
"#;
        let lines = parse_ass_to_lines(ass);
        assert_eq!(lines[0].text, "Salut, monde, ça va ?");
    }
}
