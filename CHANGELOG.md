<!-- SPDX-FileCopyrightText: 2026 Vernum Projecten B.V. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# Changelog

All notable changes to `cc-shift` are recorded here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Removed

- The early start: `cc-shift early`, `early on`, `early off` and `early run`,
  and the scheduled job that started 5-hour windows before the workday.
  `cc-shift setup` and `cc-shift uninstall` delete the job and its working
  folder that an older version installed, and `cc-shift doctor` reports one
  that is left. A state file that still holds the early start setting loads,
  and early starts already in the history still show in `cc-shift log`
  (#126).

## [0.1.3] - 2026-10-18

### Fixed

- `cc-shift` switches before the limit while the main session waits on
  background subagents. The status line now also runs every 30 seconds, the
  active account's usage is read from the usage endpoint when the status line
  has reported no new numbers for a minute, and `cc-shift refresh` switches
  when that reading is at or past a threshold. Run `cc-shift setup` again to
  add the 30-second status line to an existing setup; `cc-shift doctor` says
  when it is missing (#123).
- A status line reading that repeats an older, lower number for the same
  window no longer lowers the recorded usage (#123).
- A turn that hits the 5-hour limit while the week shows a higher percentage
  marks the 5-hour window full, no longer the week, and the history names the
  5-hour window (#123).

## [0.1.2] - 2026-10-16

### Changed

- The next account is the one with the most of its week left, an untouched
  account first, so the weeks drain together and several accounts keep some
  week to switch to when a 5-hour window fills. An account whose week resets
  within 5 hours still goes first, so what is left of it is not lost. A
  running 5-hour window no longer goes first (#120).

## [0.1.1] - 2026-10-15

### Added

- `cc-shift` builds for Windows. Logins move between the `.credentials.json`
  files Claude Code keeps on Windows, under `%USERPROFILE%`; a switch stops
  with a clear message while Claude Code keeps logins in Windows Credential
  Manager. On Windows the rate-limit hook runs `cc-shift.exe` directly, the
  status line works in Git Bash and PowerShell, `cc-shift add` waits for
  Claude Code, `curl.exe` comes from the Windows system folder, child
  programs start without a console window, and the early start runs from
  Task Scheduler.
- Each release has `.zip` archives of `cc-shift.exe` for x64 and Arm64
  Windows, and CI builds, lints and tests on Windows as well.
- `cc-shift completion powershell` prints a completion script for Windows
  PowerShell 5.1 and PowerShell 7, for the commands, the profile names and
  the flags.
- The README, the crates.io description, docs.rs and the GitHub repository
  describe Windows, with a `platform: macOS | Linux | Windows` badge and the
  PowerShell steps to install the Windows binary.

### Changed

- The error for a missing home folder names `USERPROFILE` on Windows as well
  as `HOME`.
- `cc-shift add` refuses a name that differs from an existing profile only
  in case, and on Windows the device names `con`, `prn`, `aux`, `nul`,
  `com1` to `com9` and `lpt1` to `lpt9`.
- Temporary JSON files and the IDE module file carry the name `cc-shift`
  instead of the old name `cs`.

## [0.1.0] - 2026-10-15

### Added

- `cc-shift` builds and runs on Linux. Logins are moved between the
  `.credentials.json` files Claude Code keeps on Linux (mode 0600, replaced
  in one rename so open windows follow with their next message), and the
  early start runs from a systemd user timer.
- Each release has static Linux binaries for x86_64 and arm64 next to the
  macOS ones, and CI builds, lints and tests on Linux as well as macOS.
- `doctor` checks that `curl` is installed, and `cc-shift` finds `curl` on
  `PATH` when `/usr/bin/curl` is missing, as on `NixOS`.
- A switch holds Claude Code's own refresh lock of `~/.claude` while it
  saves and replaces the active login, so no open window refreshes the login
  at that moment. This makes switching safer on macOS and Linux.
- Every file carries its copyright holder and its licence (`Apache-2.0`) as
  SPDX lines, and the repository follows the REUSE 3.3 specification,
  checked in CI.
- The README, the crates.io description and keywords, docs.rs and the
  GitHub repository describe macOS and Linux, with a `platform: macOS | Linux`
  badge.

### Changed

- Messages say "stored login" instead of "Keychain login", and the help says
  the early start is run by the scheduled job, so they read right on Linux.
- A morning early start that runs more than 3 hours after its set time (a
  machine that woke or started late) starts nothing and says so in
  `cc-shift log`.

## [0.0.9] - 2026-10-15

### Added

- `cc-shift early on [--at 06:00] [--accounts 2]` starts the 5-hour windows of
  a few accounts each workday morning, so they reset during the workday and
  give an extra window. A `launchd` agent runs it Monday to Friday; Claude
  Code sends one short Haiku message per account in the account's profile.
  Off by default; `cc-shift early off` stops it and `cc-shift early` shows
  it. `cc-shift log`, `cc-shift status`, `doctor` and `uninstall` know about
  it, and `early` is no longer a valid profile name.

### Changed

- The README is reordered for new users and shows how to install a prebuilt
  binary with its checksum and build attestation. The release archives now
  include `NOTICE` next to `LICENSE`.

### Fixed

- `cc-shift help` prints the version and tagline under the logo, on their
  own line, so the help no longer wraps in a narrow terminal.

## [0.0.8] - 2026-10-15

### Added

- `cc-shift --version` (also `-V` and `cc-shift version`) prints the running
  version, and `cc-shift help` opens with an ASCII logo on a terminal.
- `cc-shift log` shows the history of switches (with their reason), refreshed
  and logged-out logins, and when all accounts were full and free again,
  from `~/.claude-profiles/history.jsonl`; `--since`, `--all`, `--verbose`
  and `--json`.
- `cc-shift stats` shows the tokens of all accounts together per project and
  model: output with its share, input, cache writes and cache reads, read
  from the session transcripts of `~/.claude` and every profile, subagents
  included, each response counted once. `--since 2h|7d|30d` and `--all` pick
  the period and add a section per day, `--project <name>` shows one project
  per model and day, `--cost` adds the cost at Anthropic's API prices,
  `--per-account` adds a block per account from the switch history, and
  `--json` prints one document. What it read is cached per file in
  `~/.claude-profiles/stats-cache.json`. `stats` can no longer name a profile.
- A weekly workflow compares Anthropic's pricing page with the price table
  and opens an issue when a price changes or a model is added.

### Changed

- The switch thresholds are per window and configurable: the 5-hour window
  switches at 95% and the 7-day window at 99% by default, each settable from
  50 to 100 under `thresholds` in `~/.claude-profiles/state.json`.

## [0.0.7] - 2026-10-15

### Added

- `cc-shift doctor` checks the status line and the hook, `claude` and its
  version, every profile's login and signed-in account, two profiles signed
  in to the same account, the state file, an unfinished switch, logged-out
  accounts and the account details in `~/.claude.json`, and prints the
  command that fixes each problem. It exits 1 when a check fails.
- `cc-shift completion zsh` prints a zsh completion script for the commands,
  the profile names after `use` and `remove`, and the flags.
- `cc-shift remove <name>` deletes a profile, its Keychain login and what
  the state remembers about it, after you type `yes`; the active account
  cannot be removed.
- `cc-shift uninstall` removes the status line and the rate-limit hook and
  puts back the status line you had before; `--all` also deletes every
  profile and its Keychain login after you type `yes`.

## [0.0.6] - 2026-10-15

### Changed

- `cc-shift status` opens with a summary line (accounts, how many are free,
  the active and the next account) and lists the accounts in the order
  `cc-shift` will use them, each window as a bar with its percentage and
  time to reset, and a state: active, next, ready, full or week full with
  when it frees up, logged out, or unknown with the reason.
- The status line shows the window's account with a bar for the 5-hour
  window, the 7-day percentage, the next account and how many are free.
- Colour (green, yellow, red by how full) on a terminal and in the status
  line, off when `NO_COLOR` is set.

### Added

- `cc-shift status --json` prints the same overview as JSON for scripts:
  the active and next account, how many are free, and per account its
  state, both windows with RFC 3339 reset times and seconds left, and
  whether its numbers are confirmed.

## [0.0.5] - 2026-10-15

### Fixed

- Automatic switching no longer cascades through accounts. Right after a
  switch the status line still reports the usage of the account just left;
  that reading was recorded under the new account, which then looked full.
  Readings are now matched to the account whose own confirmed usage has the
  same reset times, a switch needs a reading proven to be the active
  account's, and no switch follows another within five minutes.
- The active account's usage is read from Anthropic's usage endpoint in the
  background too, so every number in `cc-shift status` comes from the
  account itself.
- The account details in `~/.claude.json` are put right when a Claude Code
  window writes old ones back.

### Changed

- The status line names the account the window really uses, shows
  `moving to <account>` right after a switch, and names the next account
  when all are full.

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

[Unreleased]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.9...v0.1.0
[0.0.9]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.8...v0.0.9
[0.0.8]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.7...v0.0.8
[0.0.7]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.6...v0.0.7
[0.0.6]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.5...v0.0.6
[0.0.5]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/Vernum-Projecten/cc-shift/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/Vernum-Projecten/cc-shift/releases/tag/v0.0.3
