use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Config as NotifyConfig, EventKind};
use futures::executor::block_on;
use crate::state::AppState;
use crate::subtitle::types::{Workflow, Preset};
use crate::subtitle::group::group_transcription_into_lines;
use crate::subtitle::normalize::normalize_and_fix_overlaps;
use crate::subtitle::srt::{generate_srt_content, parse_srt_to_lines, parse_ass_to_lines};
use crate::subtitle::ass::generate_ass_content;
use crate::subtitle::llm::llm_correct_lines;

pub async fn start_all_watchers(state: AppState) {
    let workflows = state.workflows.read().await.clone();
    for wf in workflows {
        if wf.enabled {
            let s = state.clone();
            tokio::spawn(async move {
                if let Err(e) = start_workflow_watcher(wf, s).await {
                    tracing::error!("Watcher error: {}", e);
                }
            });
        }
    }
}

pub async fn start_workflow_watcher(workflow: Workflow, state: AppState) -> anyhow::Result<()> {
    let watch_dir = PathBuf::from(&workflow.watch_dir);
    if !watch_dir.exists() {
        tokio::fs::create_dir_all(&watch_dir).await?;
    }

    let (tx, mut rx) = mpsc::channel::<PathBuf>(64);
    let (std_tx, std_rx) = std::sync::mpsc::channel();

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        let _ = std_tx.send(path);
                    }
                }
            }
        },
        NotifyConfig::default(),
    )?;

    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

    // Bridge sync to async
    tokio::task::spawn_blocking(move || {
        while let Ok(path) = std_rx.recv() {
            let ext = path.extension().unwrap_or_default().to_string_lossy().to_lowercase();
            if ["mp4", "mov", "mkv", "avi", "webm"].contains(&ext.as_str()) {
                let _ = block_on(tx.send(path));
            }
        }
    });

    while let Some(path) = rx.recv().await {
        tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

        let filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let ext = path.extension().unwrap_or_default().to_string_lossy().to_string();

        tracing::info!("[Watchdog] Processing {} in workflow {}", filename, workflow.name);

        let srt_path = path.with_extension("srt");
        let ass_path = path.with_extension("ass");
        let json_path = path.with_extension("json");

        let presets = state.presets.read().await.clone();
        let preset = presets.iter().find(|p| p.name == workflow.preset_name)
            .or_else(|| presets.iter().find(|p| p.name == "Défaut"))
            .cloned()
            .unwrap_or_default();

        // Check companion files
        let mut lines = vec![];
        let mut used_companion = false;
        
        if ass_path.exists() {
            used_companion = true;
            if let Ok(c) = tokio::fs::read_to_string(&ass_path).await {
                lines = parse_ass_to_lines(&c);
            }
        } else if srt_path.exists() {
            used_companion = true;
            if let Ok(c) = tokio::fs::read_to_string(&srt_path).await {
                lines = parse_srt_to_lines(&c);
            }
        }

        if !used_companion {
            let audio_path = path.with_extension("wav");
            let audio_path_str = audio_path.to_string_lossy().to_string();
            
            if let Ok(()) = crate::pipeline::audio::extract_audio(&path.to_string_lossy(), &audio_path_str).await {
                let settings = state.settings.read().await.clone();
                if let Ok(t) = crate::pipeline::transcribe::transcribe_audio(&audio_path_str, &settings, &state.http_client).await {
                    lines = group_transcription_into_lines(&t, preset.max_chars, preset.max_lines);
                    lines = llm_correct_lines(lines, &settings, &state.http_client).await;
                }
                let _ = tokio::fs::remove_file(&audio_path).await;
            }
        }

        lines = normalize_and_fix_overlaps(&lines);

        let out_dir = PathBuf::from(&workflow.output_dir);
        if !out_dir.exists() {
            let _ = tokio::fs::create_dir_all(&out_dir).await;
        }

        let out_base = format!("{} - ST.{}", stem, ext);
        let out_path = out_dir.join(&out_base);
        
        let out_ass = out_dir.join(format!("{} - ST.ass", stem));
        let out_srt = out_dir.join(format!("{} - ST.srt", stem));
        
        let _ = tokio::fs::write(&out_ass, generate_ass_content(&lines, &preset)).await;
        let _ = tokio::fs::write(&out_srt, generate_srt_content(&lines)).await;

        let settings = state.settings.read().await.clone();
        let _ = crate::pipeline::burn::generate_ass_and_burn(
            &path.to_string_lossy(),
            &out_path.to_string_lossy(),
            &out_ass.to_string_lossy(),
            &state.config,
            &settings,
            &preset,
            None
        ).await;

        let arc_dir = PathBuf::from(&workflow.archives_dir);
        if !arc_dir.exists() {
            let _ = tokio::fs::create_dir_all(&arc_dir).await;
        }

        let _ = tokio::fs::rename(&path, arc_dir.join(path.file_name().unwrap())).await;
        if srt_path.exists() { let _ = tokio::fs::rename(&srt_path, arc_dir.join(srt_path.file_name().unwrap())).await; }
        if ass_path.exists() { let _ = tokio::fs::rename(&ass_path, arc_dir.join(ass_path.file_name().unwrap())).await; }
        if json_path.exists() { let _ = tokio::fs::rename(&json_path, arc_dir.join(json_path.file_name().unwrap())).await; }

        tracing::info!("[Watchdog] ✅ Finished {}", filename);
    }
    Ok(())
}
