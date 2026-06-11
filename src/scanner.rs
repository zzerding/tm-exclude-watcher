// ABOUTME: 目录扫描器 - 并行递归扫描并排除匹配的目录

use crate::{Config, Database, RuleMatcher, TmBackend};
use anyhow::Result;
use std::path::Path;

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
    /// 性能设计：
    /// - jwalk 并行遍历（rayon 线程池读目录），实测比 walkdir 快 3-4x
    /// - 剪枝：匹配的目录在 process_read_dir 回调中被标记不再下钻。
    ///   Time Machine 排除是递归的，子树内嵌套的匹配目录无需单独排除
    pub fn scan(&self, path: &Path) -> Result<ScanResult> {
        let mut excluded_count = 0;
        let mut skipped_count = 0;
        let mut errors = Vec::new();

        // 边界：扫描根目录本身就匹配规则时，直接处理并返回（无需遍历子树）
        if let Some(rule) = self.matcher.matches(path) {
            self.exclude_one(path, &rule, &mut excluded_count, &mut skipped_count)?;
            return Ok(ScanResult {
                excluded_count,
                skipped_count,
                errors,
            });
        }

        // 剪枝回调在 rayon 线程上运行，需要独立的 matcher 副本
        let matcher = self.matcher.clone();
        let walker = jwalk::WalkDir::new(path)
            .follow_links(false)
            .skip_hidden(false) // 规则含 .venv/.cache 等隐藏目录，必须关闭
            .process_read_dir(move |_depth, _path, _state, children| {
                // 并行读目录时剪枝：匹配的子目录不再下钻
                for child in children.iter_mut().flatten() {
                    if child.file_type.is_dir()
                        && matcher
                            .matches_name(&child.file_name.to_string_lossy())
                            .is_some()
                    {
                        child.read_children_path = None; // 剪枝
                    }
                }
            });

        for entry in walker {
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

            // 只处理目录
            if !entry.file_type().is_dir() {
                continue;
            }

            let entry_path = entry.path();

            // 检查是否匹配规则（匹配的目录已在回调中剪枝，这里负责排除和记录）
            if let Some(rule) = self.matcher.matches(&entry_path) {
                self.exclude_one(&entry_path, &rule, &mut excluded_count, &mut skipped_count)?;
            }
        }

        Ok(ScanResult {
            excluded_count,
            skipped_count,
            errors,
        })
    }

    /// 排除单个目录：幂等检查 → tmutil 排除 → 数据库记录
    fn exclude_one(
        &self,
        path: &Path,
        rule: &str,
        excluded_count: &mut usize,
        skipped_count: &mut usize,
    ) -> Result<()> {
        if self.tm_backend.is_excluded(path)? {
            *skipped_count += 1;
        } else {
            self.tm_backend.add_exclusion(path)?;
            self.database.record_exclusion(path, rule, None)?;
            *excluded_count += 1;
        }
        Ok(())
    }
}
