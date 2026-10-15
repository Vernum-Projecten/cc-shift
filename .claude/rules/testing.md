---
paths: ["**/*.rs", "**/tests/**"]
---

# Testing discipline

Test discipline is non-negotiable (a standing hard rule; see `CLAUDE.md`).

## The hard rule

- **Never** silently weaken, skip, or delete an existing test to make a build
  pass.
- **Never** edit a test to route around a runtime bug it exposes. If a test
  fails and the fix is unclear, leave it failing and record a
  `// TODO(#NNNN):` naming its issue; do not touch the test to make it green.
- A test asserts the documented behaviour (`ARCHITECTURE.md` for `cc-shift` itself,
  Claude Code's documentation for what `cc-shift` relies on), never whatever the
  current code happens to do.

## Tooling

- **Runner:** `cargo-nextest` (`cargo nextest run`), not `cargo test`, plus
  `cargo test --doc` for doctests.
- **Screens:** terminal output (`cc-shift status`, `stats`, `log`, the status
  line, error messages) is pinned with `assert_eq!` against the exact text of
  the approved screen, rendered at a fixed "now" and without colour. A
  changed screen is a reviewed diff of that expected text.
- **Properties:** `proptest` for the selection invariants (the rule spends what
  expires first, an exhausted account is never chosen before its reset
  time, ties resolve the same way every run) and for profile-name validation.
- **A fake home and a fake `claude`.** Integration tests never touch the real
  `~/.claude-profiles/`, the real Keychain, or a real account. Each test runs
  with `HOME` pointed at a temporary directory and a stub `claude` first on
  `PATH` that records its arguments and environment. The Keychain and HTTP
  are behind traits, so a test asserts which login moved where, and what
  the state, the history and the settings hold afterwards.
- **Time is injected.** Selection takes "now" as a parameter, so reset-time
  logic is tested at fixed instants, never against the wall clock.

## Where tests live

Unit tests live beside the code they test (`#[cfg(test)] mod tests` in the same
file), and ONLY there: **dedicated test FILES under `src/` are banned**. A test
that drives the public API belongs in `tests/`; a test of private internals
stays a small inline module. If an internals test grows large, that is a
design signal to test through the public seam.

**One integration-test binary**: the `tests/` directory is `tests/it/main.rs`
plus one `mod` per topic file, not one top-level `.rs` per topic. Cargo
compiles and links every top-level `tests/*.rs` as its own crate ("each
integration test results in a separate executable binary … this can be
inefficient", <https://doc.rust-lang.org/cargo/reference/cargo-targets.html>).
Shared helpers live in a plain module under `tests/it/`.

**A binary-only crate is untestable by construction** (Book ch11.3): its
`main.rs` cannot be imported from `tests/`. `cc-shift` therefore keeps a thin
`main.rs` over a testable `lib.rs` run path (Book ch12.3), and its integration
tests import the lib or run the built binary.

## Test shapes (the Book ch11 doctrine)

- **`Result`-returning tests are the preferred shape**: `fn t() -> Result<(),
  E>` with `?` instead of unwrap chains
  (<https://doc.rust-lang.org/book/ch11-01-writing-tests.html>).
  `clippy::panic_in_result_fn` fires on a Result-returning test that also
  asserts, so such a test carries the lint in the same scoped relaxation its
  file uses for `panic`/`unwrap`/`expect` (`#![allow(…, reason = "test
  assertions")]` at the test-file root, or a `#[expect(…, reason)]` on the
  single test). Never relaxed for the whole crate and never in a non-test
  module.
- **`#[should_panic]` always carries `expected = "…"`:** bare `should_panic`
  passes when the code panics for the WRONG reason (Book ch11.1).
- **Assertions**: `assert_eq!` and `assert_ne!` over bare `assert!` for
  comparisons (they print both values); a production-code assert carries a
  message.
- **Doctests are copy-paste templates**: `?` via a hidden `# Ok::<(), E>(())`
  tail or a hidden `fn main`, never `unwrap`. `no_run` for an example that
  would launch a process, `text` for non-code, never `ignore`.

## Coverage is a mandate, not just pass rate

A green suite over a thin set of cases proves almost nothing. The bar is
coverage of what `ARCHITECTURE.md` defines:

- Every command `cc-shift help` lists and every error path `ARCHITECTURE.md`
  section 8 names: a missing `HOME`, an invalid or reserved
  profile name, a missing profile folder, a folder that cannot be created, and
  `claude` missing from `PATH`, each with its own small, isolated case.
- The command line: every flag and every misuse, and `CLAUDE_CONFIG_DIR`
  naming the right folder wherever `cc-shift` starts `claude`.
- Selection: every rule of automatic selection, including all profiles
  exhausted, a tie, a reset time that has just passed, and a profile whose
  usage could not be read.
- **A documented behaviour with no case is a COVERAGE GAP, never an acceptable
  omission.** Close it or record the honest boundary on the issue.
- **Coverage only ratchets up.** Cases are added, never removed to go green.
- **One behaviour per case:** many small isolated cases beat one broad case.

## No personal data

Fixtures use invented profile names and invented accounts. Never commit a real
account's email address, a token, a Keychain dump, or a copy of a real
`~/.claude-profiles/` folder.

## Target

Compiling, clippy-clean, tested increments at all times; a green suite is the
standing bar and every change preserves it. Green comes ONLY from fixing the
defect, never from bending a test or a fixture to match the implementation.
