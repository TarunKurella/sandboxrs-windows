mod common;

#[cfg(target_os = "windows")]
mod smoke {
    use crate::common;

    #[test]
    #[ignore = "requires a working Windows backend (M0/M1)"]
    fn cmd_echo_works() {
        let workspace = common::fresh_workspace("smoke-echo");
        let sandbox = common::workspace_sandbox(&workspace);
        let output = common::run(&sandbox, "cmd", &["/c", "echo", "hello"]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("hello"));
        common::cleanup(workspace);
    }
}
