# 守护进程 P2 Bug 修复计划

## 问题总结

1. **P2-1: cmd_start 启动成功判断过早**
   - 现状：spawn 子进程后立即写 PID 并报告成功
   - 风险：子进程可能因 TM 未配置/DB 打不开立即退出，但父进程已报告成功并留下过期 PID
   
2. **P2-2: interval_hours=0 导致清理任务死循环**
   - 现状：run_periodic_cleanup() 不校验零值
   - 风险：Duration::from_secs(0) 导致 sleep 立即返回，循环疯狂调用 Cleaner::clean()
   
3. **P2-3: PID 复用时误杀无关进程**
   - 现状：cmd_stop 仅用 kill(pid, 0) 判断进程存活
   - 风险：PID 被复用时会向无关进程发送 SIGTERM

## 修复方案（TDD 优先）

### Bug #1: cmd_start 启动验证

**测试目标：**
- 在父进程中预检 TM 配置和数据库可用性
- 启动失败时不应留下 PID 文件

**实现方案：**
```rust
// cmd_start 添加预检
pub fn cmd_start(
    config_path: &Path,
    db_path: &Path,
    pid_path: &Path,
    log_path: &Path,
) -> Result<()> {
    // 1. 预检 TM 配置
    let backend = crate::tm_backend::TmBackendImpl::new();
    check_tm_configured(&backend)?;
    
    // 2. 预检数据库可访问
    Database::new(db_path)?;
    
    // 3. 启动子进程（原有逻辑）
    // ...
}
```

**测试用例：**
- `test_cmd_start_fails_if_tm_not_configured()` - TM 未配置时拒绝启动
- `test_cmd_start_fails_if_db_cannot_open()` - 数据库不可访问时拒绝启动
- `test_cmd_start_no_pid_file_on_precheck_failure()` - 预检失败时不留 PID 文件

### Bug #2: interval_hours=0 验证

**测试目标：**
- run_periodic_cleanup() 拒绝零值间隔
- Config 加载时验证 interval_hours >= 1

**实现方案：**
```rust
// run_periodic_cleanup 开头添加
pub async fn run_periodic_cleanup(...) {
    if interval_hours == 0 {
        eprintln!("错误: 清理间隔不能为 0");
        return;
    }
    // 原有逻辑...
}

// Config::load_from_file 添加验证
impl Config {
    pub fn load_from_file(path: &Path) -> Result<Self> {
        let config = // ... 解析 TOML
        if config.interval_hours == 0 {
            anyhow::bail!("配置错误: interval_hours 不能为 0");
        }
        Ok(config)
    }
}
```

**测试用例：**
- `test_run_periodic_cleanup_rejects_zero_interval()` - 传入 0 时立即返回
- `test_config_rejects_zero_interval_hours()` - 配置文件 interval_hours=0 时加载失败

### Bug #3: PID 复用安全检查

**测试目标：**
- is_daemon_running() 验证进程名是否为 tm-watcher
- cmd_stop 在 PID 不匹配时删除过期文件而不发信号

**实现方案：**
```rust
// 替换 is_daemon_running
pub fn is_daemon_running(pid: u32) -> bool {
    if unsafe { libc::kill(pid as i32, 0) } != 0 {
        return false;
    }
    is_our_process(pid)
}

fn is_our_process(pid: u32) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("ps")
            .args(&["-p", &pid.to_string(), "-o", "comm="])
            .output();
        
        if let Ok(output) = output {
            if let Ok(comm) = String::from_utf8(output.stdout) {
                return comm.trim().contains("tm-watcher");
            }
        }
    }
    false
}
```

**测试用例：**
- `test_is_daemon_running_rejects_wrong_process()` - 模拟 PID 复用场景
- `test_cmd_stop_removes_stale_pid_without_killing()` - 过期 PID 不发信号

## 执行顺序

1. Bug #2（最简单）→ 写测试 → 实现修复
2. Bug #1（中等）→ 写测试 → 实现修复
3. Bug #3（需要 ps 命令）→ 写测试 → 实现修复

## 验证清单

- [ ] cargo test 全部通过
- [ ] cargo clippy --all-targets -- -D warnings 通过
- [ ] 手动测试三个场景：
  - TM 未配置时 start 失败
  - 配置 interval_hours=0 时启动失败
  - 手动创建过期 PID 文件，stop 不发信号
