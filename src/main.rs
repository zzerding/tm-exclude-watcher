// ABOUTME: CLI 入口 - tm-watcher scan <path>

use anyhow::{Context, Result};
use std::path::PathBuf;
use tm_watcher::{Config, Database, RealTmBackend, Scanner};

fn main() {
    if let Err(err) = run() {
        eprintln!("错误: {}", err);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("scan") => {
            let path = args.get(2).context("用法: tm-watcher scan <path>")?;
            cmd_scan(path)
        }
        _ => {
            eprintln!("tm-watcher - macOS Time Machine 自动排除工具");
            eprintln!();
            eprintln!("用法:");
            eprintln!("  tm-watcher scan <path>    扫描指定路径并排除匹配的目录");
            std::process::exit(1);
        }
    }
}

fn cmd_scan(path: &str) -> Result<()> {
    let scan_path = PathBuf::from(expand_tilde(path));
    if !scan_path.exists() {
        anyhow::bail!("路径不存在: {}", scan_path.display());
    }

    // 加载配置（不存在时自动生成默认配置）
    let config_path = default_config_path()?;
    let config = Config::load_or_create(&config_path)?;

    // 初始化数据库
    let db_path = default_db_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建数据目录: {}", parent.display()))?;
    }
    let database = Database::new(&db_path)?;

    // 使用真实 tmutil 后端扫描
    let scanner = Scanner::with_backend(config, database, Box::new(RealTmBackend::new()))?;

    println!("扫描中: {}", scan_path.display());
    let result = scanner.scan(&scan_path)?;

    // 输出统计信息
    println!();
    println!("扫描完成:");
    println!("  新排除: {} 个目录", result.excluded_count);
    println!("  已跳过: {} 个目录（之前已排除）", result.skipped_count);
    if !result.errors.is_empty() {
        println!("  错误: {} 个", result.errors.len());
        for err in &result.errors {
            println!("    - {}", err);
        }
    }

    Ok(())
}

/// 默认配置文件路径: ~/.config/tm-watcher/config.toml
fn default_config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    Ok(home.join(".config/tm-watcher/config.toml"))
}

/// 默认数据库路径: ~/.local/share/tm-watcher/exclusions.db
fn default_db_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    Ok(home.join(".local/share/tm-watcher/exclusions.db"))
}

/// 展开路径中的 ~ 为用户主目录
fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest).to_string_lossy().into_owned();
    }
    path.to_string()
}
