// ABOUTME: tm-watcher 核心库，提供 Time Machine 排除目录的扫描和管理功能

mod database;
mod rules;
mod scanner;
pub mod tmutil;
mod cleaner;
mod watcher;

pub use database::{Database, ExclusionRecord};
pub use rules::RuleMatcher;
pub use scanner::{Scanner, ScanResult};
pub use tmutil::{TmUtilTrait, RealTmUtil};
pub use cleaner::{Cleaner, CleanStats};
pub use watcher::{Watcher, WatchError};
