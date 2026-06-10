# Issue #3: 实时目录监控 - TDD 实施计划

## 📋 核心需求回顾

1. **FSEvents 实时监控**（notify crate）
2. **5 秒确认延迟机制**：目录创建后等待 5 秒，期间删除则取消
3. **自动清理数据库**：删除事件触发数据库记录清理
4. **CLI 命令**：`tm-watcher watch <path>` 前台运行
5. **异步运行时**：tokio

---

## 🎯 测试优先级清单（垂直切片）

### **P0: 核心监控逻辑（第一个可运行的 Tracer Bullet）**

1. ✅ **测试：检测到新建目录事件**
   - 创建测试目录 → 触发 Create 事件 → 捕获路径
   - 验证：EventHandler 收到正确的 Create 事件

2. ✅ **测试：检测到删除目录事件**
   - 删除已有目录 → 触发 Remove 事件 → 捕获路径
   - 验证：EventHandler 收到正确的 Remove 事件

3. ✅ **测试：5 秒确认机制 - 创建后等待**
   - 创建目录 → 5 秒内不触发排除 → 5 秒后触发排除
   - 验证：`tmutil.add_exclusion` 在 5 秒后被调用

4. ✅ **测试：5 秒确认机制 - 取消逻辑**
   - 创建目录 → 3 秒后删除 → 不触发排除
   - 验证：`tmutil.add_exclusion` 从未被调用

### **P1: 数据库集成**

5. ✅ **测试：确认后写入数据库**
   - 创建目录 → 等待 5 秒 → 数据库记录存在
   - 验证：`database.is_recorded()` 返回 true

6. ✅ **测试：删除事件清理数据库**
   - 已记录目录被删除 → 数据库记录被删除
   - 验证：`database.is_recorded()` 返回 false

7. ✅ **测试：规则匹配过滤**
   - 创建 `node_modules/` → 触发排除
   - 创建 `random_dir/` → 不触发排除
   - 验证：只有匹配规则的目录被处理

### **P2: 边界情况与 CLI**

8. ✅ **测试：忽略已记录目录**
   - 创建已在数据库中的目录 → 不重复排除
   - 验证：`tmutil.add_exclusion` 未被调用

9. ⚠️ **测试：CLI 启动与退出**
   - `tm-watcher watch /path` → 监控运行 → Ctrl+C 退出
   - 验证：优雅关闭，无泄漏

10. ⚠️ **测试：错误处理**
    - 监控不存在路径 → 返回错误
    - 监控无权限路径 → 返回错误

---

## 📦 新增依赖

```toml
[dependencies]
rusqlite = { version = "0.31", features = ["bundled"] }
walkdir = "2.4"
dirs = "5.0"
# 新增：
notify = "6.1"                    # FSEvents 监控
tokio = { version = "1.35", features = ["full"] }  # 异步运行时
clap = { version = "4.4", features = ["derive"] }  # CLI 参数解析

[dev-dependencies]
tempfile = "3.8"
# 新增：
tokio-test = "0.4"                # 异步测试工具
```

---

## 🏗️ 核心接口设计

### **1. Watcher 结构体**

```rust
// src/watcher.rs
// ABOUTME: 实时目录监控器，基于 FSEvents 检测目录变化并触发排除逻辑

use notify::{Watcher as NotifyWatcher, RecommendedWatcher, RecursiveMode, Event};
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{Database, RuleMatcher, TmUtilTrait};

pub struct Watcher {
    database: Database,
    rule_matcher: RuleMatcher,
    tmutil: Arc<dyn TmUtilTrait>,
    pending_dirs: Arc<tokio::sync::Mutex<HashMap<PathBuf, tokio::task::JoinHandle<()>>>>,
}

impl Watcher {
    pub fn new(
        database: Database,
        rules: Vec<String>,
        tmutil: Arc<dyn TmUtilTrait>,
    ) -> Self {
        Self {
            database,
            rule_matcher: RuleMatcher::new(rules),
            tmutil,
            pending_dirs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    /// 启动监控（阻塞主线程）
    pub async fn watch(&self, path: &Path) -> Result<(), String> {
        // 实现：创建 notify watcher，处理事件
    }

    /// 处理创建事件（5 秒延迟确认）
    async fn handle_create(&self, path: PathBuf) {
        // 实现：启动 5 秒计时器，取消逻辑
    }

    /// 处理删除事件（取消待处理任务 + 清理数据库）
    async fn handle_remove(&self, path: PathBuf) {
        // 实现：取消 pending 任务，删除数据库记录
    }

    /// 执行排除逻辑
    async fn execute_exclusion(&self, path: PathBuf) {
        // 实现：调用 tmutil + 记录数据库
    }
}
```

### **2. CLI 集成**

```rust
// src/main.rs 新增子命令

#[derive(Parser)]
enum Commands {
    Scan { path: PathBuf },
    List,
    Clean { path: PathBuf },
    // 新增：
    Watch {
        /// 要监控的根目录
        path: PathBuf,
    },
}

async fn handle_watch(path: PathBuf, config: Config) -> Result<(), String> {
    let database = Database::new(&db_path)?;
    let tmutil = Arc::new(RealTmUtil);
    let watcher = Watcher::new(database, config.rules, tmutil);
    
    println!("🔍 开始监控: ", path.display());
    watcher.watch(&path).await?;
    Ok(())
}
```

---

## 🧪 测试策略

### **异步测试挑战**

1. **时间控制**：需要测试 5 秒延迟，但不能让测试运行 5 秒
   - **解决**：使用 `tokio::time::pause()` + `advance()` 控制虚拟时间
   
2. **文件系统事件模拟**：真实 FSEvents 难以在测试中稳定重现
   - **解决**：抽象 `EventSource` trait，生产用 notify，测试用 mock

3. **并发任务验证**：多个目录同时创建/删除
   - **解决**：使用 `tokio::sync::Notify` 同步测试断言点

### **测试架构**

```rust
// tests/watcher_test.rs

#[tokio::test]
async fn test_create_triggers_exclusion_after_5s() {
    tokio::time::pause();  // 暂停时间
    
    let db = Database::new_in_memory().unwrap();
    let mock_tmutil = Arc::new(MockTmUtil::new());
    let watcher = Watcher::new(db, vec!["node_modules".into()], mock_tmutil.clone());
    
    // 模拟创建事件
    watcher.handle_create(PathBuf::from("/test/node_modules")).await;
    
    // 前进 4 秒 → 未触发
    tokio::time::advance(Duration::from_secs(4)).await;
    assert_eq!(mock_tmutil.call_count(), 0);
    
    // 前进到 5 秒 → 触发排除
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::time::sleep(Duration::from_millis(10)).await;  // 让任务执行
    assert_eq!(mock_tmutil.call_count(), 1);
}

#[tokio::test]
async fn test_remove_before_confirm_cancels() {
    tokio::time::pause();
    
    let db = Database::new_in_memory().unwrap();
    let mock_tmutil = Arc::new(MockTmUtil::new());
    let watcher = Watcher::new(db, vec!["node_modules".into()], mock_tmutil.clone());
    
    let path = PathBuf::from("/test/node_modules");
    watcher.handle_create(path.clone()).await;
    
    // 3 秒后删除
    tokio::time::advance(Duration::from_secs(3)).await;
    watcher.handle_remove(path).await;
    
    // 继续前进到 10 秒 → 依然未触发
    tokio::time::advance(Duration::from_secs(7)).await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    assert_eq!(mock_tmutil.call_count(), 0);
}
```

### **Mock vs 真实集成测试**

- **单元测试**：使用 MockTmUtil + 虚拟时间（快速，稳定）
- **集成测试**：使用 tempfile + 真实 notify watcher（慢，验证真实行为）
  - 仅写 1-2 个关键场景的集成测试
  - 放在 `tests/integration/` 目录，用 `#[ignore]` 标记

---

## 🚀 实施顺序（Tracer Bullet）

### **第 1 步：基础异步框架（~30 分钟）**
- [ ] 添加依赖：notify, tokio, clap
- [ ] 创建 `src/watcher.rs` 骨架
- [ ] 测试：创建 Watcher 实例不崩溃

### **第 2 步：事件检测（~1 小时）**
- [ ] 测试：`test_detect_create_event`（红）
- [ ] 实现：notify watcher + channel 接收事件（绿）
- [ ] 测试：`test_detect_remove_event`（红）
- [ ] 实现：处理 Remove 事件（绿）

### **第 3 步：5 秒延迟机制（~1.5 小时）**
- [ ] 测试：`test_create_triggers_after_5s`（使用虚拟时间，红）
- [ ] 实现：`handle_create` + `tokio::time::sleep(5s)`（绿）
- [ ] 测试：`test_remove_cancels_pending`（红）
- [ ] 实现：HashMap 跟踪待处理任务 + `abort()`（绿）
- [ ] 重构：提取 `execute_exclusion` 方法

### **第 4 步：数据库集成（~45 分钟）**
- [ ] 测试：`test_exclusion_writes_to_db`（红）
- [ ] 实现：`execute_exclusion` 调用 database（绿）
- [ ] 测试：`test_remove_cleans_db`（红）
- [ ] 实现：`handle_remove` 调用 `database.delete_record`（绿）

### **第 5 步：规则匹配（~30 分钟）**
- [ ] 测试：`test_only_matched_dirs_excluded`（红）
- [ ] 实现：在 `handle_create` 中调用 `rule_matcher.should_exclude`（绿）
- [ ] 测试：`test_ignore_already_recorded`（红）
- [ ] 实现：检查 `database.is_recorded` 后跳过（绿）

### **第 6 步：CLI 命令（~45 分钟）**
- [ ] 添加 `watch` 子命令到 clap
- [ ] 实现 `handle_watch` 函数
- [ ] 手动测试：`cargo run -- watch /tmp/test`
- [ ] 测试 Ctrl+C 优雅退出（tokio signal）

### **第 7 步：错误处理（~30 分钟）**
- [ ] 测试：监控不存在路径返回错误
- [ ] 测试：监控文件（非目录）返回错误
- [ ] 添加日志输出（println! 或 log crate）

---

## ⚠️ 已知风险与降级方案

### **风险 1：notify 在测试中不稳定**
- **症状**：CI 中 FSEvents 偶尔漏事件
- **降级**：将 P2 的集成测试标记为 `#[ignore]`，仅本地手动验证

### **风险 2：虚拟时间与真实异步任务冲突**
- **症状**：`tokio::time::advance` 后任务未执行
- **解决**：在 `advance` 后加 `tokio::task::yield_now().await`

### **风险 3：5 秒太长导致误删**
- **需求澄清**：是否需要可配置？
- **临时方案**：硬编码 5 秒，后续迭代加配置

---

## ✅ 完成定义（DoD）

- [ ] 所有 P0/P1 测试通过（绿色）
- [ ] `cargo test` 耗时 < 10 秒（虚拟时间加速）
- [ ] `cargo run -- watch /tmp/test` 可手动验证
- [ ] 代码覆盖率 > 80%（cargo-llvm-cov）
- [ ] 无 clippy 警告
- [ ] 更新 `src/lib.rs` 导出 `Watcher`

---

## 🔄 与已完成模块的集成点

| 已有模块 | 集成方式 |
|---------|---------|
| `Database` | Watcher 持有 Database，调用 `record_exclusion` / `delete_record` |
| `RuleMatcher` | Watcher 持有 RuleMatcher，过滤事件 |
| `TmUtilTrait` | Watcher 持有 `Arc<dyn TmUtilTrait>`（支持并发） |
| `Scanner` | 独立运行，不与 Watcher 交互（未来可选：watch 启动前先 scan 一次） |

---

## 📝 备注

- **异步测试复杂度较高**：预计 70% 时间在写测试，30% 在实现
- **虚拟时间是关键**：务必先验证 `tokio::time::pause()` 工作正常
- **不要过度 Mock**：仅 Mock `TmUtilTrait`，Database 用真实内存实例
- **Tracer Bullet 优先**：第 1-3 步完成后应有可运行的最小原型
