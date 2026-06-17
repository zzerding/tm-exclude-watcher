// ABOUTME: 清理器 - 对数据库排除记录做维护、删除失效路径并修复 Time Machine 排除状态

use crate::{Database, RealTmBackend, TmBackendError};
use anyhow::{Context, Result};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct Cleaner {
    database: Database,
    tm_backend: RealTmBackend,
}

pub struct CleanResult {
    pub cleaned_count: usize,
    pub checked_count: usize,
    pub errors: Vec<String>,
}

impl Cleaner {
    pub fn new(database: Database) -> Self {
        Self::with_tm_backend(database, RealTmBackend::new())
    }

    pub fn with_tm_backend(database: Database, tm_backend: RealTmBackend) -> Self {
        Self {
            database,
            tm_backend,
        }
    }

    pub fn clean(&self) -> Result<CleanResult> {
        let mut cleaned_count = 0;
        let mut checked_count = 0;
        let mut errors = Vec::new();
        let mut existing_records = Vec::new();

        for record in self.database.get_exclusions()? {
            match self.classify_record(record) {
                Ok(CleanTarget::Missing(path)) => match self.clean_missing_path(&path) {
                    Ok(()) => cleaned_count += 1,
                    Err(err) => {
                        tracing::warn!(
                            path = %path.display(),
                            error = %err,
                            "清理记录时遇到错误"
                        );
                        errors.push(format!("{}: {}", path.display(), err));
                    }
                },
                Ok(CleanTarget::Existing(target)) => existing_records.push(target),
                Err(err) => {
                    errors.push(err.to_string());
                }
            }
        }

        let exclusion_states = self.batch_exclusion_states(&existing_records);
        for (index, target) in existing_records.iter().enumerate() {
            let exclusion_state = exclusion_states
                .as_ref()
                .ok()
                .and_then(|states| states.get(index).copied());
            match self.clean_existing_path(target, exclusion_state) {
                Ok(()) => checked_count += 1,
                Err(err) => {
                    tracing::warn!(
                        path = %target.path.display(),
                        error = %err,
                        "清理记录时遇到错误"
                    );
                    errors.push(format!("{}: {}", target.path.display(), err));
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

    fn classify_record(&self, record: crate::ExclusionRecord) -> Result<CleanTarget> {
        let metadata = match fs::symlink_metadata(&record.path) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(CleanTarget::Missing(record.path));
            }
            Err(err) => return Err(err.into()),
        };

        let path_mtime_ns = path_mtime_ns(&record.path, &metadata)?;
        Ok(CleanTarget::Existing(CleanExistingTarget {
            path: record.path,
            size_bytes: record.size_bytes,
            recorded_path_mtime_ns: record.recorded_path_mtime_ns,
            metadata,
            path_mtime_ns,
        }))
    }

    fn batch_exclusion_states(&self, targets: &[CleanExistingTarget]) -> Result<Vec<bool>> {
        if targets.is_empty() {
            return Ok(Vec::new());
        }

        let paths = targets
            .iter()
            .map(|target| target.path.clone())
            .collect::<Vec<_>>();
        self.tm_backend.are_excluded(&paths)
    }

    fn clean_existing_path(
        &self,
        target: &CleanExistingTarget,
        batched_exclusion_state: Option<bool>,
    ) -> Result<()> {
        self.refresh_size_if_needed(target)?;

        let is_excluded = match batched_exclusion_state {
            Some(is_excluded) => is_excluded,
            None => self.tm_backend.is_excluded(&target.path)?,
        };
        if !is_excluded {
            tracing::warn!(path = %target.path.display(), "排除状态缺失，正在修复");
            self.tm_backend.add_exclusion(&target.path)?;
        }

        Ok(())
    }

    fn refresh_size_if_needed(&self, target: &CleanExistingTarget) -> Result<()> {
        if target.size_bytes.is_some()
            && target.recorded_path_mtime_ns == Some(target.path_mtime_ns)
        {
            self.database.touch_exclusion_check(&target.path)?;
            return Ok(());
        }

        let size_bytes = record_size(&target.path, &target.metadata)?;
        self.database
            .update_exclusion_check(&target.path, size_bytes, target.path_mtime_ns)
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

enum CleanTarget {
    Missing(PathBuf),
    Existing(CleanExistingTarget),
}

struct CleanExistingTarget {
    path: PathBuf,
    size_bytes: Option<i64>,
    recorded_path_mtime_ns: Option<i64>,
    metadata: fs::Metadata,
    path_mtime_ns: i64,
}

fn record_size(path: &Path, metadata: &fs::Metadata) -> Result<i64> {
    if !metadata.is_dir() {
        return Ok(metadata.len() as i64);
    }

    directory_size(path)
}

fn directory_size(path: &Path) -> Result<i64> {
    let mut total = 0;

    for entry in jwalk::WalkDir::new(path)
        .follow_links(false)
        .skip_hidden(false)
    {
        let entry = entry?;
        let file_type = entry.file_type();
        if file_type.is_file() || file_type.is_symlink() {
            total += fs::symlink_metadata(entry.path())?.len() as i64;
        }
    }

    Ok(total)
}

fn path_mtime_ns(path: &Path, metadata: &fs::Metadata) -> Result<i64> {
    let modified = metadata
        .modified()
        .with_context(|| format!("无法读取修改时间: {}", path.display()))?;
    let duration = modified
        .duration_since(UNIX_EPOCH)
        .with_context(|| format!("修改时间早于 Unix epoch: {}", path.display()))?;
    i64::try_from(duration.as_nanos())
        .with_context(|| format!("修改时间超出可存储范围: {}", path.display()))
}
