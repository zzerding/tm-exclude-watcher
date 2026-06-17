// ABOUTME: 守护进程模块入口 - 按职责导出调度、预检与命令。

mod lifecycle;
mod preflight;
mod scheduling;
mod status;

pub use lifecycle::{cmd_restart, cmd_start, cmd_stop};
pub use preflight::{check_tm_configured, precheck_daemon_start};
pub use scheduling::run_periodic_cleanup;
pub use status::cmd_status;
