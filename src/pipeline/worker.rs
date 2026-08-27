use std::path::PathBuf;
use crate::state::AppState;
use crate::subtitle::types::{JobStatus, JobEvent};
use crate::subtitle::group::group_transcription_into_lines;
use crate::subtitle::normalize::normalize_and_fix_overlaps;
use crate::subtitle::llm::llm_correct_lines;

pub async fn run_transcribe_job(
    job_id: String,
    input_path: PathBuf,
    _original_name: String,
    max_chars: u32,
    max_lines: u32,
    state: AppState,
) {
    if let Some(mut job) = state.jobs.get_mut(&job_id) {
        job.status = JobStatus::Transcribing;
        let _ = state.tx.send(JobEvent { id: job_id.clone(), status: JobStatus::Transcribing, progress: None, error: None });
    }

    let audio_path = input_path.with_extension("wav");
    let audio_path_str = audio_path.to_string_lossy().to_string();

    if let Err(e) = super::audio::extract_audio(&input_path.to_string_lossy(), &audio_path_str).await {
        tracing::error!("Audio extraction failed: {}", e);
        if let Some(mut job) = state.jobs.get_mut(&job_id) {
            job.status = JobStatus::Error;
            job.error = Some(e.to_string());
            let _ = state.tx.send(JobEvent { id: job_id.clone(), status: JobStatus::Error, progress: None, error: Some(e.to_string()) });
        }
        return;
    }

    let settings = state.settings.read().await.clone();
    let transcribe_result = super::transcribe::transcribe_audio(&audio_path_str, &settings, &state.http_client).await;
    
    // Clean up audio
    let _ = tokio::fs::remove_file(&audio_path).await;

    let transcription = match transcribe_result {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Transcription failed: {}", e);
            if let Some(mut job) = state.jobs.get_mut(&job_id) {
                job.status = JobStatus::Error;
                job.error = Some(e.to_string());
                let _ = state.tx.send(JobEvent { id: job_id.clone(), status: JobStatus::Error, progress: None, error: Some(e.to_string()) });
            }
            return;
        }
    };

    let lines = group_transcription_into_lines(&transcription, max_chars, max_lines);
    let mut corrected = llm_correct_lines(lines, &settings, &state.http_client).await;
    corrected = normalize_and_fix_overlaps(&corrected);

    if let Some(mut job) = state.jobs.get_mut(&job_id) {
        job.status = JobStatus::Ready;
        job.lines = Some(corrected.clone());
        let _ = state.tx.send(JobEvent { id: job_id.clone(), status: JobStatus::Ready, progress: None, error: None });
    }
}
