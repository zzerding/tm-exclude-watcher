# ADR-0002: 守护进程生命周期分阶段实现

**状态：** 已接受  
**日期：** 2026-06-10  
**决策者：** Doctor Biz

## 上下文

macOS 后台服务有多种实现方式，需要在"快速验证核心功能"和"完整用户体验"之间做权衡。

**用户期望：**
- 开机自动启动
- 崩溃自动重启
- 集成系统服务管理

**开发约束：**
- MVP 需要快速验证核心监控逻辑
- LaunchAgent 需要处理权限、安装脚本、plist 生成等复杂性

## 决策

**分阶段实现守护进程生命周期管理：**

### MVP (v0.1.0)：后台进程 + PID 文件
- `tm-watcher start`：fork 到后台，写 PID 到 `~/.local/var/run/tm-watcher.pid`
- `tm-watcher stop`：读取 PID 并发送 SIGTERM
- 用户需要每次开机后手动启动

### v1.0.0：LaunchAgent 集成
- 安装 `~/Library/LaunchAgents/com.tm-watcher.plist`
- `tm-watcher start/stop` 调用 `launchctl load/unload`
- Homebrew 安装时自动配置 LaunchAgent
- macOS 自动管理开机启动和崩溃重启

## 备选方案

### 方案 A：MVP 直接实现 LaunchAgent（未采用）
**拒绝理由：**
- LaunchAgent 配置错误可能导致启动失败，增加调试时间
- 需要处理权限问题（某些 macOS 版本需要用户授权）
- plist 模板、安装脚本、卸载逻辑会拖慢 MVP 开发
- 核心监控逻辑尚未验证，过早优化用户体验

### 方案 B：永远只用 PID 文件（未采用）
**拒绝理由：**
- 用户体验差：每次开机手动启动
- 不符合 macOS 生态最佳实践
- 无崩溃重启机制

## 后果

### 正面
- **快速迭代：** MVP 可专注于核心监控逻辑，2-3 天内完成验证
- **降低风险：** 避免过早陷入 LaunchAgent 的坑（权限、沙盒、日志重定向）
- **渐进增强：** v0.1 用户可手动启动，v1.0 用户自动获得 LaunchAgent 升级

### 负面
- **用户体验欠佳：** v0.1 用户需要每次开机后执行 `tm-watcher start`
- **迁移成本：** v0.1 → v1.0 升级时需要指导用户从手动启动迁移到 LaunchAgent

## 风险缓解

- v0.1 文档中明确说明"需要手动启动"，设置用户预期
- v1.0 升级脚本自动检测并迁移：kill 旧进程 → 安装 LaunchAgent → 启动服务
- 提供 `tm-watcher install-service` 命令，让 v0.1 用户可提前手动安装 LaunchAgent
