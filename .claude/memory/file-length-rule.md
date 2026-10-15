---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
name: file-length-rule
description: "A hand-written Rust file is at most 1000 lines and is split into a module folder at 750; owner rule, in force here from the first .rs file"
metadata:
  type: feedback
---

A hand-written Rust file is at most 1000 lines, and one over 750 is split into
a module folder (`<name>/mod.rs` plus one child per concern) before it grows.

**Why:** on another of the owner's projects one file reached 4600 lines; a
file that size cannot be reviewed, and every worker that touched it grew it
further.

**How to apply:** `scripts/checks/file-length.sh` runs in CI, and no file is
ever added to an allow-list here. A split moves code only; a defect found on
the way is filed. The full rule is `.claude/rules/rust-style.md` §File and
module size.
