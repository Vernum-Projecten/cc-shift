# Changelog

All notable changes to `cc-shift` are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.4] - 2026-10-15

### Added

- Releases are built by one workflow on a `v*` tag: a GitHub release with
  `cc-shift` for Apple silicon and Intel Macs, each with a SHA-256 file and
  build provenance, and the crate published to crates.io through Trusted
  Publishing.
- A `NOTICE` file naming Vernum Projecten B.V. as the copyright holder,
  shipped with the crate.

### Changed

- The README is written for crates.io: `cargo install cc-shift` first, a
  quick start, update and uninstall steps and a FAQ.
- docs.rs builds the documentation for the two macOS targets.
- The crate package no longer includes `ARCHITECTURE.md`; the README links
  to it on GitHub.

## [0.0.3] - 2026-10-15

The first published version. `cc-shift` switches Claude Code between several
Claude accounts on macOS on its own, before a usage limit is hit.

### Added

- `cc-shift add <name>` creates a profile and opens Claude Code in it to run
  `/login` once for that account.
- `cc-shift setup` installs the status line and a `StopFailure` hook with
  matcher `rate_limit` in `~/.claude/settings.json`, keeps every other
  setting and chains your own status line, and makes the best account active.
- `cc-shift status` shows every account with both usage windows, the time
  until each resets, the active account and the next one.
- `cc-shift use <name>` makes an account active in every open Claude Code
  window at once, the way `/login` does, with no restart.
- `cc-shift refresh` refreshes parked logins and reads parked accounts' usage.
  `cc-shift status` and the status line run it on their own.
- `cc-shift list` prints the profile names.
- Automatic switching: the status line records the active account's usage
  after every reply and switches at 95% of the 5-hour or the 7-day window;
  the rate-limit hook switches at once when a turn hits a limit anyway.
- The selection rule spends what expires first: an account whose 5-hour
  window is running goes first, the one that resets soonest first, then an
  untouched account, the one whose week resets soonest first.
- The status line shows the active account with both windows and their time
  to reset.
- Logins are moved between the macOS login Keychain items of the profiles
  and of `~/.claude`, never copied, so each login keeps one live copy; a
  switch that stops halfway is finished by the next run.
- Parked logins are refreshed by `cc-shift`, and their usage is read from
  Anthropic's usage endpoint; the active login is left to Claude Code.
- A token is never a command-line argument: Keychain writes and HTTP
  requests receive it on stdin.

[Unreleased]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.4...HEAD
[0.0.4]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/Vernum-Projecten/cc-shift/releases/tag/v0.0.3
