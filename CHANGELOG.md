<!-- ABOUTME: 记录 tm-watcher 面向用户的版本变化和破坏性迁移说明。 -->

# Changelog

## Unreleased

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
