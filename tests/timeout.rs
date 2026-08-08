mod common;

#[cfg(target_os = "windows")]
mod timeout_tests {
    use std::time::Duration;

    use sandboxrs_windows::Sandbox;

    use crate::common;

    #[test]
    #[ignore = "requires a working Windows backend (M0/M1)"]
    fn timeout_terminates_tree() {
        let workspace = common::fresh_workspace("timeout-tree");
        let sandbox = Sandbox::builder(&workspace)
            .timeout(Duration::from_millis(500))
            .build()
            .expect("sandbox should build");

        let mut command = sandbox.command(common::attacker());
        command.args(["spawn-many", "3"]);
        command
            .stdout(sandboxrs_windows::Stdio::null())
            .stderr(sandboxrs_windows::Stdio::null());
        let output = command.output().expect("output should complete");
        assert!(
            !output.status.success(),
            "timed out tree should not succeed"
        );
        common::cleanup(workspace);
    }
}
