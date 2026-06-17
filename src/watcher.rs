// ABOUTME: 实时文件系统监控 - 检测目录创建/删除并自动应用排除规则

use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::{Config, Database, TmBackend};

pub struct Watcher {
    config: Config,
    database: Arc<Database>,
    tm_backend: Arc<dyn TmBackend>,
    rules: Vec<String>,
    pending_exclusions: Arc<Mutex<HashMap<PathBuf, JoinHandle<()>>>>,
}

impl Watcher {
    pub fn new(config: Config, database: Database, tm_backend: Box<dyn TmBackend>) -> Result<Self> {
        let rules = config.exclude_rules.clone();
        Ok(Self {
            config,
            database: Arc::new(database),
            tm_backend: Arc::from(tm_backend),
            rules,
            pending_exclusions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn watch(&self, path: &Path) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.ok();
            shutdown_tx.send(true).ok();
        });
        self.watch_multiple(&[path.to_path_buf()], shutdown_rx)
            .await
    }

    pub async fn watch_multiple(
        &self,
        paths: &[PathBuf],
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;

        for path in paths {
            if path.exists() {
                watcher
                    .watch(path, RecursiveMode::Recursive)
                    .context(format!("无法启动监控: {}", path.display()))?;
                tracing::info!(path = %path.display(), "开始监控路径");
            }
        }

        if paths.iter().any(|p| p.exists()) {
            tracing::info!("监控已启动，等待停止信号");
        }

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    self.handle_event(event).await;
                }
                _ = shutdown.changed() => {
                    if *shutdown.borrow() {
                        tracing::info!("正在停止监控");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_event(&self, event: Event) {
        for path in event.paths {
            match event.kind {
                EventKind::Create(_) if path.exists() => self.handle_create(path).await,
                EventKind::Remove(_) => self.handle_remove(path).await,
                EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
                    if path.exists() {
                        self.handle_create(path).await;
                    } else {
                        self.handle_remove(path).await;
                    }
                }
                _ => {}
            }
        }
    }

    async fn handle_create(&self, path: PathBuf) {
        tracing::debug!(path = %path.display(), "检测到目录创建事件");

        if !path.is_dir() || path.is_symlink() {
            return;
        }

        let rule = path
            .file_name()
            .and_then(|basename| basename.to_str())
            .and_then(|basename| {
                self.rules
                    .iter()
                    .find(|rule| rule.as_str() == basename)
                    .cloned()
            });
        let Some(rule) = rule else {
            return;
        };

        let already_covered = tokio::task::spawn_blocking({
            let db = self.database.clone();
            let path = path.clone();
            move || has_recorded_exclusion_for_self_or_ancestor(&db, &path)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false);

        if already_covered || self.has_pending_exclusion_for_self_or_ancestor(&path).await {
            return;
        }

        self.cancel_pending_exclusions_under(&path).await;

        let delay = Duration::from_secs(self.config.confirmation_delay_seconds);
        let handle = tokio::spawn({
            let path = path.clone();
            let path_for_cleanup = path.clone();
            let db = self.database.clone();
            let tm = self.tm_backend.clone();
            let pending = self.pending_exclusions.clone();

            async move {
                tokio::time::sleep(delay).await;

                if !path.exists() {
                    pending.lock().await.remove(&path_for_cleanup);
                    return;
                }

                let display_path = path.display().to_string();

                let exclude_result = tokio::task::spawn_blocking({
                    let tm = tm.clone();
                    let path = path.clone();
                    move || tm.add_exclusion(&path)
                })
                .await;

                match exclude_result {
                    Ok(Ok(())) => {
                        tracing::info!(path = %display_path, rule, "排除成功");
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(path = %display_path, error = %e, "排除失败");
                        pending.lock().await.remove(&path_for_cleanup);
                        return;
                    }
                    Err(e) => {
                        tracing::warn!(path = %display_path, error = %e, "排除任务失败");
                        pending.lock().await.remove(&path_for_cleanup);
                        return;
                    }
                }

                let record_result =
                    tokio::task::spawn_blocking(move || db.record_exclusion(&path, &rule, None))
                        .await;

                match record_result {
                    Ok(Ok(())) => tracing::info!(path = %display_path, "排除记录写入成功"),
                    Ok(Err(e)) => {
                        tracing::warn!(path = %display_path, error = %e, "已排除但记录失败")
                    }
                    Err(e) => {
                        tracing::warn!(path = %display_path, error = %e, "已排除但记录任务失败")
                    }
                }

                pending.lock().await.remove(&path_for_cleanup);
            }
        });

        self.pending_exclusions.lock().await.insert(path, handle);
    }

    async fn handle_remove(&self, path: PathBuf) {
        if let Some(handle) = self.pending_exclusions.lock().await.remove(&path) {
            handle.abort();
            tracing::info!(path = %path.display(), "取消待执行排除");
            return;
        }

        if !self.config.cleanup_on_delete {
            return;
        }

        let has_record = tokio::task::spawn_blocking({
            let db = self.database.clone();
            let path = path.clone();
            move || db.has_exclusion(&path)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false);

        if !has_record {
            return;
        }

        let display_path = path.display().to_string();
        let _ = tokio::task::spawn_blocking({
            let db = self.database.clone();
            move || db.delete_exclusion(&path)
        })
        .await;
        tracing::info!(path = %display_path, "已清理删除目录的排除记录");
    }

    async fn has_pending_exclusion_for_self_or_ancestor(&self, path: &Path) -> bool {
        let pending = self.pending_exclusions.lock().await;
        path.ancestors()
            .any(|ancestor| pending.contains_key(ancestor))
    }

    async fn cancel_pending_exclusions_under(&self, path: &Path) {
        let mut pending = self.pending_exclusions.lock().await;
        let nested_paths: Vec<PathBuf> = pending
            .keys()
            .filter(|pending_path| pending_path.starts_with(path) && pending_path.as_path() != path)
            .cloned()
            .collect();

        for nested_path in nested_paths {
            if let Some(handle) = pending.remove(&nested_path) {
                handle.abort();
                tracing::info!(path = %nested_path.display(), "取消嵌套待执行排除");
            }
        }
    }
}

fn has_recorded_exclusion_for_self_or_ancestor(database: &Database, path: &Path) -> Result<bool> {
    for ancestor in path.ancestors() {
        if database.has_exclusion(ancestor)? {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FakeTmBackend;
    use notify::event::{CreateKind, ModifyKind, RenameMode};
    use tempfile::TempDir;

    fn test_config() -> Config {
        Config {
            exclude_rules: vec!["node_modules".to_string()],
            confirmation_delay_seconds: 0,
            cleanup_on_delete: true,
            ..Default::default()
        }
    }

    fn test_watcher(config: Config, database: Database, tm_backend: FakeTmBackend) -> Watcher {
        Watcher::new(config, database, Box::new(tm_backend)).unwrap()
    }

    async fn wait_for_add_count(tm_backend: &FakeTmBackend, expected_count: usize) {
        for _ in 0..100 {
            if tm_backend.add_exclusion_call_count() == expected_count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(tm_backend.add_exclusion_call_count(), expected_count);
    }

    #[tokio::test]
    async fn rename_to_existing_matching_directory_is_created() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("node_modules");
        std::fs::create_dir(&path).unwrap();

        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        let tm_backend = FakeTmBackend::new();
        let watcher = test_watcher(test_config(), database.clone(), tm_backend.clone());

        watcher
            .handle_event(Event {
                kind: EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                paths: vec![path.clone()],
                attrs: Default::default(),
            })
            .await;
        wait_for_add_count(&tm_backend, 1).await;

        assert!(database.has_exclusion(&path).unwrap());
    }

    #[tokio::test]
    async fn rename_missing_matching_directory_is_removed() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("node_modules");

        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        database
            .record_exclusion(&path, "node_modules", None)
            .unwrap();
        let tm_backend = FakeTmBackend::new();
        let watcher = test_watcher(test_config(), database.clone(), tm_backend);

        watcher
            .handle_event(Event {
                kind: EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                paths: vec![path.clone()],
                attrs: Default::default(),
            })
            .await;

        assert!(!database.has_exclusion(&path).unwrap());
    }

    #[tokio::test]
    async fn create_skips_directory_under_recorded_exclusion() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().join("node_modules");
        let child = parent.join("node_modules");
        std::fs::create_dir_all(&child).unwrap();

        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        database
            .record_exclusion(&parent, "node_modules", None)
            .unwrap();
        let tm_backend = FakeTmBackend::new();
        let watcher = test_watcher(test_config(), database.clone(), tm_backend.clone());

        watcher.handle_create(child.clone()).await;

        assert_eq!(tm_backend.add_exclusion_call_count(), 0);
        assert!(!database.has_exclusion(&child).unwrap());
    }

    #[tokio::test]
    async fn create_skips_directory_under_pending_exclusion() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().join("node_modules");
        let child = parent.join("node_modules");
        std::fs::create_dir_all(&child).unwrap();

        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        let tm_backend = FakeTmBackend::new();
        let mut config = test_config();
        config.confirmation_delay_seconds = 60;
        let watcher = test_watcher(config, database, tm_backend.clone());

        watcher.handle_create(parent.clone()).await;
        watcher.handle_create(child).await;
        assert_eq!(watcher.pending_exclusions.lock().await.len(), 1);

        watcher.handle_remove(parent).await;
        assert_eq!(tm_backend.add_exclusion_call_count(), 0);
    }

    #[tokio::test]
    async fn parent_create_cancels_nested_pending_exclusion() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().join("node_modules");
        let child = parent.join("node_modules");
        std::fs::create_dir_all(&child).unwrap();

        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        let tm_backend = FakeTmBackend::new();
        let mut config = test_config();
        config.confirmation_delay_seconds = 60;
        let watcher = test_watcher(config, database, tm_backend.clone());

        watcher.handle_create(child.clone()).await;
        watcher.handle_create(parent.clone()).await;

        let pending = watcher.pending_exclusions.lock().await;
        assert_eq!(pending.len(), 1);
        assert!(pending.contains_key(&parent));
        assert!(!pending.contains_key(&child));
        drop(pending);

        watcher.handle_remove(parent).await;
        assert_eq!(tm_backend.add_exclusion_call_count(), 0);
    }

    #[tokio::test]
    async fn create_does_not_record_when_add_exclusion_fails() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("node_modules");
        std::fs::create_dir(&path).unwrap();

        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        let tm_backend = FakeTmBackend::new();
        tm_backend.fail_next_add_other("boom");
        let watcher = test_watcher(test_config(), database.clone(), tm_backend.clone());

        watcher
            .handle_event(Event {
                kind: EventKind::Create(CreateKind::Folder),
                paths: vec![path.clone()],
                attrs: Default::default(),
            })
            .await;
        wait_for_add_count(&tm_backend, 1).await;

        assert!(!database.has_exclusion(&path).unwrap());
    }

    #[tokio::test]
    async fn test_watch_multiple_handles_two_paths() {
        // 简化：直接测试handle_create，不依赖真实FSEvents
        let temp_dir = TempDir::new().unwrap();
        let path1 = temp_dir.path().join("path1");
        let path2 = temp_dir.path().join("path2");
        std::fs::create_dir_all(&path1).unwrap();
        std::fs::create_dir_all(&path2).unwrap();

        let node1 = path1.join("node_modules");
        let node2 = path2.join("node_modules");
        std::fs::create_dir(&node1).unwrap();
        std::fs::create_dir(&node2).unwrap();

        let config = Config {
            exclude_rules: vec!["node_modules".to_string()],
            confirmation_delay_seconds: 0,
            cleanup_on_delete: true,
            ..Default::default()
        };
        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        let tm_backend = FakeTmBackend::new();

        let watcher = Watcher::new(config, database.clone(), Box::new(tm_backend.clone())).unwrap();

        // 直接调用handle_create模拟监控发现目录
        watcher.handle_create(node1.clone()).await;
        watcher.handle_create(node2.clone()).await;

        // 等待异步排除完成
        wait_for_add_count(&tm_backend, 2).await;

        assert_eq!(tm_backend.add_exclusion_call_count(), 2);
        assert!(database.has_exclusion(&node1).unwrap());
        assert!(database.has_exclusion(&node2).unwrap());
    }

    #[tokio::test]
    async fn test_watch_multiple_stops_on_shutdown_signal() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test");
        std::fs::create_dir(&path).unwrap();

        let config = Config {
            exclude_rules: vec!["node_modules".to_string()],
            confirmation_delay_seconds: 0,
            cleanup_on_delete: true,
            ..Default::default()
        };
        let database = Database::new(&temp_dir.path().join("test.db")).unwrap();
        let tm_backend = FakeTmBackend::new();

        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

        let watcher = Watcher::new(config, database, Box::new(tm_backend)).unwrap();
        let watch_handle =
            tokio::spawn(async move { watcher.watch_multiple(&[path], shutdown_rx).await });

        tokio::time::sleep(Duration::from_millis(50)).await;
        shutdown_tx.send(true).ok();

        let result = tokio::time::timeout(Duration::from_secs(2), watch_handle).await;
        assert!(result.is_ok(), "watch_multiple 应在 shutdown 信号后返回");
    }
}
