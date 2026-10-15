# cc-shift: automatic Claude account switching (macOS)

## 1. Purpose

`cc-shift` lets one person use several Claude accounts with Claude Code without ever thinking about usage limits. Every Claude Code window (RustRover, a terminal) keeps running as normal. When the account in use gets close to its limit, `cc-shift` moves the windows to the next account, the same way `/login` does: no restart, no lost conversation.

The accounts are several work accounts, each with a profile of its own. The examples below use invented profile names.

## 2. How it works

Claude Code keeps its login in the macOS Keychain, in one item per config folder (<https://code.claude.com/docs/en/authentication#credential-management>). A new session reads that item when it starts. A running session re-reads that item before each request, with a 30-second cache, so it picks up a login written there while it runs; the owner saw a `/login` in one window reach the others (#12), and the Claude Code 2.1.300 binary shows how (#50).

`cc-shift` builds on that:

```
~/.claude-profiles/member1   parked login of member1
~/.claude-profiles/member2   parked login of member2
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder. That login stays parked in the profile's Keychain item.
- Every window runs with the default `~/.claude`. Its Keychain item holds the active account.
- A switch moves the next account's parked login into the item of `~/.claude`, and copies that account's details (`oauthAccount` in the profile's `.claude.json`) into `~/.claude.json`. `/login` writes both, and open windows follow only when both change (observed on Claude Code 2.1.300, #24). Every open window continues on the new account within about 30 seconds.

A refresh token works once: after a refresh the old one is dead, and presenting a dead copy can revoke the login. So a login is always moved, never copied, and each login has exactly one live copy and one process that refreshes it: Claude Code for the active login, `cc-shift` for the parked ones.

## 3. Commands

| Command | What it does |
|---|---|
| `cc-shift add <name>` | Creates the profile and opens Claude Code in it to run `/login` once. |
| `cc-shift setup` | Installs the status line and the hook in `~/.claude/settings.json` and makes the best account active. Run once. |
| `cc-shift status` | Shows every account in the order of use, with both windows as bars, their reset times and each account's state. |
| `cc-shift status --json` | The same overview as JSON, for scripts. |
| `cc-shift use <name>` | Makes an account active now. Never needed; for steering by hand. |
| `cc-shift list` | Prints the profile names. |
| `cc-shift refresh` | Refreshes parked logins and reads parked accounts' usage now. `cc-shift status` and the status line run it on their own. |
| `cc-shift doctor` | Checks the setup and prints the command that fixes each problem. |
| `cc-shift remove <name>` | Deletes a profile and its Keychain login, after confirmation; never the active account. |
| `cc-shift uninstall [--all]` | Undoes `cc-shift setup`; with `--all` also deletes every profile and its login. |
| `cc-shift completion zsh` | Prints the zsh completion script. |
| `cc-shift statusline` | Called by Claude Code after every reply. Not run by hand. |
| `cc-shift hook rate-limit` | Called by Claude Code when a turn hits a usage limit. Not run by hand. |

After `cc-shift add` for each account and one `cc-shift setup`, `cc-shift` tracks every account and switches on its own.

A profile name uses ASCII letters, digits, `-` and `_`, starts with a letter or digit, and is not a command word (`add`, `setup`, `status`, `use`, `list`, `statusline`, `hook`, `help`, `remove`, `refresh`, `doctor`, `uninstall`, `completion`).

## 4. Automatic switching

### What `cc-shift` knows per account

For both usage windows Claude Code reports, the 5-hour and the 7-day window: the percentage used and the time it resets. An account is **available** when it is logged in and both windows are below the switch threshold, 95%. A window whose reset time has passed counts as 0%.

### Where the numbers come from

- **The active account:** Claude Code passes `rate_limits` to the status line command after every reply, with `five_hour` and `seven_day`, each a `used_percentage` and a `resets_at` (<https://code.claude.com/docs/en/statusline>). No token is needed. The docs say this is sent for Pro and Max accounts.
- **A parked account:** `cc-shift` sends its access token to Anthropic's usage endpoint, `GET https://api.anthropic.com/api/oauth/usage`, which returns the same two windows. The endpoint is not documented and has changed shape before, so `cc-shift` accepts both known shapes.

A source that fails or returns an unknown shape makes that account "unknown". It is shown as such and chosen last, and never stops `cc-shift` as a whole (#8).

### Which account is next

`cc-shift` spends what expires first (owner decision, #36). Usage left in a window when it resets is lost, and the 5-hour window only starts with an account's first message, so an untouched account loses nothing by waiting. Among the available accounts:

1. An account whose 5-hour window is running goes first, the one that resets soonest first. Its remaining usage is the next to be lost.
2. Then an account with no running 5-hour window, the one whose 7-day window resets soonest first, then the one with the most 7-day usage left. This spreads the weekly limits, so no account runs out of its week early.
3. Then the name, so the choice is the same every run.

`cc-shift` stays on the active account until it reaches the threshold, so it never switches back and forth. The effect is that one account is drained at a time and a new 5-hour clock starts only when it is needed; by the time the others are used, the first has reset. If none is available, `cc-shift` stays on the current account and `cc-shift status` shows when the first one resets.

### When it switches

A running Claude Code session re-reads its login from the Keychain before each request (with a 30-second cache) and after a 401, and before it refreshes a token it re-reads the Keychain under a lock and takes over a login it finds there instead of refreshing over it (Claude Code 2.1.300, read from the binary, #50). So every open window follows a switch within about 30 seconds, and a window never refreshes over a login `cc-shift` moved in.

The status line reports the usage of the window's last response. Right after a switch that response was still made on the account just left, so a reading cannot be trusted by when it arrives. On 2026-10-15 this made the switches cascade: the old account's numbers were recorded under the new one, which then looked full (#50). So:

- **Readings are matched to accounts.** Every account's windows reset at their own times, so a status line reading belongs to the account whose confirmed reading (from the usage endpoint, with its own token) has the same reset times. A reading that matches no confirmed account goes to the active account only when that account has no confirmed reading yet, and is dropped otherwise.
- **Before the limit:** when a reading proven to be the active account's reaches 95% in either window, the status line switches to the next account, between two replies.
- **At the limit:** Claude Code runs a `StopFailure` hook with matcher `rate_limit` when a turn ends on a usage limit (<https://code.claude.com/docs/en/hooks>). `cc-shift hook rate-limit` records the active account as full and switches at once.
- **No switch follows another within five minutes,** so a reading or a hook left over from before a switch cannot start a second one.
- **The account details are put right.** A window can write its old account details back into `~/.claude.json`, so each proven reading of the active account sets them again.

The status line names the account the window really uses. Right after a switch it shows `moving to <account>` until the window's next request runs on the new login.

The status line prints at once. Slow work (refreshing a parked login, reading an account's usage when its numbers are older than five minutes or not yet confirmed, the active account included) runs in a detached `cc-shift refresh`. Only one runs at a time.

### Several windows

All windows share the active account, so a switch moves all of them, as `/login` does. Every switch and every write of the state takes one lock, and the five-minute pause after a switch keeps a second window from starting another.

## 5. Logins and tokens

### A switch: `cc-shift use member2`

Under the lock, in this order:

1. Save the login now in the item of `~/.claude` back to the parked item of the account that was active. Claude Code may have refreshed it, so this keeps the live tokens.
2. Write `member2`'s parked login into the item of `~/.claude`, and set `oauthAccount` in `~/.claude.json` to `member2`'s, changing nothing else in that file.
3. Clear `member2`'s parked item and record `member2` as active in the state.

The state records each step before it runs, so a switch that stops halfway is finished by the next `cc-shift` run, and no login is lost or left with two copies. Steps 1 and 2 are recorded as one step: run again after a stop between them, it saves `~/.claude` once more, so tokens Claude Code refreshed in the meantime are kept, and it skips both when `~/.claude` already holds the next login.

At `cc-shift setup` the login already in `~/.claude` has no profile. It is a separate login from the parked ones, so replacing it breaks nothing; `cc-shift setup` says so before it does.

### Refreshing parked logins

A parked login has no Claude Code process to refresh it, so `cc-shift` does. When a parked access token is close to expiry, `cc-shift` takes the lock, sends the refresh token to Anthropic's OAuth token endpoint, and writes the new tokens back to the same item at once. `cc-shift` never refreshes the active login: Claude Code does that. A refused refresh (a revoked login) marks the account logged out; `cc-shift add <name>` and `/login` fix it. Any other failure leaves the item as it was.

## 6. Where things are stored

| What | Where | Written by |
|---|---|---|
| The `cc-shift` binary | `~/.cargo/bin/cs` | cargo |
| One folder per profile, used for its `/login` | `~/.claude-profiles/<name>/` | cc-shift, Claude Code |
| Parked logins | Keychain, the item of each profile folder | Claude Code at `/login`; cc-shift on a switch or refresh |
| The active login | Keychain, `Claude Code-credentials` (the item of `~/.claude`) | Claude Code; cc-shift on a switch |
| Active account, usage per account, unfinished switch | `~/.claude-profiles/state.json` | cc-shift |
| The lock | `~/.claude-profiles/.lock` | cc-shift |
| Status line and hook | `~/.claude/settings.json` | cc-shift setup |

`cc-shift list` treats every folder in `~/.claude-profiles/` as a profile, so the state and lock are files there. A profile's item is `Claude Code-credentials-` followed by the first 8 hex characters of the SHA-256 of its folder's absolute path, with the macOS user name as the account (observed on Claude Code 2.1.300, #6).

## 7. Code

A small library with a thin binary: `src/main.rs` only calls `cs::run` in `src/lib.rs`, so everything is testable from `tests/`. The modules follow the domain, one folder each, with a `mod.rs` that holds the folder's module documentation and the items its files share:

```
src/
├── main.rs, lib.rs, error.rs
├── command/      the command line: parse, and one file per command
│                 (add, list, activate for cc-shift use, setup, status, refresh,
│                 statusline, hook); mod.rs dispatches and holds the helpers
├── store/        the files cc-shift owns: profile (names, the profiles folder),
│                 state (state.json behind the lock, written in one rename)
├── usage/        mod.rs: the two windows per account, the selection view
│                 and time left; select: the selection rule; overview:
│                 every account's state in the order of use
├── render/       the status table and the status line, with bars and
│                 colour (NO_COLOR is honoured)
├── login/        mod.rs: Login, Slot and the Logins trait every Keychain
│                 access goes through; keychain: the macOS backend through
│                 /usr/bin/security; account: oauthAccount in .claude.json
├── switching/    transfer: the move of section 5 with its recovery;
│                 auto: when to switch on its own and where to go
├── anthropic/    http: requests through curl with the request on stdin;
│                 oauth: the refresh and usage calls; parked: keeping
│                 parked accounts ready
└── claude_code/  settings (cc-shift setup), statusline (record and print),
                  launch (exec claude for cc-shift add)
```

Every import names the module that defines the item; there are no re-exports. A login never appears in debug output, and the Keychain and HTTP are behind traits, so tests use memory and fakes.

The runtime dependencies are `thiserror` for the error type, `serde` with `serde_json` (key order kept) for the JSON files, `sha2` for the Keychain service names, and `jiff` for the reset times; each further dependency is added in the pull request that needs it.

## 8. Errors

Every failure is one short line on stderr and a non-zero exit, and nothing is ignored silently. The exceptions are the two calls Claude Code makes: `cc-shift statusline` always prints a status line and `cc-shift hook rate-limit` always exits 0, so a `cc-shift` problem never breaks a Claude Code window. They report problems through `cc-shift status` instead.

## 9. Checking it by hand

These need real accounts, so they are done once by the owner and recorded on the issue with the Claude Code version:

- After `cc-shift add` for two accounts, Keychain Access shows one "Claude Code" item per profile (#6).
- With two windows open on `~/.claude`, `cc-shift use` to the other account, then `/status` in both windows shows the new account.
- The status line of the work accounts receives `rate_limits`.

## 10. Security

`cc-shift` touches tokens for three reasons only: to move a login on a switch, to refresh a parked login, and to read a parked access token for its usage. Tokens stay in the Keychain, encrypted at rest. `cc-shift` holds them in memory only while it uses them, and never writes them to a file, a log, the terminal or the state. It never refreshes the active login and never keeps two live copies of one login. The owner asked for this on #12.

Removing an account means deleting its profile folder and its "Claude Code" Keychain item; deleting only the folder leaves the login in the Keychain. A later `cc-shift remove <name>` does both.
