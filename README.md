# tm-watcher

[![GitHub Release](https://img.shields.io/github/v/release/zzerding/tm-exclude-watcher)](https://github.com/zzerding/tm-exclude-watcher/releases)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)
[![Rust](https://img.shields.io/badge/Rust-Edition%202024-orange?logo=rust)](https://www.rust-lang.org/)
![Release Workflow](https://github.com/zzerding/tm-exclude-watcher/actions/workflows/release.yml/badge.svg)

你的 Time Machine 备份盘是不是被无数个 `node_modules`、`target`、`build` 目录迅速塞满？`tm-watcher` 是一款轻量级的 macOS 命令行工具，安装后只需 `tm-watcher daemon start`，即可自动把可重现的依赖目录排除在备份之外，目录删除后还会自动清理记录。从此告别手动维护 Time Machine 排除列表，让备份更快、备份盘更耐用，你也更专注于写代码。

## 功能特性

- 🔍 **递归扫描**：扫描指定路径下所有匹配规则的子目录
- 👀 **预览模式**：先用 `scan --dry-run` 查看将排除的目录，不修改系统状态
- 🚫 **自动排除**：调用 `tmutil` 将目录添加到 Time Machine 排除列表
- 📊 **记录管理**：本地数据库记录所有排除操作
- 🧹 **智能清理**：检测失效记录并同步清理 Time Machine 排除列表
- 🩺 **健康检查**：检查 Time Machine、配置、数据库和 daemon 状态

## 系统要求

- **操作系统**：macOS 10.13 或更高版本
- **Time Machine**：必须已启用并配置备份磁盘

## 安装

### Homebrew

stable 发布后，可通过 Homebrew 安装：

```bash
brew tap zzerding/tap
brew install tm-watcher
```

Homebrew 安装后不会自动启动 daemon。需要后台监控时，显式运行 `tm-watcher daemon start`；检查状态用 `tm-watcher daemon status`；停止后台监控用 `tm-watcher daemon stop`。

### GitHub Release 二进制

从 GitHub Release 下载对应版本和架构的 tarball：

```text
tm-watcher-v<version>-aarch64-apple-darwin.tar.gz
tm-watcher-v<version>-x86_64-apple-darwin.tar.gz
```

解包后把 `tm-watcher` 放到 PATH 中：

```bash
VERSION=<version>
ARCH=aarch64-apple-darwin
shasum -a 256 -c SHA256SUMS
tar -xzf "tm-watcher-v${VERSION}-${ARCH}.tar.gz"
install -m 0755 tm-watcher*/tm-watcher /usr/local/bin/tm-watcher
```

### 源码安装

需要 Rust 工具链。如果没有请先安装：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

从源码编译安装：

```bash
git clone https://github.com/zzerding/tm-exclude-watcher.git
cd tm-exclude-watcher
cargo install --path .
```

## 使用

### 扫描现有项目目录

递归扫描指定路径，自动排除所有匹配规则的依赖目录：

```bash
tm-watcher scan ~/Documents/src
```

输出示例：

```text
扫描中: /Users/biz/Documents/src

扫描完成:
  新排除: 12 个目录
  已跳过: 3 个目录（之前已排除）
```

预览将被排除的目录，不调用 `tmutil`，不写数据库：

```bash
tm-watcher scan ~/Documents/src --dry-run
```

### 查看已排除的目录

```bash
tm-watcher list
```

输出示例：

```text
排除记录: 3 条，已知大小合计 2.4 GB，未知大小 1 条

#  大小      规则          检查时间          路径
1  2.3 GB    target        2026-06-11 10:30  ~/Code/project-b/target
2  145.2 MB  node_modules  2026-06-11 10:30  ~/Code/project-a/node_modules
3  未知      vendor        未检查            ~/Code/project-c/vendor
```

### 清理失效记录

检查已记录的目录是否仍存在，清理失效记录并刷新目录大小：

```bash
tm-watcher clean
```

输出示例：

```text
清理完成:
  清理: 2 条记录
  检查: 15 条记录
  错误: 0 个
```

### 守护进程模式

启动守护进程，自动监控配置的路径并定期清理失效记录：

```bash
# 启动
tm-watcher daemon start

# 查看状态
tm-watcher daemon status

# 停止
tm-watcher daemon stop
```

**特性：**
- **登录自启**：守护进程在用户登录时自动启动（macOS LaunchAgent）
- **崩溃重启**：异常退出时自动重启，正常退出（stop 命令）不会拉起
- **日志路径**：`~/.local/share/tm-watcher/daemon.log`
- **升级提示**：`tm-watcher daemon status` 会检查 daemon 状态；如果 LaunchAgent 仍指向旧二进制路径，会提示运行 `tm-watcher daemon stop && tm-watcher daemon start`

**开发者注意：**
- plist 指向 `current_exe()` 绝对路径，开发模式下是 `target/debug/tm-watcher`
- `cargo clean` 后需重新执行 `tm-watcher daemon start`
- 手动替换二进制后可用 `tm-watcher daemon status` 检查是否需要重启 daemon

### 查看 daemon 日志

```bash
# 显示最近 50 行
tm-watcher logs

# 显示最近 100 行
tm-watcher logs -n 100

# 实时追踪
tm-watcher logs --follow
```

### 健康检查

检查 Time Machine、配置文件、数据库、daemon 状态和 LaunchAgent：

```bash
tm-watcher doctor
```

## 配置

配置文件位于 `~/.config/tm-watcher/config.toml`，首次运行时自动生成。

默认排除规则包括：`node_modules`、`target`、`vendor`、`.venv`、`venv`、`__pycache__`、`build`、`dist`、`.next`、`.nuxt`、`.cache` 等常见开发依赖目录。

使用 `tm-watcher config` 查看或更新监控路径和排除规则：

```bash
# 查看当前配置
tm-watcher config show

# 添加监控路径
tm-watcher config add-path ~/Projects

# 添加排除规则
tm-watcher config add-rule ".pytest_cache"
```

配置变更后，运行 `tm-watcher daemon restart` 重启 daemon 使其生效。

## 工作原理

1. **扫描**：递归遍历指定目录，查找匹配规则的子目录（如 `node_modules`、`target`）
2. **排除**：调用 `tmutil addexclusion` 将目录添加到 Time Machine 排除列表
3. **记录**：写入本地 SQLite 数据库（`~/.local/share/tm-watcher/exclusions.db`）
4. **清理**：`clean` 命令检查已记录目录是否仍存在，自动清理失效记录并修正状态漂移

## 发布状态

- [x] 手动扫描与排除（v0.1）
- [x] 清理失效记录（v0.1）
- [x] 实时文件系统监控（v0.2）
- [x] 后台守护进程（v0.2）
- [x] 日志和可观测性（v0.2）
- [x] GitHub Release macOS 双架构资产（v0.2）
- [x] Homebrew formula 生成和 tap 更新 workflow（v0.2）
- [x] 日志查看命令（v0.3）
- [x] 配置管理命令（v0.3）
- [x] 健康检查和扫描预览（v0.3）
- [ ] Apple Silicon 真机 E2E 和 stable 发布验收（v0.3）

当前版本：**v0.3.0**

## 联系我

有问题、想反馈，或者单纯打个招呼，可以通过以下方式找到我：

- [linux.do](https://linux.do/u/zzerd/summary)
- [V2EX](https://v2ex.com/member/zzerd)

## 相关文档

- [英文版 README](./README.en.md) — 本页的英文翻译
- [docs/CONTEXT.md](./docs/CONTEXT.md) — 领域语言、核心概念与长期设计约定
- [docs/tm-exclude-watcher-prd.md](./docs/tm-exclude-watcher-prd.md) — 产品目标、功能范围、路线图与测试策略
- [skills/stacked-issue-pr-workflow/SKILL.md](./skills/stacked-issue-pr-workflow/SKILL.md) — 通过 stacked pull request 实现 GitHub issue 的协作流程说明

## License

MIT
