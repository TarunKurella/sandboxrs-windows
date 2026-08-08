//! M0 capability probe.
//!
//! The desired M0 behavior is to launch `cmd /c exit 0` through
//! `Experimental_CreateProcessInSandbox`, grant one directory RW, prove a
//! write elsewhere fails, and run without admin. The exact experimental FFI
//! signature must be confirmed from a Windows SDK/VDI before this probe can be
//! implemented; until then the backend is detected but never selected.
