// ABOUTME: 实时文件系统监控器，检测目录创建/删除事件并自动排除

use crate::{Database, RuleMatcher, TmUtilTrait};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::AbortHandle;

#[derive(Debug)]
pub enum WatchError {
    PathNotFound(PathBuf),
    NotifyError(String),
}

pub struct Watcher {
    database: Database,
    rule_matcher: RuleMatcher,
    tmutil: Arc<dyn TmUtilTrait>,
    pending_dirs: Mutex<HashMap<PathBuf, AbortHandle>>,
    delay: Duration,
}

impl Watcher {
    pub fn new(
        database: Database,
        rule_matcher: RuleMatcher,
        tmutil: Arc<dyn TmUtilTrait>,
        delay: Duration,
    ) -> Self {
        Self {
            database,
            rule_matcher,
            tmutil,
            pending_dirs: Mutex::new(HashMap::new()),
            delay,
        }
    }

    pub async fn watch(
        &self,
        path: &Path,
    ) -> Result<(), WatchError> {
        if !path.exists() {
            return Err(WatchError::PathNotFound(path.to_path_buf()));
        }

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        ).map_err(|e| WatchError::NotifyError(e.to_string()))?;

        watcher
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| WatchError::NotifyError(e.to_string()))?;

        println!("监控中: {} (按 Ctrl+C 停止)", path.display());

        while let Some(event) = rx.recv().await {
            for path in event.paths {
                if !path.is_dir() {
                    continue;
                }

                match event.kind {
                    EventKind::Create(_) => {
                        self.handle_create(path).await;
                    }
                    EventKind::Remove(_) => {
                        self.handle_remove(path).await;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    pub async fn handle_create(&self, path: PathBuf) {
        // 检查规则匹配
        if self.rule_matcher.should_exclude(&path).is_none() {
            return;
        }

        let mut pending = self.pending_dirs.lock().await;

        // 取消旧任务（如果存在）
        if let Some(old_handle) = pending.remove(&path) {
            old_handle.abort();
        }

        // 启动新任务
        let path_clone = path.clone();
        let delay = self.delay;
        let database = self.database.clone();
        let tmutil = Arc::clone(&self.tmutil);
        let rule_matcher = self.rule_matcher.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(delay).await;

            // 延迟后执行排除
            if let Some(rule) = rule_matcher.should_exclude(&path_clone) {
                let path_str = path_clone.to_string_lossy();

                // 检查是否已记录
                if let Ok(false) = database.is_recorded(&path_str) {
                    // 调用 tmutil
                    if let Err(e) = tmutil.add_exclusion(&path_clone) {
                        eprintln!("排除失败 {}: {}", path_str, e);
                        return;
                    }

                    // 记录到数据库
                    if let Ok(()) = database.record_exclusion(&path_str, rule, 0) {
                        println!("✓ 已排除: {}", path_str);
                    }
                }
            }
        });

        pending.insert(path, handle.abort_handle());
    }

    pub async fn handle_remove(&self, path: PathBuf) {
        let mut pending = self.pending_dirs.lock().await;

        // 取消待处理任务
        if let Some(handle) = pending.remove(&path) {
            handle.abort();
        }

        // 如果已记录，清理数据库
        let path_str = path.to_string_lossy();
        if let Ok(true) = self.database.is_recorded(&path_str) {
            let _ = self.database.delete_record(&path_str);
            let _ = self.tmutil.remove_exclusion(&path);
            println!("✓ 已清理: {}", path_str);
        }
    }

    pub async fn pending_count(&self) -> usize {
        self.pending_dirs.lock().await.len()
    }
}
