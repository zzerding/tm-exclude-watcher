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

fn assert_migration_error_without_state(args: &[&str], suggestion: &str) {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(args, &home);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("未知命令"));
    assert!(stderr.contains(suggestion));
    assert_no_user_state_created(&home);
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

fn config_path(home: &TempDir) -> std::path::PathBuf {
    home.path().join(".config/tm-watcher/config.toml")
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
            "Daemon 生命周期:",
            "扫描与清理:",
            "诊断与日志:",
            "配置管理:",
            "前台调试:",
            "scan <path>",
            "list",
            "clean",
            "logs",
            "config show",
            "config add-path",
            "config add-rule",
            "watch <path>",
            "daemon start",
            "daemon stop",
            "daemon restart",
            "daemon status",
            "doctor",
        ] {
            assert!(stdout.contains(expected), "missing help text: {expected}");
        }
        assert!(output.stderr.is_empty());
        assert_no_user_state_created(&home);
    }
}

#[test]
fn test_daemon_help_covers_lifecycle_subcommands_without_user_state() {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(&["daemon", "--help"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "tm-watcher daemon",
        "daemon start",
        "daemon stop",
        "daemon restart",
        "daemon status",
    ] {
        assert!(
            stdout.contains(expected),
            "missing daemon help text: {expected}"
        );
    }
    assert!(output.stderr.is_empty());
    assert_no_user_state_created(&home);
}

#[test]
fn test_config_help_covers_config_subcommands_without_user_state() {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(&["config", "--help"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "tm-watcher config",
        "config show",
        "config add-path <路径>",
        "config add-rule <规则>",
    ] {
        assert!(
            stdout.contains(expected),
            "missing config help text: {expected}"
        );
    }
    assert!(output.stderr.is_empty());
    assert_no_user_state_created(&home);
}

#[test]
fn test_old_daemon_commands_report_migration_without_user_state() {
    assert_migration_error_without_state(&["start"], "daemon start");
    assert_migration_error_without_state(&["stop"], "daemon stop");
    assert_migration_error_without_state(&["status"], "daemon status");
}

#[test]
fn test_status_without_launch_agent_plist_has_no_binary_warning() {
    let home = TempDir::new().unwrap();
    let output = run_tm_watcher(&["daemon", "status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("LaunchAgent 仍指向旧的 tm-watcher 二进制路径"));
    assert!(!stdout.contains("tm-watcher daemon stop && tm-watcher daemon start"));
}

#[test]
fn test_status_with_current_launch_agent_path_has_no_binary_warning() {
    let home = TempDir::new().unwrap();
    write_launch_agent_plist(&home, Path::new(env!("CARGO_BIN_EXE_tm-watcher")));

    let output = run_tm_watcher(&["daemon", "status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("LaunchAgent 仍指向旧的 tm-watcher 二进制路径"));
    assert!(!stdout.contains("tm-watcher daemon stop && tm-watcher daemon start"));
}

#[test]
fn test_status_warns_when_launch_agent_points_to_old_binary() {
    let home = TempDir::new().unwrap();
    let old_path = Path::new("/opt/homebrew/Cellar/tm-watcher/0.1.0/bin/tm-watcher");
    write_launch_agent_plist(&home, old_path);

    let output = run_tm_watcher(&["daemon", "status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("警告: LaunchAgent 仍指向旧的 tm-watcher 二进制路径"));
    assert!(stdout.contains("/opt/homebrew/Cellar/tm-watcher/0.1.0/bin/tm-watcher"));
    assert!(stdout.contains(env!("CARGO_BIN_EXE_tm-watcher")));
    assert!(stdout.contains("tm-watcher daemon stop && tm-watcher daemon start"));
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

    let output = run_tm_watcher(&["daemon", "status"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("状态:"));
    assert!(stdout.contains("累计节省空间: 约 1 GB (1 个目录已知大小，1 个未知)"));
}

#[test]
fn test_config_show_prints_full_friendly_config() {
    let home = TempDir::new().unwrap();

    let output = run_tm_watcher(&["config", "show"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("配置文件:"));
    assert!(stdout.contains("监控路径:"));
    assert!(stdout.contains("排除规则:"));
    assert!(stdout.contains("确认延迟: 5 秒"));
    assert!(stdout.contains("删除时清理: 是"));
    assert!(stdout.contains("定期清理间隔: 24 小时"));
    assert!(!stdout.contains("重启 daemon"));
    assert!(output.stderr.is_empty());
    assert!(!home.path().join(".local/share/tm-watcher").exists());
}

#[test]
fn test_config_add_rule_updates_config_and_prints_restart_hint() {
    let home = TempDir::new().unwrap();

    let output = run_tm_watcher(&["config", "add-rule", ".pytest_cache"], &home);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("已添加排除规则: .pytest_cache"));
    assert!(stdout.contains("配置已更新，请运行 'tm-watcher daemon restart' 重启 daemon 使其生效"));
    let config = std::fs::read_to_string(config_path(&home)).unwrap();
    assert!(config.contains("\".pytest_cache\""));
}

#[test]
fn test_config_add_rule_skips_duplicate() {
    let home = TempDir::new().unwrap();
    let first = run_tm_watcher(&["config", "add-rule", ".pytest_cache"], &home);
    assert!(first.status.success());

    let output = run_tm_watcher(&["config", "add-rule", ".pytest_cache"], &home);

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "排除规则已存在，跳过: .pytest_cache\n"
    );
}

#[test]
fn test_config_add_path_expands_tilde_and_updates_config() {
    let home = TempDir::new().unwrap();

    let output = run_tm_watcher(&["config", "add-path", "~/Workspace"], &home);

    assert!(output.status.success());
    let expanded = home.path().join("Workspace").to_string_lossy().into_owned();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains(&format!("已添加监控路径: {expanded}")));
    let config = std::fs::read_to_string(config_path(&home)).unwrap();
    assert!(config.contains(&format!("\"{expanded}\"")));
}

#[test]
fn test_config_add_path_skips_child_covered_by_existing_parent() {
    let home = TempDir::new().unwrap();
    let first = run_tm_watcher(&["config", "add-path", "~/Workspace"], &home);
    assert!(first.status.success());

    let output = run_tm_watcher(&["config", "add-path", "~/Workspace/project"], &home);

    assert!(output.status.success());
    let expanded_parent = home.path().join("Workspace").to_string_lossy().into_owned();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("监控路径已被 {expanded_parent} 覆盖，跳过\n")
    );
}

#[test]
fn test_config_add_path_skips_parent_covering_existing_children() {
    let home = TempDir::new().unwrap();
    let first = run_tm_watcher(&["config", "add-path", "~/Workspace/project"], &home);
    assert!(first.status.success());

    let output = run_tm_watcher(&["config", "add-path", "~/Workspace"], &home);

    assert!(output.status.success());
    let expanded_child = home
        .path()
        .join("Workspace/project")
        .to_string_lossy()
        .into_owned();
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("监控路径将覆盖 {expanded_child}，跳过\n")
    );
}

#[test]
fn test_config_rejects_multiple_operations() {
    let home = TempDir::new().unwrap();

    let output = run_tm_watcher(&["config", "show", "add-rule", ".cache"], &home);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("用法: tm-watcher config"));
    assert!(!config_path(&home).exists());
}

#[test]
fn test_config_malformed_file_reports_clear_error() {
    let home = TempDir::new().unwrap();
    fs::create_dir_all(config_path(&home).parent().unwrap()).unwrap();
    fs::write(config_path(&home), "watch_paths = [").unwrap();

    let output = run_tm_watcher(&["config", "show"], &home);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("配置文件格式错误"));
    assert!(stderr.contains("config.toml"));
}

#[test]
fn test_old_config_flags_report_migration_without_user_state() {
    assert_migration_error_without_state(&["config", "--show"], "config show");
    assert_migration_error_without_state(
        &["config", "--add-path", "~/Workspace"],
        "config add-path",
    );
    assert_migration_error_without_state(
        &["config", "--add-rule", ".pytest_cache"],
        "config add-rule",
    );
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
