use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::backend;
use crate::builder::SandboxBuilder;
use crate::command::SandboxCommand;
use crate::filesystem::FilesystemPlan;
use crate::{BackendKind, BackendPreference, ResourceLimits, SandboxError};

/// Reusable, validated authority boundary.
///
/// A sandbox represents one policy. Multiple commands may run inside it
/// without rebuilding policy each time.
pub struct Sandbox {
    workspace: PathBuf,
    plan: FilesystemPlan,
    backend: BackendKind,
    limits: ResourceLimits,
    timeout: Option<Duration>,
}

impl Sandbox {
    /// Start building a sandbox whose workspace is read-write.
    pub fn builder(workspace: impl AsRef<Path>) -> SandboxBuilder {
        SandboxBuilder::new(workspace)
    }

    pub(crate) fn new(
        workspace: PathBuf,
        plan: FilesystemPlan,
        backend: BackendKind,
        limits: ResourceLimits,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            workspace,
            plan,
            backend,
            limits,
            timeout,
        }
    }

    /// Create a command inside this sandbox.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> SandboxCommand<'_> {
        SandboxCommand::new(self, program.as_ref())
    }

    /// The backend enforcing this sandbox.
    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    pub(crate) fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub(crate) fn plan(&self) -> &FilesystemPlan {
        &self.plan
    }

    pub(crate) fn limits(&self) -> ResourceLimits {
        self.limits
    }

    pub(crate) fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Backends that are usable on this machine, probed live.
    pub fn available_backends() -> Vec<BackendKind> {
        backend::probe_report(BackendPreference::Auto)
            .entries
            .into_iter()
            .filter(|entry| entry.usable)
            .map(|entry| entry.backend)
            .collect()
    }

    /// Capability probe report for diagnostics.
    pub fn probe() -> BackendProbe {
        backend::probe_report(BackendPreference::Auto)
    }
}

/// Result of probing sandbox backends.
#[derive(Debug, Clone)]
pub struct BackendProbe {
    pub entries: Vec<BackendProbeEntry>,
}

#[derive(Debug, Clone)]
pub struct BackendProbeEntry {
    pub backend: BackendKind,
    pub export_present: bool,
    pub usable: bool,
    pub detail: String,
}
