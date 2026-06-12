# tm-watcher

自动管理 macOS Time Machine 备份排除列表的命令行工具。监控开发目录，自动排除 `node_modules`、`target` 等依赖目录，目录删除后自动清理排除记录。

## 功能特性

- 🔍 **递归扫描**：扫描指定路径下所有匹配规则的子目录
- 🚫 **自动排除**：调用 `tmutil` 将目录添加到 Time Machine 排除列表
- 📊 **记录管理**：本地数据库记录所有排除操作
- 🧹 **智能清理**：检测失效记录并同步清理 Time Machine 排除列表

## 系统要求

- **操作系统**：macOS 10.13 或更高版本
- **Time Machine**：必须已启用并配置备份磁盘

## 安装

### Homebrew

stable 发布后，普通用户可通过 Homebrew 安装：

```bash
brew tap zzerding/tap
brew install tm-watcher
```

Homebrew 安装后不会自动启动 daemon。需要后台监控时，显式运行 `tm-watcher start`；检查状态用 `tm-watcher status`；停止后台监控用 `tm-watcher stop`。

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

```
扫描中: /Users/biz/Documents/src

扫描完成:
  新排除: 12 个目录
  已跳过: 3 个目录（之前已排除）
```

### 查看已排除的目录

```bash
tm-watcher list
```

输出示例：

```
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

```
清理完成:
  清理: 2 条记录
  检查: 15 条记录
  错误: 0 个
```

### 守护进程模式

启动守护进程，自动监控配置的路径并定期清理失效记录：

```bash
# 启动
tm-watcher start

# 查看状态
tm-watcher status

# 停止
tm-watcher stop
```

**特性:**
- **登录自启:** 守护进程在用户登录时自动启动（macOS LaunchAgent）
- **崩溃重启:** 异常退出时自动重启，正常退出（stop 命令）不会拉起
- **日志路径:** `~/.local/share/tm-watcher/daemon.log`
- **升级提示:** `tm-watcher status` 会检查 daemon 状态；如果 LaunchAgent 仍指向旧二进制路径，会提示运行 `tm-watcher stop && tm-watcher start`

**开发者注意:**
- plist 指向 `current_exe()` 绝对路径，开发模式下是 `target/debug/tm-watcher`
- `cargo clean` 后需重新执行 `tm-watcher start`
- 手动替换二进制后可用 `tm-watcher status` 检查是否需要重启 daemon

## 配置

配置文件位于 `~/.config/tm-watcher/config.toml`，首次运行时自动生成。

默认排除规则包括：`node_modules`、`target`、`vendor`、`.venv`、`venv`、`__pycache__`、`build`、`dist`、`.next`、`.nuxt`、`.cache` 等常见开发依赖目录。

可手动编辑配置文件来自定义规则和监控路径。

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
- [ ] Apple Silicon 真机 E2E 和 stable 发布验收（v0.2）

当前版本：**v0.2.0-rc.2**

## License

MIT
