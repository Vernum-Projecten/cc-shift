//! `cs statusline`: the status line command Claude Code runs after every
//! reply.
//!
//! Claude Code passes a JSON object on stdin whose `rate_limits` holds the
//! active account's `five_hour` and `seven_day` windows, each with
//! `used_percentage` and `resets_at`
//! (<https://code.claude.com/docs/en/statusline>). `cs` records them and then
//! prints the status line the user had before, if any.

use std::io::Write;
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::error::Error;
use crate::store::state::{Recorded, State, StateFile};
use crate::usage::Window;

/// The part of the status line input `cs` reads; every other field is
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

/// Records the active account's usage from the status line `input`, and
/// returns the chained status line command, if the user had one.
///
/// Input that does not parse, or carries no `rate_limits`, records nothing.
/// A window missing from the input keeps its last known value. Nothing is
/// recorded before `cs` knows which account is active.
///
/// # Errors
///
/// Returns the state file's error when it cannot be read or written.
pub fn record(input: &[u8], state_file: &StateFile, now: i64) -> Result<Option<String>, Error> {
    let limits = serde_json::from_slice::<Input>(input)
        .ok()
        .and_then(|input| input.rate_limits);
    state_file.update(|state| {
        if let (Some(limits), Some(active)) = (limits, state.active.clone()) {
            let recorded = state.usage.entry(active).or_insert_with(Recorded::default);
            if let Some(window) = limits.five_hour {
                recorded.five_hour = Some(window.into());
            }
            if let Some(window) = limits.seven_day {
                recorded.seven_day = Some(window.into());
            }
            recorded.read_at = Some(now);
        }
        state.chained_statusline.clone()
    })
}

/// Returns the `cs` part of the status line: the active account and both
/// windows with the time to reset, for example
/// `claude5 · 5h 31% (4h0m) · 7d 26% (5d8h)`.
#[must_use]
pub fn summary(state: &State, now: i64) -> String {
    let Some(active) = &state.active else {
        return "cs: run cs setup".to_owned();
    };
    let recorded = state.usage.get(active);
    let window = |label: &str, window: Option<Window>| match window {
        Some(w) if w.resets_at > now => format!(
            " · {label} {:.0}% ({})",
            w.used_percent,
            crate::usage::time_left(w.resets_at - now)
        ),
        Some(_) => format!(" · {label} 0%"),
        None => String::new(),
    };
    format!(
        "{active}{}{}",
        window("5h", recorded.and_then(|r| r.five_hour)),
        window("7d", recorded.and_then(|r| r.seven_day))
    )
}

/// Runs the user's own status line command with the same input and returns
/// what it printed; any failure gives an empty line.
#[must_use]
pub fn chained(command: &str, input: &[u8]) -> Vec<u8> {
    let Ok(mut child) = Command::new("/bin/sh")
        .arg("-c")
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
    use super::{chained, record, summary};
    use crate::error::Error;
    use crate::store::profile::ProfilesRoot;
    use crate::store::state::StateFile;
    use crate::store::state::{Recorded, State};
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
        file.update(|state| state.active = Some("claude1".to_owned()))?;
        Ok(file)
    }

    #[test]
    fn records_both_windows_for_the_active_account() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = with_active(&home)?;
        record(INPUT.as_bytes(), &file, 1500)?;
        let state = file.read()?;
        let recorded = state.usage.get("claude1").ok_or("nothing recorded")?;
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
        let recorded = state.usage.get("claude1").ok_or("nothing recorded")?;
        assert_eq!(recorded.five_hour.map(|w| w.used_percent), Some(30.0));
        assert_eq!(recorded.seven_day.map(|w| w.used_percent), Some(41.2));
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

    #[test]
    fn the_summary_names_the_account_and_both_windows() {
        let mut state = State {
            active: Some("claude5".to_owned()),
            ..State::default()
        };
        state.usage.insert(
            "claude5".to_owned(),
            Recorded {
                five_hour: Some(Window {
                    used_percent: 31.0,
                    resets_at: 1000 + 14_400,
                }),
                seven_day: Some(Window {
                    used_percent: 26.4,
                    resets_at: 1000 + 460_800,
                }),
                read_at: Some(1000),
            },
        );
        assert_eq!(
            summary(&state, 1000),
            "claude5 · 5h 31% (4h0m) · 7d 26% (5d8h)"
        );
    }

    #[test]
    fn the_summary_before_setup_says_so() {
        assert_eq!(summary(&State::default(), 0), "cs: run cs setup");
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
}
