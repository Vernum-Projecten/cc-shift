<!-- SPDX-FileCopyrightText: 2026 Vernum Projecten B.V. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Memory index

Tracked project memory, shared by every session and contributor
([memory-lives-in-repo](memory-lives-in-repo.md)). One line per file.

## Working style

- [owner-work-style](owner-work-style.md): evidence before foundational choices, no scaffolding while the design is open, ask, pause when asked
- [memory-lives-in-repo](memory-lives-in-repo.md): learnings go in `.claude/memory/`, never a per-user directory
- [subagent-reports-to-file](subagent-reports-to-file.md): a subagent writes a long report to a scratchpad file
- [no-personal-data-in-requests](no-personal-data-in-requests.md): no email address or contact in any external request

## Tracker and releases

- [native-issue-types-and-priority](native-issue-types-and-priority.md): type, priority and effort are native fields set with `scripts/gh/fields.sh`
- [milestones-0-0-x](milestones-0-0-x.md): milestones start at v0.0.1 and step by the patch number
- [cut-releases-promptly](cut-releases-promptly.md): cut when the code work is done; move stragglers
- [release-tag-is-mine](release-tag-is-mine.md): the session tags and pushes every release

## Git and pull requests

- [pr-auto-merge](pr-auto-merge.md): arm auto-merge on open; branch from `origin/HEAD`; prune after merge
- [auto-merge-follow-ups](auto-merge-follow-ups.md): never push more to an armed PR
- [git-c-absolute-paths](git-c-absolute-paths.md): `git -C <abs path>` for every worktree operation
- [clean-up-agent-worktrees](clean-up-agent-worktrees.md): remove an agent's worktree after its PR merges, one at a time

## Code and gates

- [lint-bar-parity](lint-bar-parity.md): the strict family lint bar in the root `Cargo.toml`
- [file-length-rule](file-length-rule.md): at most 1000 lines per Rust file, split at 750
- [gates-on-host](gates-on-host.md): gates run on the host; Gatekeeper stalls a first test run
- [deps-latest-sweep](deps-latest-sweep.md): check crate pins and Claude Code's changelog each session
- [perl-edit-pitfalls](perl-edit-pitfalls.md): heredoc literals for perl edits; rustfmt reflows targets
- [mermaid-diagrams](mermaid-diagrams.md): render every mermaid fence before a PR
