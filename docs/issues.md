<!-- ABOUTME: 保存项目早期拆分 GitHub issues 的草稿与验收标准。 -->

# Issue 状态

## Issue #4: 守护进程模式（launchd lifecycle）- 已重构

**Labels**: enhancement

**状态**：代码已实现。原始 fork/PID 文件方案已重构为 macOS launchd 托管。

### 当前实现

- `src/launchd.rs`：生成 `~/Library/LaunchAgents/com.zzerding.tm-watcher.plist`，执行 `launchctl bootstrap` / `launchctl bootout`，并解析 `launchctl print`。
- `src/daemon.rs`：保留 Time Machine、配置、数据库预检；清理旧 PID 文件残留；实现 LaunchAgent 的启动、停止和状态查询。
- `src/main.rs`：接入 `daemon start`、`daemon stop`、`daemon status` 和 `__daemon`。
- `__daemon`：运行多路径监控和定期清理，并在收到 SIGTERM 后退出。
- README 已记录登录自启、崩溃重启、plist 路径行为和 daemon log 路径。

### 相比原 issue 的范围变化

- 已移除：手写 fork 后台化、PID 文件管理、PID 复用检查、手动 SIGTERM 进程管理。
- 已新增：LaunchAgent plist、`RunAtLoad`、`KeepAlive.SuccessfulExit = false`、stdout/stderr 重定向到 `~/.local/share/tm-watcher/daemon.log`。
- 已保留：Time Machine 预检、数据库访问预检、watcher 集成、定期清理、`daemon start` / `daemon stop` / `daemon status` CLI。

### 验收状态

- [x] `tm-watcher daemon start` 预检 Time Machine、配置和数据库访问。
- [x] `tm-watcher daemon start` 写入 LaunchAgent plist，并通过 `launchctl bootstrap` 启动。
- [x] 通过 `launchctl print` 的 PID 状态检测已运行的 daemon。
- [x] LaunchAgent 运行 `tm-watcher __daemon`。
- [x] `__daemon` 运行配置的监控路径和定期清理。
- [x] LaunchAgent 将 stdout/stderr 重定向到 `~/.local/share/tm-watcher/daemon.log`。
- [x] `tm-watcher daemon status` 输出运行 PID、监控路径、排除数量和上次清理时间。
- [x] `tm-watcher daemon stop` 通过 `launchctl bootout` 卸载 LaunchAgent，并删除 plist。
- [x] `tm-watcher daemon start` 时静默清理旧的 `~/.local/var/run/tm-watcher.pid` 残留。
- [x] `__daemon` 内部仍处理 SIGTERM 优雅退出。

### 发布前跟进

- 真实机器上的 `daemon start` / `daemon stop` / `daemon status` E2E 验证归入 Issue #6。

---

## Issue #5: 日志和可观测性（Logging and observability）

**Labels**: enhancement

**Body**:
```
## Parent

无（顶层切片）

## What to build

集成 tracing 日志系统，为所有模块添加结构化日志，区分 CLI 模式（输出到 stderr）和守护进程模式（输出到文件）的日志行为。

这个切片包含：
- tracing + tracing-subscriber 集成
- CLI 模式日志配置（stderr, INFO 级别）
- 守护进程模式日志配置（文件, INFO 级别）
- 关键操作的日志埋点（排除/清理/监控启动/错误）

## Acceptance criteria

- [ ] 初始化 tracing-subscriber，CLI 模式输出到 stderr，守护进程模式输出到 ~/.local/share/tm-watcher/daemon.log
- [ ] 所有模块添加适当级别的日志：
  - exclusion.rs: `info!("Excluded {}")`, `warn!("Permission denied: {}")`
  - scanner.rs: `info!("Scan completed: {} excluded")`
  - cleaner.rs: `info!("Cleanup completed: {} cleaned")`
  - watcher.rs: `info!("Watching paths: {:?}")`, `debug!("Detected create: {}")`
  - daemon.rs: `info!("Daemon started")`, `info!("Received SIGTERM, shutting down")`
- [ ] CLI 命令运行时日志输出到 stderr 可见
- [ ] 守护进程日志持久化到文件，使用 tracing-appender 的 rolling file
- [ ] 错误场景有清晰的日志记录（权限不足、TM 未配置、tmutil 调用失败）
- [ ] 所有 panic! 改为返回 Result，顶层 main() 捕获错误并记录日志后优雅退出

## Blocked by

无 — 可与其他切片并行
```

### 当前实现

- 已新增 `tracing` / `tracing-subscriber` / `tracing-appender`。
- CLI 模式初始化 INFO 级别 stderr 日志；守护进程模式写入 `~/.local/share/tm-watcher/daemon.log`。
- `scanner` / `cleaner` / `watcher` / `daemon` 已添加排除、扫描完成、清理完成、监控启动、错误与 shutdown 日志。
- 顶层 `main()` 捕获错误并记录日志后退出。
- 生产路径中已移除 SIGTERM 注册和数据库非 UTF-8 路径处理的 panic 风险。

### 验收状态

- [x] 初始化 tracing-subscriber，CLI 模式输出到 stderr，守护进程模式输出到 `~/.local/share/tm-watcher/daemon.log`。
- [x] 关键模块添加适当级别日志；当前仓库无 `exclusion.rs`，排除日志落在 `scanner.rs`、`watcher.rs` 和 `cleaner.rs`。
- [x] CLI 命令运行时日志输出到 stderr 可见。
- [x] 守护进程日志持久化到文件，使用 `tracing-appender` writer。
- [x] 错误场景有清晰日志记录（权限/访问失败、Time Machine 未配置、tmutil 调用失败）。
- [x] 生产路径 panic 风险改为返回 `Result`，顶层捕获错误并记录日志后退出。

---

## Issue #6: 端到端测试与发布准备（E2E testing and release polish）

**Labels**: enhancement

**Body**:
```
## Parent

无（顶层切片）

## What to build

编写完整的端到端测试脚本，验证所有核心功能的集成效果，完成文档和发布前检查。这是 HITL（需要人工审查）切片。

这个切片包含：
- 端到端测试脚本（创建测试目录结构 → 启动守护进程 → 验证排除 → 验证清理）
- 错误场景测试（TM 未配置/重复启动/权限问题/scan 不存在路径）
- 性能验证（CPU/内存占用/大量目录扫描）
- README 文档（安装/使用/配置说明）
- .gitignore 文件
- 最终检查清单验证

## Acceptance criteria

- [ ] 端到端测试脚本通过所有验证点：
  - 创建测试目录结构（/tmp/tm_test/project-a/node_modules、project-b/target、project-c/.venv）
  - 启动守护进程，等待 10 秒
  - 使用 `tmutil isexcluded` 验证这些目录已被排除
  - 删除 project-a/node_modules，等待 5 秒
  - 验证排除记录已从数据库清理
  - 停止守护进程，清理测试数据
- [ ] 错误场景测试覆盖：
  - TM 未配置时启动 → 报错退出
  - 重复启动守护进程 → 提示已在运行
  - scan 不存在路径 → 报错
- [ ] 性能验证：
  - 扫描包含 1000+ 目录的路径
  - 使用 top 监控资源占用
  - 确认 CPU < 1%，内存 < 10 MB
- [ ] README 完整包含：项目简介、安装方法（cargo install --path .）、使用方法（各命令示例）、配置说明、注意事项（需要 macOS、Time Machine 配置）
- [ ] .gitignore 包含：target/、Cargo.lock、*.db、*.log
- [ ] 最终检查清单全部通过：
  - 所有 CLI 命令可用
  - 守护进程正常启动/停止
  - 自动排除生效（延迟 5 秒后）
  - 实时清理生效
  - 定期清理生效（可缩短 interval 测试）
  - 数据库记录正确
  - 日志文件生成
  - 配置文件自动生成
  - 错误处理正确
  - 符号链接不跟随

## Blocked by

- #5 — 日志和可观测性
- launchd `daemon start` / `daemon stop` / `daemon status` 真实机器 E2E 验证
```
