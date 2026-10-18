<!-- SPDX-FileCopyrightText: 2026 Vernum Projecten B.V. -->
<!-- SPDX-License-Identifier: Apache-2.0 -->

# cc-shift: automatic Claude account switching (macOS and Linux)

## 1. Purpose

`cc-shift` lets one person use several Claude accounts with Claude Code without ever thinking about usage limits. It runs on macOS, Linux and Windows (#102). Every Claude Code window (RustRover, a terminal) keeps running as normal. When the account in use gets close to its limit, `cc-shift` moves the windows to the next account, the same way `/login` does: no restart, no lost conversation.

The accounts are several work accounts, each with a profile of its own. The examples below use invented profile names.

## 2. How it works

Claude Code keeps its login in the macOS Keychain, in one item per config folder (<https://code.claude.com/docs/en/authentication#credential-management>). A new session reads that item when it starts. A running session re-reads that item before each request, with a 30-second cache, so it picks up a login written there while it runs; the owner saw a `/login` in one window reach the others (#12), and the Claude Code 2.1.300 binary shows how (#50).

On Linux, Claude Code keeps the login of a config folder in `<folder>/.credentials.json` with mode 0600 and uses no keyring (same page). A running session checks that file's modification time before each request and drops its cached login when the file changed (the 2.1.300 Linux build, #85), so it picks up a switch with its next request. Everything below holds on both systems; where they differ, a "slot" is the Keychain item on macOS and the `.credentials.json` file on Linux.

On Windows, Claude Code keeps the same file, `%USERPROFILE%\.claude\.credentials.json`, protected by the access rules of the user's folder, and checks it before each request as on Linux (the 2.1.300 Windows build, #102). A server-side flag (`tengu_windows_credman`, cached in the folder's `.claude.json`) or `CLAUDE_CODE_FORCE_WINDOWS_CREDMAN=1` moves its logins into Windows Credential Manager, which it then reads first; a moved file would be ignored, so while either is on for a folder, every login access stops with an error that says so (#104). The home folder on Windows is `%USERPROFILE%`, as Claude Code uses.

`cc-shift` builds on that:

```
~/.claude-profiles/member1   parked login of member1
~/.claude-profiles/member2   parked login of member2
~/.claude                    the active account, used by every window
```

- Each account is logged in once, in its own profile folder. That login stays parked in the profile's Keychain item.
- Every window runs with the default `~/.claude`. Its Keychain item holds the active account.
- A switch moves the next account's parked login into the item of `~/.claude`, and copies that account's details (`oauthAccount` in the profile's `.claude.json`) into `~/.claude.json`. `/login` writes both, and open windows follow only when both change (observed on Claude Code 2.1.300, #24). Every open window continues on the new account, within about 30 seconds on macOS and with its next request on Linux.

A refresh token works once: after a refresh the old one is dead, and presenting a dead copy can revoke the login. So a login is always moved, never copied, and each login has exactly one live copy and one process that refreshes it: Claude Code for the active login, `cc-shift` for the parked ones.

## 3. Commands

| Command | What it does |
|---|---|
| `cc-shift add <name>` | Creates the profile and opens Claude Code in it to run `/login` once. |
| `cc-shift setup` | Installs the status line and the hook in `~/.claude/settings.json` and makes the best account active. Run once. |
| `cc-shift status` | Shows every account in the order of use, with both windows as bars, their reset times and each account's state. |
| `cc-shift status --json` | The same overview as JSON, for scripts. |
| `cc-shift stats` | Shows the tokens of all accounts together per project, model and day, read from Claude Code's transcripts; `--since`, `--all`, `--project`, `--cost`, `--per-account` and `--json` choose the period, one project, the cost at API prices, a split per account from the history, and JSON. |
| `cc-shift use <name>` | Makes an account active now. Never needed; for steering by hand. |
| `cc-shift list` | Prints the profile names. |
| `cc-shift refresh` | Refreshes parked logins and reads parked accounts' usage now. `cc-shift status` and the status line run it on their own. |
| `cc-shift doctor` | Checks the setup and prints the command that fixes each problem. |
| `cc-shift log` | Shows what `cc-shift` did: switches with their reason, refreshed and logged-out logins, and when all accounts were full and free again; `--since`, `--all`, `--verbose` and `--json`. |
| `cc-shift remove <name>` | Deletes a profile and its Keychain login, after confirmation; never the active account. |
| `cc-shift uninstall [--all]` | Undoes `cc-shift setup`; with `--all` also deletes every profile and its login. |
| `cc-shift completion zsh\|powershell` | Prints the zsh or PowerShell completion script. |
| `cc-shift version` | Prints the version; also `--version` and `-V`. |
| `cc-shift help` | Prints the commands; also `-h` and `--help`. On a terminal it opens with the logo. |
| `cc-shift statusline` | Called by Claude Code after every reply. Not run by hand. |
| `cc-shift hook rate-limit` | Called by Claude Code when a turn hits a usage limit. Not run by hand. |

After `cc-shift add` for each account and one `cc-shift setup`, `cc-shift` tracks every account and switches on its own.

A profile name uses ASCII letters, digits, `-` and `_`, starts with a letter or digit, and is not a command word (`add`, `setup`, `status`, `use`, `list`, `statusline`, `hook`, `help`, `remove`, `refresh`, `doctor`, `uninstall`, `completion`, `log`, `version`, `stats`), nor `early`, a command of older versions. A new profile may not differ from an existing one only in case, and on Windows a name is not a device name (`con`, `prn`, `aux`, `nul`, `com1` to `com9`, `lpt1` to `lpt9`, in any case; <https://learn.microsoft.com/windows/win32/fileio/naming-a-file>).

## 4. Automatic switching

### What `cc-shift` knows per account

For both usage windows Claude Code reports, the 5-hour and the 7-day window: the percentage used and the time it resets. An account is **available** when it is logged in and each window is below its switch threshold: 95% for the 5-hour window and 99% for the 7-day window by default, each settable from 50 to 100 under `thresholds` in `state.json` (#66). The week switches later because its last percent is worth more than a 5-hour window, and the rate-limit hook catches a turn that hits the limit anyway. A window whose reset time has passed counts as 0%.

### Where the numbers come from

- **The active account:** Claude Code passes `rate_limits` to the status line command after every reply, with `five_hour` and `seven_day`, each a `used_percentage` and a `resets_at` (<https://code.claude.com/docs/en/statusline>). No token is needed. The docs say this is sent for Pro and Max accounts. When the status line has reported no new numbers for a minute, `cc-shift` also reads the active account from the usage endpoint with its access token (#123).
- **A parked account:** `cc-shift` sends its access token to Anthropic's usage endpoint, `GET https://api.anthropic.com/api/oauth/usage`, which returns the same two windows. The endpoint is not documented and has changed shape before, so `cc-shift` accepts both known shapes.

A source that fails or returns an unknown shape makes that account "unknown". It is shown as such and chosen last, and never stops `cc-shift` as a whole (#8).

### Which account is next

`cc-shift` spreads the weeks (owner decision, #120, amending #36). The week is what runs out. Usage left in a 5-hour window when it resets still counts against the week, so nothing is lost; only a week that resets with usage left loses it. Every account with week left is one more account to switch to when a 5-hour window fills. Among the available accounts:

1. An account whose 7-day window resets within 5 hours goes first, the one that resets soonest first. What is left of its week is lost at the reset, and one 5-hour window is the time there is to spend it.
2. Then the account with the most 7-day usage left; an untouched account has all of it.
3. Then the one whose 7-day window resets soonest, then the name, so the choice is the same every run.

`cc-shift` stays on the active account until it reaches the threshold, so it never switches back and forth. Each time a 5-hour window fills, the account with the most week left takes over, so the weeks drain together and several accounts keep some week for as long as possible. If none is available, `cc-shift` stays on the current account and `cc-shift status` shows when the first one resets.

### When it switches

A running Claude Code session re-reads its login from the Keychain before each request (with a 30-second cache) and after a 401, and before it refreshes a token it re-reads the Keychain under a lock and takes over a login it finds there instead of refreshing over it (Claude Code 2.1.300, read from the binary, #50). So every open window follows a switch within about 30 seconds, and a window never refreshes over a login `cc-shift` moved in.

The status line reports the usage of the window's last response. Right after a switch that response was still made on the account just left, so a reading cannot be trusted by when it arrives. On 2026-10-15 this made the switches cascade: the old account's numbers were recorded under the new one, which then looked full (#50). So:

- **Readings are matched to accounts.** Every account's windows reset at their own times, so a status line reading belongs to the account whose confirmed reading (from the usage endpoint, with its own token) has the same reset times. A reading that matches no confirmed account goes to the active account only when that account has no confirmed reading yet, and is dropped otherwise.
- **Before the limit:** when a reading proven to be the active account's reaches the threshold of either window, the status line switches to the next account, between two replies.
- **While the main session waits on subagents:** Claude Code runs the status line only on events in the main session, such as a reply, and "the event-driven triggers can go quiet when the main session is idle, for example while a coordinator waits on background subagents" (<https://code.claude.com/docs/en/statusline#when-the-status-line-updates>). The subagents keep spending the account meanwhile. So `cc-shift setup` gives the status line a `refreshInterval` of 30 seconds, which runs it on a timer as well. A timed run repeats the last reply's numbers, so a reading moves the account's reading time only when its numbers change, and a reading lower than the recorded one for the same window is an old one and is not recorded: usage in one window only rises until it resets. When the active account's numbers are a minute old, the detached `cc-shift refresh` reads it from the usage endpoint and switches when that reading is at or past a threshold (#123).
- **At the limit:** Claude Code runs a `StopFailure` hook with matcher `rate_limit` when a turn ends on a usage limit (<https://code.claude.com/docs/en/hooks>). `cc-shift hook rate-limit` records the active account as full and switches at once. It records the 5-hour window at 100%, or the week when only the week is at or past its threshold: the 5-hour limit is the one hit far more often, and a week at 85% with a 5-hour window at 80% is a 5-hour limit.
- **No switch follows another within five minutes,** so a reading or a hook left over from before a switch cannot start a second one.
- **The account details are put right.** A window can write its old account details back into `~/.claude.json`, so each proven reading of the active account sets them again.

The status line names the account the window really uses. Right after a switch it shows `moving to <account>` until the window's next request runs on the new login.

The status line prints at once. Slow work (refreshing a parked login, reading an account's usage when its numbers are not yet confirmed or older than five minutes, or one minute for the active account) runs in a detached `cc-shift refresh`. Only one runs at a time.

### Several windows

All windows share the active account, so a switch moves all of them, as `/login` does. Every switch and every write of the state takes one lock, and the five-minute pause after a switch keeps a second window from starting another.

## 5. Logins and tokens

### A switch: `cc-shift use member2`

Under the lock, in this order. Steps 1 and 2 also hold Claude Code's own refresh lock of `~/.claude` (the folder `.oauth_refresh.lock`, which Claude Code takes before it refreshes and treats as abandoned after 60 seconds; #90), so no window refreshes the login between the save and the write. A switch waits up to 10 seconds for a lock Claude Code holds, takes over an abandoned one, and otherwise stops and is retried later.


1. Save the login now in the item of `~/.claude` back to the parked item of the account that was active. Claude Code may have refreshed it, so this keeps the live tokens.
2. Write `member2`'s parked login into the item of `~/.claude`, and set `oauthAccount` in `~/.claude.json` to `member2`'s, changing nothing else in that file.
3. Clear `member2`'s parked item and record `member2` as active in the state.

On Linux each write goes to a new file with mode 0600 in the same folder, which is then renamed over the old one, so a running window sees a new file and no reader ever sees half a login. A login file that is a symbolic link is refused, as Claude Code refuses it. On Windows the new file takes the folder's access rules, and a rename that another process (a virus scanner, the search indexer) blocks for a moment is tried again for about a second.

The state records each step before it runs, so a switch that stops halfway is finished by the next `cc-shift` run, and no login is lost or left with two copies. Steps 1 and 2 are recorded as one step: run again after a stop between them, it saves `~/.claude` once more, so tokens Claude Code refreshed in the meantime are kept, and it skips both when `~/.claude` already holds the next login.

At `cc-shift setup` the login already in `~/.claude` has no profile. It is a separate login from the parked ones, so replacing it breaks nothing; `cc-shift setup` says so before it does.

### Refreshing parked logins

A parked login has no Claude Code process to refresh it, so `cc-shift` does. When a parked access token is close to expiry, `cc-shift` takes the lock, sends the refresh token to Anthropic's OAuth token endpoint, and writes the new tokens back to the same item at once. `cc-shift` never refreshes the active login: Claude Code does that. A refused refresh (a revoked login) marks the account logged out; `cc-shift add <name>` and `/login` fix it. Any other failure leaves the item as it was.

## 6. Where things are stored

| What | Where | Written by |
|---|---|---|
| The `cc-shift` binary | `~/.cargo/bin/cc-shift`, or `~/.local/bin/cc-shift` for a prebuilt binary | cargo, or the user |
| One folder per profile, used for its `/login` | `~/.claude-profiles/<name>/` | cc-shift, Claude Code |
| Parked logins | macOS: Keychain, the item of each profile folder. Linux: `~/.claude-profiles/<name>/.credentials.json`, the same on Windows | Claude Code at `/login`; cc-shift on a switch or refresh |
| The active login | macOS: Keychain, `Claude Code-credentials` (the item of `~/.claude`). Linux: `~/.claude/.credentials.json`, on Windows `%USERPROFILE%\.claude\.credentials.json` | Claude Code; cc-shift on a switch |
| Active account, usage per account, unfinished switch, thresholds | `~/.claude-profiles/state.json` | cc-shift |
| The history | `~/.claude-profiles/history.jsonl`, and `history.1.jsonl` after it rotates at about 1 MB | cc-shift |
| The locks | `~/.claude-profiles/.lock` (state and switches), `.refresh-lock` (one refresh at a time), `.history-lock` | cc-shift |
| Token counts read from the transcripts, per file | `~/.claude-profiles/stats-cache.json` | cc-shift stats |
| Status line and hook | `~/.claude/settings.json` | cc-shift setup |

Versions up to 0.1.3 had an early start (#35), a scheduled job that started some accounts' 5-hour windows before the workday. It is removed (#126). `cc-shift setup` and `cc-shift uninstall` delete what it left: on macOS the agent `~/Library/LaunchAgents/io.github.vernum-projecten.cc-shift.early-start.plist`, on Linux `cc-shift-early-start.service` and `.timer` in `~/.config/systemd/user/` (or `$XDG_CONFIG_HOME`), on Windows the Task Scheduler task `cc-shift early start` with `%LOCALAPPDATA%\cc-shift\early-start.xml`, and its empty working folder `early-start` in the user's cache folder. A state file that still holds `early_start` loads, and the field is dropped on the next write.

`cc-shift list` treats every folder in `~/.claude-profiles/` as a profile, so the state, the history, the stats cache and the locks are files there. A profile's item is `Claude Code-credentials-` followed by the first 8 hex characters of the SHA-256 of its folder's absolute path, with the macOS user name as the account (observed on Claude Code 2.1.300, #6).

## 7. Code

A small library with a thin binary: `src/main.rs` only calls `cc_shift::run` in `src/lib.rs`, so everything is testable from `tests/`. The modules follow the domain, one folder each, with a `mod.rs` that holds the folder's module documentation and the items its files share:

```
src/
├── main.rs, lib.rs, error.rs
├── command/      the command line: parse, and one file per command
│                 (add, list, activate for cc-shift use, setup, status, stats,
│                 log, refresh, doctor, remove, uninstall, completion,
│                 help, statusline, hook); mod.rs dispatches and holds the
│                 helpers
├── store/        the files cc-shift owns: profile (names, the profiles folder),
│                 state (state.json behind the lock, written in one rename),
│                 history (history.jsonl, appended under its own lock)
├── usage/        mod.rs: the two windows per account, the selection view
│                 and time left; select: the selection rule; overview:
│                 every account's state in the order of use
├── render/       table and json: cc-shift status; line: the status line;
│                 log: the history; stats and stats_json: cc-shift stats;
│                 bars and colour (NO_COLOR is honoured)
├── early/        removing the early start job older versions installed:
│                 macos/launchd: the agent; linux/systemd: the timer;
│                 windows/task: the Task Scheduler task
├── stats/        transcript: finding the session transcripts and reading
│                 only the usage fields; cache: stats-cache.json per file
│                 by size and modification time; summary: one count per
│                 message id, per project, model and local day; price: the
│                 API price table and the cost of a response
├── login/        mod.rs: Login, Slot and the Logins trait every login
│                 access goes through; account: oauthAccount in
│                 .claude.json; macos/keychain: the macOS backend through
│                 /usr/bin/security; files: the login files of Linux and
│                 Windows, with the Credential Manager check
├── switching/    transfer: the move of section 5 with its recovery;
│                 auto: when to switch on its own and where to go
├── anthropic/    http: requests through curl with the request on stdin;
│                 oauth: the refresh and usage calls; parked: keeping
│                 parked accounts ready
└── claude_code/  settings (cc-shift setup), statusline (record and print),
                  launch (exec claude for cc-shift add)
```

Code that differs per operating system lives in a `macos/`, `linux/` or `windows/` folder inside its own module, compiled only on that system; the login files are one module (`login/files.rs`) shared by Linux and Windows, which differ only in small details (`#[cfg(target_os = "...")]`); everything else is shared. `login::SystemLogins` and the removal in `early` pick the backend, so the commands name no system. Every import names the module that defines the item; there are no re-exports. A login never appears in debug output, and the Keychain and HTTP are behind traits, so tests use memory and fakes.

The runtime dependencies are `thiserror` for the error type, `serde` with `serde_json` (key order kept) for the JSON files, `sha2` for the Keychain service names, and `jiff` for the reset times and, with its system time zone support, the local days of `cc-shift stats`; each further dependency is added in the pull request that needs it.

## 8. Errors

Every failure is one short line on stderr and a non-zero exit, and nothing is ignored silently. The exceptions are the two calls Claude Code makes: `cc-shift statusline` always prints a status line and `cc-shift hook rate-limit` always exits 0, so a `cc-shift` problem never breaks a Claude Code window. They report problems through `cc-shift status` instead.

## 9. Checking it by hand

These need real accounts, so they are done once by the owner and recorded on the issue with the Claude Code version:

- After `cc-shift add` for two accounts, Keychain Access shows one "Claude Code" item per profile (#6).
- With two windows open on `~/.claude`, `cc-shift use` to the other account, then `/status` in both windows shows the new account.
- The status line of the work accounts receives `rate_limits`.
- On Linux, still to be done with real accounts: `cc-shift add` and `/login` write the profile's `.credentials.json` with mode 0600; two open windows follow `cc-shift use` and an automatic switch with their next message; a window refreshing during a switch loses no login.
- On Windows (#112), still to be done with real accounts: two open windows follow `cc-shift use` and an automatic switch; the status line and the hook run with and without Git for Windows; a parked refresh and a window refreshing during a switch lose no login; and whether each account uses the login file or Credential Manager.

## 10. Security

`cc-shift` touches tokens for three reasons only: to move a login on a switch, to refresh a parked login, and to read a parked access token for its usage. Tokens stay where Claude Code keeps them: in the Keychain, encrypted at rest, on macOS, in its login files with mode 0600 on Linux, and in its login files under the user folder's access rules on Windows. `cc-shift` holds them in memory only while it uses them, and never writes them to a log, the terminal, the state or a file of its own; on Linux it writes a login only into the login file Claude Code reads. It never refreshes the active login and never keeps two live copies of one login. The owner asked for this on #12.

Removing an account means deleting its profile folder and, on macOS, its "Claude Code" Keychain item; deleting only the folder leaves the login in the Keychain. On Linux and Windows the login file is inside the folder. `cc-shift remove <name>` does both.
