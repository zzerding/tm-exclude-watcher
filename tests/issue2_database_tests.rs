// DATABASE_01: list_all 空数据库返回空列表
#[test]
fn database_01_list_all_empty() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    let records = db.list_all().unwrap();
    assert_eq!(records.len(), 0);
}

// DATABASE_02: insert + list_all 单条记录
#[test]
fn database_02_insert_and_list() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    db.record_exclusion("/test/node_modules", "node_modules", 1024).unwrap();

    let records = db.list_all().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].path, "/test/node_modules");
    assert_eq!(records[0].rule, "node_modules");
    assert_eq!(records[0].size_bytes, 1024);
    assert!(records[0].created_at > 0);
}

// DATABASE_03: delete_record 删除存在记录
#[test]
fn database_03_delete_record() {
    let db = tm_watcher::Database::new_in_memory().unwrap();
    db.record_exclusion("/test/target", "target", 2048).unwrap();

    assert!(db.is_recorded("/test/target").unwrap());

    db.delete_record("/test/target").unwrap();

    assert!(!db.is_recorded("/test/target").unwrap());
}
