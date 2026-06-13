// ABOUTME: 集成测试 - 验证扫描、排除、数据库记录的端到端行为

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tm_watcher::{
    Cleaner, Config, Database, Scanner, TmBackend, format_exclusion_list,
    format_saved_space_summary,
};

#[test]
fn test_scan_and_exclude_matching_dirs() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // project1/node_modules/ (应被排除)
    let project1 = base_path.join("project1");
    let node_modules = project1.join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    // project1/src/ (不应被排除)
    let src_dir = project1.join("src");
    fs::create_dir(&src_dir).unwrap();

    // project2/target/ (应被排除)
    let project2 = base_path.join("project2");
    let target_dir = project2.join("target");
    fs::create_dir_all(&target_dir).unwrap();

    // 配置规则
    let rules = vec!["node_modules".to_string(), "target".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    // 创建临时数据库
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    // 使用 FakeTmBackend 执行扫描
    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    // 断言：2 个目录被排除
    assert_eq!(result.excluded_count, 2);

    // 断言：数据库有 2 条记录
    let records = database.get_exclusions().unwrap();
    assert_eq!(records.len(), 2);

    // 断言：FakeTmBackend 记录了 2 个路径
    let excluded_paths = tm_backend.get_excluded_paths();
    assert_eq!(excluded_paths.len(), 2);
    assert!(excluded_paths.contains(&node_modules));
    assert!(excluded_paths.contains(&target_dir));
}

#[test]
fn test_database_rejects_old_schema_with_clear_message() {
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("old-schema.db");

    {
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE excluded_directories (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                rule TEXT NOT NULL,
                size_bytes INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO excluded_directories (path, rule, size_bytes, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![
                "/tmp/old-node-modules",
                "node_modules",
                123_i64,
                "2026-01-02 03:04:05"
            ],
        )
        .unwrap();
    }

    let err = match Database::new(&db_path) {
        Ok(_) => panic!("old schema should be rejected"),
        Err(err) => err,
    };

    assert!(err.to_string().contains("schema 过旧"));
    assert!(err.to_string().contains("last_checked_at"));
    assert!(err.to_string().contains(&db_path.display().to_string()));
}

#[test]
fn test_clean_deletes_missing_path_record_after_path_not_found_remove() {
    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    let missing_path = db_dir.path().join("missing-node-modules");
    database
        .record_exclusion(&missing_path, "node_modules", Some(99))
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    tm_backend.fail_next_remove_path_not_found();
    let cleaner = Cleaner::new(database.clone(), Box::new(tm_backend.clone()));

    let result = cleaner.clean().unwrap();

    assert_eq!(result.cleaned_count, 1);
    assert_eq!(result.checked_count, 0);
    assert!(result.errors.is_empty());
    assert_eq!(tm_backend.remove_exclusion_call_count(), 1);
    assert!(database.get_exclusions().unwrap().is_empty());
}

#[test]
fn test_clean_updates_existing_path_size_and_last_checked_at() {
    let temp_dir = TempDir::new().unwrap();
    let excluded_path = temp_dir.path().join("target");
    fs::create_dir_all(excluded_path.join("nested")).unwrap();
    fs::write(excluded_path.join("a.bin"), [1_u8, 2, 3]).unwrap();
    fs::write(excluded_path.join("nested/b.bin"), [4_u8, 5]).unwrap();

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    database
        .record_exclusion(&excluded_path, "target", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    tm_backend.add_exclusion(&excluded_path).unwrap();
    let cleaner = Cleaner::new(database.clone(), Box::new(tm_backend.clone()));

    let result = cleaner.clean().unwrap();
    let records = database.get_exclusions().unwrap();

    assert_eq!(result.cleaned_count, 0);
    assert_eq!(result.checked_count, 1);
    assert!(result.errors.is_empty());
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].size_bytes, Some(5));
    assert!(records[0].last_checked_at.is_some());
    assert_eq!(tm_backend.is_excluded_call_count(), 1);
}

#[test]
#[cfg(unix)]
fn test_clean_does_not_follow_recorded_symlink() {
    let temp_dir = TempDir::new().unwrap();
    let real_path = temp_dir.path().join("real-target");
    fs::create_dir_all(&real_path).unwrap();
    fs::write(real_path.join("large.bin"), [0_u8; 64]).unwrap();

    let symlink_path = temp_dir.path().join("node_modules");
    std::os::unix::fs::symlink(&real_path, &symlink_path).unwrap();
    let symlink_size = fs::symlink_metadata(&symlink_path).unwrap().len() as i64;

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    database
        .record_exclusion(&symlink_path, "node_modules", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    tm_backend.add_exclusion(&symlink_path).unwrap();
    let cleaner = Cleaner::new(database.clone(), Box::new(tm_backend));

    let result = cleaner.clean().unwrap();
    let records = database.get_exclusions().unwrap();

    assert_eq!(result.cleaned_count, 0);
    assert_eq!(result.checked_count, 1);
    assert!(result.errors.is_empty());
    assert_eq!(records[0].size_bytes, Some(symlink_size));
    assert_ne!(records[0].size_bytes, Some(64));
}

#[test]
fn test_clean_repairs_existing_path_missing_tm_exclusion() {
    let temp_dir = TempDir::new().unwrap();
    let excluded_path = temp_dir.path().join("node_modules");
    fs::create_dir_all(&excluded_path).unwrap();

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    database
        .record_exclusion(&excluded_path, "node_modules", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let cleaner = Cleaner::new(database, Box::new(tm_backend.clone()));

    let result = cleaner.clean().unwrap();

    assert_eq!(result.cleaned_count, 0);
    assert_eq!(result.checked_count, 1);
    assert!(result.errors.is_empty());
    assert_eq!(tm_backend.is_excluded_call_count(), 1);
    assert_eq!(tm_backend.add_exclusion_call_count(), 1);
    assert!(tm_backend.get_excluded_paths().contains(&excluded_path));
}

#[test]
fn test_clean_records_error_and_continues_with_later_records() {
    let temp_dir = TempDir::new().unwrap();
    let missing_path = temp_dir.path().join("missing-node-modules");
    let existing_path = temp_dir.path().join("target");
    fs::create_dir_all(&existing_path).unwrap();

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    database
        .record_exclusion(&missing_path, "node_modules", None)
        .unwrap();
    database
        .record_exclusion(&existing_path, "target", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    tm_backend.fail_next_remove_other("boom");
    let cleaner = Cleaner::new(database.clone(), Box::new(tm_backend.clone()));

    let result = cleaner.clean().unwrap();
    let records = database.get_exclusions().unwrap();

    assert_eq!(result.cleaned_count, 0);
    assert_eq!(result.checked_count, 1);
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0].contains("boom"));
    assert_eq!(records.len(), 2);
    assert!(records[1].last_checked_at.is_some());
    assert_eq!(tm_backend.remove_exclusion_call_count(), 1);
    assert_eq!(tm_backend.is_excluded_call_count(), 1);
}

#[test]
fn test_format_exclusion_list_reports_empty_database() {
    let output = format_exclusion_list(&[]);

    assert_eq!(output, "没有排除记录。");
}

#[test]
fn test_format_exclusion_list_prints_compact_inspection_table() {
    let home_dir = dirs::home_dir().unwrap();
    let records = vec![
        exclusion_record(
            home_dir.join("Code/project-a/node_modules"),
            "node_modules",
            Some(512 * 1024 * 1024),
            Some("2026-06-10 08:05:59"),
        ),
        exclusion_record(
            home_dir.join("Code/project-b/target"),
            "target",
            Some(2 * 1024 * 1024 * 1024),
            Some("2026-06-11 10:30:16"),
        ),
        exclusion_record(
            home_dir.join("Code/project-c/.cache"),
            ".cache",
            Some(999),
            None,
        ),
        exclusion_record("/var/tmp/project/vendor", "vendor", None, None),
    ];

    let output = format_exclusion_list(&records);
    let expected = "\
排除记录: 4 条，已知大小合计 2.5 GB，未知大小 1 条

#  大小    规则          检查时间          路径
1  2 GB    target        2026-06-11 10:30  ~/Code/project-b/target
2  512 MB  node_modules  2026-06-10 08:05  ~/Code/project-a/node_modules
3  999 B   .cache        未检查            ~/Code/project-c/.cache
4  未知    vendor        未检查            /var/tmp/project/vendor
";

    assert_eq!(output, expected);
    assert!(!output.contains("10:30:16"));
}

#[test]
fn test_format_exclusion_list_sorts_by_size_with_predictable_ties() {
    let records = vec![
        exclusion_record("/tmp/unknown-z", "vendor", None, None),
        exclusion_record("/tmp/known-b", "target", Some(1024), None),
        exclusion_record("/tmp/known-a", "node_modules", Some(1024), None),
        exclusion_record("/tmp/largest", "target", Some(2048), None),
        exclusion_record("/tmp/unknown-y", "vendor", None, None),
    ];

    let output = format_exclusion_list(&records);

    assert_in_order(
        &output,
        &[
            "/tmp/largest",
            "/tmp/known-a",
            "/tmp/known-b",
            "/tmp/unknown-y",
            "/tmp/unknown-z",
        ],
    );
}

#[test]
fn test_format_exclusion_list_uses_automatic_size_units() {
    let records = vec![
        exclusion_record("/tmp/zero", "cache", Some(0), None),
        exclusion_record("/tmp/bytes", "cache", Some(999), None),
        exclusion_record("/tmp/kb", "cache", Some(1024), None),
        exclusion_record("/tmp/mb", "cache", Some(1536 * 1024), None),
        exclusion_record("/tmp/gb", "cache", Some(2355 * 1024 * 1024), None),
        exclusion_record("/tmp/tb", "cache", Some(1024_i64.pow(4)), None),
    ];

    let output = format_exclusion_list(&records);

    assert!(output.contains("0 B"));
    assert!(output.contains("999 B"));
    assert!(output.contains("1 KB"));
    assert!(output.contains("1.5 MB"));
    assert!(output.contains("2.3 GB"));
    assert!(output.contains("1 TB"));
}

#[test]
fn test_format_saved_space_summary_reports_known_and_unknown_counts() {
    let records = vec![
        exclusion_record(
            "/tmp/node_modules",
            "node_modules",
            Some(1024_i64.pow(3)),
            None,
        ),
        exclusion_record("/tmp/target", "target", Some(512 * 1024 * 1024), None),
        exclusion_record("/tmp/vendor", "vendor", None, None),
    ];

    let output = format_saved_space_summary(&records);

    assert_eq!(
        output,
        "累计节省空间: 约 1.5 GB (2 个目录已知大小，1 个未知)"
    );
}

#[test]
fn test_format_saved_space_summary_reports_unknown_without_known_sizes() {
    let records = vec![
        exclusion_record("/tmp/node_modules", "node_modules", None, None),
        exclusion_record("/tmp/target", "target", None, None),
    ];

    let output = format_saved_space_summary(&records);

    assert_eq!(
        output,
        "累计节省空间: 未知（运行 'tm-watcher clean' 更新大小信息）"
    );
}

fn exclusion_record(
    path: impl AsRef<Path>,
    rule: &str,
    size_bytes: Option<i64>,
    last_checked_at: Option<&str>,
) -> tm_watcher::ExclusionRecord {
    tm_watcher::ExclusionRecord {
        path: PathBuf::from(path.as_ref()),
        rule: rule.to_string(),
        size_bytes,
        created_at: "2026-06-01 00:00:00".to_string(),
        last_checked_at: last_checked_at.map(str::to_string),
    }
}

fn assert_in_order(haystack: &str, needles: &[&str]) {
    let mut previous_position = 0;
    for needle in needles {
        let position = haystack[previous_position..]
            .find(needle)
            .unwrap_or_else(|| panic!("missing expected text: {needle}"));
        previous_position += position + needle.len();
    }
}

#[test]
fn test_idempotency_no_duplicate_records() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let node_modules = base_path.join("project/node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    // 配置
    let rules = vec!["node_modules".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner = Scanner::with_backend(
        config.clone(),
        database.clone(),
        Box::new(tm_backend.clone()),
    )
    .unwrap();

    // 第一次扫描
    let result1 = scanner.scan(base_path).unwrap();
    assert_eq!(result1.excluded_count, 1);
    assert_eq!(tm_backend.add_exclusion_call_count(), 1);

    // 第二次扫描（幂等性测试）
    let scanner2 =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();
    let result2 = scanner2.scan(base_path).unwrap();

    // 断言：第二次扫描不应排除任何新目录
    assert_eq!(result2.excluded_count, 0);
    assert_eq!(result2.skipped_count, 1);
    assert_eq!(tm_backend.add_exclusion_call_count(), 1);
    assert_eq!(tm_backend.is_excluded_call_count(), 0);

    // 断言：数据库仍然只有 1 条记录
    let records = database.get_exclusions().unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn test_rule_matching_basename_only() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // 创建名字包含但不完全匹配的目录
    let my_node_modules = base_path.join("my_node_modules");
    fs::create_dir(&my_node_modules).unwrap();

    let node_modules_backup = base_path.join("node_modules_backup");
    fs::create_dir(&node_modules_backup).unwrap();

    // 创建精确匹配的目录
    let node_modules = base_path.join("project/node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    // 配置规则
    let rules = vec!["node_modules".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    // 断言：只有精确匹配的 1 个目录被排除
    assert_eq!(result.excluded_count, 1);

    let excluded_paths = tm_backend.get_excluded_paths();
    assert_eq!(excluded_paths.len(), 1);
    assert!(excluded_paths.contains(&node_modules));
    assert!(!excluded_paths.contains(&my_node_modules));
    assert!(!excluded_paths.contains(&node_modules_backup));
}

#[test]
fn test_non_matching_dirs_not_excluded() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // 创建不匹配规则的目录
    let src_dir = base_path.join("src");
    fs::create_dir(&src_dir).unwrap();

    let lib_dir = base_path.join("lib");
    fs::create_dir(&lib_dir).unwrap();

    let docs_dir = base_path.join("docs");
    fs::create_dir(&docs_dir).unwrap();

    // 配置规则（不包含 src/lib/docs）
    let rules = vec!["node_modules".to_string(), "target".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    // 断言：0 个目录被排除
    assert_eq!(result.excluded_count, 0);

    let excluded_paths = tm_backend.get_excluded_paths();
    assert_eq!(excluded_paths.len(), 0);

    // 断言：数据库没有记录
    let records = database.get_exclusions().unwrap();
    assert_eq!(records.len(), 0);
}

#[test]
fn test_symlinks_not_followed() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // 创建真实目录包含 node_modules
    let real_dir = base_path.join("real_project");
    let real_node_modules = real_dir.join("node_modules");
    fs::create_dir_all(&real_node_modules).unwrap();

    // 创建符号链接指向 real_project
    let symlink_path = base_path.join("symlink_project");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_dir, &symlink_path).unwrap();

    // 配置规则
    let rules = vec!["node_modules".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    // 断言：只有真实目录下的 node_modules 被排除
    // 符号链接指向的目录内容不应被扫描
    assert_eq!(result.excluded_count, 1);

    let excluded_paths = tm_backend.get_excluded_paths();
    assert_eq!(excluded_paths.len(), 1);
    assert!(excluded_paths.contains(&real_node_modules));

    // 确认符号链接路径下的 node_modules 没有被排除
    let symlink_node_modules = symlink_path.join("node_modules");
    assert!(!excluded_paths.contains(&symlink_node_modules));
}

#[test]
#[cfg(unix)]
fn test_permission_error_continues_scan() {
    use std::os::unix::fs::PermissionsExt;

    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    // 创建可访问的目录
    let accessible = base_path.join("accessible");
    let accessible_node_modules = accessible.join("node_modules");
    fs::create_dir_all(&accessible_node_modules).unwrap();

    // 创建受限目录
    let restricted = base_path.join("restricted");
    fs::create_dir_all(&restricted).unwrap();
    let restricted_target = restricted.join("target");
    fs::create_dir(&restricted_target).unwrap();

    // 移除读权限（使其无法访问）
    let mut perms = fs::metadata(&restricted).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&restricted, perms).unwrap();

    // 配置规则
    let rules = vec!["node_modules".to_string(), "target".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path);

    // 恢复权限以便清理
    let mut perms = fs::metadata(&restricted).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&restricted, perms).unwrap();

    // 断言：扫描成功完成（权限错误不中断）
    assert!(result.is_ok());
    let result = result.unwrap();

    // 断言：可访问的目录被排除
    assert_eq!(result.excluded_count, 1);

    let excluded_paths = tm_backend.get_excluded_paths();
    assert!(excluded_paths.contains(&accessible_node_modules));
}

#[test]
fn test_tm_not_configured_detected() {
    // 创建未配置的 FakeTmBackend
    let tm_backend = tm_watcher::FakeTmBackend::new_unconfigured();

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let rules = vec!["node_modules".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    // 断言：Scanner::with_backend() 返回错误
    let result = Scanner::with_backend(config, database, Box::new(tm_backend));
    assert!(result.is_err());
    if let Err(err) = result {
        assert!(err.to_string().contains("Time Machine 未配置"));
    }
}

#[test]
fn test_scan_result_reports_skipped_count() {
    // 创建临时目录结构
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let node_modules = base_path.join("project1/node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    let target_dir = base_path.join("project2/target");
    fs::create_dir_all(&target_dir).unwrap();

    let rules = vec!["node_modules".to_string(), "target".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    // 预先排除 node_modules（模拟之前已排除）
    // 更正：scan 热路径现在以数据库记录代表“之前已排除”的状态。
    // 预先记录 node_modules（模拟之前扫描过）
    database
        .record_exclusion(&node_modules, "node_modules", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();

    let scanner = Scanner::with_backend(config, database, Box::new(tm_backend)).unwrap();
    let result = scanner.scan(base_path).unwrap();

    // 断言：1 个新排除（target），1 个跳过（node_modules 已排除）
    // 更正：跳过现在来自数据库记录，不再来自 tmutil isexcluded。
    // 断言：1 个新排除（target），1 个跳过（node_modules 已记录）
    assert_eq!(result.excluded_count, 1);
    assert_eq!(result.skipped_count, 1);
    assert!(result.errors.is_empty());
}

#[test]
fn test_scan_dry_run_reports_matches_without_tmutil_or_database_writes() {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let node_modules = base_path.join("project1/node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    let target_dir = base_path.join("project2/target");
    fs::create_dir_all(&target_dir).unwrap();

    let config = Config {
        exclude_rules: vec!["node_modules".to_string(), "target".to_string()],
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    database
        .record_exclusion(&node_modules, "node_modules", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let result = Scanner::dry_run(config, Some(&database), base_path).unwrap();

    assert_eq!(result.to_exclude.len(), 1);
    assert_eq!(result.to_exclude[0].path, target_dir);
    assert_eq!(result.to_exclude[0].rule, "target");
    assert_eq!(result.skipped.len(), 1);
    assert_eq!(result.skipped[0].path, node_modules);
    assert_eq!(result.skipped[0].rule, "node_modules");
    assert!(result.errors.is_empty());
    assert_eq!(tm_backend.add_exclusion_call_count(), 0);
    assert_eq!(tm_backend.is_excluded_call_count(), 0);
    assert_eq!(database.get_exclusions().unwrap().len(), 1);
}

#[test]
#[cfg(unix)]
fn test_scan_dry_run_reports_matching_symlink_itself() {
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path().join("workspace");
    fs::create_dir_all(&base_path).unwrap();

    let real_deps = temp_dir.path().join("shared-deps");
    fs::create_dir_all(real_deps.join("nested/target")).unwrap();

    let node_modules_link = base_path.join("project/node_modules");
    fs::create_dir_all(node_modules_link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real_deps, &node_modules_link).unwrap();

    let config = Config {
        exclude_rules: vec!["node_modules".to_string(), "target".to_string()],
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let result = Scanner::dry_run(config, Some(&database), &base_path).unwrap();

    assert_eq!(result.to_exclude.len(), 1);
    assert_eq!(result.to_exclude[0].path, node_modules_link);
    assert_eq!(result.to_exclude[0].rule, "node_modules");
    assert!(result.skipped.is_empty());
}

#[test]
fn test_pruning_skips_subtree_of_matched_dir() {
    // 创建嵌套结构: project/node_modules/foo/node_modules
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let outer = base_path.join("project/node_modules");
    let nested = outer.join("foo/node_modules");
    fs::create_dir_all(&nested).unwrap();

    // node_modules 内部还有一个匹配其他规则的目录
    let nested_cache = outer.join("bar/.cache");
    fs::create_dir_all(&nested_cache).unwrap();

    let rules = vec!["node_modules".to_string(), ".cache".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    // 断言：只有最外层 node_modules 被排除（TM 排除是递归的，子树无需单独排除）
    assert_eq!(result.excluded_count, 1);

    let excluded_paths = tm_backend.get_excluded_paths();
    assert_eq!(excluded_paths.len(), 1);
    assert!(excluded_paths.contains(&outer));
    assert!(!excluded_paths.contains(&nested));
    assert!(!excluded_paths.contains(&nested_cache));

    // 断言：数据库只有 1 条记录，无嵌套冗余
    let records = database.get_exclusions().unwrap();
    assert_eq!(records.len(), 1);
}

#[test]
fn test_pruning_also_skips_already_excluded_dirs() {
    // 已排除的目录同样应被剪枝，不深入遍历
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let outer = base_path.join("project/node_modules");
    let nested = outer.join("foo/target");
    fs::create_dir_all(&nested).unwrap();

    let rules = vec!["node_modules".to_string(), "target".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    // 预先排除外层 node_modules（模拟之前扫描过）
    // 更正：scan 热路径现在以数据库记录代表“之前扫描过”的状态。
    // 预先记录外层 node_modules（模拟之前扫描过）
    database
        .record_exclusion(&outer, "node_modules", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();

    let scanner = Scanner::with_backend(config, database, Box::new(tm_backend.clone())).unwrap();
    let result = scanner.scan(base_path).unwrap();

    // 断言：外层跳过，内层 target 不被遍历到
    assert_eq!(result.excluded_count, 0);
    assert_eq!(result.skipped_count, 1);

    let excluded_paths = tm_backend.get_excluded_paths();
    assert!(!excluded_paths.contains(&nested));
}

#[test]
fn test_scan_trusts_database_record_without_tmutil_check() {
    // 数据库已有记录时，scan 热路径不应再调用昂贵的 tmutil isexcluded
    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let node_modules = base_path.join("project/node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    let config = Config {
        exclude_rules: vec!["node_modules".to_string()],
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();
    database
        .record_exclusion(&node_modules, "node_modules", None)
        .unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    assert_eq!(result.excluded_count, 0);
    assert_eq!(result.skipped_count, 1);
    assert_eq!(tm_backend.is_excluded_call_count(), 0);
    assert_eq!(tm_backend.add_exclusion_call_count(), 0);
    assert!(tm_backend.get_excluded_paths().is_empty());
    assert_eq!(database.get_exclusions().unwrap().len(), 1);
}

#[test]
fn test_rescan_recorded_directories_skips_tmutil_calls() {
    const RECORDED_DIR_COUNT: usize = 12;

    let temp_dir = TempDir::new().unwrap();
    let base_path = temp_dir.path();

    let config = Config {
        exclude_rules: vec!["node_modules".to_string()],
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    for project_index in 0..RECORDED_DIR_COUNT {
        let node_modules = base_path
            .join(format!("project_{project_index:02}"))
            .join("node_modules");
        fs::create_dir_all(&node_modules).unwrap();
        database
            .record_exclusion(&node_modules, "node_modules", None)
            .unwrap();
    }

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner =
        Scanner::with_backend(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(base_path).unwrap();

    assert_eq!(result.excluded_count, 0);
    assert_eq!(result.skipped_count, RECORDED_DIR_COUNT);
    assert_eq!(tm_backend.is_excluded_call_count(), 0);
    assert_eq!(tm_backend.add_exclusion_call_count(), 0);
    assert_eq!(database.get_exclusions().unwrap().len(), RECORDED_DIR_COUNT);
}

#[test]
fn test_scan_root_itself_matches_rule() {
    // 扫描根目录本身就是 node_modules：只排除根，不深入子树
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path().join("node_modules");
    let nested = root.join("foo/node_modules");
    fs::create_dir_all(&nested).unwrap();

    let rules = vec!["node_modules".to_string()];
    let config = Config {
        exclude_rules: rules,
        ..Default::default()
    };

    let db_dir = TempDir::new().unwrap();
    let db_path = db_dir.path().join("test.db");
    let database = Database::new(&db_path).unwrap();

    let tm_backend = tm_watcher::FakeTmBackend::new();
    let scanner = Scanner::with_backend(config, database, Box::new(tm_backend.clone())).unwrap();

    let result = scanner.scan(&root).unwrap();

    // 断言：只排除根目录自己
    assert_eq!(result.excluded_count, 1);
    let excluded_paths = tm_backend.get_excluded_paths();
    assert!(excluded_paths.contains(&root));
    assert!(!excluded_paths.contains(&nested));
}
