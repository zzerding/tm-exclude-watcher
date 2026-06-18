<!-- ABOUTME: 记录 tm-watcher 面向用户的版本变化和破坏性迁移说明。 -->

# Changelog

## [0.3.0] - 2026-06-18

### Breaking Changes

- CLI 生命周期命令迁移为 `daemon` 子命令：
  - `tm-watcher start` -> `tm-watcher daemon start`
  - `tm-watcher stop` -> `tm-watcher daemon stop`
  - `tm-watcher status` -> `tm-watcher daemon status`
- 配置命令迁移为显式子命令：
  - `tm-watcher config --show` -> `tm-watcher config show`
  - `tm-watcher config --add-path <path>` -> `tm-watcher config add-path <path>`
  - `tm-watcher config --add-rule <rule>` -> `tm-watcher config add-rule <rule>`
- 旧命令不会作为兼容别名继续执行，只返回迁移提示。

### Added

- `tm-watcher daemon restart` 用于停止后重新启动 LaunchAgent。
- `tm-watcher config show` 显示当前配置文件、监控路径、排除规则和清理策略。
- `tm-watcher config add-path <path>` 添加监控路径，并在路径已被覆盖时跳过重复配置。
- `tm-watcher config add-rule <rule>` 添加排除规则，并在规则已存在时跳过重复配置。
- `tm-watcher daemon status` 显示已排除目录的已知节省空间。
- `tm-watcher logs [-n <lines>] [--follow]` 查看 daemon 日志尾部并支持实时追踪。
- `tm-watcher scan <path> --dry-run` 预览将排除的目录，不调用 `tmutil`，不写入数据库。
- `tm-watcher doctor` 执行 Time Machine、配置文件、数据库、daemon 和 LaunchAgent 健康检查。
