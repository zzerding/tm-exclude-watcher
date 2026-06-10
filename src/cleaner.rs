// ABOUTME: 清理器，检查并更新数据库中的排除记录

use crate::{Database, TmUtilTrait};
use std::path::Path;

pub struct CleanStats {
    pub removed_count: usize,
    pub updated_count: usize,
    pub error_count: usize,
    pub errors: Vec<String>,
}

pub struct Cleaner {
    db: Database,
    tmutil: Box<dyn TmUtilTrait>,
}

impl Cleaner {
    pub fn new(db: Database, tmutil: Box<dyn TmUtilTrait>) -> Self {
        Self { db, tmutil }
    }

    pub fn clean(&self) -> Result<CleanStats, String> {
        let mut stats = CleanStats {
            removed_count: 0,
            updated_count: 0,
            error_count: 0,
            errors: Vec::new(),
        };

        let records = self.db.list_all().map_err(|e| e.to_string())?;

        for record in records {
            let path = Path::new(&record.path);

            if !path.exists() {
                // 目录不存在，清理
                if let Err(e) = self.tmutil.remove_exclusion(path) {
                    stats.errors.push(format!("移除排除失败 {}: {}", record.path, e));
                    stats.error_count += 1;
                    continue;
                }

                if let Err(e) = self.db.delete_record(&record.path) {
                    stats.errors.push(format!("删除记录失败 {}: {}", record.path, e));
                    stats.error_count += 1;
                    continue;
                }

                stats.removed_count += 1;
            } else {
                // 目录存在，更新元数据
                let size = 0; // TODO: 实际计算大小
                if let Err(e) = self.db.update_metadata(&record.path, size) {
                    stats.errors.push(format!("更新记录失败 {}: {}", record.path, e));
                    stats.error_count += 1;
                    continue;
                }

                stats.updated_count += 1;
            }
        }

        Ok(stats)
    }
}
