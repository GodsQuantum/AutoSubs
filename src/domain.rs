use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum FormatKey {
    #[default]
    Source,
    Portrait916,
    Landscape169,
    Square11,
    Portrait45,
    Custom,
}

impl FormatKey {
    pub const ALL: [Self; 6] = [
        Self::Source,
        Self::Portrait916,
        Self::Landscape169,
        Self::Square11,
        Self::Portrait45,
        Self::Custom,
    ];

    pub fn canonical_resolution(self) -> Option<(u32, u32)> {
        match self {
            Self::Source => None,
            Self::Portrait916 => Some((1080, 1920)),
            Self::Landscape169 => Some((1920, 1080)),
            Self::Square11 => Some((1080, 1080)),
            Self::Portrait45 => Some((1080, 1350)),
            Self::Custom => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    #[default]
    Preserve,
    Contain,
    Cover,
    Stretch,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormatProfile {
    #[serde(default)]
    pub key: FormatKey,
    #[serde(default)]
    pub fit: FitMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl Default for FormatProfile {
    fn default() -> Self {
        Self { key: FormatKey::Source, fit: FitMode::Preserve, width: None, height: None }
    }
}

impl FormatProfile {
    pub fn resolution(&self, source: Option<(u32, u32)>) -> Option<(u32, u32)> {
        match self.key {
            FormatKey::Source => source,
            FormatKey::Custom => match (self.width, self.height) {
                (Some(w), Some(h)) if w > 0 && h > 0 => Some((w, h)),
                _ => source,
            },
            key => key.canonical_resolution(),
        }
    }

    pub fn from_legacy_aspect_ratio(value: &str) -> Self {
        let key = match value.trim() {
            "9:16" => FormatKey::Portrait916,
            "16:9" => FormatKey::Landscape169,
            "1:1" => FormatKey::Square11,
            "4:5" => FormatKey::Portrait45,
            "source" => FormatKey::Source,
            _ => FormatKey::Source,
        };
        Self { key, fit: if key == FormatKey::Source { FitMode::Preserve } else { FitMode::Cover }, width: None, height: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleWord {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SubtitleLine {
    pub id: u32,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub words: Option<Vec<SubtitleWord>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
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

fn default_size() -> f64 { 26.0 }
fn default_50() -> f64 { 50.0 }
fn default_66() -> f64 { 66.0 }
fn default_white() -> String { "#ffffff".into() }
fn default_black() -> String { "#000000".into() }
fn default_highlight() -> String { "#00d2ff".into() }
fn default_font() -> String { "Roboto".into() }
fn default_true() -> bool { true }
fn default_outline() -> f64 { 2.5 }
fn default_border_style() -> u8 { 1 }
fn default_max_chars() -> u32 { 25 }
fn default_max_lines() -> u32 { 2 }
fn default_one() -> f64 { 1.0 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_id: Option<String>,
    #[serde(default, rename = "brand", skip_serializing_if = "Option::is_none")]
    pub legacy_brand: Option<String>,
    #[serde(default)]
    pub format: FormatProfile,
    #[serde(default, rename = "aspectRatio", skip_serializing_if = "Option::is_none")]
    pub legacy_aspect_ratio: Option<String>,
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
    #[serde(default)]
    pub shadow_thickness: Option<f64>,
    #[serde(default)]
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
    #[serde(default)]
    pub match_keywords: Option<String>,
    #[serde(default)]
    pub line_spacing: f64,
    #[serde(default)]
    pub outro_video: Option<String>,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Défaut".into(),
            brand_id: None,
            legacy_brand: None,
            format: FormatProfile::default(),
            legacy_aspect_ratio: None,
            animation_style: AnimationStyle::Pop,
            size: default_size(),
            position_x: default_50(),
            position_y: default_66(),
            base_color: default_white(),
            outline_color: default_black(),
            highlight_color: default_highlight(),
            font_family: default_font(),
            uppercase: true,
            outline_thickness: default_outline(),
            shadow_thickness: Some(1.5),
            shadow_color: Some(default_black()),
            border_style: default_border_style(),
            floating: false,
            max_chars: default_max_chars(),
            max_lines: default_max_lines(),
            wobble_speed: 1.0,
            bold: true,
            italic: false,
            match_keywords: None,
            line_spacing: 0.0,
            outro_video: None,
        }
    }
}

impl Preset {
    pub fn migrate(&mut self) -> bool {
        let mut changed = false;
        if self.id.trim().is_empty() {
            self.id = Uuid::new_v4().to_string();
            changed = true;
        }
        if let Some(legacy) = self.legacy_aspect_ratio.take() {
            self.format = FormatProfile::from_legacy_aspect_ratio(&legacy);
            changed = true;
        }
        if self.max_chars == 0 { self.max_chars = default_max_chars(); changed = true; }
        if self.max_lines == 0 { self.max_lines = default_max_lines(); changed = true; }
        changed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrandAssets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_outro: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub name: String,
    pub stored_file: String,
    #[serde(default)]
    pub mime: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub created_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Brand {
    #[serde(default)]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub assets: BrandAssets,
    #[serde(default)]
    pub preset_ids: Vec<String>,
    #[serde(default)]
    pub default_preset_by_format: BTreeMap<FormatKey, String>,
}

impl Brand {
    pub fn migrate(&mut self) -> bool {
        if self.id.trim().is_empty() {
            self.id = Uuid::new_v4().to_string();
            true
        } else { false }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub watch_dir: String,
    pub output_dir: String,
    #[serde(default, alias = "archivesDir")]
    pub archive_dir: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brand_id: Option<String>,
    #[serde(default, rename = "brand", skip_serializing_if = "Option::is_none")]
    pub legacy_brand: Option<String>,
    #[serde(default)]
    pub format: FormatProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset_name: Option<String>,
    #[serde(default)]
    pub enabled: bool,
}

impl Workflow {
    pub fn migrate(&mut self, presets: &[Preset]) -> bool {
        let mut changed = false;
        if self.id.trim().is_empty() {
            self.id = Uuid::new_v4().to_string();
            changed = true;
        }
        if self.archive_dir.is_empty() { self.archive_dir = "./archives".into(); changed = true; }
        if self.preset_id.is_none() {
            if let Some(name) = self.preset_name.as_deref() {
                if let Some(preset) = presets.iter().find(|p| p.name == name) {
                    self.preset_id = Some(preset.id.clone());
                    changed = true;
                }
            }
        }
        changed
    }
}

fn default_transcription_url() -> String {
    std::env::var("AUTOSUBS_TRANSCRIPTION_URL").unwrap_or_default()
}
fn default_model() -> String { std::env::var("AUTOSUBS_TRANSCRIPTION_MODEL").unwrap_or_else(|_| "large-v3".into()) }
fn default_lang() -> String { std::env::var("AUTOSUBS_TRANSCRIPTION_LANGUAGE").unwrap_or_else(|_| "fr".into()) }
fn default_llm_prompt() -> String {
    "Corrige uniquement l'orthographe, la grammaire et la ponctuation. Garde exactement le même nombre de blocs et leur ordre. Renvoie uniquement les blocs corrigés, un par ligne.".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub encoder: Encoder,
}

impl Default for Settings {
    fn default() -> Self {
        let local_url = std::env::var("AUTOSUBS_LOCAL_TRANSCRIPTION_URL")
            .or_else(|_| std::env::var("SPEACHES_URL")).unwrap_or_default();
        let local_enabled = std::env::var("AUTOSUBS_LOCAL_TRANSCRIPTION_ENABLED")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(!local_url.is_empty());
        let fallback = std::env::var("AUTOSUBS_LOCAL_FALLBACK_ENABLED")
            .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
            .unwrap_or(true);
        Self {
            transcription_url: default_transcription_url(),
            transcription_api_key: std::env::var("AUTOSUBS_TRANSCRIPTION_API_KEY").unwrap_or_default(),
            transcription_model: default_model(),
            language: default_lang(),
            local_transcription_enabled: local_enabled,
            local_fallback_enabled: fallback,
            local_transcription_url: local_url,
            local_transcription_api_key: std::env::var("AUTOSUBS_LOCAL_TRANSCRIPTION_API_KEY").unwrap_or_default(),
            local_transcription_model: std::env::var("AUTOSUBS_LOCAL_TRANSCRIPTION_MODEL").unwrap_or_else(|_| "large-v3".into()),
            llm_enabled: false, llm_endpoint: String::new(), llm_api_key: String::new(), llm_model: String::new(),
            llm_prompt: default_llm_prompt(), encoder: Encoder::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EncoderKind {
    #[default]
    Auto,
    Libx264,
    Libx265,
    NvencH264,
    NvencHevc,
    QsvH264,
    VaapiH264,
    AmfH264,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Encoder {
    #[serde(default)]
    pub kind: EncoderKind,
    #[serde(default = "default_quality")]
    pub quality: u8,
    #[serde(default = "default_encoder_preset")]
    pub preset: String,
}
fn default_quality() -> u8 { 20 }
fn default_encoder_preset() -> String { "medium".into() }
impl Default for Encoder {
    fn default() -> Self { Self { kind: EncoderKind::Auto, quality: 20, preset: "medium".into() } }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    #[default]
    Pending,
    Uploading,
    Probing,
    Transcribing,
    Correcting,
    Ready,
    Rendering,
    Done,
    Error,
    Cancelled,
    Interrupted,
}

impl JobStatus {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Uploading | Self::Probing | Self::Transcribing | Self::Correcting | Self::Rendering)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub original_name: String,
    pub status: JobStatus,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub lines: Option<Vec<SubtitleLine>>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub input_path: Option<PathBuf>,
    #[serde(default)]
    pub output_path: Option<PathBuf>,
    #[serde(default)]
    pub preset_id: Option<String>,
    #[serde(default)]
    pub format: FormatProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(default)]
    pub archive_after_success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attached_sidecar: Option<PathBuf>,
    #[serde(default)]
    pub created_at_ms: u128,
    #[serde(default)]
    pub updated_at_ms: u128,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct JobEvent {
    pub id: String,
    pub status: JobStatus,
    #[serde(default)]
    pub progress: Option<u8>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscriptionResponse {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub words: Option<Vec<RawWord>>,
    #[serde(default)]
    pub segments: Option<Vec<RawSegment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawWord {
    #[serde(default, alias = "text")]
    pub word: Option<String>,
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawSegment {
    #[serde(default)]
    pub start: Option<f64>,
    #[serde(default)]
    pub end: Option<f64>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub words: Option<Vec<RawWord>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_aspect_ratio_migrates_to_format() {
        let mut preset: Preset = serde_json::from_value(serde_json::json!({
            "name": "Vertical",
            "aspectRatio": "9:16"
        })).unwrap();
        assert!(preset.migrate());
        assert_eq!(preset.format.key, FormatKey::Portrait916);
        assert_eq!(preset.format.fit, FitMode::Cover);
        assert!(!preset.id.is_empty());
        assert!(preset.legacy_aspect_ratio.is_none());
    }

    #[test]
    fn source_is_the_new_format_default() {
        let profile = FormatProfile::default();
        assert_eq!(profile.key, FormatKey::Source);
        assert_eq!(profile.fit, FitMode::Preserve);
    }
}
