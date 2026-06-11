// ABOUTME: 数据库层 - SQLite 存储排除记录

use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const SCHEMA_VERSION: i64 = 1;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug)]
pub struct ExclusionRecord {
    pub path: PathBuf,
    pub rule: String,
    pub size_bytes: Option<i64>,
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
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                last_checked_at DATETIME
            )",
            [],
        )?;
        validate_schema(&conn, db_path)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// 记录排除目录（幂等：路径已存在则忽略）
    pub fn record_exclusion(&self, path: &Path, rule: &str, size: Option<i64>) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO excluded_directories (path, rule, size_bytes) VALUES (?, ?, ?)",
            params![path.to_str().unwrap(), rule, size],
        )?;
        Ok(())
    }

    /// 检查路径是否已有排除记录
    pub fn has_exclusion(&self, path: &Path) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM excluded_directories WHERE path = ?",
            params![path.to_str().unwrap()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// 删除指定路径的排除记录
    pub fn delete_exclusion(&self, path: &Path) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM excluded_directories WHERE path = ?",
            params![path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// 更新排除记录的大小和最近检查时间
    pub fn update_exclusion_check(&self, path: &Path, size_bytes: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE excluded_directories
             SET size_bytes = ?, last_checked_at = CURRENT_TIMESTAMP
             WHERE path = ?",
            params![size_bytes, path.to_str().unwrap()],
        )?;
        Ok(())
    }

    /// 获取所有排除记录
    pub fn get_exclusions(&self) -> Result<Vec<ExclusionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, rule, size_bytes, created_at, last_checked_at
             FROM excluded_directories
             ORDER BY id",
        )?;

        let records = stmt
            .query_map([], |row| {
                Ok(ExclusionRecord {
                    path: PathBuf::from(row.get::<_, String>(0)?),
                    rule: row.get(1)?,
                    size_bytes: row.get(2)?,
                    created_at: row.get(3)?,
                    last_checked_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(records)
    }
}

fn validate_schema(conn: &Connection, db_path: &Path) -> Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(excluded_directories)")?;
    let column_names = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<Vec<_>, _>>()?;

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
