# cs: automatic Claude account switching (macOS)

## 1. Purpose

`cs` lets one person use several Claude accounts with Claude Code without ever thinking about usage limits. Every Claude Code window (RustRover, a terminal) keeps running as normal. When the account in use gets close to its limit, `cs` moves the windows to the next account, the same way `/login` does: no restart, no lost conversation.

The accounts are several work accounts, each with a profile of its own. The examples below use invented profile names.

## 2. How it works

Claude Code keeps its login in the macOS Keychain, in one item per config folder (<https://code.claude.com/docs/en/authentication#credential-management>). A running session picks up a login written into its folder's item: on Claude Code 2.1.300 a `/login` in one window also applies to the other windows already open on that folder (observed by the owner, #12).

`cs` builds on that:

```
~/.claude-profiles/member1   parked login of member1
~/.claude-profiles/member2   parked login of member2
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder. That login stays parked in the profile's Keychain item.
- Every window runs with the default `~/.claude`. Its Keychain item holds the active account.
- A switch moves the next account's parked login into the item of `~/.claude`, and copies that account's details (`oauthAccount` in the profile's `.claude.json`) into `~/.claude.json`. `/login` writes both, and open windows follow only when both change (observed on Claude Code 2.1.300, #24). Every open window continues on the new account.

A refresh token works once: after a refresh the old one is dead, and presenting a dead copy can revoke the login. So a login is always moved, never copied, and each login has exactly one live copy and one process that refreshes it: Claude Code for the active login, `cs` for the parked ones.

## 3. Commands

| Command | What it does |
|---|---|
| `cs add <name>` | Creates the profile and opens Claude Code in it to run `/login` once. |
| `cs setup` | Installs the status line and the hook in `~/.claude/settings.json` and makes the best account active. Run once. |
| `cs status` | Shows every account with its usage and reset times, and marks the active one. |
| `cs use <name>` | Makes an account active now. Never needed; for steering by hand. |
| `cs list` | Prints the profile names. |
| `cs refresh` | Refreshes parked logins and reads parked accounts' usage now. `cs status` and the status line run it on their own. |
| `cs statusline` | Called by Claude Code after every reply. Not run by hand. |
| `cs hook rate-limit` | Called by Claude Code when a turn hits a usage limit. Not run by hand. |

After `cs add` for each account and one `cs setup`, the user never runs `cs` while working.

A profile name uses ASCII letters, digits, `-` and `_`, starts with a letter or digit, and is not a command word (`add`, `setup`, `status`, `use`, `list`, `statusline`, `hook`, `help`, `remove`, `refresh`).

## 4. Automatic switching

### What `cs` knows per account

For both usage windows Claude Code reports, the 5-hour and the 7-day window: the percentage used and the time it resets. An account is **available** when it is logged in and both windows are below the switch threshold, 95%. A window whose reset time has passed counts as 0%.

### Where the numbers come from

- **The active account:** Claude Code passes `rate_limits` to the status line command after every reply, with `five_hour` and `seven_day`, each a `used_percentage` and a `resets_at` (<https://code.claude.com/docs/en/statusline>). No token is needed. The docs say this is sent for Pro and Max accounts.
- **A parked account:** `cs` sends its access token to Anthropic's usage endpoint, `GET https://api.anthropic.com/api/oauth/usage`, which returns the same two windows. The endpoint is not documented and has changed shape before, so `cs` accepts both known shapes.

A source that fails or returns an unknown shape makes that account "unknown". It is shown as such and chosen last, and never stops `cs` as a whole (#8).

### Which account is next

`cs` spends what expires first (owner decision, #36). Usage left in a window when it resets is lost, and the 5-hour window only starts with an account's first message, so an untouched account loses nothing by waiting. Among the available accounts:

1. An account whose 5-hour window is running goes first, the one that resets soonest first. Its remaining usage is the next to be lost.
2. Then an account with no running 5-hour window, the one whose 7-day window resets soonest first, then the one with the most 7-day usage left. This spreads the weekly limits, so no account runs out of its week early.
3. Then the name, so the choice is the same every run.

`cs` stays on the active account until it reaches the threshold, so it never switches back and forth. The effect is that one account is drained at a time and a new 5-hour clock starts only when it is needed; by the time the others are used, the first has reset. If none is available, `cs` stays on the current account and `cs status` shows when the first one resets.

### When it switches

- **Before the limit:** each `cs statusline` run records the active account's numbers. When a window reaches 95%, it switches to the next account. The switch falls between two replies, so no turn fails.
- **At the limit:** Claude Code runs a `StopFailure` hook with matcher `rate_limit` when a turn ends on a usage limit (<https://code.claude.com/docs/en/hooks>). `cs hook rate-limit` records the active account as full and switches at once. The next message runs on the new account.

The status line prints at once. A switch is a few local Keychain calls and runs in the status line itself. Slow work (refreshing a parked login, asking for a parked account's usage when its numbers are more than five minutes old) runs in a detached `cs refresh`, so the next choice is made on current numbers. Only one `cs refresh` runs at a time.

### Several windows

All windows share the active account, so a switch moves all of them, as `/login` does. Every switch and every write of the state takes one lock. A second window that decides to switch at the same moment gets the lock after the first, sees that the account it wanted to leave is no longer active, and does nothing.

## 5. Logins and tokens

### A switch: `cs use member2`

Under the lock, in this order:

1. Save the login now in the item of `~/.claude` back to the parked item of the account that was active. Claude Code may have refreshed it, so this keeps the live tokens.
2. Write `member2`'s parked login into the item of `~/.claude`, and set `oauthAccount` in `~/.claude.json` to `member2`'s, changing nothing else in that file.
3. Clear `member2`'s parked item and record `member2` as active in the state.

The state records each step before it runs, so a switch that stops halfway is finished by the next `cs` run, and no login is lost or left with two copies. Steps 1 and 2 are recorded as one step: run again after a stop between them, it saves `~/.claude` once more, so tokens Claude Code refreshed in the meantime are kept, and it skips both when `~/.claude` already holds the next login.

At `cs setup` the login already in `~/.claude` has no profile. It is a separate login from the parked ones, so replacing it breaks nothing; `cs setup` says so before it does.

### Refreshing parked logins

A parked login has no Claude Code process to refresh it, so `cs` does. When a parked access token is close to expiry, `cs` takes the lock, sends the refresh token to Anthropic's OAuth token endpoint, and writes the new tokens back to the same item at once. `cs` never refreshes the active login: Claude Code does that. A refused refresh (a revoked login) marks the account logged out; `cs add <name>` and `/login` fix it. Any other failure leaves the item as it was.

## 6. Where things are stored

| What | Where | Written by |
|---|---|---|
| The `cs` binary | `~/.cargo/bin/cs` | cargo |
| One folder per profile, used for its `/login` | `~/.claude-profiles/<name>/` | cs, Claude Code |
| Parked logins | Keychain, the item of each profile folder | Claude Code at `/login`; cs on a switch or refresh |
| The active login | Keychain, `Claude Code-credentials` (the item of `~/.claude`) | Claude Code; cs on a switch |
| Active account, usage per account, unfinished switch | `~/.claude-profiles/state.json` | cs |
| The lock | `~/.claude-profiles/.lock` | cs |
| Status line and hook | `~/.claude/settings.json` | cs setup |

`cs list` treats every folder in `~/.claude-profiles/` as a profile, so the state and lock are files there. A profile's item is `Claude Code-credentials-` followed by the first 8 hex characters of the SHA-256 of its folder's absolute path, with the macOS user name as the account (observed on Claude Code 2.1.300, #6).

## 7. Code

A small library with a thin binary: `src/main.rs` only calls `cs::run` in `src/lib.rs`, so everything is testable from `tests/`. The modules follow the domain, one folder each, with a `mod.rs` that holds the folder's module documentation and the items its files share:

```
src/
├── main.rs, lib.rs, error.rs
├── command/      the command line: parse, and one file per command
│                 (add, list, activate for cs use, setup, status, refresh,
│                 statusline, hook); mod.rs dispatches and holds the helpers
├── store/        the files cs owns: profile (names, the profiles folder),
│                 state (state.json behind the lock, written in one rename)
├── usage/        mod.rs: the two windows per account, the selection view
│                 and time left; select: the selection rule
├── login/        mod.rs: Login, Slot and the Logins trait every Keychain
│                 access goes through; keychain: the macOS backend through
│                 /usr/bin/security; account: oauthAccount in .claude.json
├── switching/    transfer: the move of section 5 with its recovery;
│                 auto: when to switch on its own and where to go
├── anthropic/    http: requests through curl with the request on stdin;
│                 oauth: the refresh and usage calls; parked: keeping
│                 parked accounts ready
└── claude_code/  settings (cs setup), statusline (record and print),
                  launch (exec claude for cs add)
```

Every import names the module that defines the item; there are no re-exports. A login never appears in debug output, and the Keychain and HTTP are behind traits, so tests use memory and fakes.

The runtime dependencies are `thiserror` for the error type, `serde` with `serde_json` (key order kept) for the JSON files, `sha2` for the Keychain service names, and `jiff` for the reset times; each further dependency is added in the pull request that needs it.

## 8. Errors

Every failure is one short line on stderr and a non-zero exit, and nothing is ignored silently. The exceptions are the two calls Claude Code makes: `cs statusline` always prints a status line and `cs hook rate-limit` always exits 0, so a `cs` problem never breaks a Claude Code window. They report problems through `cs status` instead.

## 9. Checking it by hand

These need real accounts, so they are done once by the owner and recorded on the issue with the Claude Code version:

- After `cs add` for two accounts, Keychain Access shows one "Claude Code" item per profile (#6).
- With two windows open on `~/.claude`, `cs use` to the other account, then `/status` in both windows shows the new account.
- The status line of the work accounts receives `rate_limits`.

## 10. Security

`cs` touches tokens for three reasons only: to move a login on a switch, to refresh a parked login, and to read a parked access token for its usage. Tokens stay in the Keychain, encrypted at rest. `cs` holds them in memory only while it uses them, and never writes them to a file, a log, the terminal or the state. It never refreshes the active login and never keeps two live copies of one login. The owner asked for this on #12.

Removing an account means deleting its profile folder and its "Claude Code" Keychain item; deleting only the folder leaves the login in the Keychain. A later `cs remove <name>` does both.
