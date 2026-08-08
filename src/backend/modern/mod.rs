//! Experimental Windows sandbox API backend.
//!
//! All experimental FFI stays inside this module. The Microsoft API is
//! unstable, so capability probing is runtime-based and the exported symbol is
//! never treated as proof of usability.

use crate::filesystem::FilesystemPlan;
use crate::{BackendKind, SandboxError};

pub(crate) mod ffi;
pub(crate) mod probe;

pub(crate) struct ProbeOutcome {
    pub(crate) export_present: bool,
    pub(crate) usable: bool,
    pub(crate) detail: String,
}

pub(crate) fn probe() -> ProbeOutcome {
    match ffi::resolve_create_process_in_sandbox() {
        Ok(true) => ProbeOutcome {
            export_present: true,
            usable: false,
            detail: "export present; M0 launch probe pending on Windows".into(),
        },
        Ok(false) => ProbeOutcome {
            export_present: false,
            usable: false,
            detail: "processmodel.dll does not export Experimental_CreateProcessInSandbox".into(),
        },
        Err(err) => ProbeOutcome {
            export_present: false,
            usable: false,
            detail: err,
        },
    }
}

pub(crate) fn validate(
    backend: BackendKind,
    _plan: &FilesystemPlan,
) -> Result<(), SandboxError> {
    // `select()` only returns a backend whose probe succeeded. M0/M1 will
    // compile the plan into the sandbox spec before any launch is allowed.
    let _ = backend;
    Ok(())
}
