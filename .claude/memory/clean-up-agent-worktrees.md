---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
name: clean-up-agent-worktrees
description: "Remove a subagent's worktree and its branches as soon as its PR merges and the agent has reported, one at a time, never by looping over .claude/worktrees; carried from the owner's FerroFED setup (2026-10-15)"
metadata:
  type: feedback
---

On another of the owner's projects stale worktrees under `.claude/worktrees/`
piled up, each holding a full `target/` directory, and the owner asked for
cleanup three separate times.

**How to apply:** after an agent completes and its PR is merged:

```
git worktree list
gh pr list --head <branch> --state all --json number,state
git worktree unlock <path>
git worktree remove <path>
git worktree prune
git branch -D <branch> worktree-agent-<id>
```

- Never remove a worktree whose agent is still running, and never loop over
  `.claude/worktrees/*`; a loop cannot tell a live worktree from a finished
  one, and `--force` discards uncommitted work. Remove it only after the
  agent's final hand-back.
- A squash-merged branch fails `git branch -d`; confirm MERGED with `gh`,
  then `-D`. Run the steps separately, not as one compound command.

`.claude/worktrees/` belongs in `.gitignore`. See [[pr-auto-merge]] and
[[git-c-absolute-paths]].
