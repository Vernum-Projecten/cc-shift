---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
name: lint-bar-parity
description: "The lint bar is the owner's strict family bar: clippy all and pedantic at deny, the strict restriction set, unwrap/expect/panic denied in application code and allowed in tests only"
metadata:
  type: feedback
---

The owner wants a very strict code style in every Rust project, including
this small one.

**Why:** on another project the owner found the lint table looser than the
family's and asked for it to be fixed "immediately".

**How to apply:** the root `Cargo.toml` `[lints]` table, added with the crate,
carries the family bar: `clippy::all` and `pedantic` at `deny`,
`as_conversions`, `pub_use`, `dead_code`, `missing_assert_message`,
`map_err_ignore`, `unused_qualifications`, `non_ascii_idents = forbid`,
`unsafe_code = forbid`, and the deny-tier set `.claude/rules/reliability.md`
names. `clippy.toml` relaxes the panicking lints inside tests only. When a
lint fights a legitimate case, use a scoped `#[expect(lint, reason = "...")]`.
See [[gates-on-host]].
