# Contributing to cc-shift

The design of record is [`ARCHITECTURE.md`](ARCHITECTURE.md), the build is
tracked on the issues of this repository, and the working discipline is
[`CLAUDE.md`](CLAUDE.md). Read both before making a change.

## What helps most

A change that answers an open issue, with its acceptance criteria met. A
change to the command surface, the profile layout or another decision in
`ARCHITECTURE.md` starts as an issue with the evidence, before any code.

## Build and test

The shell, workflow and guard set runs on every change:

```
zizmor --min-severity=low .github/
actionlint
shellcheck --severity=style $(git ls-files '*.sh')
scripts/checks/comment-style.sh --all
scripts/checks/file-length.sh
scripts/checks/bash-compat.sh
scripts/checks/script-modes.sh
```

These are the tier-1 jobs `ci.yml` runs, with the same flags, so a local pass
means a CI pass. On macOS, `brew install shellcheck actionlint zizmor`
installs the tools. The `conclusion` job aggregates every job and is the
single required check on `main`.

The Rust gates mirror CI:

```
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --doc --locked --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features --document-private-items
cargo deny check
```

Every cargo invocation uses `--locked`, and `Cargo.lock` is committed.

## Official documentation is the authority

Claude Code's behaviour (`CLAUDE_CONFIG_DIR`, `/login`, `/status`) comes from
Anthropic's Claude Code documentation, the Keychain from Apple's Keychain
Services documentation, and Rust behaviour from the Rust reference and each
crate's docs.rs page at its pinned version. Cite the page in the pull request.
Where nothing governs a decision, say so: "no specification governs this: our
own design".

Never put a token, a Keychain item, an account email or the content of a
profile folder in a fixture, an issue, or a pull request. Fixtures are
synthetic.

## Pull requests

- Branch from `main` with a conventional-type name (`feat/`, `fix/`, `chore/`,
  `docs/`, `refactor/`, `perf/`, `test/`, `ci/`, `build/`, `release/`).
- Every commit is signed, and the pull request body declares `Closes #<n>` for
  the tracker issue it answers, one `Closes` keyword per issue.
- No AI or assistant attribution anywhere in the commits or the pull request.
- Never weaken, skip, or delete a test to make a build pass.
- Every workflow `uses:` is pinned to a full commit SHA with a trailing version
  comment; `permissions:` is `{}` at the workflow level with the minimum
  granted per job, and no untrusted context is interpolated into a `run:`
  block.
- Write prose to `.claude/rules/writing-style.md`: no em dashes, no
  "not X but Y", no decorative triads, no filler buzzwords.

## Licence

cc-shift is licensed under the Apache License 2.0 ([`LICENSE`](LICENSE)). A
contribution is licensed under the same terms (section 5 of the licence).

## Security

Report a vulnerability privately as described in [`SECURITY.md`](SECURITY.md),
never in a public issue.
