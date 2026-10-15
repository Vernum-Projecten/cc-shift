---
paths:
  - ".github/**"
  - "scripts/**"
---

# CI/CD and supply-chain discipline

No specification governs this: our own design, grounded in the OWASP GitHub
Actions Security Cheat Sheet, SLSA v1.0 and OpenSSF Scorecard. This file
carries what CI runs and the rules every workflow keeps.

## What runs

- `.github/workflows/ci.yml`: the two-tier gate. Tier 1 needs no Rust:
  `zizmor`, `actionlint`, `shellcheck`, the comment-style guard
  (`scripts/checks/comment-style.sh`), the file-length guard
  (`scripts/checks/file-length.sh`) and the self-tests of the `scripts/gh/`
  helpers. Tier 2 is the Rust set below, gated on the root `Cargo.toml`
  existing. The `conclusion` job is the single required status check on
  `main`.
- CodeQL and OpenSSF Scorecard are not set up: the repository is private
  and has no GitHub Advanced Security. If either is added later, it stays
  advisory and gates no merge.
- Dependabot (`.github/dependabot.yml`) bumps the Cargo dependencies and the
  pinned action digests.

Auto-merge on a pull request waits for `conclusion`; a pull request is never
merged by hand past a red or missing check.

## Workflow security (every workflow, no exceptions)

- **Every `uses:` is pinned to a full commit SHA** with a trailing `# vX.Y.Z`
  comment. Dependabot (`github-actions`) bumps them. A tag or branch ref is a
  finding.
- **`permissions: {}` at workflow level**, with the minimum granted per job.
- **`persist-credentials: false`** on every `actions/checkout` that does not
  push with git.
- **No `${{ }}` context interpolation inside `run:`:** pass context through
  `env:`. This prevents template injection.
- **A publishing lane restores no build cache.** A cache an untrusted run could
  poison must not feed a release.

**Enforcement:** tier 1 runs `zizmor --min-severity=low .github/`,
`actionlint`, and `shellcheck --severity=style` on every push to `main` and
every pull request. Run the same three by hand before pushing a workflow or
script change, so a finding costs a local run rather than a CI round trip.
The zizmor path is the whole of `.github`, so `dependabot.yml` and every
composite action under `.github/actions/` are audited alongside the
workflows. Never narrow it to make a finding disappear: fix the cause, or
record a `# zizmor: ignore[audit]` suppression with its reason on the line the
finding names.

## Shell scripts are analysed like code

The tooling languages here are bash and Rust (`rust-style.md` §No Python).
Every committed shell script stays clean at `shellcheck --severity=style`, its
lowest floor, so every finding gates. A finding is FIXED, or it carries a
per-line `# shellcheck disable=SCnnnn` directive with its reason on the same
line. A blanket exclusion is refused, and no `.shellcheckrc` exists.

Every script runs under bash 3.2, the bash macOS ships, because CI runs bash 5
and would never see a construct 3.2 lacks: no associative arrays, no
`mapfile`, no `${var,,}`. An array that can be empty is expanded as
`${a[@]+"${a[@]}"}`.

## Rust CI lanes (tier 2 of `ci.yml`)

The local commands mirror the CI flags verbatim:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --doc --locked --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps --all-features --document-private-items
cargo deny check
```

`cargo deny check` covers advisories, licences, bans and sources, which
subsumes `cargo audit`. `cc-shift` is macOS-only, so the test lane runs on a macOS
runner; the lint and format lanes may run on Linux. **Always `--locked`**, so
CI fails on lockfile drift rather than on registry drift. Commit `Cargo.lock`.

## Releases

A release is a `vX.Y.Z` tag on `main`, cut from the session
(`.claude/memory/release-tag-is-mine.md`). The fix for a bad cut is a new
patch version, never a retag. A release lane, when one is added, builds in a
reusable workflow with no cache, assembles the release as a draft, attaches
every asset with its checksum and build provenance, and publishes last.

## Never

- Never unpin a `uses:` to a tag or branch, widen a job's permissions without
  cause, interpolate context into `run:`, or restore a cache in a publishing
  lane.
- Never weaken a gate to go green (`testing.md`); fix the cause.
- **Never add AI or Claude attribution** to any commit, PR, or release text.

## Official documentation (durable citations)

- OWASP GitHub Actions Security Cheat Sheet:
  <https://cheatsheetseries.owasp.org/cheatsheets/GitHub_Actions_Security_Cheat_Sheet.html>
- SLSA v1.0: <https://slsa.dev/spec/v1.0/>
- OpenSSF Scorecard: <https://github.com/ossf/scorecard-action>
- GitHub Actions security hardening:
  <https://docs.github.com/en/actions/security-for-github-actions/security-guides/security-hardening-for-github-actions>
