<!-- SPDX-FileCopyrightText: 2026 Vernum Projecten B.V. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

## What changed and why

<!-- Describe the change and the reason for it. Keep it to what a reviewer needs. -->

Closes #NNN

## Checklist

- [ ] Decisions that rest on outside behaviour cite official documentation (Claude Code, Apple, the Rust reference, a crate's docs.rs page), not memory.
- [ ] Shell and workflow files are clean: `shellcheck --severity=style`, `actionlint`, `zizmor --min-severity=low .github/`.
- [ ] The committed guards pass: `scripts/checks/comment-style.sh --all`, `scripts/checks/file-length.sh`, `scripts/checks/bash-compat.sh`, `scripts/checks/script-modes.sh`, and the `--self-test` of every `scripts/` program the change touched.
- [ ] Rust gates pass: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo nextest run --locked`, `cargo test --doc --locked`, `cargo doc` with `RUSTDOCFLAGS=-D warnings`, and `cargo deny check`.
- [ ] Docs are updated, if behaviour changed.
- [ ] No token, Keychain item, account email or profile content in a fixture, test, or example.
- [ ] Every commit is signed.
- [ ] No AI or assistant attribution anywhere in the commits or this PR.

See [CONTRIBUTING.md](../CONTRIBUTING.md) for the full contribution guide.
