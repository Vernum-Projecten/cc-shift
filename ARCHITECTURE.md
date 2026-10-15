# cs: Claude Code Profile Switcher (macOS)

## 1. Purpose

`cs` is a very small command-line tool written in Rust that lets one person use several Claude accounts with Claude Code without logging in again every time they switch. Each account lives in its own isolated profile. You log in once per profile, and after that a single command such as `cs voorzitter` or `cs claude3` starts Claude Code already signed in to the right account.

All profiles are work accounts. There are seven of them, and each profile has the same name as its account:

| Profile | Start command |
|---|---|
| voorzitter | `cs voorzitter` |
| claude1 | `cs claude1` |
| claude2 | `cs claude2` |
| claude3 | `cs claude3` |
| claude5 | `cs claude5` |
| claude6 | `cs claude6` |
| claude7 | `cs claude7` |

There are two ways to use the accounts. `cs <name>` starts a separate Claude Code with that profile's own config folder. More often, every window (RustRover and a plain `claude`) runs with the default `~/.claude`, and `cs` changes which account that folder is logged in to, the same way `/login` does. Every open session then continues on the new account without a restart.

Claude Code stays responsible for logging in and for refreshing the active account's tokens. `cs` moves an account's login between its profile and `~/.claude` when it switches accounts, refreshes the logins parked in the profiles, and reads a parked login to ask for that account's usage (sections 6, 7 and 10).

## 2. Scope

### In scope

The tool creates profiles, lists them, and launches Claude Code with the selected profile. It passes any extra arguments straight through to Claude Code, so `cs claude1 --continue` behaves exactly like `claude --continue` would for that account.

The tool also manages the active account of `~/.claude`. `cs use <name>` makes an account active. For every profile `cs` knows the remaining usage, the time at which that usage resets, and whether it is logged in. `cs auto` makes the account with the most usage left active, and when the active account runs out, `cs` switches to the next one inside the running sessions (see section 7).

### Out of scope

The tool does not handle API keys, does not manage Claude Code updates, and does not sync settings between profiles (see section 6 for the manual approach).

## 3. Core idea

Claude Code reads the environment variable `CLAUDE_CONFIG_DIR`. When it is set, Claude Code uses that folder instead of the default `~/.claude` for its configuration, session history and login state. By giving each account its own folder and setting this variable before starting Claude Code, every account gets a completely separate world.

`cs` is therefore nothing more than a thin launcher:

```
cs claude1  ──►  CLAUDE_CONFIG_DIR=~/.claude-profiles/claude1  claude
```

On Unix systems the launcher uses `exec`, which replaces the `cs` process with the `claude` process. Claude Code gets the terminal directly, signals such as Ctrl+C behave normally, and no wrapper process stays running in the background.

### The active account

Claude Code keys its Keychain login to the config folder, and a running session picks up a login written into its folder's item. On Claude Code 2.1.300 a `/login` in one window also applies to the other windows already running with that folder (observed by the owner, #12). `cs` uses this for `~/.claude`:

```
~/.claude-profiles/claude1   parked login of claude1
~/.claude-profiles/claude2   parked login of claude2
~/.claude                    the active account, used by every window
```

Switching means moving the next account's parked login into the item of `~/.claude`. Nothing restarts: the conversation, its history and the working directory stay as they are.

## 4. Components

The code is a small library with a thin binary on top.

`Cargo.toml` describes the crate. Its only runtime dependency is `thiserror`, for the error type; everything else uses the Rust standard library. The release profile strips symbols and optimises for size, so the final binary stays small.

`src/main.rs` only calls `cs::run` in `src/lib.rs`, so the whole run path can be tested from `tests/`. The library has one module per concern: `cli` parses the command line, `profile` validates names and finds or creates the profiles folder, `launch` sets `CLAUDE_CONFIG_DIR` and execs `claude`, and `error` holds the messages the user sees.

After `cargo install --path .` the binary is placed at `~/.cargo/bin/cs`.

## 5. Where things are stored

| What | Location | Managed by |
|---|---|---|
| The `cs` binary | `~/.cargo/bin/cs` | cargo |
| Profiles root | `~/.claude-profiles/` | cs |
| One profile's config, history, projects | `~/.claude-profiles/<name>/` | Claude Code |
| OAuth access and refresh tokens | macOS Keychain (login keychain), one item per config folder | Claude Code; `cs` moves them on a switch |
| The shared config of every window, logged in to the active account | `~/.claude/` and `~/.claude.json` | Claude Code |
| The active account, exhausted accounts with reset times, last known usage | `~/.claude-profiles/state.json` | cs |
| The lock every switch and state write takes | `~/.claude-profiles/.lock` | cs |

The layout with all seven profiles:

```
~/.claude-profiles/
├── voorzitter/
│   ├── settings.json        (optional, or a symlink)
│   ├── CLAUDE.md            (optional, or a symlink)
│   ├── projects/            session history for this account
│   └── ...                  other files Claude Code creates
├── claude1/
├── claude2/
├── claude3/
├── claude5/
├── claude6/
└── claude7/
```

Each folder has the same internal structure as `voorzitter/`.

### Credentials and the Keychain

On macOS, Claude Code does not keep tokens in a plain file. It stores them in the login Keychain as a generic password item whose name starts with "Claude Code"; the item of the default `~/.claude` is `Claude Code-credentials`. With `CLAUDE_CONFIG_DIR` set, Claude Code keys the item to that folder, which keeps the profiles' logins apart (<https://code.claude.com/docs/en/authentication#credential-management>). The exact item name for a profile folder is recorded on #6 once two profiles are logged in.

A refresh token works once: when Claude Code refreshes, the old one stops working, and presenting a stale copy can revoke the login. So `cs` never copies a login. A switch moves it, and at any moment each login has exactly one live copy, either parked in its profile's item or active in the item of `~/.claude`. The tokens stay in the Keychain, encrypted at rest; `cs` holds them in memory only while it moves them, and never writes them to a file or a log.

## 6. How it works step by step

### One-time setup for all seven accounts

Run these one after another. Each command opens Claude Code; run `/login`, sign in with the matching account, then quit before running the next one.

```
cs add voorzitter
cs add claude1
cs add claude2
cs add claude3
cs add claude5
cs add claude6
cs add claude7
```

Signing in with the wrong account in a profile is the most likely mistake here. Using a private browser window per login, or signing out of the previous account in the browser first, makes it easier to pick the right one.

### Creating a profile: `cs add claude1`

1. `cs` checks that the name only contains ASCII letters, digits, `-` or `_`, starts with a letter or digit, and is not a command word (`add`, `list`, `use`, `auto`, `status`, `statusline`, `hook`, `help`, `remove`, `current`, `link`).
2. It creates `~/.claude-profiles/claude1/` if it does not exist.
3. It prints a reminder to log in, then launches Claude Code with `CLAUDE_CONFIG_DIR` pointing to that folder.
4. The user runs `/login` inside Claude Code and completes the browser sign-in (for example with Google).
5. Claude Code writes the tokens to the Keychain and its own files into the profile folder. The user can then quit.

### Using a profile: `cs claude1 [args...]`

1. `cs` checks that `~/.claude-profiles/claude1/` exists, and prints a helpful error if it does not.
2. When `claude1` is the active account, its login is in `~/.claude`, not in the profile, so `cs` says so and points to a plain `claude` instead.
3. Otherwise it sets `CLAUDE_CONFIG_DIR` and execs `claude` with any remaining arguments.
4. Claude Code finds the stored login for that profile and starts without asking to sign in.

### Making an account active: `cs use claude2`

Under the lock, in this order:

1. `cs` reads the login now in the item of `~/.claude` and writes it back to the parked item of the account that was active. Claude Code may have refreshed it since the last switch, so this saves the current tokens.
2. It writes `claude2`'s parked login into the item of `~/.claude`.
3. It clears `claude2`'s parked item and records `claude2` as active in `state.json`.

Every window running with `~/.claude` continues on `claude2`. If `cs` stops halfway, `state.json` and the order of the steps tell the next run which step to finish, so no login is lost or left with two copies.

### Listing profiles: `cs list`

`cs` reads the folder names inside `~/.claude-profiles/`, sorts them and prints them. If there are none, it explains how to create one.

### Token refresh

The access token is short-lived and the refresh token is long-lived. Every time Claude Code runs and finds an expired access token, it uses the refresh token to get a new one and updates the Keychain item of the folder it runs in. That is how the active account in `~/.claude` stays fresh, and `cs` never refreshes it: two processes refreshing one login would each spend the single-use refresh token the other still holds.

A parked login has no Claude Code process to refresh it, so `cs` does. Before it uses a parked login (to read usage, or to make it active) and whenever its access token is close to expiry, `cs` takes the lock, sends the refresh token to Anthropic's OAuth token endpoint, and writes the new access and refresh tokens back to the same parked item at once. Only a parked login is ever refreshed by `cs`, so each login still has one live copy and one process that refreshes it. A refresh that the endpoint refuses (a revoked or expired refresh token) marks the profile logged out; running `cs add <name>` and `/login` again fixes it. The token endpoint is not documented by Anthropic, so a refresh that fails for another reason leaves the parked item as it was and marks the profile's usage "unknown".

### Sharing settings between profiles

Because each profile is a fresh config folder, your global files in `~/.claude` are not picked up automatically. Files you want everywhere can be symlinked into each profile. For all seven profiles at once:

```
for p in voorzitter claude1 claude2 claude3 claude5 claude6 claude7; do
  ln -sf ~/.claude/CLAUDE.md     ~/.claude-profiles/$p/CLAUDE.md
  ln -sf ~/.claude/settings.json ~/.claude-profiles/$p/settings.json
  ln -sfn ~/.claude/agents       ~/.claude-profiles/$p/agents
done
```

Session history in `projects/` is not symlinked. The sessions that switch accounts all run in `~/.claude`, so their history is already in one place.

## 7. Automatic account selection

### What the tool knows per profile

For each profile `cs` keeps track of:

| Information | Used for |
|---|---|
| Remaining usage in the current window | Ranking the profiles |
| Reset time of the current window | Knowing when an exhausted profile becomes usable again |
| Whether the profile is currently exhausted | Skipping it until its reset time has passed |
| Whether the profile is logged in | Skipping profiles that need `/login` first |

### Where usage comes from

Two sources, decided on #8:

- **The active account:** Claude Code passes `rate_limits` to the status line command, with `five_hour` and `seven_day` windows, each a `used_percentage` and a `resets_at` time (<https://code.claude.com/docs/en/statusline>). `cs` installs `cs statusline` as the status line of `~/.claude`, chained in front of any status line the user already has, and it records those numbers in `state.json`. No token is read. The docs say `rate_limits` is sent for Pro and Max accounts after the first response of a session.
- **Every parked account:** `cs` reads that account's access token from its parked item, read-only, and asks Anthropic's usage endpoint (`GET https://api.anthropic.com/api/oauth/usage`) for the same two windows. The endpoint is not documented and has changed its response shape before, so `cs` accepts both known shapes.
- **Logged in or not:** `claude auth status --json` with the profile's `CLAUDE_CONFIG_DIR`.

A source that is missing, fails or returns an unknown shape makes that account's usage "unknown". `cs status` shows why, and `cs auto` ranks it after every account with known usage. No command fails as a whole because one account could not be read.

### Choosing an account: `cs auto`

1. `cs` refreshes the usage information for every profile.
2. Profiles that are logged out, or exhausted with a reset time still in the future, are skipped.
3. Of the remaining profiles, the one with the most usage left is chosen. On a tie, the profile whose window resets soonest wins, so usage that would otherwise expire unused is spent first.
4. `cs` prints which account it picked and how much usage it has left, and makes it active exactly as `cs use <name>` would.

If every profile is exhausted, `cs` prints the earliest reset time and which profile it belongs to, and exits with a non-zero code.

### Switching when the active account runs out

Claude Code runs a `StopFailure` hook with matcher `rate_limit` when a turn ends on a usage limit (<https://code.claude.com/docs/en/hooks>). `cs` installs `cs hook rate-limit` there. It marks the active account exhausted with the reset time last recorded from the status line, picks the next account with the rules of `cs auto`, and switches to it under the lock. Every open session continues on the new account: the conversation, its history and the working directory stay as they are, and nobody restarts Claude Code or resumes with `--continue`. Once an account's reset time has passed, it becomes a candidate again.

### Several sessions at once

All sessions in `~/.claude` share the active account, so a switch applies to all of them, as `/login` does. When two sessions run out at the same moment, both hooks take the lock in turn; the second sees that the account it ran out on is no longer active and does nothing, so the accounts switch once. Every read and write of `state.json` takes the same lock.

### Showing usage: `cs status`

`cs status` prints a table with every profile, its remaining usage and its reset time, and marks the active account, so the user can see at a glance which accounts are available.

## 8. Error handling

The tool keeps error handling simple and readable. A missing `HOME` variable, an invalid profile name, a missing profile folder, a folder that cannot be created, and a `claude` binary that is not on the `PATH` each produce a short message and a non-zero exit code. No error is silently ignored.

## 9. Verifying the setup

Do this check after the first two profiles, before setting up all seven, so a problem shows up early. Start `cs voorzitter` and `cs claude1` and run `/status` inside Claude Code; each should show its own account. When all seven are set up, repeat the check for every profile. To look at the stored logins directly, open the Keychain Access app and search for "Claude Code"; there should be seven items, one per profile.

If the second login replaced the first, the installed Claude Code version still uses one shared Keychain item. Updating Claude Code is the first fix to try.

Before `cs use` is trusted with real logins, check one switch by hand: with two windows open on `~/.claude`, run `cs use` to the other account, and run `/status` in both windows. Both should show the new account.

## 10. Security notes

`cs` touches credentials for three purposes only: it moves a login between a profile's Keychain item and the item of `~/.claude` on a switch, it refreshes a parked login, and it reads a parked account's access token to ask for that account's usage. It never refreshes the active login in `~/.claude`, never keeps two live copies of a login, and never writes a token to a file, a log, the terminal or `state.json`. The owner asked for this on #12 and for the refresh of parked logins afterwards, so that `cs` does what `/login` does and keeps every account ready. The profile folders contain session history and project data, which may include code or conversation content, so they should be treated like the normal `~/.claude` folder. Removing a profile means deleting its folder and also deleting its "Claude Code" item in Keychain Access; deleting only the folder leaves the login in the Keychain.

## 11. Possible later additions

Some small, safe extensions fit this design: a `cs remove <name>` command that deletes the folder and the matching Keychain item, a `cs current` command that shows which profile the current shell would use, shell completion for profile names, and an optional `cs link <name>` command that creates the shared-settings symlinks from section 6 automatically.
