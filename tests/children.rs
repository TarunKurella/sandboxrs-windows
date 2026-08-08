mod common;

#[cfg(target_os = "windows")]
mod children {
    use crate::common;

    #[test]
    #[ignore = "requires a working Windows backend (M0/M1)"]
    fn explicit_kill_terminates_descendants() {
        let workspace = common::fresh_workspace("children-kill");
        let sandbox = common::workspace_sandbox(&workspace);

        let mut command = sandbox.command(common::attacker());
        command.args(["spawn-many", "5"]);
        let mut child = command
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn should work");
        std::thread::sleep(std::time::Duration::from_millis(500));
        child.kill().expect("kill should work");
        let status = child.wait().expect("wait should work");
        assert!(!status.success());
        common::cleanup(workspace);
    }
}
