use std::fs;
use std::path::Path;
use tempfile::TempDir;

// CORE_01: basename 精确匹配
#[test]
fn core_01_basename_exact_match() {
    let matcher = tm_watcher::RuleMatcher::new(vec!["node_modules".into()]);

    // 应匹配
    assert_eq!(matcher.should_exclude(Path::new("/a/b/node_modules")), Some("node_modules"));
    assert_eq!(matcher.should_exclude(Path::new("/node_modules")), Some("node_modules"));

    // 不应匹配（basename 不同）
    assert_eq!(matcher.should_exclude(Path::new("/a/node_modules_backup")), None);
    assert_eq!(matcher.should_exclude(Path::new("/a/my_node_modules")), None);
}

// CORE_02: 重复扫描不重复记录
#[test]
fn core_02_repeated_scan_does_not_duplicate_records() {
    let tmp = TempDir::new().unwrap();
    let node_modules = tmp.path().join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tmutil = tm_watcher::tmutil::MockTmUtil::new();
    let scanner = tm_watcher::Scanner::new(
        db.clone(),
        vec!["node_modules".into()],
        Box::new(mock_tmutil.clone())
    );

    // 首次扫描
    let result1 = scanner.scan(tmp.path()).unwrap();
    assert_eq!(result1.excluded_count, 1);
    assert_eq!(result1.skipped_count, 0);

    // 重复扫描
    let result2 = scanner.scan(tmp.path()).unwrap();
    assert_eq!(result2.excluded_count, 0, "已排除目录不应重复处理");
    assert_eq!(result2.skipped_count, 1, "应跳过已有记录");
}

// CORE_06: 已排除目录不重复调用 tmutil
#[test]
fn core_06_already_excluded_dirs_skip_tmutil_call() {
    let tmp = TempDir::new().unwrap();
    let node_modules = tmp.path().join("node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tmutil = tm_watcher::tmutil::MockTmUtil::new();
    let scanner = tm_watcher::Scanner::new(
        db.clone(),
        vec!["node_modules".into()],
        Box::new(mock_tmutil.clone())
    );

    // 首次扫描
    scanner.scan(tmp.path()).unwrap();
    let first_call_count = mock_tmutil.call_count();
    assert_eq!(first_call_count, 1);

    // 重复扫描
    scanner.scan(tmp.path()).unwrap();
    let second_call_count = mock_tmutil.call_count();
    assert_eq!(second_call_count, 1, "不应再次调用 tmutil");
}

// CORE_04: 嵌套目录正确匹配
#[test]
fn core_04_nested_directories_are_matched() {
    let tmp = TempDir::new().unwrap();
    let nested = tmp.path().join("project/sub/nested/node_modules");
    fs::create_dir_all(&nested).unwrap();

    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tmutil = tm_watcher::tmutil::MockTmUtil::new();
    let scanner = tm_watcher::Scanner::new(
        db.clone(),
        vec!["node_modules".into()],
        Box::new(mock_tmutil.clone())
    );

    let result = scanner.scan(tmp.path()).unwrap();
    assert_eq!(result.excluded_count, 1);
    assert!(mock_tmutil.was_called_with(&nested));
}
