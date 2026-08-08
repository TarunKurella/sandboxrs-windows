use std::process::ExitStatus;
use std::time::Duration;

use crate::BackendKind;

/// Result of a completed sandboxed command.
#[derive(Debug, Clone)]
pub struct SandboxOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub backend: BackendKind,
    pub duration: Duration,
}

impl SandboxOutput {
    pub(crate) fn from_output(
        output: std::process::Output,
        backend: BackendKind,
        duration: Duration,
    ) -> Self {
        Self {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
            backend,
            duration,
        }
    }
}
