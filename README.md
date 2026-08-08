# sandboxrs-windows

`sandboxrs-windows` is a no-admin Rust library for launching Windows child
processes with constrained filesystem authority. It prefers Windows' modern
composable sandbox API when available and falls back to AppContainer.

```rust
use sandboxrs_windows::{Sandbox, Stdio};

let sandbox = Sandbox::builder(r"C:\repo")
    .read_only(r"C:\Users\me\.rustup")
    .read_write(r"C:\temp\sandboxrs")
    .build()?;

let output = sandbox
    .command("cargo")
    .arg("test")
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()?;
```

The library is the product. `sandboxrs.exe` is a thin adapter for Node, Python,
Java, Go, and shell scripts that invokes the same library.

## Status

The modern `Experimental_CreateProcessInSandbox` backend is implemented:

- Public Rust API: `SandboxBuilder`, `Sandbox`, `SandboxCommand`, `SandboxChild`,
  `SandboxOutput`, `BackendKind`, diagnostics.
- Private `FilesystemPlan`: normalization, duplicate/conflict validation, and
  more-specific-wins overlap rules.
- `SandboxSpec` FlatBuffer compilation (schema version `0.1.0`, file identifier
  `SBOX`) from the validated filesystem plan.
- Dynamic `processmodel.dll` loading from System32, `Experimental_QuerySandboxSupport`
  capability checks, and a real M0 probe that launches `cmd /c exit 0` and
  verifies an outside-write is denied without admin.
- Suspended process creation, Job Object assignment before resume, process/memory
  limits, `KILL_ON_JOB_CLOSE`, timeout-driven tree termination, and explicit kill.
- Piped/inherit/null stdio through `STARTUPINFO`, wide environment blocks with
  `CREATE_UNICODE_ENVIRONMENT`, and a downlevel retry without the environment
  block when the API rejects it.
- Fail-closed backend selection. There is no silent `std::process::Command`
  fallback.

The backend must still be empirically validated on the target Windows 11
machine/VDI; the contract tests are present but `#[ignore]`d until that M0 gate
passes. The AppContainer fallback is scheduled for M3 and is intentionally not
advertised until it can pass the shared contract suite. On non-Windows hosts,
`SandboxBuilder::build()` fails closed with `UnsupportedPlatform`.

## Commands

```powershell
cargo build --release
cargo run --bin sandboxrs -- doctor
cargo run --bin sandboxrs -- exec --workspace C:\repo -- cargo test
cargo run --bin sandboxrs -- exec --json --workspace C:\repo -- cargo check
```

## Testing on Windows

The shared contract tests are present but `#[ignore]`d until a backend can
actually launch a sandboxed process. They cover smoke commands, workspace
reads/writes, outside-write denial, reparse-point escapes, timeouts, kills, and
descendant lifecycle.

## Layout

```text
src/
  lib.rs                 public API surface
  builder.rs             SandboxBuilder
  sandbox.rs             reusable Sandbox
  command.rs             SandboxCommand
  child.rs               SandboxChild
  output.rs              SandboxOutput
  error.rs               SandboxError
  filesystem.rs          private FilesystemPlan
  job.rs                 Windows Job Object lifecycle
  backend/
    modern/              experimental Windows sandbox API (private FFI)
    appcontainer/        rappct-based fallback (M3)
  bin/sandboxrs.rs       thin EXE adapter
```

## Threat model

This library is not a promise of perfect containment of arbitrary hostile
binaries. Reparse points, inherited handles, and experimental API instability
are treated as adversarial in the contract tests. The core enforces mechanism
only; callers decide what authority to grant.
