# P2 Bug 修复总结

## 修复的问题

### 1. Config 加载时拒绝 interval_hours=0
- **问题**: `run_periodic_cleanup()` 不校验零值，导致 `Duration::from_secs(0)` 死循环
- **修复**: 
  - `Config::load_or_create()` 加载时验证 `interval_hours != 0`
  - `run_periodic_cleanup()` 开头检查并立即返回
- **测试**: 
  - `test_config_rejects_zero_interval_hours`
  - `test_run_periodic_cleanup_rejects_zero_interval`

### 2. cmd_start 启动前预检
- **问题**: spawn 子进程后立即写 PID 并报告成功，子进程可能立即失败
- **修复**:
  - 新增 `precheck_daemon_start()` 验证 TM 配置和数据库可访问
  - `cmd_start()` 在启动子进程前调用预检
- **测试**:
  - `test_precheck_fails_if_tm_not_configured`
  - `test_precheck_fails_if_database_inaccessible`

### 3. PID 复用安全检查
- **问题**: `is_daemon_running()` 仅用 `kill(pid, 0)` 判断进程存活，不验证进程名
- **修复**:
  - 新增 `is_our_process()` 用 `ps -p <pid> -o comm=` 验证进程名包含 "tm-watcher"
  - `is_daemon_running()` 同时检查进程存活 + 进程名匹配
- **测试**:
  - `test_is_our_process_rejects_wrong_process`
  - `test_is_our_process_returns_false_for_nonexistent_pid`
  - 更新 `test_is_daemon_running_alive_pid` 以匹配新行为

## 验证结果

- ✅ 48 个测试通过（24 单元 + 3 配置 + 21 集成）
- ✅ cargo clippy --all-targets -- -D warnings 通过
- ✅ 所有原有测试保持通过

## 新增测试

- config.rs: 1 个新测试
- daemon.rs: 5 个新测试，1 个更新
