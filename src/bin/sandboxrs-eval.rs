use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use sandboxrs_windows::{Sandbox, SandboxOutput};
use serde::Serialize;

#[derive(Serialize, Clone, Default)]
struct EvalReport {
    os: String,
    backends: Vec<BackendInfo>,
    selected: Option<String>,
    admin: bool,
    security_contract: Vec<Check>,
    path_escape: Vec<Check>,
    lifecycle: Vec<Check>,
    compatibility: Vec<Check>,
    environment: Vec<Check>,
    totals: Totals,
}

#[derive(Serialize, Clone)]
struct BackendInfo {
    backend: String,
    export_present: bool,
    usable: bool,
    detail: String,
}

#[derive(Serialize, Clone)]
struct Check {
    name: String,
    expected: String,
    actual: String,
    pass: bool,
}

#[derive(Serialize, Clone, Default)]
struct Totals {
    security_contract: Score,
    path_escape: Score,
    lifecycle: Score,
    compatibility: Score,
    environment: Score,
}

#[derive(Serialize, Clone, Default)]
struct Score {
    passed: usize,
    total: usize,
}

fn main() {
    let report_path = std::env::args()
        .position(|arg| arg == "--report")
        .and_then(|index| std::env::args().nth(index + 1));

    #[cfg(windows)]
    let report = run_windows_evals();
    #[cfg(not(windows))]
    let report = EvalReport {
        os: std::env::consts::OS.to_string(),
        ..Default::default()
    };

    let json = serde_json::to_string_pretty(&report).expect("serialize report");
    if let Some(path) = report_path {
        let _ = std::fs::create_dir_all(PathBuf::from(&path).parent().unwrap_or(Path::new(".")));
        let _ = std::fs::write(path, &json);
    } else {
        println!("{json}");
    }

    let selected = report.selected.as_deref().unwrap_or("");
    if selected.is_empty()
        || report.totals.security_contract.passed != report.totals.security_contract.total
    {
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run_windows_evals() -> EvalReport {
    use std::fs;
    use std::time::{Duration, Instant};

    let probe = Sandbox::probe();
    let backends: Vec<BackendInfo> = probe
        .entries
        .iter()
        .map(|entry| BackendInfo {
            backend: entry.backend.as_str().to_string(),
            export_present: entry.export_present,
            usable: entry.usable,
            detail: entry.detail.clone(),
        })
        .collect();

    let selected = Sandbox::available_backends().first().copied();
    let mut report = EvalReport {
        os: os_version(),
        backends,
        selected: selected.map(|backend| backend.as_str().to_string()),
        admin: running_elevated(),
        ..Default::default()
    };
    let Some(_backend) = selected else {
        return report;
    };

    let fixture = std::env::temp_dir().join(format!(
        "sandboxrs-eval-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let workspace = fixture.join("workspace");
    let readonly = fixture.join("readonly");
    let secret = fixture.join("secret");
    for dir in [&workspace, &readonly, &secret, &workspace.join("nested")] {
        fs::create_dir_all(dir).unwrap();
    }
    fs::write(workspace.join("allowed.txt"), b"allowed").unwrap();
    fs::write(readonly.join("readable.txt"), b"readable").unwrap();
    fs::write(secret.join("DO_NOT_READ.txt"), b"top-secret").unwrap();

    let attacker = std::env::current_exe()
        .ok()
        .and_then(|path| {
            path.parent()
                .map(|dir| dir.join("sandboxrs-test-attacker.exe"))
        })
        .and_then(|path| path.to_str().map(str::to_owned))
        .expect("attacker helper must be built next to the eval binary");
    let sandbox = Sandbox::builder(&workspace)
        .read_only(&readonly)
        .read_only(exe_dir())
        .timeout(Duration::from_secs(20))
        .build()
        .expect("sandbox should build");

    // Control: the same forbidden operations must succeed outside the sandbox.
    let control_write = native_succeeds(
        Command::new(&attacker)
            .arg("write")
            .arg(secret.join("control-pwned.txt")),
    );
    let control_read = native_succeeds(
        Command::new(&attacker)
            .arg("read")
            .arg(secret.join("DO_NOT_READ.txt")),
    );
    report.security_contract.push(Check {
        name: "control: native outside write succeeds".into(),
        expected: "true".into(),
        actual: control_write.to_string(),
        pass: control_write,
    });
    report.security_contract.push(Check {
        name: "control: native secret read succeeds".into(),
        expected: "true".into(),
        actual: control_read.to_string(),
        pass: control_read,
    });

    let mut contract = Vec::new();
    contract.push(expect_run(
        "read workspace",
        true,
        run_cwd(
            &sandbox,
            &attacker,
            &["read", workspace.join("allowed.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "write workspace",
        true,
        run_cwd(
            &sandbox,
            &attacker,
            &[
                "write",
                workspace.join("workspace-write.txt").to_str().unwrap(),
            ],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "delete workspace",
        true,
        run_cwd(
            &sandbox,
            &attacker,
            &[
                "delete",
                workspace.join("workspace-write.txt").to_str().unwrap(),
            ],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "read readonly",
        true,
        run_cwd(
            &sandbox,
            &attacker,
            &["read", readonly.join("readable.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "write readonly",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["write", readonly.join("readable-new.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "delete readonly",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["delete", readonly.join("readable.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "read secret",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["read", secret.join("DO_NOT_READ.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "write secret",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["write", secret.join("pwned.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "child write secret",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &[
                "spawn-write",
                secret.join("child-pwned.txt").to_str().unwrap(),
            ],
            &workspace,
        ),
    ));
    contract.push(expect_run(
        "grandchild write secret",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &[
                "grandchild-write",
                secret.join("grandchild-pwned.txt").to_str().unwrap(),
            ],
            &workspace,
        ),
    ));
    report.security_contract.extend(contract);

    // Path attacks.
    let attacks: Vec<(&str, PathBuf)> = vec![
        ("dotdot relative", PathBuf::from(r"..\secret\pwned.txt")),
        ("workspace dotdot", workspace.join(r"..\secret\pwned.txt")),
        ("absolute path", secret.join("pwned.txt")),
        ("case variation", secret.join("PWNED.TXT")),
        (
            "extended path",
            PathBuf::from(format!(r"\\?\{}", secret.join("pwned.txt").display())),
        ),
        ("relative path", PathBuf::from(r".\..\secret\pwned.txt")),
    ];
    for (name, path) in attacks {
        let output = run_cwd(
            &sandbox,
            &attacker,
            &["write", path.to_str().unwrap()],
            &workspace,
        );
        report.path_escape.push(expect_run(name, false, output));
    }

    let junction = workspace.join("innocent");
    let symlink = workspace.join("symlink-file.txt");
    let _ = fs::remove_dir_all(&junction);
    let _ = fs::remove_file(&symlink);
    let _ = std::os::windows::fs::symlink_dir(&secret, &junction);
    let _ = std::os::windows::fs::symlink_file(secret.join("DO_NOT_READ.txt"), &symlink);
    report.path_escape.push(expect_run(
        "junction write",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["write", junction.join("pwned.txt").to_str().unwrap()],
            &workspace,
        ),
    ));
    report.path_escape.push(expect_run(
        "symlink read",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["read", symlink.to_str().unwrap()],
            &workspace,
        ),
    ));
    let nested_junction = workspace.join("nested").join("escape");
    let _ = std::os::windows::fs::symlink_dir(&secret, &nested_junction);
    report.path_escape.push(expect_run(
        "nested junction write",
        false,
        run_cwd(
            &sandbox,
            &attacker,
            &["write", nested_junction.join("pwned.txt").to_str().unwrap()],
            &workspace,
        ),
    ));

    // Lifecycle containment.
    let pidfile = fixture.join("orphan.pid");
    let mut orphan = sandbox
        .command(&attacker)
        .arg("spawn-child-sleep")
        .arg(&pidfile)
        .stdout(sandboxrs_windows::Stdio::null())
        .stderr(sandboxrs_windows::Stdio::null())
        .spawn()
        .expect("spawn orphan helper");
    std::thread::sleep(Duration::from_millis(800));
    let child_pid: u32 = fs::read_to_string(&pidfile)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0);
    orphan.kill().expect("kill sandbox");
    let _ = orphan.wait();
    let orphan_gone = child_pid == 0 || !process_exists(child_pid);
    report.lifecycle.push(Check {
        name: "kill terminates descendant".into(),
        expected: "true".into(),
        actual: orphan_gone.to_string(),
        pass: orphan_gone,
    });

    let timeout_sandbox = Sandbox::builder(&workspace)
        .read_only(exe_dir())
        .timeout(Duration::from_secs(2))
        .build()
        .expect("timeout sandbox");
    let start = Instant::now();
    let timeout_out = run_cwd(&timeout_sandbox, &attacker, &["sleep", "120"], &workspace);
    let timed_out = !timeout_out.0.success() && start.elapsed() < Duration::from_secs(10);
    report.lifecycle.push(Check {
        name: "timeout terminates process".into(),
        expected: "true".into(),
        actual: format!("status={} elapsed={:?}", timeout_out.0, start.elapsed()),
        pass: timed_out,
    });

    let bomb_sandbox = Sandbox::builder(&workspace)
        .read_only(exe_dir())
        .max_processes(16)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("bomb sandbox");
    let bomb = run_cwd(
        &bomb_sandbox,
        &attacker,
        &["spawn-many", "1000"],
        &workspace,
    );
    let bomb_contained = !bomb.0.success();
    report.lifecycle.push(Check {
        name: "process limit contains bomb".into(),
        expected: "true".into(),
        actual: format!("status={}", bomb.0),
        pass: bomb_contained,
    });

    let memory_sandbox = Sandbox::builder(&workspace)
        .read_only(exe_dir())
        .max_memory(256 * 1024 * 1024)
        .timeout(Duration::from_secs(20))
        .build()
        .expect("memory sandbox");
    let memory = run_cwd(
        &memory_sandbox,
        &attacker,
        &["allocate-memory", "2048"],
        &workspace,
    );
    let memory_contained = !memory.0.success();
    report.lifecycle.push(Check {
        name: "memory limit terminates allocation".into(),
        expected: "true".into(),
        actual: format!("status={}", memory.0),
        pass: memory_contained,
    });

    // Developer compatibility.
    let compat_roots = [
        std::env::var_os("CARGO_HOME").map(PathBuf::from),
        std::env::var_os("RUSTUP_HOME").map(PathBuf::from),
        std::env::var_os("USERPROFILE").map(PathBuf::from),
        std::env::var_os("ProgramFiles").map(PathBuf::from),
    ];
    let mut compat_builder = Sandbox::builder(&workspace).timeout(Duration::from_secs(30));
    for root in compat_roots.into_iter().flatten() {
        compat_builder = compat_builder.read_only(root);
    }
    let compat_sandbox = compat_builder.build().expect("compat sandbox");
    for (name, program, args) in [
        ("cmd", "cmd", vec!["/c", "echo", "hello"]),
        ("git", "git", vec!["--version"]),
        ("node", "node", vec!["--version"]),
        ("python", "python", vec!["--version"]),
        ("cargo", "cargo", vec!["--version"]),
    ] {
        if !native_succeeds(Command::new(program).arg(args[0])) {
            continue;
        }
        let output = run_cwd(&compat_sandbox, program, &args, &workspace);
        report.compatibility.push(expect_run(name, true, output));
    }

    // Environment leakage.
    std::env::set_var("SANDBOXRS_TEST_SECRET", "super-secret-value");
    let mut env_cleared = compat_sandbox.command(attacker);
    env_cleared
        .env_clear()
        .args(["env", "SANDBOXRS_TEST_SECRET"])
        .stdout(sandboxrs_windows::Stdio::piped())
        .stderr(sandboxrs_windows::Stdio::piped());
    let leaked = env_cleared.output();
    let leak_blocked = match leaked {
        Ok(output) => {
            !output.status.success()
                || !String::from_utf8_lossy(&output.stdout).contains("super-secret-value")
        }
        Err(_) => false,
    };
    report.environment.push(Check {
        name: "env_clear removes secret".into(),
        expected: "true".into(),
        actual: format!("leak_blocked={leak_blocked}"),
        pass: leak_blocked,
    });

    let _ = fs::remove_dir_all(&fixture);
    report.totals = Totals {
        security_contract: score(&report.security_contract),
        path_escape: score(&report.path_escape),
        lifecycle: score(&report.lifecycle),
        compatibility: score(&report.compatibility),
        environment: score(&report.environment),
    };
    report
}

#[cfg(windows)]
fn run_cwd(
    sandbox: &Sandbox,
    program: &str,
    args: &[&str],
    cwd: &Path,
) -> (std::process::ExitStatus, String) {
    let output = run_output(sandbox, program, args, cwd);
    (
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
    )
}

#[cfg(windows)]
fn run_output(sandbox: &Sandbox, program: &str, args: &[&str], cwd: &Path) -> SandboxOutput {
    let mut command = sandbox.command(program);
    command.args(args);
    command.current_dir(cwd);
    command
        .stdout(sandboxrs_windows::Stdio::piped())
        .stderr(sandboxrs_windows::Stdio::piped());
    command.output().expect("sandboxed command should run")
}

#[cfg(windows)]
fn expect_run(
    name: &str,
    expected_success: bool,
    output: (std::process::ExitStatus, String),
) -> Check {
    let pass = output.0.success() == expected_success;
    Check {
        name: name.into(),
        expected: if expected_success {
            "success"
        } else {
            "denied"
        }
        .into(),
        actual: format!("status={} stdout={:?}", output.0, output.1),
        pass,
    }
}

#[cfg(windows)]
fn native_succeeds(command: &mut Command) -> bool {
    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn score(checks: &[Check]) -> Score {
    Score {
        passed: checks.iter().filter(|check| check.pass).count(),
        total: checks.len(),
    }
}

#[cfg(windows)]
fn process_exists(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return false;
    }
    let mut code = 0u32;
    let ok = unsafe { GetExitCodeProcess(handle, &mut code) };
    let active = ok != 0 && code == STILL_ACTIVE as u32;
    unsafe {
        CloseHandle(handle);
    }
    active
}

#[cfg(windows)]
fn running_elevated() -> bool {
    Command::new("net")
        .arg("session")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(windows)]
fn os_version() -> String {
    Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .unwrap_or_else(|_| std::env::consts::OS.to_string())
}
