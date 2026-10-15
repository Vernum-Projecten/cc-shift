---
name: native-issue-types-and-priority
description: "Type, priority and effort are GitHub's native issue type and the Vernum-Projecten Priority and Effort issue fields, set with scripts/gh/fields.sh, never labels; carried from the owner's FerroFED setup (2026-10-15)"
metadata:
  type: project
---

`cs` tracks the kind and urgency of its work with the native issue type
(`Bug`, `Feature`, `Task`), the organisation's `Priority` field (`Urgent`,
`High`, `Medium`, `Low`) and its `Effort` field (`High`, `Medium`, `Low`).
The `Vernum-Projecten` organisation carries all three (read on 2026-10-15).

**Why:** labels for type and priority duplicate what GitHub models natively,
and the board and the issue list filter on the fields.

**How to apply:**

- File every issue with `scripts/gh/fields.sh new <type> <priority> <effort>
  <gh issue create args…>`; set or change one with `fields.sh type|priority|
  effort <n> <value>`; read with `fields.sh show <n>`.
- A Task carries exactly one work-kind label (`documentation`, `chore`,
  `refactor`, `perf`, `test`, `ci`); a Bug or a Feature carries none.
- Read an issue with `gh issue view <n> --json title,body,comments`; on gh
  2.101.0 `--comments` prints nothing for an issue without comments.
- Related: [[pr-auto-merge]].
