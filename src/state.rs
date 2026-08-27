use crate::{config::Config, db::Database, domain::{Brand, FormatKey, Job, JobEvent, JobStatus, Preset, Settings, Workflow}, media::render::EncoderCapabilities};
use anyhow::Result;
use dashmap::DashMap;
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::{RwLock, Semaphore, broadcast};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: Database,
    pub settings: Arc<RwLock<Settings>>,
    pub presets: Arc<RwLock<Vec<Preset>>>,
    pub brands: Arc<RwLock<Vec<Brand>>>,
    pub workflows: Arc<RwLock<Vec<Workflow>>>,
    pub jobs: Arc<DashMap<String, Job>>,
    pub job_tokens: Arc<DashMap<String, CancellationToken>>,
    pub watcher_tokens: Arc<DashMap<String, (String, CancellationToken)>>,
    pub events: broadcast::Sender<JobEvent>,
    pub http: reqwest::Client,
    pub render_slots: Arc<Semaphore>,
    pub transcription_slots: Arc<Semaphore>,
    pub active_job_slots: Arc<Semaphore>,
    pub encoders: Arc<RwLock<EncoderCapabilities>>,
}

impl AppState {
    pub async fn load(mut config: Config) -> Result<Self> {
        config.init_dirs()?;
        let db = Database::open(&config.db_file())?;
        let settings: Settings = db.get_singleton("settings")?.unwrap_or_default();
        if db.get_singleton::<Settings>("settings")?.is_none() { db.set_singleton("settings", &settings)?; }

        let mut presets: Vec<Preset> = db.list("preset")?;
        if presets.is_empty() {
            presets = default_presets();
            for preset in &presets { db.upsert("preset", &preset.id, preset)?; }
        } else {
            for preset in &mut presets { if preset.migrate() { db.upsert("preset", &preset.id, preset)?; } }
        }

        let mut brands: Vec<Brand> = db.list("brand")?;
        for brand in &mut brands { if brand.migrate() { db.upsert("brand", &brand.id, brand)?; } }
        migrate_legacy_brands(&db, &mut presets, &mut brands)?;

        let mut workflows: Vec<Workflow> = db.list("workflow")?;
        for workflow in &mut workflows { if workflow.migrate(&presets) { db.upsert("workflow", &workflow.id, workflow)?; } }

        let jobs = Arc::new(DashMap::new());
        for mut job in db.list::<Job>("job")? {
            if job.status.is_active() {
                job.status = JobStatus::Interrupted;
                job.error = Some("AutoSubs restarted before this job completed".into());
                db.upsert("job", &job.id, &job)?;
            }
            jobs.insert(job.id.clone(), job);
        }
        let (events, _) = broadcast::channel(1024);
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .user_agent(concat!("AutoSubs/", env!("CARGO_PKG_VERSION"))).build()?;
        let render = config.max_render_jobs.max(1); let transcribe = config.max_transcription_jobs.max(1); let active = config.max_queued_jobs.max(1);
        Ok(Self {
            config: Arc::new(config), db, settings: Arc::new(RwLock::new(settings)), presets: Arc::new(RwLock::new(presets)),
            brands: Arc::new(RwLock::new(brands)), workflows: Arc::new(RwLock::new(workflows)), jobs,
            job_tokens: Arc::new(DashMap::new()), watcher_tokens: Arc::new(DashMap::new()), events,
            http, render_slots: Arc::new(Semaphore::new(render)), transcription_slots: Arc::new(Semaphore::new(transcribe)),
            active_job_slots: Arc::new(Semaphore::new(active)), encoders: Arc::new(RwLock::new(EncoderCapabilities::default())),
        })
    }

    pub fn emit_job(&self, job: &Job) {
        let _ = self.events.send(JobEvent { id: job.id.clone(), status: job.status.clone(), progress: job.progress, error: job.error.clone() });
    }
}

fn default_presets() -> Vec<Preset> {
    let mut base = Preset::default(); base.name = "Default".into(); base.migrate();
    let mut clean = Preset { name: "Clean Pop".into(), size: 32.0, max_chars: 22, max_lines: 2, ..Preset::default() }; clean.migrate();
    let mut karaoke = Preset { name: "Karaoke".into(), animation_style: crate::domain::AnimationStyle::Karaoke, size: 30.0, max_chars: 24, max_lines: 2, ..Preset::default() }; karaoke.migrate();
    vec![base, clean, karaoke]
}

fn migrate_legacy_brands(db: &Database, presets: &mut [Preset], brands: &mut Vec<Brand>) -> Result<()> {
    let names: Vec<String> = presets.iter().filter_map(|p| p.legacy_brand.clone()).collect();
    for name in names {
        let brand_id = if let Some(brand) = brands.iter().find(|b| b.name.eq_ignore_ascii_case(&name)) { brand.id.clone() } else {
            let id = Uuid::new_v4().to_string();
            let brand = Brand { id: id.clone(), name: name.clone(), description: String::new(), assets: Default::default(), preset_ids: Vec::new(), default_preset_by_format: BTreeMap::new() };
            db.upsert("brand", &id, &brand)?; brands.push(brand); id
        };
        for preset in presets.iter_mut().filter(|p| p.legacy_brand.as_deref() == Some(name.as_str())) {
            preset.brand_id = Some(brand_id.clone()); preset.legacy_brand = None; db.upsert("preset", &preset.id, preset)?;
        }
    }
    for brand in brands.iter_mut() {
        brand.preset_ids = presets.iter().filter(|p| p.brand_id.as_deref() == Some(brand.id.as_str())).map(|p| p.id.clone()).collect();
        for key in FormatKey::ALL {
            if brand.default_preset_by_format.contains_key(&key) { continue; }
            if let Some(preset) = presets.iter().find(|p| p.brand_id.as_deref() == Some(brand.id.as_str()) && p.format.key == key) {
                brand.default_preset_by_format.insert(key, preset.id.clone());
            }
        }
        db.upsert("brand", &brand.id, brand)?;
    }
    Ok(())
}
