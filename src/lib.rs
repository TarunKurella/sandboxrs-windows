//! No-admin Windows process sandbox library with a `std::process::Command`-like API.
//!
//! ```no_run
//! use sandboxrs_windows::Sandbox;
//!
//! let sandbox = Sandbox::builder(r"C:\repo")
//!     .read_only(r"C:\Users\me\.rustup")
//!     .read_write(r"C:\temp\sandboxrs")
//!     .build()?;
//!
//! let output = sandbox
//!     .command("cargo")
//!     .arg("test")
//!     .output()?;
//! # Ok::<(), sandboxrs_windows::SandboxError>(())
//! ```

mod backend;
mod builder;
mod child;
mod command;
mod error;
mod filesystem;
#[cfg(windows)]
mod job;
mod output;
mod sandbox;

pub use builder::SandboxBuilder;
pub use child::SandboxChild;
pub use command::SandboxCommand;
pub use error::SandboxError;
pub use output::SandboxOutput;
pub use sandbox::{BackendProbe, BackendProbeEntry, Sandbox};

/// Standard stream mode for a sandboxed command.
///
/// This mirrors `std::process::Stdio`, which is opaque and therefore cannot be
/// inspected by a library after it is passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stdio {
    Inherit,
    Null,
    Piped,
}

impl Stdio {
    pub fn inherit() -> Self {
        Self::Inherit
    }

    pub fn null() -> Self {
        Self::Null
    }

    pub fn piped() -> Self {
        Self::Piped
    }
}

/// Backend that enforces a sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    WindowsSandboxApi,
    AppContainer,
}

impl BackendKind {
    /// Stable machine-readable name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsSandboxApi => "windows-sandbox-api",
            Self::AppContainer => "appcontainer",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::WindowsSandboxApi => "Windows Sandbox API",
            Self::AppContainer => "AppContainer",
        }
    }
}

/// Backend selection preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendPreference {
    Auto,
    WindowsSandboxApi,
    AppContainer,
}

/// Resource and lifecycle limits applied through the Job Object.
#[derive(Debug, Clone, Copy, Default)]
#[allow(dead_code)]
pub(crate) struct ResourceLimits {
    pub(crate) max_processes: Option<u32>,
    pub(crate) max_memory: Option<u64>,
}

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
