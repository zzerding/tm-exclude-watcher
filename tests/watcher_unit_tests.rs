use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tm_watcher::{Database, RuleMatcher};

// 测试 1: handle_create 启动延迟任务
#[tokio::test]
async fn test_handle_create_starts_timer() {
    let db = Database::new_in_memory().unwrap();
    let rules = RuleMatcher::new(vec!["node_modules".to_string()]);
    let mock_tmutil = Arc::new(tm_watcher::tmutil::MockTmUtil::new());

    let watcher = tm_watcher::Watcher::new(
        db,
        rules,
        mock_tmutil.clone(),
        Duration::from_millis(100),
    );

    watcher.handle_create(PathBuf::from("/test/node_modules")).await;

    // 验证任务已加入 pending_dirs
    let pending = watcher.pending_count().await;
    assert_eq!(pending, 1);
}

// 测试 2: 延迟后 tmutil 被调用
#[tokio::test]
async fn test_delayed_exclusion_triggers() {
    let db = Database::new_in_memory().unwrap();
    let rules = RuleMatcher::new(vec!["node_modules".to_string()]);
    let mock_tmutil = Arc::new(tm_watcher::tmutil::MockTmUtil::new());

    let watcher = tm_watcher::Watcher::new(
        db,
        rules,
        mock_tmutil.clone(),
        Duration::from_millis(100),
    );

    watcher.handle_create(PathBuf::from("/test/node_modules")).await;

    // 未到时间
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(mock_tmutil.call_count(), 0);

    // 超过延迟时间
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert_eq!(mock_tmutil.call_count(), 1);
}

// 测试 3: handle_remove 取消任务
#[tokio::test]
async fn test_handle_remove_cancels_pending() {
    let db = Database::new_in_memory().unwrap();
    let rules = RuleMatcher::new(vec!["node_modules".to_string()]);
    let mock_tmutil = Arc::new(tm_watcher::tmutil::MockTmUtil::new());

    let watcher = tm_watcher::Watcher::new(
        db,
        rules,
        mock_tmutil.clone(),
        Duration::from_millis(100),
    );

    let path = PathBuf::from("/test/node_modules");
    watcher.handle_create(path.clone()).await;

    // 在延迟期间删除
    tokio::time::sleep(Duration::from_millis(50)).await;
    watcher.handle_remove(path).await;

    // 等待原本的延迟时间
    tokio::time::sleep(Duration::from_millis(70)).await;

    // tmutil 不应被调用
    assert_eq!(mock_tmutil.call_count(), 0);
}

// 测试 4: execute_exclusion 写入数据库
#[tokio::test]
async fn test_exclusion_writes_to_database() {
    let db = Database::new_in_memory().unwrap();
    let rules = RuleMatcher::new(vec!["target".to_string()]);
    let mock_tmutil = Arc::new(tm_watcher::tmutil::MockTmUtil::new());

    let watcher = tm_watcher::Watcher::new(
        db.clone(),
        rules,
        mock_tmutil,
        Duration::from_millis(100),
    );

    watcher.handle_create(PathBuf::from("/test/target")).await;

    // 等待执行完成
    tokio::time::sleep(Duration::from_millis(150)).await;

    // 验证数据库记录
    assert!(db.is_recorded("/test/target").unwrap());
}

// 测试 5: handle_remove 清理数据库
#[tokio::test]
async fn test_handle_remove_cleans_database() {
    let db = Database::new_in_memory().unwrap();
    let rules = RuleMatcher::new(vec!["node_modules".to_string()]);
    let mock_tmutil = Arc::new(tm_watcher::tmutil::MockTmUtil::new());

    // 先记录到数据库
    db.record_exclusion("/test/node_modules", "node_modules", 0).unwrap();

    let watcher = tm_watcher::Watcher::new(
        db.clone(),
        rules,
        mock_tmutil.clone(),
        Duration::from_millis(100),
    );

    let path = PathBuf::from("/test/node_modules");
    watcher.handle_remove(path).await;

    // 验证数据库记录已删除
    assert!(!db.is_recorded("/test/node_modules").unwrap());

    // 验证调用了 remove_exclusion
    assert!(mock_tmutil.remove_was_called_with(&PathBuf::from("/test/node_modules")));
}

// 测试 6: 已记录目录跳过处理
#[tokio::test]
async fn test_skip_already_recorded() {
    let db = Database::new_in_memory().unwrap();
    let rules = RuleMatcher::new(vec!["node_modules".to_string()]);
    let mock_tmutil = Arc::new(tm_watcher::tmutil::MockTmUtil::new());

    // 先记录到数据库
    db.record_exclusion("/test/node_modules", "node_modules", 0).unwrap();

    let watcher = tm_watcher::Watcher::new(
        db.clone(),
        rules,
        mock_tmutil.clone(),
        Duration::from_millis(100),
    );

    watcher.handle_create(PathBuf::from("/test/node_modules")).await;

    // 等待执行完成
    tokio::time::sleep(Duration::from_millis(150)).await;

    // tmutil 不应被再次调用
    assert_eq!(mock_tmutil.call_count(), 0);
}
