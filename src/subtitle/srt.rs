use crate::subtitle::normalize::normalize_and_fix_overlaps;
use crate::subtitle::types::SubtitleLine;
use std::collections::HashSet;

// ─── SRT time helpers ─────────────────────────────────────────────────────────

pub fn parse_srt_time(s: &str) -> f64 {
    // HH:MM:SS,mmm
    let s = s.trim();
    let (hms, ms_part) = s.split_once(',').unwrap_or((s, "0"));
    let parts: Vec<f64> = hms.split(':').map(|p| p.parse().unwrap_or(0.0)).collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (*h, *m, *s),
        [m, s]    => (0.0, *m, *s),
        [s]       => (0.0, 0.0, *s),
        _         => (0.0, 0.0, 0.0),
    };
    h * 3600.0 + m * 60.0 + sec + ms_part.parse::<f64>().unwrap_or(0.0) / 1000.0
}

pub fn format_srt_time(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let h = (s / 3600.0).floor() as u32;
    let m = ((s % 3600.0) / 60.0).floor() as u32;
    let sec = (s % 60.0).floor() as u32;
    let ms = ((s % 1.0) * 1000.0).round() as u32;
    format!("{:02}:{:02}:{:02},{:03}", h, m, sec, ms)
}

// ─── SRT parse / generate ─────────────────────────────────────────────────────

pub fn parse_srt_to_lines(srt: &str) -> Vec<SubtitleLine> {
    let normalized = srt.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<SubtitleLine> = Vec::new();
    let mut id = 0u32;

    for block in normalized.trim().split("\n\n") {
        let parts: Vec<&str> = block.splitn(3, '\n').collect();
        if parts.len() < 3 { continue; }
        let timecode = parts[1];
        let text = parts[2].trim();
        if let Some((start_str, end_str)) = timecode.split_once(" --> ") {
            let start = parse_srt_time(start_str);
            let end   = parse_srt_time(end_str);
            if !text.is_empty() {
                lines.push(SubtitleLine { id, start, end, text: text.to_string(), words: None });
                id += 1;
            }
        }
    }
    normalize_and_fix_overlaps(&lines)
}

pub fn generate_srt_content(lines: &[SubtitleLine]) -> String {
    let safe = normalize_and_fix_overlaps(lines);
    safe.iter()
        .enumerate()
        .map(|(i, l)| {
            format!(
                "{}\n{} --> {}\n{}\n",
                i + 1,
                format_srt_time(l.start),
                format_srt_time(l.end),
                l.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ─── ASS time helpers ─────────────────────────────────────────────────────────

pub fn parse_ass_time(s: &str) -> f64 {
    // H:MM:SS.cc
    let s = s.trim();
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    if parts.len() != 3 { return 0.0; }
    let h: f64 = parts[0].parse().unwrap_or(0.0);
    let m: f64 = parts[1].parse().unwrap_or(0.0);
    let sec: f64 = parts[2].parse().unwrap_or(0.0); // SS.cc
    h * 3600.0 + m * 60.0 + sec
}

// ─── ASS parse ────────────────────────────────────────────────────────────────

fn strip_ass_tags(s: &str) -> String {
    // Remove {tags} and replace \N with newline
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '{' => in_tag = true,
            '}' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("\\N", "\n").replace("\\n", "\n")
}

pub fn parse_ass_to_lines(ass: &str) -> Vec<SubtitleLine> {
    let events_start = match ass.find("[Events]") {
        Some(i) => i,
        None => return vec![],
    };
    let events_text = &ass[events_start..];

    let mut lines: Vec<SubtitleLine> = Vec::new();
    // Deduplicate pop-mode duplicate entries by (start_cs, end_cs, text)
    let mut seen: HashSet<(i64, i64, String)> = HashSet::new();
    let mut id = 0u32;

    for line in events_text.lines() {
        if !line.starts_with("Dialogue:") { continue; }
        let after_dialogue = &line[9..]; // skip "Dialogue:"
        let parts: Vec<&str> = after_dialogue.splitn(10, ',').collect();
        if parts.len() < 10 { continue; }
        let start_str = parts[1].trim();
        let end_str   = parts[2].trim();
        let raw_text  = parts[9..].join(",");
        let text = strip_ass_tags(&raw_text).trim().to_string();
        if text.is_empty() { continue; }

        let start = parse_ass_time(start_str);
        let end   = parse_ass_time(end_str);

        // Deduplicate by line boundary (not word boundary)
        let key = (
            (start * 100.0).round() as i64,
            (end   * 100.0).round() as i64,
            text.clone(),
        );
        if !seen.insert(key) { continue; }

        lines.push(SubtitleLine { id, start, end, text, words: None });
        id += 1;
    }
    normalize_and_fix_overlaps(&lines)
}
