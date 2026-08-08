use std::fs;
use std::path::PathBuf;
use std::process::Command;

use sandboxrs_windows::Sandbox;

pub fn attacker() -> &'static str {
    env!("CARGO_BIN_EXE_sandboxrs-test-attacker")
}

pub fn fresh_workspace(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "sandboxrs-{}-{}-{}",
        name,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    root
}

pub fn workspace_sandbox(workspace: &std::path::Path) -> sandboxrs_windows::Sandbox {
    Sandbox::builder(workspace)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .expect("sandbox should build")
}

pub fn run(
    sandbox: &Sandbox,
    program: &str,
    args: &[&str],
) -> sandboxrs_windows::SandboxOutput {
    run_at(sandbox, std::path::Path::new("."), program, args)
}

pub fn run_at(
    sandbox: &Sandbox,
    cwd: &std::path::Path,
    program: &str,
    args: &[&str],
) -> sandboxrs_windows::SandboxOutput {
    let mut command = sandbox.command(program);
    command.args(args);
    command.current_dir(cwd);
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    command.output().expect("sandboxed command should run")
}

pub fn expect_denied(output: &sandboxrs_windows::SandboxOutput) {
    assert!(
        !output.status.success(),
        "operation should have been denied: {:?}",
        output.status
    );
}

pub fn native_writes(path: &std::path::Path) {
    let output = Command::new(attacker())
        .arg("write")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "native helper should be able to write: {:?}",
        output
    );
}

pub fn cleanup(root: PathBuf) {
    let _ = fs::remove_dir_all(root);
}
