mod common;

#[cfg(target_os = "windows")]
mod backend_contract {
    use std::fs;

    use crate::common;

    #[test]
    #[ignore = "requires a working Windows backend (M0/M1)"]
    fn workspace_read_write_and_reuse() {
        let workspace = common::fresh_workspace("contract-workspace");
        let sandbox = common::workspace_sandbox(&workspace);

        let output = common::run_at(
            &sandbox,
            &workspace,
            common::attacker(),
            &["write", "out.txt"],
        );
        assert!(
            output.status.success(),
            "workspace write should succeed: {:?}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            fs::read_to_string(workspace.join("out.txt")).unwrap(),
            "attacker"
        );

        let output = common::run_at(
            &sandbox,
            &workspace,
            common::attacker(),
            &["read", "out.txt"],
        );
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "attacker");

        let output = common::run_at(&sandbox, &workspace, "cmd", &["/c", "echo", "reused"]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("reused"));
        common::cleanup(workspace);
    }

    #[test]
    #[ignore = "requires a working Windows backend (M0/M1)"]
    fn outside_write_is_denied_for_root_child_grandchild() {
        let workspace = common::fresh_workspace("contract-outside-write");
        let outside = common::fresh_workspace("contract-outside-secret");
        let sandbox = common::workspace_sandbox(&workspace);

        let targets = [
            outside.join("root.txt"),
            outside.join("child.txt"),
            outside.join("grandchild.txt"),
        ];
        let denied = [
            &["write", targets[0].to_str().unwrap()][..],
            &["spawn-write", targets[1].to_str().unwrap()][..],
            &["spawn-grandchild-write", targets[2].to_str().unwrap()][..],
        ];
        for args in denied {
            let output = common::run_at(&sandbox, &workspace, common::attacker(), args);
            common::expect_denied(&output);
        }

        assert!(!targets[0].exists());
        assert!(!targets[1].exists());
        assert!(!targets[2].exists());
        common::cleanup(workspace);
        common::cleanup(outside);
    }
}
