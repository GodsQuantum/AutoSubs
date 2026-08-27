use std::process::ExitStatus;
use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("process cancelled")]
    Cancelled,
    #[error("failed to spawn {program}: {source}")]
    Spawn { program: String, source: std::io::Error },
    #[error("process I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("process failed ({status}): {stderr}")]
    Failed { status: ExitStatus, stderr: String },
}

#[derive(Debug)]
pub struct ProcessOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

pub async fn run_capture(mut command: Command, token: &CancellationToken) -> Result<ProcessOutput, ProcessError> {
    let program = command.as_std().get_program().to_string_lossy().to_string();
    command.kill_on_drop(true);
    command.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = command.spawn().map_err(|source| ProcessError::Spawn { program, source })?;
    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    let stdout_task = tokio::spawn(async move {
        let mut data = Vec::new();
        stdout.read_to_end(&mut data).await.map(|_| data)
    });
    let stderr_task = tokio::spawn(async move {
        let mut data = Vec::new();
        stderr.read_to_end(&mut data).await.map(|_| data)
    });

    let status = tokio::select! {
        status = child.wait() => status?,
        _ = token.cancelled() => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            stdout_task.abort();
            stderr_task.abort();
            return Err(ProcessError::Cancelled);
        }
    };

    let stdout = stdout_task.await.map_err(|e| std::io::Error::other(e.to_string()))??;
    let stderr = stderr_task.await.map_err(|e| std::io::Error::other(e.to_string()))??;
    if !status.success() {
        return Err(ProcessError::Failed { status, stderr: String::from_utf8_lossy(&stderr).into_owned() });
    }
    Ok(ProcessOutput { status, stdout, stderr })
}
