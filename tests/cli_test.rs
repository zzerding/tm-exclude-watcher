// ABOUTME: CLI 黑盒测试 - 验证发布身份入口不会触发用户状态变更

use std::fs;
use std::path::Path;
use std::process::Output;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
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

fn write_exclusion_record(home: &TempDir, path: &Path, rule: &str) {
    write_exclusion_record_with_size(home, path, rule, None);
}

fn write_exclusion_record_with_size(home: &TempDir, path: &Path, rule: &str, size: Option<i64>) {
    let data_dir = home.path().join(".local/share/tm-watcher");
    fs::create_dir_all(&data_dir).unwrap();
    let conn = rusqlite::Connection::open(data_dir.join("exclusions.db")).unwrap();
    conn.execute(
        "CREATE TABLE IF NOT EXISTS excluded_directories (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            rule TEXT NOT NULL,
            size_bytes INTEGER,
            created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
            last_checked_at DATETIME
        )",
        [],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO excluded_directories (path, rule, size_bytes) VALUES (?, ?, ?)",
        rusqlite::params![path.to_str().unwrap(), rule, size],
    )
    .unwrap();
}

fn write_daemon_log(home: &TempDir, lines: &[String]) {
    let log_dir = home.path().join(".local/share/tm-watcher");
    fs::create_dir_all(&log_dir).unwrap();
    fs::write(log_dir.join("daemon.log"), lines.join("\n")).unwrap();
}

fn spawn_tm_watcher(args: &[&str], home: &TempDir) -> Child {
    Command::new(env!("CARGO_BIN_EXE_tm-watcher"))
        .args(args)
        .env("HOME", home.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap()
}

fn collect_follow_output(mut child: Child) -> Output {
    std::thread::sleep(Duration::from_millis(500));
    let _ = child.kill();
    child.wait_with_output().unwrap()
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
            "logs",
            "watch <path>",
            "start",
            "stop",
            "status",
            "doctor",
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

#[test]
fn test_status_reports_saved_space_regardless_of_daemon_state() {
    let home = TempDir::new().unwrap();
    write_exclusion_record_with_size(
        &home,
        &home.path().join("project/node_modules"),
        "node_modules",
        Some(1024_i64.pow(3)),
    );
    write_exclusion_record_with_size(&home, &home.path().join("project/target"), "target", None);

    let output = run_tm_watcher(&["status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("状态:"));
    assert!(stdout.contains("累计节省空间: 约 1 GB (1 个目录已知大小，1 个未知)"));
}

#[test]
fn test_doctor_reports_all_checks_and_exits_nonzero_on_warnings() {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(&["doctor"], &home);

    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("tm-watcher 健康检查"));
    for expected in [
        "Time Machine",
        "配置文件",
        "数据库",
        "Daemon",
        "LaunchAgent plist",
    ] {
        assert!(
            stdout.contains(expected),
            "missing doctor check: {expected}"
        );
    }
}

#[test]
fn test_scan_dry_run_reports_preview_without_user_state() {
    let home = TempDir::new().unwrap();
    let scan_root = home.path().join("Code");
    fs::create_dir_all(scan_root.join("project/node_modules")).unwrap();

    let output = run_tm_watcher(&["scan", "--dry-run", scan_root.to_str().unwrap()], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("扫描预览:"));
    assert!(stdout.contains("将要排除的目录（1 个）:"));
    assert!(stdout.contains("~/Code/project/node_modules"));
    assert!(stdout.contains("匹配规则: node_modules"));
    assert!(stdout.contains("已跳过（之前已排除）: 0 个"));
    assert!(stdout.contains("提示: 使用 'tm-watcher scan"));
    assert!(stdout.contains("' 执行实际排除"));
    assert!(output.stderr.is_empty());
    assert_no_user_state_created(&home);
}

#[test]
fn test_scan_dry_run_missing_path_reports_clear_error() {
    let home = TempDir::new().unwrap();
    let missing_path = home.path().join("missing");
    let output = run_tm_watcher(
        &["scan", "--dry-run", missing_path.to_str().unwrap()],
        &home,
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("路径不存在"));
}

#[test]
fn test_scan_dry_run_reports_recorded_paths_as_skipped() {
    let home = TempDir::new().unwrap();
    let scan_root = home.path().join("Code");
    let node_modules = scan_root.join("project-a/node_modules");
    let target_dir = scan_root.join("project-b/target");
    fs::create_dir_all(&node_modules).unwrap();
    fs::create_dir_all(&target_dir).unwrap();
    write_exclusion_record(&home, &node_modules, "node_modules");

    let output = run_tm_watcher(&["scan", "--dry-run", scan_root.to_str().unwrap()], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("将要排除的目录（1 个）:"));
    assert!(stdout.contains("~/Code/project-b/target"));
    assert!(stdout.contains("匹配规则: target"));
    assert!(stdout.contains("已跳过（之前已排除）: 1 个"));
    assert!(stdout.contains("~/Code/project-a/node_modules"));
}

#[test]
fn test_logs_defaults_to_last_50_lines() {
    let home = TempDir::new().unwrap();
    let lines: Vec<String> = (1..=60).map(|line| format!("line {line}")).collect();
    write_daemon_log(&home, &lines);

    let output = run_tm_watcher(&["logs"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("line 10\n"));
    assert!(stdout.contains("line 11\n"));
    assert!(stdout.contains("line 60"));
    assert!(output.stderr.is_empty());
}

#[test]
fn test_logs_line_count_option_controls_tail_size() {
    let home = TempDir::new().unwrap();
    let lines: Vec<String> = (1..=120).map(|line| format!("line {line}")).collect();
    write_daemon_log(&home, &lines);

    let output = run_tm_watcher(&["logs", "-n", "100"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("line 20\n"));
    assert!(stdout.contains("line 21\n"));
    assert!(stdout.contains("line 120"));
}

#[test]
fn test_logs_missing_file_reports_friendly_message() {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(&["logs"], &home);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "日志文件不存在，daemon 可能未曾运行过\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn test_logs_empty_file_reports_friendly_message() {
    let home = TempDir::new().unwrap();
    write_daemon_log(&home, &[]);

    let output = run_tm_watcher(&["logs"], &home);

    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "日志为空\n");
}

#[test]
fn test_logs_follow_empty_file_prints_appended_lines() {
    let home = TempDir::new().unwrap();
    write_daemon_log(&home, &[]);

    let child = spawn_tm_watcher(&["logs", "--follow"], &home);
    std::thread::sleep(Duration::from_millis(100));
    fs::write(
        home.path().join(".local/share/tm-watcher/daemon.log"),
        "appended\n",
    )
    .unwrap();

    let output = collect_follow_output(child);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("日志为空"));
    assert!(stdout.contains("appended"));
}

#[test]
fn test_logs_follow_prints_appended_lines() {
    let home = TempDir::new().unwrap();
    write_daemon_log(&home, &[String::from("initial")]);

    let child = spawn_tm_watcher(&["logs", "--follow"], &home);
    std::thread::sleep(Duration::from_millis(100));
    fs::write(
        home.path().join(".local/share/tm-watcher/daemon.log"),
        "initial\nappended\n",
    )
    .unwrap();

    let output = collect_follow_output(child);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("initial"));
    assert!(stdout.contains("appended"));
}

#[test]
fn test_logs_follow_combines_with_line_count() {
    let home = TempDir::new().unwrap();
    let lines: Vec<String> = (1..=30).map(|line| format!("line {line}")).collect();
    write_daemon_log(&home, &lines);

    let child = spawn_tm_watcher(&["logs", "-n", "20", "--follow"], &home);
    std::thread::sleep(Duration::from_millis(100));
    let mut updated_lines = lines;
    updated_lines.push(String::from("line 31"));
    write_daemon_log(&home, &updated_lines);

    let output = collect_follow_output(child);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(!stdout.contains("line 10\n"));
    assert!(stdout.contains("line 11\n"));
    assert!(stdout.contains("line 31"));
}
