//! M3 probe: real AppContainer launch and outside-write denial without admin.
//!
//! The probe exercises the same `spawn` path used by the library, not a
//! separate launch helper.

#[cfg(windows)]
pub(crate) fn launch_probe() -> Result<(), String> {
    use std::fs;
    use std::time::SystemTime;

    use crate::filesystem::FilesystemPlan;
    use crate::sandbox::Sandbox;
    use crate::{BackendKind, ResourceLimits, Stdio};

    use super::{build_state, spawn};

    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos() as u64;
    let identity = format!("sandbox-probe-{nonce:016x}");
    let temp = std::env::temp_dir();
    let workspace = temp.join(format!("{identity}-workspace"));
    let outside = temp.join(format!("{identity}-outside.txt"));

    let result = (|| {
        fs::create_dir_all(&workspace).map_err(|err| err.to_string())?;
        let plan = FilesystemPlan::compile(&workspace, &[], &[]).map_err(|err| err.to_string())?;
        let appcontainer = build_state(&identity, &plan).map_err(|err| err.to_string())?;
        let sandbox = Sandbox::new(
            workspace.clone(),
            plan,
            BackendKind::AppContainer,
            identity,
            ResourceLimits::default(),
            None,
            Some(appcontainer),
        );

        let mut success = spawn::spawn(
            &sandbox,
            "cmd".into(),
            vec!["/c".into(), "exit".into(), "0".into()],
            false,
            Default::default(),
            Vec::new(),
            Some(workspace.clone()),
            Stdio::Piped,
            Stdio::Piped,
            Stdio::Piped,
        )
        .map_err(|err| err.to_string())?;
        let status = success.wait().map_err(|err| err.to_string())?;
        if !status.success() {
            return Err(format!("probe launch exited with {status:?}"));
        }

        let mut denied = spawn::spawn(
            &sandbox,
            "cmd".into(),
            vec![
                "/c".into(),
                format!("type nul > {}", outside.display()).into(),
            ],
            false,
            Default::default(),
            Vec::new(),
            Some(workspace.clone()),
            Stdio::Piped,
            Stdio::Piped,
            Stdio::Piped,
        )
        .map_err(|err| err.to_string())?;
        let denied_status = denied.wait().map_err(|err| err.to_string())?;
        if denied_status.success() || outside.exists() {
            return Err(format!(
                "outside-write was not denied (status {denied_status:?}, file existed={})",
                outside.exists()
            ));
        }
        Ok(())
    })();

    let _ = fs::remove_dir_all(&workspace);
    let _ = fs::remove_file(&outside);
    result
}

#[cfg(not(windows))]
pub(crate) fn launch_probe() -> Result<(), String> {
    Err("AppContainer probe requires Windows".into())
}
