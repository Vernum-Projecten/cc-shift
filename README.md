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
work · 5h 60% (3h33m) · 7d 31% (5d7h)
```

## Contents

- [Requirements](#requirements)
- [Install](#install)
- [Set up](#set-up)
- [Commands](#commands)
- [How it works](#how-it-works)
- [Which account is next](#which-account-is-next)
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
  ACCOUNT  5-HOUR            7-DAY
  spare    9% 4h3m           64% 3d13h
* work     60% 3h33m         31% 5d7h

next: spare
```

From here on you never need to run `cc-shift` while you work.

## Commands

| Command | What it does |
|---|---|
| `cc-shift add <name>` | Creates a profile and opens Claude Code in it to run `/login` once. |
| `cc-shift setup` | Turns on automatic switching. Run once. |
| `cc-shift status` | Shows every account with its usage and the time until each limit resets, marks the active one, and names the next. |
| `cc-shift use <name>` | Moves every open window to that account now. Never needed; for steering by hand. |
| `cc-shift refresh` | Refreshes the logins of the accounts that are not active and reads their usage now. Runs on its own as well. |
| `cc-shift list` | Prints the profile names. |
| `cc-shift help` | Prints the commands. |

Claude Code itself runs `cc-shift statusline` after every reply and `cc-shift hook rate-limit` when a turn hits a limit; you never run those.

## How it works

```text
~/.claude-profiles/work      parked login of the account "work"
~/.claude-profiles/spare     parked login of the account "spare"
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder, and its login stays parked there in the Keychain.
- Every Claude Code window uses the default `~/.claude`, which holds the active account.
- After every reply, the status line records the active account's usage. When the 5-hour or the 7-day window reaches 95%, `cc-shift` moves the next account's login into `~/.claude` and copies its account details into `~/.claude.json`. Open windows pick both up, as they do after `/login`.
- If a turn hits a limit anyway, the rate-limit hook switches at once.
- In the background, `cc-shift` refreshes the parked logins and reads their usage, so the next choice is made on current numbers.

The full design is in [ARCHITECTURE.md](https://github.com/Vernum-Projecten/cc-shift/blob/main/ARCHITECTURE.md).

## Which account is next

Usage left in a window when it resets is lost, and a 5-hour window only starts with an account's first message. So `cc-shift` spends what expires first:

1. An account whose 5-hour window is already running, the one that resets soonest first.
2. Then an untouched account, the one whose week resets soonest first, then the one with the most of its week left.

It stays on the active account until it reaches 95%, so it never switches back and forth. You use up one account at a time, and by the time you have used the others, the first one has reset.

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

To uninstall, remove `cc-shift`'s `statusLine` and its `StopFailure` hook from `~/.claude/settings.json`, then:

```sh
cargo uninstall cc-shift
rm -r ~/.claude-profiles
```

Removing `~/.claude-profiles` leaves the parked logins in the Keychain. Delete the "Claude Code-credentials-…" items in Keychain Access to remove them too.

## FAQ

**Does switching restart Claude Code?** No. Every open window continues on the new account, with the same conversation.

**Does it work with Team accounts?** Yes. Team, Pro and Max accounts all report their usage to the status line.

**An account shows `logged out`.** Its login was revoked or expired. Run `cc-shift add <name>` and `/login` again.

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

## License

[Apache License 2.0](https://github.com/Vernum-Projecten/cc-shift/blob/main/LICENSE). Copyright 2026 Vernum-Projecten; see [NOTICE](https://github.com/Vernum-Projecten/cc-shift/blob/main/NOTICE).
