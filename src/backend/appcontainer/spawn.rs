//! Sandboxed process creation through a regular AppContainer via `rappct`.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use rappct::launch::{JobLimits, LaunchOptions, StdioConfig};
use rappct::{launch_in_container_with_io, SecurityCapabilitiesBuilder};
use windows_sys::Win32::Foundation::{GetLastError, HANDLE};
use windows_sys::Win32::System::JobObjects::{
    JobObjectExtendedLimitInformation, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
};

use crate::child::SandboxChild;
use crate::sandbox::Sandbox;
use crate::{BackendKind, SandboxError, Stdio};

use super::map_rappct_error;

pub(crate) fn spawn(
    sandbox: &Sandbox,
    program: OsString,
    args: Vec<OsString>,
    env_clear: bool,
    envs: BTreeMap<OsString, OsString>,
    removals: Vec<OsString>,
    current_dir: Option<PathBuf>,
    stdin: Stdio,
    stdout: Stdio,
    stderr: Stdio,
) -> Result<SandboxChild, SandboxError> {
    let appcontainer = sandbox
        .appcontainer()
        .ok_or_else(|| SandboxError::PolicyCompileFailed {
            backend: BackendKind::AppContainer,
            reason: "AppContainer backend state was not initialized".into(),
        })?;
    let exe = resolve_program(&program)?;
    let command_line = build_command_line(&args);
    let env = build_env(env_clear, &envs, &removals);
    let cwd = current_dir.or_else(|| std::env::current_dir().ok());

    let caps = SecurityCapabilitiesBuilder::new(&appcontainer.profile.sid)
        .build()
        .map_err(map_rappct_error)?;
    let limits = sandbox.limits();
    let timeout = sandbox.timeout();

    // rappct only adds the PROC_THREAD_ATTRIBUTE_HANDLE_LIST for its pipe
    // path; its NUL path sets inherit_handles without the list, which
    // CreateProcessW rejects with ERROR_INVALID_PARAMETER when security
    // capabilities are attached. Route every non-inherit mode through pipes
    // and keep the parent ends owned by the child.
    let stdio_config = if matches!(
        (stdin, stdout, stderr),
        (Stdio::Inherit, Stdio::Inherit, Stdio::Inherit)
    ) {
        StdioConfig::Inherit
    } else {
        StdioConfig::Pipe
    };

    let launched = launch_in_container_with_io(
        &caps,
        &LaunchOptions {
            exe,
            cmdline: Some(command_line),
            cwd,
            env: Some(rappct::launch::merge_parent_env(env)),
            stdio: stdio_config,
            suspended: false,
            join_job: Some(JobLimits {
                memory_bytes: limits.max_memory.map(|bytes| bytes as usize),
                cpu_rate_percent: None,
                kill_on_job_close: true,
            }),
            startup_timeout: None,
        },
    )
    .map_err(map_rappct_error)?;

    if let Some(count) = limits.max_processes {
        if let Some(job) = launched.job_guard.as_ref() {
            apply_process_limit(job.as_handle().0, count)?;
        }
    }

    let process = open_process_handle(launched.pid)?;
    Ok(SandboxChild::new_appcontainer(
        BackendKind::AppContainer,
        launched.pid,
        process,
        launched.job_guard,
        launched.stdin,
        launched.stdout,
        launched.stderr,
        timeout,
    ))
}

fn apply_process_limit(handle: HANDLE, count: u32) -> Result<(), SandboxError> {
    let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
    info.BasicLimitInformation.ActiveProcessLimit = count;
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_ACTIVE_PROCESS;
    // SAFETY: `handle` is a valid job handle owned by rappct's JobGuard, and
    // the info structure is fully initialized.
    let ok = unsafe {
        SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<core::ffi::c_void>(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if ok == 0 {
        return Err(SandboxError::ProcessCreationFailed {
            backend: BackendKind::AppContainer,
            win32_code: Some(unsafe { GetLastError() }),
            message: "failed to apply active process limit to AppContainer job".into(),
        });
    }
    Ok(())
}

fn open_process_handle(pid: u32) -> Result<HANDLE, SandboxError> {
    let access = PROCESS_QUERY_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE;
    // SAFETY: pid is the just-created child process id and the access mask only
    // requests standard process rights.
    let handle = unsafe { OpenProcess(access, 0, pid) };
    if handle.is_null() {
        return Err(SandboxError::ProcessCreationFailed {
            backend: BackendKind::AppContainer,
            win32_code: Some(unsafe { GetLastError() }),
            message: "failed to open AppContainer child process handle".into(),
        });
    }
    Ok(handle)
}

fn resolve_program(program: &OsStr) -> Result<PathBuf, SandboxError> {
    let path = PathBuf::from(program);
    if path.is_absolute() {
        return Ok(path);
    }
    let extensions = ["", ".exe", ".com", ".bat", ".cmd"];
    let search = std::env::var_os("PATH")
        .map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
        .unwrap_or_default();
    for dir in search {
        for extension in extensions {
            let candidate = dir.join(format!("{}{}", program.to_string_lossy(), extension));
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(SandboxError::ProcessCreationFailed {
        backend: BackendKind::AppContainer,
        win32_code: None,
        message: format!("could not resolve executable {:?} on PATH", program),
    })
}

fn build_command_line(args: &[OsString]) -> String {
    let mut line = String::new();
    for arg in args {
        line.push(' ');
        line.push_str(&quote_arg(arg));
    }
    line
}

fn quote_arg(arg: &OsStr) -> String {
    let value = arg.to_string_lossy();
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    let mut backslashes = 0usize;
    for ch in value.chars() {
        if ch == '\\' {
            backslashes += 1;
        } else if ch == '"' {
            for _ in 0..(2 * backslashes + 1) {
                quoted.push('\\');
            }
            quoted.push('"');
            backslashes = 0;
        } else {
            for _ in 0..backslashes {
                quoted.push('\\');
            }
            quoted.push(ch);
            backslashes = 0;
        }
    }
    for _ in 0..(2 * backslashes) {
        quoted.push('\\');
    }
    quoted.push('"');
    quoted
}

fn build_env(
    env_clear: bool,
    envs: &BTreeMap<OsString, OsString>,
    removals: &[OsString],
) -> Vec<(OsString, OsString)> {
    let mut entries: Vec<(OsString, OsString)> = if env_clear {
        Vec::new()
    } else {
        std::env::vars_os().collect()
    };
    for removal in removals {
        let removed = removal.to_string_lossy().to_uppercase();
        entries.retain(|(key, _)| key.to_string_lossy().to_uppercase() != removed);
    }
    for (key, value) in envs {
        let upper = key.to_string_lossy().to_uppercase();
        entries.retain(|(existing, _)| existing.to_string_lossy().to_uppercase() != upper);
        entries.push((key.clone(), value.clone()));
    }
    entries
}
