// ABOUTME: 库入口 - 导出公共接口

mod config;
mod database;
mod rules;
mod scanner;
mod tm_backend;

pub use config::Config;
pub use database::Database;
pub use rules::RuleMatcher;
pub use scanner::{ScanResult, Scanner};
pub use tm_backend::{FakeTmBackend, RealTmBackend, TmBackend};
