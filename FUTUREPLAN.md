# FUTUREPLAN.md — sandboxrs-windows

This file contains **deliberately deferred** ideas.

Nothing here is required for v1.

Promotion rule:

> A feature moves into `PLAN.md` only when a real compatibility issue, security gap, performance problem, or repeated user request proves it is needed.

"Cool" is not sufficient.

---

# 1. Stable Microsoft sandbox API

If Microsoft replaces the current experimental entry point with a stable API:

```text
StableSandboxApi
      ↓
ExperimentalSandboxApi
      ↓
AppContainer
```

Keep the public Rust API unchanged.

Only backend internals should move.

---

# 2. Formal backend capability model

If backend differences become important to callers:

```rust
pub struct BackendCapabilities {
    pub read_isolation: bool,
    pub write_isolation: bool,
    pub network_isolation: bool,
    pub ui_isolation: bool,
    pub resource_limits: bool,
}
```

Then callers may require:

```rust
SandboxRequirements {
    read_isolation: true,
    write_isolation: true,
}
```

Do not expose this until needed.

---

# 3. Restricted-token fallback

Possible future third backend:

```text
CreateRestrictedToken
+
WRITE_RESTRICTED
+
restricting SID
+
Job Object
```

Value:

- no admin,
- excellent developer-tool compatibility,
- broad reads with constrained writes.

Cost:

- weaker read isolation,
- different security semantics,
- larger maintenance/test matrix.

Only add if AppContainer fallback proves unusable in real deployments.

If added, advertise its weaker capabilities explicitly.

---

# 4. LPAC mode

Potential strict mode:

```rust
Sandbox::builder(repo)
    .isolation(Isolation::Strict)
```

Possible implementation:

```text
LPAC
```

Good for:

- parsers,
- high-risk helper binaries,
- less compatibility-sensitive workloads.

Not v1 because regular AppContainer is a better fallback for developer tools.

---

# 5. Network isolation

Possible API:

```rust
Network::Inherit
Network::None
Network::Proxy(...)
Network::AllowList(...)
```

Possible implementation mechanisms:

- modern API capabilities,
- brokered proxy,
- optional enterprise/admin mode using WFP,
- outer VM policy.

Rule:

> Never claim strong network isolation using only proxy environment variables.

---

# 6. UI / Win32k hardening

Potential headless mode:

```rust
.headless(true)
```

Could compile to:

- Win32k disable,
- UI restrictions,
- maybe private desktop.

Only add after compatibility/security evidence.

---

# 7. Toolchain profiles

Profiles remain an interesting optional layer, not core v1.

Examples:

```text
Rust
Node
Python
Java
.NET
Go
```

A profile answers:

> Which extra read-only resources does this toolchain commonly require?

Possible API:

```rust
Sandbox::builder(repo)
    .profile(Profile::Rust)
```

Correct separation:

```text
profile = compatibility knowledge
sandbox = authority mechanism
```

A profile must never silently weaken requested isolation.

If profiles grow, prefer a companion crate:

```text
sandboxrs-profiles
```

instead of bloating `sandboxrs-windows`.

---

# 8. Redirected user directories

Future convenience:

```text
USERPROFILE → sandbox/home
HOME        → sandbox/home
TEMP        → sandbox/tmp
TMP         → sandbox/tmp
```

and possibly language caches:

```text
CARGO_HOME
NPM_CONFIG_CACHE
PIP_CACHE_DIR
```

This is environment construction, not filesystem virtualization.

Caller can already implement this via `.env()` in v1.

---

# 9. Sandboxie-style filesystem overlay

Interesting, but explicitly outside current scope.

Concept:

```text
write C:\foo
    ↓
redirect into sandbox copy
```

Would introduce:

- filesystem interception,
- overlay semantics,
- host/sandbox merge rules,
- registry questions,
- huge Windows compatibility burden.

Do not build unless this becomes a separate project.

---

# 10. Git/workspace transactions

Potential companion crate:

```text
sandboxrs-workspace-git
```

Example:

```rust
let ws = GitWorkspace::temporary(repo)?;
let sandbox = Sandbox::builder(ws.path()).build()?;

sandbox.command("cargo").arg("test").output()?;

let diff = ws.diff()?;
ws.discard()?;
```

Do not mix source-control semantics into process isolation.

---

# 11. Mutation reporting

Potential future result:

```rust
SandboxOutput {
    modified_files: ...
}
```

Possible sources:

- Git diff,
- snapshots,
- USN journal,
- caller-provided monitor.

Mutation reporting is observability, not a security boundary.

Keep optional.

---

# 12. Long-lived task abstraction

If repeated setup becomes expensive or harnesses need task-local state:

```rust
let task = SandboxTask::create(config)?;

task.exec(...)?;
task.exec(...)?;
task.close()?;
```

Could preserve:

- sandbox identity,
- temp directories,
- cache,
- backend state.

Do not add until measured need exists.

The existing reusable `Sandbox` already covers the simplest multi-command case.

---

# 13. Persistent caches

Potential lifetimes:

```text
per execution
per sandbox
shared
```

Security issues:

- poisoned caches,
- executable artifacts,
- cross-task leakage.

Safer pattern if ever added:

```text
host shared cache → RO
sandbox-local cache → RW
```

Never share writable caches implicitly.

---

# 14. Streaming machine protocol

Bubblewrap's `--json-status-fd` suggests a good future design:

> Child stdout/stderr must remain separate from sandbox lifecycle metadata.

Possible future EXE layout:

```text
stdout → child stdout
stderr → child stderr
status pipe/handle → JSONL sandbox events
```

Possible events:

```json
{"type":"started","execution_id":"ex_1","pid":1234}
{"type":"completed","execution_id":"ex_1","exit_code":0}
```

Compatibility rule:

> Consumers must ignore unknown future fields and unknown event kinds.

Do not build this in v1.

---

# 15. Long-lived stdio server

If repeated `sandboxrs.exe` startup becomes expensive:

```text
sandboxrs.exe serve --stdio
```

with a small protocol.

Possible requests:

```json
{"id":1,"method":"sandbox.create", ...}
{"id":2,"method":"exec", ...}
{"id":3,"method":"sandbox.destroy", ...}
```

Only add after real Node/Python adoption proves the need.

---

# 16. Native language bindings

Possible later:

```text
sandboxrs-node
sandboxrs-python
```

using:

- N-API,
- PyO3,
- or a shared C ABI.

Do not multiply packaging/release surfaces before users ask.

The EXE is the universal fallback.

---

# 17. C ABI

Potential cross-language foundation:

```c
sandboxrs_create(...)
sandboxrs_spawn(...)
sandboxrs_wait(...)
sandboxrs_destroy(...)
```

Useful if:

- EXE latency matters,
- streaming process control matters,
- several languages want bindings.

Still optional.

---

# 18. Execution IDs

Anthropic's sandbox runtime benefits from correlating violations/events with a concrete command invocation.

Future internal/public type:

```rust
pub struct ExecutionId(u64);
```

Could support:

- diagnostics,
- structured events,
- audit logs,
- streaming EXE protocol.

No agent trace semantics should be added to core.

---

# 19. Structured hooks

Potential generic hooks:

```text
spawn
stdout
stderr
exit
timeout
policy failure
```

Useful to:

- agent harnesses,
- test runners,
- build systems,
- telemetry.

Keep generic.

Do not define LLM conversation/event formats.

---

# 20. Advanced diagnostics

Potential commands:

```text
sandboxrs doctor
sandboxrs explain
sandboxrs inspect
```

`explain` could answer:

```text
Why is C:\foo readable?
Why is C:\bar denied?
Which rule/backend caused this?
```

Worth adding only when policies become complex enough to require provenance.

---

# 21. Policy provenance

If profiles/presets arrive:

```rust
PathRule {
    path,
    access,
    source: RuleSource,
}
```

Possible sources:

```text
user
workspace default
profile
preset
backend requirement
```

Useful for diagnostics.

Not needed for explicit v1 RO/RW roots.

---

# 22. Policy DSL / config file

Potential future:

```toml
[filesystem]
rw = ["${workspace}"]
ro = ["${rustup}"]
```

Useful for:

- CLI reuse,
- CI,
- enterprise policy.

Avoid inventing a security language until builder APIs become insufficient.

---

# 23. Presets

Possible:

```text
Minimal
BuildTool
Untrusted
```

Danger:

Presets can hide authority decisions.

Only add if semantics are obvious, stable, and inspectable.

---

# 24. Policy fingerprint

Potential:

```text
sha256(normalized policy)
```

Useful for:

- debugging,
- audit logs,
- traces,
- reproducibility.

Cheap later feature.

---

# 25. Async Rust support

Possible:

```rust
sandbox.command(...).spawn_async().await?
```

Avoid forcing Tokio into core.

Options:

- feature-gated adapter,
- separate companion crate,
- generic async wrapper.

Start synchronous unless real need emerges.

---

# 26. ConPTY / interactive shells

Could support:

- REPLs,
- terminal agents,
- interactive CLI tools.

Adds:

- pseudo-console lifecycle,
- terminal resizing,
- signal behavior.

Not needed for v1 process execution.

---

# 27. Resource accounting

Potential output:

```text
peak memory
CPU time
process count
I/O bytes
```

Job Objects can help.

Useful for:

- benchmarking,
- CI,
- agent evals,
- resource budgets.

Keep separate from basic sandbox correctness.

---

# 28. More resource limits

Potential:

```rust
.max_cpu_time(...)
.max_io(...)
.max_wall_time(...)
```

Add only where Windows enforcement is clear and testable.

---

# 29. Secret-aware helpers

Possible convenience:

```rust
.deny_common_secret_locations()
```

Could cover:

```text
.ssh
.aws
.azure
browser profiles
```

But paths are environment-specific and enterprise-specific.

Prefer documentation/examples before opinionated defaults.

---

# 30. AppContainer lifecycle optimization

If fallback setup becomes expensive:

- deterministic profile names,
- safe profile reuse,
- caching,
- ACL reconciliation.

Measure first.

Correctness beats optimization.

---

# 31. ACL cleanup

If fallback modifies ACLs, future hardening may require:

- grant bookkeeping,
- idempotent repair,
- cleanup ownership.

Hard rule:

> Never remove an ACL entry unless the library can prove it created that entry.

---

# 32. Security-hardening investigations

Potential areas:

- inherited handle allowlisting,
- named pipes,
- object-manager namespace,
- COM,
- registry exposure,
- device access,
- window/desktop isolation,
- process breakaway,
- UNC paths,
- SMB shares,
- mapped drives,
- VDI redirected drives,
- clipboard channels,
- reparse-point races.

Research/test first.

Do not turn every discovery into a public knob.

---

# 33. Fuzzing

High-value future tests:

- path normalization fuzzing,
- overlapping-rule fuzzing,
- NT/Unicode path edge cases,
- malformed CLI JSON,
- backend-contract property testing.

Windows path semantics justify serious fuzzing eventually.

---

# 34. ARM64

Potential target:

```text
aarch64-pc-windows-msvc
```

Only after:

- backend availability is proven,
- CI exists,
- demand appears.

---

# 35. Enterprise/admin deployment tier

If enterprises later want centrally managed stronger network isolation and accept admin setup:

```text
dedicated sandbox user
+
restricted token
+
WFP
+
machine policy
```

This becomes closer to current large-lab Windows sandbox architectures.

It should remain a separate optional deployment mode.

Never break the core promise:

```text
no-admin baseline
```

---

# 36. VM-aware documentation

Do not manage VMs.

Future threat-model docs can explain:

- redirected host drives,
- shared folders,
- clipboard integration,
- VDI channels,
- outer network boundary.

The crate secures child-process authority inside Windows.

It does not secure the hypervisor or VDI integration layer.

---

# 37. Cross-platform facade

Do not rename this project into a cross-platform framework prematurely.

If demand appears:

```text
sandboxrs
├─ sandboxrs-windows
├─ sandboxrs-linux
└─ sandboxrs-macos
```

Possible native mechanisms:

```text
Windows → modern sandbox API / AppContainer
Linux   → namespaces / Landlock / seccomp / bwrap
macOS   → Seatbelt
```

The Windows crate should remain independently useful.

---

# 38. Harness adapters

If agent/coding harness integration becomes common, create adapters outside core:

```text
Agent Harness
      ↓
adapter crate
      ↓
sandboxrs-windows
```

Potential adapters may expose the crate as an `exec` tool.

Core must never import:

- model-provider concepts,
- messages,
- tool-call formats,
- plans,
- agent state.

---

# 39. Compatibility knowledge base

Sandboxie and Anthropic both demonstrate that compatibility knowledge accumulates.

If needed, make it separate:

```text
sandboxrs-profiles
```

Community profiles could cover:

```text
Rust MSVC
Rust GNU
Node
Python
Java
.NET
Visual Studio Build Tools
```

Keep independently versioned from the security core.

---

# 40. Overlay research

Interesting questions only:

- Can modern Windows Bound File System mechanisms provide useful disposable views?
- Can user-mode redirection cover specific developer-tool state without hooks?
- Can environment redirection remove most compatibility pain?
- Is an overlay ever necessary for coding harnesses?

Research does not imply implementation.

---

# 41. Candidate post-0.1 roadmap

Possible:

## 0.2

- error polish,
- backend diagnostics,
- async adapter if needed,
- more hostile path testing.

## 0.3

- optional profile companion crate,
- structured streaming EXE status channel,
- AppContainer lifecycle optimization.

## 0.4

- C ABI or language bindings if adoption warrants them.

## 1.0

Only after:

- Microsoft API direction is clearer,
- fallback semantics are battle-tested,
- public Rust API stabilizes,
- external security review exists.

---

# 42. Explicitly rejected unless requirements change

Be suspicious of:

```text
kernel drivers
DLL injection
filesystem hooks
registry hooks
transparent virtualization
built-in Git orchestration
LLM conversation storage
agent planning
agent memory
multi-agent primitives
Docker replacement
VM orchestration
general-purpose Windows security suite
```

These may be interesting projects.

They are not `sandboxrs-windows`.

---

# 43. Promotion rule

Move a feature into `PLAN.md` only when one is true:

1. A real application cannot work safely without it.
2. A security test demonstrates a material gap.
3. Multiple users independently request it.
4. Measurements show a meaningful performance/UX problem.
5. Microsoft changes Windows in a way that forces the feature.

Otherwise leave it here.

---

# 44. Long-term identity

If this project succeeds, its reputation should be:

> The boring Rust crate you reach for when a Windows child process should have much less authority than your application, without asking the user for admin.

That is enough.
