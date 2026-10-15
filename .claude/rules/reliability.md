# Reliability and safety hard rules

`cc-shift` starts Claude Code signed in to an account and decides which account to
use. A wrong decision is quiet: the wrong profile starts, a profile with no
usage left is picked, or a stale reset time keeps a usable account out of the
rotation. Silent wrong answers are worse than loud failures. The principles
below are the standing bar for hand-written code, and each names the lint or
check that enforces it. **A rule without a failing check is a wish**, so every
lint here is wired into the root `Cargo.toml` `[lints]` table in the change
that creates the crate. Principles from the Rust API Guidelines checklist, the
Rust Book's error-handling and overflow chapters, and the Clippy book.

## Enforcement tiers (strongest first)

1. **Compile property:** the type system makes the violation unrepresentable
   (newtypes, `#[must_use]`, sealed traits, `forbid`).
2. **Lint at `deny`/`forbid`:** fails every `cargo clippy`, local and CI
   (`Cargo.toml` `[lints]`). `forbid` cannot be relaxed by any attribute;
   escaping it is an owner decision, not an `#[allow]`.
3. **Warn plus CI `-D warnings`:** `clippy::all` and `clippy::pedantic` both
   live here, so every pedantic lint is effectively a hard rule (including
   `missing_errors_doc` and `missing_panics_doc`).
4. **A committed check script or CI job:** `scripts/checks/comment-style.sh`
   (per edit through a hook), `scripts/checks/file-length.sh`, and the
   workflow lanes in `ci-cd.md`.
5. **Review-enforced** (weakest; minimize): only for properties no tool can
   check, each marked below.

## The rules

- **No `unsafe`, ever** (`unsafe_code = "forbid"`, tier 2). No exceptions; a
  need for `unsafe` is a design defect to solve differently. `exec` goes
  through `std::os::unix::process::CommandExt::exec`, which is safe.
  `unnecessary_safety_comment` and `unnecessary_safety_doc` deny a
  `// SAFETY:` comment or a `# Safety` doc section on safe code.
- **Fail loud, never wrap**: release builds run with `overflow-checks = true`
  so integer overflow panics instead of wrapping into a wrong value. On
  load-bearing arithmetic (usage percentages, the time left until a reset)
  prefer explicit `checked_*` or `saturating_*` with a typed error; a panic is
  the backstop, not the design. Numeric-honesty lints back this at deny tier:
  `float_cmp_const`, `lossy_float_literal`, `as_underscore`,
  `fn_to_numeric_cast_any`, `ambiguous_negative_literals`, plus
  `integer_division` at warn.
- **No `unwrap`/`expect`/`panic!`/`unreachable!`/`unimplemented!`/`todo!` in
  application code** (deny-tier lints). Tests keep the `clippy.toml`
  `allow-*-in-tests` scoping; a panicking assertion is how a test fails.
  Recoverable failures return typed errors (`thiserror` in the library half,
  `anyhow` only in `main.rs`), the Book ch9 split: panic is for states that
  cannot happen, `Result` for everything that can.
- **The ONE sanctioned escape for a logically-impossible `Err`/`None`** (Book
  ch9): a narrowly-scoped `#[expect(clippy::expect_used, reason = "…")]` on the
  smallest item, whose reason states the inspection proving unreachability,
  plus a *should*-phrased message (`.expect("the profile name should have
  been validated by parse")`). Dodging the lint with `unwrap_or_default()`
  instead is FORBIDDEN: that converts a loud impossible state into a value the
  program then acts on.
- **No panicking indexing** (deny tier: `indexing_slicing`, `string_slice`):
  `.get(..)` or pattern matching over `x[i]` and `&s[a..b]`. `string_slice`
  panics on a UTF-8 boundary. Tests are scoped out via `clippy.toml`.
- **Guards are never silently dropped**: `let _ = lock/handle;` is denied
  (`let_underscore_drop` plus `let_underscore_lock`): bind guards to named
  variables that live to scope end. `unused_result_ok` (deny) closes the
  `.ok();` variant. Edition-2024 corollary (review-enforced): a guard or borrow
  produced in an `if let` scrutinee is dropped BEFORE the `else` branch runs
  (<https://doc.rust-lang.org/edition-guide/rust-2024/temporary-if-let-scope.html>);
  rewrite as `match` when the guard must span both arms.
- **An error carries its cause** (RFC 0201; `Error::source`). Every wrapping
  variant uses `#[source]` or `#[from]`; a stringified cause
  (`map_err(|e| Variant(e.to_string()))`) cannot be walked, matched, or logged
  structurally. There is no lint for it; new code carries the source and
  reviewers check for it.
- **An unknown is never flattened into a default.** Usage that could not be
  read, a reset time that could not be parsed, or a profile whose login state
  is unclear is a typed error or an explicit "unknown" state, never `0`, never
  "full", and never "logged in". Automatic selection skips an unknown profile
  and says so; it never ranks a guess. (Review-enforced; the selection tests
  pin it.)
- **`Result → Option` inside a chain is a DECISION** (review-enforced; no lint
  can make the distinction). `.filter_map(|x| f(x).ok())` and `f(x).ok()?` turn
  an error into a missing element with no trace. A fallible conversion whose
  failure means "the input is DEFECTIVE" propagates a typed error; only one
  whose failure means "this input is legitimately ABSENT" may become `Option`,
  and it carries a `// NOTE:` saying so. A folder under `~/.claude-profiles/`
  that is not a valid profile name is the legitimate shape; a profile dropped
  from the ranking because its usage read failed is the defect shape.
- **Determinism is lint-backed** (deny tier: `iter_over_hash_type`): HashMap
  and HashSet iteration order is undefined; anything that feeds output or a
  ranking iterates ordered structures (`BTreeMap`, sorted vectors), so
  `cc-shift list` and a tie in `cc-shift auto` resolve the same way every run.
- **Output goes through one place.** `print_stdout` and `print_stderr` are
  denied outside the module that owns terminal output, which relaxes them with
  a scoped `#[expect]` and a reason; `dbg_macro` is denied everywhere. Nothing
  `cc-shift` prints or logs carries a token, a Keychain secret, or an account's
  email address (review-enforced).
- **Banned APIs are compile-time bans** (`clippy.toml` `disallowed-methods`
  and `disallowed-types`). A legitimate exception site carries a scoped
  `#[expect(clippy::disallowed_methods, reason)]`.
- **Errors are types, not strings, at every boundary that branches**
  (C-GOOD-ERR): a caller that needs to distinguish outcomes gets an enum
  variant, not a substring match.
- **Ids are distinct types where confusion is fatal** (C-NEWTYPE): a profile
  name is a validated `ProfileName`, never a bare `String`, so an unvalidated
  name cannot reach a path join.
- **Every public item: documented, `Debug`, with concrete `# Errors` and
  `# Panics` sections** (C-DOC, C-DEBUG, C-FAILURE): `missing_docs`,
  `missing_debug_implementations`, `missing_errors_doc`,
  `missing_panics_doc`.
- **Visibility is deliberate** (C-STRUCT-PRIVATE): private by default, scoped
  visibility only at real module boundaries, zero re-exports (every import
  names its defining module), `unreachable_pub` watched in CI.
- **Constructors and conversions follow the standard shapes** (C-CTOR,
  C-CONV): `new` and `with_*` builders, `From` and `TryFrom` over an ad-hoc
  `to_x()`, getters without a `get_` prefix.
- **Dependencies are pinned, locked, and vetted**: no new dependency for what
  the standard library or the pinned set already provides, and verify the
  version against crates.io or docs.rs at the moment of adding. CI builds run
  `--locked`, and `cargo deny check` gates advisories, licences and sources.
- **Comment style is machine-enforced** (`comments.md`).
- **A field's default value lives inline in its struct's `Default` impl**
  (`rust-style.md` §Default values).
- **Workflow security is audited** (`ci-cd.md`): pinned action digests,
  `permissions: {}` at workflow level, no context interpolation in a `run:`
  block, and `persist-credentials: false` on checkout.

## When a lint fights a legitimate case

**`#[expect(lint, reason = "…")]` is the default suppression:** it self-reports
the moment the expectation stops being fulfilled
(`unfulfilled_lint_expectations`), so stale suppressions cannot accumulate.
`#[allow(lint, reason = "…")]` is reserved for cases where the lint fires only
in SOME configurations (cfg-dependent code, macro expansions). Both forms MUST
carry `reason = "…"` (`allow_attributes_without_reason` = deny). Scope every
suppression to the smallest item; a file- or crate-level suppression needs the
owner's sign-off in the PR.
