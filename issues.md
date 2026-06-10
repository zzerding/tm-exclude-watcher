# Issues to Create

## Issue #4: 守护进程模式（Daemon mode）

**Labels**: enhancement

**Body**:
```
## Parent

无（顶层切片）

## What to build

实现守护进程模式，将监控和定期清理功能整合到后台服务中。用户可以启动/停止守护进程，守护进程同时运行实时监控任务和定期清理调度器。

这个切片包含：
- 守护进程管理器：fork 到后台、PID 文件管理、信号处理（SIGTERM）
- 后台监控任务：整合 #3 的 FsWatcher，监控配置中的所有 watch_paths
- 定期清理调度器：整合 #2 的 Cleaner，按 cleanup.interval_hours 配置定期执行
- CLI 命令：`tm-watcher start`、`tm-watcher stop`、`tm-watcher status`

## Acceptance criteria

- [ ] `tm-watcher start` 启动守护进程并 fork 到后台
- [ ] PID 文件写入 ~/.local/var/run/tm-watcher.pid
- [ ] 如果守护进程已在运行，重复启动时返回错误提示「已在运行」
- [ ] 守护进程同时运行两个任务：实时监控所有 watch_paths + 定期清理（按 interval_hours 配置）
- [ ] 日志输出到 ~/.local/share/tm-watcher/daemon.log
- [ ] `tm-watcher status` 显示：运行状态（是否运行中）、监控路径列表、已排除目录数量、最后清理时间（从日志或数据库推断）
- [ ] `tm-watcher stop` 发送 SIGTERM 信号，守护进程优雅退出（清理资源、删除 PID 文件）
- [ ] 守护进程接收 SIGTERM 后记录日志并退出
- [ ] 检查 Time Machine 是否配置（`tmutil destinationinfo`），未配置则拒绝启动并输出错误

## Blocked by

- #2 — 需要 Cleaner 模块
- #3 — 需要 FsWatcher 模块
```

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

- #1 — 手动扫描与排除
- #2 — 查看和手动清理
- #3 — 实时目录监控
- #4 — 守护进程模式
- #5 — 日志和可观测性
```
