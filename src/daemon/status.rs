// ABOUTME: 守护进程状态命令 - 展示 launchd 状态、监控路径与清理摘要。

use anyhow::Result;

use crate::{Config, Database, format_saved_space_summary, launchd};

/// cmd_status: 显示守护进程状态
pub fn cmd_status(config: &Config, database: &Database) -> Result<()> {
    warn_if_launch_agent_uses_stale_binary();

    // 检查运行状态
    let running = if let Some(pid) = launchd::query_status() {
        println!("状态: 运行中 (PID: {})", pid);
        true
    } else {
        println!("状态: 未运行");
        false
    };

    let records = database.get_exclusions()?;
    println!("{}", format_saved_space_summary(&records));

    if running {
        // 显示监控路径
        println!("\n监控路径:");
        for path_str in &config.watch_paths {
            let expanded = expand_tilde(path_str);
            println!("  - {}", expanded);
        }

        // 显示已排除目录数量
        println!("\n已排除目录: {} 个", records.len());

        // 显示最后清理时间
        match database.last_cleanup_time()? {
            Some(time) => println!("上次清理时间: {}", time),
            None => println!("上次清理时间: 从未"),
        }
    }

    Ok(())
}

fn warn_if_launch_agent_uses_stale_binary() {
    let Some(configured_path) = launchd::configured_program_path() else {
        return;
    };
    let Ok(current_path) = std::env::current_exe() else {
        return;
    };

    if configured_path != current_path {
        println!("警告: LaunchAgent 仍指向旧的 tm-watcher 二进制路径");
        println!("  旧路径: {}", configured_path.display());
        println!("  当前路径: {}", current_path.display());
        println!("  修复: tm-watcher daemon stop && tm-watcher daemon start");
        println!();
    }
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
