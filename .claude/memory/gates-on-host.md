---
name: gates-on-host
description: "Every gate, tests included, runs on the host CLI in the working copy, never in a container; a first test run can stall on macOS Gatekeeper"
metadata:
  type: feedback
---

Run the gates in the working copy on the host: the tier-1 guards (`zizmor`,
`actionlint`, `shellcheck`, `scripts/checks/*.sh`) and the Rust set in
`.claude/rules/ci-cd.md`.

**Why:** `cs` is macOS-only and launches a macOS binary, so a Linux container
cannot run its tests faithfully; on another of the owner's projects a
container's target volume reached 104 GB and crashed Docker.

**How to apply:** if a test run seems to hang before anything prints, it is
macOS Gatekeeper checking a fresh unsigned test binary (about four minutes on
first run); wait it out. Tests never touch the real `~/.claude-profiles/` or
the Keychain (`.claude/rules/testing.md`).
