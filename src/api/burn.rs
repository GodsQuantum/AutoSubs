use crate::subtitle::types::Preset;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use std::path::PathBuf;
use crate::state::AppState;
use crate::subtitle::types::{SubtitleLine, SubtitleWord, JobStatus, JobEvent};
use crate::subtitle::normalize::normalize_and_fix_overlaps;
use crate::subtitle::srt::generate_srt_content;
use crate::subtitle::ass::generate_ass_content;
use crate::subtitle::group::group_transcription_into_lines;
use crate::pipeline::burn::generate_ass_and_burn;
use tokio::sync::mpsc;

#[derive(Deserialize)]
pub struct BurnRequest {
    id: String,
    #[serde(rename = "originalName")]
    original_name: String,
    lines: Vec<SubtitleLine>,
    #[serde(rename = "presetName")]
    preset_name: String,
}

pub async fn burn(
    State(state): State<AppState>,
    Json(req): Json<BurnRequest>,
) -> impl IntoResponse {
    let presets = state.presets.read().await.clone();
    let preset = presets.iter().find(|p| p.name == req.preset_name)
        .or_else(|| presets.iter().find(|p| p.name == "Défaut"))
        .cloned()
        .unwrap_or_default();

    let input_path = state.config.uploads_dir().join(&req.id);
    let safe_lines = normalize_and_fix_overlaps(&req.lines);

    let ext = std::path::Path::new(&req.original_name)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4");
    let base = req.original_name.replace(&format!(".{}", ext), "");
    
    let mut out_name = format!("{} - ST.{}", base, ext);

    let output_dir_cfg = state.config.data_dir.parent().unwrap().join("output");
    let output_dir = if output_dir_cfg.exists() { output_dir_cfg } else { state.config.data_dir.join("output") };
    let _ = tokio::fs::create_dir_all(&output_dir).await;

    let mut output_path = output_dir.join(&out_name);
    let mut counter = 1;
    while output_path.exists() {
        out_name = format!("{} - ST({}).{}", base, counter, ext);
        output_path = output_dir.join(&out_name);
        counter += 1;
    }

    let stem = output_path.file_stem().unwrap().to_string_lossy();
    let ass_path = output_dir.join(format!("{}.ass", stem));
    let srt_path = output_dir.join(format!("{}.srt", stem));
    let json_path = output_dir.join(format!("{}.json", stem));

    let _ = tokio::fs::write(&ass_path, generate_ass_content(&safe_lines, &preset)).await;
    let _ = tokio::fs::write(&srt_path, generate_srt_content(&safe_lines)).await;
    let _ = tokio::fs::write(&json_path, serde_json::to_string_pretty(&safe_lines).unwrap()).await;

    let s = state.clone();
    let job_id = req.id.clone();
    
    // Spawn burn task
    tokio::spawn(async move {
        if let Some(mut job) = s.jobs.get_mut(&job_id) {
            job.status = JobStatus::Burning;
        }
        
        let (tx, mut rx) = mpsc::channel(32);
        let s_clone = s.clone();
        let j_id = job_id.clone();
        
        // Progress forwarder
        tokio::spawn(async move {
            while let Some(pct) = rx.recv().await {
                if let Some(mut job) = s_clone.jobs.get_mut(&j_id) {
                    job.progress = Some(pct);
                }
                let _ = s_clone.tx.send(JobEvent { id: j_id.clone(), status: JobStatus::Burning, progress: Some(pct), error: None });
            }
        });

        let settings = s.settings.read().await.clone();
        let res = generate_ass_and_burn(
            &input_path.to_string_lossy(),
            &output_path.to_string_lossy(),
            &ass_path.to_string_lossy(),
            &s.config,
            &settings,
            &preset,
            Some(tx)
        ).await;

        if let Some(mut job) = s.jobs.get_mut(&job_id) {
            if res.is_ok() {
                job.status = JobStatus::Done;
                job.progress = Some(100);
                let _ = tokio::fs::remove_file(&input_path).await;
            } else {
                job.status = JobStatus::Error;
                job.error = Some(res.as_ref().unwrap_err().to_string());
            }
        }
        let _ = s.tx.send(JobEvent {
            id: job_id.clone(),
            status: if res.is_ok() { JobStatus::Done } else { JobStatus::Error },
            progress: if res.is_ok() { Some(100) } else { None },
            error: res.err().map(|e| e.to_string()),
        });
    });

    Json(serde_json::json!({ "success": true, "message": "Burn started" }))
}

#[derive(Deserialize)]
pub struct BatchBurnFile {
    id: String,
    #[serde(rename = "originalName")]
    original_name: String,
    lines: Option<Vec<SubtitleLine>>,
}

#[derive(Deserialize)]
pub struct BatchBurnRequest {
    files: Vec<BatchBurnFile>,
    #[serde(rename = "presetName")]
    preset_name: String,
    #[serde(rename = "globalOutroVideo")]
    global_outro_video: Option<String>,
}

pub async fn batch_burn(
    State(state): State<AppState>,
    Json(req): Json<BatchBurnRequest>,
) -> impl IntoResponse {
    let s = state.clone();
    tokio::spawn(async move {
        tracing::info!("Batch burn started for {} files", req.files.len());
        let presets = s.presets.read().await;
        let default_preset = Preset::default();
        let preset = presets.iter().find(|p| p.name == req.preset_name).unwrap_or(&default_preset).clone();
        drop(presets);
        let settings = s.settings.read().await.clone();

        for file in req.files {
            let job_id = file.id.clone();
            let input_path = s.config.uploads_dir().join(&file.original_name);
            let output_name = format!("{}_subbed.mp4", file.original_name.trim_end_matches(".mp4"));
            let output_path = s.config.jobs_dir().join(&output_name);
            let ass_path = s.config.jobs_dir().join(format!("{}.ass", job_id));

            if let Some(mut job) = s.jobs.get_mut(&job_id) {
                job.status = JobStatus::Burning;
                job.progress = Some(0);
            }

            if let Some(lines) = file.lines {
                let ass_content = crate::subtitle::ass::generate_ass_content(&lines, &preset);
                let _ = tokio::fs::write(&ass_path, ass_content).await;
                let res = generate_ass_and_burn(
                    &input_path.to_string_lossy(),
                    &output_path.to_string_lossy(),
                    &ass_path.to_string_lossy(),
                    &s.config,
                    &settings,
                    &preset,
                    None
                ).await;
                
                if let Some(mut job) = s.jobs.get_mut(&job_id) {
                    if res.is_ok() {
                        job.status = JobStatus::Done;
                        job.progress = Some(100);
                    } else {
                        job.status = JobStatus::Error;
                        job.error = Some(res.unwrap_err().to_string());
                    }
                }
            }
        }
    });

    Json(serde_json::json!({ "success": true, "message": "Batch burn started" }))
}

#[derive(Deserialize)]
pub struct RegroupRequest {
    words: Vec<SubtitleWord>,
    #[serde(rename = "maxChars")]
    max_chars: Option<u32>,
    #[serde(rename = "maxLines")]
    max_lines: Option<u32>,
}

pub async fn regroup(
    Json(req): Json<RegroupRequest>,
) -> impl IntoResponse {
    let max_c = req.max_chars.unwrap_or(25);
    let max_l = req.max_lines.unwrap_or(2);
    
    // Convert to mock TranscriptionResponse
    let raw = crate::subtitle::types::TranscriptionResponse {
        text: None,
        segments: None,
        words: Some(req.words.into_iter().map(|w| crate::subtitle::types::RawWord {
            word: Some(w.word),
            start: Some(w.start),
            end: Some(w.end),
        }).collect()),
    };

    let lines = group_transcription_into_lines(&raw, max_c, max_l);
    Json(serde_json::json!({ "lines": lines }))
}
