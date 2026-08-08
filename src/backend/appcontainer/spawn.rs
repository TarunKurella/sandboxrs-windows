//! Sandboxed process creation through a regular AppContainer.
//!
//! `rappct` provides profile/ACL/capability helpers, but its published launch
//! helper has a known attribute-list/handle-list defect that fails with
//! ERROR_INVALID_PARAMETER on current Windows builds. This module implements
//! the launch itself with a single `STARTUPINFOEXW`: SECURITY_CAPABILITIES,
//! HANDLE_LIST, suspended creation, Job assignment before resume.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, RawHandle};
use std::path::PathBuf;

use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, LocalFree, SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT, HLOCAL,
};
use windows_sys::Win32::Security::Authorization::ConvertStringSidToSidW;
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Security::{PSID, SECURITY_CAPABILITIES, SID_AND_ATTRIBUTES};
use windows_sys::Win32::System::Console::{
    GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Pipes::CreatePipe;
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList, ResumeThread,
    UpdateProcThreadAttribute, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

use crate::child::SandboxChild;
use crate::job::Job;
use crate::sandbox::Sandbox;
use crate::{BackendKind, ResourceLimits, SandboxError, Stdio};

use super::AppContainerSandbox;

struct StdioSetup {
    startup: STARTUPINFOEXW,
    stdin_parent: Option<File>,
    stdout_parent: Option<File>,
    stderr_parent: Option<File>,
    child_handles: Vec<HANDLE>,
    keepalive: Vec<File>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IoSlot {
    Stdin,
    Stdout,
    Stderr,
}

struct SecurityCaps {
    capabilities: SECURITY_CAPABILITIES,
    sid: HLOCAL,
}

impl SecurityCaps {
    fn from_sddl(sddl: &str) -> Result<Self, SandboxError> {
        let wide: Vec<u16> = sddl.encode_utf16().chain(std::iter::once(0)).collect();
        let mut sid: PSID = std::ptr::null_mut();
        // SAFETY: `wide` is a valid null-terminated wide string and `sid` is a
        // valid out-parameter. The returned SID is owned by LocalAlloc.
        let ok = unsafe { ConvertStringSidToSidW(wide.as_ptr(), &mut sid) };
        if ok == 0 || sid.is_null() {
            return Err(SandboxError::PolicyCompileFailed {
                backend: BackendKind::AppContainer,
                reason: format!("ConvertStringSidToSidW failed for {sddl}"),
            });
        }
        let capabilities = SECURITY_CAPABILITIES {
            AppContainerSid: sid,
            Capabilities: std::ptr::null_mut::<SID_AND_ATTRIBUTES>(),
            CapabilityCount: 0,
            Reserved: 0,
        };
        Ok(Self {
            capabilities,
            sid: sid as HLOCAL,
        })
    }
}

impl Drop for SecurityCaps {
    fn drop(&mut self) {
        // SAFETY: `sid` was allocated by ConvertStringSidToSidW and is freed
        // exactly once after the launch call completes.
        unsafe {
            let _ = LocalFree(self.sid);
        }
    }
}

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
    let command_line = build_command_line(&exe, &args)?;
    let env_block = build_env_block(env_clear, &envs, &removals);
    let current_dir = current_dir
        .as_deref()
        .and_then(|path| path.to_str())
        .map(|value| {
            value
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>()
        });
    let stdio = setup_stdio(stdin, stdout, stderr)?;
    let limits = sandbox.limits();
    let timeout = sandbox.timeout();

    let spawned = launch(
        appcontainer,
        &exe,
        &command_line,
        current_dir.as_deref(),
        env_block.as_deref(),
        &stdio,
        limits,
    )?;

    Ok(SandboxChild::new(
        BackendKind::AppContainer,
        spawned.process,
        spawned.thread,
        spawned.pid,
        spawned.job,
        stdio.stdin_parent,
        stdio.stdout_parent,
        stdio.stderr_parent,
        timeout,
    ))
}

pub(crate) struct SpawnedProcess {
    pub(crate) process: HANDLE,
    pub(crate) thread: HANDLE,
    pub(crate) pid: u32,
    pub(crate) job: Job,
}

fn launch(
    appcontainer: &AppContainerSandbox,
    exe: &std::path::Path,
    command_line: &[u16],
    current_dir: Option<&[u16]>,
    env_block: Option<&[u16]>,
    stdio: &StdioSetup,
    limits: ResourceLimits,
) -> Result<SpawnedProcess, SandboxError> {
    let caps = SecurityCaps::from_sddl(appcontainer.profile.sid.as_string())?;
    let mut command_line = command_line.to_vec();

    let attribute_count = 2u32;
    let mut needed = 0usize;
    // SAFETY: The first call intentionally passes NULL to query the required
    // buffer size; it fails with ERROR_INSUFFICIENT_BUFFER and fills `needed`.
    unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), attribute_count, 0, &mut needed);
    }
    let mut attribute_buffer = vec![0u8; needed];
    let attribute_list = attribute_buffer.as_mut_ptr() as LPPROC_THREAD_ATTRIBUTE_LIST;
    // SAFETY: `attribute_buffer` is large enough and outlives the attribute
    // list; the list is initialized before use.
    let ok = unsafe {
        InitializeProcThreadAttributeList(attribute_list, attribute_count, 0, &mut needed)
    };
    if ok == 0 {
        return Err(SandboxError::PolicyCompileFailed {
            backend: BackendKind::AppContainer,
            reason: format!(
                "InitializeProcThreadAttributeList failed, Win32 error {}",
                unsafe { GetLastError() }
            ),
        });
    }

    // SAFETY: `caps` and `stdio.child_handles` outlive the CreateProcessW call;
    // the attribute list is valid until deleted below.
    let ok = unsafe {
        UpdateProcThreadAttribute(
            attribute_list,
            0,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
            (&raw const caps.capabilities).cast(),
            std::mem::size_of::<SECURITY_CAPABILITIES>(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        delete_attribute_list(attribute_list);
        return Err(SandboxError::PolicyCompileFailed {
            backend: BackendKind::AppContainer,
            reason: format!(
                "UpdateProcThreadAttribute(security) failed, Win32 error {}",
                unsafe { GetLastError() }
            ),
        });
    }
    let ok = unsafe {
        UpdateProcThreadAttribute(
            attribute_list,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            stdio.child_handles.as_ptr().cast(),
            std::mem::size_of::<HANDLE>() * stdio.child_handles.len(),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    };
    if ok == 0 {
        delete_attribute_list(attribute_list);
        return Err(SandboxError::PolicyCompileFailed {
            backend: BackendKind::AppContainer,
            reason: format!(
                "UpdateProcThreadAttribute(handles) failed, Win32 error {}",
                unsafe { GetLastError() }
            ),
        });
    }

    let mut startup = stdio.startup;
    startup.lpAttributeList = attribute_list;
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;

    let exe_wide: Vec<u16> = exe
        .to_str()
        .ok_or_else(|| SandboxError::InvalidPath {
            path: exe.to_path_buf(),
            reason: "executable path is not valid Unicode".into(),
        })?
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut process_information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
    let creation_flags = EXTENDED_STARTUPINFO_PRESENT
        | CREATE_SUSPENDED
        | CREATE_NO_WINDOW
        | if env_block.is_some() {
            CREATE_UNICODE_ENVIRONMENT
        } else {
            0
        };

    // SAFETY: All pointers are valid and live through the call; the command
    // line buffer is mutable; reserved security attributes are NULL.
    let result = unsafe {
        CreateProcessW(
            exe_wide.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            creation_flags,
            env_block
                .map(|block| block.as_ptr() as *const core::ffi::c_void)
                .unwrap_or(std::ptr::null()),
            current_dir
                .map(|dir| dir.as_ptr())
                .unwrap_or(std::ptr::null()),
            &mut startup as *mut STARTUPINFOEXW as *mut _,
            &mut process_information,
        )
    };
    delete_attribute_list(attribute_list);

    if result == 0 {
        let error = unsafe { GetLastError() };
        return Err(SandboxError::ProcessCreationFailed {
            backend: BackendKind::AppContainer,
            win32_code: Some(error),
            message: format!("CreateProcessW failed: {error}"),
        });
    }

    let job = Job::new(limits).map_err(|err| {
        terminate_and_reap(process_information.hProcess, process_information.hThread);
        err
    })?;
    if let Err(err) = job.assign_raw(process_information.hProcess) {
        let error = unsafe { GetLastError() };
        terminate_and_reap(process_information.hProcess, process_information.hThread);
        return Err(SandboxError::ProcessCreationFailed {
            backend: BackendKind::AppContainer,
            win32_code: Some(error),
            message: format!("job assignment failed: {err}"),
        });
    }
    // SAFETY: `hThread` is the just-created main thread handle; ResumeThread
    // only adjusts its suspend count.
    let resumed = unsafe { ResumeThread(process_information.hThread) };
    if resumed == u32::MAX {
        let error = unsafe { GetLastError() };
        let _ = job.terminate();
        terminate_and_reap(process_information.hProcess, process_information.hThread);
        return Err(SandboxError::ProcessCreationFailed {
            backend: BackendKind::AppContainer,
            win32_code: Some(error),
            message: format!("ResumeThread failed: {error}"),
        });
    }

    Ok(SpawnedProcess {
        process: process_information.hProcess,
        thread: process_information.hThread,
        pid: process_information.dwProcessId,
        job,
    })
}

fn delete_attribute_list(attribute_list: LPPROC_THREAD_ATTRIBUTE_LIST) {
    // SAFETY: The list was initialized by InitializeProcThreadAttributeList and
    // is deleted exactly once after CreateProcessW.
    unsafe {
        DeleteProcThreadAttributeList(attribute_list);
    }
}

fn terminate_and_reap(process: HANDLE, thread: HANDLE) {
    // SAFETY: Both handles were returned by CreateProcessW and are still owned
    // by the failed spawn path.
    unsafe {
        let _ = windows_sys::Win32::System::Threading::TerminateProcess(process, 1);
        let _ = windows_sys::Win32::System::Threading::WaitForSingleObject(process, u32::MAX);
        let _ = CloseHandle(process);
        let _ = CloseHandle(thread);
    }
}

fn setup_stdio(stdin: Stdio, stdout: Stdio, stderr: Stdio) -> Result<StdioSetup, SandboxError> {
    let mut setup = StdioSetup {
        startup: unsafe { std::mem::zeroed() },
        stdin_parent: None,
        stdout_parent: None,
        stderr_parent: None,
        child_handles: Vec::new(),
        keepalive: Vec::new(),
    };

    setup.startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    let stdin_child = prepare_handle(stdin, IoSlot::Stdin, &mut setup)?;
    let stdout_child = prepare_handle(stdout, IoSlot::Stdout, &mut setup)?;
    let stderr_child = prepare_handle(stderr, IoSlot::Stderr, &mut setup)?;

    setup.startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    setup.startup.StartupInfo.hStdInput = stdin_child;
    setup.startup.StartupInfo.hStdOutput = stdout_child;
    setup.startup.StartupInfo.hStdError = stderr_child;
    Ok(setup)
}

fn prepare_handle(
    mode: Stdio,
    slot: IoSlot,
    setup: &mut StdioSetup,
) -> Result<HANDLE, SandboxError> {
    match mode {
        Stdio::Inherit => {
            let handle = unsafe {
                GetStdHandle(match slot {
                    IoSlot::Stdin => STD_INPUT_HANDLE,
                    IoSlot::Stdout => STD_OUTPUT_HANDLE,
                    IoSlot::Stderr => STD_ERROR_HANDLE,
                })
            };
            if handle.is_null() {
                return Err(SandboxError::Io(io::Error::last_os_error()));
            }
            make_inheritable(handle)?;
            setup.child_handles.push(handle);
            Ok(handle)
        }
        Stdio::Null => {
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open("NUL")
                .map_err(SandboxError::Io)?;
            let handle = file.as_raw_handle();
            make_inheritable(handle)?;
            setup.child_handles.push(handle);
            setup.keepalive.push(file);
            Ok(handle)
        }
        Stdio::Piped => {
            let attributes = SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: std::ptr::null_mut(),
                bInheritHandle: 1,
            };
            let mut read_end: HANDLE = std::ptr::null_mut();
            let mut write_end: HANDLE = std::ptr::null_mut();
            // SAFETY: Both out-parameters are valid and `attributes` is fully
            // initialized.
            let ok = unsafe { CreatePipe(&mut read_end, &mut write_end, &attributes, 0) };
            if ok == 0 {
                return Err(SandboxError::Io(io::Error::last_os_error()));
            }

            let (child_end, parent_end) = match slot {
                IoSlot::Stdin => (write_end, read_end),
                IoSlot::Stdout | IoSlot::Stderr => (read_end, write_end),
            };
            // SAFETY: `parent_end` is a valid pipe handle created above and
            // ownership transfers to the returned File.
            let file = unsafe { File::from_raw_handle(parent_end as RawHandle) };
            make_inheritable(child_end)?;
            setup.child_handles.push(child_end);
            match slot {
                IoSlot::Stdin => setup.stdin_parent = Some(file),
                IoSlot::Stdout => setup.stdout_parent = Some(file),
                IoSlot::Stderr => setup.stderr_parent = Some(file),
            }
            Ok(child_end)
        }
    }
}

fn make_inheritable(handle: HANDLE) -> Result<(), SandboxError> {
    // SAFETY: `handle` is a valid handle owned by the caller; the flags only
    // adjust inheritance on that handle.
    let ok = unsafe { SetHandleInformation(handle, HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) };
    if ok == 0 {
        return Err(SandboxError::Io(io::Error::last_os_error()));
    }
    Ok(())
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

fn build_command_line(exe: &std::path::Path, args: &[OsString]) -> Result<Vec<u16>, SandboxError> {
    let mut parts = vec![quote_arg(exe.as_os_str())?];
    parts.extend(
        args.iter()
            .map(|arg| quote_arg(arg.as_os_str()))
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(parts
        .join(" ")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect())
}

fn quote_arg(arg: &OsStr) -> Result<String, SandboxError> {
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
    Ok(quoted)
}

fn build_env_block(
    env_clear: bool,
    envs: &BTreeMap<OsString, OsString>,
    removals: &[OsString],
) -> Option<Vec<u16>> {
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
    // An empty custom environment makes CreateProcessW fail with error 203
    // ("environment could not be set") on current Windows builds. Always
    // include essential Windows variables so sandboxed children can load.
    if entries.is_empty() {
        for key in [
            "SystemRoot",
            "windir",
            "ComSpec",
            "PATHEXT",
            "TEMP",
            "TMP",
            "PATH",
        ] {
            if let Some(value) = std::env::var_os(key) {
                entries.push((OsString::from(key), value));
            }
        }
        // CreateProcessW also needs drive current-directory variables
        // ("=C:=C:\\...") when a custom environment block is supplied.
        for (key, value) in std::env::vars_os() {
            if key.to_string_lossy().starts_with('=') {
                entries.push((key, value));
            }
        }
    }
    entries.sort_by(|a, b| {
        a.0.to_string_lossy()
            .to_ascii_uppercase()
            .cmp(&b.0.to_string_lossy().to_ascii_uppercase())
    });
    let mut block = Vec::new();
    for (key, value) in entries {
        block.extend(
            format!("{}={}", key.to_string_lossy(), value.to_string_lossy()).encode_utf16(),
        );
        block.push(0);
    }
    block.push(0);
    Some(block)
}
