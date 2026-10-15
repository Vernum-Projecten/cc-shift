//! The state `cs` keeps between runs, in `~/.claude-profiles/state.json`.
//!
//! Every read and write takes the lock file `~/.claude-profiles/.lock`, so
//! the status line of several windows, the hook and the commands never see
//! half a change. A write goes to a temporary file that replaces
//! `state.json` in one rename.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::store::profile::ProfilesRoot;
use crate::usage::Window;

/// Everything `cs` remembers. No field ever holds a token.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct State {
    /// The account whose login is in `~/.claude`, once `cs` has set one.
    pub active: Option<String>,
    /// The last usage read per account, by profile name.
    pub usage: BTreeMap<String, Recorded>,
    /// The status line command the user had before `cs setup`, run after
    /// `cs statusline` records usage.
    pub chained_statusline: Option<String>,
    /// A switch that has started and not yet finished.
    pub pending: Option<Pending>,
    /// When `cs` last switched on its own, in Unix seconds.
    pub switched_at: Option<i64>,
    /// Accounts whose refresh Anthropic refused; they need `/login` again.
    pub logged_out: BTreeSet<String>,
}

/// The usage of one account as last read.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Recorded {
    /// The 5-hour window, if it was reported.
    pub five_hour: Option<Window>,
    /// The 7-day window, if it was reported.
    pub seven_day: Option<Window>,
    /// When the numbers were read, in Unix seconds.
    pub read_at: Option<i64>,
}

/// A switch in progress, saved before each step so the next run can finish
/// it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pending {
    /// The account that was active, if `cs` knew one.
    pub from: Option<String>,
    /// The account being made active.
    pub to: String,
    /// The step about to run.
    pub step: Step,
}

/// The steps of a switch, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Save the login of `~/.claude` back to the account that was active,
    /// then write the next account's parked login into `~/.claude`.
    Move,
    /// Clear the next account's parked login and record it as active.
    ClearParked,
}

/// The state file and its lock inside the profiles root.
#[derive(Debug, Clone)]
pub struct StateFile {
    path: PathBuf,
    lock: PathBuf,
    root: PathBuf,
}

impl StateFile {
    /// Returns the state file of a profiles root.
    #[must_use]
    pub fn in_root(root: &ProfilesRoot) -> Self {
        Self {
            path: root.path().join("state.json"),
            lock: root.path().join(".lock"),
            root: root.path().to_path_buf(),
        }
    }

    /// Reads the state under the lock; a missing file is the empty state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when the lock or the file cannot be used,
    /// and [`Error::StateFormat`] when the file does not parse.
    pub fn read(&self) -> Result<State, Error> {
        self.lock()?.load()
    }

    /// Reads the state, applies `change` and writes it back, all under the
    /// lock, and returns what `change` returned.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when the root, the lock or the file cannot
    /// be used, and [`Error::StateFormat`] when the file does not parse.
    pub fn update<T>(&self, change: impl FnOnce(&mut State) -> T) -> Result<T, Error> {
        let locked = self.lock()?;
        let mut state = locked.load()?;
        let before = state.clone();
        let result = change(&mut state);
        if state != before {
            locked.save(&state)?;
        }
        Ok(result)
    }

    /// Takes the lock, which is held until the returned guard is dropped.
    ///
    /// A switch holds it across its Keychain steps and saves its progress
    /// between them.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when the root or the lock cannot be used.
    pub fn lock(&self) -> Result<Locked<'_>, Error> {
        fs::create_dir_all(&self.root).map_err(|e| io_error(&self.root, e))?;
        let guard = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.lock)
            .map_err(|e| io_error(&self.lock, e))?;
        guard.lock().map_err(|e| io_error(&self.lock, e))?;
        Ok(Locked {
            file: self,
            _guard: guard,
        })
    }
}

/// The state file while this process holds its lock.
#[derive(Debug)]
pub struct Locked<'a> {
    file: &'a StateFile,
    _guard: File,
}

impl Locked<'_> {
    /// Reads the state; a missing file is the empty state.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when the file cannot be read, and
    /// [`Error::StateFormat`] when it does not parse.
    pub fn load(&self) -> Result<State, Error> {
        let path = &self.file.path;
        match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|source| Error::StateFormat {
                path: path.clone(),
                source,
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(State::default()),
            Err(e) => Err(io_error(path, e)),
        }
    }

    /// Replaces the state file in one rename.
    ///
    /// # Errors
    ///
    /// Returns [`Error::State`] when the file cannot be written, and
    /// [`Error::StateFormat`] when the state cannot be encoded.
    pub fn save(&self, state: &State) -> Result<(), Error> {
        let path = &self.file.path;
        let temporary = path.with_extension("json.tmp");
        let text = serde_json::to_vec_pretty(state).map_err(|source| Error::StateFormat {
            path: path.clone(),
            source,
        })?;
        fs::write(&temporary, text).map_err(|e| io_error(&temporary, e))?;
        fs::rename(&temporary, path).map_err(|e| io_error(path, e))
    }
}

/// Wraps an I/O error on one of the state paths.
fn io_error(path: &Path, source: io::Error) -> Error {
    Error::State {
        path: path.to_path_buf(),
        source,
    }
}
#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{Recorded, State, StateFile};
    use crate::error::Error;
    use crate::store::profile::ProfilesRoot;
    use crate::usage::Window;
    use std::fs;
    use tempfile::TempDir;

    fn state_file(home: &TempDir) -> Result<StateFile, Error> {
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        Ok(StateFile::in_root(&root))
    }

    #[test]
    fn a_missing_file_is_the_empty_state() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        assert_eq!(state_file(&home)?.read()?, State::default());
        Ok(())
    }

    #[test]
    fn an_update_is_read_back() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = state_file(&home)?;
        let recorded = Recorded {
            five_hour: Some(Window {
                used_percent: 42.0,
                resets_at: 1000,
            }),
            seven_day: None,
            read_at: Some(900),
        };
        file.update(|state| {
            state.active = Some("claude1".to_owned());
            state.usage.insert("claude1".to_owned(), recorded.clone());
        })?;
        let state = file.read()?;
        assert_eq!(state.active.as_deref(), Some("claude1"));
        assert_eq!(state.usage.get("claude1"), Some(&recorded));
        Ok(())
    }

    #[test]
    fn a_read_writes_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        state_file(&home)?.read()?;
        assert!(!home.path().join(".claude-profiles/state.json").exists());
        Ok(())
    }

    #[test]
    fn a_damaged_file_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let file = state_file(&home)?;
        fs::create_dir_all(home.path().join(".claude-profiles"))?;
        fs::write(
            home.path().join(".claude-profiles/state.json"),
            "{ not json",
        )?;
        assert!(matches!(file.read(), Err(Error::StateFormat { .. })));
        Ok(())
    }
}
