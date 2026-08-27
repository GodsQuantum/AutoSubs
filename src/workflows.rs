use crate::{domain::{JobStatus, Workflow}, jobs, state::AppState};
use anyhow::{Result, anyhow};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}, time::{Duration, UNIX_EPOCH}};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub async fn run_supervisor(state: AppState, shutdown: CancellationToken) {
    let mut tick = tokio::time::interval(Duration::from_secs(3));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = shutdown.cancelled() => break,
            _ = tick.tick() => if let Err(error) = reconcile(&state).await { tracing::error!(%error, "workflow reconcile failed"); },
        }
    }
    for token in state.watcher_tokens.iter() { token.value().1.cancel(); }
}

pub async fn reconcile(state: &AppState) -> Result<()> {
    let workflows = state.workflows.read().await.clone();
    let enabled: HashSet<String> = workflows.iter().filter(|w| w.enabled).map(|w| w.id.clone()).collect();
    let existing: Vec<String> = state.watcher_tokens.iter().map(|e| e.key().clone()).collect();
    for id in existing {
        if !enabled.contains(&id) {
            if let Some((_, (_, token))) = state.watcher_tokens.remove(&id) { token.cancel(); }
        }
    }
    for workflow in workflows.into_iter().filter(|w| w.enabled) {
        let signature = serde_json::to_string(&workflow)?;
        if state.watcher_tokens.get(&workflow.id).is_some_and(|entry| entry.value().0 == signature) { continue; }
        if let Some((_, (_, token))) = state.watcher_tokens.remove(&workflow.id) { token.cancel(); }
        let token = CancellationToken::new();
        state.watcher_tokens.insert(workflow.id.clone(), (signature.clone(), token.clone()));
        let task_state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = watch_workflow(task_state.clone(), workflow.clone(), token.clone()).await {
                if !token.is_cancelled() { tracing::error!(workflow = %workflow.name, %error, "workflow stopped"); }
            }
            // Generation-safe cleanup: an old task cannot remove its replacement.
            let should_remove = task_state.watcher_tokens.get(&workflow.id).is_some_and(|entry| entry.value().0 == signature);
            if should_remove { task_state.watcher_tokens.remove(&workflow.id); }
        });
    }
    Ok(())
}

async fn watch_workflow(state: AppState, workflow: Workflow, token: CancellationToken) -> Result<()> {
    let watch_dir = PathBuf::from(&workflow.watch_dir);
    for (label, configured) in [("watch", &workflow.watch_dir), ("output", &workflow.output_dir), ("archive", &workflow.archive_dir)] {
        let path = PathBuf::from(configured);
        if !path.is_dir() { return Err(anyhow!("{label} directory does not exist: {}", path.display())); }
        if !state.config.path_is_allowed(&path) { return Err(anyhow!("{label} directory is outside allowed roots: {}", path.display())); }
    }
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<PathBuf>();
    let callback_tx = event_tx.clone();
    let mut watcher: Option<RecommendedWatcher> = notify::recommended_watcher(move |result: notify::Result<Event>| {
        if let Ok(event) = result { for path in event.paths { let _ = callback_tx.send(path); } }
    }).ok();
    let native_error = watcher.as_mut().and_then(|w| w.watch(&watch_dir, RecursiveMode::NonRecursive).err());
    if let Some(error) = native_error { tracing::warn!(%error, "native watcher unavailable; reconciliation scan remains active"); watcher = None; }
    let mut interval = tokio::time::interval(Duration::from_secs(state.config.workflow_scan_seconds.max(1)));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut attempted: HashMap<PathBuf, (u64, i128)> = HashMap::new();
    loop {
        tokio::select! {
            _ = token.cancelled() => break,
            Some(path) = event_rx.recv() => { try_process_path(&state, &workflow, &path, &token, &mut attempted).await; }
            _ = interval.tick() => {
                let mut entries = match tokio::fs::read_dir(&watch_dir).await { Ok(v) => v, Err(e) => { tracing::warn!(%e, "workflow scan failed"); continue; } };
                while let Ok(Some(entry)) = entries.next_entry().await { try_process_path(&state, &workflow, &entry.path(), &token, &mut attempted).await; }
            }
        }
    }
    drop(watcher);
    Ok(())
}

async fn try_process_path(state: &AppState, workflow: &Workflow, path: &Path, token: &CancellationToken, attempted: &mut HashMap<PathBuf, (u64,i128)>) {
    if token.is_cancelled() || !path.is_file() || is_sidecar_or_staging(path) { return; }
    let Ok((size, mtime)) = stable_fingerprint(path, Duration::from_millis(state.config.file_stability_ms.max(250)), token).await else { return; };
    if attempted.get(path).is_some_and(|fp| *fp == (size,mtime)) { return; }
    let path_text = path.to_string_lossy().into_owned();
    if state.db.workflow_seen(&workflow.id, &path_text, size, mtime).unwrap_or(false) { attempted.insert(path.to_path_buf(), (size,mtime)); return; }
    if !crate::media::probe::is_video(path, token).await { attempted.insert(path.to_path_buf(), (size,mtime)); return; }
    attempted.insert(path.to_path_buf(), (size,mtime));
    match process_workflow_file(state, workflow, path, token).await {
        Ok(()) => { if let Err(error) = state.db.mark_workflow_seen(&workflow.id, &path_text, size, mtime) { tracing::warn!(%error, "could not persist workflow dedupe fingerprint"); } }
        Err(error) => tracing::error!(workflow = %workflow.name, path = %path.display(), %error, "workflow processing failed; source was not archived by the watcher"),
    }
}

async fn process_workflow_file(state: &AppState, workflow: &Workflow, path: &Path, supervisor: &CancellationToken) -> Result<()> {
    let original_name = path.file_name().and_then(|v| v.to_str()).ok_or_else(|| anyhow!("invalid filename"))?.to_owned();
    let sidecar = ["ass","ssa","srt","json"].into_iter().map(|ext| path.with_extension(ext)).find(|p| p.exists());
    let job = jobs::create_job(state, original_name, path.to_path_buf(), sidecar, workflow.preset_id.clone(), Some(workflow))?;
    jobs::enqueue_prepare(state.clone(), job.id.clone())?;
    wait_for(&state.clone(), &job.id, supervisor, |status| matches!(status, JobStatus::Ready | JobStatus::Error | JobStatus::Cancelled)).await?;
    let prepared = jobs::get_job(state, &job.id)?;
    if prepared.status != JobStatus::Ready { return Err(anyhow!(prepared.error.unwrap_or_else(|| format!("job ended as {:?}", prepared.status)))); }
    jobs::enqueue_render(state.clone(), job.id.clone())?;
    wait_for(&state.clone(), &job.id, supervisor, |status| matches!(status, JobStatus::Done | JobStatus::Error | JobStatus::Cancelled)).await?;
    let rendered = jobs::get_job(state, &job.id)?;
    if rendered.status == JobStatus::Done { Ok(()) } else { Err(anyhow!(rendered.error.unwrap_or_else(|| format!("job ended as {:?}", rendered.status)))) }
}

async fn wait_for<F>(state: &AppState, id: &str, supervisor: &CancellationToken, done: F) -> Result<()>
where F: Fn(&JobStatus) -> bool {
    loop {
        tokio::select! {
            _ = supervisor.cancelled() => { let _ = jobs::cancel_job(state, id); return Err(anyhow!("workflow cancelled")); }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                let job = jobs::get_job(state, id)?; if done(&job.status) { return Ok(()); }
            }
        }
    }
}

async fn stable_fingerprint(path: &Path, delay: Duration, token: &CancellationToken) -> Result<(u64,i128)> {
    let a = tokio::fs::metadata(path).await?; if !a.is_file() { return Err(anyhow!("not a file")); }
    tokio::select! { _ = token.cancelled() => return Err(anyhow!("cancelled")), _ = tokio::time::sleep(delay) => {} }
    let b = tokio::fs::metadata(path).await?;
    let afp = fingerprint(&a); let bfp = fingerprint(&b); if afp != bfp { return Err(anyhow!("file is still changing")); } Ok(bfp)
}
fn fingerprint(meta: &std::fs::Metadata) -> (u64,i128) {
    let time = meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos() as i128).unwrap_or(0); (meta.len(), time)
}
fn is_sidecar_or_staging(path: &Path) -> bool {
    let name = path.file_name().and_then(|v| v.to_str()).unwrap_or_default().to_ascii_lowercase();
    if name.contains(".partial-") || name.ends_with(".uploading") || name.ends_with("_words.json") { return true; }
    matches!(path.extension().and_then(|v| v.to_str()).unwrap_or_default().to_ascii_lowercase().as_str(), "srt"|"ass"|"ssa"|"json")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn staging_and_sidecars_are_not_video_candidates() {
        for name in ["a.srt","a.ass","a.ssa","a.json",".x.partial-123.mp4","clip.uploading","clip_words.json"] { assert!(is_sidecar_or_staging(Path::new(name)), "{name}"); }
        assert!(!is_sidecar_or_staging(Path::new("clip.weirdcontainer")));
    }
}
