use crate::domain::{AnimationStyle, Preset, SubtitleLine};
use crate::subtitle::normalize::{NormalizeOptions, normalize_subtitles};

pub fn format_ass_time(seconds: f64) -> String {
    let total_cs = (seconds.max(0.0) * 100.0).round() as u64;
    let h = total_cs / 360_000;
    let m = (total_cs / 6_000) % 60;
    let s = (total_cs / 100) % 60;
    let cs = total_cs % 100;
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}

pub fn to_ass_color(hex: &str, alpha_percent: u8) -> String {
    let value = hex.trim().trim_start_matches('#');
    fn byte(value: &str) -> Option<u8> {
        u8::from_str_radix(value, 16).ok()
    }
    match value.len() {
        6 => {
            let (Some(r), Some(g), Some(b)) =
                (byte(&value[0..2]), byte(&value[2..4]), byte(&value[4..6]))
            else {
                return "&H00FFFFFF&".into();
            };
            let alpha = ((alpha_percent.min(100) as f64 / 100.0) * 255.0).round() as u8;
            format!("&H{alpha:02X}{b:02X}{g:02X}{r:02X}&")
        }
        8 => {
            let (Some(r), Some(g), Some(b), Some(a)) = (
                byte(&value[0..2]),
                byte(&value[2..4]),
                byte(&value[4..6]),
                byte(&value[6..8]),
            ) else {
                return "&H00FFFFFF&".into();
            };
            format!("&H{a:02X}{b:02X}{g:02X}{r:02X}&")
        }
        _ => "&H00FFFFFF&".into(),
    }
}

fn safe_text(value: &str, uppercase: bool) -> String {
    let escaped = value.replace('{', "﹛").replace('}', "﹜");
    if uppercase {
        escaped.to_uppercase()
    } else {
        escaped
    }
}

fn float_tags(duration_ms: i64, speed: f64) -> String {
    if duration_ms <= 0 || speed <= 0.0 {
        return String::new();
    }
    let cycle = (1000.0 / speed).round().max(100.0) as i64;
    let mut tags = String::new();
    let mut start = 0;
    while start < duration_ms {
        let one = (start + cycle / 3).min(duration_ms);
        let two = (start + cycle * 2 / 3).min(duration_ms);
        let end = (start + cycle).min(duration_ms);
        tags.push_str(&format!(
            "{{\\t({start},{one},\\frz-1.5)\\t({one},{two},\\frz1.5)\\t({two},{end},\\frz0)}}"
        ));
        start += cycle;
    }
    tags
}

pub fn generate_ass_content(
    lines: &[SubtitleLine],
    preset: &Preset,
    source_resolution: Option<(u32, u32)>,
) -> String {
    let normalized = normalize_subtitles(lines, NormalizeOptions::default()).lines;
    let (play_x, play_y) = preset
        .format
        .resolution(source_resolution)
        .unwrap_or((1920, 1080));
    let x = ((preset.position_x.clamp(0.0, 100.0) / 100.0) * play_x as f64).round() as i32;
    let y = ((preset.position_y.clamp(0.0, 100.0) / 100.0) * play_y as f64).round() as i32;
    let primary = to_ass_color(&preset.base_color, 0);
    let outline = to_ass_color(&preset.outline_color, 0);
    let highlight = to_ass_color(&preset.highlight_color, 0);
    let shadow = to_ass_color(preset.shadow_color.as_deref().unwrap_or("#000000"), 0);
    let bold = if preset.bold { -1 } else { 0 };
    let italic = if preset.italic { -1 } else { 0 };
    let shadow_size = preset.shadow_thickness.unwrap_or(0.0);

    let mut out = format!(
        r#"[Script Info]
ScriptType: v4.00+
PlayResX: {play_x}
PlayResY: {play_y}
WrapStyle: 2
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,{font},{size},{primary},{highlight},{outline},{shadow},{bold},{italic},0,0,100,100,0,0,{border},{outline_size},{shadow_size},5,0,0,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#,
        font = preset.font_family,
        size = preset.size,
        border = preset.border_style,
        outline_size = preset.outline_thickness,
    );

    for line in normalized {
        let visual_lines: Vec<&str> = line
            .text
            .split('\n')
            .filter(|v| !v.trim().is_empty())
            .collect();
        if visual_lines.is_empty() {
            continue;
        }
        let total_words = visual_lines
            .iter()
            .map(|v| v.split_whitespace().count())
            .sum::<usize>();
        if total_words == 0 {
            continue;
        }
        let words = line.words.clone().unwrap_or_default();
        let total_height =
            visual_lines.len().saturating_sub(1) as f64 * (preset.size + preset.line_spacing);
        let start_y = y as f64 - total_height / 2.0;
        let name = format!("autosubs:{}", line.id);

        match preset.animation_style {
            AnimationStyle::Pop => {
                for active in 0..total_words {
                    let start = if active == 0 {
                        line.start
                    } else {
                        words.get(active).map(|w| w.start).unwrap_or(line.start)
                    };
                    let end = if active + 1 >= total_words {
                        line.end
                    } else {
                        words
                            .get(active + 1)
                            .map(|w| w.start)
                            .unwrap_or(line.end)
                            .max(start + 0.02)
                    };
                    let float = if preset.floating {
                        float_tags(((end - start) * 1000.0) as i64, preset.wobble_speed)
                    } else {
                        String::new()
                    };
                    let mut rendered = format!("{{\\q2\\pos({x},{y})}}{float}");
                    let mut global_index = 0usize;
                    for (visual_index, visual) in visual_lines.iter().enumerate() {
                        if visual_index > 0 {
                            rendered.push_str("\\N");
                        }
                        for (word_index, token) in visual.split_whitespace().enumerate() {
                            if word_index > 0 {
                                rendered.push(' ');
                            }
                            let token = safe_text(token, preset.uppercase);
                            if global_index == active {
                                rendered.push_str(&format!("{{\\c{highlight}\\fscx112\\fscy112}}{token}{{\\c{primary}\\fscx100\\fscy100}}"));
                            } else {
                                rendered.push_str(&token);
                            }
                            global_index += 1;
                        }
                    }
                    out.push_str(&format!(
                        "Dialogue: 0,{},{},Default,{name},0,0,0,,{}\n",
                        format_ass_time(start),
                        format_ass_time(end),
                        rendered
                    ));
                }
            }
            _ => {
                let duration_ms = ((line.end - line.start) * 1000.0).round() as i64;
                let float = if preset.floating {
                    float_tags(duration_ms, preset.wobble_speed)
                } else {
                    String::new()
                };
                let curr_y = start_y.round() as i32;
                let (position, animation) = match preset.animation_style {
                    AnimationStyle::Fade => (
                        format!("{{\\q2\\pos({x},{curr_y})}}"),
                        "{\\fad(150,150)}".to_string(),
                    ),
                    AnimationStyle::Bounce => (
                        format!("{{\\q2\\pos({x},{curr_y})}}"),
                        "{\\t(0,120,\\fscy125\\fscx105)\\t(120,240,\\fscy100\\fscx100)}"
                            .to_string(),
                    ),
                    AnimationStyle::SlideUp => (
                        String::new(),
                        format!("{{\\q2\\move({x},{},{x},{curr_y},0,180)}}", curr_y + 25),
                    ),
                    _ => (format!("{{\\q2\\pos({x},{curr_y})}}"), String::new()),
                };
                let mut rendered = format!("{position}{float}{animation}");
                let mut global_word = 0usize;
                for (line_index, visual) in visual_lines.iter().enumerate() {
                    if line_index > 0 {
                        rendered.push_str("\\N");
                    }
                    if preset.animation_style == AnimationStyle::Karaoke {
                        for (idx, token) in visual.split_whitespace().enumerate() {
                            if idx > 0 {
                                rendered.push(' ');
                            }
                            let centiseconds = words
                                .get(global_word)
                                .map(|w| ((w.end - w.start) * 100.0).round().max(1.0) as u64)
                                .unwrap_or(10);
                            rendered.push_str(&format!(
                                "{{\\k{centiseconds}}}{}",
                                safe_text(token, preset.uppercase)
                            ));
                            global_word += 1;
                        }
                    } else {
                        rendered.push_str(&safe_text(visual, preset.uppercase));
                    }
                }
                out.push_str(&format!(
                    "Dialogue: 0,{},{},Default,{name},0,0,0,,{}\n",
                    format_ass_time(line.start),
                    format_ass_time(line.end),
                    rendered
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FitMode, FormatKey, FormatProfile};

    fn sample_line() -> SubtitleLine {
        SubtitleLine {
            id: 4,
            start: 1.0,
            end: 2.0,
            text: "Bonjour monde".into(),
            words: None,
        }
    }

    #[test]
    fn source_profile_uses_probed_resolution() {
        let preset = Preset {
            format: FormatProfile {
                key: FormatKey::Source,
                fit: FitMode::Preserve,
                width: None,
                height: None,
            },
            ..Preset::default()
        };
        let ass = generate_ass_content(&[sample_line()], &preset, Some((3840, 2160)));
        assert!(ass.contains("PlayResX: 3840"));
        assert!(ass.contains("PlayResY: 2160"));
    }

    #[test]
    fn square_profile_uses_square_playres() {
        let preset = Preset {
            format: FormatProfile {
                key: FormatKey::Square11,
                fit: FitMode::Cover,
                width: None,
                height: None,
            },
            ..Preset::default()
        };
        let ass = generate_ass_content(&[sample_line()], &preset, Some((1920, 1080)));
        assert!(ass.contains("PlayResX: 1080"));
        assert!(ass.contains("PlayResY: 1080"));
    }

    #[test]
    fn slide_up_uses_move_without_conflicting_pos_tag() {
        let preset = Preset {
            animation_style: AnimationStyle::SlideUp,
            ..Preset::default()
        };
        let ass = generate_ass_content(&[sample_line()], &preset, Some((1920, 1080)));
        let dialogue = ass
            .lines()
            .find(|line| line.starts_with("Dialogue:"))
            .unwrap();
        assert!(dialogue.contains("\\move("));
        assert!(!dialogue.contains("\\pos("));
    }

    #[test]
    fn generated_dialogues_carry_stable_autosubs_name() {
        let ass = generate_ass_content(&[sample_line()], &Preset::default(), Some((1080, 1920)));
        assert!(ass.contains("Default,autosubs:0,")); // normalization reindexes line IDs
    }

    #[test]
    fn two_line_dialogue_has_explicit_lines_and_disables_implicit_wrap() {
        let line = SubtitleLine {
            text: "Une ligne\nDeux lignes".into(),
            ..sample_line()
        };
        let ass = generate_ass_content(
            &[line],
            &Preset {
                uppercase: false,
                ..Preset::default()
            },
            Some((1920, 1080)),
        );
        let dialogue = ass
            .lines()
            .find(|line| line.starts_with("Dialogue:"))
            .unwrap();
        assert!(dialogue.contains("\\q2"));
        assert!(
            dialogue.contains("\\N") && dialogue.contains("Deux lignes"),
            "{dialogue}"
        );
        assert_eq!(dialogue.matches("\\N").count(), 1);
    }
}
