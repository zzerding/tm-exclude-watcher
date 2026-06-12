// ABOUTME: CLI 黑盒测试 - 验证发布身份入口不会触发用户状态变更

use std::fs;
use std::path::Path;
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

fn write_launch_agent_plist(home: &TempDir, exe_path: &Path) {
    let plist_dir = home.path().join("Library/LaunchAgents");
    fs::create_dir_all(&plist_dir).unwrap();
    fs::write(
        plist_dir.join("com.zzerding.tm-watcher.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.zzerding.tm-watcher</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>__daemon</string>
    </array>
</dict>
</plist>"#,
            exe_path.display()
        ),
    )
    .unwrap();
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

#[test]
fn test_status_without_launch_agent_plist_has_no_binary_warning() {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(&["status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("LaunchAgent 仍指向旧的 tm-watcher 二进制路径"));
    assert!(!stdout.contains("tm-watcher stop && tm-watcher start"));
}

#[test]
fn test_status_with_current_launch_agent_path_has_no_binary_warning() {
    let home = TempDir::new().unwrap();
    write_launch_agent_plist(&home, Path::new(env!("CARGO_BIN_EXE_tm-watcher")));

    let output = run_tm_watcher(&["status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("LaunchAgent 仍指向旧的 tm-watcher 二进制路径"));
    assert!(!stdout.contains("tm-watcher stop && tm-watcher start"));
}

#[test]
fn test_status_warns_when_launch_agent_points_to_old_binary() {
    let home = TempDir::new().unwrap();
    let old_path = Path::new("/opt/homebrew/Cellar/tm-watcher/0.1.0/bin/tm-watcher");
    write_launch_agent_plist(&home, old_path);

    let output = run_tm_watcher(&["status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("警告: LaunchAgent 仍指向旧的 tm-watcher 二进制路径"));
    assert!(stdout.contains("/opt/homebrew/Cellar/tm-watcher/0.1.0/bin/tm-watcher"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_tm-watcher")));
    assert!(stdout.contains("tm-watcher stop && tm-watcher start"));
}
