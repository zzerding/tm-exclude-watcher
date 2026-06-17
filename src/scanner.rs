// ABOUTME: 目录扫描器 - 并行递归扫描并排除匹配的目录

use crate::{Config, Database, TmBackend};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct Scanner {
    rules: Vec<String>,
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

pub struct ScanDryRunResult {
    pub to_exclude: Vec<ScanDryRunEntry>,
    pub skipped: Vec<ScanDryRunEntry>,
    /// 扫描过程中遇到的错误（如权限不足）
    pub errors: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ScanDryRunEntry {
    pub path: PathBuf,
    pub rule: String,
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
            rules: config.exclude_rules,
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
        let root_rule = path
            .file_name()
            .and_then(|basename| basename.to_str())
            .and_then(|basename| {
                self.rules
                    .iter()
                    .find(|rule| rule.as_str() == basename)
                    .cloned()
            });
        if let Some(rule) = root_rule {
            self.exclude_one(path, &rule, &mut excluded_count, &mut skipped_count)?;
            tracing::info!(
                path = %path.display(),
                excluded_count,
                skipped_count,
                error_count = errors.len(),
                "扫描完成"
            );
            return Ok(ScanResult {
                excluded_count,
                skipped_count,
                errors,
            });
        }

        // 剪枝回调在 rayon 线程上运行，需要独立的 matcher 副本
        // 更正：旧匹配器结构已移除，这里 clone 规则列表供回调使用。
        let rules = self.rules.clone();
        let walker = jwalk::WalkDir::new(path)
            .follow_links(false)
            .skip_hidden(false) // 规则含 .venv/.cache 等隐藏目录，必须关闭
            .process_read_dir(move |_depth, _path, _state, children| {
                // 并行读目录时剪枝：匹配的子目录不再下钻
                for child in children.iter_mut().flatten() {
                    if child.file_type.is_dir()
                        && rules
                            .iter()
                            .any(|rule| rule.as_str() == child.file_name.to_string_lossy())
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
                    tracing::warn!(error = %err, "跳过无法访问的路径");
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
            let entry_rule = entry_path
                .file_name()
                .and_then(|basename| basename.to_str())
                .and_then(|basename| {
                    self.rules
                        .iter()
                        .find(|rule| rule.as_str() == basename)
                        .cloned()
                });
            if let Some(rule) = entry_rule {
                self.exclude_one(&entry_path, &rule, &mut excluded_count, &mut skipped_count)?;
            }
        }

        tracing::info!(
            path = %path.display(),
            excluded_count,
            skipped_count,
            error_count = errors.len(),
            "扫描完成"
        );

        Ok(ScanResult {
            excluded_count,
            skipped_count,
            errors,
        })
    }

    /// 预览扫描结果，不调用 tmutil，也不写入数据库。
    pub fn dry_run(
        config: Config,
        database: Option<&Database>,
        path: &Path,
    ) -> Result<ScanDryRunResult> {
        let rules = config.exclude_rules;
        let mut result = ScanDryRunResult {
            to_exclude: Vec::new(),
            skipped: Vec::new(),
            errors: Vec::new(),
        };

        // 边界：扫描根目录本身就匹配规则时，直接处理并返回（无需遍历子树）
        let root_rule = path
            .file_name()
            .and_then(|basename| basename.to_str())
            .and_then(|basename| rules.iter().find(|rule| rule.as_str() == basename).cloned());
        if let Some(rule) = root_rule {
            push_dry_run_match(database, path, &rule, &mut result)?;
            tracing::debug!(
                path = %path.display(),
                would_exclude_count = result.to_exclude.len(),
                skipped_count = result.skipped.len(),
                error_count = result.errors.len(),
                "扫描预览完成"
            );
            return Ok(result);
        }

        // 剪枝回调在 rayon 线程上运行，需要独立的 matcher 副本
        // 更正：旧匹配器结构已移除，这里 clone 规则列表供回调使用。
        let walker_rules = rules.clone();
        let walker = jwalk::WalkDir::new(path)
            .follow_links(false)
            .skip_hidden(false) // 规则含 .venv/.cache 等隐藏目录，必须关闭
            .process_read_dir(move |_depth, _path, _state, children| {
                // 并行读目录时剪枝：匹配的子目录不再下钻
                for child in children.iter_mut().flatten() {
                    // 更正：符号链接本身也参与规则匹配，但不跟随目标。
                    if is_scan_candidate(&child.file_type)
                        && walker_rules
                            .iter()
                            .any(|rule| rule.as_str() == child.file_name.to_string_lossy())
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
                    tracing::warn!(error = %err, "跳过无法访问的路径");
                    result.errors.push(msg);
                    continue;
                }
            };

            // 只处理目录
            // 更正：符号链接本身也参与 basename 匹配，但不递归进入目标。
            if !is_scan_candidate(&entry.file_type()) {
                continue;
            }

            let entry_path = entry.path();

            // 检查是否匹配规则（匹配的目录已在回调中剪枝，这里负责预览分类）
            let entry_rule = entry_path
                .file_name()
                .and_then(|basename| basename.to_str())
                .and_then(|basename| rules.iter().find(|rule| rule.as_str() == basename).cloned());
            if let Some(rule) = entry_rule {
                push_dry_run_match(database, &entry_path, &rule, &mut result)?;
            }
        }

        tracing::debug!(
            path = %path.display(),
            would_exclude_count = result.to_exclude.len(),
            skipped_count = result.skipped.len(),
            error_count = result.errors.len(),
            "扫描预览完成"
        );

        Ok(result)
    }

    /// 排除单个目录：幂等检查 → tmutil 排除 → 数据库记录
    /// 更正：scan 热路径现在先信任数据库记录，已有记录时不调用 tmutil。
    /// 排除单个目录：数据库热路径检查 → tmutil 排除 → 数据库记录
    fn exclude_one(
        &self,
        path: &Path,
        rule: &str,
        excluded_count: &mut usize,
        skipped_count: &mut usize,
    ) -> Result<()> {
        if self.database.has_exclusion(path)? {
            *skipped_count += 1;
            return Ok(());
        }

        self.tm_backend.add_exclusion(path)?;
        self.database.record_exclusion(path, rule, None)?;
        *excluded_count += 1;
        tracing::info!(path = %path.display(), rule, "已排除扫描匹配目录");
        Ok(())
    }
}

fn push_dry_run_match(
    database: Option<&Database>,
    path: &Path,
    rule: &str,
    result: &mut ScanDryRunResult,
) -> Result<()> {
    let entry = ScanDryRunEntry {
        path: path.to_path_buf(),
        rule: rule.to_string(),
    };

    let already_recorded = database
        .map(|database| database.has_exclusion(path))
        .transpose()?
        .unwrap_or(false);
    if already_recorded {
        result.skipped.push(entry);
    } else {
        result.to_exclude.push(entry);
    }

    Ok(())
}

fn is_scan_candidate(file_type: &std::fs::FileType) -> bool {
    file_type.is_dir() || file_type.is_symlink()
}
