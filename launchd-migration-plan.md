# 守护进程迁移到 launchd 计划

## 背景与动机

当前守护进程(Issue #4 实现)采用手写进程管理:self-respawn `__daemon` 子命令、PID 文件、
进程名嗅探防 PID 复用、SIGTERM + 轮询停止。这些都是 macOS 原生 launchd 已经免费提供的能力。
本项目本来就锁死 macOS(依赖 tmutil、FSEvents),不存在跨平台保留通用 daemon 的理由。

### 当前实现 vs launchd 对照

| 手写代码 | launchd 原生等价物 |
|---|---|
| PID 文件读写删 (`daemon.rs` write/read/delete_pid_file) | 不需要,launchd 自己追踪进程 |
| PID 复用防误杀 (`is_our_process` 用 `ps -p` 嗅探进程名) | 不需要,整个 hack 消失 |
| self-respawn + 日志重定向 (`cmd_start` spawn `__daemon`) | `StandardOutPath` / `StandardErrorPath` 两行 plist |
| `cmd_stop` 发 SIGTERM + 轮询 5 秒 | `launchctl bootout` |
| 单实例检查 (PID 文件 + 探活) | Label 唯一性,launchd 保证 |
| ❌ 没有:崩溃自动重启 | `KeepAlive = { SuccessfulExit = false }` |
| ❌ 没有:登录自启 | `RunAtLoad = true` |

净效果:删掉约 200 行核心逻辑 + 对应测试,换来崩溃重启和登录自启两个新能力。

## 核心思路

**进程生命周期全部交给 launchd,daemon 本体退化为前台命令。**

```
tm-watcher start  → 预检 → 生成 plist → launchctl bootstrap gui/$UID ...
tm-watcher stop   → launchctl bootout gui/$UID/com.zzerding.tm-watcher
tm-watcher status → launchctl print 取运行状态 + 现有数据库统计
__daemon          → 保持前台运行(watcher + 定期清理),由 launchd 托管
```

## plist 设计

路径: `~/Library/LaunchAgents/com.zzerding.tm-watcher.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.zzerding.tm-watcher</string>
    <key>ProgramArguments</key>
    <array>
        <string><!-- exe 绝对路径,来自 current_exe() --></string>
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
    <string><!-- ~/.local/share/tm-watcher/daemon.log --></string>
    <key>StandardErrorPath</key>
    <string><!-- 同上 --></string>
</dict>
</plist>
```

关键决策:
- `KeepAlive.SuccessfulExit = false`:异常退出才拉起,正常退出(如 SIGTERM 优雅停止)不拉起,
  避免 crashloop。
- 不用 `StartCalendarInterval` 拆第二个清理 job:进程内 tokio 定时器已工作正常,
  拆 job 属于过度设计(Karpathy 原则)。

## 变更清单

### 删除(daemon.rs)

- `write_pid_file` / `read_pid_file` / `delete_pid_file` + 测试
- `is_daemon_running` / `is_our_process` + 测试(整个 PID 复用防御消失)
- `cmd_start` 中 self-respawn、日志文件打开/重定向、PID 写入
- `cmd_stop` 中 SIGTERM 发送 + 5 秒轮询
- `main.rs` 中 `default_pid_path()`、`cmd_daemon_wrapper` 末尾的 PID 文件清理

### 新增(src/launchd.rs)

- `generate_plist(exe_path: &Path, log_path: &Path) -> String` — 纯函数,单元可测
- `plist_path() -> Result<PathBuf>` — `~/Library/LaunchAgents/com.zzerding.tm-watcher.plist`
- `launchctl` 调用薄封装:
  - `bootstrap(plist_path)` → `launchctl bootstrap gui/$UID <plist>`
  - `bootout(label)` → `launchctl bootout gui/$UID/<label>`
  - `query_status(label) -> Option<u32>` → 解析 `launchctl print gui/$UID/<label>` 取 PID

### 改造

| 函数 | 新行为 |
|---|---|
| `cmd_start` | `precheck_daemon_start`(保留)→ 写 plist → `bootstrap` → 报告成功 |
| `cmd_stop` | `bootout`,不存在时报"未运行" |
| `cmd_status` | `query_status` 取 PID/运行态,数据库统计部分不变 |
| `__daemon`(`cmd_daemon_wrapper`) | 基本不动:SIGTERM 优雅退出保留(bootout 时 launchd 发的就是 SIGTERM),仅删 PID 文件清理 |

### 保留(不动)

- `run_periodic_cleanup` + 零间隔校验(P2-2 修复保留)
- `check_tm_configured` / `precheck_daemon_start`(P2-1 修复保留)
- `Config.interval_hours` 校验
- watcher 多路径监控 `watch_multiple`
- 日志路径 `~/.local/share/tm-watcher/daemon.log`(改由 launchd 重定向)

注:P2-3(PID 复用安全)在本方案下整块删除——不是回退,而是问题本身不复存在。

## TDD 切片

### Slice 1: plist 生成纯函数
**测试:**
- `test_generate_plist_contains_label_and_exe_path`
- `test_generate_plist_sets_keepalive_successful_exit_false`
- `test_generate_plist_redirects_stdout_stderr_to_log`
- `test_generate_plist_escapes_paths`(路径含空格/特殊字符)

**文件:** src/launchd.rs(新建), src/lib.rs

### Slice 2: launchctl 薄封装 + status 解析
**测试:**
- `test_parse_launchctl_print_extracts_pid`(对固定样例文本解析)
- `test_parse_launchctl_print_not_running` → None
- launchctl 真实调用不做单元测试,集成阶段手工验证

**文件:** src/launchd.rs

### Slice 3: cmd_start 改造
**测试:**
- `test_cmd_start_writes_plist_to_launch_agents`(注入临时目录)
- `test_cmd_start_fails_if_precheck_fails`(沿用 FakeTmBackend 思路)
- **手工验证:** `cargo run -- start` → `launchctl print gui/$UID/com.zzerding.tm-watcher` 有输出

**文件:** src/daemon.rs, src/main.rs

### Slice 4: cmd_stop / cmd_status 改造
**测试:**
- `test_cmd_stop_reports_not_running_when_no_job`
- `test_cmd_status_shows_db_stats`(运行态部分注入 fake 查询结果)
- **手工验证:** start → stop → `launchctl print` 报 not found

**文件:** src/daemon.rs

### Slice 5: 清理 + 文档
- 删除 PID 相关死代码、`default_pid_path`
- README 更新 start/stop/status 说明,注明登录自启/崩溃重启行为
- 注明开发模式 plist 指向 `target/debug` 的已知行为

## 风险与对策

1. **exe 路径**:plist 必须用绝对路径。沿用 `current_exe()`;开发模式下指向
   `target/debug/tm-watcher`,重新编译后二进制变化无需重写 plist(路径不变),
   但 `cargo clean` 后需重新 `start`。README 注明。
2. **测试 launchctl**:不强行 mock。plist 生成是纯函数直接测;`launchctl print`
   输出解析对固定样例文本测;真实 bootstrap/bootout 靠手工验证 + 后续 E2E(Issue #6)。
3. **crashloop**:TM 未配置时 daemon 立即失败 → start 前预检挡掉(P2-1 修复保留),
   且 `SuccessfulExit=false` 只在异常退出时拉起,launchd 自带退避(throttle)兜底。
4. **升级路径**:老版本用户可能有残留 PID 文件,首次 `start` 时检测到旧 PID 文件
   可提示或静默忽略(直接忽略即可,旧 daemon 不会被 launchd 管理,文档提示先用旧版 stop)。

## 分支与合并策略

- 当前 daemon 实现(含 P2 修复)在 `fix/daemon-p2-bugs`,尚未合 main
- 从 `fix/daemon-p2-bugs` 切 `refactor/launchd-migration` 实施
- PR #14(`feat/issue-4-daemon`)的 base 分支已合并,其内容已被 `fix/daemon-p2-bugs`
  包含,处理方式待定(关闭或重定 base)
