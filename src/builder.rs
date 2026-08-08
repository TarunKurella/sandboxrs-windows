use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::backend;
use crate::filesystem::FilesystemPlan;
use crate::sandbox::Sandbox;
use crate::{BackendPreference, ResourceLimits, SandboxError};

/// Validates and builds a reusable [`Sandbox`].
pub struct SandboxBuilder {
    workspace: PathBuf,
    read_only: Vec<PathBuf>,
    read_write: Vec<PathBuf>,
    timeout: Option<Duration>,
    max_memory: Option<u64>,
    max_processes: Option<u32>,
    preferred_backend: BackendPreference,
}

impl SandboxBuilder {
    pub(crate) fn new(workspace: impl AsRef<Path>) -> Self {
        Self {
            workspace: workspace.as_ref().to_path_buf(),
            read_only: Vec::new(),
            read_write: Vec::new(),
            timeout: None,
            max_memory: None,
            max_processes: None,
            preferred_backend: BackendPreference::Auto,
        }
    }

    /// Grant read-only access to an explicit root.
    pub fn read_only(mut self, path: impl AsRef<Path>) -> Self {
        self.read_only.push(path.as_ref().to_path_buf());
        self
    }

    /// Grant read-write access to an explicit root.
    pub fn read_write(mut self, path: impl AsRef<Path>) -> Self {
        self.read_write.push(path.as_ref().to_path_buf());
        self
    }

    /// Fail the command after `timeout`, terminating its process tree.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Maximum memory (in bytes) the sandboxed process tree may commit.
    pub fn max_memory(mut self, bytes: u64) -> Self {
        self.max_memory = Some(bytes);
        self
    }

    /// Maximum number of concurrently active sandboxed processes.
    pub fn max_processes(mut self, count: u32) -> Self {
        self.max_processes = Some(count);
        self
    }

    /// Prefer a specific backend; `Auto` probes in plan order.
    pub fn preferred_backend(mut self, preference: BackendPreference) -> Self {
        self.preferred_backend = preference;
        self
    }

    /// Validate policy, probe/select a backend, and perform setup.
    ///
    /// This is a real initialization boundary. If any required step fails, no
    /// [`Sandbox`] value exists.
    pub fn build(self) -> Result<Sandbox, SandboxError> {
        let plan = FilesystemPlan::compile(&self.workspace, &self.read_only, &self.read_write)?;
        let backend_kind = backend::select(self.preferred_backend)?;
        backend::validate(&backend_kind, &plan)?;

        Ok(Sandbox::new(
            self.workspace,
            plan,
            backend_kind,
            ResourceLimits {
                max_processes: self.max_processes,
                max_memory: self.max_memory,
            },
            self.timeout,
        ))
    }

}
