// ABOUTME: 库入口 - 导出公共接口

mod cleaner;
mod config;
mod daemon;
mod database;
mod list;
mod rules;
mod scanner;
mod tm_backend;
mod watcher;

pub use cleaner::{CleanResult, Cleaner};
pub use config::Config;
pub use daemon::{cmd_start, cmd_status, cmd_stop, check_tm_configured, run_periodic_cleanup};
pub use database::{Database, ExclusionRecord};
pub use list::format_exclusion_list;
pub use rules::RuleMatcher;
pub use scanner::{ScanResult, Scanner};
pub use tm_backend::{FakeTmBackend, RealTmBackend, TmBackend, TmBackendError};
pub use watcher::Watcher;
