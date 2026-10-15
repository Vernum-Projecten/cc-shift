//! The history of what `cc-shift` did, in `~/.claude-profiles/history.jsonl`.
//!
//! One JSON object per line, appended under its own lock so several windows
//! writing at once never interleave. The file rotates at about 1 MB into
//! `history.1.jsonl`, keeping one older file. No token or email address is
//! ever written.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::store::profile::ProfilesRoot;

/// The size at which the history rotates.
const ROTATE_BYTES: u64 = 1024 * 1024;

/// Why a switch happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    /// A window of the active account reached its threshold.
    Threshold,
    /// A turn hit a usage limit and the hook switched.
    LimitHit,
    /// The user ran `cc-shift use`.
    ByHand,
    /// `cc-shift setup` made an account active.
    Setup,
}

/// Which window a threshold switch was about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    /// The 5-hour window.
    FiveHour,
    /// The 7-day window.
    SevenDay,
}

/// What became of a status line reading that did not go to the active
/// account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingOutcome {
    /// It matched no account and was dropped.
    Dropped,
    /// It matched another account's reset times and was recorded there.
    Elsewhere,
}

/// One thing that happened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    /// `cc-shift setup` ran.
    Setup {
        /// The account it left active.
        active: Option<String>,
    },
    /// The active account changed.
    Switch {
        /// Why.
        reason: Reason,
        /// The window that reached its threshold, for a threshold switch.
        window: Option<WindowKind>,
        /// The account left, if `cc-shift` knew one.
        from: Option<String>,
        /// The account made active.
        to: String,
        /// How full the account left was, in percent.
        from_used: Option<f64>,
        /// How full the new account was, in percent.
        to_used: Option<f64>,
    },
    /// A parked login was refreshed.
    Refresh {
        /// The account.
        account: String,
    },
    /// A refresh was refused; the account needs `/login` again.
    LoggedOut {
        /// The account.
        account: String,
    },
    /// No account but the active one had room left.
    AllFull {
        /// The account `cc-shift` stays on.
        staying_on: String,
        /// The account that frees up first.
        first_free: Option<String>,
        /// When it frees up, in Unix seconds.
        free_at: Option<i64>,
    },
    /// An account is free again after all were full.
    FreeAgain {
        /// The account.
        account: String,
        /// How long all accounts were full, in seconds.
        after_seconds: i64,
    },
    /// The early start ran.
    EarlyStart {
        /// The accounts whose 5-hour window it started.
        started: Vec<String>,
        /// How many it was set to start.
        wanted: u8,
    },
    /// The early start ran too long after its set time and started nothing.
    EarlyStartSkipped {
        /// Why.
        reason: String,
    },
    /// The early start could not start an account's window.
    EarlyStartFailed {
        /// The account.
        account: String,
        /// Why.
        reason: String,
    },
    /// A status line reading that did not go to the active account.
    Reading {
        /// The account it was recorded under, if any.
        account: Option<String>,
        /// What became of it.
        outcome: ReadingOutcome,
    },
}

impl Event {
    /// Returns whether the event is only shown with `--verbose`.
    #[must_use]
    pub fn is_diagnostic(&self) -> bool {
        matches!(self, Self::Reading { .. })
    }
}

/// One line of the history: when, and what.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    /// When it happened, RFC 3339 in UTC.
    pub at: String,
    /// What happened.
    #[serde(flatten)]
    pub event: Event,
}

impl Entry {
    /// Returns an entry for `event` at `now` (Unix seconds).
    #[must_use]
    pub fn new(event: Event, now: i64) -> Self {
        Self {
            at: jiff::Timestamp::from_second(now)
                .map_or_else(|_| now.to_string(), |t| t.to_string()),
            event,
        }
    }

    /// Returns the time of the entry in Unix seconds, if it parses.
    #[must_use]
    pub fn seconds(&self) -> Option<i64> {
        self.at
            .parse::<jiff::Timestamp>()
            .ok()
            .map(jiff::Timestamp::as_second)
    }
}

/// The history files and their lock inside the profiles root.
#[derive(Debug, Clone)]
pub struct History {
    current: PathBuf,
    older: PathBuf,
    lock: PathBuf,
}

impl History {
    /// Returns the history of a profiles root.
    #[must_use]
    pub fn in_root(root: &ProfilesRoot) -> Self {
        Self {
            current: root.path().join("history.jsonl"),
            older: root.path().join("history.1.jsonl"),
            lock: root.path().join(".history-lock"),
        }
    }

    /// Appends one entry, rotating first when the file has grown past about
    /// 1 MB.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when a history file or its lock cannot be
    /// used.
    pub fn append(&self, entry: &Entry) -> Result<(), Error> {
        if let Some(parent) = self.current.parent() {
            fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
        }
        let guard = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock)
            .map_err(|e| io_error(&self.lock, e))?;
        guard.lock().map_err(|e| io_error(&self.lock, e))?;
        if fs::metadata(&self.current).is_ok_and(|m| m.len() >= ROTATE_BYTES) {
            fs::rename(&self.current, &self.older).map_err(|e| io_error(&self.older, e))?;
        }
        let mut line = serde_json::to_vec(entry).map_err(|source| Error::StateFormat {
            path: self.current.clone(),
            source,
        })?;
        line.push(b'\n');
        let mut file: File = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.current)
            .map_err(|e| io_error(&self.current, e))?;
        file.write_all(&line)
            .map_err(|e| io_error(&self.current, e))
    }

    /// Returns every entry, oldest first, at or after `since` (Unix
    /// seconds) when given. A line that does not parse is skipped.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when a history file exists but cannot be
    /// read.
    pub fn read(&self, since: Option<i64>) -> Result<Vec<Entry>, Error> {
        let mut entries = Vec::new();
        for path in [&self.older, &self.current] {
            let text = match fs::read_to_string(path) {
                Ok(text) => text,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(io_error(path, e)),
            };
            entries.extend(
                text.lines()
                    .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
                    .filter(|entry| {
                        since.is_none_or(|from| entry.seconds().is_some_and(|at| at >= from))
                    }),
            );
        }
        Ok(entries)
    }
}

/// Appends `event` to the history of `root`, ignoring a failure: the history
/// must never stop a switch or break a status line.
pub fn note(root: &ProfilesRoot, event: Event, now: i64) {
    // NOTE: no specification governs this: our own design. The history is a
    // record for the user; losing one line is better than failing the work.
    let _ignored: Result<(), Error> = History::in_root(root).append(&Entry::new(event, now));
}

fn io_error(path: &Path, source: io::Error) -> Error {
    Error::State {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{Entry, Event, History, ROTATE_BYTES, Reason, WindowKind};
    use crate::store::profile::ProfilesRoot;
    use std::fs;
    use tempfile::TempDir;

    fn history(home: &TempDir) -> Result<History, Box<dyn std::error::Error>> {
        Ok(History::in_root(&ProfilesRoot::from_home(Some(
            home.path().into(),
        ))?))
    }

    fn switch(to: &str) -> Event {
        Event::Switch {
            reason: Reason::Threshold,
            window: Some(WindowKind::FiveHour),
            from: Some("work".to_owned()),
            to: to.to_owned(),
            from_used: Some(95.0),
            to_used: Some(10.0),
        }
    }

    #[test]
    fn an_entry_is_one_json_line_in_the_approved_shape() -> Result<(), Box<dyn std::error::Error>> {
        let line = serde_json::to_value(Entry::new(switch("spare"), 1_792_057_632))?;
        assert_eq!(
            line,
            serde_json::json!({
                "at": "2026-10-15T09:47:12Z",
                "event": "switch",
                "reason": "threshold",
                "window": "five_hour",
                "from": "work",
                "to": "spare",
                "from_used": 95.0,
                "to_used": 10.0
            })
        );
        Ok(())
    }

    #[test]
    fn appended_entries_are_read_back_in_order() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let history = history(&home)?;
        history.append(&Entry::new(switch("a"), 1000))?;
        history.append(&Entry::new(switch("b"), 2000))?;
        let entries = history.read(None)?;
        assert_eq!(entries.len(), 2);
        assert_eq!(history.read(Some(1500))?.len(), 1, "since filters");
        Ok(())
    }

    #[test]
    fn a_full_file_rotates_and_both_are_read() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let history = history(&home)?;
        history.append(&Entry::new(switch("a"), 1000))?;
        let current = home.path().join(".claude-profiles/history.jsonl");
        let mut text = fs::read_to_string(&current)?;
        while u64::try_from(text.len())? < ROTATE_BYTES {
            text.push_str("x\n");
        }
        fs::write(&current, text)?;
        history.append(&Entry::new(switch("b"), 2000))?;
        assert!(
            home.path()
                .join(".claude-profiles/history.1.jsonl")
                .exists(),
            "rotated"
        );
        assert_eq!(history.read(None)?.len(), 2, "junk lines skipped");
        Ok(())
    }
}
