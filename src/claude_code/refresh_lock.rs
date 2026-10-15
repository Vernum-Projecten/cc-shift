// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Claude Code's refresh lock, held while a switch replaces the active login.
//!
//! Before it refreshes a login, Claude Code creates the folder
//! `<config>/.oauth_refresh.lock`, keeps its modification time fresh while
//! it works, treats one older than 60 seconds as abandoned, and re-reads the
//! login under it (observed in the 2.1.300 builds for macOS and Linux,
//! recorded on #90). A switch takes the same folder for the moment it saves
//! and replaces the login of `~/.claude`, so no window refreshes in between.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use crate::error::Error;

/// The name of the lock folder in a config folder.
pub const NAME: &str = ".oauth_refresh.lock";

/// A lock older than this is abandoned, as Claude Code treats it.
pub const STALE: Duration = Duration::from_secs(60);

/// How long a switch waits for a lock Claude Code holds.
pub const WAIT: Duration = Duration::from_secs(10);

/// How often a held lock is tried again.
const POLL: Duration = Duration::from_millis(200);

/// The refresh lock while it is held; dropping it releases the lock.
#[derive(Debug)]
pub struct RefreshLock {
    path: Option<PathBuf>,
}

impl Drop for RefreshLock {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            // NOTE: no specification governs this: our own design. A lock
            // that is already gone needs no release.
            let _released: io::Result<()> = fs::remove_dir(path);
        }
    }
}

/// Takes the refresh lock of `config_dir`, waiting up to `wait` while Claude
/// Code holds it and taking over a lock older than [`STALE`].
///
/// A config folder that does not exist has no Claude Code to wait for, so
/// nothing is locked.
///
/// # Errors
///
/// Returns [`Error::RefreshLocked`] when Claude Code still holds the lock
/// after `wait`, and [`Error::State`] when the lock folder cannot be made.
#[expect(
    clippy::create_dir,
    reason = "the lock is taken by creating its one folder, which must fail when it exists"
)]
pub fn hold(config_dir: &Path, wait: Duration) -> Result<RefreshLock, Error> {
    if !config_dir.is_dir() {
        return Ok(RefreshLock { path: None });
    }
    let path = config_dir.join(NAME);
    let started = Instant::now();
    loop {
        match fs::create_dir(&path) {
            Ok(()) => return Ok(RefreshLock { path: Some(path) }),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                if is_stale(&path) {
                    // NOTE: no specification governs this: our own design.
                    // Another process may take it first; the next try sees.
                    let _taken_over: io::Result<()> = fs::remove_dir(&path);
                    continue;
                }
                if started.elapsed() >= wait {
                    return Err(Error::RefreshLocked(path));
                }
                std::thread::sleep(POLL);
            }
            Err(source) => return Err(Error::State { path, source }),
        }
    }
}

/// Returns whether the lock at `path` was last touched more than [`STALE`]
/// ago; a lock whose age cannot be read is not stale.
fn is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|touched| SystemTime::now().duration_since(touched).ok())
        .is_some_and(|age| age > STALE)
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
#[expect(
    clippy::create_dir,
    reason = "the tests make the lock folder as Claude Code does"
)]
mod tests {
    use super::{NAME, STALE, hold};
    use crate::error::Error;
    use std::fs::{self, File, FileTimes};
    use std::time::{Duration, SystemTime};
    use tempfile::TempDir;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn a_free_lock_is_taken_and_released() -> TestResult {
        let dir = TempDir::new()?;
        let lock = hold(dir.path(), Duration::ZERO)?;
        assert!(dir.path().join(NAME).is_dir(), "held");
        drop(lock);
        assert!(!dir.path().join(NAME).exists(), "released");
        Ok(())
    }

    #[test]
    fn a_fresh_lock_is_waited_for_and_then_reported() -> TestResult {
        let dir = TempDir::new()?;
        fs::create_dir(dir.path().join(NAME))?;
        let held = hold(dir.path(), Duration::from_millis(300));
        assert!(matches!(held, Err(Error::RefreshLocked(_))), "{held:?}");
        assert!(dir.path().join(NAME).is_dir(), "Claude Code's lock is kept");
        Ok(())
    }

    #[test]
    fn a_stale_lock_is_taken_over() -> TestResult {
        let dir = TempDir::new()?;
        let path = dir.path().join(NAME);
        fs::create_dir(&path)?;
        let old = SystemTime::now() - STALE - Duration::from_secs(5);
        File::open(&path)?.set_times(FileTimes::new().set_modified(old))?;
        let lock = hold(dir.path(), Duration::ZERO)?;
        assert!(path.is_dir());
        drop(lock);
        assert!(!path.exists());
        Ok(())
    }

    #[test]
    fn a_missing_config_folder_locks_nothing() -> TestResult {
        let dir = TempDir::new()?;
        let missing = dir.path().join(".claude");
        let lock = hold(&missing, Duration::ZERO)?;
        drop(lock);
        assert!(!missing.exists(), "no folder is made");
        Ok(())
    }
}
