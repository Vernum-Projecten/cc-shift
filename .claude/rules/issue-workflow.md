# Issue workflow (the tracker loop)

**The tracker is GitHub Issues on `Vernum-Projecten/cs`: the open issue list
IS the worklist.** Issue state is edited only via `gh`; never track work only
in chat. This file is the loop, the label taxonomy, and the cadence.
Relationships between issues live in `issue-relationships.md`; the board is
`project-board.md`.

## The loop

1. **Orient.** `gh issue list --state open` (the SessionStart hook injects it,
   annotated with each issue's sub-issue progress `{k/n}`, `child-of #parent`,
   and open `BLOCKED-by`/`blocks` edges). Pick the pinned issue (pins are the
   current focus, max 3) or the issue the user names; **skip an issue shown
   `BLOCKED-by` an open issue** (work its blocker first) and prefer the next
   open child of a parent. Among the candidates, the `Priority` field decides
   the order (`Urgent`, `High`, `Medium`, `Low`) and the oldest issue goes
   first within a priority. Read the contract with the `--json` form below
   and its relationship graph with `scripts/gh/rel.sh tree <n>`.

   **Never read an issue with `gh issue view <n> --comments`.** On gh 2.101.0
   it prints nothing at all and exits 0 for an issue with no comments, so the
   contract comes back empty and nothing says so. Every reader of an issue
   uses:

   ```sh
   gh issue view <n> --json title,body,comments \
     --jq '.title, .body, (.comments[] | "--- comment ---", .body)'
   ```
2. **Read the contract.** The issue body opens with a plain summary (no
   heading: what and why, decisions, citations), followed by an
   `## Acceptance criteria` checklist and an optional `## Tasks` task list. New
   work discovered en route gets its own issue (`scripts/gh/fields.sh new`)
   that is then **linked** with `scripts/gh/rel.sh`, as a sub-issue of the
   issue it decomposes or a `blocked-by`/`blocking` dependency for real
   sequencing, never a prose "see also" deferral.
3. **Do the work.** At pickup, move the issue to `In Progress` on the board
   (`scripts/gh/project.sh status <n> in-progress`). Read `ARCHITECTURE.md`
   first; a change that alters the design updates it in the same PR. Build
   compiling, tested increments.
4. **Record progress on the issue.** Tick verified acceptance-criteria
   checkboxes (`gh issue edit <n>`), and post substantive status or decisions
   as comments (`gh issue comment <n>`); the issue thread is the durable
   record.
5. **Commit on a conventional-type branch** (see the branch rule below) with a
   descriptive subject; the PR body declares `Closes #<n>` so the merge into
   `main` auto-closes the issue (never close by hand when a PR carries the
   work). One `Closes` keyword closes one issue: "Closes #1, #2" closes only
   #1, so repeat the keyword per issue and verify after the merge. Arm
   auto-merge the moment the PR is open, as its own command
   (`gh pr merge <n> --auto`, `.claude/memory/pr-auto-merge.md`).
6. **Close out** with `/phase-done`: verify the acceptance criteria are met,
   write the close narrative into the PR description, and post the handoff
   comment on the issue.

## Type, priority and labels

**The type, the priority and the effort of an issue are GitHub's own fields,
never labels.** The `Vernum-Projecten` organisation defines all three, and
`scripts/gh/fields.sh` sets each by issue number, resolving every node id and
failing loud on a typo:

| Fact | Where it lives | Values | Command |
|---|---|---|---|
| Type | the native issue type of the organisation | `Bug`, `Feature`, `Task` | `scripts/gh/fields.sh type <n> bug\|feature\|task` |
| Priority | the organisation's `Priority` issue field | `Urgent`, `High`, `Medium`, `Low` | `scripts/gh/fields.sh priority <n> urgent\|high\|medium\|low` |
| Effort | the organisation's `Effort` issue field | `High`, `Medium`, `Low` | `scripts/gh/fields.sh effort <n> high\|medium\|low` |

`scripts/gh/fields.sh show <n>` prints all three with the labels and the
milestone. `scripts/gh/fields.sh new <type> <priority> <effort> <gh issue
create args…>` creates an issue that carries all three from its first
second, so every issue is filed through it, never through a bare `gh issue
create`. The SessionStart dump and `/phase-status` print type and priority as
`<Type/Priority>` after the title.

**An issue filed without the three** (a scheduled workflow whose token cannot
read the organisation's issue types, or a person using the web form) is
completed by whoever picks it up: set the type, the priority, the effort and
the milestone before doing anything else with it.

The priorities read: `Urgent` is drop everything, `High` is the current
focus, `Medium` is normal, `Low` is the backlog. Effort is the size of the
work as filed: `Low` is one sitting (a guard, a one-file fix), `Medium` is one
pull request that touches several modules or needs new test fixtures, `High`
is more than one pull request or a design question. It is set at filing and
re-set when the work turns out bigger; it never changes the order of the
worklist, which the priority and the age of the issue decide.

The type maps to the conventional-commit type of the branch and the commit: a
`Bug` is a `fix`, a `Feature` is a `feat`, and a `Task` says which one it is
with exactly ONE work-kind label.

Labels carry what the platform has no field for. Bootstrap them once with
`scripts/gh/labels.sh`:

- **Work kind, on a `Task` only, exactly ONE:** `documentation`, `chore`,
  `refactor`, `perf`, `test`, `ci`. A `Bug` and a `Feature` carry none,
  because the type already names the commit type.
- **Area:** `profiles` (creating, listing and launching profiles), `usage`
  (reading remaining usage and reset times), `selection` (automatic choice
  and switching), `keychain` (anything that touches login state). Add more as
  the project grows; keep the set small and meaningful.
- **Workflow:** `security`, `dependencies` (Dependabot), `blocked-upstream`
  (waits on a Claude Code release or another upstream change).

## Milestones = releases

A milestone is a delivery promise (`vX.Y.Z`, starting at `v0.0.1` and stepping
by the patch number, `.claude/memory/milestones-0-0-x.md`). A release is cut
when its milestone reaches zero open issues (or the owner calls the cut and
moves the stragglers to the next milestone). Every en-route issue goes in the
CURRENT milestone, never the next. A milestone with a due date feeds the
board's Roadmap view (`scripts/gh/project.sh sync-dates`).

## The fix-first cadence

**An issue filed while working a unit is FIXED before the next unit starts.**
Review findings, guard gaps, and en-route defects: filing the issue is the
record, never permission to move on. Tractable findings are fixed in the same
branch; genuinely separable work becomes a linked issue that is closed before
the current work advances.

## Branches use conventional types

`<type>/<kebab-case-slug>` with `type` in `feat`, `fix`, `chore`, `docs`,
`refactor`, `perf`, `test`, `ci`, `build`, `release` (the Conventional Commits
type set): for example `feat/auto-select-profile`, `fix/reset-time-parse`.
Pick the type by the dominant change; an issue's branch mirrors the issue's
type (a `Feature` is `feat/`) or, for a `Task`, its work-kind label. Never
force-push `main`.

## Durable versus session-scoped

Issues and git survive `/clear` and `/compact`; the built-in todo tool is
session-scoped, so the tracker is the durable layer. Record progress (tick
checkboxes, comment, open issues for new work) and commit before ending a
session.

## Never add AI or Claude attribution

Commit messages, PR text, issue and comment bodies describe only the change
itself: no `Co-Authored-By: Claude`, no "Generated with", no bot trailer or
footer, ever. This is an absolute rule with no exceptions.
