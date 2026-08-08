# PLAN.md — sandboxrs-windows

## 0. One-line goal

Build a **Rust-native, no-admin Windows process sandbox library** that feels close to `std::process::Command`.

Backend order:

1. **Primary:** `Experimental_CreateProcessInSandbox` when present and usable.
2. **Fallback:** regular AppContainer via `rappct`.

Secondary deliverable:

```text
sandboxrs.exe
```

as a thin wrapper for Node, Python, Java, Go, shell scripts, and other runtimes.

The product is a **process sandbox library**. It is not an agent framework.

---

# 1. Product definition

## 1.1 Cargo package

```text
sandboxrs-windows
```

Rust import:

```rust
use sandboxrs_windows::Sandbox;
```

Binary:

```text
sandboxrs.exe
```

## 1.2 Primary user

A Rust developer who currently writes:

```rust
std::process::Command::new("cargo")
    .arg("test")
    .current_dir(repo)
    .spawn()?;
```

and wants:

```rust
let sandbox = Sandbox::builder(repo)
    .build()?;

let output = sandbox
    .command("cargo")
    .arg("test")
    .output()?;
```

without understanding:

- AppContainer SIDs,
- ACL grants,
- `STARTUPINFOEX`,
- Job Objects,
- experimental Windows sandbox structs,
- backend selection.

## 1.3 Secondary user

A non-Rust process that invokes:

```powershell
sandboxrs.exe exec --workspace C:\repo -- cargo test
```

and optionally consumes machine-readable JSON.

---

# 2. Design principles

## 2.1 Rust-native first

The Rust library is the product.

The EXE is only an adapter over the same library.

## 2.2 Mechanism, not agent policy

Borrow the Bubblewrap philosophy:

> The sandbox library supplies isolation mechanisms. The caller decides what authority to grant.

The crate understands:

```text
read-only path
read-write path
environment
cwd
stdio
timeout
resource limits
```

It does not understand:

```text
LLMs
agents
plans
tools
memories
Git tasks
profiles
workflows
```

## 2.3 Reusable sandbox, separate command

A `Sandbox` represents a validated, reusable authority boundary.

A `SandboxCommand` represents one process execution inside it.

```text
SandboxBuilder
      │
    build()
      ▼
   Sandbox
      │
      ├── command("cargo")
      ├── command("git")
      └── command("python")
```

This allows one validated sandbox to run several commands without rebuilding policy every time.

## 2.4 Fail before execution

`SandboxBuilder::build()` should perform as much validation as practical:

- backend discovery,
- backend probe,
- path validation,
- policy normalization,
- backend capability validation,
- backend-specific setup.

If construction succeeds, command execution should not later discover that the sandbox mechanism itself is unavailable.

## 2.5 Fail closed

If requested isolation cannot be enforced:

```text
return error
```

Never:

```text
warn and run unsandboxed
```

There is no automatic `std::process::Command` fallback.

---

# 3. Non-negotiable constraints

- Windows only for v1.
- Baseline operation must not require administrator privileges.
- No daemon.
- No Windows service.
- No kernel driver.
- No DLL injection.
- No transparent filesystem virtualization.
- No registry virtualization.
- No Git/worktree abstraction in core.
- No agent-specific abstractions in core.
- No Docker dependency.
- No WSL dependency.
- No Hyper-V dependency.
- No Windows Sandbox dependency.
- No machine-wide firewall/WFP setup.
- No local sandbox users.
- No network-isolation requirement for v1.
- Experimental API detection must be runtime capability probing, not OS-version checking.
- No shell-string command API as the primitive.

---

# 4. Public architecture

```text
                   Rust caller
                       │
                       ▼
                SandboxBuilder
                       │
                    build()
                       │
       validate + probe + normalize
                       │
                       ▼
                    Sandbox
              immutable authority
                       │
                 command(...)
                       │
                       ▼
                SandboxCommand
                       │
                    spawn()
                       │
                       ▼
                 SandboxChild
                       │
                  wait/output
```

Internal backend architecture:

```text
                 SandboxBuilder
                       │
                       ▼
             private FilesystemPlan
                       │
               backend selection
                 /           \
                /             \
               ▼               ▼
   Windows Sandbox API     AppContainer
        backend            rappct backend
                \             /
                 \           /
                  ▼         ▼
                   Job Object
                       │
                       ▼
             child + descendants
```

---

# 5. Core public types

Keep the public surface intentionally small.

```rust
pub struct SandboxBuilder { /* private */ }

pub struct Sandbox { /* private */ }

pub struct SandboxCommand<'a> { /* private */ }

pub struct SandboxChild { /* private */ }

pub struct SandboxOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub backend: BackendKind,
    pub duration: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    WindowsSandboxApi,
    AppContainer,
}
```

Errors:

```rust
pub enum SandboxError {
    SandboxUnavailable,
    BackendProbeFailed { /* ... */ },
    UnsupportedPolicy { /* ... */ },
    InvalidPath { /* ... */ },
    PolicyCompileFailed { /* ... */ },
    ProcessCreationFailed { /* ... */ },
    Timeout,
    Io(std::io::Error),
}
```

Do not expose backend-specific Windows structures.

---

# 6. Public Rust API

## 6.1 Default flow

```rust
use sandboxrs_windows::Sandbox;

let sandbox = Sandbox::builder(r"C:\repo")
    .build()?;

let output = sandbox
    .command("cargo")
    .arg("test")
    .output()?;
```

## 6.2 Explicit filesystem roots

```rust
let sandbox = Sandbox::builder(r"C:\repo")
    .read_only(r"C:\Users\me\.rustup")
    .read_only(r"C:\Program Files")
    .read_write(r"C:\temp\sandboxrs")
    .build()?;
```

Then reuse:

```rust
sandbox.command("cargo")
    .arg("check")
    .output()?;

sandbox.command("cargo")
    .arg("test")
    .output()?;
```

## 6.3 Command API

Keep names familiar to `std::process::Command`:

```rust
sandbox
    .command("cargo")
    .arg("test")
    .args(["--workspace"])
    .env("RUST_BACKTRACE", "1")
    .env_clear()
    .current_dir(r"C:\repo")
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;
```

No primitive like:

```rust
sandbox.wrap("cargo test && echo hi")
```

If callers want a shell:

```rust
sandbox
    .command("powershell.exe")
    .args(["-Command", "..."])
```

The library preserves:

```text
program
argv[]
environment
cwd
stdio
```

all the way to process creation.

---

# 7. SandboxBuilder semantics

`SandboxBuilder` describes authority and setup.

Suggested v1 methods:

```rust
Sandbox::builder(workspace)

    .read_only(path)
    .read_write(path)

    .timeout(duration)
    .max_memory(bytes)
    .max_processes(count)

    .preferred_backend(BackendPreference::Auto)

    .build()
```

Environment and cwd belong primarily to `SandboxCommand`, not `SandboxBuilder`.

Reason:

A sandbox is reusable across commands; each command may have different environment/cwd.

The builder may later gain sandbox-wide defaults, but avoid that in v1 unless needed.

---

# 8. `build()` contract

`build()` is a real initialization boundary.

It should:

```text
1. validate workspace
2. normalize filesystem rules
3. probe/select backend
4. verify requested semantics are representable
5. perform backend setup
6. construct Job/lifecycle state if appropriate
7. return immutable Sandbox
```

If any required step fails, no `Sandbox` value exists.

Conceptually:

```rust
let sandbox = Sandbox::builder(repo).build()?;

// at this point:
// - a supported backend exists
// - requested filesystem policy is valid
// - backend setup has succeeded
```

This is preferred over a global singleton manager.

Multiple independent sandboxes should coexist:

```rust
let a = Sandbox::builder(repo_a).build()?;
let b = Sandbox::builder(repo_b).build()?;
```

The only reasonable global state is immutable cached backend probing:

```rust
OnceLock<BackendProbeResult>
```

---

# 9. Private FilesystemPlan

This is the most important internal abstraction.

Do not send raw user path rules directly into process creation.

Compile first.

```rust
struct FilesystemPlan {
    roots: Vec<PathRule>,
}

struct PathRule {
    path: PathBuf,
    access: Access,
}

enum Access {
    Hidden,
    ReadOnly,
    ReadWrite,
}
```

`Hidden` may remain internal in v1 even if the public API only exposes RO/RW.

Why keep it internally?

Because both backends ultimately enforce a view where unspecified resources are not granted, and future policy evolution should not require redesigning the internal representation.

Pipeline:

```text
builder input
    ↓
normalize
    ↓
validate overlap
    ↓
FilesystemPlan
    ↓
backend compiler
    ↓
OS enforcement
```

Path-policy resolution and process spawning must remain separate.

---

# 10. Filesystem policy

## 10.1 v1 public rules

Expose only:

```rust
.read_only(path)
.read_write(path)
```

The workspace passed to:

```rust
Sandbox::builder(workspace)
```

is automatically `ReadWrite`.

Everything else is backend-default inaccessible unless required by the backend/Windows baseline.

## 10.2 Path normalization

Before backend compilation:

- normalize Windows separators,
- compare case-insensitively where required,
- reject empty paths,
- reject malformed paths,
- resolve duplicates,
- identify overlapping roots,
- canonicalize only where safe,
- preserve enough original path information for diagnostics.

## 10.3 Overlap rule

Use:

> More specific path wins.

Example:

```text
C:\repo             RW
C:\repo\.readonly   RO
```

If a backend cannot faithfully enforce a specific overlap:

```text
return UnsupportedPolicy
```

Never silently flatten authority.

## 10.4 Reparse points

Treat these as adversarial:

- symlinks,
- junctions,
- directory reparse points.

Lexical containment is not security containment.

Shared tests must attempt workspace escape using reparse points.

---

# 11. Backend selection

Do not inspect:

```text
Windows 11 build >= X
```

Instead:

```text
1. Load processmodel.dll.
2. Resolve Experimental_CreateProcessInSandbox.
3. Perform a minimal real sandbox launch.
4. If successful, select modern backend.
5. Otherwise probe AppContainer fallback.
6. If neither succeeds, SandboxUnavailable.
```

Pseudo-code:

```rust
fn select_backend() -> Result<BackendKind> {
    if modern::probe().is_ok() {
        return Ok(BackendKind::WindowsSandboxApi);
    }

    if appcontainer::probe().is_ok() {
        return Ok(BackendKind::AppContainer);
    }

    Err(SandboxError::SandboxUnavailable)
}
```

Cache backend probe results for the process lifetime.

An exported symbol is not sufficient proof of usability.

Corporate policy or schema incompatibility may still make the call fail.

---

# 12. Backend A — modern Windows sandbox API

## 12.1 Role

Preferred backend.

Map:

```text
RW roots → fs_read_write
RO roots → fs_read_only
```

Use AppContainer-backed sandboxing provided through the modern API.

## 12.2 Dynamic-link boundary

All experimental FFI stays inside:

```text
src/backend/modern/
```

Suggested layout:

```text
modern/
├─ mod.rs
├─ ffi.rs
├─ probe.rs
├─ compile.rs
└─ spawn.rs
```

Responsibilities:

- dynamic DLL load,
- symbol resolution,
- minimal FFI definitions,
- sandbox-spec construction,
- schema/version validation,
- process creation,
- error conversion.

No Microsoft experimental type appears in public Rust APIs.

## 12.3 API instability

If Microsoft:

- renames the API,
- changes schema,
- publishes a stable replacement,

only this backend should change.

Callers remain on:

```rust
Sandbox
SandboxCommand
SandboxChild
```

---

# 13. Backend B — regular AppContainer via rappct

## 13.1 Role

Fallback when modern backend is absent or unusable.

Use **regular AppContainer**, not LPAC, in v1.

Why:

- better Windows compatibility,
- no-admin baseline,
- read/write isolation,
- avoids another restricted-token security model,
- simpler semantic match with the modern backend.

## 13.2 Isolation boundary

All `rappct` code stays in:

```text
src/backend/appcontainer/
```

Suggested layout:

```text
appcontainer/
├─ mod.rs
├─ probe.rs
├─ compile.rs
└─ spawn.rs
```

It owns:

- AppContainer profile lifecycle,
- SID handling,
- grants needed for explicit roots,
- secure launch,
- cleanup,
- Job integration where needed.

Do not leak `rappct` types publicly.

## 13.3 Fallback quality rule

The fallback is meaningful only if it can enforce the requested public contract.

If a specific rule cannot be represented safely:

```text
UnsupportedPolicy
```

Do not degrade semantics silently.

---

# 14. Process-tree lifecycle is part of the sandbox

This is not merely a resource-limit feature.

A command may produce:

```text
cargo
 ├─ rustc
 ├─ linker
 ├─ build.rs.exe
 └─ test.exe
```

The entire descendant tree belongs to one sandboxed execution.

Use a Job Object for lifecycle control.

Minimum behavior:

```text
KILL_ON_JOB_CLOSE
process-count limit
optional memory limit
timeout-driven termination
```

## 14.1 Parent death / object teardown

The desired property is analogous to Bubblewrap's "die with parent":

> The sandbox must not leave arbitrary descendants alive when its owning execution is torn down.

Exact Rust `Drop` behavior must be designed carefully.

Do not block for long periods in `Drop`.

But ownership must be explicit:

```text
SandboxChild
      │
      └── owns process-tree containment
```

## 14.2 Race avoidance

Where possible, attach to the Job at process creation or before untrusted code can meaningfully execute.

Avoid a known window where a child can escape before assignment.

---

# 15. Process I/O

Support standard Rust process patterns:

```rust
Stdio::inherit()
Stdio::piped()
Stdio::null()
```

Required v1 result:

```rust
pub struct SandboxOutput {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub backend: BackendKind,
    pub duration: Duration,
}
```

No terminal emulation in v1.

No ConPTY in v1.

No interactive RPC in v1.

---

# 16. Environment

The core sandbox does not invent a virtual HOME or language-specific cache setup.

Each `SandboxCommand` supports:

```rust
.env(key, value)
.env_clear()
```

The caller may choose:

```text
TEMP
TMP
HOME
USERPROFILE
CARGO_HOME
```

but the sandbox library does not automatically apply toolchain opinions in v1.

This keeps mechanism separate from compatibility policy.

---

# 17. Errors

Errors must remain useful to systems programmers.

Preserve:

- backend,
- operation,
- Win32/HRESULT/NTSTATUS information where relevant,
- affected path where relevant.

Example shape:

```rust
pub enum SandboxError {
    SandboxUnavailable,

    BackendProbeFailed {
        backend: BackendKind,
        source: Box<dyn Error + Send + Sync>,
    },

    UnsupportedPolicy {
        backend: BackendKind,
        feature: &'static str,
    },

    InvalidPath {
        path: PathBuf,
        reason: String,
    },

    PolicyCompileFailed {
        backend: BackendKind,
        reason: String,
    },

    ProcessCreationFailed {
        backend: BackendKind,
        win32_code: Option<u32>,
        message: String,
    },

    Timeout,

    Io(std::io::Error),
}
```

Avoid context-free:

```text
Access denied
```

Prefer:

```text
AppContainer backend failed granting RO access to C:\foo
Win32 error: 5 (ACCESS_DENIED)
```

---

# 18. Diagnostics

Keep diagnostics intentionally small.

Library:

```rust
Sandbox::probe()
Sandbox::available_backends()
sandbox.backend()
```

CLI:

```powershell
sandboxrs.exe doctor
```

Example:

```text
sandboxrs-windows 0.1.0

Windows Sandbox API
  export      yes
  probe       success

AppContainer
  probe       success

Selected
  windows-sandbox-api

Admin required
  no
```

No automatic policy learning.

No ProcMon clone.

---

# 19. Binary

The EXE is secondary and thin.

```text
Node/Python/Java/Go
       │
       ▼
 sandboxrs.exe
       │
       ▼
sandboxrs-windows crate
```

## 19.1 Human mode

```powershell
sandboxrs.exe exec `
  --workspace C:\repo `
  --ro C:\Users\me\.rustup `
  --rw C:\temp\sandboxrs `
  -- cargo test
```

## 19.2 Machine mode

```powershell
sandboxrs.exe exec --json ...
```

Result after completion:

```json
{
  "backend": "windows-sandbox-api",
  "exit_code": 0,
  "duration_ms": 1831,
  "stdout": "...",
  "stderr": ""
}
```

v1 does not need streaming JSONL.

## 19.3 Do not corrupt child stdout

Even in v1, keep one invariant:

> Child stdout/stderr are child data. Sandbox metadata is separate structured output.

Do not mix lines like:

```text
{"sandbox_started":true}
Compiling foo
{"exit":0}
```

into a single stream.

This matters for future protocol compatibility.

---

# 20. Non-Rust runtimes

Do not ship SDKs in v1.

Document:

```text
spawn sandboxrs.exe
      +
--json
```

This is enough for:

- Node,
- Python,
- Java,
- Go,
- shell scripts.

Bindings can be added later only if real use demands lower latency or richer process control.

---

# 21. Repository layout

```text
sandboxrs-windows/
├─ Cargo.toml
├─ README.md
├─ PLAN.md
├─ FUTUREPLAN.md
├─ LICENSE
├─ src/
│  ├─ lib.rs
│  ├─ builder.rs
│  ├─ sandbox.rs
│  ├─ command.rs
│  ├─ child.rs
│  ├─ output.rs
│  ├─ error.rs
│  ├─ job.rs
│  ├─ filesystem.rs
│  ├─ backend/
│  │  ├─ mod.rs
│  │  ├─ modern/
│  │  │  ├─ mod.rs
│  │  │  ├─ ffi.rs
│  │  │  ├─ probe.rs
│  │  │  ├─ compile.rs
│  │  │  └─ spawn.rs
│  │  └─ appcontainer/
│  │     ├─ mod.rs
│  │     ├─ probe.rs
│  │     ├─ compile.rs
│  │     └─ spawn.rs
│  └─ bin/
│     └─ sandboxrs.rs
├─ tests/
│  ├─ smoke.rs
│  ├─ filesystem.rs
│  ├─ children.rs
│  ├─ timeout.rs
│  ├─ reparse_points.rs
│  └─ backend_contract.rs
└─ examples/
   ├─ cargo_test.rs
   ├─ multiple_commands.rs
   └─ call_from_node.js
```

Do not split into multiple crates until there is real pressure.

---

# 22. Shared backend contract tests

This is critical.

Both backends must run the same behavior suite.

A backend cannot advertise support merely because its API theoretically can do something.

It must pass tests.

## 22.1 Smoke

Must work:

```text
cmd /c echo hello
cargo --version
rustc --version
```

## 22.2 Workspace

Must work:

```text
read workspace file
create workspace file
modify workspace file
child modifies workspace file
grandchild modifies workspace file
```

## 22.3 Outside write

Must fail:

```text
root process writes outside RW roots
child writes outside RW roots
grandchild writes outside RW roots
```

## 22.4 Outside read

Where the backend/public contract claims hidden/ungranted read isolation:

Must fail:

```text
root reads synthetic secret
child reads synthetic secret
grandchild reads synthetic secret
```

Never use real credentials in tests.

## 22.5 Reparse attacks

Attempt:

- workspace junction → outside directory,
- workspace symlink → outside directory,
- nested reparse-point escapes,
- rename/reparse edge cases where practical.

## 22.6 Lifecycle

Test:

- timeout kills descendants,
- explicit kill kills descendants,
- teardown does not leave workers alive,
- process limit works,
- memory limit works where configured.

---

# 23. Hostile helper executable

Create a tiny test-only executable:

```text
sandboxrs-test-attacker.exe
```

Commands:

```text
write <path>
read <path>
spawn-write <path>
spawn-read <path>
spawn-grandchild-write <path>
sleep
allocate-memory
spawn-many
```

This gives deterministic adversarial tests independent of PowerShell quoting or language runtimes.

The test helper is not shipped as part of the public product.

---

# 24. Security review checklist

Before `0.1.0`:

- [ ] No silent unsandboxed execution.
- [ ] Backend detection uses capability probing.
- [ ] Modern experimental API is isolated behind private FFI.
- [ ] `rappct` remains private.
- [ ] `SandboxBuilder::build()` fails before command execution when setup is invalid.
- [ ] `FilesystemPlan` is normalized before backend compilation.
- [ ] Descendant process lifecycle is contained.
- [ ] Outside-write tests pass.
- [ ] Outside-read tests pass where claimed.
- [ ] Junction/symlink escapes are tested.
- [ ] Handle inheritance has been reviewed.
- [ ] Temporary files are created safely.
- [ ] AppContainer grants/profile lifecycle are safe.
- [ ] No baseline path requires admin.
- [ ] No machine-wide network/firewall state is modified.
- [ ] Errors preserve Windows diagnostics.
- [ ] Unsupported semantics fail closed.
- [ ] Multiple `Sandbox` instances can coexist.
- [ ] Multiple commands can reuse one `Sandbox`.

---

# 25. Implementation milestones

## M0 — prove modern API on real machine

Build:

```text
sandbox-probe.exe
```

It must:

- dynamically resolve the API,
- launch `cmd /c exit 0`,
- grant one directory RW,
- prove writing elsewhere fails,
- run without admin.

Exit gate:

> The desired boundary is empirically proven on the target VDI.

## M1 — modern backend + reusable Rust API

Implement:

```rust
let sandbox = Sandbox::builder(repo).build()?;

sandbox.command("cmd")
    .args(["/c", "echo", "hello"])
    .output()?;
```

Deliver:

- builder,
- reusable sandbox,
- command,
- child,
- output,
- private `FilesystemPlan`,
- Job lifecycle,
- modern backend.

No fallback yet.

## M2 — real developer workload

Make this work:

```rust
sandbox.command("cargo")
    .arg("test")
    .output()?;
```

Add explicit RO roots as needed.

Do not add profiles.

Document required paths in examples.

## M3 — AppContainer fallback

Add `rappct`.

Run the same backend contract tests.

Exit gate:

> Calling code does not change when modern API is unavailable.

## M4 — process quality

Add:

- stdout/stderr piping,
- timeout,
- explicit kill,
- memory/process limits,
- robust errors,
- environment APIs,
- diagnostics.

## M5 — EXE

Add:

```text
sandboxrs.exe exec
sandboxrs.exe doctor
```

and:

```text
--json
```

No daemon.
No JSON-RPC.
No language bindings.

## M6 — hardening

Focus on:

- reparse points,
- inherited handles,
- Windows path edge cases,
- cleanup,
- repeated sandbox construction/destruction,
- simultaneous sandboxes,
- child/grandchild behavior,
- backend-equivalence testing.

Then publish `0.1.0`.

---

# 26. Release criteria for 0.1.0

All must be true:

1. A real Rust program can replace a normal `Command` call with `Sandbox`.
2. One `Sandbox` can safely execute several commands.
3. `build()` validates/probes before execution.
4. `cargo test` works through the library.
5. Modern backend works without admin on supported Windows.
6. AppContainer fallback works without admin on a machine where modern backend is unavailable.
7. Both backends pass the shared filesystem/process-tree contract suite.
8. Child/grandchild escape tests pass.
9. No silent security downgrade exists.
10. EXE invokes the exact same library implementation.
11. README documents the threat model and limitations.
12. Experimental API instability is explicit.
13. At least one external sample harness can use it successfully.

---

# 27. README positioning

Do not market:

> Perfectly secure execution of arbitrary hostile Windows binaries.

Prefer:

> `sandboxrs-windows` is a no-admin Rust library for launching Windows child processes with constrained filesystem authority. It prefers Windows' modern composable sandbox API when available and falls back to AppContainer.

Adoption story:

```text
std::process::Command
        ↓
sandboxrs_windows::Sandbox
```

The library can be useful to:

- coding harnesses,
- build systems,
- test runners,
- plugin hosts,
- local automation,
- AI agents.

But none of those domains leak into the core abstraction.

---

# 28. Design guardrail

Before adding a feature, ask:

> Does this directly improve the security, correctness, lifecycle, or ergonomics of spawning constrained Windows child processes from Rust?

If not, it belongs in `FUTUREPLAN.md` or another crate.

---

# 29. Final architecture in one page

```text
Rust caller
    │
    ▼
SandboxBuilder
    │
    │ build()
    ▼
validate
normalize paths
probe backend
compile filesystem plan
    │
    ▼
Sandbox
(reusable immutable authority)
    │
    ├── command("cargo")
    ├── command("git")
    └── command("python")
            │
            ▼
       SandboxCommand
            │
          spawn()
            │
      ┌─────┴─────┐
      ▼           ▼
 Modern API    AppContainer
                 via rappct
      │           │
      └─────┬─────┘
            ▼
        Job Object
            │
            ▼
     child + descendants
```

That is the v1 product.
