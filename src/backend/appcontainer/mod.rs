//! Regular AppContainer fallback via `rappct` (M3).

use crate::filesystem::FilesystemPlan;
use crate::{BackendKind, SandboxError};

pub(crate) mod compile;
pub(crate) mod probe;

pub(crate) struct ProbeOutcome {
    pub(crate) export_present: bool,
    pub(crate) usable: bool,
    pub(crate) detail: String,
}

pub(crate) fn probe() -> ProbeOutcome {
    ProbeOutcome {
        export_present: false,
        usable: false,
        detail: "AppContainer fallback planned for M3; not yet implemented".into(),
    }
}

pub(crate) fn validate(
    backend: BackendKind,
    _plan: &FilesystemPlan,
) -> Result<(), SandboxError> {
    let _ = backend;
    Ok(())
}
