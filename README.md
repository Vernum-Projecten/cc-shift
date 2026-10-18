<!-- SPDX-FileCopyrightText: 2026 Vernum Projecten B.V. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# cc-shift

[![crates.io](https://img.shields.io/crates/v/cc-shift?logo=rust)](https://crates.io/crates/cc-shift)
[![Downloads](https://img.shields.io/crates/d/cc-shift)](https://crates.io/crates/cc-shift)
[![CI](https://github.com/Vernum-Projecten/cc-shift/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Vernum-Projecten/cc-shift/actions/workflows/ci.yml)
[![Platform: macOS | Linux | Windows](https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-lightgrey)](#requirements)
[![Rust 1.98+](https://img.shields.io/badge/rust-1.98%2B-orange?logo=rust)](https://github.com/Vernum-Projecten/cc-shift/blob/main/rust-toolchain.toml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Vernum-Projecten/cc-shift/blob/main/LICENSE)

**Use several Claude accounts with Claude Code, and never think about usage limits again.**

`cc-shift` watches the usage of the account Claude Code is signed in to. When it gets close to its 5-hour or 7-day limit, `cc-shift` moves every open Claude Code window to your next account, the same way `/login` does. There is no restart: the conversation, its history and the working directory stay as they are.

> [!IMPORTANT]
> `cc-shift` works on **macOS, Linux and Windows**. It keeps logins where Claude Code keeps them: in the login Keychain on macOS, and in a `.credentials.json` file per config folder on Linux and Windows. Inside WSL 2, Claude Code is the Linux build, so use the Linux binary there.

> [!WARNING]
> Spreading your work over several subscriptions so that one person gets past each account's own usage limit may be against [Anthropic's terms](https://www.anthropic.com/legal/consumer-terms), and Anthropic can suspend accounts that break them. Check the terms that apply to your accounts before you use `cc-shift`. You use it at your own risk.

## Quick start

```sh
cargo install cc-shift

cc-shift add work        # run /login with your first account, then quit
cc-shift add spare       # run /login with your second account, then quit

cc-shift setup           # once; switching is automatic from now on
```

Then use Claude Code as you always do. Under the prompt you see which account is active and how much of it is used:

```text
work ▕██████░░░░▏ 60% · 3h33m │ 7d 31% │ next spare · 2/4 free
```

## Contents

- [Requirements](#requirements)
- [Install](#install)
- [Set up](#set-up)
- [How it works](#how-it-works)
- [Which account is next](#which-account-is-next)
- [Switch thresholds](#switch-thresholds)
- [Commands](#commands)
- [Token statistics](#token-statistics)
- [History](#history)
- [Security](#security)
- [Update and uninstall](#update-and-uninstall)
- [FAQ](#faq)
- [Contributing](#contributing)
- [License](#license)

## Requirements

| What | Version |
|---|---|
| System | macOS (Apple Silicon or Intel), Linux (x86_64 or arm64, any distribution, WSL 2 included), or Windows 10 1809 or later and Windows 11 (x64 or Arm64) |
| [Claude Code](https://code.claude.com/docs) | 2.1.300 or later |
| Claude accounts | Pro, Max or Team, two or more |
| Rust | 1.98 or later, only to build `cc-shift` yourself ([install Rust](https://rustup.rs)) |

## Install

### From crates.io (recommended)

```sh
cargo install cc-shift
```

This builds `cc-shift` and puts it in `~/.cargo/bin`. Check that it is on your `PATH`:

```sh
cc-shift --version
```

### Prebuilt binary

Every [release](https://github.com/Vernum-Projecten/cc-shift/releases) has a binary for each system, with a SHA-256 file and a build attestation:

| System | `target` |
|---|---|
| macOS, Apple Silicon | `aarch64-apple-darwin` |
| macOS, Intel | `x86_64-apple-darwin` |
| Linux, x86_64 | `x86_64-unknown-linux-musl` |
| Linux, arm64 | `aarch64-unknown-linux-musl` |
| Windows, x64 | `x86_64-pc-windows-msvc` (a `.zip`) |
| Windows, Arm64 | `aarch64-pc-windows-msvc` (a `.zip`) |

The Linux binaries are static, so they run on any distribution. On macOS and Linux, download with `curl`; on a Mac, a browser download is quarantined by macOS:

```sh
version=0.1.3
target=aarch64-apple-darwin    # your target from the table above
archive=cc-shift-v$version-$target.tar.gz

curl -LO https://github.com/Vernum-Projecten/cc-shift/releases/download/v$version/$archive
curl -LO https://github.com/Vernum-Projecten/cc-shift/releases/download/v$version/$archive.sha256
shasum -a 256 -c $archive.sha256    # or: sha256sum -c $archive.sha256
tar xzf $archive
mkdir -p ~/.local/bin && mv cc-shift-v$version-$target/cc-shift ~/.local/bin/
```

`~/.local/bin` must be on your `PATH`. On Windows, in PowerShell:

```powershell
$version = '0.1.3'
$target = 'x86_64-pc-windows-msvc'    # aarch64-pc-windows-msvc on Arm64
$archive = "cc-shift-v$version-$target.zip"
$base = "https://github.com/Vernum-Projecten/cc-shift/releases/download/v$version"

Invoke-WebRequest "$base/$archive" -OutFile $archive
Invoke-WebRequest "$base/$archive.sha256" -OutFile "$archive.sha256"
(Get-FileHash $archive -Algorithm SHA256).Hash.ToLower() -eq (Get-Content "$archive.sha256").Split(' ')[0]
Expand-Archive $archive -DestinationPath .
New-Item -ItemType Directory -Force "$env:USERPROFILE\.local\bin" | Out-Null
Move-Item "cc-shift-v$version-$target\cc-shift.exe" "$env:USERPROFILE\.local\bin\"
```

The hash line must print `True`, and `%USERPROFILE%\.local\bin` must be on your `PATH`. Keep `cc-shift.exe` in a folder whose path has no spaces: the status line runs it through Git Bash or PowerShell, and `cc-shift setup` refuses a path neither can run unquoted.

With the [GitHub CLI](https://cli.github.com) you can also check that GitHub built the archive from this repository:

```sh
gh attestation verify $archive --repo Vernum-Projecten/cc-shift
```

### From source

```sh
git clone https://github.com/Vernum-Projecten/cc-shift.git
cd cc-shift
cargo install --path . --locked
```

### Shell completion

To complete the commands, profile names and flags with Tab in zsh, add this line to `~/.zshrc`, after `compinit`:

```sh
source <(cc-shift completion zsh)
```

In PowerShell (Windows PowerShell 5.1 or PowerShell 7), add this line to your profile, the file `$PROFILE` names:

```powershell
cc-shift completion powershell | Out-String | Invoke-Expression
```

## Set up

**1. Add each account once.** For every account:

```sh
cc-shift add work
```

Claude Code opens in that account's own profile. Run `/login`, sign in with that account, then quit Claude Code. To be sure you pick the right account, use a private browser window per account, or sign out of the previous account in the browser first.

Profile names use letters, digits, `-` and `_`, and start with a letter or digit. A new name may not differ from an existing profile only in case (`Work` beside `work`), and on Windows `con`, `prn`, `aux`, `nul`, `com1` to `com9` and `lpt1` to `lpt9` are not allowed.

**2. Turn on automatic switching:**

```sh
cc-shift setup
```

This adds a status line and a rate-limit hook to `~/.claude/settings.json` and keeps every other setting. The status line also runs every 30 seconds, so `cc-shift` keeps switching while your main session waits on subagents. Run `cc-shift setup` again after an upgrade; `cc-shift doctor` tells you when a setup is out of date. A status line you already had keeps working and is shown first. `cc-shift setup` then makes your best account active. Running it again changes nothing.

**3. Check it.** Send a message in any Claude Code window, then:

```sh
cc-shift status
```

```text
cc-shift · 4 accounts · 2 free now · active work · next spare

  ACCOUNT  5-HOUR                   7-DAY                    STATE
▶ work     ██░░░░░░░░  23%  4h22m   ███████░░░  67%  4d2h    active
  spare    █░░░░░░░░░  10%  2h52m   ██████░░░░  64%  3d12h   next
  extra    ██████████ 100%  2m      █████░░░░░  49%  3d18h   full · free in 2m
  old      not started              ██████████ 100%  16h22m  week full · free in 16h22m
```

Accounts are listed in the order `cc-shift` will use them. In a terminal the bars are green, yellow or red by how full they are; set `NO_COLOR` to turn colour off.

From here on you never need to run `cc-shift` while you work. If something looks wrong, `cc-shift doctor` checks the whole setup.

## How it works

```text
~/.claude-profiles/work      parked login of the account "work"
~/.claude-profiles/spare     parked login of the account "spare"
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder, and its login stays parked there: in the Keychain on macOS, in the profile's `.credentials.json` on Linux and Windows.
- Every Claude Code window uses the default `~/.claude`, which holds the active account.
- After every reply, the status line records the active account's usage. When the 5-hour window reaches 95% or the 7-day window 99% (both [configurable](#switch-thresholds)), `cc-shift` moves the next account's login into `~/.claude` and copies its account details into `~/.claude.json`. Open windows pick both up, as they do after `/login`.
- If a turn hits a limit anyway, the rate-limit hook switches at once.
- In the background, `cc-shift` refreshes the parked logins and reads their usage, so the next choice is made on current numbers. While subagents work and the main session gets no replies, it reads the active account's usage every minute and switches when it reaches a threshold.

The full design is in [ARCHITECTURE.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/ARCHITECTURE.md).

## Which account is next

The week is what runs out: usage left in a 5-hour window when it resets still counts against the week, and only a week that resets with usage left loses it. So `cc-shift` spreads the weeks, keeping as many accounts as it can with some week left to switch to:

1. An account whose week resets within 5 hours, the one that resets soonest first, so what is left of that week is not lost.
2. Then the account with the most of its week left, an untouched account first.
3. Then the one whose week resets soonest.

It stays on the active account until it reaches its threshold, so it never switches back and forth. Each time a 5-hour window fills, the account with the most week left takes over.

## Switch thresholds

`cc-shift` switches when the 5-hour window reaches **95%** or the 7-day window **99%**. The week switches later because its last percent is worth more than a whole 5-hour window, and if a turn hits a limit anyway, the rate-limit hook switches at once. Both can be set from 50 to 100 in `~/.claude-profiles/state.json`; these are the defaults:

```json
"thresholds": { "five_hour": 95, "seven_day": 99 }
```

A value outside 50 to 100 is ignored and the default is used.

## Commands

| Command | What it does |
|---|---|
| `cc-shift add <name>` | Creates a profile and opens Claude Code in it to run `/login` once. |
| `cc-shift setup` | Turns on automatic switching. Run once. |
| `cc-shift status` | Shows every account with its usage and the time until each limit resets, marks the active one, and names the next. |
| `cc-shift status --json` | The same as JSON, for scripts ([shape](#json-for-scripts)). |
| `cc-shift use <name>` | Moves every open window to that account now. Never needed; for steering by hand. |
| `cc-shift list` | Prints the profile names. |
| `cc-shift refresh` | Refreshes the logins of the accounts that are not active and reads their usage now. Runs on its own as well. |
| `cc-shift doctor` | Checks the setup (status line and hook, `claude`, every login and account, two profiles on one account, the state, a left-over early start job) and says how to fix what is wrong. |
| `cc-shift log` | Shows the history: switches with their reason, refreshes, logged-out accounts and when all were full ([history](#history)). |
| `cc-shift stats` | Shows the tokens of all accounts together per project and model ([token statistics](#token-statistics)). |
| `cc-shift remove <name>` | Deletes a profile and its stored login, after you type `yes`. The active account cannot be removed. |
| `cc-shift uninstall [--all]` | Undoes `cc-shift setup`; with `--all`, also deletes every profile and its stored login. |
| `cc-shift completion zsh` | Prints the zsh completion script; `cc-shift completion powershell` prints the PowerShell one ([how to load them](#shell-completion)). |
| `cc-shift --version` | Prints the version (also `-V` and `cc-shift version`). |
| `cc-shift help` | Prints the commands. |

Claude Code itself runs `cc-shift statusline` after every reply and `cc-shift hook rate-limit` when a turn hits a limit; you never run those.

### JSON for scripts

`cc-shift status --json` prints the same overview as JSON, for scripts, widgets and menu bar apps. Times are RFC 3339 in UTC; no token or email address is included.

```json
{
  "active": "work",
  "next": "spare",
  "free": 2,
  "total": 4,
  "first_free": null,
  "accounts": [
    {
      "name": "work",
      "state": "active",
      "free_at": null,
      "reason": null,
      "five_hour": { "used_percent": 23.0, "resets_at": "2026-10-15T18:00:00Z", "seconds_left": 15720 },
      "seven_day": { "used_percent": 67.0, "resets_at": "2026-10-19T16:00:00Z", "seconds_left": 352800 },
      "confirmed": true
    }
  ]
}
```

`state` is one of `active`, `next`, `ready`, `full`, `week_full`, `logged_out` and `unknown`; `free_at` is set for `full` and `week_full`, and `reason` for `unknown`. A window that has not started is `null`. A change to this shape is a changelog entry.

## Token statistics

`cc-shift stats` shows how many tokens all your accounts used together, per project and per model. The accounts work as one, so the totals are what count. `cc-shift status` stays about the accounts.

```text
$ cc-shift stats --cost
cc-shift stats · today · all accounts together · 14 sessions · 312 responses

  PROJECT       OUTPUT   SHARE                  INPUT   CACHE WRITE  CACHE READ   ≈ API COST
  backend-api    410K    ███████░░░░░░░░  45%   0.5M    2.9M         17.6M        $41.20
  website        280K    █████░░░░░░░░░░  31%   0.3M    2.0M         12.4M        $27.90
  infra          150K    ██░░░░░░░░░░░░░  16%   0.2M    1.1M          7.0M        $14.10
  scratch         70K    █░░░░░░░░░░░░░░   8%   0.1M    0.4M          2.8M         $5.50
  total          910K                           1.1M    6.4M         39.8M        $88.70

  MODEL               OUTPUT   SHARE
  claude-opus-5-5      843K    ███████████████░  93%
  claude-sonnet-5-5     67K    █░░░░░░░░░░░░░░░   7%
```

Rows are sorted and shared by output. The four kinds of tokens stand side by side and are never added into one total, because cache reads outnumber everything else.

| Option | What it does |
|---|---|
| `--since 2h`, `--since 7d`, `--since 30d` | Counts the last hours, or today and the days before it, instead of today. With days, a section per day follows, today marked. |
| `--all` | Counts everything in the transcripts, with a section per day. |
| `--project <name>` | Shows one project, by the name in the table, per model with the four kinds and per day. |
| `--cost` | Adds `≈ API COST`. |
| `--per-account` | Adds a block per account: each response counts for the account that was active at its time, by the switches in `cc-shift log`. Output from before the history began is shown apart, not split. |
| `--json` | Prints one JSON document: the period, sessions, responses, and per project, model and day the four kinds (and the cost with `--cost`, the accounts with `--per-account`). |

The options combine freely, except `--since` with `--all`.

How it counts:

- It reads the session transcripts Claude Code writes under `~/.claude/projects` and under `projects` in every profile, subagent transcripts included. Of each response it reads the model, the token counts, the time, the session and the working folder, never the content.
- Each response counts once, by its message id, also when a resumed session repeats earlier ones.
- A project is its working folder, shown by name. Two folders with the same name get their parent folder added, and a Claude Code worktree counts toward its project.
- Days and "today" are in your local time zone.
- What it read is kept in `~/.claude-profiles/stats-cache.json` per file, so the next run reads only the files that changed. The cache holds counts, models, times, session ids and folder paths, never content.

The cost is a comparison, not a bill. It is what the same tokens would cost at the Claude API's standard prices from the [pricing page](https://platform.claude.com/docs/en/about-claude/pricing), with cache writes at their 5-minute or 1-hour price as Claude Code recorded them. Your subscription is not billed per token. A model without a known price shows `?`, is left out of the total, and is named under the table. A weekly workflow compares the pricing page with the table in `cc-shift` and opens an issue when a price changes or a model is added.

## History

`cc-shift log` shows what `cc-shift` did: every switch with its reason, refreshed and logged-out logins, and when all accounts were full. Early starts from versions up to 0.1.3 stay in it.

```text
cc-shift history · last 24 hours

  TIME          EVENT        DETAILS
  today 09:02   setup        work active · status line and hook installed
  today 11:47   switch       work → spare · 5h at 95% · spare had 10% used
  today 14:30   switch       spare → extra · limit hit (hook)
  today 16:40   all full     staying on extra · work frees up in 1h10m
  today 17:50   free again   work is free again after 1h10m
  today 18:05   use          work → spare · by hand

  24 hours · 3 switches · all accounts full 1× for 1h10m
```

`--since 2h`, `7d` or `30d` and `--all` choose the period; `--verbose` adds diagnostic events, such as a status line reading that was recorded under another account; `--json` prints one JSON object per line. The history lives in `~/.claude-profiles/history.jsonl`, rotates at about 1 MB and never holds a token or an email address.

## Security

- Logins stay where Claude Code keeps them: in the login Keychain on macOS, in `.credentials.json` files with mode 0600 on Linux, and in `.credentials.json` files under your user folder's access rules on Windows. `cc-shift` holds a token in memory only while it uses it, and never writes one to a log, the terminal or a file of its own.
- A login is moved, never copied: a refresh token works once, so each login has exactly one live copy at any time.
- A token is never a command-line argument. Keychain writes and HTTP requests get it on stdin, so `ps` cannot show it. On Linux and Windows a login is written to a new file that replaces the old one in one step.
- `cc-shift` refreshes only the parked logins. Claude Code refreshes the active one, and a switch holds Claude Code's own refresh lock so the two never overlap.
- The refresh and usage requests are the ones Claude Code itself makes. Anthropic does not document them, so a Claude Code update can change them; `cc-shift` then shows that account's usage as unknown instead of failing.

Report a vulnerability as described in [SECURITY.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/SECURITY.md).

## Update and uninstall

Update to the latest version with `cargo install cc-shift`, or download the new [prebuilt binary](#prebuilt-binary) over the old one. Then run `cc-shift setup` again: it brings the settings up to date and removes the scheduled early start job of versions up to 0.1.3.

To uninstall:

```sh
cc-shift uninstall --all   # removes the status line, the hook, every profile and its login
cargo uninstall cc-shift   # or delete the binary from ~/.local/bin
```

`cc-shift uninstall` without `--all` only removes the status line and the hook (and puts back a status line you had before), and keeps the profiles. `--all` asks you to type `yes` first. The login in `~/.claude` stays, so Claude Code keeps working with the account that was active.

## FAQ

**Does switching restart Claude Code?** No. Every open window continues on the new account, with the same conversation.

**Does it work with Team accounts?** Yes. Team, Pro and Max accounts all report their usage to the status line.

**Does it work on Linux or Windows?** Yes, with the same commands. Logins are in `.credentials.json` files, and open windows follow a switch with their next message. Inside WSL 2, Claude Code is the Linux build, so use the Linux binary there; `where.exe claude` in PowerShell finds a native Windows Claude Code, `command -v claude` in WSL the Linux one.

**Windows says Claude Code keeps logins in Credential Manager.** Anthropic can switch a Windows account to Windows Credential Manager (or `CLAUDE_CODE_FORCE_WINDOWS_CREDMAN=1` does). Claude Code then reads Credential Manager first and would ignore a moved file, so `cc-shift` stops instead of switching, and `cc-shift doctor` names the account.

**An account shows `logged out`.** Its login was revoked or expired. Run `cc-shift add <name>` and `/login` again.

**An account shows `unknown`.** Its usage has not been read yet, or the usage request failed. `cc-shift refresh` reads it now.

**What happens when every account is full?** `cc-shift` stays on the current account, and `cc-shift status` shows which account frees up first and when.

**Something does not work.** Run `cc-shift doctor`: it checks every part of the setup and prints the command that fixes each problem.

## Contributing

Issues and pull requests are welcome on [GitHub](https://github.com/Vernum-Projecten/cc-shift). [CONTRIBUTING.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/CONTRIBUTING.md) has the rules for working in the repository and how a release is made. Before a pull request:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked
cargo deny check
```

The tests never touch your real Keychain, profiles or accounts: they run against a temporary `HOME`, a stub `claude`, an in-memory login store and a fake HTTP client.

## License

[Apache License 2.0](https://github.com/Vernum-Projecten/cc-shift/blob/main/LICENSE). Copyright 2026 Vernum Projecten B.V.; see [NOTICE](https://github.com/Vernum-Projecten/cc-shift/blob/main/NOTICE).
