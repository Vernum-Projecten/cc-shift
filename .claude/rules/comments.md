---
# SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
# SPDX-License-Identifier: Apache-2.0
paths: ["**/*.rs"]
---

# Comments and documentation (RFC 505 + RFC 1574)

The authority is the official Rust API documentation conventions: RFC 505
(<https://rust-lang.github.io/rfcs/0505-api-comment-conventions.html>) and
RFC 1574
(<https://rust-lang.github.io/rfcs/1574-more-api-documentation-conventions.html>),
plus the rustdoc book. Comments are the fastest-rotting artifact in a repo, and
essays in code are banned. This file is the single home of the comment rules;
`rust-style.md` points here.

## The prime rule: a comment earns its lines

Code says WHAT; a comment exists only for what the code cannot show: the
citation, the non-obvious why, the constraint. Everything else already has a
durable home and goes there, not into the source:

| Content | Home |
|---|---|
| Decisions, trade-off discussions, history | the PR description or tracker issue |
| Design decisions | `ARCHITECTURE.md` |
| API usage, contracts, examples | doc comments (`///`) |
| What changed and why it is correct | the PR, never the code |

**No change-narration in comments**: "previously…", "now correctly…", "before
this refactor…", "the former X is retired" is PR text; git history carries it.
A comment describes the code as it IS.

## Budgets

- `// NOTE:` = a citation plus ONE sentence, **max 3 physical lines**. The full
  reasoning lives on the issue or PR.
- A plain `//` comment run is **max 8 physical lines**. Longer prose is either
  API documentation (move it into the item's `///` docs) or a record (move it
  to the PR or issue).
- Block comments (`/* … */`, `/** … */`) are banned: "Avoid block comments. Use
  line comments instead" (RFC 505). `/* arg */` parameter labels become named
  locals.

## Doc comments (`///`)

- `///` documents items; `//!` is ONLY for crate- and module-level docs
  ("nothing else", RFC 1574); above a `mod` block prefer `///` outside it.
- **Summary line**: the first line is a single short sentence, third person
  singular present indicative ("Returns…", "Creates…", never "Return" or "This
  function returns"), properly punctuated, followed by a blank `///` line
  before any detail (`clippy::too_long_first_doc_paragraph`).
- Full sentences over fragments; American English spelling.
- Markdown with `#` top-level section headings, in this order where present:
  `# Examples` (always plural, even for one example), `# Panics`, `# Errors`,
  `# Safety` (`missing_errors_doc` and `missing_panics_doc` enforce presence;
  `unnecessary_safety_doc` bans `# Safety` on safe code).
- Code fragments in backticks (`clippy::doc_markdown`); longer examples in
  triple-backtick blocks, non-Rust blocks explicitly tagged (```text):
  rustdoc tests every untagged block as Rust. Doctest shapes: `testing.md`
  §Test shapes.
- Name generic types fully (`Option<T>`, not `Option`); link with intra-doc
  links (`[`Type`]`) and reference-style links; bare URLs in `<…>`.
- Doc comments state the CURRENT contract only, with no history and no
  decision trail. Cite only durable sources: official documentation (the Rust
  book and reference, Apple's documentation for a macOS API, Claude Code's
  documentation for `CLAUDE_CONFIG_DIR` and its other settings, the docs.rs
  page of a pinned crate).

## Annotation vocabulary (the only sanctioned markers)

- `// TODO(#NNNN): <what is missing>`, pending work, ALWAYS with its tracker
  issue. Deferred work is a TODO, never prose ("lands later", "deferred to X"
  is banned); phase and plan markers are banned.
- `// NOTE: <citation plus one sentence>`, a SETTLED decision: the citation,
  or the explicit flag "no specification governs this: our own design". If it
  describes something not yet done, it is a TODO.
- `// SAFETY:`, reserved for `unsafe` (forbidden anyway;
  `unnecessary_safety_comment` denies misuse).
- No other marker form exists: `FIXME`, `HACK`, `XXX`, `WIP`, and any bespoke
  vocabulary fail the guard.

## Citations of internal files

A comment never cites an internal file as an authority (`ARCHITECTURE.md`,
`CLAUDE.md`, a path into `.claude/`, or a rule or memory file by name).
Internal docs move and get rewritten; the comment then points at nothing. Cite
the official documentation the decision rests on, or write "no specification
governs this: our own design".

## Enforcement register

- `scripts/checks/comment-style.sh`: block comments, the `TODO(#N)` form,
  banned marker vocabulary, the NOTE and `//` run budgets, and citations of
  internal files. It runs per edit (the `rust_fmt_clippy.sh` PostToolUse hook)
  and in CI.
- `clippy::too_long_first_doc_paragraph` (CI `-D warnings`): the RFC 1574
  summary line, configured in `[lints.clippy]` of the root `Cargo.toml`.
- Companion doc lints (`doc_markdown`, `missing_errors_doc`,
  `missing_panics_doc`, `unnecessary_safety_comment`,
  `unnecessary_safety_doc`, the `[lints.rustdoc]` table plus the CI doc job):
  the same file.
- Review-enforced (no tool can judge them): change-narration, prose deferrals,
  third-person summary phrasing, and essays relocated into doc comments to
  dodge the `//` budget.
