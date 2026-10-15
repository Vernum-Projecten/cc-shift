//! `cc-shift early`: starting some accounts' 5-hour windows before the
//! workday.

use std::io::Write;
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::anthropic::parked;
use crate::command::{home, refresh, unix_now};
use crate::early::{self, EarlyStart, MOST_ACCOUNTS, launchd, start};
use crate::error::Error;
use crate::store::history::{self, Event};
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::usage::accounts;

/// What `cc-shift early` is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Show the setting and the next run.
    Show,
    /// Turn the early start on, changing the time or the count when given.
    On {
        /// The time of the run, as hour and minute.
        at: Option<(u8, u8)>,
        /// How many accounts to start.
        accounts: Option<u8>,
    },
    /// Turn the early start off.
    Off,
    /// Start the windows now; run by the `launchd` agent.
    Run,
}

/// Parses the words after `cc-shift early`, or returns the misuse.
///
/// # Errors
///
/// Returns the reason when the words are not an action this command takes.
pub fn action(flags: &[&str]) -> Result<Action, String> {
    match flags {
        [] => Ok(Action::Show),
        ["off"] => Ok(Action::Off),
        ["run"] => Ok(Action::Run),
        ["on", rest @ ..] => {
            let mut at = None;
            let mut count = None;
            let mut words = rest.iter();
            while let Some(word) = words.next() {
                match *word {
                    "--at" if at.is_none() => {
                        at =
                            Some(words.next().and_then(|w| early::parse_time(w)).ok_or(
                                "--at takes a time of day as HH:MM, like 06:00".to_owned(),
                            )?);
                    }
                    "--accounts" if count.is_none() => {
                        count = Some(
                            words
                                .next()
                                .and_then(|w| w.parse::<u8>().ok())
                                .filter(|n| (1..=MOST_ACCOUNTS).contains(n))
                                .ok_or(format!(
                                    "--accounts takes a number from 1 to {MOST_ACCOUNTS}"
                                ))?,
                        );
                    }
                    other => return Err(format!("early on does not take '{other}'")),
                }
            }
            Ok(Action::On {
                at,
                accounts: count,
            })
        }
        _ => Err("early takes on, off or nothing".to_owned()),
    }
}

/// Runs the action against the real files, `launchd` and Claude Code.
///
/// # Errors
///
/// Returns the error of a missing `HOME`, the profiles root, the state file,
/// a missing `claude`, or the `launchd` agent.
pub fn run(action: Action) -> Result<(), Error> {
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let file = StateFile::in_root(&root);
    match action {
        Action::Show => {
            let setting = file.read()?.early_start;
            print(&describe(
                setting.as_ref(),
                Timestamp::now(),
                &TimeZone::system(),
            ));
            Ok(())
        }
        Action::On { at, accounts } => turn_on(&home, &file, at, accounts),
        Action::Off => {
            file.update(|s| s.early_start = None)?;
            launchd::remove(&home)?;
            print("Early start is off.\n");
            Ok(())
        }
        Action::Run => start_windows(&home, &root, &file),
    }
}

/// Saves the setting and installs the agent that runs it.
fn turn_on(
    home: &Path,
    file: &StateFile,
    at: Option<(u8, u8)>,
    count: Option<u8>,
) -> Result<(), Error> {
    let path = std::env::var("PATH").unwrap_or_default();
    if find_on(&path, "claude").is_none() {
        return Err(Error::ClaudeNotFound);
    }
    let binary = std::env::current_exe()
        .map_err(|e| Error::Settings(format!("cannot find the cc-shift binary: {e}")))?;
    let setting = file.update(|s| {
        let mut setting = s.early_start.unwrap_or_default();
        if let Some((hour, minute)) = at {
            setting.hour = hour;
            setting.minute = minute;
        }
        if let Some(count) = count {
            setting.accounts = count;
        }
        s.early_start = Some(setting);
        setting
    })?;
    launchd::install(home, setting, &binary, &path)?;
    print(&format!(
        "Early start is on: Monday to Friday at {}, {}.\n\
         cc-shift picks the accounts each morning, the ones it would use first.\n\
         Each costs one short Haiku message.\n",
        setting.at(),
        setting.count(),
    ));
    Ok(())
}

/// Starts the windows of the accounts the setting picks, on a workday.
fn start_windows(home: &Path, root: &ProfilesRoot, file: &StateFile) -> Result<(), Error> {
    let Some(setting) = file.read()?.early_start else {
        return Ok(());
    };
    let today = Timestamp::now().to_zoned(TimeZone::system()).weekday();
    if !early::is_workday(today) {
        return Ok(());
    }
    // NOTE: no specification governs this: our own design. A failed refresh
    // leaves the last numbers, and picking from them is still sound.
    let _reports: Result<Vec<parked::Report>, Error> = refresh::refresh_parked();
    let _held = parked::hold_refreshes(root)?;
    let state = file.read()?;
    let now = unix_now();
    let all = accounts(&root.list()?, &state);
    let workdir = home.join("Library/Caches/cc-shift/early-start");
    let mut started = Vec::new();
    for account in early::pick(&all, setting.accounts, now) {
        let config_dir: Option<PathBuf> = (state.active.as_deref() != Some(account.name.as_str()))
            .then(|| root.path().join(&account.name));
        match start::start_window(config_dir.as_deref(), &workdir) {
            Ok(()) => started.push(account.name.clone()),
            Err(error) => history::note(
                root,
                Event::EarlyStartFailed {
                    account: account.name.clone(),
                    reason: error.to_string(),
                },
                unix_now(),
            ),
        }
    }
    history::note(
        root,
        Event::EarlyStart {
            started,
            wanted: setting.accounts,
        },
        now,
    );
    Ok(())
}

/// Returns the line `cc-shift early` prints for `setting` at `now`.
#[must_use]
pub fn describe(setting: Option<&EarlyStart>, now: Timestamp, zone: &TimeZone) -> String {
    let Some(setting) = setting else {
        return "Early start: off · turn it on with cc-shift early on\n".to_owned();
    };
    let mut line = format!(
        "Early start: on · Monday to Friday at {} · {}",
        setting.at(),
        setting.count()
    );
    if let Some(next) = early::next_run(*setting, now, zone) {
        line.push_str(" · next ");
        line.push_str(&next.to_zoned(zone.clone()).strftime("%a %H:%M").to_string());
    }
    line.push('\n');
    line
}

/// Returns the first `name` in the folders of `path`.
fn find_on(path: &str, name: &str) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|folder| folder.join(name))
        .find(|candidate| candidate.is_file())
}

/// Writes `text` to stdout.
fn print(text: &str) {
    // NOTE: no specification governs this: our own design. A failed write to
    // stdout (a closed pipe) is not an error for a message.
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{Action, action, describe};
    use crate::early::EarlyStart;
    use jiff::Timestamp;
    use jiff::tz::TimeZone;

    #[test]
    fn the_actions_parse() {
        assert_eq!(action(&[]), Ok(Action::Show));
        assert_eq!(action(&["off"]), Ok(Action::Off));
        assert_eq!(action(&["run"]), Ok(Action::Run));
        assert_eq!(
            action(&["on"]),
            Ok(Action::On {
                at: None,
                accounts: None
            })
        );
        assert_eq!(
            action(&["on", "--accounts", "3", "--at", "05:30"]),
            Ok(Action::On {
                at: Some((5, 30)),
                accounts: Some(3)
            })
        );
    }

    #[test]
    fn bad_words_are_refused() {
        for words in [
            &["on", "--at"][..],
            &["on", "--at", "6"],
            &["on", "--accounts", "0"],
            &["on", "--accounts", "21"],
            &["on", "--accounts", "two"],
            &["on", "--at", "06:00", "--at", "07:00"],
            &["on", "--days", "7"],
            &["off", "now"],
            &["maybe"],
        ] {
            assert!(action(words).is_err(), "{words:?}");
        }
    }

    #[test]
    fn the_setting_reads_as_one_line() {
        let thursday_noon = Timestamp::from_second(1_792_065_600).unwrap();
        assert_eq!(
            describe(Some(&EarlyStart::default()), thursday_noon, &TimeZone::UTC),
            "Early start: on · Monday to Friday at 06:00 · 2 accounts · next Fri 06:00\n"
        );
        assert_eq!(
            describe(None, thursday_noon, &TimeZone::UTC),
            "Early start: off · turn it on with cc-shift early on\n"
        );
    }
}
