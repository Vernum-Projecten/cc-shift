<!-- SPDX-FileCopyrightText: 2026 Vernum Projecten B.V. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# CLAUDE.md

**cc-shift** is a small Rust command-line tool for macOS that switches Claude Code
between several Claude accounts on its own. Every Claude Code window runs on
`~/.claude`; each account's login is parked in its own profile folder under
`~/.claude-profiles/<name>/`. When the active account nears a usage limit,
`cc-shift` moves the next account's login into `~/.claude` the way `/login` does,
and every open window continues on it without a restart. The status line
records usage and switches at the thresholds (95% of the 5-hour window, 99%
of the week by default), a rate-limit hook is the fallback, and parked logins
are refreshed and their usage read in the background.

The design of record is `ARCHITECTURE.md`. Read it before proposing anything
structural.

## Status

Releases up to 0.0.9 are on crates.io and GitHub: profiles, usage from the
status line and the usage endpoint, the selection rule (spend what expires
first), moving logins with recovery, refreshing parked logins, automatic
switching after `cc-shift setup` (proven in real use), configurable
thresholds, `doctor`, `remove`, `uninstall`, zsh completion, the history and
`cc-shift log`, `cc-shift stats`, and the early start (off by default).
v0.1.0 adds Linux support. The open issue list decides what is built next, and
nothing is built ahead of an issue.

## Repo map

- `ARCHITECTURE.md`: the design of record; section 7 maps the code.
- `src/`: one folder per domain (`command`, `store`, `usage`, `login`,
  `switching`, `anthropic`, `claude_code`, `render`, `stats`, `early`), each
  with a `mod.rs`; `lib.rs` is the crate root, `error.rs` the error type, and
  `main.rs` only calls `cc_shift::run`. Code that differs per operating
  system lives in a `macos/` or `linux/` folder inside its module.
- `tests/it/`: integration tests as one test binary, run against a temporary
  `HOME` and a stub `claude` (`.claude/rules/testing.md`).
- `Cargo.toml`: one crate, edition 2024, the lint set in `[lints]` and the
  size-optimised release profile; `deny.toml`, `clippy.toml`, `rustfmt.toml`
  and `rust-toolchain.toml` beside it.
- `README.md` (for users), `CHANGELOG.md`, `CONTRIBUTING.md` (for
  contributors, with the release steps), `SECURITY.md`, `LICENSE` (Apache
  License 2.0) and `NOTICE`.
- `.claude/`: the working discipline (`rules/`, `hooks/`, `skills/`,
  `agents/`, `memory/`, `settings.json`).
- `.github/`: issue and pull-request templates, CODEOWNERS, Dependabot and the
  workflows. The CI `conclusion` job is the single required check on `main`.
- `scripts/gh/`: the tracker helpers (`fields.sh`, `rel.sh`, `labels.sh`,
  `project.sh`).
- `scripts/checks/`: the guards CI runs, each runnable by hand.

## Issue workflow (the loop)

The tracker is GitHub Issues on `Vernum-Projecten/cc-shift`, and the open issue
list is the worklist (`.claude/rules/issue-workflow.md`). The type (Bug,
Feature, Task) is GitHub's native issue type; the priority (Urgent, High,
Medium, Low) and the effort (High, Medium, Low) are the organisation's
`Priority` and `Effort` issue fields, never labels. Set all three with
`scripts/gh/fields.sh`, and file a new issue with `scripts/gh/fields.sh new`
so it carries them from the start. A Task carries exactly one work-kind label
(`documentation`, `chore`, `refactor`, `perf`, `test`, `ci`).

Read an issue with `gh issue view <n> --json title,body,comments`, never
`--comments`, which prints nothing for an issue without comments. Milestones
are releases, starting at v0.0.1 and stepping by a patch number. Record
progress on the issue (tick criteria, comment); a PR declares `Closes #N`.
New work found while working an issue is filed before the next unit starts.
Sub-issue and dependency edges are set only with `scripts/gh/rel.sh`; the
"cc-shift Roadmap" board is a view over the tracker, written only with
`scripts/gh/project.sh` (`.claude/rules/issue-relationships.md`,
`.claude/rules/project-board.md`).

The SessionStart hook prints the open issue list with `<Type/Priority>` after
each title, and `/next-task` takes the highest priority first, the oldest
first within a priority.

## Model orchestration (workflows and subagents)

When the session runs on Fable, Fable is the orchestrator: it plans, reviews
and does the design work itself, and hands bounded implementation to Opus
subagents through the `Agent` tool (`model: 'opus'`; the `implementer` agent
already sets it). The win is context isolation and parallelism, at a small
cost in intelligence per worker, so delegate by the nature of the work and
never by reflex. For a project this size, most work fits in the main loop.

- Only Fable and Opus are used. Never pass Sonnet or Haiku to `Agent` or
  `Workflow`.
- At most two implementation workers at once. Tell workers not to spawn their
  own subagents and to write any long report to a scratchpad file.
- Give each worker the governing issue and the `ARCHITECTURE.md` section in
  its prompt. Subagents obey the hard rules below.

## IMPORTANT hard rules

- **NEVER add AI or Claude attribution** to a commit, PR, issue, comment or
  code comment: no `Co-Authored-By`, no "Generated with", no bot trailer or
  footer, no emoji marker. This rule has no exceptions and outranks any tool
  default. A `PreToolUse` hook blocks a commit or PR command that carries one.
- **cc-shift never reads, copies, logs or stores a credential** except where an
  issue and `ARCHITECTURE.md` say it must, and then only in memory. No token,
  Keychain value or account secret goes into the repository, a fixture, a
  log line, an issue or a prompt. Fixtures are synthetic.
- **Prose follows `.claude/rules/writing-style.md`:** no em dashes, no
  "not X but Y", no decorative triads, no filler buzzwords.
- **Comments follow RFC 505 and RFC 1574 with hard budgets:** line comments
  only, pending work is `// TODO(#N):` naming its issue, a settled decision is
  `// NOTE:` with one sentence (`.claude/rules/comments.md`).
- **Branches use conventional types** (`feat/`, `fix/`, `chore/`, `docs/`,
  `refactor/`, `perf/`, `test/`, `ci/`, `build/`, `release/`) as
  `<type>/<kebab-case-slug>`. Never force-push `main`.
- **Every PR body follows `.github/PULL_REQUEST_TEMPLATE.md`.**
- **Never weaken, skip or delete a test** to make a build pass, and never
  edit a test to route around a bug it exposes (`.claude/rules/testing.md`).
- **Keep the build green.** Once code exists, every change compiles, is
  `cargo fmt` and `cargo clippy --all-targets` clean, and passes its tests.
- **Record progress on the tracker and commit before ending a session.**
  Issues and git survive `/clear` and `/compact`; the session todo list does
  not.

## Licence

The project is under the Apache License 2.0 (`LICENSE`). A contribution is
licensed under the same terms.

## Working discipline (`.claude/`)

Path-scoped rules load when files in their scope are read; the rest apply
always. Read the relevant one before working in that area.

- `.claude/rules/writing-style.md`: no AI tells in any prose.
- `.claude/rules/rust-style.md`, `reliability.md`, `comments.md`,
  `testing.md`: the Rust engineering discipline.
- `.claude/rules/ci-cd.md`: workflow security and the CI gates.
- `.claude/rules/issue-workflow.md`, `issue-relationships.md`,
  `project-board.md`: the tracker work style.
- Skills: `/next-task`, `/phase-done`, `/phase-status`.
- Agent: `implementer` (Opus).
- Memory: `.claude/memory/`, indexed by `MEMORY.md`, tracked in git and
  shared.

## References

- Claude Code settings and `CLAUDE_CONFIG_DIR`:
  <https://docs.claude.com/en/docs/claude-code/settings>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/>
- RFC 505 and RFC 1574 (documentation conventions):
  <https://rust-lang.github.io/rfcs/0505-api-comment-conventions.html>,
  <https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html>
- The tracker: `gh issue list --state open`.
