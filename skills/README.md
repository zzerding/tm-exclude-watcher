<!-- ABOUTME: 说明项目本地 skills 的存放位置与 agent 调用流程。 -->

# 项目 Skills

此目录存放项目本地 skill，作为不同 agent 工具共享的流程说明来源。

## stacked-issue-pr-workflow

路径：`skills/stacked-issue-pr-workflow/SKILL.md`

当 GitHub issue 需要实现为 stacked pull request 时使用此 skill，尤其适用于需要 subagent 规划、TDD 切片、审查、创建 PR、更新 issue 进度的任务。

### 使用流程

1. 明确调用 skill：

   ```text
   Use $stacked-issue-pr-workflow to implement issue #<number>.
   ```

2. 固定当前状态：
   - 如果存在项目 `AGENT.md` / `agent.md`，先读取；
   - 检查 git 状态和近期提交；
   - 检查相关 GitHub issues 和 PRs；
   - 确认 stack base，以及必须忽略的无关脏文件。

3. 让只读规划 subagent 输出可观察需求、垂直 TDD 切片、可能涉及的文件、风险和范围边界。

4. 只有当计划涉及 schema/API 变更、架构选择、不可逆决策或影响文件较多时，才使用计划审查 subagent。

5. 从正确的 base 创建分支：
   - 如果已有更早的 PR 未合并，从该 PR 的 head 分支创建；
   - 否则从 `main` 创建。

6. 编码前先把实施计划发布到 GitHub issue。

7. 只有当委派能降低风险或减少上下文负担时，才把实现交给 worker；worker 不得 commit、push 或触碰无关文件。

8. 本地验证后运行 standards/spec 审查，修复真实问题，只提交预期文件。

9. 创建或更新 PR，写明摘要、验证结果、审查结果、issue 引用和 stack base 说明。

10. 更新 issue，说明进度、PR 链接、验证情况，以及 issue 是否仍需保持打开。

### 规则

- 不在 `.claude/` 或 `.agents/` 下保留重复副本。
- 不保留 `openai.yaml`；此项目 skill 只由 `SKILL.md` 和本 README 说明。
- 永远不要 stage 无关脏文件。
