use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::process::Stdio;

use crate::child::SandboxChild;
use crate::sandbox::Sandbox;
use crate::{SandboxError, SandboxOutput};

/// A sandboxed command, mirroring the familiar `std::process::Command` API.
pub struct SandboxCommand<'a> {
    sandbox: &'a Sandbox,
    program: OsString,
    args: Vec<OsString>,
    env_clear: bool,
    envs: BTreeMap<OsString, OsString>,
    removals: Vec<OsString>,
    current_dir: Option<PathBuf>,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
}

impl<'a> SandboxCommand<'a> {
    pub(crate) fn new(sandbox: &'a Sandbox, program: &OsStr) -> Self {
        Self {
            sandbox,
            program: program.to_os_string(),
            args: Vec::new(),
            env_clear: false,
            envs: BTreeMap::new(),
            removals: Vec::new(),
            current_dir: None,
            stdin: None,
            stdout: None,
            stderr: None,
        }
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_os_string());
        self
    }

    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        self.args
            .extend(args.into_iter().map(|arg| arg.as_ref().to_os_string()));
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.envs
            .insert(key.as_ref().to_os_string(), value.as_ref().to_os_string());
        self
    }

    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.removals.push(key.as_ref().to_os_string());
        self
    }

    pub fn env_clear(&mut self) -> &mut Self {
        self.env_clear = true;
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<OsStr>) -> &mut Self {
        self.current_dir = Some(dir.as_ref().to_os_string().into());
        self
    }

    pub fn stdin(&mut self, stdio: Stdio) -> &mut Self {
        self.stdin = Some(stdio);
        self
    }

    pub fn stdout(&mut self, stdio: Stdio) -> &mut Self {
        self.stdout = Some(stdio);
        self
    }

    pub fn stderr(&mut self, stdio: Stdio) -> &mut Self {
        self.stderr = Some(stdio);
        self
    }

    pub fn spawn(&mut self) -> Result<SandboxChild, SandboxError> {
        crate::backend::spawn(
            self.sandbox,
            self.program.clone(),
            self.args.clone(),
            self.env_clear,
            self.envs.clone(),
            self.removals.clone(),
            self.current_dir.clone(),
            self.stdin.unwrap_or(Stdio::inherit()),
            self.stdout.unwrap_or(Stdio::inherit()),
            self.stderr.unwrap_or(Stdio::inherit()),
        )
    }

    /// Run the command and capture stdout/stderr.
    pub fn output(&mut self) -> Result<SandboxOutput, SandboxError> {
        let mut child = self.spawn()?;
        child.wait_with_output()
    }
}
