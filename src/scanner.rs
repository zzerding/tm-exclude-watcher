// ABOUTME: 目录扫描器 - 递归扫描并排除匹配的目录

use crate::{Config, Database, RuleMatcher, TmBackend};
use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

pub struct Scanner {
    matcher: RuleMatcher,
    database: Database,
    tm_backend: Box<dyn TmBackend>,
}

pub struct ScanResult {
    /// 本次新排除的目录数量
    pub excluded_count: usize,
    /// 已排除而跳过的目录数量
    pub skipped_count: usize,
    /// 扫描过程中遇到的错误（如权限不足）
    pub errors: Vec<String>,
}

impl Scanner {
    /// 测试专用：注入 TmBackend
    pub fn with_backend(
        config: Config,
        database: Database,
        tm_backend: Box<dyn TmBackend>,
    ) -> Result<Self> {
        // 检查 Time Machine 是否配置
        if !tm_backend.check_configured()? {
            anyhow::bail!("Time Machine 未配置，无法启动");
        }

        Ok(Self {
            matcher: RuleMatcher::new(config.exclude_rules),
            database,
            tm_backend,
        })
    }

    /// 扫描指定路径，排除匹配的目录
    ///
    /// 性能优化：匹配的目录会被剪枝（skip_current_dir），不再遍历其子树。
    /// Time Machine 排除本身是递归的，子树内嵌套的匹配目录无需单独排除。
    pub fn scan(&self, path: &Path) -> Result<ScanResult> {
        let mut excluded_count = 0;
        let mut skipped_count = 0;
        let mut errors = Vec::new();

        let mut it = WalkDir::new(path).follow_links(false).into_iter();
        while let Some(entry) = it.next() {
            // 处理权限错误：记录但继续扫描
            let entry = match entry {
                Ok(e) => e,
                Err(err) => {
                    let msg = format!("跳过无法访问的路径: {}", err);
                    eprintln!("警告: {}", msg);
                    errors.push(msg);
                    continue;
                }
            };

            let entry_path = entry.path();

            // 只处理目录
            if !entry.file_type().is_dir() {
                continue;
            }

            // 检查是否匹配规则
            if let Some(rule) = self.matcher.matches(entry_path) {
                // 检查是否已排除（幂等性）
                if self.tm_backend.is_excluded(entry_path)? {
                    skipped_count += 1;
                } else {
                    // 添加排除
                    self.tm_backend.add_exclusion(entry_path)?;

                    // 记录到数据库
                    self.database.record_exclusion(entry_path, &rule, None)?;

                    excluded_count += 1;
                }

                // 剪枝：已排除的目录子树无需遍历
                it.skip_current_dir();
            }
        }

        Ok(ScanResult {
            excluded_count,
            skipped_count,
            errors,
        })
    }
}
