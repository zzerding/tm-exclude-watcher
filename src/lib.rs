// ABOUTME: 库入口 - 导出公共接口

mod cleaner;
mod config;
mod database;
mod list;
mod rules;
mod scanner;
mod tm_backend;

pub use cleaner::{CleanResult, Cleaner};
pub use config::Config;
pub use database::{Database, ExclusionRecord};
pub use list::format_exclusion_list;
pub use rules::RuleMatcher;
pub use scanner::{ScanResult, Scanner};
pub use tm_backend::{FakeTmBackend, RealTmBackend, TmBackend, TmBackendError};
