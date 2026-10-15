---
name: deps-latest-sweep
description: "At the start of a work session and before a release cut, compare the Cargo.toml pins with their latest crates.io releases and check Claude Code's changelog for changes cs relies on"
metadata:
  type: feedback
---

The owner wants dependencies kept at their latest releases rather than left
to drift until Dependabot catches up.

**Why:** carried from the owner's other Rust projects. For `cs` the larger
risk is upstream: `cs` relies on Claude Code's `CLAUDE_CONFIG_DIR` handling,
its per-folder Keychain items, and the usage and reset information automatic
selection reads, and a Claude Code release can change any of them.

**How to apply:** at the start of a work session and before a release cut,
compare every `[dependencies]` pin with crates.io (`max_stable_version`, with
a neutral User-Agent, [[no-personal-data-in-requests]]) and read Claude Code's
changelog since the last check for changes to config folders, login storage,
or usage reporting. A change that affects `cs` becomes an issue.
