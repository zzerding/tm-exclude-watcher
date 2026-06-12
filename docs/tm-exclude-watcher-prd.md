<!-- ABOUTME: 记录 tm-watcher 的产品目标、功能范围、路线图和测试策略。 -->

# Time Machine 排除监控工具 - 产品文档

## 📋 项目概述

**项目名称：** `tm-watcher` (Time Machine Exclude Watcher)  
**开发语言：** Rust  
**目标平台：** macOS  
**核心功能：** 自动监控并排除开发依赖目录，目录删除后自动清理排除规则

---

## 🎯 产品目标

构建一个轻量级、高性能的 macOS 后台服务，自动管理 Time Machine 备份排除列表，专为开发者优化备份效率。

### 核心价值
- **自动化：** 无需手动管理排除列表
- **智能清理：** 目录删除后自动清理过期规则
- **零配置：** 开箱即用，支持常见开发工具
- **高性能：** Rust 实现，资源占用极低
- **可扩展：** 灵活的配置系统，支持自定义规则

---

## ✨ 核心功能

### 1. 自动监控与排除

#### 1.1 目录监控
- 使用 macOS FSEvents API 监控文件系统变化
- 实时检测新建的开发依赖目录
- 支持递归扫描指定路径

#### 1.2 默认排除规则
```
node_modules/      # Node.js 依赖
target/            # Rust 构建输出
vendor/            # PHP/Go 依赖
.venv/             # Python 虚拟环境
venv/              # Python 虚拟环境
virtualenv/        # Python 虚拟环境
__pycache__/       # Python 缓存
build/             # 通用构建目录
dist/              # 通用发行目录
.next/             # Next.js 构建缓存
.nuxt/             # Nuxt.js 构建缓存
.cache/            # 项目缓存目录
```

#### 1.3 自动排除流程
1. 检测到新建符合规则的目录
2. 延迟 5 秒确认目录仍存在（避免临时目录）
3. 调用 `tmutil addexclusion <path>` 添加排除
4. 记录到本地数据库

### 2. 智能清理

#### 2.1 过期规则检测
- 定期扫描已排除的目录列表
- 检查目录是否仍然存在
- 识别已删除目录的排除规则

#### 2.2 自动清理
- 对不存在的目录，调用 `tmutil removeexclusion <path>`
- 从本地数据库移除记录
- 生成清理报告

#### 2.3 清理策略
- **实时清理：** 监控到目录删除事件时立即清理
- **定期清理：** 每日一次全量扫描（可配置）
- **手动触发：** 命令行工具支持手动清理

---

## 🏗️ 技术架构

### 技术栈
- **语言：** Rust 1.70+
- **文件监控：** `notify` 或 `fsevent-sys` crate
- **数据存储：** SQLite (通过 `rusqlite`)
- **CLI：** `clap` crate
- **日志：** `tracing` + `tracing-subscriber`
- **macOS API：** 通过 `std::process::Command` 调用 `tmutil`

### 核心模块

```
tm-watcher/
├── src/
│   ├── main.rs              # 程序入口
│   ├── watcher.rs           # 文件系统监控
│   ├── exclusion.rs         # Time Machine 排除管理
│   ├── database.rs          # SQLite 数据库操作
│   ├── config.rs            # 配置管理
│   ├── rules.rs             # 匹配规则引擎
│   └── cleaner.rs           # 过期规则清理
├── config.toml              # 默认配置文件
└── Cargo.toml
```

---

## 🎨 用户界面

### CLI 命令（按版本分组）

#### v0.1.0 - 已实现 ✅

**立即扫描**
```bash
tm-watcher scan ~/Documents/src
# 扫描指定目录并排除所有符合规则的子目录
```

**列出已排除目录**
```bash
tm-watcher list
# 显示所有由本工具管理的排除目录
```

**立即清理**
```bash
tm-watcher clean
# 清理所有过期的排除规则
```

#### v0.2.0 - 代码已实现，发布验证待完成

**启动守护进程**
```bash
tm-watcher start
# 通过 macOS LaunchAgent 启动后台监控服务
```

**停止守护进程**
```bash
tm-watcher stop
```

**查看状态**
```bash
tm-watcher status
# 显示：监控路径、已排除目录数量、最后清理时间
```

#### v0.4.0 - 计划中

**配置管理**
```bash
tm-watcher config --add-path ~/Projects
tm-watcher config --add-rule "*.log"
tm-watcher config --show
```

---

## ⚙️ 配置系统

### 配置文件位置
```
~/.config/tm-watcher/config.toml
```

### 配置示例
```toml
# 监控路径列表（v0.1.0 读取，v0.2.0+ 守护进程监控）
watch_paths = [
    "~/Documents/src",
    "~/Projects",
    "~/Code"
]

# 排除规则（目录名精确匹配，v0.1.0 已支持）
exclude_rules = [
    "node_modules",
    "target",
    "vendor",
    ".venv",
    "venv",
    "__pycache__"
]

# 清理策略（v0.2.0+）
[cleanup]
enabled = true
interval_hours = 24        # 定期清理间隔
cleanup_on_delete = true   # 检测到删除时立即清理

# 行为配置（v0.2.0+）
[behavior]
confirmation_delay_seconds = 5    # 确认延迟

# 高级功能（v0.4.0+）
# min_directory_size_mb = 100     # 只排除大于此值的目录（可选）
```

**注：** v0.1.0 仅实现 `watch_paths` 和 `exclude_rules`，其他配置项为规划功能。

---

## 📊 数据存储

### SQLite Schema
```sql
CREATE TABLE excluded_directories (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    path TEXT NOT NULL UNIQUE,
    rule_matched TEXT NOT NULL,        -- 匹配的规则
    size_bytes INTEGER,                -- 目录大小（可选）
    created_at INTEGER NOT NULL,       -- Unix timestamp
    last_checked_at INTEGER NOT NULL   -- 最后检查时间
);

CREATE INDEX idx_path ON excluded_directories(path);
CREATE INDEX idx_last_checked ON excluded_directories(last_checked_at);
```

---

## 🚀 实现路线图

### MVP (v0.1.0) - 手动扫描与清理 ✅ **已完成**
- [x] 配置系统与默认规则
- [x] 规则匹配引擎
- [x] SQLite 数据存储
- [x] tmutil 包装器
- [x] 递归扫描与自动排除
- [x] 基础 CLI（`scan` / `list` / `clean`）
- [x] 失效记录清理与状态漂移修复

### v0.2.0 - 实时监控与守护进程
- [x] 文件系统监控（FSEvents API）
- [x] 目录创建延迟确认（5秒）
- [x] 目录删除实时清理
- [x] 守护进程模式（`start` / `stop` / `status`）
- [x] LaunchAgent 托管生命周期与 SIGTERM 优雅退出
- [x] 定期清理任务（每 24 小时）
- [ ] 真实机器 E2E 验证与发布打磨

### v0.3.0 - 日志与可观测性
- [ ] 集成 `tracing` 日志系统
- [ ] CLI stderr 与 daemon log 文件分离
- [ ] 操作审计日志
- [ ] 用户通知（macOS 通知中心）

### v0.4.0 - 高级配置
- [ ] 目录大小过滤（`min_directory_size_mb`）
- [ ] 白名单机制（不排除某些特定目录）
- [ ] 自定义规则支持（glob 模式）
- [ ] 排除前预览与确认模式
- [ ] 配置管理命令（`config --add-path` / `--add-rule`）

### v1.0.0 - 生产就绪
- [ ] GUI 状态栏应用（可选）
- [ ] Homebrew 发布
- [ ] 完整文档和测试覆盖

---

## 🔒 安全考虑

1. **权限最小化：** 仅请求必要的文件系统读取权限
2. **数据隔离：** 数据库存储在用户目录，不访问系统级配置
3. **错误处理：** 优雅处理 `tmutil` 命令失败
4. **审计日志：** 所有排除/清理操作记录日志

---

## 📦 分发方式

### Homebrew
```bash
brew install tm-watcher
brew services start tm-watcher
```

### 源码编译
```bash
git clone https://github.com/yourusername/tm-watcher.git
cd tm-watcher
cargo build --release
cargo install --path .
```

### 二进制发布
通过 GitHub Releases 提供预编译二进制文件

---

## 🧪 测试策略

1. **单元测试：** 规则匹配、数据库操作
2. **集成测试：** 文件监控、tmutil 调用
3. **手动测试：** 在真实 macOS 环境验证
4. **性能测试：** 大量目录场景下的资源占用

---

## 📈 成功指标

- 资源占用 < 10 MB RAM
- CPU 空闲时 < 0.1%
- 监控响应延迟 < 5 秒
- 清理准确率 100%（无误删）

---

## 🤝 竞品对比

| 特性 | Asimov (PHP) | tm-watcher (Rust) |
|------|--------------|-------------------|
| 自动监控 | ✅ | ✅ |
| 自动清理 | ❌ | ✅ |
| 性能 | 中等 | 高 |
| 资源占用 | ~50MB | <10MB |
| 可配置性 | 基础 | 高级 |
| 安装依赖 | PHP 运行时 | 无（静态编译）|

---

## 📝 开发注意事项

1. **macOS 兼容性：** 测试 macOS 12+
2. **tmutil 返回值处理：** 某些目录可能返回 Error -43（不存在）
3. **符号链接处理：** 正确识别符号链接，避免重复排除
4. **大文件夹扫描优化：** 使用增量扫描，避免阻塞

---

## 🎯 下一步

1. 创建 GitHub 仓库
2. 实现 MVP 核心功能
3. 编写单元测试
4. 发布 v0.1.0 alpha 版本
5. 收集用户反馈

---

**文档版本：** v1.0  
**最后更新：** 2026-06-10  
**作者：** Doctor Biz  
**License：** MIT
