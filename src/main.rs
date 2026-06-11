// ABOUTME: CLI 入口 - tm-watcher scan/list/clean/watch/start/stop/status

use anyhow::{Context, Result};
use std::path::PathBuf;
use tm_watcher::{
    Cleaner, Config, Database, RealTmBackend, Scanner, Watcher, check_tm_configured,
    cmd_start, cmd_status, cmd_stop, format_exclusion_list,
};

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
        Some("list") => cmd_list(),
        Some("clean") => cmd_clean(),
        Some("watch") => {
            let path = args.get(2).context("用法: tm-watcher watch <path>")?;
            cmd_watch(path)
        }
        Some("start") => cmd_start_wrapper(),
        Some("stop") => cmd_stop_wrapper(),
        Some("status") => cmd_status_wrapper(),
        Some("__daemon") => cmd_daemon_wrapper(),
        _ => {
            eprintln!("tm-watcher - macOS Time Machine 自动排除工具");
            eprintln!();
            eprintln!("用法:");
            eprintln!("  tm-watcher scan <path>    扫描指定路径并排除匹配的目录");
            eprintln!("  tm-watcher list           显示已记录的排除目录");
            eprintln!("  tm-watcher clean          清理失效记录并检查排除状态");
            eprintln!("  tm-watcher watch <path>   实时监控路径并自动排除匹配目录");
            eprintln!("  tm-watcher start          启动守护进程（后台监控+定期清理）");
            eprintln!("  tm-watcher stop           停止守护进程");
            eprintln!("  tm-watcher status         显示守护进程状态");
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
    let database = open_default_database()?;

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

fn cmd_list() -> Result<()> {
    let database = open_default_database()?;
    let records = database.get_exclusions()?;
    println!("{}", format_exclusion_list(&records));
    Ok(())
}

fn cmd_clean() -> Result<()> {
    let database = open_default_database()?;
    let cleaner = Cleaner::new(database, Box::new(RealTmBackend::new()));
    let result = cleaner.clean()?;

    println!("清理完成:");
    println!("  清理: {} 条记录", result.cleaned_count);
    println!("  检查: {} 条记录", result.checked_count);
    println!("  错误: {} 个", result.errors.len());
    if !result.errors.is_empty() {
        for err in &result.errors {
            println!("    - {}", err);
        }
    }

    Ok(())
}

fn cmd_watch(path: &str) -> Result<()> {
    let watch_path = PathBuf::from(expand_tilde(path));
    if !watch_path.exists() {
        anyhow::bail!("路径不存在: {}", watch_path.display());
    }

    let config_path = default_config_path()?;
    let config = Config::load_or_create(&config_path)?;

    let database = open_default_database()?;

    let watcher = Watcher::new(config, database, Box::new(RealTmBackend::new()))?;

    tokio::runtime::Runtime::new()?.block_on(watcher.watch(&watch_path))
}

fn open_default_database() -> Result<Database> {
    let db_path = default_db_path()?;
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建数据目录: {}", parent.display()))?;
    }
    Database::new(&db_path)
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

fn cmd_start_wrapper() -> Result<()> {
    let config_path = default_config_path()?;
    let db_path = default_db_path()?;
    let pid_path = default_pid_path()?;
    let log_path = default_log_path()?;

    cmd_start(&config_path, &db_path, &pid_path, &log_path)
}

fn cmd_stop_wrapper() -> Result<()> {
    let pid_path = default_pid_path()?;
    cmd_stop(&pid_path)
}

fn cmd_status_wrapper() -> Result<()> {
    let config_path = default_config_path()?;
    let config = Config::load_or_create(&config_path)?;
    let database = open_default_database()?;
    let pid_path = default_pid_path()?;

    cmd_status(&config, &database, &pid_path)
}

fn cmd_daemon_wrapper() -> Result<()> {
    use tm_watcher::run_periodic_cleanup;
    use tokio::sync::watch;

    let config_path = default_config_path()?;
    let config = Config::load_or_create(&config_path)?;
    
    let database = open_default_database()?;
    let tm_backend = Box::new(RealTmBackend::new());
    let pid_path = default_pid_path()?;

    // 检查 Time Machine 是否配置
    check_tm_configured(tm_backend.as_ref())?;

    println!("守护进程启动中...");
    println!("配置文件: {}", config_path.display());
    println!("数据库: {}", default_db_path()?.display());

    // 设置 shutdown 信号
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    
    tokio::runtime::Runtime::new()?.block_on(async move {
        // 处理 SIGTERM
        tokio::spawn(async move {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .unwrap()
                .recv()
                .await;
            println!("收到停止信号，正在退出...");
            shutdown_tx.send(true).ok();
        });

        // 启动多路径监控
        let watch_paths: Vec<PathBuf> = config
            .watch_paths
            .iter()
            .map(|p| PathBuf::from(expand_tilde(p)))
            .filter(|p| p.exists())
            .collect();

        let watch_handle = if !watch_paths.is_empty() {
            let watcher_clone = Watcher::new(config.clone(), database.clone(), Box::new(RealTmBackend::new()))?;
            let shutdown_rx_clone = shutdown_rx.clone();
            Some(tokio::spawn(async move {
                watcher_clone.watch_multiple(&watch_paths, shutdown_rx_clone).await
            }))
        } else {
            None
        };

        // 启动定期清理
        let cleanup_handle = tokio::spawn({
            let db = database.clone();
            let interval = config.interval_hours;
            let shutdown_rx_clone = shutdown_rx.clone();
            async move {
                run_periodic_cleanup(db, || Box::new(RealTmBackend::new()), interval, shutdown_rx_clone).await;
            }
        });

        // 等待 shutdown
        shutdown_rx.clone().changed().await.ok();

        // 等待任务完成
        if let Some(handle) = watch_handle {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        }
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), cleanup_handle).await;

        // 删除 PID 文件
        let _ = std::fs::remove_file(&pid_path);

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

/// 默认 PID 文件路径
fn default_pid_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    Ok(home.join(".local/var/run/tm-watcher.pid"))
}

/// 默认日志文件路径
fn default_log_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    Ok(home.join(".local/share/tm-watcher/daemon.log"))
}
