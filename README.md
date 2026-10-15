# cc-shift

[![crates.io](https://img.shields.io/crates/v/cc-shift?logo=rust)](https://crates.io/crates/cc-shift)
[![Downloads](https://img.shields.io/crates/d/cc-shift)](https://crates.io/crates/cc-shift)
[![CI](https://github.com/Vernum-Projecten/cc-shift/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Vernum-Projecten/cc-shift/actions/workflows/ci.yml)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey?logo=apple)](#requirements)
[![Rust 1.98+](https://img.shields.io/badge/rust-1.98%2B-orange?logo=rust)](https://github.com/Vernum-Projecten/cc-shift/blob/main/rust-toolchain.toml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](https://github.com/Vernum-Projecten/cc-shift/blob/main/LICENSE)

**Use several Claude accounts with Claude Code, and never think about usage limits again.**

`cc-shift` watches the usage of the account Claude Code is signed in to. When it gets close to its 5-hour or 7-day limit, `cc-shift` moves every open Claude Code window to your next account, the same way `/login` does. There is no restart: the conversation, its history and the working directory stay as they are.

> [!IMPORTANT]
> `cc-shift` works on **macOS only**. It keeps logins in the macOS login Keychain, the same way Claude Code does on a Mac.

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
- [Commands](#commands)
- [Token statistics](#token-statistics)
- [How it works](#how-it-works)
- [Which account is next](#which-account-is-next)
- [Early start](#early-start)
- [Security](#security)
- [Update and uninstall](#update-and-uninstall)
- [FAQ](#faq)
- [Contributing](#contributing)
- [License](#license)

## Requirements

| What | Version |
|---|---|
| macOS | Apple Silicon or Intel |
| [Claude Code](https://code.claude.com/docs) | 2.1.300 or later |
| Claude accounts | Pro, Max or Team, two or more |
| Rust | 1.98 or later, to build `cc-shift` ([install Rust](https://rustup.rs)) |

## Install

### From crates.io (recommended)

```sh
cargo install cc-shift
```

This builds `cc-shift` and puts it in `~/.cargo/bin`. Check that it is on your `PATH`:

```sh
cc-shift help
```

### From source

```sh
git clone https://github.com/Vernum-Projecten/cc-shift.git
cd cc-shift
cargo install --path . --locked
```

### Shell completion

To complete the commands and profile names with Tab in zsh, add this line to `~/.zshrc`, after `compinit`:

```sh
source <(cc-shift completion zsh)
```

## Set up

**1. Add each account once.** For every account:

```sh
cc-shift add work
```

Claude Code opens in that account's own profile. Run `/login`, sign in with that account, then quit Claude Code. To be sure you pick the right account, use a private browser window per account, or sign out of the previous account in the browser first.

Profile names use letters, digits, `-` and `_`, and start with a letter or digit.

**2. Turn on automatic switching:**

```sh
cc-shift setup
```

This adds a status line and a rate-limit hook to `~/.claude/settings.json` and keeps every other setting. A status line you already had keeps working and is shown first. `cc-shift setup` then makes your best account active. Running it again changes nothing.

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

From here on you never need to run `cc-shift` while you work.

## Commands

| Command | What it does |
|---|---|
| `cc-shift add <name>` | Creates a profile and opens Claude Code in it to run `/login` once. |
| `cc-shift setup` | Turns on automatic switching. Run once. |
| `cc-shift status` | Shows every account with its usage and the time until each limit resets, marks the active one, and names the next. |
| `cc-shift status --json` | The same as JSON, for scripts ([shape](#json-for-scripts)). |
| `cc-shift stats` | Shows the tokens of all accounts together per project and model ([token statistics](#token-statistics)). |
| `cc-shift use <name>` | Moves every open window to that account now. Never needed; for steering by hand. |
| `cc-shift refresh` | Refreshes the logins of the accounts that are not active and reads their usage now. Runs on its own as well. |
| `cc-shift list` | Prints the profile names. |
| `cc-shift completion zsh` | Prints the zsh completion script ([how to load it](#shell-completion)). |
| `cc-shift --version` | Prints the version (also `-V` and `cc-shift version`). |
| `cc-shift doctor` | Checks the setup (status line and hook, `claude`, every login and account, two profiles on one account, the state) and says how to fix what is wrong. |
| `cc-shift log` | Shows the history: switches with their reason, refreshes, logged-out accounts and when all were full. |
| `cc-shift early on` | Starts some accounts' 5-hour windows before the workday, Monday to Friday ([early start](#early-start)). `cc-shift early off` stops it; `cc-shift early` shows it. |
| `cc-shift remove <name>` | Deletes a profile and its Keychain login, after you type `yes`. The active account cannot be removed. |
| `cc-shift uninstall [--all]` | Undoes `cc-shift setup`; with `--all`, also deletes every profile and its Keychain login. |
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
- A project is its working folder, shown by name. Two folders with the same name get their parent folder added.
- Days and "today" are in your local time zone.
- What it read is kept in `~/.claude-profiles/stats-cache.json` per file, so the next run reads only the files that changed. The cache holds counts, models, times, session ids and folder paths, never content.

The cost is a comparison, not a bill. It is what the same tokens would cost at the Claude API's standard prices from the [pricing page](https://platform.claude.com/docs/en/about-claude/pricing), with cache writes at their 5-minute or 1-hour price as Claude Code recorded them. Your subscription is not billed per token. A model without a known price shows `?`, is left out of the total, and is named under the table. A weekly workflow compares the pricing page with the table in `cc-shift` and opens an issue when a price changes or a model is added.

## How it works

```text
~/.claude-profiles/work      parked login of the account "work"
~/.claude-profiles/spare     parked login of the account "spare"
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder, and its login stays parked there in the Keychain.
- Every Claude Code window uses the default `~/.claude`, which holds the active account.
- After every reply, the status line records the active account's usage. When the 5-hour window reaches 95% or the 7-day window 99% (both [configurable](#switch-thresholds)), `cc-shift` moves the next account's login into `~/.claude` and copies its account details into `~/.claude.json`. Open windows pick both up, as they do after `/login`.
- If a turn hits a limit anyway, the rate-limit hook switches at once.
- In the background, `cc-shift` refreshes the parked logins and reads their usage, so the next choice is made on current numbers.

The full design is in [ARCHITECTURE.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/ARCHITECTURE.md).

## History

`cc-shift log` shows what `cc-shift` did: every switch with its reason, refreshed and logged-out logins, and when all accounts were full.

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

## Switch thresholds

`cc-shift` switches when the 5-hour window reaches **95%** or the 7-day window **99%**. The week switches later because its last percent is worth more than a whole 5-hour window, and if a turn hits a limit anyway, the rate-limit hook switches at once. Both can be set from 50 to 100 in `~/.claude-profiles/state.json`:

```json
"thresholds": { "five_hour": 95, "seven_day": 100 }
```

A value outside 50 to 100 is ignored and the default is used.

## Which account is next

Usage left in a window when it resets is lost, and a 5-hour window only starts with an account's first message. So `cc-shift` spends what expires first:

1. An account whose 5-hour window is already running, the one that resets soonest first.
2. Then an untouched account, the one whose week resets soonest first, then the one with the most of its week left.

It stays on the active account until it reaches its threshold, so it never switches back and forth. You use up one account at a time, and by the time you have used the others, the first one has reset.

## Early start

A 5-hour window starts with an account's first message and resets five hours later. If a window starts at 06:00, it resets at 11:00, during your workday, and you get an extra window that day. `cc-shift` can start a few windows for you each morning. It is off until you turn it on:

```console
$ cc-shift early on --at 06:00 --accounts 2
Early start is on: Monday to Friday at 06:00, 2 accounts.
cc-shift picks the accounts each morning, the ones it would use first.
Each costs one short Haiku message.

$ cc-shift early
Early start: on · Monday to Friday at 06:00 · 2 accounts · next Fri 06:00

$ cc-shift early off
Early start is off.
```

- **Which accounts:** each morning, the first ones in the order above whose 5-hour window is not running. Full, out-of-week and logged-out accounts are skipped. Start only as many as your workday needs: every extra window spends the week sooner, and the accounts left alone stay fresh for the evening.
- **How:** a `launchd` agent runs `cc-shift early run` Monday to Friday at the set time. If your Mac is asleep then, it runs when the Mac wakes; if the Mac is off, that day is skipped.
- **The message:** Claude Code sends it itself, in the account's own profile, on Haiku, with no tools, hooks or MCP servers and no saved session. `cc-shift` touches no token for it.
- **Where to see it:** `cc-shift log` shows each start and each account that could not be started. `cc-shift status` shows `early start 06:00 · 2 accounts` under the table while it is on.

## Security

- Logins stay in the macOS login Keychain. `cc-shift` holds a token in memory only while it uses it, and never writes one to a file, a log or the terminal.
- A login is moved, never copied: a refresh token works once, so each login has exactly one live copy at any time.
- A token is never a command-line argument. Keychain writes and HTTP requests get it on stdin, so `ps` cannot show it.
- `cc-shift` refreshes only the parked logins. Claude Code refreshes the active one.
- The refresh and usage requests are the ones Claude Code itself makes. Anthropic does not document them, so a Claude Code update can change them; `cc-shift` then shows that account's usage as unknown instead of failing.

Report a vulnerability as described in [SECURITY.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/SECURITY.md).

## Update and uninstall

Update to the latest version:

```sh
cargo install cc-shift
```

To uninstall:

```sh
cc-shift uninstall --all   # removes the status line, the hook, every profile and its login
cargo uninstall cc-shift
```

`cc-shift uninstall` without `--all` only removes the status line and the hook (and puts back a status line you had before), and keeps the profiles. `--all` asks you to type `yes` first. The login in `~/.claude` stays, so Claude Code keeps working with the account that was active.

## FAQ

**Does switching restart Claude Code?** No. Every open window continues on the new account, with the same conversation.

**Does it work with Team accounts?** Yes. Team, Pro and Max accounts all report their usage to the status line.

**An account shows `logged out`.** Its login was revoked or expired. Run `cc-shift add <name>` and `/login` again.

**Something does not work.** Run `cc-shift doctor`: it checks every part of the setup and prints the command that fixes each problem.

**An account shows `unknown`.** Its usage has not been read yet, or the usage request failed. `cc-shift refresh` reads it now.

**What happens when every account is full?** `cc-shift` stays on the current account, and `cc-shift status` shows which account frees up first and when.

## Contributing

Issues and pull requests are welcome on [GitHub](https://github.com/Vernum-Projecten/cc-shift). The rules for working in the repository are in [CONTRIBUTING.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/CONTRIBUTING.md). Before a pull request:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --locked
cargo deny check
```

The tests never touch your real Keychain, profiles or accounts: they run against a temporary `HOME`, a stub `claude`, an in-memory Keychain and a fake HTTP client.

### Releasing

A release is one `v*` tag, and [`release.yml`](https://github.com/Vernum-Projecten/cc-shift/blob/main/.github/workflows/release.yml) does the rest:

1. **plan** checks that the tag names the version in `Cargo.toml`, that `publish` is on, and that [`CHANGELOG.md`](https://github.com/Vernum-Projecten/cc-shift/blob/main/CHANGELOG.md) has a section for the version, which becomes the release notes.
2. **github release** creates the GitHub release as a draft.
3. **binary** builds `cc-shift` for Apple silicon and Intel Macs, attaches each archive with its SHA-256 file to the draft, and attests its build provenance (`gh attestation verify <archive> --repo Vernum-Projecten/cc-shift`).
4. **crates.io** publishes through [Trusted Publishing](https://crates.io/docs/trusted-publishing): the workflow's GitHub identity is exchanged for a short-lived crates.io token, so no token is stored anywhere. A version already on crates.io is skipped.
5. **github release (publish)** publishes the draft once every archive is attached.

The crate's Trusted Publisher entry on crates.io names the repository `Vernum-Projecten/cc-shift`, the workflow `release.yml` and the environment `crates-io`.

## License

[Apache License 2.0](https://github.com/Vernum-Projecten/cc-shift/blob/main/LICENSE). Copyright 2026 Vernum Projecten B.V.; see [NOTICE](https://github.com/Vernum-Projecten/cc-shift/blob/main/NOTICE).
