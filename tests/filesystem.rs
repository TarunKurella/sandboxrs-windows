use std::path::Path;

use sandboxrs_windows::{BackendKind, Sandbox};

#[test]
fn backend_names_are_stable() {
    assert_eq!(
        BackendKind::WindowsSandboxApi.as_str(),
        "windows-sandbox-api"
    );
    assert_eq!(BackendKind::AppContainer.as_str(), "appcontainer");
}

#[test]
fn builder_rejects_relative_workspace() {
    let err = Sandbox::builder("relative/workspace").build().unwrap_err();
    assert!(
        err.to_string().contains("invalid path"),
        "unexpected error: {err}"
    );
}

#[test]
fn builder_rejects_parent_components() {
    let err = Sandbox::builder(Path::new("/workspace/../escape"))
        .build()
        .unwrap_err();
    assert!(err.to_string().contains("invalid path"));
}
