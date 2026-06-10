use std::fs;
use tempfile::TempDir;

#[test]
fn e2e_01_basic_scan_excludes_node_modules() {
    let tmp = TempDir::new().unwrap();
    let node_modules = tmp.path().join("project/node_modules");
    fs::create_dir_all(&node_modules).unwrap();

    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tmutil = tm_watcher::tmutil::MockTmUtil::new();
    let scanner = tm_watcher::Scanner::new(
        db.clone(),
        vec!["node_modules".into()],
        Box::new(mock_tmutil.clone())
    );

    let result = scanner.scan(tmp.path()).unwrap();

    assert_eq!(result.excluded_count, 1);
    assert!(mock_tmutil.was_called_with(&node_modules));
    assert!(db.is_recorded(node_modules.to_str().unwrap()).unwrap());
}
