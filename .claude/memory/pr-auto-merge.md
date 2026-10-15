---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
name: pr-auto-merge
description: "Every pull request gets auto-merge armed the moment it is opened (gh pr merge <n> --auto); clean up local branches after each merge and branch from origin/HEAD; carried from the owner's FerroFED setup (2026-10-15)"
metadata:
  type: feedback
---

After opening a pull request, arm auto-merge at once, as its own command:

```sh
gh pr merge <n> --auto
```

Never leave a pull request waiting for a manual merge.

**Why:** the owner wants a green pull request to merge without a hand-off;
the `main` ruleset requires the `conclusion` check, so auto-merge is the
hand-off.

**How to apply:**

- Auto-merge needs "Allow auto-merge" in the repository settings and a
  required check on `main`. Until `ci.yml` reports `conclusion`, an armed
  auto-merge waits; say so in the hand-off. Never use `--admin` without the
  owner asking.
- After every merge: `git fetch --prune`, delete each local branch whose
  upstream is `[gone]`, keep only `main` and branches with an open pull
  request. Push every branch the moment it has a commit.
- Branch from `origin/HEAD`, never from a local `main`:
  `git fetch origin && git checkout -b <type>/<slug> origin/HEAD`. Stage named
  paths (`git add <files>`), never `git add -A` from the root.
- Never push a follow-up commit to an armed PR ([[auto-merge-follow-ups]]).
- `.claude/settings.json` allows `gh pr merge * --auto` only; a plain
  immediate merge stays the owner's call, and `--admin`, `-R` and `--repo`
  are denied.
