// ABOUTME: 数据库层 - SQLite 存储排除记录

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SCHEMA_VERSION: i64 = 2;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug)]
pub struct ExclusionRecord {
    pub path: PathBuf,
    pub rule: String,
    pub size_bytes: Option<i64>,
    pub recorded_path_mtime_ns: Option<i64>,
    pub created_at: String,
    pub last_checked_at: Option<String>,
}

impl Database {
    /// 创建数据库并初始化 schema
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // 初始化 schema
        conn.execute(
            "CREATE TABLE IF NOT EXISTS excluded_directories (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE,
                rule TEXT NOT NULL,
                size_bytes INTEGER,
                recorded_path_mtime_ns INTEGER,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_checked_at DATETIME
            )",
            [],
        )?;
        migrate_schema(&conn, db_path)?;
        validate_schema(&conn, db_path)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 只读打开已有数据库；文件不存在时返回 None，且不创建任何目录或 schema。
    pub fn open_read_only_if_exists(db_path: &Path) -> Result<Option<Self>> {
        if !db_path.exists() {
            return Ok(None);
        }

        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        validate_read_only_schema(&conn, db_path)?;

        Ok(Some(Self {
            conn: Arc::new(Mutex::new(conn)),
        }))
    }

    /// 记录排除目录（幂等：路径已存在则忽略）
    pub fn record_exclusion(&self, path: &Path, rule: &str, size: Option<i64>) -> Result<()> {
        let path_text = path_for_database(path)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO excluded_directories (path, rule, size_bytes) VALUES (?, ?, ?)",
            params![path_text, rule, size],
        )?;
        Ok(())
    }

    /// 检查路径是否已有排除记录
    pub fn has_exclusion(&self, path: &Path) -> Result<bool> {
        let path_text = path_for_database(path)?;
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM excluded_directories WHERE path = ?",
            params![path_text],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// 删除指定路径的排除记录
    pub fn delete_exclusion(&self, path: &Path) -> Result<()> {
        let path_text = path_for_database(path)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM excluded_directories WHERE path = ?",
            params![path_text],
        )?;
        Ok(())
    }

    /// 更新排除记录的大小、顶层修改时间和最近检查时间
    pub fn update_exclusion_check(
        &self,
        path: &Path,
        size_bytes: i64,
        recorded_path_mtime_ns: i64,
    ) -> Result<()> {
        let path_text = path_for_database(path)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE excluded_directories
             SET size_bytes = ?, recorded_path_mtime_ns = ?, last_checked_at = CURRENT_TIMESTAMP
             WHERE path = ?",
            params![size_bytes, recorded_path_mtime_ns, path_text],
        )?;
        Ok(())
    }

    /// 仅更新最近检查时间；用于大小仍新鲜时的清理确认。
    pub fn touch_exclusion_check(&self, path: &Path) -> Result<()> {
        let path_text = path_for_database(path)?;
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE excluded_directories
             SET last_checked_at = CURRENT_TIMESTAMP
             WHERE path = ?",
            params![path_text],
        )?;
        Ok(())
    }

    /// 获取所有排除记录
    pub fn get_exclusions(&self) -> Result<Vec<ExclusionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, rule, size_bytes, recorded_path_mtime_ns, created_at, last_checked_at
             FROM excluded_directories
             ORDER BY id",
        )?;

        let records = stmt
            .query_map([], |row| {
                Ok(ExclusionRecord {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    rule: row.get(1)?,
                    size_bytes: row.get(2)?,
                    recorded_path_mtime_ns: row.get(3)?,
                    created_at: row.get(4)?,
                    last_checked_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }

    /// 获取最后一次清理时间（基于 MAX(last_checked_at)）
    pub fn last_cleanup_time(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: Option<String> = conn
            .query_row(
                "SELECT MAX(last_checked_at) FROM excluded_directories",
                [],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        Ok(result)
    }
}

fn validate_read_only_schema(conn: &Connection, db_path: &Path) -> Result<()> {
    let column_names = column_names(conn)?;
    validate_base_columns(&column_names, db_path)
}

fn migrate_schema(conn: &Connection, db_path: &Path) -> Result<()> {
    let column_names = column_names(conn)?;
    validate_base_columns(&column_names, db_path)?;

    if !column_names
        .iter()
        .any(|name| name == "recorded_path_mtime_ns")
    {
        conn.execute(
            "ALTER TABLE excluded_directories
             ADD COLUMN recorded_path_mtime_ns INTEGER",
            [],
        )?;
    }

    Ok(())
}

fn validate_schema(conn: &Connection, db_path: &Path) -> Result<()> {
    let column_names = column_names(conn)?;
    validate_base_columns(&column_names, db_path)?;

    if !column_names
        .iter()
        .any(|name| name == "recorded_path_mtime_ns")
    {
        anyhow::bail!(
            "数据库 schema 过旧（缺少 recorded_path_mtime_ns），请删除 {} 后重新运行 scan",
            db_path.display()
        );
    }

    Ok(())
}

fn column_names(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("PRAGMA table_info(excluded_directories)")?;
    let column_names = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(column_names)
}

fn validate_base_columns(column_names: &[String], db_path: &Path) -> Result<()> {
    let required_columns = [
        "path",
        "rule",
        "size_bytes",
        "created_at",
        "last_checked_at",
    ];
    for column in required_columns {
        if !column_names.iter().any(|name| name == column) {
            anyhow::bail!(
                "数据库 schema 过旧（缺少 {}），请删除 {} 后重新运行 scan",
                column,
                db_path.display()
            );
        }
    }

    Ok(())
}

fn path_for_database(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("数据库路径不是有效 UTF-8: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_last_cleanup_time_empty_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();

        assert_eq!(database.last_cleanup_time().unwrap(), None);
    }

    #[test]
    fn test_last_cleanup_time_returns_max() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();

        // 插入三条记录，手动设置不同的 last_checked_at
        let conn = database.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO excluded_directories (path, rule, last_checked_at) VALUES (?, ?, ?)",
            rusqlite::params!["/tmp/a", "node_modules", "2026-06-10 10:00:00"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO excluded_directories (path, rule, last_checked_at) VALUES (?, ?, ?)",
            rusqlite::params!["/tmp/b", "target", "2026-06-11 15:30:22"],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO excluded_directories (path, rule, last_checked_at) VALUES (?, ?, ?)",
            rusqlite::params!["/tmp/c", "vendor", "2026-06-09 08:00:00"],
        )
        .unwrap();
        drop(conn);

        assert_eq!(
            database.last_cleanup_time().unwrap(),
            Some("2026-06-11 15:30:22".to_string())
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_returns_error_instead_of_panicking() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        let database = Database::new(&db_path).unwrap();
        let path = PathBuf::from(OsString::from_vec(vec![0xff]));

        let result = database.record_exclusion(&path, "node_modules", None);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("UTF-8"));
    }
}
