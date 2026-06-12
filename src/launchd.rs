// ABOUTME: launchd 集成 - plist 生成、launchctl 调用、进程状态查询

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const LABEL: &str = "com.zzerding.tm-watcher";

/// 生成 launchd plist 内容
pub fn generate_plist(exe_path: &Path, log_path: &Path) -> String {
    let exe_escaped = xml_escape(exe_path.to_string_lossy().as_ref());
    let log_escaped = xml_escape(log_path.to_string_lossy().as_ref());

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>__daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>StandardOutPath</key>
    <string>{}</string>
    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>"#,
        LABEL, exe_escaped, log_escaped, log_escaped
    )
}

/// XML 转义
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// plist 文件路径
pub fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("无法获取用户主目录")?;
    Ok(home
        .join("Library/LaunchAgents")
        .join(format!("{}.plist", LABEL)))
}

/// launchctl bootstrap
pub fn bootstrap(plist: &Path) -> Result<()> {
    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{}", uid);

    let output = Command::new("launchctl")
        .args(["bootstrap", &target, &plist.to_string_lossy()])
        .output()
        .context("无法执行 launchctl bootstrap")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("launchctl bootstrap 失败: {}", stderr);
    }

    Ok(())
}

/// launchctl bootout
pub fn bootout() -> Result<()> {
    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{}/{}", uid, LABEL);

    let output = Command::new("launchctl")
        .args(["bootout", &target])
        .output()
        .context("无法执行 launchctl bootout")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // bootout 不存在时也报错,调用方需要检查
        anyhow::bail!("launchctl bootout 失败: {}", stderr);
    }

    Ok(())
}

/// 检查 LaunchAgent 是否已加载（无论是否有运行中的 PID）
pub fn is_loaded() -> bool {
    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{}/{}", uid, LABEL);

    Command::new("launchctl")
        .args(["print", &target])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// 如果 LaunchAgent 已加载则卸载，返回是否执行了卸载操作
pub fn bootout_if_loaded() -> Result<bool> {
    if !is_loaded() {
        return Ok(false);
    }

    bootout()?;
    Ok(true)
}

/// 查询守护进程状态,返回 PID(运行中)或 None(未运行)
pub fn query_status() -> Option<u32> {
    let uid = unsafe { libc::getuid() };
    let target = format!("gui/{}/{}", uid, LABEL);

    let output = Command::new("launchctl")
        .args(["print", &target])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_launchctl_print(&stdout)
}

/// 解析 launchctl print 输出
fn parse_launchctl_print(output: &str) -> Option<u32> {
    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("pid = ")
            && let Ok(pid) = rest.parse::<u32>()
        {
            return Some(pid);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_plist_contains_label() {
        let plist = generate_plist(
            Path::new("/usr/bin/tm-watcher"),
            Path::new("/var/log/daemon.log"),
        );
        assert!(plist.contains("<string>com.zzerding.tm-watcher</string>"));
    }

    #[test]
    fn test_generate_plist_contains_exe_path() {
        let plist = generate_plist(
            Path::new("/usr/bin/tm-watcher"),
            Path::new("/var/log/daemon.log"),
        );
        assert!(plist.contains("<string>/usr/bin/tm-watcher</string>"));
        assert!(plist.contains("<string>__daemon</string>"));
    }

    #[test]
    fn test_generate_plist_sets_keepalive_successful_exit_false() {
        let plist = generate_plist(
            Path::new("/usr/bin/tm-watcher"),
            Path::new("/var/log/daemon.log"),
        );
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<key>SuccessfulExit</key>"));
        assert!(plist.contains("<false/>"));
    }

    #[test]
    fn test_generate_plist_redirects_stdout_stderr() {
        let plist = generate_plist(
            Path::new("/usr/bin/tm-watcher"),
            Path::new("/var/log/daemon.log"),
        );
        assert!(plist.contains("<key>StandardOutPath</key>"));
        assert!(plist.contains("<key>StandardErrorPath</key>"));
        assert!(plist.contains("<string>/var/log/daemon.log</string>"));
    }

    #[test]
    fn test_generate_plist_escapes_paths() {
        let plist = generate_plist(
            Path::new("/path/with spaces/tm-watcher"),
            Path::new("/log/with'quote.log"),
        );
        assert!(plist.contains("/path/with spaces/tm-watcher"));
        assert!(plist.contains("/log/with&apos;quote.log"));
    }

    #[test]
    fn test_xml_escape_handles_special_chars() {
        assert_eq!(xml_escape("a&b"), "a&amp;b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("\"quoted\""), "&quot;quoted&quot;");
        assert_eq!(xml_escape("'single'"), "&apos;single&apos;");
    }

    #[test]
    fn test_parse_launchctl_print_extracts_pid() {
        let sample = r#"com.zzerding.tm-watcher = {
    active count = 1
    path = /Users/test/Library/LaunchAgents/com.zzerding.tm-watcher.plist
    state = running
    pid = 12345
}"#;
        assert_eq!(parse_launchctl_print(sample), Some(12345));
    }

    #[test]
    fn test_parse_launchctl_print_not_running() {
        let sample = "service not found";
        assert_eq!(parse_launchctl_print(sample), None);
    }

    #[test]
    fn test_parse_launchctl_print_loaded_without_pid() {
        let sample = r#"com.zzerding.tm-watcher = {
    active count = 0
    path = /Users/test/Library/LaunchAgents/com.zzerding.tm-watcher.plist
    state = waiting
}"#;
        assert_eq!(parse_launchctl_print(sample), None);
    }

    #[test]
    fn test_plist_path_returns_launch_agents_path() {
        let path = plist_path().unwrap();
        assert!(path.to_string_lossy().contains("Library/LaunchAgents"));
        assert!(
            path.to_string_lossy()
                .ends_with("com.zzerding.tm-watcher.plist")
        );
    }
}
