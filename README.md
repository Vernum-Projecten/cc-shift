# cc-shift

[![CI](https://github.com/Vernum-Projecten/cc-shift/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/Vernum-Projecten/cc-shift/actions/workflows/ci.yml)
[![Platform: macOS](https://img.shields.io/badge/platform-macOS-lightgrey?logo=apple)](#requirements)
[![Rust 1.98](https://img.shields.io/badge/rust-1.98-orange?logo=rust)](rust-toolchain.toml)
[![Edition 2024](https://img.shields.io/badge/edition-2024-orange)](Cargo.toml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue)](LICENSE)

`cc-shift` switches Claude Code between several Claude accounts on its own, so you can keep working without thinking about usage limits.

Every Claude Code window (RustRover, a terminal) keeps running as normal. When the account in use gets close to its 5-hour or 7-day limit, `cc-shift` moves every open window to the next account, the same way `/login` does. There is no restart, and the conversation, its history and the working directory stay as they are.

> [!IMPORTANT]
> `cc-shift` works on **macOS only**. It keeps logins in the macOS login Keychain through `/usr/bin/security`, the same way Claude Code does on a Mac.

## Contents

- [How it works](#how-it-works)
- [Requirements](#requirements)
- [Install](#install)
- [Set up](#set-up)
- [Commands](#commands)
- [The status line](#the-status-line)
- [Which account is next](#which-account-is-next)
- [Security](#security)
- [Development](#development)
- [License](#license)

## How it works

```
~/.claude-profiles/work      parked login of the account "work"
~/.claude-profiles/spare     parked login of the account "spare"
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder. Its login stays parked in that profile's Keychain item.
- Every Claude Code window runs with the default `~/.claude`, which holds the active account.
- A switch moves the next account's login into the Keychain item of `~/.claude` and copies its account details into `~/.claude.json`. Open windows pick both up, as they do after `/login`.
- A login is moved, never copied: a refresh token works once, so each login has exactly one live copy at any time.

Claude Code calls `cc-shift` after every reply through its status line, which records the active account's usage and switches at 95%. A rate-limit hook is the fallback when a turn hits a limit anyway. The full design is in [ARCHITECTURE.md](ARCHITECTURE.md).

## Requirements

| What | Version |
|---|---|
| macOS | any version with the login Keychain and `/usr/bin/curl` |
| [Claude Code](https://code.claude.com/docs) | 2.1.300 or later, signed in with a Pro, Max or Team account |
| Rust | 1.98 (pinned in [`rust-toolchain.toml`](rust-toolchain.toml)), to build `cc-shift` |

## Install

```sh
git clone https://github.com/Vernum-Projecten/cc-shift.git
cd cc-shift
cargo install --path . --locked
```

This puts `cc-shift` in `~/.cargo/bin`. Run `cc-shift help` to check it is on your `PATH`.

## Set up

1. **Log in each account once.** For every account:

   ```sh
   cc-shift add work
   ```

   Claude Code opens. Run `/login`, sign in with that account, then quit. Use a private browser window per account, or sign out of the previous account in the browser first, so you pick the right one.

2. **Turn on automatic switching:**

   ```sh
   cc-shift setup
   ```

   This adds the status line and the rate-limit hook to `~/.claude/settings.json`, keeps every other setting, and makes the best account active. Your own status line, if you had one, keeps working and is shown first.

3. **Check it:** send a message in any Claude Code window, then run `cc-shift status`.

From here on you never need to run `cc-shift` while you work.

## Commands

| Command | What it does |
|---|---|
| `cc-shift add <name>` | Creates a profile and opens Claude Code in it to run `/login` once. |
| `cc-shift setup` | Installs the status line and the hook once and makes the best account active. Running it again changes nothing. |
| `cc-shift status` | Shows every account with its usage and the time until each limit resets, marks the active one, and names the next. |
| `cc-shift use <name>` | Makes an account active in every open window now. Never needed; for steering by hand. |
| `cc-shift refresh` | Refreshes parked logins and reads their usage now. `cc-shift status` and the status line do this on their own. |
| `cc-shift list` | Prints the profile names. |
| `cc-shift statusline` | Run by Claude Code after every reply. |
| `cc-shift hook rate-limit` | Run by Claude Code when a turn hits a usage limit. |

`cc-shift status` looks like this:

```text
  ACCOUNT  5-HOUR            7-DAY
  spare    9% 4h3m           64% 3d13h
* work     60% 3h33m         31% 5d7h

next: spare
```

## The status line

Under the Claude Code prompt you see the active account and both limits with the time until they reset:

```text
work · 5h 60% (3h33m) · 7d 31% (5d7h)
```

## Which account is next

Usage left in a window when it resets is lost, and a 5-hour window only starts with an account's first message. So `cc-shift` spends what expires first:

1. An account whose 5-hour window is already running, the one that resets soonest first.
2. Then an untouched account, the one whose week resets soonest first, then the one with the most of its week left.

It stays on the active account until it reaches 95%, so it never switches back and forth. One account is used up at a time, and by the time the others are used, the first has reset.

## Security

- Tokens stay in the macOS login Keychain. `cc-shift` holds them in memory only while it uses them, and never writes them to a file, a log, the terminal or its state file.
- A token is never a command-line argument: Keychain writes and HTTP requests get it on stdin, so `ps` cannot show it.
- `cc-shift` refreshes only parked logins, with a lock held for the whole request. Claude Code refreshes the active one.
- The refresh and usage requests are the ones Claude Code itself makes. Anthropic does not document them, so they can change with a Claude Code update; `cc-shift` then shows that account's usage as unknown instead of failing.

Report a vulnerability as described in [SECURITY.md](SECURITY.md).

## Development

The rules for working in this repository are in [CONTRIBUTING.md](CONTRIBUTING.md) and [CLAUDE.md](CLAUDE.md). Before a pull request:

```sh
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo nextest run --locked
cargo test --doc --locked
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items
cargo deny check
scripts/checks/comment-style.sh --all
scripts/checks/file-length.sh
```

The tests never touch your real Keychain, profiles or accounts: they run against a temporary `HOME`, a stub `claude`, an in-memory Keychain and a fake HTTP client.

## License

[Apache License 2.0](LICENSE).
