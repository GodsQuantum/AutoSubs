use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::{broadcast, RwLock};
use crate::config::Config;
use crate::subtitle::types::{Job, JobEvent, Preset, Settings, Workflow};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub jobs: Arc<DashMap<String, Job>>,
    pub tx: Arc<broadcast::Sender<JobEvent>>,
    pub settings: Arc<RwLock<Settings>>,
    pub presets: Arc<RwLock<Vec<Preset>>>,
    pub workflows: Arc<RwLock<Vec<Workflow>>>,
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let (tx, _) = broadcast::channel(256);

        let settings  = Self::load_json(&config.settings_file())
            .unwrap_or_default();
        let presets: Vec<Preset> = Self::load_json(&config.presets_file())
            .filter(|v: &Vec<Preset>| !v.is_empty())
            .unwrap_or_else(default_presets);
        let workflows: Vec<Workflow> = Self::load_json(&config.workflows_file())
            .unwrap_or_default();

        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("http client");

        Self {
            config: Arc::new(config),
            jobs: Arc::new(DashMap::new()),
            tx: Arc::new(tx),
            settings:  Arc::new(RwLock::new(settings)),
            presets:   Arc::new(RwLock::new(presets)),
            workflows: Arc::new(RwLock::new(workflows)),
            http_client,
        }
    }

    fn load_json<T: serde::de::DeserializeOwned>(path: &std::path::Path) -> Option<T> {
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

fn default_presets() -> Vec<Preset> {
    use crate::subtitle::types::AnimationStyle;
    vec![
        Preset::default(),
        Preset {
            name: "Hormozi".into(),
            size: 34.0,
            position_y: 55.0,
            highlight_color: "#ffe600".into(),
            font_family: "Montserrat".into(),
            outline_thickness: 3.5,
            shadow_thickness: Some(2.5),
            max_chars: 16,
            max_lines: 1,
            wobble_speed: 1.5,
            ..Preset::default()
        },
        Preset {
            name: "MrBeast".into(),
            animation_style: AnimationStyle::Bounce,
            size: 38.0,
            position_y: 72.0,
            highlight_color: "#22c55e".into(),
            font_family: "Anton".into(),
            outline_thickness: 4.0,
            shadow_thickness: Some(3.0),
            max_chars: 18,
            max_lines: 1,
            italic: true,
            ..Preset::default()
        },
        Preset {
            name: "Minimalist".into(),
            animation_style: AnimationStyle::Fade,
            size: 20.0,
            position_y: 88.0,
            highlight_color: "#ffffff".into(),
            font_family: "Liberation Sans".into(),
            outline_thickness: 1.2,
            shadow_thickness: Some(1.0),
            max_chars: 38,
            max_lines: 2,
            wobble_speed: 0.0,
            bold: false,
            ..Preset::default()
        },
    ]
}
