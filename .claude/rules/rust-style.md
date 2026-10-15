---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
paths: ["**/*.rs"]
---

# Rust style: idiomatic application code

Applies to every hand-written `.rs` file in the repository. `cc-shift` is one small
binary crate at the repository root; `ARCHITECTURE.md` is its design. Claude
Code's own documented behaviour (`CLAUDE_CONFIG_DIR`, the login and Keychain
handling) is the authority for anything `cc-shift` relies on; behaviour that no
documentation states is verified by hand and recorded on the issue.

## Build idiomatic, compiling, tested code

- **Use proper crates; do not hand-roll** what the ecosystem provides (JSON,
  time, hashing). Keep the dependency set small: `cc-shift` is a small tool,
  and every dependency has to earn its place in the PR that adds it. The one
  deliberate exception is the command line: `src/command/parse.rs` is a small
  hand-written parser with its own tests, because a parser crate would cost
  more than the few flags it saves.
  Verify each version against crates.io or docs.rs at the moment it is added,
  never from memory.
- **The crate compiles, is clippy-clean, and is tested before you move on.**
- **Never touch credentials.** `cc-shift` never reads, writes, copies or refreshes a
  token or a Keychain item unless an issue the owner accepted says so, and
  then only through the documented API that issue names.

## File and module size

A hand-written Rust file is at most 1000 lines, and a file over 750 lines is
split into a module folder before it grows further. The shape of a split is a
folder `<name>/mod.rs` holding the module doc, the `mod` declarations and the
items every child shares, with one child file per concern; tests split the
same way, one file per subject under `tests/it/<subject>/`. A split never
creates a test file under `src/`: a test of private internals stays an inline
`#[cfg(test)] mod tests` in the child whose internals it reads, and a test of
the public API moves to `tests/it/` (`testing.md` §Where tests live). The
zero-re-export rule stands: every import names the defining module, so a
split never adds `pub use`. A split moves code without changing behaviour:
the same tests pass before and after, and a defect found on the way is filed,
never fixed in the split. Enforced by `scripts/checks/file-length.sh` in CI.

## Comments and documentation

The comment and doc-comment discipline lives in **`comments.md`** (RFC 505 plus
RFC 1574): line comments only, `// TODO(#NNNN):` / `// NOTE:` / `// SAFETY:` as
the only markers, NOTE as a citation plus one sentence (at most 3 lines), `//`
runs at most 8 lines, doc-comment summary-line and section conventions.
Enforced by `scripts/checks/comment-style.sh` (hook) and
`clippy::too_long_first_doc_paragraph`.

## Default values live in the struct's `Default` impl (RFC 3681 shape)

The shape is RFC 3681's
(<https://rust-lang.github.io/rfcs/3681-default-field-values.html>), written by
hand: **one** `impl Default` per struct, every default value inline, and
container-level `#[serde(default)]` so serde fills omitted fields from it.

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SwitchPolicy {
    pub prefer_soonest_reset: bool,
    pub min_remaining_percent: u8,
}

impl Default for SwitchPolicy {
    fn default() -> Self {
        Self {
            prefer_soonest_reset: true,
            min_remaining_percent: 5,
        }
    }
}
```

The RFC's own syntax (`strict: bool = true`) is **nightly-only** (feature
`default_field_values`, tracking issue
<https://github.com/rust-lang/rust/issues/132162>), and this project pins
stable, so the expansion above is the form we write. Do not reach for nightly
to get it.

Three forms are banned, because each one puts a field's default value
somewhere other than the field's own struct:

- **`#[serde(default = "path")]`:** the per-field path form. The default then
  lives in a function, so `Default::default()` and a deserialized value can
  disagree about the same field with no warning. Container-level
  `#[serde(default)]` (no path) is the required form and reads the one
  `Default` impl.
- **`fn default_x() -> T`:** a zero-argument constructor that exists to be one
  field's default.
- **`const DEFAULT_X` with a single reader:** a constant that nothing shares is
  a default value spelled far from its struct.

A `const` with MORE THAN ONE consumer stays a constant and may be referenced
from inside the `Default` impl.

## RFC-grounded conventions

| RFC | The rule | Enforcement |
|---|---|---|
| 0505 + 1574 | comment and doc conventions | `comments.md` + `scripts/checks/comment-style.sh` + `too_long_first_doc_paragraph` |
| 3681 | a field's default lives inline in its `Default` impl | §Default values above |
| 3107 | `#[derive(Default)]` + `#[default]` where the default is a VARIANT | `clippy::derivable_impls` (deny); hand-write `impl Default` only for VALUES |
| 0199 / 0344 / 0430 | `as_`/`to_`/`into_` cost conventions, naming | `clippy::wrong_self_convention` + rustc naming lints |
| 1940 | `#[must_use]` on functions whose result is the point | `clippy::must_use_candidate` (pedantic) |
| 2383 | every suppression carries `reason = "…"` | `clippy::allow_attributes_without_reason` (deny) |
| 1946 | intra-doc links over bare paths | `rustdoc::broken_intra_doc_links` (deny) + the CI doc job |
| 0201 / 0344 | an error carries its cause (`Error::source`) | `#[source]`/`#[from]` on every wrapping variant; review-enforced |

The lints named above are configured in the root `Cargo.toml` `[lints]`
table.

## Type and error conventions

- `thiserror` error enums inside the library half (`lib.rs` and its modules);
  `main.rs` only turns the result of `cc_shift::run` into an exit code. No
  `unwrap`/`expect` outside `#[cfg(test)]`;
  `todo!()` and `unimplemented!()` are banned: an unready dependency gets a
  typed error or real code, never a panic placeholder.
- Model closed sets as Rust `enum`s (a command, a profile state); trait
  objects only for genuinely open, runtime polymorphism.
- `std::sync::LazyLock` (edition 2024) for statics, not `once_cell`.
- **No `use X as Y` import renaming.** Import types under their direct names.
  An alias papers over a naming problem: if the name is bad, FIX THE NAME at
  its definition; if two imports genuinely collide, qualify one at the use site
  (full path) instead of renaming. Only alias in highly exceptional cases where
  no other solution exists, with a comment saying why. (A trait import as
  `use Trait as _;` is not a rename and is fine.)
- Edition 2024, resolver v3. `cargo fmt` clean; run `cargo clippy` before
  considering a change done.
- **Suppressions are `#[expect(lint, reason = "…")]`**, scoped to the smallest
  item; `#[allow(lint, reason = "…")]` only for configuration-conditional fire
  (full policy: `reliability.md`).

## Documentation (`missing_docs` is enforced)

- Every public item carries a doc comment (rustc `missing_docs`). Shape,
  sections, and summary-line rules: `comments.md` (RFC 1574).
- Intra-doc links (`[`Type`]`) resolve in the scope of the module where the
  item is DEFINED. `rustdoc::broken_intra_doc_links` is deny; the CI doc job is
  the gate. Literal square brackets in prose are escaped `\[…\]`.

## Edition-2024 standing guidance (behaviour that compiles fine and differs)

- **`if let` scrutinee temporaries drop before `else`:** the guard rule lives in
  `reliability.md`; rewrite as `match` when a guard must span arms.
- **Never-type fallback**: `f()?;` on a function generic over the `Ok` type can
  now infer `!`; annotate the turbofish or binding type at such call sites
  instead of leaning on inference.
- **RPIT captures every in-scope lifetime by default:** when a returned
  `impl Trait` must NOT capture one, say so with precise capturing (`use<…>`).
- **`Future` and `IntoFuture` are in the prelude:** a collision with a local
  trait method is resolved with fully-qualified syntax, never an import rename.
- **`std::env::set_var` and `remove_var` are `unsafe` in edition 2024.** `cc-shift`
  never calls them: it sets `CLAUDE_CONFIG_DIR` on a child `claude` with
  `Command::env`, which is safe (`cc-shift add` then `exec`s it, the early
  start waits for it).
- The other `unsafe_*` 2024 items are moot under `unsafe_code = "forbid"`.

## Naming

A name says what the thing is in its domain, never which language, layer, or
pipeline stage produced it. Domain names follow `ARCHITECTURE.md`: a
**profile** is one account's config folder, the **profiles root** is
`~/.claude-profiles/`, and **remaining usage** and **reset time** are the two
facts automatic selection reads. Modules are named for their contents, verbs
only for a stage that is genuinely a transformation.

## No Python, anywhere

The tooling languages are **bash and Rust**. Python is banned across the
repository, in standalone scripts, and especially embedded in shell (a heredoc
nested inside a command substitution makes bash scan the *Python* for quote
pairs, so a single apostrophe breaks the whole script while pointing at the
wrong line). Reach for:

| Job | Tool |
|---|---|
| JSON read and build | `jq` |
| Line scans, field extraction | `awk`, `sed`, `grep` |
| Anything with real data structures | Rust |

## What not to do

- Do not special-case one profile by name in the code. Every profile is
  handled the same way; a difference between accounts is data, never a branch.
- Do not grow `cc-shift` into a wrapper process that stays running around `claude`
  or a resident daemon. Its work runs in short-lived processes (a command,
  the status line, the hook, a detached refresh, the early start run);
  anything new that must watch a session is designed in `ARCHITECTURE.md`
  first.
