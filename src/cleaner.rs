// ABOUTME: 清理器 - 对数据库排除记录做维护、删除失效路径并修复 Time Machine 排除状态

use crate::{Database, TmBackend, TmBackendError};
use anyhow::Result;
use std::fs;
use std::io;
use std::path::Path;

pub struct Cleaner {
    database: Database,
    tm_backend: Box<dyn TmBackend>,
}

pub struct CleanResult {
    pub cleaned_count: usize,
    pub checked_count: usize,
    pub errors: Vec<String>,
}

impl Cleaner {
    pub fn new(database: Database, tm_backend: Box<dyn TmBackend>) -> Self {
        Self {
            database,
            tm_backend,
        }
    }

    pub fn clean(&self) -> Result<CleanResult> {
        let mut cleaned_count = 0;
        let mut checked_count = 0;
        let mut errors = Vec::new();

        for record in self.database.get_exclusions()? {
            match self.clean_one(&record.path) {
                Ok(CleanAction::Cleaned) => cleaned_count += 1,
                Ok(CleanAction::Checked) => checked_count += 1,
                Err(err) => {
                    tracing::warn!(
                        path = %record.path.display(),
                        error = %err,
                        "清理记录时遇到错误"
                    );
                    errors.push(format!("{}: {}", record.path.display(), err));
                }
            }
        }

        tracing::info!(
            cleaned_count,
            checked_count,
            error_count = errors.len(),
            "清理完成"
        );

        Ok(CleanResult {
            cleaned_count,
            checked_count,
            errors,
        })
    }

    fn clean_one(&self, path: &Path) -> Result<CleanAction> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                self.clean_missing_path(path)?;
                return Ok(CleanAction::Cleaned);
            }
            Err(err) => return Err(err.into()),
        };

        let size_bytes = record_size(path, &metadata)?;
        self.database.update_exclusion_check(path, size_bytes)?;
        if !self.tm_backend.is_excluded(path)? {
            tracing::warn!(path = %path.display(), "排除状态缺失，正在修复");
            self.tm_backend.add_exclusion(path)?;
        }

        Ok(CleanAction::Checked)
    }

    fn clean_missing_path(&self, path: &Path) -> Result<()> {
        let remove_result = self.tm_backend.remove_exclusion(path);
        if let Err(err) = remove_result
            && err.downcast_ref::<TmBackendError>().is_none()
        {
            return Err(err);
        }

        self.database.delete_exclusion(path)?;
        tracing::info!(path = %path.display(), "已清理失效排除记录");
        Ok(())
    }
}

enum CleanAction {
    Cleaned,
    Checked,
}

fn record_size(path: &Path, metadata: &fs::Metadata) -> Result<i64> {
    if !metadata.is_dir() {
        return Ok(metadata.len() as i64);
    }

    directory_size(path)
}

fn directory_size(path: &Path) -> Result<i64> {
    let mut total = 0;

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let metadata = fs::symlink_metadata(entry.path())?;
        if metadata.is_dir() {
            total += directory_size(&entry.path())?;
        } else if metadata.is_file() || metadata.file_type().is_symlink() {
            total += metadata.len() as i64;
        }
    }

    Ok(total)
}
