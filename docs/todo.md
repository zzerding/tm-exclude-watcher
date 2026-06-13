<!-- ABOUTME: 记录当前 issue 实现状态和剩余发布前待办。 -->

# 当前待办状态

最后更新：2026-06-12

## 已完成

- [x] Issue #1：手动扫描与排除能力。
- [x] Issue #2：查看和手动清理，包含失效记录清理、大小刷新和状态漂移修正。
- [x] Issue #3：实时目录监控，包含 `watch` / `watch_multiple`、延迟确认、删除取消和多路径监控。
- [x] Issue #4：守护进程生命周期已重构为 launchd 托管。
- [x] Issue #5：日志和可观测性，包含 tracing 初始化、CLI/daemon 日志分流和关键操作日志。
- [x] 根目录遗留计划文档已清理，长期文档已归类到 `docs/`。
- [x] 项目本地 skill 已移动到 `skills/stacked-issue-pr-workflow/`。

## 剩余待办

- [ ] Issue #6：端到端测试与发布准备。
  - 需要真实 macOS/Time Machine 环境验证 `daemon start` / `daemon stop` / `daemon status`。
  - 需要验证 LaunchAgent plist、登录自启、异常重启和日志写入。
  - 需要补齐 `.gitignore` 发布项；当前只包含 `/target`。
  - README 已有基础使用说明，但发布前仍需按最终行为复查。

## 当前文档规则

- 根目录只保留入口级文档。
- 已实现的计划和修复总结应清理，不作为长期文档保留。
- 长期产品、领域、issue 和 ADR 文档放在 `docs/`。
