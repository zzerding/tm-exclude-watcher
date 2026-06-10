// ABOUTME: SQLite 数据库操作，记录已排除目录

use rusqlite::{Connection, Result};
use std::path::Path;
use std::sync::{Arc, Mutex};

pub struct ExclusionRecord {
    pub path: String,
    pub rule: String,
    pub size_bytes: i64,
    pub created_at: i64,
    pub last_checked_at: Option<i64>,
}

impl ExclusionRecord {
    pub fn size_mb(&self) -> f64 {
        self.size_bytes as f64 / 1024.0 / 1024.0
    }

    pub fn created_at_display(&self) -> String {
        use std::time::{Duration, UNIX_EPOCH};
        let duration = Duration::from_secs(self.created_at as u64);
        let datetime = UNIX_EPOCH + duration;
        if let Ok(elapsed) = datetime.elapsed() {
            format!("{} 秒前", elapsed.as_secs())
        } else {
            format!("时间戳: {}", self.created_at)
        }
    }
}

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn new(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS exclusions (
                path TEXT PRIMARY KEY,
                rule TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                last_checked_at INTEGER
            )",
            [],
        )?;
        Ok(())
    }

    pub fn record_exclusion(&self, path: &str, rule: &str, size: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "INSERT OR IGNORE INTO exclusions (path, rule, size_bytes, created_at) VALUES (?1, ?2, ?3, ?4)",
            [path, rule, &size.to_string(), &timestamp.to_string()],
        )?;
        Ok(())
    }

    pub fn is_recorded(&self, path: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT 1 FROM exclusions WHERE path = ?1")?;
        Ok(stmt.exists([path])?)
    }

    pub fn list_all(&self) -> Result<Vec<ExclusionRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT path, rule, size_bytes, created_at, last_checked_at
             FROM exclusions
             ORDER BY created_at DESC"
        )?;

        let records = stmt.query_map([], |row| {
            Ok(ExclusionRecord {
                path: row.get(0)?,
                rule: row.get(1)?,
                size_bytes: row.get(2)?,
                created_at: row.get(3)?,
                last_checked_at: row.get(4)?,
            })
        })?;

        let mut result = Vec::new();
        for record in records {
            result.push(record?);
        }
        Ok(result)
    }

    pub fn delete_record(&self, path: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM exclusions WHERE path = ?1", [path])?;
        Ok(())
    }

    pub fn update_metadata(&self, path: &str, size: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        conn.execute(
            "UPDATE exclusions SET size_bytes = ?1, last_checked_at = ?2 WHERE path = ?3",
            [&size.to_string(), &timestamp.to_string(), path],
        )?;
        Ok(())
    }
}
