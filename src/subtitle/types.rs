use serde::{Deserialize, Serialize};

// ─── Subtitle primitives ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubtitleWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubtitleLine {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<SubtitleWord>>,
}

// ─── Animation & Preset ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AnimationStyle {
    #[default]
    Pop,
    Karaoke,
    Fade,
    SlideUp,
    Bounce,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AspectRatio {
    #[default]
    #[serde(rename = "9:16")]
    Portrait,
    #[serde(rename = "16:9")]
    Landscape,
    #[serde(rename = "1:1")]
    Square,
    #[serde(rename = "4:5")]
    Instagram,
}

impl AspectRatio {
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            AspectRatio::Portrait  => (1080, 1920),
            AspectRatio::Landscape => (1920, 1080),
            AspectRatio::Square    => (1080, 1080),
            AspectRatio::Instagram => (1080, 1350),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]

    pub brand: Option<String>,
    #[serde(default)]
    pub animation_style: AnimationStyle,
    #[serde(default = "default_size")]
    pub size: f64,
    #[serde(default = "default_50")]
    pub position_x: f64,
    #[serde(default = "default_66")]
    pub position_y: f64,
    #[serde(default = "default_white")]
    pub base_color: String,
    #[serde(default = "default_black")]
    pub outline_color: String,
    #[serde(default = "default_highlight")]
    pub highlight_color: String,
    #[serde(default = "default_font")]
    pub font_family: String,
    #[serde(default = "default_true")]
    pub uppercase: bool,
    #[serde(default = "default_outline")]
    pub outline_thickness: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_color: Option<String>,
    #[serde(default = "default_border_style")]
    pub border_style: u8,
    #[serde(default)]
    pub floating: bool,
    #[serde(default = "default_max_chars")]
    pub max_chars: u32,
    #[serde(default = "default_max_lines")]
    pub max_lines: u32,
    #[serde(default = "default_one")]
    pub wobble_speed: f64,
    #[serde(default = "default_true")]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_keywords: Option<String>,
    #[serde(default)]
    pub line_spacing: f64,
    #[serde(default)]
    pub aspect_ratio: AspectRatio,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outro_video: Option<String>,
}

fn default_size()        -> f64    { 26.0 }
fn default_50()          -> f64    { 50.0 }
fn default_66()          -> f64    { 66.0 }
fn default_white()       -> String { "#ffffff".into() }
fn default_black()       -> String { "#000000".into() }
fn default_highlight()   -> String { "#00d2ff".into() }
fn default_font()        -> String { "Roboto".into() }
fn default_true()        -> bool   { true }
fn default_outline()     -> f64    { 2.5 }
fn default_border_style()-> u8     { 1 }
fn default_max_chars()   -> u32    { 25 }
fn default_max_lines()   -> u32    { 2 }
fn default_one()         -> f64    { 1.0 }

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "Défaut".into(),
            animation_style: AnimationStyle::Pop,
            size: 26.0,
            position_x: 50.0,
            position_y: 66.0,
            base_color: "#ffffff".into(),
            outline_color: "#000000".into(),
            highlight_color: "#00d2ff".into(),
            font_family: "Roboto".into(),
            uppercase: true,
            outline_thickness: 2.5,
            shadow_thickness: Some(1.5),
            shadow_color: Some("#000000".into()),
            border_style: 1,
            floating: false,
            max_chars: 25,
            max_lines: 2,
            wobble_speed: 1.0,
            bold: true,
            italic: false,
            match_keywords: None,
            line_spacing: 0.0,
            aspect_ratio: AspectRatio::Portrait,
            outro_video: None,
            brand: None,
        }
    }
}

// ─── Settings ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(default = "default_transcription_url")]
    pub transcription_url: String,
    #[serde(default)]
    pub transcription_api_key: String,
    #[serde(default = "default_model")]
    pub transcription_model: String,
    #[serde(default = "default_lang")]
    pub language: String,
    #[serde(default)]
    pub local_transcription_enabled: bool,
    #[serde(default = "default_true")]
    pub local_fallback_enabled: bool,
    #[serde(default)]
    pub local_transcription_url: String,
    #[serde(default)]
    pub local_transcription_api_key: String,
    #[serde(default)]
    pub local_transcription_model: String,
    #[serde(default)]
    pub llm_enabled: bool,
    #[serde(default)]
    pub llm_endpoint: String,
    #[serde(default)]
    pub llm_api_key: String,
    #[serde(default)]
    pub llm_model: String,
    #[serde(default = "default_llm_prompt")]
    pub llm_prompt: String,
    /// "auto" | "nvenc" | "cpu"
    #[serde(default = "default_hw_accel")]
    pub hardware_accel: String,
}

fn default_transcription_url() -> String {
    std::env::var("SPEACHES_URL")
        .unwrap_or_else(|_| "http://speaches:8000/v1/audio/transcriptions".into())
}
fn default_model() -> String { "speaches-ai/faster-whisper-large-v3".into() }
fn default_lang()  -> String { "fr".into() }
fn default_hw_accel() -> String { "auto".into() }
fn default_llm_prompt() -> String {
    "Corrige l'orthographe, la grammaire et la ponctuation. Garde le même nombre exact de lignes. Renvoie uniquement le texte corrigé.".into()
}

impl Default for Settings {
    fn default() -> Self {
        serde_json::from_str("{}").unwrap()
    }
}

// ─── Workflow (multi-watch-folder) ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]

    pub brand: Option<String>,
    pub watch_dir: String,
    pub output_dir: String,
    pub archives_dir: String,
    pub preset_name: String,
    #[serde(default)]
    pub enabled: bool,
}

// ─── Job ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    #[default]
    Pending,
    Uploading,
    Transcribing,
    Ready,
    Burning,
    Done,
    Error,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub original_name: String,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<SubtitleLine>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Absolute path to the uploaded source video on disk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_path: Option<String>,
}

// ─── SSE events ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub id: String,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ─── Raw transcription response from Whisper ────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TranscriptionResponse {
    pub text: Option<String>,
    pub words: Option<Vec<RawWord>>,
    pub segments: Option<Vec<RawSegment>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawWord {
    #[serde(alias = "text")]
    pub word: Option<String>,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawSegment {
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub text: Option<String>,
    pub words: Option<Vec<RawWord>>,
}
