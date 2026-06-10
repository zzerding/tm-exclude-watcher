# tm-watcher 实现计划

## 总体架构

基于 CONTEXT.md 和 ADR 文档，MVP (v0.1.0) 包含以下模块：

```
tm-watcher/
├── src/
│   ├── main.rs           # CLI 入口
│   ├── config.rs         # 配置管理
│   ├── database.rs       # SQLite 操作
│   ├── rules.rs          # 规则匹配引擎
│   ├── exclusion.rs      # tmutil 包装
│   ├── scanner.rs        # 目录扫描
│   ├── watcher.rs        # FSEvents 监控
│   ├── cleaner.rs        # 清理逻辑
│   └── daemon.rs         # 守护进程管理
└── Cargo.toml
```

## 依赖层级

```
第 0 层（无依赖）：config.rs, rules.rs
第 1 层（依赖第 0 层）：database.rs, exclusion.rs
第 2 层（依赖第 0-1 层）：scanner.rs, cleaner.rs
第 3 层（依赖第 0-2 层）：watcher.rs, daemon.rs
第 4 层（依赖所有）：main.rs
```

## 实现阶段

### 阶段 1：基础设施（数据层）
1. 初始化 Rust 项目
2. 配置管理（读取/生成默认配置）
3. 数据库 schema 和基础操作
4. 规则匹配引擎

### 阶段 2：核心操作（业务层）
5. tmutil 包装器（addexclusion/removeexclusion）
6. 目录扫描器（递归扫描 + 规则匹配）
7. 清理器（检查目录存在性 + 清理记录）

### 阶段 3：监控与自动化（服务层）
8. FSEvents 文件系统监控
9. 守护进程管理（PID 文件 + 后台运行）
10. 定期清理调度器

### 阶段 4：CLI 接口（应用层）
11. CLI 命令实现（start/stop/scan/list/status/clean）
12. 日志系统集成
13. 错误处理和用户反馈

## 详细步骤分解

见下文各 Prompt。

---

# 实现 Prompts

## Prompt 1: 初始化项目和配置模块

```
创建 Rust 项目 tm-watcher，实现配置管理模块。

要求：
1. 初始化 Cargo 项目，添加依赖：
   - serde = { version = "1.0", features = ["derive"] }
   - toml = "0.8"
   - dirs = "5.0"

2. 在 src/config.rs 实现配置结构和加载逻辑：
   - Config 结构体，字段：watch_paths, exclude_rules, cleanup (enabled, interval_hours, cleanup_on_delete), behavior (confirmation_delay_seconds)
   - load() 函数：读取 ~/.config/tm-watcher/config.toml，不存在则生成默认配置
   - 默认配置：watch_paths = ["~/Documents", "~/Projects", "~/Code", "~/Developer"]
                exclude_rules = ["node_modules", "target", "vendor", ".venv", "venv", "virtualenv", "__pycache__", "build", "dist", ".next", ".nuxt", ".cache"]
                cleanup.enabled = true, interval_hours = 24, cleanup_on_delete = true
                behavior.confirmation_delay_seconds = 5
   - 展开 ~ 为用户主目录

3. 在 src/main.rs 验证配置加载：
   - 加载配置并打印 watch_paths 和 exclude_rules

保持代码最简，仅实现必要功能，无额外抽象。
```

---

## Prompt 2: 规则匹配引擎

```
实现规则匹配引擎 src/rules.rs。

要求：
1. RuleMatcher 结构体，包含 exclude_rules: Vec<String>
2. new(rules: Vec<String>) -> Self
3. matches(&self, path: &Path) -> Option<String>：
   - 检查路径的 basename 是否精确匹配任一规则
   - 仅匹配目录（不匹配文件）
   - 不跟随符号链接
   - 返回匹配的规则名称（用于记录到数据库）

4. 在 src/main.rs 验证：
   - 加载配置中的 exclude_rules
   - 创建 RuleMatcher
   - 测试路径："/Users/biz/Code/project/node_modules", "/Users/biz/Code/node_modules_backup"
   - 打印匹配结果

保持简单，不支持 glob 或 regex，仅精确 basename 匹配。
```

---

## Prompt 3: 数据库模块

```
实现数据库模块 src/database.rs。

要求：
1. 添加依赖：rusqlite = { version = "0.31", features = ["bundled"] }

2. Database 结构体，包装 rusqlite::Connection
3. new() -> Result<Self>：
   - 连接到 ~/.local/share/tm-watcher/exclusions.db
   - 目录不存在则创建
   - 设置 busy_timeout = 5000ms
   - 执行 schema 初始化

4. Schema (user_version = 1)：
   CREATE TABLE excluded_directories (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       path TEXT NOT NULL UNIQUE,
       rule_matched TEXT NOT NULL,
       size_bytes INTEGER,
       created_at INTEGER NOT NULL,
       last_checked_at INTEGER NOT NULL
   );
   CREATE INDEX idx_path ON excluded_directories(path);

5. 基础操作：
   - insert(&self, path: &str, rule: &str, size: Option<i64>) -> Result<()>
     使用 INSERT OR IGNORE，created_at 和 last_checked_at 设为当前 Unix 时间戳
   - exists(&self, path: &str) -> Result<bool>
   - list_all(&self) -> Result<Vec<ExclusionRecord>>
     ExclusionRecord 结构体包含所有字段
   - delete(&self, path: &str) -> Result<()>

6. 在 src/main.rs 验证：
   - 创建数据库连接
   - 插入测试记录
   - 列出所有记录
   - 删除记录

无额外封装，直接使用 rusqlite API。
```

---

## Prompt 4: tmutil 包装器

```
实现 tmutil 包装器 src/exclusion.rs。

要求：
1. TmUtil 结构体（空结构体或单例）

2. check_tm_configured() -> Result<bool>：
   - 执行 tmutil destinationinfo
   - 返回码 0 表示已配置，非 0 表示未配置

3. add_exclusion(path: &Path) -> Result<()>：
   - 执行 tmutil addexclusion <path>
   - 错误处理：
     * Exit code 0 → Ok
     * Exit code 非 0 且 stderr 包含 "No such file" → 返回特定错误 ExclusionError::PathNotFound
     * 其他错误 → 返回 ExclusionError::PermissionDenied 或 ExclusionError::Other
   - 使用自定义 ExclusionError 枚举

4. remove_exclusion(path: &Path) -> Result<()>：
   - 执行 tmutil removeexclusion <path>
   - 同样的错误处理逻辑

5. is_excluded(path: &Path) -> Result<bool>：
   - 执行 tmutil isexcluded <path>
   - 解析输出判断是否已排除

6. 在 src/main.rs 验证：
   - 检查 TM 配置状态
   - 创建测试目录 /tmp/test_exclude_dir
   - 添加排除
   - 检查是否已排除
   - 移除排除
   - 清理测试目录

直接调用 std::process::Command，无额外抽象。
```

---

## Prompt 5: 目录扫描器

```
实现目录扫描器 src/scanner.rs。

要求：
1. Scanner 结构体，包含 rule_matcher: RuleMatcher, database: Database, exclusion: TmUtil

2. new(rule_matcher: RuleMatcher, database: Database) -> Self

3. scan_path(&self, root: &Path) -> Result<ScanResult>：
   - 递归遍历 root 下所有目录（使用 walkdir crate）
   - 添加依赖：walkdir = "2.4"
   - 跳过符号链接（不跟随）
   - 对每个目录：
     * 检查是否匹配规则
     * 如果匹配且数据库中不存在：
       - 调用 exclusion.add_exclusion()
       - 计算目录大小（使用 fs_extra crate 的 dir::get_size）
       - 插入数据库
     * 如果匹配但已存在记录：跳过
   - 返回 ScanResult { excluded: usize, skipped: usize, errors: Vec<String> }

4. 错误处理：
   - ExclusionError::PathNotFound → 记录 debug 日志，继续
   - ExclusionError::PermissionDenied → 记录 warning，加入 errors，继续
   - 其他错误 → 加入 errors，继续

5. 添加依赖：fs_extra = "1.3"

6. 在 src/main.rs 验证：
   - 加载配置、创建数据库、规则匹配器
   - 创建 Scanner
   - 扫描测试路径（例如当前目录）
   - 打印扫描结果

保持简单，单线程扫描，无进度报告。
```

---

## Prompt 6: 清理器

```
实现清理器 src/cleaner.rs。

要求：
1. Cleaner 结构体，包含 database: Database, exclusion: TmUtil

2. new(database: Database) -> Self

3. clean(&self) -> Result<CleanResult>：
   - 从数据库获取所有记录
   - 对每条记录：
     * 检查路径是否存在（std::fs::metadata）
     * 如果不存在：
       - 调用 exclusion.remove_exclusion()（忽略 PathNotFound 错误）
       - 从数据库删除记录
       - cleaned_count++
     * 如果存在：
       - 重新计算目录大小
       - 更新数据库的 size_bytes 和 last_checked_at
       - checked_count++
   - 返回 CleanResult { cleaned: usize, checked: usize, errors: Vec<String> }

4. 错误处理策略同 Scanner

5. 在 src/main.rs 验证：
   - 创建 Cleaner
   - 先用 Scanner 扫描添加一些记录
   - 手动删除某个被排除的目录
   - 执行清理
   - 打印清理结果
   - 验证数据库记录已删除

无后台调度，仅实现单次清理逻辑。
```

---

## Prompt 7: 文件系统监控器

```
实现文件系统监控器 src/watcher.rs。

要求：
1. 添加依赖：
   - notify = "6.1"
   - tokio = { version = "1.35", features = ["full"] }

2. FsWatcher 结构体，包含：
   - rule_matcher: RuleMatcher
   - database: Database
   - exclusion: TmUtil
   - config: Config
   - pending_exclusions: HashMap<PathBuf, tokio::task::JoinHandle<()>>

3. new(rule_matcher, database, config) -> Self

4. async fn run(&mut self, watch_paths: Vec<PathBuf>) -> Result<()>：
   - 创建 notify::RecommendedWatcher
   - 监控所有 watch_paths（递归模式）
   - 处理事件循环：
     * 检测到目录创建（EventKind::Create）：
       - 检查是否匹配规则
       - 如果匹配：启动延迟任务（tokio::spawn）
         * sleep(confirmation_delay_seconds)
         * 检查目录是否仍存在
         * 如果存在且数据库中无记录：排除并记录
       - 将 JoinHandle 存入 pending_exclusions
     * 检测到目录删除（EventKind::Remove）：
       - 取消该路径的 pending_exclusion（JoinHandle.abort）
       - 如果 config.cleanup_on_delete，立即清理该路径

5. 在 src/main.rs 验证：
   - 创建 FsWatcher
   - 监控 /tmp/test_watch
   - 创建测试目录 /tmp/test_watch/node_modules
   - 等待 6 秒（超过 confirmation_delay）
   - 检查数据库是否有记录
   - 删除目录
   - 等待 1 秒
   - 检查数据库记录是否被清理
   - 停止监控

使用 tokio 异步运行时，保持逻辑简单。
```

---

## Prompt 8: 守护进程管理

```
实现守护进程管理 src/daemon.rs。

要求：
1. 添加依赖：
   - daemonize = "0.5"
   - signal-hook = "0.3"
   - signal-hook-tokio = { version = "0.3", features = ["futures-v0_3"] }

2. DaemonManager 结构体

3. start() -> Result<()>：
   - 读取 PID 文件 ~/.local/var/run/tm-watcher.pid
   - 如果已存在且进程运行中，返回错误"已在运行"
   - fork 到后台（使用 daemonize crate）
   - 写入新 PID
   - 设置日志输出到 ~/.local/share/tm-watcher/daemon.log
   - 启动主守护进程逻辑（见下）

4. stop() -> Result<()>：
   - 读取 PID 文件
   - 发送 SIGTERM
   - 等待进程退出（轮询 kill -0）
   - 删除 PID 文件

5. is_running() -> Result<bool>：
   - 读取 PID 文件
   - 检查进程是否存在

6. async fn daemon_main(config: Config, database: Database) -> Result<()>：
   - 创建 FsWatcher 和 Cleaner
   - 启动 FsWatcher 监控任务
   - 启动定期清理任务（tokio::spawn interval loop）
   - 监听 SIGTERM 信号优雅退出

7. 在 src/main.rs 验证：
   - 实现简单的 start/stop/status 命令
   - 启动守护进程
   - 检查状态
   - 停止守护进程

保持 PID 文件管理简单，无锁文件。
```

---

## Prompt 9: CLI 命令实现

```
实现完整 CLI 接口 src/main.rs。

要求：
1. 添加依赖：clap = { version = "4.4", features = ["derive"] }

2. 使用 clap 定义命令：
   - start: 启动守护进程
   - stop: 停止守护进程
   - status: 显示状态（是否运行、监控路径、已排除目录数量、最后清理时间）
   - scan <path>: 扫描指定路径
   - list: 列出所有已排除目录（显示路径、规则、大小、创建时间）
   - clean: 手动触发清理

3. 启动前检查：
   - 检查 Time Machine 是否配置（exclusion.check_tm_configured）
   - 未配置则输出错误并退出

4. 每个命令的实现：
   - start: DaemonManager::start()
   - stop: DaemonManager::stop()
   - status: 
     * 检查 is_running()
     * 读取配置的 watch_paths
     * 查询数据库记录数量
     * 显示格式化输出
   - scan: 创建 Scanner，执行 scan_path()，显示结果
   - list: 查询数据库，格式化输出（路径、规则、大小 MB、创建时间）
   - clean: 创建 Cleaner，执行 clean()，显示结果

5. 输出友好的用户消息（中文）

6. 测试完整流程：
   - tm-watcher start
   - tm-watcher status（应显示运行中）
   - tm-watcher scan /tmp/test_path
   - tm-watcher list
   - tm-watcher clean
   - tm-watcher stop

整合所有模块，形成完整可用的 CLI 工具。
```

---

## Prompt 10: 日志系统集成

```
集成日志系统。

要求：
1. 添加依赖：
   - tracing = "0.1"
   - tracing-subscriber = { version = "0.3", features = ["env-filter"] }
   - tracing-appender = "0.2"

2. 在 src/main.rs 初始化日志：
   - CLI 命令：输出到 stderr，级别 INFO
   - 守护进程：输出到 ~/.local/share/tm-watcher/daemon.log，级别 INFO

3. 在关键位置添加日志：
   - exclusion.rs: info!("Excluded {}", path), warn!("Permission denied: {}", path)
   - scanner.rs: info!("Scan completed: {} excluded", result.excluded)
   - cleaner.rs: info!("Cleanup completed: {} cleaned", result.cleaned)
   - watcher.rs: info!("Watching paths: {:?}", paths), debug!("Detected create: {}", path)
   - daemon.rs: info!("Daemon started"), info!("Received SIGTERM, shutting down")

4. 错误处理改进：
   - 将所有 panic 改为返回 Result
   - 顶层 main() 捕获错误并记录日志
   - 优雅退出

5. 测试：
   - 启动守护进程，检查日志文件是否创建
   - 执行各命令，检查 stderr 输出
   - 触发错误场景（TM 未配置、权限不足），检查日志

保持日志简洁，仅记录重要事件和错误。
```

---

## Prompt 11: 最终集成和测试

```
完成最终集成和端到端测试。

要求：
1. 创建 README.md：
   - 项目简介
   - 安装方法（cargo install --path .）
   - 使用方法（各命令示例）
   - 配置说明
   - 注意事项（需要 macOS、Time Machine 配置）

2. 创建 .gitignore：
   - target/
   - Cargo.lock
   - *.db
   - *.log

3. 完整的端到端测试脚本：
   - 检查 TM 状态
   - 创建测试目录结构：
     /tmp/tm_test/
     ├── project-a/node_modules/
     ├── project-b/target/
     └── project-c/.venv/
   - 启动守护进程
   - 等待 10 秒（让监控生效）
   - 检查这些目录是否被排除（tmutil isexcluded）
   - 删除 project-a/node_modules
   - 等待 5 秒
   - 检查排除记录是否被清理
   - 停止守护进程
   - 清理测试数据

4. 错误场景测试：
   - TM 未配置时启动 → 应报错退出
   - 重复启动守护进程 → 应提示已在运行
   - scan 不存在的路径 → 应报错

5. 性能检查：
   - 扫描包含 1000+ 目录的路径
   - 监控资源占用（top 命令）
   - 确认 CPU < 1%，内存 < 10 MB

6. 最终检查清单：
   - [ ] 所有 CLI 命令可用
   - [ ] 守护进程正常启动/停止
   - [ ] 自动排除生效（延迟 5 秒后）
   - [ ] 实时清理生效
   - [ ] 定期清理生效（可缩短 interval 测试）
   - [ ] 数据库记录正确
   - [ ] 日志文件生成
   - [ ] 配置文件自动生成
   - [ ] 错误处理正确
   - [ ] 符号链接不跟随

完成后项目达到 MVP 可发布状态。
```

---

## 实现顺序总结

| 步骤 | 模块 | 依赖 | 验证方式 |
|------|------|------|----------|
| 1 | config.rs | 无 | 打印配置内容 |
| 2 | rules.rs | 无 | 测试路径匹配 |
| 3 | database.rs | 无 | CRUD 操作 |
| 4 | exclusion.rs | 无 | tmutil 命令调用 |
| 5 | scanner.rs | 1-4 | 扫描测试目录 |
| 6 | cleaner.rs | 3-4 | 清理测试记录 |
| 7 | watcher.rs | 1-6 | 监控测试目录 |
| 8 | daemon.rs | 1-7 | 启动/停止守护进程 |
| 9 | main.rs (CLI) | 1-8 | 执行所有命令 |
| 10 | 日志系统 | 9 | 查看日志输出 |
| 11 | 集成测试 | 10 | 完整流程验证 |

每个步骤独立可验证，逐步构建完整系统。






