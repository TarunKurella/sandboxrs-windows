# sandboxrs-windows

`sandboxrs-windows` is a no-admin Rust library for launching Windows child
processes with constrained filesystem authority. It prefers Windows' modern
composable sandbox API when available and falls back to AppContainer.

```rust
use sandboxrs_windows::Sandbox;

let sandbox = Sandbox::builder(r"C:\repo")
    .read_only(r"C:\Users\me\.rustup")
    .read_write(r"C:\temp\sandboxrs")
    .build()?;

let output = sandbox
    .command("cargo")
    .arg("test")
    .output()?;
```

The library is the product. `sandboxrs.exe` is a thin adapter for Node, Python,
Java, Go, and shell scripts that invokes the same library.

## Status

This repository is the M0/M1 scaffold:

- Public Rust API: `SandboxBuilder`, `Sandbox`, `SandboxCommand`, `SandboxChild`,
  `SandboxOutput`, `BackendKind`, diagnostics.
- Private `FilesystemPlan`: normalization, duplicate/conflict validation, and
  more-specific-wins overlap rules.
- Job Object lifecycle wrapper for descendant containment, process limits, and
  memory limits.
- Fail-closed backend selection. There is no silent `std::process::Command`
  fallback.

The experimental `Experimental_CreateProcessInSandbox` backend currently
resolves the DLL export during probing, but the M0 launch probe still needs to
be proven on a real Windows machine before `build()` can select it. The
AppContainer backend is scheduled for M3 and is intentionally not advertised
until it can pass the shared contract suite.

Until M0/M1 land on Windows, `SandboxBuilder::build()` fails closed with
`SandboxUnavailable` on Windows and `UnsupportedPlatform` on other platforms.

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
