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

需要 Rust 工具链，如果没有请先安装：

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
已排除的目录 (3 条记录):

1. /Users/biz/Code/project-a/node_modules
   规则: node_modules | 大小: 145.2 MB | 最后检查: 2026-06-11 10:30:15

2. /Users/biz/Code/project-b/target
   规则: target | 大小: 2.3 GB | 最后检查: 2026-06-11 10:30:16
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

## 配置

配置文件位于 `~/.config/tm-watcher/config.toml`，首次运行时自动生成。

默认排除规则包括：`node_modules`、`target`、`vendor`、`.venv`、`venv`、`__pycache__`、`build`、`dist`、`.next`、`.nuxt`、`.cache` 等常见开发依赖目录。

可手动编辑配置文件来自定义规则和监控路径。

## 工作原理

1. **扫描**：递归遍历指定目录，查找匹配规则的子目录（如 `node_modules`、`target`）
2. **排除**：调用 `tmutil addexclusion` 将目录添加到 Time Machine 排除列表
3. **记录**：写入本地 SQLite 数据库（`~/.local/share/tm-watcher/exclusions.db`）
4. **清理**：`clean` 命令检查已记录目录是否仍存在，自动清理失效记录并修正状态漂移

## Roadmap

- [x] 手动扫描与排除（v0.1）
- [x] 清理失效记录（v0.1）
- [ ] 实时文件系统监控（v0.2）
- [ ] 后台守护进程（v0.2）
- [ ] Homebrew 发布（v1.0）

当前版本：**v0.1.0 (MVP)**

## License

MIT
