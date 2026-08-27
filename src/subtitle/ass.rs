use crate::subtitle::normalize::{normalize_and_fix_overlaps, normalize_line_words};
use crate::subtitle::types::{AnimationStyle, Preset, SubtitleLine};

// ─── Time formatting ──────────────────────────────────────────────────────────

pub fn format_ass_time(seconds: f64) -> String {
    let s = seconds.max(0.0);
    let h = (s / 3600.0).floor() as u32;
    let m = ((s % 3600.0) / 60.0).floor() as u32;
    let sec = (s % 60.0).floor() as u32;
    let cs = ((s % 1.0) * 100.0).floor() as u32;
    format!("{}:{:02}:{:02}.{:02}", h, m, sec, cs)
}

// ─── Color conversion ─────────────────────────────────────────────────────────

pub fn to_ass_color(hex: &str, alpha_percent: u8) -> String {
    let h = hex.trim_start_matches('#').trim();
    let parse_byte = |s: &str| u8::from_str_radix(s, 16).unwrap_or(255);

    if h.len() == 8 {
        // RRGGBBAA
        let r = parse_byte(&h[0..2]);
        let g = parse_byte(&h[2..4]);
        let b = parse_byte(&h[4..6]);
        let a = parse_byte(&h[6..8]);
        return format!("&H{:02X}{:02X}{:02X}{:02X}&", a, b, g, r);
    }
    if h.len() == 6 {
        let r = parse_byte(&h[0..2]);
        let g = parse_byte(&h[2..4]);
        let b = parse_byte(&h[4..6]);
        let a = (alpha_percent as f64 / 100.0 * 255.0).min(255.0).round() as u8;
        return format!("&H{:02X}{:02X}{:02X}{:02X}&", a, b, g, r);
    }
    "&H00FFFFFF&".to_string()
}

// ─── Float/wobble tag generator ───────────────────────────────────────────────

fn float_tags(dur_ms: i64, wobble_speed: f64) -> String {
    if wobble_speed <= 0.0 || dur_ms <= 0 {
        return String::new();
    }
    let cycle_ms = (1000.0 / wobble_speed).floor() as i64;
    let cycles = ((dur_ms as f64) / cycle_ms as f64).ceil() as i64;
    let mut out = String::new();
    for c in 0..cycles {
        let c_start = c * cycle_ms;
        if c_start >= dur_ms { break; }
        let c_mid1 = c_start + cycle_ms / 3;
        let c_mid2 = c_start + cycle_ms * 2 / 3;
        let c_end = ((c + 1) * cycle_ms).min(dur_ms);
        out.push_str(&format!(
            "{{\\t({},{},\\frz-1.5)\\t({},{},\\frz1.5)\\t({},{},\\frz0)}}",
            c_start, c_mid1, c_mid1, c_mid2, c_mid2, c_end
        ));
    }
    out
}

// ─── Main ASS generator ───────────────────────────────────────────────────────

pub fn generate_ass_content(lines: &[SubtitleLine], preset: &Preset) -> String {
    let safe_lines = normalize_and_fix_overlaps(lines);

    let primary   = to_ass_color(&preset.base_color, 0);
    let outline_c = to_ass_color(&preset.outline_color, 0);
    let highlight  = to_ass_color(&preset.highlight_color, 0);
    let shadow_c  = to_ass_color(
        preset.shadow_color.as_deref().unwrap_or("#000000"), 0,
    );

    let (play_res_x, play_res_y) = preset.aspect_ratio.resolution();
    let ass_x = ((preset.position_x / 100.0) * play_res_x as f64).round() as i32;
    let ass_y = ((preset.position_y / 100.0) * play_res_y as f64).round() as i32;

    let font_size = preset.size;
    let font_name = &preset.font_family;
    let outline_t = preset.outline_thickness;
    let shadow_t  = preset.shadow_thickness.unwrap_or(0.0);
    let border_s  = preset.border_style;
    let bold      = if preset.bold { -1i32 } else { 0 };
    let italic    = if preset.italic { -1i32 } else { 0 };
    let line_sp   = preset.line_spacing;

    let mut out = format!(
        "[Script Info]\nScriptType: v4.00+\nPlayResX: {play_res_x}\nPlayResY: {play_res_y}\nWrapStyle: 2\nScaledBorderAndShadow: yes\n\n\
        [V4+ Styles]\n\
        Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
        Style: Default,{font_name},{font_size},{primary},{highlight},{outline_c},{shadow_c},{bold},{italic},0,0,100,100,0,0,{border_s},{outline_t},{shadow_t},5,0,0,0,1\n\n\
        [Events]\n\
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    );

    for line in &safe_lines {
        let line_start = line.start;
        let line_end   = line.end;
        let dur_ms     = ((line_end - line_start) * 1000.0).floor() as i64;

        let visual_lines: Vec<&str> = line.text
            .split('\n')
            .filter(|l| !l.trim().is_empty())
            .collect();
        if visual_lines.is_empty() { continue; }

        // Compute word assignments per visual line
        struct VLine<'a> {
            text: &'a str,
            raw_tokens: Vec<&'a str>,
            words_only: Vec<&'a str>,
            word_start_index: usize,
        }

        let mut vlines: Vec<VLine> = Vec::new();
        let mut word_offset = 0usize;
        for vl_text in &visual_lines {
            let words_only: Vec<&str> = vl_text.split_whitespace().collect();
            let raw_tokens: Vec<&str> = vl_text.split(' ').collect(); // preserve spacing intent
            vlines.push(VLine {
                text: vl_text,
                raw_tokens,
                words_only: words_only.clone(),
                word_start_index: word_offset,
            });
            word_offset += words_only.len();
        }
        let total_words = word_offset;
        if total_words == 0 { continue; }

        let total_visual = vlines.len();
        let total_height = (total_visual.saturating_sub(1)) as f64 * (font_size + line_sp);
        let start_y = ass_y as f64 - total_height / 2.0;

        let words = match &line.words {
            Some(w) if w.len() == total_words => w.clone(),
            _ => normalize_line_words(line),
        };

        match preset.animation_style {
            AnimationStyle::Pop => {
                for i in 0..total_words {
                    let w_start = if i == 0 { line_start } else { words[i].start };
                    let w_end   = if i == total_words - 1 {
                        line_end
                    } else {
                        words[i + 1].start.max(w_start + 0.02)
                    };
                    let w_dur_ms = ((w_end - w_start) * 1000.0).floor() as i64;
                    let float_tag = if preset.floating {
                        float_tags(w_dur_ms, preset.wobble_speed)
                    } else {
                        String::new()
                    };

                    for (vl_idx, vl) in vlines.iter().enumerate() {
                        let curr_y = (start_y + vl_idx as f64 * (font_size + line_sp)).round() as i32;
                        let mut text = format!("{{\\pos({},{})}}{}",  ass_x, curr_y, float_tag);

                        let mut local_word_idx = 0usize;
                        for token in &vl.raw_tokens {
                            if token.trim().is_empty() {
                                text.push(' ');
                            } else {
                                let global = vl.word_start_index + local_word_idx;
                                let mut word_text = token.replace(['{', '}'], "");
                                if preset.uppercase { word_text = word_text.to_uppercase(); }
                                if global == i {
                                    text.push_str(&format!(
                                        "{{\\c{}\\fscx112\\fscy112}}{}{{\\c{}\\fscx100\\fscy100}}",
                                        highlight, word_text, primary
                                    ));
                                } else {
                                    text.push_str(&word_text);
                                }
                                local_word_idx += 1;
                            }
                        }
                        out.push_str(&format!(
                            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                            format_ass_time(w_start),
                            format_ass_time(w_end),
                            text.trim()
                        ));
                    }
                }
            }
            _ => {
                // Non-pop styles: one Dialogue per visual line
                let anim_tag = match preset.animation_style {
                    AnimationStyle::Fade   => "{\\fad(150,150)}".to_string(),
                    AnimationStyle::Bounce => "{\\t(0,120,\\fscy125\\fscx105)\\t(120,240,\\fscy100\\fscx100)}".to_string(),
                    _ => String::new(),
                };

                let float_tag = if preset.floating {
                    float_tags(dur_ms, preset.wobble_speed)
                } else {
                    String::new()
                };

                let mut karaoke_delay_ms = 0i64;

                for (vl_idx, vl) in vlines.iter().enumerate() {
                    let curr_y = (start_y + vl_idx as f64 * (font_size + line_sp)).round() as i32;

                    let line_anim = match preset.animation_style {
                        AnimationStyle::SlideUp => format!(
                            "{{\\move({},{},{},{},0,180)}}",
                            ass_x, curr_y + 25, ass_x, curr_y
                        ),
                        _ => anim_tag.clone(),
                    };

                    let mut text = format!("{{\\pos({},{})}}{}{}",
                        ass_x, curr_y, float_tag, line_anim);

                    match preset.animation_style {
                        AnimationStyle::Karaoke => {
                            if karaoke_delay_ms > 0 {
                                text.push_str(&format!("{{\\k{}}}", karaoke_delay_ms / 10));
                            }
                            let mut local_word_idx = 0usize;
                            for token in &vl.raw_tokens {
                                if token.trim().is_empty() {
                                    text.push(' ');
                                } else {
                                    let global = vl.word_start_index + local_word_idx;
                                    let w_obj = words.get(global);
                                    let k_dur = w_obj.map(|w| {
                                        ((w.end - w.start) * 100.0).max(0.5).floor() as i64
                                    }).unwrap_or(50);
                                    let mut word_text = token.replace(['{', '}'], "");
                                    if preset.uppercase { word_text = word_text.to_uppercase(); }
                                    text.push_str(&format!("{{\\k{}}}{}", k_dur, word_text));
                                    karaoke_delay_ms += k_dur * 10;
                                    local_word_idx += 1;
                                }
                            }
                        }
                        _ => {
                            for token in &vl.raw_tokens {
                                let mut t = token.replace(['{', '}'], "");
                                if preset.uppercase { t = t.to_uppercase(); }
                                text.push_str(&t);
                            }
                        }
                    }

                    out.push_str(&format!(
                        "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
                        format_ass_time(line_start),
                        format_ass_time(line_end),
                        text.trim()
                    ));
                }
            }
        }
    }
    out
}
