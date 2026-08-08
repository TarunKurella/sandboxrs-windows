use std::io::Read;
use std::thread;
use std::time::{Duration, Instant};

use crate::output::SandboxOutput;
use crate::{BackendKind, SandboxError};

/// A spawned sandboxed process tree.
pub struct SandboxChild {
    backend: BackendKind,
    started: Instant,
    timeout: Option<Duration>,
    #[cfg(windows)]
    child: Option<std::process::Child>,
    #[cfg(windows)]
    job: Option<crate::job::Job>,
    #[cfg(not(windows))]
    _marker: (),
}

impl SandboxChild {
    #[cfg(windows)]
    pub(crate) fn new(
        backend: BackendKind,
        child: std::process::Child,
        job: crate::job::Job,
        timeout: Option<Duration>,
    ) -> Self {
        Self {
            backend,
            started: Instant::now(),
            timeout,
            child: Some(child),
            job: Some(job),
        }
    }

    #[cfg(not(windows))]
    pub(crate) fn unsupported(backend: BackendKind) -> Self {
        Self {
            backend,
            started: Instant::now(),
            timeout: None,
            _marker: (),
        }
    }

    pub fn id(&self) -> Option<u32> {
        #[cfg(windows)]
        {
            self.child.as_ref().map(|child| child.id())
        }
        #[cfg(not(windows))]
        {
            let _ = self.backend;
            None
        }
    }

    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>, SandboxError> {
        #[cfg(windows)]
        {
            let child = self.child.as_mut().ok_or(no_child())?;
            Ok(child.try_wait()?)
        }
        #[cfg(not(windows))]
        {
            let _ = self.backend;
            Err(SandboxError::UnsupportedPlatform)
        }
    }

    pub fn wait(&mut self) -> Result<std::process::ExitStatus, SandboxError> {
        #[cfg(windows)]
        {
            let child = self.child.as_mut().ok_or(no_child())?;
            let deadline = self.timeout.map(|timeout| Instant::now() + timeout);
            loop {
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    if let Some(job) = &self.job {
                        job.terminate()?;
                    }
                    return Ok(child.wait()?);
                }
                thread::sleep(Duration::from_millis(25));
            }
        }
        #[cfg(not(windows))]
        {
            let _ = self.backend;
            Err(SandboxError::UnsupportedPlatform)
        }
    }

    pub fn kill(&mut self) -> Result<(), SandboxError> {
        #[cfg(windows)]
        {
            if let Some(job) = &self.job {
                let _ = job.terminate();
            }
            let child = self.child.as_mut().ok_or(no_child())?;
            Ok(child.kill()?)
        }
        #[cfg(not(windows))]
        {
            let _ = self.backend;
            Err(SandboxError::UnsupportedPlatform)
        }
    }

    pub fn wait_with_output(&mut self) -> Result<SandboxOutput, SandboxError> {
        #[cfg(windows)]
        {
            let backend = self.backend;
            let started = self.started;
            let timeout = self.timeout;
            let job = self.job.take();
            let mut child = self.child.take().ok_or(no_child())?;

            let stdout_reader = child
                .stdout
                .take()
                .map(|mut pipe| thread::spawn(move || read_all(&mut pipe)));
            let stderr_reader = child
                .stderr
                .take()
                .map(|mut pipe| thread::spawn(move || read_all(&mut pipe)));

            let deadline = timeout.map(|timeout| Instant::now() + timeout);
            let status = loop {
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    if let Some(job) = &job {
                        job.terminate()?;
                    }
                    break child.wait()?;
                }
                thread::sleep(Duration::from_millis(25));
            };

            let stdout = stdout_reader
                .map(|reader| reader.join().unwrap_or_default())
                .unwrap_or_default();
            let stderr = stderr_reader
                .map(|reader| reader.join().unwrap_or_default())
                .unwrap_or_default();

            Ok(SandboxOutput::from_output(
                std::process::Output {
                    status,
                    stdout,
                    stderr,
                },
                backend,
                started.elapsed(),
            ))
        }
        #[cfg(not(windows))]
        {
            let _ = self.backend;
            Err(SandboxError::UnsupportedPlatform)
        }
    }
}

#[cfg(windows)]
fn read_all(pipe: &mut impl Read) -> Vec<u8> {
    let mut bytes = Vec::new();
    let _ = pipe.read_to_end(&mut bytes);
    bytes
}

#[cfg(windows)]
fn no_child() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "sandbox child is no longer available",
    )
}
