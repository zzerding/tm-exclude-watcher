// ABOUTME: 集成测试 - 验证扫描、排除、数据库记录的端到端行为

use std::fs;
use tempfile::TempDir;
use tm_watcher::{Config, Database, Scanner};

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
