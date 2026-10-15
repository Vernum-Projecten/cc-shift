---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
name: owner-work-style
description: How the owner wants work done (evidence before foundational choices, no scaffolding while a design is open, ask when a decision is theirs, pause when asked); carried from the owner's FerroFED setup (2026-10-15)
metadata:
  type: feedback
---

The owner decides foundational choices from evidence, not convention, and
takes the first design seriously because it is the hardest part to change
later.

**Why:** carried from the owner's other Rust projects, where an early scaffold
was stopped because the design was still open.

**How to apply:**

- For any foundational choice, put the evidence and the options in front of
  the owner with a recommendation before building.
- Do not scaffold code while the design is open. `ARCHITECTURE.md` is the
  design; each piece of behaviour gets its own issue, and nothing is built
  ahead of one.
- Take the owner's design intuitions seriously and test them against
  evidence; reconcile rather than dismiss.
- Pure Rust, small, single binary, no resident daemon are standing
  constraints. Short-lived background work the owner approved is fine: the
  detached `cc-shift refresh` and the early start run by `launchd`.
- **Pause when asked.** A pause names a boundary: the work up to it finishes
  (merges, cleanup, the report) and nothing new starts until the owner writes
  again.
- Ask the owner when a decision is theirs.
