use std::fs;
use tempfile::TempDir;

// CLEAN_01: 目录不存在时调用 remove_exclusion + delete_record
#[test]
fn clean_01_removes_missing_directory() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tm = tm_watcher::tmutil::MockTmUtil::new();

    db.record_exclusion("/nonexistent/path", "node_modules", 512).unwrap();

    let cleaner = tm_watcher::Cleaner::new(db.clone(), Box::new(mock_tm.clone()));
    let stats = cleaner.clean().unwrap();

    assert_eq!(stats.removed_count, 1);
    assert_eq!(stats.updated_count, 0);
    assert!(!db.is_recorded("/nonexistent/path").unwrap());
    assert!(mock_tm.remove_was_called_with(std::path::Path::new("/nonexistent/path")));
}

// CLEAN_02: 目录存在时更新 metadata
#[test]
fn clean_02_updates_existing_directory() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tm = tm_watcher::tmutil::MockTmUtil::new();
    let temp_dir = TempDir::new().unwrap();
    let test_path = temp_dir.path().join("node_modules");
    fs::create_dir(&test_path).unwrap();

    db.record_exclusion(test_path.to_str().unwrap(), "node_modules", 100).unwrap();

    let cleaner = tm_watcher::Cleaner::new(db.clone(), Box::new(mock_tm));
    let stats = cleaner.clean().unwrap();

    assert_eq!(stats.updated_count, 1);
    assert_eq!(stats.removed_count, 0);

    // 验证 last_checked_at 已更新
    let records = db.list_all().unwrap();
    assert!(records[0].last_checked_at.is_some());
}

// CLEAN_03: 统计 cleaned/checked 计数
#[test]
fn clean_03_outputs_statistics() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tm = tm_watcher::tmutil::MockTmUtil::new();
    let temp_dir = TempDir::new().unwrap();
    let existing = temp_dir.path().join("target");
    fs::create_dir(&existing).unwrap();

    db.record_exclusion(existing.to_str().unwrap(), "target", 200).unwrap();
    db.record_exclusion("/missing/path", "node_modules", 100).unwrap();

    let cleaner = tm_watcher::Cleaner::new(db.clone(), Box::new(mock_tm));
    let stats = cleaner.clean().unwrap();

    assert_eq!(stats.removed_count, 1);
    assert_eq!(stats.updated_count, 1);
    assert_eq!(stats.error_count, 0);
    assert!(stats.errors.is_empty());
}
