use autosubs::domain::{RawWord, TranscriptionResponse};
use autosubs::subtitle::group_transcription_into_lines;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct Case {
    label: String,
    words: Vec<Word>,
    default_max_chars: u32,
    default_max_lines: u32,
}
#[derive(Deserialize)]
struct Word {
    word: String,
    start: f64,
    end: f64,
}
#[derive(Deserialize)]
struct Fixture {
    jobs: Vec<Case>,
}

fn is_orphan_head(t: &str) -> bool {
    // A complete clause ending with punctuation is never an orphan.
    if t.trim().ends_with(['.', '!', '?', '…', '。', '！', '？']) {
        return false;
    }
    let clean = t
        .trim_matches(|x: char| !x.is_alphanumeric() && x != char::from(39u8) && x != '’')
        .to_lowercase();
    matches!(
        clean.as_str(),
        "le" | "la"
            | "les"
            | "un"
            | "une"
            | "je"
            | "tu"
            | "il"
            | "elle"
            | "nous"
            | "vous"
            | "ils"
            | "elles"
            | "the"
            | "a"
            | "an"
            | "he"
            | "she"
            | "we"
            | "they"
            | "will"
            | "has"
            | "have"
            | "had"
            | "was"
            | "were"
            | "are"
    )
}
fn main() -> anyhow::Result<()> {
    let p = std::env::args().nth(1).expect("fixture path argument");
    let raw: Fixture = serde_json::from_str(&std::fs::read_to_string(p)?)?;
    let mut total = (0usize, 0usize, 0usize, 0usize, 0usize);
    for case in raw.jobs {
        let inp = TranscriptionResponse {
            text: None,
            segments: None,
            words: Some(
                case.words
                    .iter()
                    .map(|w| RawWord {
                        word: Some(w.word.clone()),
                        start: Some(w.start),
                        end: Some(w.end),
                    })
                    .collect(),
            ),
        };
        let lines =
            group_transcription_into_lines(&inp, case.default_max_chars, case.default_max_lines);
        let mut n_orphans = 0usize;
        let mut n_cue_orphans = 0usize;
        let mut n_visual_orphans = 0usize;
        let mut lost = 0usize;
        for line in &lines {
            lost += line.words.as_ref().map_or(0, Vec::len);
            let chunks: Vec<&str> = line.text.lines().collect();
            for visual in chunks.windows(2) {
                let before = visual[0].split_whitespace().last().unwrap_or("");
                if is_orphan_head(before) {
                    n_orphans += 1;
                    n_visual_orphans += 1;
                    eprintln!("VISUAL {:?}: {:?} | {:?}", case.label, visual[0], visual[1]);
                }
            }
        }
        for pair in lines.windows(2) {
            let last = pair[0]
                .words
                .as_ref()
                .and_then(|w| w.last())
                .map(|w| w.word.as_str())
                .unwrap_or("");
            if is_orphan_head(last) {
                n_orphans += 1;
                n_cue_orphans += 1;
                eprintln!(
                    "CUE {:?}: {:?} | {:?}",
                    case.label, pair[0].text, pair[1].text
                );
            }
        }
        let mut changed = 0;
        for b in &lines {
            if let Some(words) = &b.words
                && (words.first().is_some_and(|w| w.start > b.start + 0.0001)
                    || words.last().is_some_and(|w| w.end < b.end - 0.0001))
            {
                changed += 1;
            }
        }
        total.0 += case.words.len();
        total.1 += lines.len();
        total.2 += n_orphans;
        total.3 += n_cue_orphans;
        total.4 += n_visual_orphans;
        let row = json!({"label":case.label,"words":case.words.len(),"subtitle_events":lines.len(),
            "orphan_count":n_orphans,"cue_orphans":n_cue_orphans,"visual_orphans":n_visual_orphans,
            "word_count_preserved":lost==case.words.len(),"endpoints_mismatch":changed,
            "max_chars":case.default_max_chars,"max_lines":case.default_max_lines});
        println!("{}", row);
    }
    println!(
        "{}",
        json!({"total":true,"words":total.0,"subtitle_events":total.1,
        "orphan_count":total.2,"cue_orphans":total.3,"visual_orphans":total.4})
    );
    Ok(())
}
