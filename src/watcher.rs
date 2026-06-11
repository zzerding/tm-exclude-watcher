// ABOUTME: 实时文件系统监控 - 检测目录创建/删除并自动应用排除规则

use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::{Config, Database, RuleMatcher, TmBackend};

pub struct Watcher {
    config: Config,
    database: Arc<Database>,
    tm_backend: Arc<dyn TmBackend>,
    matcher: RuleMatcher,
    pending_exclusions: Arc<Mutex<HashMap<PathBuf, JoinHandle<()>>>>,
}

impl Watcher {
    pub fn new(config: Config, database: Database, tm_backend: Box<dyn TmBackend>) -> Result<Self> {
        let matcher = RuleMatcher::new(config.exclude_rules.clone());
        Ok(Self {
            config,
            database: Arc::new(database),
            tm_backend: Arc::from(tm_backend),
            matcher,
            pending_exclusions: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn watch(&self, path: &Path) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })?;

        watcher
            .watch(path, RecursiveMode::Recursive)
            .context("无法启动文件系统监控")?;

        println!("开始监控: {}", path.display());
        println!("按 Ctrl+C 停止监控");

        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    self.handle_event(event).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    println!("\n正在停止监控...");
                    break;
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
                EventKind::Modify(notify::event::ModifyKind::Name(_)) if !path.exists() => {
                    self.handle_remove(path).await
                }
                _ => {}
            }
        }
    }

    async fn handle_create(&self, path: PathBuf) {
        if !path.is_dir() || path.is_symlink() {
            return;
        }

        if self.matcher.matches(&path).is_none() {
            return;
        }

        let already_recorded = tokio::task::spawn_blocking({
            let db = self.database.clone();
            let path = path.clone();
            move || db.has_exclusion(&path)
        })
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or(false);

        if already_recorded {
            return;
        }

        let delay = Duration::from_secs(self.config.confirmation_delay_seconds);
        let handle = tokio::spawn({
            let path = path.clone();
            let path_for_cleanup = path.clone();
            let db = self.database.clone();
            let tm = self.tm_backend.clone();
            let matcher = self.matcher.clone();
            let pending = self.pending_exclusions.clone();

            async move {
                tokio::time::sleep(delay).await;

                if !path.exists() {
                    pending.lock().await.remove(&path_for_cleanup);
                    return;
                }

                let rule = matcher
                    .matches(&path)
                    .unwrap_or_else(|| "unknown".to_string());
                let display_path = path.display().to_string();

                let exclude_result = tokio::task::spawn_blocking({
                    let tm = tm.clone();
                    let path = path.clone();
                    move || tm.add_exclusion(&path)
                })
                .await;

                if let Ok(Ok(())) = exclude_result {
                    let record_result = tokio::task::spawn_blocking(move || {
                        db.record_exclusion(&path, &rule, None)
                    })
                    .await;

                    match record_result {
                        Ok(Ok(())) => println!("✓ 已排除: {}", display_path),
                        Ok(Err(e)) => eprintln!("⚠ 已排除但记录失败: {} - {}", display_path, e),
                        Err(e) => eprintln!("⚠ 已排除但记录任务失败: {} - {}", display_path, e),
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
            println!("✗ 取消排除: {}", path.display());
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
        println!("🗑 清理记录: {}", display_path);
    }
}
