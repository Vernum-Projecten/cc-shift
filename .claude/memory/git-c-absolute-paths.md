---
name: git-c-absolute-paths
description: "A cd inside a compound Bash command does not reliably set the working directory for git in this harness; use git -C <abs path> and absolute paths for every worktree operation; carried from the owner's FerroFED setup (2026-10-15)"
metadata:
  type: feedback
---

On another of the owner's projects a `cd <worktree> && git checkout -B
<branch>` ran in a different worktree than the one named, and the follow-up
force push overwrote a sibling pull request's remote branch.

**Why:** the Bash tool's working directory persists and is reported one call
late, so a compound command's `cd` and the harness's idea of the cwd disagree.

**How to apply:** every git call on a worktree uses `git -C /abs/path ...`,
file edits use absolute paths, and a force push is preceded by
`git -C <path> rev-parse --abbrev-ref HEAD` in the same command. See
[[auto-merge-follow-ups]] and [[clean-up-agent-worktrees]].
