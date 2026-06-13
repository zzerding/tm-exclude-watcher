# ADR-0002: 使用 launchd 管理守护进程生命周期

**状态：** 已接受，已按实现修订
**日期：** 2026-06-10
**修订日期：** 2026-06-12
**决策者：** Doctor Biz

## 上下文

macOS 后台服务有多种实现方式。最初的计划是先用手写后台进程和 PID 文件快速验证核心功能，再在后续版本迁移到 LaunchAgent。

Issue #4 实现过程中，守护进程生命周期已经重构为 `launchd` 托管。项目只面向 macOS，核心依赖 `tmutil` 和 FSEvents，不需要保留跨平台 daemon 管理抽象。继续维护手写 PID 文件、fork 后台化、PID 复用防御和手动 SIGTERM 轮询，复杂度高且价值低。

**用户期望：**
- 开机自动启动
- 崩溃自动重启
- 集成系统服务管理

**开发约束：**
- 生命周期管理应尽量交给 macOS 原生机制
- `daemon start` / `daemon stop` / `daemon status` 仍需保持简单的 CLI 入口
- 守护进程主体应以前台进程运行，便于 `launchd` 管理和重启

## 决策

**使用 `launchd` / LaunchAgent 作为守护进程生命周期的唯一实现。**

当前行为：

- `tm-watcher daemon start`：
  - 预检 Time Machine、配置文件和数据库可访问性
  - 生成 `~/Library/LaunchAgents/com.zzerding.tm-watcher.plist`
  - 使用当前可执行文件路径作为 `ProgramArguments[0]`
  - 通过 `launchctl bootstrap gui/$UID <plist>` 启动服务
  - 静默清理旧版本遗留的 `~/.local/var/run/tm-watcher.pid`
- `tm-watcher daemon stop`：
  - 通过 `launchctl bootout gui/$UID/com.zzerding.tm-watcher` 停止服务
  - 删除生成的 plist
- `tm-watcher daemon status`：
  - 通过 `launchctl print gui/$UID/com.zzerding.tm-watcher` 查询运行状态和 PID
  - 展示监控路径、排除记录数量和上次清理时间
- `tm-watcher daemon restart`：
  - 确认 daemon 正在运行，预检 Time Machine、配置文件和数据库，再停止并重新启动服务
- `tm-watcher __daemon`：
  - 以前台进程运行多路径 watcher 和定期清理
  - 收到 SIGTERM 后优雅退出

plist 关键设置：

- `RunAtLoad = true`：用户登录时自动启动
- `KeepAlive.SuccessfulExit = false`：异常退出后自动重启，正常停止不拉起
- `StandardOutPath` / `StandardErrorPath`：重定向到 `~/.local/share/tm-watcher/daemon.log`

## 备选方案

### 方案 A：手写后台进程 + PID 文件（已替代）
**拒绝理由：**
- 用户体验差：无法登录自启
- 无原生崩溃重启机制
- PID 文件会留下过期状态
- PID 复用防误杀需要额外进程名探测，属于脆弱实现
- `launchd` 已经原生提供单实例、生命周期、日志重定向和重启管理

### 方案 B：`launchd` 拆分多个 job（未采用）
**拒绝理由：**
- watcher 和定期清理共享配置与数据库，拆成多个 job 会增加协调成本
- 进程内 tokio 定时器已经足够简单
- 当前需求不需要独立调度清理 job

## 后果

### 正面
- **系统原生：** 生命周期交给 macOS `launchd`，符合平台习惯
- **功能更完整：** 登录自启和异常重启直接可用
- **代码更简单：** 删除手写 PID 文件、PID 复用检查、fork 后台化和停止轮询逻辑
- **状态更可靠：** 运行状态来自 `launchctl print`，不是本地 PID 文件猜测
- **日志路径稳定：** stdout/stderr 统一由 plist 重定向到 daemon log

### 负面
- **更依赖 macOS 行为：** 本地验证需要真实 `launchctl` 环境
- **开发路径敏感：** plist 使用 `current_exe()`；开发模式下可能指向 `target/debug/tm-watcher`
- **清理要求更明确：** `cargo clean` 删除开发二进制后，需要重新执行 `tm-watcher daemon start`

## 风险缓解

- README 说明开发模式 plist 指向 `current_exe()`，`cargo clean` 后需重新执行 `tm-watcher daemon start`
- `daemon start` 前预检 Time Machine、配置和数据库，避免 daemon 启动后立即失败造成 crash loop
- `daemon start` 静默清理旧 PID 文件残留，避免旧实现状态干扰新实现
- Issue #6 负责真实 macOS/Time Machine 环境下的 `daemon start` / `daemon stop` / `daemon status` E2E 验证
