// ABOUTME: CLI 入口 - tm-watcher scan/list/clean/watch/start/stop/status
// 更正：公开 daemon 生命周期命令已迁移到 `tm-watcher daemon ...` 子命令。

use anyhow::{Context, Result};
use std::path::PathBuf;
use tm_watcher::{
    CONFIG_RESTART_HINT, Cleaner, Config, ConfigUpdate, Database, LaunchAgentDoctorState,
    RealTmBackend, ScanDryRunEntry, ScanDryRunResult, Scanner, Watcher, check_tm_configured,
    cmd_restart, cmd_start, cmd_status, cmd_stop, expand_tilde_path, format_exclusion_list,
    logging, run_doctor_checks,
};

fn main() {
    let is_daemon = std::env::args().nth(1).as_deref() == Some("__daemon");
    let _logging_guard = match init_logging(is_daemon) {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("错误: {}", err);
            std::process::exit(1);
        }
    };

    if let Err(err) = run() {
        tracing::error!("错误: {:#}", err);
        std::process::exit(1);
    }
}

fn init_logging(is_daemon: bool) -> Result<logging::LoggingGuard> {
    if is_daemon {
        return logging::init_daemon(&default_log_path()?);
    }

    logging::init_cli()
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("tm-watcher {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help") | Some("-h") => {
            print!("{HELP_TEXT}");
            Ok(())
        }
        Some("scan") => cmd_scan_wrapper(&args[2..]),
        Some("list") => cmd_list(),
        Some("clean") => cmd_clean(),
        Some("logs") => cmd_logs_wrapper(&args[2..]),
        Some("config") => cmd_config_wrapper(&args[2..]),
        Some("watch") => {
            let path = args.get(2).context("用法: tm-watcher watch <path>")?;
            cmd_watch(path)
        }
        Some("daemon") => cmd_daemon_command(&args[2..]),
        Some("start") => bail_migrated_command("start", "daemon start"),
        Some("stop") => bail_migrated_command("stop", "daemon stop"),
        Some("status") => bail_migrated_command("status", "daemon status"),
        Some("doctor") => cmd_doctor_wrapper(),
        Some("__daemon") => cmd_daemon_entrypoint(),
        _ => {
            eprint!("{HELP_TEXT}");
            std::process::exit(1);
        }
    }
}

const HELP_TEXT: &str = "tm-watcher - macOS Time Machine 自动排除工具

用法:
Daemon 生命周期:
  tm-watcher daemon start   启动守护进程（后台监控+定期清理）
  tm-watcher daemon stop    停止守护进程
  tm-watcher daemon restart 重启守护进程
  tm-watcher daemon status  显示守护进程状态

扫描与清理:
  tm-watcher scan <path>    扫描指定路径并排除匹配的目录
  tm-watcher scan <path> --dry-run
  tm-watcher scan --dry-run <path>
                            预览将排除的目录，不调用 tmutil，不写数据库
  tm-watcher list           显示已记录的排除目录
  tm-watcher clean          清理失效记录并检查排除状态

诊断与日志:
  tm-watcher logs [-n <行数>] [--follow]
                            显示 daemon 日志
  tm-watcher doctor         运行系统健康检查

配置管理:
  tm-watcher config show    显示配置
  tm-watcher config add-path <路径>
                            添加监控路径
  tm-watcher config add-rule <规则>
                            添加排除规则

前台调试:
  tm-watcher watch <path>   实时监控路径并自动排除匹配目录
";

const DAEMON_HELP_TEXT: &str = "tm-watcher daemon - 管理后台守护进程

用法:
  tm-watcher daemon start   启动守护进程（后台监控+定期清理）
  tm-watcher daemon stop    停止守护进程
  tm-watcher daemon restart 重启守护进程
  tm-watcher daemon status  显示守护进程状态
";

const CONFIG_HELP_TEXT: &str = "tm-watcher config - 查看和更新配置

用法:
  tm-watcher config show             显示配置
  tm-watcher config add-path <路径>  添加监控路径
  tm-watcher config add-rule <规则>  添加排除规则
";

fn bail_migrated_command<T>(command: &str, suggestion: &str) -> Result<T> {
    anyhow::bail!("未知命令 '{command}'，你是想用 '{suggestion}' 吗？")
}

fn cmd_scan_wrapper(args: &[String]) -> Result<()> {
    match args {
        [flag, path] if flag == "--dry-run" => cmd_scan_dry_run(path),
        [path, flag] if flag == "--dry-run" => cmd_scan_dry_run(path),
        [path] if path != "--dry-run" => cmd_scan(path),
        _ => anyhow::bail!("用法: tm-watcher scan <path> [--dry-run]"),
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

fn cmd_scan_dry_run(path: &str) -> Result<()> {
    let scan_path = PathBuf::from(expand_tilde(path));
    if !scan_path.exists() {
        anyhow::bail!("路径不存在: {}", scan_path.display());
    }

    let config_path = default_config_path()?;
    let config = load_config_for_dry_run(&config_path)?;

    let db_path = default_db_path()?;
    let database = Database::open_read_only_if_exists(&db_path)?;

    let result = Scanner::dry_run(config, database.as_ref(), &scan_path)?;
    print!("{}", format_scan_dry_run(path, &scan_path, &result));

    Ok(())
}

fn load_config_for_dry_run(config_path: &std::path::Path) -> Result<Config> {
    if config_path.exists() {
        return Config::load_or_create(config_path);
    }

    Ok(Config::default_config())
}

fn format_scan_dry_run(
    input_path: &str,
    scan_path: &std::path::Path,
    result: &ScanDryRunResult,
) -> String {
    let mut output = format!("扫描预览: {}\n\n", format_display_path(scan_path));
    output.push_str(&format!(
        "将要排除的目录（{} 个）:\n",
        result.to_exclude.len()
    ));
    append_dry_run_entries(&mut output, &result.to_exclude);

    output.push('\n');
    output.push_str(&format!(
        "已跳过（之前已排除）: {} 个\n",
        result.skipped.len()
    ));
    append_dry_run_entries(&mut output, &result.skipped);

    if !result.errors.is_empty() {
        output.push('\n');
        output.push_str(&format!("错误: {} 个\n", result.errors.len()));
        for err in &result.errors {
            output.push_str(&format!("  - {err}\n"));
        }
    }

    output.push('\n');
    output.push_str(&format!(
        "提示: 使用 'tm-watcher scan {input_path}' 执行实际排除\n"
    ));
    output
}

fn append_dry_run_entries(output: &mut String, entries: &[ScanDryRunEntry]) {
    if entries.is_empty() {
        output.push_str("  无\n");
        return;
    }

    for entry in entries {
        output.push_str(&format!(
            "  {}  (匹配规则: {})\n",
            format_display_path(&entry.path),
            entry.rule
        ));
    }
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

fn cmd_logs_wrapper(args: &[String]) -> Result<()> {
    tm_watcher::cmd_logs(&default_log_path()?, args)
}

fn cmd_config_wrapper(args: &[String]) -> Result<()> {
    if matches!(args, [flag] if flag == "--help" || flag == "-h") {
        print!("{CONFIG_HELP_TEXT}");
        return Ok(());
    }

    let config_path = default_config_path()?;
    let operation = parse_config_operation(args)?;
    let mut config = Config::load_or_create(&config_path)?;

    match operation {
        ConfigOperation::Show => {
            print!("{}", config.render(&config_path));
        }
        ConfigOperation::AddPath(path) => {
            let update = config.add_path(&expand_tilde_path(path))?;
            print_config_update(update, &config, &config_path)?;
        }
        ConfigOperation::AddRule(rule) => {
            let update = config.add_rule(&rule)?;
            print_config_update(update, &config, &config_path)?;
        }
    }

    Ok(())
}

enum ConfigOperation {
    Show,
    AddPath(String),
    AddRule(String),
}

fn parse_config_operation(args: &[String]) -> Result<ConfigOperation> {
    match args {
        [command] if command == "show" => Ok(ConfigOperation::Show),
        [command, value] if command == "add-path" => Ok(ConfigOperation::AddPath(value.clone())),
        [command, value] if command == "add-rule" => Ok(ConfigOperation::AddRule(value.clone())),
        [flag] if flag == "--show" => bail_migrated_command("config --show", "config show"),
        [flag, ..] if flag == "--add-path" => {
            bail_migrated_command("config --add-path", "config add-path")
        }
        [flag, ..] if flag == "--add-rule" => {
            bail_migrated_command("config --add-rule", "config add-rule")
        }
        _ => anyhow::bail!("用法: tm-watcher config show | add-path <路径> | add-rule <规则>"),
    }
}

fn print_config_update(
    update: ConfigUpdate,
    config: &Config,
    config_path: &std::path::Path,
) -> Result<()> {
    match update {
        ConfigUpdate::Updated(message) => {
            config.save(config_path)?;
            println!("{message}");
            println!("{CONFIG_RESTART_HINT}");
        }
        ConfigUpdate::Skipped(message) => println!("{message}"),
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

fn format_display_path(path: &std::path::Path) -> String {
    let Some(home) = dirs::home_dir() else {
        return path.display().to_string();
    };

    if path == home {
        return "~".to_string();
    }

    match path.strip_prefix(&home) {
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

fn cmd_start_wrapper() -> Result<()> {
    let config_path = default_config_path()?;
    let db_path = default_db_path()?;
    let log_path = default_log_path()?;

    cmd_start(&config_path, &db_path, &log_path)
}

fn cmd_stop_wrapper() -> Result<()> {
    cmd_stop()
}

fn cmd_status_wrapper() -> Result<()> {
    let config_path = default_config_path()?;
    let config = Config::load_or_create(&config_path)?;
    let database = open_default_database()?;

    cmd_status(&config, &database)
}

fn cmd_daemon_command(args: &[String]) -> Result<()> {
    match args {
        [flag] if flag == "--help" || flag == "-h" => {
            print!("{DAEMON_HELP_TEXT}");
            Ok(())
        }
        [command] if command == "start" => cmd_start_wrapper(),
        [command] if command == "stop" => cmd_stop_wrapper(),
        [command] if command == "restart" => cmd_daemon_restart_wrapper(),
        [command] if command == "status" => cmd_status_wrapper(),
        _ => anyhow::bail!("用法: tm-watcher daemon <start|stop|restart|status>"),
    }
}

fn cmd_daemon_restart_wrapper() -> Result<()> {
    let config_path = default_config_path()?;
    let db_path = default_db_path()?;
    let log_path = default_log_path()?;

    cmd_restart(&config_path, &db_path, &log_path)
}

fn cmd_doctor_wrapper() -> Result<()> {
    let config_path = default_config_path()?;
    let db_path = default_db_path()?;
    let tm_backend = RealTmBackend::new();
    let report = run_doctor_checks(
        &config_path,
        &db_path,
        &tm_backend,
        LaunchAgentDoctorState::current(),
    );

    print!("{}", report.render());
    if report.has_issues() {
        std::process::exit(1);
    }

    Ok(())
}

fn cmd_daemon_entrypoint() -> Result<()> {
    use tm_watcher::run_periodic_cleanup;
    use tokio::sync::watch;

    let config_path = default_config_path()?;
    let config = Config::load_or_create(&config_path)?;

    let database = open_default_database()?;
    let tm_backend = Box::new(RealTmBackend::new());

    // 检查 Time Machine 是否配置
    check_tm_configured(tm_backend.as_ref())?;

    tracing::info!("守护进程启动中");
    tracing::info!(path = %config_path.display(), "加载配置文件");
    tracing::info!(path = %default_db_path()?.display(), "打开数据库");

    // 设置 shutdown 信号
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    tokio::runtime::Runtime::new()?.block_on(async move {
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .context("无法注册 SIGTERM 处理器")?;

        // 处理 SIGTERM
        tokio::spawn(async move {
            sigterm.recv().await;
            tracing::info!("收到 SIGTERM，正在退出");
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
            let watcher_clone = Watcher::new(
                config.clone(),
                database.clone(),
                Box::new(RealTmBackend::new()),
            )?;
            let shutdown_rx_clone = shutdown_rx.clone();
            Some(tokio::spawn(async move {
                watcher_clone
                    .watch_multiple(&watch_paths, shutdown_rx_clone)
                    .await
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
                run_periodic_cleanup(
                    db,
                    || Box::new(RealTmBackend::new()),
                    interval,
                    shutdown_rx_clone,
                )
                .await;
            }
        });

        // 等待 shutdown
        shutdown_rx.clone().changed().await.ok();

        // 等待任务完成
        if let Some(handle) = watch_handle {
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        }
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), cleanup_handle).await;

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}

/// 默认配置文件路径: ~/.config/tm-watcher/config.toml
fn default_log_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    Ok(home.join(".local/share/tm-watcher/daemon.log"))
}
