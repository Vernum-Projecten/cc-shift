---
name: perl-edit-pitfalls
description: "Editing with perl from Bash: heredoc-quoted literals, never q{} or s{}{} around text with braces; read the current text right before matching, since rustfmt reflows it; a stale rmeta after fast edits needs cargo clean"
metadata:
  type: feedback
---

Three failures repeated on another of the owner's projects: a `q{...}` or
`s{...}{...}` perl string containing Rust braces breaks the script; a
replacement written from memory fails once `cargo fmt` has reflowed the
target; and an edit in the same second as the previous build leaves a stale
`.rmeta`, so the build reports "unresolved import" for an item that exists.

**How to apply:** put every literal in a perl `<<'END'` heredoc and match with
`\Q...\E`, dying on a miss so nothing half-applies; prefer the Edit tool for
exact multi-line replacements; when an error names an item that is plainly
there, run `cargo clean` before chasing it.
