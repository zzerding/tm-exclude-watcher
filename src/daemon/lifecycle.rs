// ABOUTME: 守护进程生命周期命令 - 启动、停止和重启 launchd job。

use anyhow::{Context, Result};
use std::path::Path;

use crate::launchd;

use super::precheck_daemon_start;

// 追加到 daemon.rs 的 CLI 命令实现
// 更正：daemon 生命周期命令已拆分到 daemon/lifecycle.rs。

/// cmd_start: 启动守护进程(launchd 模式)
pub fn cmd_start(config_path: &Path, db_path: &Path, log_path: &Path) -> Result<()> {
    // 预检：TM 配置、配置文件验证和数据库可访问性
    precheck_daemon_start(config_path, db_path)?;

    cmd_start_prechecked(log_path)
}

fn cmd_start_prechecked(log_path: &Path) -> Result<()> {
    // 静默清理旧版本 PID 文件(Issue #4 遗留)
    if let Some(home) = dirs::home_dir() {
        let old_pid = home.join(".local/var/run/tm-watcher.pid");
        if old_pid.exists() {
            let _ = std::fs::remove_file(old_pid);
        }
    }

    // 检查是否已在运行
    if let Some(pid) = launchd::query_status() {
        anyhow::bail!("守护进程已在运行 (PID: {})", pid);
    }

    // 生成 plist
    let exe_path = std::env::current_exe().context("无法获取当前可执行文件路径")?;
    let plist_content = launchd::generate_plist(&exe_path, log_path);
    let plist_path = launchd::plist_path()?;

    // 创建必要的目录
    if let Some(parent) = plist_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建 plist 目录: {}", parent.display()))?;
    }
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 写入 plist
    std::fs::write(&plist_path, plist_content)
        .with_context(|| format!("无法写入 plist: {}", plist_path.display()))?;

    // 启动 launchd job
    launchd::bootstrap(&plist_path)?;

    println!("✓ 守护进程已启动");
    println!("  日志: {}", log_path.display());
    println!("  plist: {}", plist_path.display());
    println!("  登录自启: 启用");
    println!("  崩溃重启: 启用");

    Ok(())
}

/// cmd_restart: 预检配置后受控重启守护进程(launchd 模式)
pub fn cmd_restart(config_path: &Path, db_path: &Path, log_path: &Path) -> Result<()> {
    if launchd::query_status().is_none() {
        anyhow::bail!("守护进程未运行，请先使用 'tm-watcher daemon start' 启动");
    }

    precheck_daemon_start(config_path, db_path).context("配置预检失败，请修复配置文件后重试")?;

    cmd_stop().context("重启失败：停止守护进程失败")?;
    cmd_start_prechecked(log_path)
        .context("重启失败：启动守护进程失败，请手动运行 'tm-watcher daemon start'")?;

    println!("✓ 守护进程已重启，配置已生效");
    Ok(())
}

/// cmd_stop: 停止守护进程(launchd 模式)
pub fn cmd_stop() -> Result<()> {
    let was_loaded = launchd::bootout_if_loaded().context("停止守护进程失败")?;

    let plist_path = launchd::plist_path()?;
    if plist_path.exists() {
        std::fs::remove_file(&plist_path)
            .with_context(|| format!("无法删除 plist: {}", plist_path.display()))?;
    }

    if was_loaded {
        println!("✓ 守护进程已停止");
    } else {
        println!("守护进程未运行");
    }

    Ok(())
}
