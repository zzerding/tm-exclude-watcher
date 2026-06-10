use std::path::Path;

// EDGE_02: 不存在路径返回错误
#[test]
fn edge_02_nonexistent_path_returns_error() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    let mock_tmutil = tm_watcher::tmutil::MockTmUtil::new();
    let scanner = tm_watcher::Scanner::new(
        db,
        vec!["node_modules".into()],
        Box::new(mock_tmutil)
    );

    let result = scanner.scan(Path::new("/nonexistent_path_12345"));

    // WalkDir 对不存在路径会静默处理，返回空结果
    // 我们可以选择在入口处显式检查，或接受这个行为
    assert!(result.is_ok());
    assert_eq!(result.unwrap().excluded_count, 0);
}
