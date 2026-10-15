//! Non-destructive multi-range video cutter. Exact boundaries via trim/atrim + concat.
//! Render jobs run server-side and expose polling status (no reverse-proxy timeout).
use crate::{
    db::Database,
    domain::Job,
    error::{AppError, AppResult},
    jobs,
    media::probe::probe_media,
    state::AppState,
};
use axum::{
    Json,
    body::Body,
    extract::{Path, State},
    http::{StatusCode, header},
    response::Response,
};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use tokio::{fs, process::Command, sync::Semaphore};
use tokio_util::{io::ReaderStream, sync::CancellationToken};
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeepRange {
    pub start: f64,
    pub end: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutRequest {
    pub keep: Vec<KeepRange>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutState {
    pub id: String,
    pub status: String,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

fn load_cut(db: &Database, id: &str) -> anyhow::Result<Option<CutState>> {
    let mut saved = db.get::<CutState>("cut", id)?;
    if let Some(item) = saved.as_mut()
        && matches!(item.status.as_str(), "queued" | "running")
    {
        item.status = "interrupted".into();
        item.error = Some("Le serveur a redémarré pendant ce découpage. Relancez Cut.".into());
    }
    Ok(saved)
}

fn lookup_cut(state: &AppState, id: &str) -> AppResult<CutState> {
    if Uuid::parse_str(id).is_err() {
        return Err(AppError::NotFound("unknown cut id".into()));
    }
    if let Some(item) = cuts().get(id) {
        return Ok(item.clone());
    }
    load_cut(&state.db, id)
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("cut not found".into()))
}

static CUTS: OnceLock<DashMap<String, CutState>> = OnceLock::new();
static CUT_SLOTS: OnceLock<Arc<Semaphore>> = OnceLock::new();

fn cuts() -> &'static DashMap<String, CutState> {
    CUTS.get_or_init(DashMap::new)
}
fn slots() -> Arc<Semaphore> {
    CUT_SLOTS
        .get_or_init(|| Arc::new(Semaphore::new(1)))
        .clone()
}

fn validate(ranges: &[KeepRange], duration: f64) -> AppResult<()> {
    if !duration.is_finite() || duration <= 0.0 {
        return Err(AppError::BadRequest("source duration is invalid".into()));
    }
    if ranges.is_empty() || ranges.len() > 32 {
        return Err(AppError::BadRequest(
            "provide between 1 and 32 keep ranges".into(),
        ));
    }
    let mut previous_end = 0.0;
    for (i, r) in ranges.iter().enumerate() {
        if !r.start.is_finite()
            || !r.end.is_finite()
            || r.start < 0.0
            || r.end <= r.start + 0.04
            || r.end > duration + 0.01
            || (i > 0 && r.start < previous_end)
        {
            return Err(AppError::BadRequest(format!(
                "invalid or overlapping range #{i}"
            )));
        }
        previous_end = r.end;
    }
    Ok(())
}

fn filter_graph(ranges: &[KeepRange], has_audio: bool) -> String {
    let mut nodes = Vec::new();
    for (i, r) in ranges.iter().enumerate() {
        nodes.push(format!(
            "[0:v:0]trim=start={:.6}:end={:.6},setpts=PTS-STARTPTS[v{i}]",
            r.start, r.end
        ));
        if has_audio {
            nodes.push(format!(
                "[0:a:0]atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS[a{i}]",
                r.start, r.end
            ));
        }
    }
    let mut inputs = String::new();
    for i in 0..ranges.len() {
        inputs.push_str(&format!("[v{i}]"));
        if has_audio {
            inputs.push_str(&format!("[a{i}]"));
        }
    }
    nodes.push(format!(
        "{inputs}concat=n={}:v=1:a={}[v]{}",
        ranges.len(),
        if has_audio { 1 } else { 0 },
        if has_audio { "[a]" } else { "" }
    ));
    nodes.join(";")
}

pub async fn create(
    State(state): State<AppState>,
    Path(job_id): Path<String>,
    Json(request): Json<CutRequest>,
) -> AppResult<(StatusCode, Json<CutState>)> {
    let job: Job = jobs::get_job(&state, &job_id)
        .map_err(|_| AppError::NotFound("video job not found".into()))?;
    if job.status.is_active() {
        return Err(AppError::Conflict("cannot cut an active job".into()));
    }
    let source = job
        .input_path
        .ok_or_else(|| AppError::BadRequest("job has no source video".into()))?;
    let source = state
        .config
        .resolve_allowed_file(&source)
        .map_err(|_| AppError::Forbidden("video outside allowed roots".into()))?;
    let probe = probe_media(&source, &CancellationToken::new())
        .await
        .map_err(|err| AppError::BadRequest(format!("cannot read source media: {err}")))?;
    validate(&request.keep, probe.duration)?;
    let dir = state.config.data_dir.join("cuts");
    fs::create_dir_all(&dir).await?;
    let id = Uuid::new_v4().to_string();
    let basename = job.original_name.trim_end_matches(['.', ' ']);
    let stem = std::path::Path::new(basename)
        .file_stem()
        .and_then(|v| v.to_str())
        .unwrap_or("video");
    let stem: String = stem
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '))
        .take(60)
        .collect();
    let output = dir.join(format!("{}-cut-{}.mp4", stem.trim(), &id[..8]));
    let cut = CutState {
        id: id.clone(),
        status: "queued".into(),
        output_path: None,
        error: None,
    };
    state
        .db
        .upsert("cut", &id, &cut)
        .map_err(AppError::Internal)?;
    cuts().insert(id.clone(), cut.clone());
    let db = state.db.clone();
    tokio::spawn(async move {
        let result = execute(id.clone(), source, output, request.keep, probe.has_audio).await;
        if let Some(mut entry) = cuts().get_mut(&id) {
            match result {
                Ok(output) => {
                    entry.status = "done".into();
                    entry.output_path = Some(output.to_string_lossy().to_string());
                }
                Err(e) => {
                    entry.status = "error".into();
                    entry.error = Some(e);
                }
            }
            let snapshot = entry.clone();
            drop(entry);
            if let Err(e) = db.upsert("cut", &id, &snapshot) {
                tracing::error!(error=%e, "persist cut state failed");
            }
        }
    });
    Ok((StatusCode::ACCEPTED, Json(cut)))
}

async fn execute(
    id: String,
    source: PathBuf,
    output: PathBuf,
    keep: Vec<KeepRange>,
    has_audio: bool,
) -> Result<PathBuf, String> {
    let _permit = slots().acquire_owned().await.map_err(|e| e.to_string())?;
    if let Some(mut cut) = cuts().get_mut(&id) {
        cut.status = "running".into();
    }
    let temp = output.with_extension("part.mp4");
    let graph = filter_graph(&keep, has_audio);
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&source)
        .args(["-filter_complex", &graph, "-map", "[v]"]);
    if has_audio {
        cmd.args(["-map", "[a]"]);
    }
    cmd.args([
        "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
    ]);
    if has_audio {
        cmd.args(["-c:a", "aac", "-b:a", "192k"]);
    } else {
        cmd.arg("-an");
    }
    cmd.args(["-movflags", "+faststart"])
        .arg(&temp)
        .kill_on_drop(true);
    let result = cmd.output().await.map_err(|e| format!("ffmpeg spawn: {e}"));
    let outcome = match result {
        Ok(result) if result.status.success() => fs::rename(&temp, &output)
            .await
            .map(|_| output.clone())
            .map_err(|e| format!("save output: {e}")),
        Ok(result) => Err(format!(
            "ffmpeg exited {}: {}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
                .chars()
                .take(1200)
                .collect::<String>()
        )),
        Err(error) => Err(error),
    };
    if outcome.is_err() {
        let _ = fs::remove_file(&temp).await;
    }
    outcome
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<CutState>> {
    lookup_cut(&state, &id).map(Json)
}

pub async fn download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let item = lookup_cut(&state, &id)?;
    if item.status != "done" {
        return Err(AppError::Conflict("cut is not ready".into()));
    }
    let file = PathBuf::from(
        item.output_path
            .ok_or_else(|| AppError::NotFound("output missing".into()))?,
    );
    let reader = fs::File::open(&file).await?;
    let filename = file
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("cut.mp4");
    let safe = filename.replace(['"', '\\'], "_");
    let size = reader.metadata().await?.len();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "video/mp4")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{safe}\""),
        )
        .header(header::CONTENT_LENGTH, size.to_string())
        .body(Body::from_stream(ReaderStream::new(reader)))
        .map_err(|e| AppError::Internal(e.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persisted_clip_survives_server_cache_reset() {
        let temp = tempfile::tempdir().unwrap();
        let db = crate::db::Database::open(&temp.path().join("clips.sqlite")).unwrap();
        let result = CutState {
            id: Uuid::new_v4().to_string(),
            status: "done".into(),
            output_path: Some("/data/cuts/result.mp4".into()),
            error: None,
        };
        db.upsert("cut", &result.id, &result).unwrap();
        let restored = load_cut(&db, &result.id).unwrap().unwrap();
        assert_eq!(restored.status, "done");
        assert_eq!(restored.output_path, result.output_path);
        let queued = CutState {
            status: "running".into(),
            ..result
        };
        db.upsert("cut", &queued.id, &queued).unwrap();
        assert_eq!(
            load_cut(&db, &queued.id).unwrap().unwrap().status,
            "interrupted"
        );
    }

    #[test]
    fn rejects_overlap_and_bad_bounds() {
        assert!(
            validate(
                &[
                    KeepRange {
                        start: 0.0,
                        end: 5.0
                    },
                    KeepRange {
                        start: 4.0,
                        end: 8.0
                    }
                ],
                10.0
            )
            .is_err()
        );
        assert!(
            validate(
                &[
                    KeepRange {
                        start: 0.0,
                        end: 5.0
                    },
                    KeepRange {
                        start: 7.0,
                        end: 10.0
                    }
                ],
                10.0
            )
            .is_ok()
        );
        assert!(
            validate(
                &[KeepRange {
                    start: 0.0,
                    end: 10.5
                }],
                10.0
            )
            .is_err()
        );
    }
    #[test]
    fn graph_has_matching_stream_pairs() {
        let graph = filter_graph(
            &[
                KeepRange {
                    start: 1.0,
                    end: 2.0,
                },
                KeepRange {
                    start: 4.0,
                    end: 6.0,
                },
            ],
            true,
        );
        assert!(graph.contains("concat=n=2:v=1:a=1[v][a]"));
        assert!(graph.contains("atrim=start=4.000000:end=6.000000"));
    }
}
