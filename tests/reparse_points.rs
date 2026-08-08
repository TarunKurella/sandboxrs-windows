mod common;

#[cfg(target_os = "windows")]
mod reparse_points {
    use std::os::windows::fs as winfs;

    use crate::common;

    #[test]
    #[ignore = "requires a working Windows backend (M0/M1)"]
    fn junction_escape_is_denied() {
        let workspace = common::fresh_workspace("reparse-workspace");
        let outside = common::fresh_workspace("reparse-outside");
        let junction = workspace.join("escape");

        winfs::symlink_dir(&outside, &junction).expect("junction should be created");
        let sandbox = common::workspace_sandbox(&workspace);
        let output = common::run(
            &sandbox,
            common::attacker(),
            &["write", junction.join("escaped.txt").to_str().unwrap()],
        );
        common::expect_denied(&output);
        assert!(!outside.join("escaped.txt").exists());

        let _ = std::fs::remove_dir_all(&junction);
        common::cleanup(workspace);
        common::cleanup(outside);
    }
}
