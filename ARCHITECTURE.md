# cs: Claude Code Profile Switcher (macOS)

## 1. Purpose

`cs` is a very small command-line tool written in Rust that lets one person use several Claude accounts with Claude Code without logging in again every time they switch. Each account lives in its own isolated profile. You log in once per profile, and after that a single command such as `cs primary` or `cs member3` starts Claude Code already signed in to the right account.

All profiles are work accounts. There are seven of them, and each profile has the same name as its account:

| Profile | Start command |
|---|---|
| primary | `cs primary` |
| member1 | `cs member1` |
| member2 | `cs member2` |
| member3 | `cs member3` |
| member5 | `cs member5` |
| member6 | `cs member6` |
| member7 | `cs member7` |

The tool does not store, read, copy or refresh any tokens itself. It only tells Claude Code which configuration folder to use. Claude Code remains fully responsible for logging in, storing credentials and refreshing them.

## 2. Scope

### In scope

The tool creates profiles, lists them, and launches Claude Code with the selected profile. It passes any extra arguments straight through to Claude Code, so `cs member1 --continue` behaves exactly like `claude --continue` would for that account.

The tool can also choose the account automatically. For every profile it knows the remaining usage, the time at which that usage resets, and the other information it needs to decide. With `cs auto` it starts Claude Code with the profile that has the most usage left, and it switches to another profile when the current one runs out (see section 7).

### Out of scope

The tool does not handle API keys, does not manage Claude Code updates, and does not sync settings between profiles (see section 6 for the manual approach).

## 3. Core idea

Claude Code reads the environment variable `CLAUDE_CONFIG_DIR`. When it is set, Claude Code uses that folder instead of the default `~/.claude` for its configuration, session history and login state. By giving each account its own folder and setting this variable before starting Claude Code, every account gets a completely separate world.

`cs` is therefore nothing more than a thin launcher:

```
cs member1  ──►  CLAUDE_CONFIG_DIR=~/.claude-profiles/member1  claude
```

On Unix systems the launcher uses `exec`, which replaces the `cs` process with the `claude` process. Claude Code gets the terminal directly, signals such as Ctrl+C behave normally, and no wrapper process stays running in the background.

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
| OAuth access and refresh tokens | macOS Keychain (login keychain) | Claude Code |
| Default, non-profile Claude Code config | `~/.claude/` and `~/.claude.json` | Claude Code |

The layout with all seven profiles:

```
~/.claude-profiles/
├── primary/
│   ├── settings.json        (optional, or a symlink)
│   ├── CLAUDE.md            (optional, or a symlink)
│   ├── projects/            session history for this account
│   └── ...                  other files Claude Code creates
├── member1/
├── member2/
├── member3/
├── member5/
├── member6/
└── member7/
```

Each folder has the same internal structure as `primary/`.

### Credentials and the Keychain

On macOS, Claude Code does not keep tokens in a plain file. It stores them in the login Keychain as a generic password item whose name starts with "Claude Code". Recent versions of Claude Code create a separate Keychain item for each `CLAUDE_CONFIG_DIR`, which is what keeps the profiles' logins apart. This behaviour depends on the Claude Code version, so it should be verified once after setup (see section 9).

Because the tokens live in the Keychain, they are encrypted at rest and protected by your macOS login. `cs` never touches them, which also means there is nothing sensitive in the `cs` binary or its folder.

## 6. How it works step by step

### One-time setup for all seven accounts

Run these one after another. Each command opens Claude Code; run `/login`, sign in with the matching account, then quit before running the next one.

```
cs add primary
cs add member1
cs add member2
cs add member3
cs add member5
cs add member6
cs add member7
```

Signing in with the wrong account in a profile is the most likely mistake here. Using a private browser window per login, or signing out of the previous account in the browser first, makes it easier to pick the right one.

### Creating a profile: `cs add member1`

1. `cs` checks that the name only contains ASCII letters, digits, `-` or `_`, starts with a letter or digit, and is not a command word (`add`, `list`, `auto`, `status`, `help`, `remove`, `current`, `link`).
2. It creates `~/.claude-profiles/member1/` if it does not exist.
3. It prints a reminder to log in, then launches Claude Code with `CLAUDE_CONFIG_DIR` pointing to that folder.
4. The user runs `/login` inside Claude Code and completes the browser sign-in (for example with Google).
5. Claude Code writes the tokens to the Keychain and its own files into the profile folder. The user can then quit.

### Using a profile: `cs member1 [args...]`

1. `cs` checks that `~/.claude-profiles/member1/` exists, and prints a helpful error if it does not.
2. It sets `CLAUDE_CONFIG_DIR` and execs `claude` with any remaining arguments.
3. Claude Code finds the stored login for that profile and starts without asking to sign in.

### Listing profiles: `cs list`

`cs` reads the folder names inside `~/.claude-profiles/`, sorts them and prints them. If there are none, it explains how to create one.

### Token refresh

The access token is short-lived and the refresh token is long-lived. Every time Claude Code runs and finds an expired access token, it uses the refresh token to get a new one and updates the Keychain item. Nothing in `cs` is involved. In practice this means a profile stays logged in as long as it is used now and then and the refresh token has not been revoked, for example by logging out or changing the account password. If a profile is ever logged out, running `cs <name>` and `/login` again fixes it.

### Sharing settings between profiles

Because each profile is a fresh config folder, your global files in `~/.claude` are not picked up automatically. Files you want everywhere can be symlinked into each profile. For all seven profiles at once:

```
for p in primary member1 member2 member3 member5 member6 member7; do
  ln -sf ~/.claude/CLAUDE.md     ~/.claude-profiles/$p/CLAUDE.md
  ln -sf ~/.claude/settings.json ~/.claude-profiles/$p/settings.json
  ln -sfn ~/.claude/agents       ~/.claude-profiles/$p/agents
done
```

Session history in `projects/` should stay per profile, so it is not symlinked.

## 7. Automatic account selection

### What the tool knows per profile

For each profile `cs` keeps track of:

| Information | Used for |
|---|---|
| Remaining usage in the current window | Ranking the profiles |
| Reset time of the current window | Knowing when an exhausted profile becomes usable again |
| Whether the profile is currently exhausted | Skipping it until its reset time has passed |
| Whether the profile is logged in | Skipping profiles that need `/login` first |

### Choosing a profile: `cs auto [args...]`

1. `cs` refreshes the usage information for every profile.
2. Profiles that are logged out, or exhausted with a reset time still in the future, are skipped.
3. Of the remaining profiles, the one with the most usage left is chosen. On a tie, the profile whose window resets soonest wins, so usage that would otherwise expire unused is spent first.
4. `cs` prints which profile it picked and how much usage it has left, then starts Claude Code with that profile exactly as `cs <name>` would.

If every profile is exhausted, `cs` prints the earliest reset time and which profile it belongs to, and exits with a non-zero code.

### Switching when a profile runs out

When the active profile runs out of usage, `cs` marks it as exhausted together with its reset time and picks the next best profile using the same rules. The switch happens inside the running Claude Code session, the same way `/login` changes the account of a session: the conversation, its history and the working directory stay as they are, and the user does not restart Claude Code or resume with `--continue`. Once a profile's reset time has passed, it becomes a candidate again automatically.

### Showing usage: `cs status`

`cs status` prints a table with every profile, its remaining usage and its reset time, so the user can see at a glance which accounts are available.

## 8. Error handling

The tool keeps error handling simple and readable. A missing `HOME` variable, an invalid profile name, a missing profile folder, a folder that cannot be created, and a `claude` binary that is not on the `PATH` each produce a short message and a non-zero exit code. No error is silently ignored.

## 9. Verifying the setup

Do this check after the first two profiles, before setting up all seven, so a problem shows up early. Start `cs primary` and `cs member1` and run `/status` inside Claude Code; each should show its own account. When all seven are set up, repeat the check for every profile. To look at the stored logins directly, open the Keychain Access app and search for "Claude Code"; there should be seven items, one per profile.

If the second login replaced the first, the installed Claude Code version still uses one shared Keychain item. Updating Claude Code is the first fix to try.

## 10. Security notes

`cs` never reads or writes credentials, so it cannot leak them. The profile folders contain session history and project data, which may include code or conversation content, so they should be treated like the normal `~/.claude` folder. Removing a profile means deleting its folder and also deleting its "Claude Code" item in Keychain Access; deleting only the folder leaves the login in the Keychain.

## 11. Possible later additions

Some small, safe extensions fit this design: a `cs remove <name>` command that deletes the folder and the matching Keychain item, a `cs current` command that shows which profile the current shell would use, shell completion for profile names, and an optional `cs link <name>` command that creates the shared-settings symlinks from section 6 automatically.
