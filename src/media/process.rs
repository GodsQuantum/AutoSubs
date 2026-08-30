use std::process::{ExitStatus, Output, Stdio};
use thiserror::Error;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("{program} could not be started: {source}")]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    #[error("process failed ({status}): {stderr}")]
    Failed { status: ExitStatus, stderr: String },
    #[error("operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub async fn run_capture(
    mut command: Command,
    token: &CancellationToken,
) -> Result<Output, ProcessError> {
    let program = format!("{:?}", command.as_std().get_program());
    command
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let child = command
        .spawn()
        .map_err(|source| ProcessError::Spawn { program, source })?;
    tokio::select! {
        result = child.wait_with_output() => {
            let output = result?;
            if output.status.success() { Ok(output) } else {
                Err(ProcessError::Failed { status: output.status, stderr: String::from_utf8_lossy(&output.stderr).into_owned() })
            }
        }
        _ = token.cancelled() => Err(ProcessError::Cancelled),
    }
}
