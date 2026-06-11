# tm-watcher 实现待办

## 阶段 1：基础设施（数据层）
- [x] Prompt 1: 初始化项目和配置模块
- [x] Prompt 2: 规则匹配引擎
- [x] Prompt 3: 数据库模块
- [x] Prompt 4: tmutil 包装器

## 阶段 2：核心操作（业务层）
- [x] Prompt 5: 目录扫描器
- [x] Prompt 6: 清理器

## 阶段 3：监控与自动化（服务层）
- [ ] Prompt 7: 文件系统监控器
- [ ] Prompt 8: 守护进程管理

## 阶段 4：CLI 接口（应用层）
- [ ] Prompt 9: CLI 命令实现
  - [x] `scan <path>` 手动扫描
  - [x] `list` 查看排除记录
  - [x] `clean` 手动清理
  - [ ] `start` / `stop` / `status` 守护进程命令
- [ ] Prompt 10: 日志系统集成
- [ ] Prompt 11: 最终集成和测试

---

## 当前状态
- **当前步骤：** 准备开始 Issue #3 / Prompt 7: 实时目录监控
- **完成进度：** 6/11 完成，Prompt 9 部分完成（`scan` / `list` / `clean` 已可用）
- **最后更新：** 2026-06-11

## 已完成切片
- ✅ Issue #1 / PR #7: 手动扫描与排除能力（配置、规则、数据库、tmutil 后端、扫描器、`scan <path>`）
- ✅ Issue #2 / PR #8: 扫描热路径优化，数据库已有记录时跳过 `tmutil isexcluded`
- ✅ Issue #2 / PR #9: `list` / `clean`，包含失效记录清理、大小与检查时间刷新、DB/TM 状态漂移修复

## 当前切片：Issue #3 / Prompt 7 实时目录监控
- [ ] 添加 `notify`、`tokio`、`clap` 相关依赖
- [ ] 新增 `src/watcher.rs`，实现前台实时目录监控
- [ ] 支持目录创建事件：匹配规则后延迟确认再执行排除
- [ ] 支持目录删除事件：取消待处理任务并清理数据库记录
- [ ] 新增 `watch <path>` CLI 命令用于手动验证
- [ ] 覆盖核心测试：创建延迟、删除取消、规则过滤、已记录目录跳过
- [ ] 导出 watcher 公共接口并通过 `cargo test`

## 后续切片
- [ ] Issue #4 / Prompt 8: 守护进程模式
  - 阻塞于 Issue #3
  - 包含 `start` / `stop` / `status`、PID 文件、SIGTERM、后台监控与定期清理
- [ ] Issue #5 / Prompt 10: 日志和可观测性
  - 可并行推进
  - 集成 `tracing`，区分 CLI stderr 与 daemon log 文件
- [ ] Issue #6 / Prompt 11: 端到端测试与发布准备
  - 阻塞于 Issue #3 / #4 / #5
  - 包含 README、.gitignore、E2E 脚本、错误场景与性能验证

---

## 注意事项
- 每个 Prompt 完成后标记为已完成
- 遇到问题记录在对应步骤下方
- 保持代码最简，避免过度设计
- 每步完成后验证功能正常再继续
