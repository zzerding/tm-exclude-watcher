// ABOUTME: 日志初始化 - 为 CLI stderr 与 daemon 文件日志配置 tracing subscriber

use anyhow::{Context, Result};
use std::path::Path;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;

pub struct LoggingGuard {
    _guard: Option<WorkerGuard>,
}

impl LoggingGuard {
    fn empty() -> Self {
        Self { _guard: None }
    }

    fn file(guard: WorkerGuard) -> Self {
        Self {
            _guard: Some(guard),
        }
    }
}

pub fn init_cli() -> Result<LoggingGuard> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_max_level(Level::INFO)
        .with_target(false)
        .try_init()
        .map_err(|err| anyhow::anyhow!("无法初始化 CLI 日志: {err}"))?;

    Ok(LoggingGuard::empty())
}

pub fn init_daemon(log_path: &Path) -> Result<LoggingGuard> {
    let (writer, guard) = daemon_writer(log_path)?;

    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .with_target(false)
        .try_init()
        .map_err(|err| anyhow::anyhow!("无法初始化守护进程日志: {err}"))?;

    Ok(LoggingGuard::file(guard))
}

fn daemon_writer(
    log_path: &Path,
) -> Result<(tracing_appender::non_blocking::NonBlocking, WorkerGuard)> {
    let parent = log_path.parent().context("daemon 日志路径缺少父目录")?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("无法创建日志目录: {}", parent.display()))?;

    let file_name = log_path
        .file_name()
        .context("daemon 日志路径缺少文件名")?
        .to_string_lossy()
        .into_owned();
    let appender = tracing_appender::rolling::never(parent, file_name);

    Ok(tracing_appender::non_blocking(appender))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn daemon_writer_creates_parent_directory_and_writes_info() {
        let temp_dir = TempDir::new().unwrap();
        let log_path = temp_dir.path().join("nested/daemon.log");
        let (writer, guard) = daemon_writer(&log_path).unwrap();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(writer)
            .with_max_level(Level::INFO)
            .with_ansi(false)
            .with_target(false)
            .finish();

        tracing::subscriber::with_default(subscriber, || {
            tracing::info!("daemon 日志初始化测试");
        });
        drop(guard);

        let content = std::fs::read_to_string(log_path).unwrap();
        assert!(content.contains("daemon 日志初始化测试"));
    }
}
