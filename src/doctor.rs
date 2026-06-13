// ABOUTME: Doctor 健康检查报告 - 汇总系统配置、数据库和 LaunchAgent 状态。

use anyhow::Result;
use std::path::Path;

use crate::{Config, Database, TmBackend, launchd};

pub struct DoctorReport {
    checks: Vec<DoctorCheck>,
}

impl DoctorReport {
    pub fn has_issues(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status != DoctorStatus::Pass)
    }

    pub fn render(&self) -> String {
        let mut output = String::from("tm-watcher 健康检查\n");
        for check in &self.checks {
            output.push_str(&check.render());
            output.push('\n');
        }
        output
    }
}

#[derive(Clone, Copy)]
pub struct LaunchAgentDoctorState {
    pub pid: Option<u32>,
    pub is_loaded: bool,
    pub plist_exists: bool,
}

impl LaunchAgentDoctorState {
    pub fn current() -> Self {
        let plist_exists = launchd::plist_path()
            .map(|path| path.exists())
            .unwrap_or(false);

        Self {
            pid: launchd::query_status(),
            is_loaded: launchd::is_loaded(),
            plist_exists,
        }
    }
}

pub fn run_doctor_checks(
    config_path: &Path,
    db_path: &Path,
    tm_backend: &dyn TmBackend,
    launch_agent: LaunchAgentDoctorState,
) -> DoctorReport {
    DoctorReport {
        checks: vec![
            check_time_machine(tm_backend),
            check_config(config_path),
            check_database(db_path),
            check_daemon(launch_agent),
            check_launch_agent_plist(launch_agent),
        ],
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DoctorStatus {
    Pass,
    Fail,
    Warn,
}

struct DoctorCheck {
    status: DoctorStatus,
    message: String,
}

impl DoctorCheck {
    fn pass(message: impl Into<String>) -> Self {
        Self {
            status: DoctorStatus::Pass,
            message: message.into(),
        }
    }

    fn fail(message: impl Into<String>) -> Self {
        Self {
            status: DoctorStatus::Fail,
            message: message.into(),
        }
    }

    fn warn(message: impl Into<String>) -> Self {
        Self {
            status: DoctorStatus::Warn,
            message: message.into(),
        }
    }

    fn render(&self) -> String {
        format!("{} {}", self.status.symbol(), self.message)
    }
}

impl DoctorStatus {
    fn symbol(self) -> &'static str {
        match self {
            Self::Pass => "✅",
            Self::Fail => "❌",
            Self::Warn => "⚠️",
        }
    }
}

fn check_time_machine(tm_backend: &dyn TmBackend) -> DoctorCheck {
    match tm_backend.check_configured() {
        Ok(true) => DoctorCheck::pass("Time Machine 已配置"),
        Ok(false) => {
            DoctorCheck::fail("Time Machine 未配置（请先在系统设置中配置 Time Machine 备份目的地）")
        }
        Err(err) => DoctorCheck::fail(format!(
            "Time Machine 检查失败: {}（确认 tmutil 可用后重试）",
            err
        )),
    }
}

fn check_config(config_path: &Path) -> DoctorCheck {
    match Config::load_or_create(config_path) {
        Ok(_) => DoctorCheck::pass(format!("配置文件有效 ({})", config_path.display())),
        Err(err) => DoctorCheck::fail(format!(
            "配置文件无效 ({}): {}（修复 config.toml 后重试）",
            config_path.display(),
            err
        )),
    }
}

fn check_database(db_path: &Path) -> DoctorCheck {
    let result = open_database_for_doctor(db_path)
        .and_then(|database| database.get_exclusions().map(|records| records.len()));

    match result {
        Ok(record_count) => DoctorCheck::pass(format!("数据库可访问 ({} 条记录)", record_count)),
        Err(err) => DoctorCheck::fail(format!(
            "数据库不可访问 ({}): {}（检查数据目录权限后重试）",
            db_path.display(),
            err
        )),
    }
}

fn open_database_for_doctor(db_path: &Path) -> Result<Database> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    Database::new(db_path)
}

fn check_daemon(launch_agent: LaunchAgentDoctorState) -> DoctorCheck {
    match launch_agent.pid {
        Some(pid) => DoctorCheck::pass(format!("Daemon 正在运行 (PID: {})", pid)),
        None => DoctorCheck::fail("Daemon 未运行（使用 `tm-watcher daemon start` 启动）"),
    }
}

fn check_launch_agent_plist(launch_agent: LaunchAgentDoctorState) -> DoctorCheck {
    match (launch_agent.plist_exists, launch_agent.is_loaded) {
        (true, true) => DoctorCheck::pass("LaunchAgent plist 已加载"),
        (true, false) => {
            DoctorCheck::warn("LaunchAgent plist 未加载（使用 `tm-watcher daemon start` 重新加载）")
        }
        (false, _) => {
            DoctorCheck::warn("LaunchAgent plist 不存在（使用 `tm-watcher daemon start` 创建）")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FakeTmBackend;
    use tempfile::TempDir;

    #[test]
    fn test_doctor_checks_continue_after_failures() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let db_path = temp_dir.path().join("exclusions.db");
        std::fs::write(&config_path, "interval_hours = 0\n").unwrap();

        let report = run_doctor_checks(
            &config_path,
            &db_path,
            &FakeTmBackend::new_unconfigured(),
            LaunchAgentDoctorState {
                pid: None,
                is_loaded: false,
                plist_exists: false,
            },
        );

        assert!(report.has_issues());
        assert_eq!(report.checks.len(), 5);

        let output = report.render();
        assert!(output.contains("Time Machine 未配置"));
        assert!(output.contains("配置文件无效"));
        assert!(output.contains("数据库可访问 (0 条记录)"));
        assert!(output.contains("Daemon 未运行"));
        assert!(output.contains("LaunchAgent plist 不存在"));
    }

    #[test]
    fn test_doctor_report_is_healthy_when_all_checks_pass() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let db_path = temp_dir.path().join("exclusions.db");

        let report = run_doctor_checks(
            &config_path,
            &db_path,
            &FakeTmBackend::new(),
            LaunchAgentDoctorState {
                pid: Some(123),
                is_loaded: true,
                plist_exists: true,
            },
        );

        assert!(!report.has_issues());

        let output = report.render();
        assert!(output.contains("✅ Time Machine 已配置"));
        assert!(output.contains("✅ 配置文件有效"));
        assert!(output.contains("✅ 数据库可访问 (0 条记录)"));
        assert!(output.contains("✅ Daemon 正在运行 (PID: 123)"));
        assert!(output.contains("✅ LaunchAgent plist 已加载"));
    }
}
