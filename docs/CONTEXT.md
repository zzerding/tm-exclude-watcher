<!-- ABOUTME: 定义 tm-watcher 的领域语言、核心概念和长期设计约定。 -->

# Time Machine Watcher - 领域语言

## 核心概念

### 排除规则 (Exclusion Rule)
匹配模式，定义在配置文件中。例如：`"node_modules"`、`"target"`、`".venv"`。

规则本身是静态的、持久的，不会"过期"。

**匹配语义：精确的目录名匹配（exact basename match）**
- `"node_modules"` 匹配任何路径下名字恰好是 `node_modules` 的目录
- `~/Code/node_modules_backup/` 不匹配（名字是 `node_modules_backup`）
- `~/Code/foo/bar/node_modules/` 匹配（basename 是 `node_modules`）
- 配置中不带末尾 `/`（文档中的 `/` 仅为说明这是目录）

### 排除目录 (Excluded Directory)
被规则匹配到的具体文件系统路径。例如：`/Users/biz/Code/project-a/node_modules`。

这是实际被添加到 Time Machine 排除列表的路径。可能因为目录被删除而"过期"。

### 排除记录 (Exclusion Record)
数据库中存储的条目，包含：
- 被排除的目录路径
- 匹配到的规则
- 目录大小（`size_bytes`）
- 时间戳和元数据

记录的生命周期：目录被监控到 → 添加记录 → 目录删除 → 清理记录。

**目录大小的用途与维护：**
- **用途：** 统计功能（"节省了多少备份空间"），辅助用户决策哪些目录值得手动清理
- **维护策略：** 定期刷新模式
  - 排除时：计算并记录初始大小
  - 定期清理时：如果目录仍存在，优先用记录路径本身的顶层修改时间判断是否需要重新计算
  - 如果记录已有 `size_bytes` 且 `recorded_path_mtime_ns` 与当前顶层修改时间一致，跳过递归大小计算，只更新 `last_checked_at`
  - 如果记录缺少顶层修改时间或顶层修改时间变化，重新计算并更新 `size_bytes` 和 `recorded_path_mtime_ns`
  - 数据新鲜度：最多延迟一个清理周期（默认 24 小时）

**顶层修改时间取舍：**
`recorded_path_mtime_ns` 只记录被排除路径本身的修改时间，不扫描子树寻找最新修改时间。这个策略用少量精度换取 `clean` 的稳定性能：嵌套文件内容变化但顶层目录 mtime 未变化时，大小统计可能延迟到后续顶层变化或手动刷新策略更新；Time Machine 排除状态修复和缺失路径清理不受影响。

### 监控路径 (Watch Path)
用户配置的根目录，例如 `~/Documents/src`。

监控行为：**递归扫描所有子目录，无深度限制**。FSEvents 会报告该路径下任何深度的文件系统变化。

理由：开发者项目结构千差万别（monorepo、嵌套 workspace），限制深度会漏掉真实场景。

### 确认延迟 (Confirmation Delay)
检测到符合规则的目录后，等待一段时间再执行排除操作。默认 5 秒。

**目的：Debounce 机制**
1. 过滤生命周期 < 延迟时长的临时目录
2. 避免在文件系统事件风暴中立即触发操作

**行为：**
- 检测到目录创建 → 启动延迟计时器
- 延迟期间目录被删除 → **取消这次排除操作**
- 延迟结束且目录仍存在 → 执行排除并记录

### 清理 (Cleanup)
移除已不存在的目录的排除记录和 Time Machine 排除列表条目。

**三种触发方式（非互斥，共存）：**

1. **实时清理**：监控到目录删除事件时立即清理
2. **定期清理**：定时全量扫描（默认每日一次），检查所有记录对应的目录是否仍存在
3. **手动清理**：用户通过 `tm-watcher clean` 命令手动触发

**定期清理的作用（兜底机制）：**
- 修正实时监控丢失的事件（系统重启、程序崩溃窗口期）
- 检测用户直接调用 `tmutil removeexclusion` 绕过本工具的情况
- 对比数据库记录与 `tmutil isexcluded` 的真实状态，修正脏数据
- 保证最终一致性

## 已移除的设计

### ~~目录大小过滤 (min_directory_size_mb)~~
PRD 最初提议根据目录大小决定是否排除。

**移除理由：**
1. 小的依赖目录（如 10 MB 的 `node_modules`）也不应该备份——可重现的构建产物不需要历史版本
2. 递归计算目录大小是昂贵操作，会成为性能瓶颈
3. 增加不必要的复杂度和配置负担

**结论：** 只要匹配规则，无论大小都排除。

### 守护进程 (Daemon)
后台运行的监控服务，响应文件系统事件并执行排除/清理操作。

**生命周期管理：macOS LaunchAgent**
- `tm-watcher daemon start`：预检 Time Machine、配置和数据库，生成 `~/Library/LaunchAgents/com.zzerding.tm-watcher.plist`，再调用 `launchctl bootstrap`
- `tm-watcher daemon stop`：调用 `launchctl bootout`，并删除 plist
- `tm-watcher daemon status`：调用 `launchctl print` 查询运行状态和 PID
- `tm-watcher daemon restart`：确认 daemon 正在运行，预检新配置，再停止并重新启动 LaunchAgent
- `tm-watcher __daemon`：以前台进程运行 watcher 和定期清理，由 launchd 托管
- LaunchAgent 配置 `RunAtLoad = true` 和 `KeepAlive.SuccessfulExit = false`，提供登录自启和异常重启

**已移除的旧方案：PID 文件守护进程**
- 不再 fork 后自行管理后台进程
- 不再依赖 `~/.local/var/run/tm-watcher.pid` 判断运行状态
- 不再手写 PID 复用防御和 SIGTERM 轮询停止
- `daemon start` 会静默清理旧版本遗留 PID 文件

### 扫描 (Scan)
手动触发的全量扫描命令：`tm-watcher scan <path>`

**行为：**
- 递归扫描指定路径及其所有子目录（与守护进程监控行为一致）
- 对每个匹配规则的目录，检查是否已有数据库记录：
  - 已有记录 → 信任数据库并跳过，不在扫描热路径调用 `tmutil isexcluded`
  - 无记录 → 立即排除并创建记录（无延迟）
- 数据库记录与 Time Machine 真实状态不一致时，由 `clean` 命令或定期清理负责修正；扫描命令优先保证重扫性能。

**用途：**
- 用户刚安装工具，一次性排除所有现有依赖目录（"补历史"）
- 守护进程停止期间创建的目录，手动触发扫描补漏

**幂等性：** 多次扫描同一路径是安全的，不会产生重复记录或重复操作。

**预览模式：** `tm-watcher scan --dry-run <path>` 只预览将被排除的目录。
- 不调用 `tmutil`
- 不写数据库
- 通过已有数据库记录区分"将要排除"和"已跳过"
- 数据库不存在时按无已记录路径处理，不创建数据目录或数据库文件

### 符号链接 (Symlink)
符号链接的处理策略：**不跟随（treat as regular files）**

**行为：**
- 文件系统监控和扫描只处理符号链接本身，不递归进入其指向的目录
- 仅当符号链接的**名字**匹配规则时才排除它（例如名为 `node_modules` 的符号链接）
- 符号链接指向的真实目录由其自身路径决定是否被排除

**理由：**
1. 避免循环引用导致的无限递归
2. 避免重复排除（真实目录可能已在其他路径被排除）
3. 简化逻辑，提升性能
4. Time Machine 本身默认不跟随符号链接

**示例：**
```
~/Code/shared-deps/node_modules/        (真实目录) → 被排除
~/Code/project/node_modules -> ../shared-deps/node_modules/  (符号链接) → 被排除（名字匹配）
~/Code/project/deps -> /tmp/cache/      (符号链接) → 不被排除（名字不匹配）
```

### 配置 (Configuration)
工具行为通过配置文件控制，位于 `~/.config/tm-watcher/config.toml`。

**配置层级（MVP）：**
- **单一全局配置**：所有设置、监控路径、规则都在这一个文件中
- 不支持项目级配置文件（未来可通过白名单机制实现定制需求）

**零配置体验：**
首次运行 `tm-watcher daemon start` 时，如果配置文件不存在，自动生成默认配置：
```toml
watch_paths = ["~/Documents", "~/Projects", "~/Code", "~/Developer"]
exclude_rules = [
    "node_modules", "target", "vendor", 
    ".venv", "venv", "virtualenv", "__pycache__",
    "build", "dist", ".next", ".nuxt", ".cache"
]
cleanup_on_delete = true
confirmation_delay_seconds = 5
interval_hours = 24
```

**配置说明：**
- 默认监控常见开发目录（路径不存在也不报错，静默跳过）
- 用户通过 `tm-watcher config show` 查看配置，通过 `config add-path` / `config add-rule` 添加监控路径和排除规则
- 配置变更后需要运行 `tm-watcher daemon restart` 重启 daemon 使其生效
- 真正零配置：安装后直接 `tm-watcher daemon start` 即可工作

**配置内容：**
- `watch_paths`：要监控的根目录列表
- `exclude_rules`：目录名匹配规则列表
- `confirmation_delay_seconds`：确认延迟（秒）
- `cleanup_on_delete`：是否启用实时清理
- `interval_hours`：定期清理间隔（小时）

**未来扩展（v0.3+）：**
可通过 `whitelist_paths` 实现特定路径的排除豁免（例如某些特殊项目的依赖需要备份）。

### 错误处理 (Error Handling)
与 `tmutil` 交互时的失败场景分级处理。

**场景分类与处理策略：**

**1. 目录不存在（Error -43）**
- 场景：`tmutil addexclusion` 时目录在延迟期间被删除
- 处理：静默忽略，记录 debug 级别日志
- 理由：目录已不存在，无需排除，这是正常的竞态条件

**2. 权限不足（Operation not permitted）**
- 场景：尝试排除受保护的系统路径
- 处理：记录 warning 日志，跳过该目录，**不写入数据库**
- 理由：无法排除的目录不应记录为"已排除"

**3. Time Machine 未配置**
- 场景：系统未启用 Time Machine
- 处理：**启动时检测，未配置则输出错误并退出**
- 理由：Time Machine 是工具运行的前提条件，不满足无法工作

**前置检查：**
守护进程启动时执行 `tmutil status` 或 `tmutil destinationinfo`，确认 Time Machine 已配置且可用。

### 日志 (Logging)
使用 `tracing` + `tracing-subscriber` 记录运行状态和错误。

**输出目标（MVP）：**
- **守护进程：** 写入文件 `~/.local/share/tm-watcher/daemon.log`
- **CLI 命令：** 输出到 stderr（用户交互式执行时可见）

**日志级别分类：**
- **Info：** 排除/清理操作（"Excluded /path/to/dir", "Cleaned 3 stale entries"）
- **Warning：** tmutil 失败但可恢复的错误（权限不足、目录不存在）
- **Error：** 致命错误（Time Machine 未配置、数据库损坏）

**日志轮转（MVP）：**
使用 `tracing-appender` 的 rolling writer 写入固定文件 `daemon.log`；当前不实现日志保留、压缩或清理策略。后续版本可加入"保留最近 7 天"策略。

**查看命令：**
`tm-watcher logs` 默认显示 `daemon.log` 最后 50 行；`tm-watcher logs -n <行数>` 控制显示行数；`tm-watcher logs --follow` 实时追踪新增日志。

### 数据存储 (Data Storage)
使用 SQLite 存储排除记录，文件位于 `~/.local/share/tm-watcher/exclusions.db`。

**目录结构：**
```
~/.local/share/tm-watcher/
├── daemon.log         # 守护进程日志
└── exclusions.db      # SQLite 数据库
```

**Schema 版本管理：**
- 使用 SQLite `PRAGMA user_version` 记录 schema 版本号
- v1 schema：基础排除记录字段（`path`、`rule`、`size_bytes`、`created_at`、`last_checked_at`）
- v2 schema：新增 `recorded_path_mtime_ns`，用于 `clean` 跳过未变化目录的递归大小计算
- 可写打开数据库时，工具会把 v1 自动迁移到 v2
- `scan --dry-run` 只读打开数据库且不写入 schema；它允许读取 v1 基础 schema 来保持预览模式不改用户状态

**理由：**
- 排除记录可重建，但保留大小统计和检查时间能减少用户重新扫描成本
- v1 → v2 是追加 nullable 列，迁移风险低，适合自动执行
- 预览模式必须继续满足“不写数据库”的承诺

### 并发控制 (Concurrency Control)
多个操作可能同时访问数据库和文件系统（守护进程、定期清理、CLI 命令）。

**并发策略：多进程 + 数据库事务**

**架构：**
- CLI 命令（`scan`/`clean`）直接操作数据库，不通过 IPC
- SQLite 的文件锁机制保证多进程并发安全
- 守护进程内部使用单线程事件循环（Rust async runtime），避免线程竞争
- 定期清理作为独立 task，与 FSEvents 处理通过 channel 通信

**防止竞态条件：**
1. **重复排除：** 使用 `INSERT OR IGNORE`（schema 中 `path` 字段有 `UNIQUE` 约束）
2. **排除与清理冲突：** 清理操作先 `SELECT` 检查记录是否存在，再 `DELETE`
3. **目录状态检查：** 操作前用 `std::fs::metadata` 确认目录当前状态

**理由：**
- SQLite 自带并发控制，无需额外 IPC 机制
- 实现简单，符合 MVP 目标
- 性能足够（文件系统操作比数据库慢得多，不存在瓶颈）

## v0.2.0 发布范围

### 包含的功能
- **核心监控：** FSEvents 文件系统监控，递归扫描监控路径
- **自动排除：** 检测匹配规则的目录，延迟确认后排除
- **实时清理：** 检测到目录删除时立即清理排除记录
- **定期清理：** 每 24 小时全量扫描，修正脏数据（兜底机制）
- **CLI 命令：**
  - `daemon start` / `daemon stop` - 通过 launchd 启动/停止守护进程
  - `scan <path>` - 手动扫描指定路径
  - `list` - 列出所有已排除目录
  - `daemon status` - 显示监控状态（监控路径、已排除数量、最后清理时间）
  - `clean` - 手动触发清理
- **数据存储：** SQLite 数据库，记录排除目录及元数据
- **日志系统：** 写入 `~/.local/share/tm-watcher/daemon.log`
- **错误处理：** tmutil 失败分级处理，启动时检测 Time Machine 状态
- **零配置：** 首次运行自动生成默认配置
- **GitHub Release：** stable/RC tag 生成 macOS 双架构 tarball 和 `SHA256SUMS`
- **Homebrew 安装：** stable 发布自动更新 `zzerding/homebrew-tap` formula，安装后不自动启动 daemon

### 推迟到后续版本
- macOS 通知中心集成
- 日志轮转

## v0.3.0 开发范围

### 已进入实现
- `doctor` 命令：执行 Time Machine、配置文件、数据库、daemon 和 LaunchAgent plist 健康检查；任何失败或警告返回非 0。
- `scan --dry-run`：预览匹配目录并显示匹配规则；不调用 `tmutil`，不写数据库。
- `logs` 命令：查看 daemon 日志尾部，支持 `-n <行数>` 和 `--follow`。
- `daemon status` 命令：显示数据库已知大小合计的累计节省空间；没有已知大小时提示运行 `tm-watcher clean` 更新大小信息。
- `config` 命令：支持 `show`、`add-path <路径>`、`add-rule <规则>`；更新配置后提示重启 daemon。
