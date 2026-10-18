// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift statusline`: the status line command Claude Code runs after every
//! reply.
//!
//! Claude Code passes a JSON object on stdin whose `rate_limits` holds the
//! active account's `five_hour` and `seven_day` windows, each with
//! `used_percentage` and `resets_at`
//! (<https://code.claude.com/docs/en/statusline>). `cc-shift` records them and then
//! prints the status line the user had before, if any.

use std::io::Write;
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::error::Error;
use crate::store::state::{Recorded, State, StateFile};
use crate::usage::Window;

/// The part of the status line input `cc-shift` reads; every other field is
/// ignored.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Input {
    rate_limits: Option<RateLimits>,
}

/// The two usage windows as Claude Code reports them.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct RateLimits {
    five_hour: Option<Reported>,
    seven_day: Option<Reported>,
}

/// One window as Claude Code reports it.
#[derive(Debug, Deserialize)]
struct Reported {
    used_percentage: f64,
    resets_at: i64,
}

impl From<Reported> for Window {
    fn from(reported: Reported) -> Self {
        Self {
            used_percent: reported.used_percentage,
            resets_at: reported.resets_at,
        }
    }
}

/// How far apart, in seconds, two readings of one window's reset time may be
/// and still name the same window: the status line and the usage endpoint
/// round it differently.
const SAME_RESET_SECONDS: i64 = 120;

/// What a status line reading was recorded as.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Reading {
    /// The status line command the user had before setup, if any.
    pub chained: Option<String>,
    /// The account this window really uses, as told by its reset times; `None`
    /// when the reading matched no account, or there was none.
    pub account: Option<String>,
    /// Whether a confirmed reading of that account proves it.
    pub proven: bool,
    /// Whether the input carried usage that matched no account and was
    /// dropped.
    pub dropped: bool,
}

/// Records usage from the status line `input` under the account the window
/// really uses, and returns that account with the chained status line.
///
/// A window keeps the login it started with, so after a switch its usage can
/// belong to an account other than the active one. Every account's windows
/// reset at their own times, so a reading belongs to the account whose
/// confirmed reading (from the usage endpoint, with its own token) has the
/// same reset times, the active account first. A reading that matches no
/// confirmed account goes to the active account when that account has no
/// confirmed reading yet, and is dropped otherwise.
///
/// Input that does not parse, or carries no `rate_limits`, records nothing.
/// A window missing from the input keeps its last known value, and a lower
/// reading of a window with the same reset time keeps the higher one. The
/// reading time moves only when the numbers change: a timed run while the
/// main session waits on subagents repeats the last response's numbers, and
/// must not hide that they are getting old.
///
/// # Errors
///
/// Returns the state file's error when it cannot be read or written.
pub fn record(input: &[u8], state_file: &StateFile, now: i64) -> Result<Reading, Error> {
    let limits = serde_json::from_slice::<Input>(input)
        .ok()
        .and_then(|input| input.rate_limits);
    state_file.update(|state| {
        let chained = state.chained_statusline.clone();
        let Some(limits) = limits else {
            return Reading {
                chained,
                account: None,
                proven: false,
                dropped: false,
            };
        };
        let five: Option<Window> = limits.five_hour.map(Window::from);
        let seven: Option<Window> = limits.seven_day.map(Window::from);
        let found = owner(state, five, seven, now);
        let account = found.as_ref().map(|(name, _)| name.clone());
        let proven = found.as_ref().is_some_and(|(_, proven)| *proven);
        if let Some((name, proven)) = found {
            let recorded = state.usage.entry(name).or_insert_with(Recorded::default);
            let five_hour = rising(recorded.five_hour, five);
            let seven_day = rising(recorded.seven_day, seven);
            if five_hour != recorded.five_hour
                || seven_day != recorded.seven_day
                || recorded.read_at.is_none()
            {
                recorded.read_at = Some(now);
            }
            recorded.five_hour = five_hour;
            recorded.seven_day = seven_day;
            recorded.confirmed = proven;
        }
        Reading {
            chained,
            dropped: account.is_none(),
            account,
            proven,
        }
    })
}

/// Returns the window to keep after a `reported` reading of a `known` one.
///
/// Usage in one window only rises until it resets, so a lower reading of the
/// same window is an old one and the known window stays. A window missing
/// from the reading keeps its last known value.
fn rising(known: Option<Window>, reported: Option<Window>) -> Option<Window> {
    match (known, reported) {
        (Some(k), Some(r))
            if (k.resets_at - r.resets_at).abs() <= SAME_RESET_SECONDS
                && r.used_percent < k.used_percent =>
        {
            Some(k)
        }
        (k, None) => k,
        (_, r) => r,
    }
}

/// Returns the account a reading with these windows belongs to, and whether
/// a confirmed reading of that account proves it.
fn owner(
    state: &State,
    five: Option<Window>,
    seven: Option<Window>,
    now: i64,
) -> Option<(String, bool)> {
    let close = |a: i64, b: i64| (a - b).abs() <= SAME_RESET_SECONDS;
    let matches = |recorded: &Recorded| {
        recorded.confirmed
            && (matches!((seven, recorded.seven_day), (Some(r), Some(k)) if close(r.resets_at, k.resets_at))
                || matches!((five, recorded.five_hour), (Some(r), Some(k)) if k.resets_at > now && close(r.resets_at, k.resets_at)))
    };
    let active = state.active.clone()?;
    let active_record = state.usage.get(&active);
    if active_record.is_some_and(&matches) {
        return Some((active, true));
    }
    if let Some((name, _)) = state.usage.iter().find(|(_, r)| matches(r)) {
        return Some((name.clone(), true));
    }
    if active_record.is_some_and(|r| r.confirmed) {
        return None;
    }
    Some((active, false))
}

/// Returns the shell and its command flag that run a status line command as
/// Claude Code does: `/bin/sh -c` on macOS and Linux, and on Windows Git Bash
/// when it is installed, otherwise `PowerShell`
/// (<https://code.claude.com/docs/en/statusline#windows-configuration>).
fn chain_shell() -> (std::ffi::OsString, &'static str) {
    if !cfg!(windows) {
        return ("/bin/sh".into(), "-c");
    }
    let git = crate::claude_code::launch::find_program(
        &std::env::var_os("PATH").unwrap_or_default(),
        &["git.exe"],
    );
    let bash = git_bash_candidates(
        std::env::var_os("CLAUDE_CODE_GIT_BASH_PATH"),
        &[
            std::env::var_os("ProgramFiles"),
            std::env::var_os("ProgramFiles(x86)"),
        ],
        git.as_deref(),
    )
    .into_iter()
    .find(|candidate| candidate.is_file());
    match bash {
        Some(bash) => (bash.into_os_string(), "-c"),
        None => ("powershell.exe".into(), "-Command"),
    }
}

/// Returns where Git Bash may be, in the order Claude Code looks: the path
/// `CLAUDE_CODE_GIT_BASH_PATH` names, `Git\bin\bash.exe` in each program files
/// folder (`%ProgramFiles%`, `%ProgramFiles(x86)%`), and the `bin\bash.exe` of
/// the Git whose `git.exe` is on `PATH` (in its `cmd` or `bin` folder)
/// (<https://code.claude.com/docs/en/setup#install-on-native-windows>).
fn git_bash_candidates(
    configured: Option<std::ffi::OsString>,
    program_files: &[Option<std::ffi::OsString>],
    git: Option<&std::path::Path>,
) -> Vec<std::path::PathBuf> {
    let installed = program_files
        .iter()
        .flatten()
        .filter(|folder| !folder.is_empty())
        .map(|folder| {
            std::path::Path::new(folder)
                .join("Git")
                .join("bin")
                .join("bash.exe")
        });
    let beside_git = git
        .and_then(std::path::Path::parent)
        .and_then(std::path::Path::parent)
        .map(|root| root.join("bin").join("bash.exe"));
    configured
        .filter(|path| !path.is_empty())
        .map(std::path::PathBuf::from)
        .into_iter()
        .chain(installed)
        .chain(beside_git)
        .collect()
}

/// Runs the user's own status line command with the same input and returns
/// what it printed; any failure gives an empty line.
#[must_use]
pub fn chained(command: &str, input: &[u8]) -> Vec<u8> {
    let (shell, flag) = chain_shell();
    let Ok(mut child) = crate::claude_code::launch::without_window(&mut Command::new(shell))
        .arg(flag)
        .arg(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return Vec::new();
    };
    if let Some(mut stdin) = child.stdin.take() {
        // NOTE: no specification governs this: our own design. A command
        // that ignores its input closes the pipe early, which is not an error.
        let _ignored: std::io::Result<()> = stdin.write_all(input);
    }
    child
        .wait_with_output()
        .map(|output| output.stdout)
        .unwrap_or_default()
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{chained, git_bash_candidates, record};
    use crate::error::Error;
    use crate::store::profile::ProfilesRoot;
    use crate::store::state::Recorded;
    use crate::store::state::StateFile;
    use crate::usage::Window;
    use tempfile::TempDir;

    const INPUT: &str = r#"{
        "model": {"display_name": "Opus"},
        "rate_limits": {
            "five_hour": {"used_percentage": 23.5, "resets_at": 2000},
            "seven_day": {"used_percentage": 41.2, "resets_at": 9000}
        }
    }"#;

    fn with_active(home: &TempDir) -> Result<StateFile, Error> {
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        let file = StateFile::in_root(&root);
        file.update(|state| state.active = Some("member1".to_owned()))?;
        Ok(file)
    }

    #[test]
    fn records_both_windows_for_the_active_account() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        let state = file.read()?;
        let recorded = state.usage.get("member1").ok_or("nothing recorded")?;
        assert_eq!(
            recorded.five_hour,
            Some(Window {
                used_percent: 23.5,
                resets_at: 2000
            })
        );
        assert_eq!(
            recorded.seven_day,
            Some(Window {
                used_percent: 41.2,
                resets_at: 9000
            })
        );
        assert_eq!(recorded.read_at, Some(1500));
        Ok(())
    }

    #[test]
    fn a_missing_window_keeps_its_last_value() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        let later =
            r#"{"rate_limits": {"five_hour": {"used_percentage": 30.0, "resets_at": 2000}}}"#;
        record(later.as_bytes(), &file, 1600)?;
        let state = file.read()?;
        let recorded = state.usage.get("member1").ok_or("nothing recorded")?;
        assert_eq!(recorded.five_hour.map(|w| w.used_percent), Some(30.0));
        assert_eq!(recorded.seven_day.map(|w| w.used_percent), Some(41.2));
        Ok(())
    }

    #[test]
    fn a_lower_reading_of_the_same_window_keeps_the_higher_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        let older =
            r#"{"rate_limits": {"five_hour": {"used_percentage": 20.0, "resets_at": 2030}}}"#;
        record(older.as_bytes(), &file, 1600)?;
        let state = file.read()?;
        let recorded = state.usage.get("member1").ok_or("nothing recorded")?;
        assert_eq!(recorded.five_hour.map(|w| w.used_percent), Some(23.5));
        Ok(())
    }

    #[test]
    fn a_lower_reading_of_a_new_window_replaces_the_old_one()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        let next =
            r#"{"rate_limits": {"five_hour": {"used_percentage": 2.0, "resets_at": 20000}}}"#;
        record(next.as_bytes(), &file, 2100)?;
        let state = file.read()?;
        let recorded = state.usage.get("member1").ok_or("nothing recorded")?;
        assert_eq!(
            recorded.five_hour,
            Some(Window {
                used_percent: 2.0,
                resets_at: 20000
            })
        );
        Ok(())
    }

    #[test]
    fn an_unchanged_reading_keeps_its_reading_time() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        record(INPUT.as_bytes(), &file, 1800)?;
        let state = file.read()?;
        let recorded = state.usage.get("member1").ok_or("nothing recorded")?;
        assert_eq!(recorded.read_at, Some(1500), "the numbers are from 1500");
        Ok(())
    }

    #[test]
    fn a_changed_reading_moves_its_reading_time() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        let later =
            r#"{"rate_limits": {"five_hour": {"used_percentage": 30.0, "resets_at": 2000}}}"#;
        record(later.as_bytes(), &file, 1800)?;
        let state = file.read()?;
        let recorded = state.usage.get("member1").ok_or("nothing recorded")?;
        assert_eq!(recorded.read_at, Some(1800));
        Ok(())
    }

    #[test]
    fn input_without_rate_limits_records_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(br#"{"model": {}}"#, &file, 1500)?;
        record(b"not json", &file, 1500)?;
        assert!(file.read()?.usage.is_empty(), "nothing recorded");
        Ok(())
    }

    #[test]
    fn nothing_is_recorded_without_an_active_account() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        let file = StateFile::in_root(&root);
        record(INPUT.as_bytes(), &file, 1500)?;
        assert!(file.read()?.usage.is_empty(), "nothing recorded");
        Ok(())
    }

    /// A confirmed reading of `name` whose 7-day window resets at `seven`.
    fn confirmed(file: &StateFile, name: &str, seven: i64) -> Result<(), Error> {
        file.update(|state| {
            state.usage.insert(
                name.to_owned(),
                Recorded {
                    seven_day: Some(Window {
                        used_percent: 10.0,
                        resets_at: seven,
                    }),
                    read_at: Some(1000),
                    confirmed: true,
                    ..Recorded::default()
                },
            );
        })
    }

    #[test]
    fn a_reading_of_another_account_is_recorded_there() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        confirmed(&file, "member1", 50_000)?;
        confirmed(&file, "member6", 9_000)?;
        let reading = record(INPUT.as_bytes(), &file, 1500)?;
        assert_eq!(reading.account.as_deref(), Some("member6"));
        let state = file.read()?;
        let member6 = state.usage.get("member6").ok_or("member6")?;
        assert_eq!(member6.five_hour.map(|w| w.used_percent), Some(23.5));
        assert!(member6.confirmed, "proven by its reset time");
        let member1 = state.usage.get("member1").ok_or("member1")?;
        assert_eq!(member1.five_hour, None, "the active account is untouched");
        Ok(())
    }

    #[test]
    fn a_reading_of_no_known_account_is_dropped() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        confirmed(&file, "member1", 50_000)?;
        let reading = record(INPUT.as_bytes(), &file, 1500)?;
        assert_eq!(reading.account, None);
        let state = file.read()?;
        assert_eq!(
            state.usage.get("member1").and_then(|r| r.five_hour),
            None,
            "nothing recorded"
        );
        Ok(())
    }

    #[test]
    fn a_reading_before_any_confirmation_goes_to_the_active_account()
    -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        let reading = record(INPUT.as_bytes(), &file, 1500)?;
        assert_eq!(reading.account.as_deref(), Some("member1"));
        let state = file.read()?;
        assert!(
            state.usage.get("member1").is_some_and(|r| !r.confirmed),
            "recorded, not proven"
        );
        Ok(())
    }

    #[test]
    fn the_chained_command_gets_the_same_input() {
        let out = chained("cat", INPUT.as_bytes());
        assert_eq!(out, INPUT.as_bytes());
    }

    #[test]
    fn a_failing_chained_command_prints_what_it_printed() {
        assert_eq!(chained("printf hi; exit 3", b""), b"hi");
    }

    #[test]
    fn git_bash_is_looked_for_where_claude_code_looks() {
        use std::ffi::OsString;
        use std::path::{Path, PathBuf};
        let found = git_bash_candidates(
            Some(OsString::from("/custom/bash.exe")),
            &[
                Some(OsString::from("/pf")),
                None,
                Some(OsString::new()),
                Some(OsString::from("/pf86")),
            ],
            Some(Path::new("/tools/Git/cmd/git.exe")),
        );
        let expected: Vec<PathBuf> = [
            PathBuf::from("/custom/bash.exe"),
            Path::new("/pf").join("Git").join("bin").join("bash.exe"),
            Path::new("/pf86").join("Git").join("bin").join("bash.exe"),
            Path::new("/tools/Git").join("bin").join("bash.exe"),
        ]
        .into();
        assert_eq!(found, expected);
        assert!(git_bash_candidates(None, &[], None).is_empty());
    }
}
