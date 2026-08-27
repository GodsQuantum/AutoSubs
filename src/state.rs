use crate::config::Config;
use crate::domain::{Brand, FormatKey, Job, JobEvent, JobStatus, Preset, Settings, Workflow};
use crate::media::render::EncoderCapabilities;
use crate::persistence::{atomic_write_json, load_json_or_default};
use anyhow::{Context, Result};
use dashmap::DashMap;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock, Semaphore};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub settings: Arc<RwLock<Settings>>,
    pub presets: Arc<RwLock<Vec<Preset>>>,
    pub brands: Arc<RwLock<Vec<Brand>>>,
    pub workflows: Arc<RwLock<Vec<Workflow>>>,
    pub jobs: Arc<DashMap<String, Job>>,
    pub job_tokens: Arc<DashMap<String, CancellationToken>>,
    pub watcher_tokens: Arc<DashMap<String, (String, CancellationToken)>>,
    pub events: broadcast::Sender<JobEvent>,
    pub http: reqwest::Client,
    pub encode_slots: Arc<Semaphore>,
    pub encoders: Arc<RwLock<EncoderCapabilities>>,
}

impl AppState {
    pub async fn load(config: Config) -> Result<Self> {
        config.init_dirs()?;
        let mut presets: Vec<Preset> = load_json_or_default(&config.presets_file())?;
        if presets.is_empty() { presets = default_presets(); }
        let mut preset_changed = false;
        for preset in &mut presets { preset_changed |= preset.migrate(); }

        let mut brands: Vec<Brand> = load_json_or_default(&config.brands_file())?;
        let mut brands_changed = false;
        for brand in &mut brands { brands_changed |= brand.migrate(); }

        // Migrate the previous free-text `brand` property into first-class Brand objects.
        let legacy_names: Vec<String> = presets.iter().filter_map(|p| p.legacy_brand.clone()).collect();
        for legacy_name in legacy_names {
            let brand_id = if let Some(brand) = brands.iter().find(|b| b.name.eq_ignore_ascii_case(&legacy_name)) {
                brand.id.clone()
            } else {
                let id = Uuid::new_v4().to_string();
                brands.push(Brand { id: id.clone(), name: legacy_name.clone(), description: String::new(), assets: Default::default(), preset_ids: Vec::new(), default_preset_by_format: BTreeMap::new() });
                brands_changed = true;
                id
            };
            for preset in presets.iter_mut().filter(|p| p.legacy_brand.as_deref() == Some(legacy_name.as_str())) {
                preset.brand_id = Some(brand_id.clone());
                preset.legacy_brand = None;
                preset_changed = true;
            }
        }

        // Rebuild brand preset membership without duplicates.
        for brand in &mut brands {
            let expected: Vec<String> = presets.iter().filter(|p| p.brand_id.as_deref() == Some(brand.id.as_str())).map(|p| p.id.clone()).collect();
            if brand.preset_ids != expected { brand.preset_ids = expected; brands_changed = true; }
            for key in FormatKey::ALL {
                if brand.default_preset_by_format.contains_key(&key) { continue; }
                if let Some(preset) = presets.iter().find(|p| p.brand_id.as_deref() == Some(brand.id.as_str()) && p.format.key == key) {
                    brand.default_preset_by_format.insert(key, preset.id.clone());
                    brands_changed = true;
                }
            }
        }

        let mut workflows: Vec<Workflow> = load_json_or_default(&config.workflows_file())?;
        let mut workflows_changed = false;
        for workflow in &mut workflows {
            workflows_changed |= workflow.migrate(&presets);
            if let Some(legacy) = workflow.legacy_brand.take() {
                if let Some(brand) = brands.iter().find(|b| b.name.eq_ignore_ascii_case(&legacy)) {
                    workflow.brand_id = Some(brand.id.clone());
                    workflows_changed = true;
                }
            }
        }

        if preset_changed || !config.presets_file().exists() { atomic_write_json(&config.presets_file(), &presets)?; }
        if brands_changed || !config.brands_file().exists() { atomic_write_json(&config.brands_file(), &brands)?; }
        if workflows_changed || !config.workflows_file().exists() { atomic_write_json(&config.workflows_file(), &workflows)?; }

        let settings: Settings = load_json_or_default(&config.settings_file())?;
        if !config.settings_file().exists() { atomic_write_json(&config.settings_file(), &settings)?; }

        let jobs = Arc::new(DashMap::new());
        let mut entries = tokio::fs::read_dir(config.jobs_dir()).await.context("read jobs dir")?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.path().extension().and_then(|v| v.to_str()) != Some("json") { continue; }
            let bytes = tokio::fs::read(entry.path()).await?;
            if let Ok(mut job) = serde_json::from_slice::<Job>(&bytes) {
                if job.status.is_active() {
                    job.status = JobStatus::Interrupted;
                    job.error = Some("AutoSubs restarted before this job completed".into());
                    crate::persistence::atomic_write_json(&entry.path(), &job)?;
                }
                jobs.insert(job.id.clone(), job);
            }
        }

        let encode_jobs = config.max_encode_jobs.max(1);
        let (events, _) = broadcast::channel(512);
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()?;

        Ok(Self {
            config: Arc::new(config),
            settings: Arc::new(RwLock::new(settings)),
            presets: Arc::new(RwLock::new(presets)),
            brands: Arc::new(RwLock::new(brands)),
            workflows: Arc::new(RwLock::new(workflows)),
            jobs,
            job_tokens: Arc::new(DashMap::new()),
            watcher_tokens: Arc::new(DashMap::new()),
            events,
            http,
            encode_slots: Arc::new(Semaphore::new(encode_jobs)),
            encoders: Arc::new(RwLock::new(EncoderCapabilities::default())),
        })
    }
}

fn default_presets() -> Vec<Preset> {
    let mut base = Preset::default();
    base.migrate();
    let mut hormone = Preset { name: "Hormozi".into(), size: 34.0, max_chars: 16, max_lines: 1, highlight_color: "#ffe600".into(), ..Preset::default() };
    hormone.migrate();
    let mut beast = Preset { name: "MrBeast".into(), animation_style: crate::domain::AnimationStyle::Bounce, size: 38.0, max_chars: 18, max_lines: 1, highlight_color: "#22c55e".into(), italic: true, ..Preset::default() };
    beast.migrate();
    vec![base, hormone, beast]
}
