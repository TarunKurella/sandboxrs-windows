//! M3 probe: real AppContainer launch and outside-write denial without admin.

#[cfg(windows)]
pub(crate) fn launch_probe() -> Result<(), String> {
    use std::fs;
    use std::time::SystemTime;

    use rappct::launch::{JobLimits, LaunchOptions, StdioConfig};
    use rappct::{launch_in_container_with_io, SecurityCapabilitiesBuilder};

    use crate::filesystem::FilesystemPlan;

    std::env::set_var("RAPPCT_DEBUG_LAUNCH", "1");

    let nonce = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|err| err.to_string())?
        .as_nanos() as u64;
    let identity = format!("sandbox-probe-{nonce:016x}");
    let temp = std::env::temp_dir();
    let workspace = temp.join(format!("{identity}-workspace"));
    let outside = temp.join(format!("{identity}-outside.txt"));
    let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());

    let result = (|| {
        fs::create_dir_all(&workspace).map_err(|err| err.to_string())?;

        let plan = FilesystemPlan::compile(&workspace, &[], &[]).map_err(|err| err.to_string())?;
        let profile = rappct::AppContainerProfile::ensure(&identity, &identity, None)
            .map_err(|err| err.to_string())?;
        let result = (|| {
            super::compile::apply_grants(&plan, &profile).map_err(|err| err.to_string())?;
            let caps = SecurityCapabilitiesBuilder::new(&profile.sid)
                .build()
                .map_err(|err| err.to_string())?;

            let ok = launch_in_container_with_io(
                &caps,
                &LaunchOptions {
                    exe: format!(r"{system_root}\System32\cmd.exe").into(),
                    cmdline: Some(" /C exit 0".into()),
                    cwd: Some(workspace.clone()),
                    env: Some(rappct::launch::merge_parent_env(Vec::new())),
                    stdio: StdioConfig::Pipe,
                    suspended: false,
                    join_job: Some(JobLimits {
                        memory_bytes: None,
                        cpu_rate_percent: None,
                        kill_on_job_close: true,
                    }),
                    startup_timeout: None,
                },
            )
            .map_err(|err| err.to_string())?;
            let code = ok.wait(None).map_err(|err| err.to_string())?;
            if code != 0 {
                return Err(format!("probe launch exited with code {code}"));
            }

            let denied = launch_in_container_with_io(
                &caps,
                &LaunchOptions {
                    exe: format!(r"{system_root}\System32\cmd.exe").into(),
                    cmdline: Some(format!(r#" /C type nul > "{}""#, outside.display())),
                    cwd: Some(workspace.clone()),
                    env: Some(rappct::launch::merge_parent_env(Vec::new())),
                    stdio: StdioConfig::Pipe,
                    suspended: false,
                    join_job: Some(JobLimits {
                        memory_bytes: None,
                        cpu_rate_percent: None,
                        kill_on_job_close: true,
                    }),
                    startup_timeout: None,
                },
            )
            .map_err(|err| err.to_string())?;
            let denied_code = denied.wait(None).map_err(|err| err.to_string())?;
            if denied_code == 0 || outside.exists() {
                return Err(format!(
                    "outside-write was not denied (exit {denied_code}, file existed={})",
                    outside.exists()
                ));
            }
            Ok(())
        })();

        let _ = profile.delete();
        result
    })();

    let _ = fs::remove_dir_all(&workspace);
    let _ = fs::remove_file(&outside);
    result
}

#[cfg(not(windows))]
pub(crate) fn launch_probe() -> Result<(), String> {
    Err("AppContainer probe requires Windows".into())
}
