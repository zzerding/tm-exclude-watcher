// ABOUTME: CLI 黑盒测试 - 验证发布身份入口不会触发用户状态变更

use std::process::Command;
use tempfile::TempDir;

fn run_tm_watcher(args: &[&str], home: &TempDir) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_tm-watcher"))
        .args(args)
        .env("HOME", home.path())
        .output()
        .unwrap()
}

fn assert_no_user_state_created(home: &TempDir) {
    assert!(!home.path().join(".config/tm-watcher").exists());
    assert!(!home.path().join(".local/share/tm-watcher").exists());
}

#[test]
fn test_version_outputs_package_version_without_user_state() {
    for arg in ["--version", "-V"] {
        let home = TempDir::new().unwrap();
        let output = run_tm_watcher(&[arg], &home);

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("tm-watcher {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(output.stderr.is_empty());
        assert_no_user_state_created(&home);
    }
}

#[test]
fn test_help_covers_public_commands_without_user_state() {
    for arg in ["--help", "-h"] {
        let home = TempDir::new().unwrap();
        let output = run_tm_watcher(&[arg], &home);

        assert!(output.status.success());
        let stdout = String::from_utf8(output.stdout).unwrap();
        for expected in [
            "tm-watcher - macOS Time Machine 自动排除工具",
            "用法:",
            "scan <path>",
            "list",
            "clean",
            "watch <path>",
            "start",
            "stop",
            "status",
        ] {
            assert!(stdout.contains(expected), "missing help text: {expected}");
        }
        assert!(output.stderr.is_empty());
        assert_no_user_state_created(&home);
    }
}
