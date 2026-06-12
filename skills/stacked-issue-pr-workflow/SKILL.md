---
name: stacked-issue-pr-workflow
description: Use when implementing a GitHub issue as a stacked pull request with an issue plan comment, subagent TDD planning, optional plan review, delegated implementation, standards/spec review, verification, commits, PR creation/update, and issue progress comments. Trigger for requests like "pick/finish an issue", "make a stacked PR", "use subagents to plan/review/implement", or "update the issue after the PR".
---

# Stacked Issue PR Workflow

Run a small-to-medium GitHub issue through a disciplined stacked-PR loop: inspect current state, plan publicly, delegate where useful, implement with tests, review against standards and spec, then commit, push, open/update PR, and update the issue.

## Core Rules

- Read project-local `AGENT.md` / `agent.md` first when present.
- Treat current worktree, GitHub issue, and PR state as authoritative.
- Never stage unrelated worktree changes.
- Prefer stacked branches when earlier PRs are still open; base the new branch on the previous PR head. When no earlier PRs are open, base on main.
- Every subagent prompt must include the pinned state: issue number, branch name, stack base ref, diff baseline, and unrelated dirty files to ignore. Subagents do their own deep reading; the parent passes coordinates and constraints, not file contents.
- Use subagents only for the specific delegated roles the user asked for or where parallel review reduces risk.
- Do not close the issue unless the issue scope is complete and the relevant PRs are merged.
- Keep user-facing progress short, but include concrete branch/PR/verification facts.
- Stacked PRs require non-squash merge. If the repository uses squash-merge by default, warn the user before creating the stack.

## Workflow

1. **Pin state**
   - Run `git status --short --branch`, `git log --oneline --decorate -8`, and inspect open PRs/issues with `gh`.
   - Read the issue body and comments, not only the title.
   - Identify previous branch/PR if the work must be stacked.
   - Record unrelated dirty files and leave them alone.

2. **TDD plan subagent**
   - Spawn one read-only planning subagent.
   - Pass pinned state: issue number, current branch, stack base, unrelated dirty files.
   - Ask it to read the issue, current code, tests, domain docs, and return:
     - observable requirements,
     - vertical TDD slices,
     - files likely to change,
     - risks and scope boundaries.

3. **Plan review subagent (conditional)**
   - Only when the plan involves schema/API changes, architectural choices, or touches >5 files.
   - Otherwise skip — the issue plan comment (step 4) is the review gate.
   - If triggered: spawn one read-only reviewer for the plan.
   - Ask whether the plan satisfies the issue/spec without over-design.
   - Fold concrete findings into the plan before coding.

4. **Branch and issue plan**
   - Create the stacked branch from the correct base branch.
   - Show the plan comment to the user before posting to the GitHub issue.
   - Comment the implementation plan on the GitHub issue before coding.
   - Include assumptions, vertical slices, expected files, and explicit non-goals.

5. **Implementation subagent**
   - Delegate implementation to one worker when requested or useful.
   - Pass pinned state and scope boundaries.
   - Give it a bounded write scope and tell it not to commit, push, or touch unrelated files.
   - Require tests-first where practical and at least one full verification run.
   - Parent agent reviews the diff the worker produced (`git diff`) before proceeding.

6. **Local verification**
   - Read the diff including untracked files.
   - Run the repo's standard gates. Typical Rust gates:
     - `cargo fmt -- --check`
     - `cargo test`
     - `cargo clippy --all-targets -- -D warnings`
   - Fix failures; never bypass hooks or verification.

7. **Dual review pass**
   - Review against the fixed point for this PR, usually the stacked base branch.
   - If work is uncommitted, review `git diff --cached` after staging the intended files.
   - Run two independent subagents:
     - Standards: documented repo/project/user rules.
     - Spec: issue body/comments or PRD.
   - Pass pinned state (stack base ref, issue number) to both reviewers.
   - Fix real findings, add tests when behavior changed, rerun gates, and rerun targeted review if needed.

8. **Commit and PR**
   - Stage only intended files.
   - Run `git diff --cached --check`, inspect staged stat, and check recent commits.
   - Commit with a conventional message.
   - Push the branch.
   - Open or update the PR with:
     - summary,
     - verification commands/results,
     - review outcome,
     - issue reference,
     - stacked base note when applicable.

9. **Issue update and completion audit**
   - Comment on the issue with what was completed, verification, PR link, and whether the issue remains open.
   - Confirm branch tracking, remote SHA, PR base/head, PR state, merge state, and issue state.
   - Leave the issue open if any stacked PR is still unmerged or scope remains.

## Stack Maintenance

When a prior PR in the stack is merged or updated:

1. **After base PR merge**
   - The merged PR's branch is typically deleted by GitHub.
   - Rebase current branch onto main: `git rebase --onto main <old-base-branch-name>`
   - Retarget the PR base to main via `gh pr edit <pr-number> --base main`
   - Verify the PR diff still shows only the current PR's changes.

2. **After base PR code changes**
   - If the base PR was amended during review, rebase current branch onto the updated base.
   - Run full verification gates again to catch integration issues.
   - Push with `--force-with-lease` if the rebase rewrote history.

3. **Conflict resolution**
   - Conflicts during rebase indicate overlapping changes between stack layers.
   - Resolve carefully and verify tests still pass after resolution.
   - Never skip verification after conflict resolution.

## Review Prompts

Use concise prompts. For Standards review, include:

```text
Read standards docs, then read the diff. Report documented-standard violations only.
Distinguish hard violations from judgment calls. Skip fmt/clippy mechanical items.
```

For Spec review, include:

```text
Read the issue/spec, then read the diff. Report missing/partial requirements,
scope creep, and implemented behavior that appears wrong.
Quote issue/spec evidence for each finding.
```

## Completion Checklist

- Issue/spec requirements mapped to evidence.
- Tests cover the risky behavior, not just internal shape.
- Standard gates pass.
- Standards review has no unresolved findings.
- Spec review has no unresolved findings.
- Commit is pushed to the expected remote branch.
- PR base/head are correct for the stack.
- Issue has a plan comment and a completion/progress comment.
- No unrelated dirty files were staged or reverted.
