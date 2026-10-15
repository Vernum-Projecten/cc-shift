// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The login files of Linux and Windows: `.credentials.json` in each config
//! folder.
//!
//! On Linux and Windows, Claude Code keeps a config folder's login in
//! `<folder>/.credentials.json`: with mode 0600 on Linux, and with the
//! access rules of the user's profile folder on Windows
//! (<https://code.claude.com/docs/en/authentication#credential-management>).
//! A running window checks the file before each request and drops its cached
//! login when it changed (the 2.1.300 builds, recorded on #85 and #102), so a
//! login is written to a fresh file and renamed into place: the window then
//! sees a new file and follows.
//!
//! On Windows a server-side flag can move Claude Code's logins into Windows
//! Credential Manager, which it then reads before the file. A moved file
//! would be ignored there, so every access stops while the flag is on (#104).

use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::login::{Login, Logins, Slot};
use crate::store::profile::{ProfilesRoot, home_var};

/// The name of the login file in a config folder.
pub const FILE_NAME: &str = ".credentials.json";

/// The variable that makes Claude Code on Windows use Credential Manager.
pub const FORCE_CREDMAN: &str = "CLAUDE_CODE_FORCE_WINDOWS_CREDMAN";

/// How often a rename blocked by another process is tried on Windows.
const RENAME_TRIES: u32 = 10;

/// The login files of `~/.claude` and of every profile.
#[derive(Debug, Clone)]
pub struct CredentialFiles {
    active: PathBuf,
    root: PathBuf,
}

impl CredentialFiles {
    /// Returns the login files in the user's home folder (`HOME`, or
    /// `USERPROFILE` on Windows), for the profiles in `root`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoHome`] when the home folder is unset or empty.
    pub fn for_user(root: &ProfilesRoot) -> Result<Self, Error> {
        let home = home_var()
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)
            .ok_or(Error::NoHome)?;
        Ok(Self::new(&home.join(".claude"), root.path()))
    }

    /// Returns the login files with `active` as the config folder of the
    /// active login and `root` as the profiles root.
    #[must_use]
    pub fn new(active: &Path, root: &Path) -> Self {
        Self {
            active: active.to_path_buf(),
            root: root.to_path_buf(),
        }
    }

    /// Returns the config folder of a slot.
    fn folder(&self, slot: &Slot) -> PathBuf {
        match slot {
            Slot::Active => self.active.clone(),
            Slot::Parked(name) => self.root.join(name),
        }
    }

    /// Returns the path of a slot's file.
    #[must_use]
    pub fn path(&self, slot: &Slot) -> PathBuf {
        self.folder(slot).join(FILE_NAME)
    }

    /// Returns the global config Claude Code reads for a slot's folder:
    /// `.config.json` in the folder when it is not empty, otherwise
    /// `~/.claude.json` for `~/.claude` and `.claude.json` in a profile.
    fn global_config(&self, slot: &Slot) -> PathBuf {
        let folder = self.folder(slot);
        let config = folder.join(".config.json");
        if fs::metadata(&config).is_ok_and(|m| m.len() > 0) {
            return config;
        }
        match slot {
            Slot::Active => folder.with_file_name(".claude.json"),
            Slot::Parked(_) => folder.join(".claude.json"),
        }
    }

    /// Stops when Claude Code keeps this slot's login in Credential Manager.
    fn refuse_credential_manager(&self, slot: &Slot) -> Result<(), Error> {
        let forced = std::env::var_os(FORCE_CREDMAN);
        if cfg!(windows) && uses_credential_manager(&self.global_config(slot), forced.as_deref()) {
            Err(Error::CredentialManager(self.folder(slot)))
        } else {
            Ok(())
        }
    }

    fn fail(slot: &Slot, path: &Path, reason: impl std::fmt::Display) -> Error {
        Error::LoginStore {
            slot: slot.to_string(),
            reason: format!("{}: {reason}", path.display()),
        }
    }

    /// Refuses a login file that is a symbolic link, as Claude Code does.
    fn refuse_link(slot: &Slot, path: &Path) -> Result<(), Error> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => Err(Self::fail(
                slot,
                path,
                "is a symbolic link; Claude Code does not use a linked login file",
            )),
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Self::fail(slot, path, e)),
        }
    }

    /// Runs the checks every access makes first.
    fn check(&self, slot: &Slot, path: &Path) -> Result<(), Error> {
        self.refuse_credential_manager(slot)?;
        Self::refuse_link(slot, path)
    }
}

impl Logins for CredentialFiles {
    fn read(&self, slot: &Slot) -> Result<Option<Login>, Error> {
        let path = self.path(slot);
        self.check(slot, &path)?;
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(Login::from_bytes(bytes))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Self::fail(slot, &path, e)),
        }
    }

    fn write(&self, slot: &Slot, login: &Login) -> Result<(), Error> {
        let path = self.path(slot);
        self.check(slot, &path)?;
        let folder = path
            .parent()
            .ok_or_else(|| Self::fail(slot, &path, "has no folder"))?;
        let temporary = folder.join(format!("{FILE_NAME}.cc-shift-{}", std::process::id()));
        // NOTE: no specification governs this: our own design. A file with this
        // name is left from an earlier run with the same process id.
        let _stale: io::Result<()> = fs::remove_file(&temporary);
        let written = write_new(&temporary, login.as_bytes())
            .and_then(|()| replace(&temporary, &path))
            .and_then(|()| sync_folder(folder));
        if let Err(e) = written {
            // NOTE: no specification governs this: our own design. A
            // temporary file that was never created is already gone.
            let _gone: io::Result<()> = fs::remove_file(&temporary);
            return Err(Self::fail(slot, &path, e));
        }
        if self.read(slot)?.as_ref() == Some(login) {
            Ok(())
        } else {
            Err(Self::fail(slot, &path, "the login did not reach the file"))
        }
    }

    fn clear(&self, slot: &Slot) -> Result<(), Error> {
        let path = self.path(slot);
        self.check(slot, &path)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Self::fail(slot, &path, e)),
        }
    }
}

/// Returns whether Claude Code on Windows keeps a folder's login in
/// Credential Manager.
///
/// That is when `forced` (the [`FORCE_CREDMAN`] variable) is `1`, or when the
/// folder's global config caches the server feature `tengu_windows_credman`
/// as `true` (the 2.1.300 Windows build, #104).
#[must_use]
pub fn uses_credential_manager(global_config: &Path, forced: Option<&OsStr>) -> bool {
    if forced.is_some_and(|v| v == "1") {
        return true;
    }
    // NOTE: no specification governs this: our own design. A missing or
    // unreadable config holds no flag, which is Claude Code's default too.
    fs::read(global_config)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|json| {
            json.get("cachedGrowthBookFeatures")?
                .get("tengu_windows_credman")?
                .as_bool()
        })
        .unwrap_or(false)
}

/// Creates `path` (mode 0600 on Unix), writes `bytes` and flushes them to
/// disk; fails when `path` already exists.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Renames `from` over `to`. On Windows a file another process holds open
/// for a moment (a virus scanner, the search indexer) refuses the rename, so
/// it is tried again for about a second.
fn replace(from: &Path, to: &Path) -> io::Result<()> {
    let mut tries = 1;
    loop {
        match fs::rename(from, to) {
            Err(e) if cfg!(windows) && tries < RENAME_TRIES && held_open(&e) => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            result => return result,
        }
    }
}

/// Returns whether `e` is Windows refusing a file another process has open:
/// access denied (5) or a sharing violation (32).
fn held_open(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(5 | 32))
}

/// Flushes the folder entry of the rename to disk on Unix; Windows cannot
/// open a folder as a file and needs no such step.
fn sync_folder(folder: &Path) -> io::Result<()> {
    if cfg!(unix) {
        fs::File::open(folder)?.sync_all()
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{CredentialFiles, FILE_NAME, uses_credential_manager};
    use crate::login::{Login, Logins, Slot};
    use std::ffi::OsStr;
    use std::fs;
    use tempfile::TempDir;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn files(home: &TempDir) -> Result<CredentialFiles, Box<dyn std::error::Error>> {
        let claude = home.path().join(".claude");
        let root = home.path().join(".claude-profiles");
        fs::create_dir_all(&claude)?;
        fs::create_dir_all(root.join("work"))?;
        Ok(CredentialFiles::new(&claude, &root))
    }

    fn login(text: &str) -> Login {
        Login::from_bytes(text.as_bytes().to_vec())
    }

    #[test]
    fn slots_are_the_credentials_file_of_each_folder() -> TestResult {
        let home = TempDir::new()?;
        let files = files(&home)?;
        assert_eq!(
            files.path(&Slot::Active),
            home.path().join(".claude").join(FILE_NAME)
        );
        assert_eq!(
            files.path(&Slot::Parked("work".to_owned())),
            home.path().join(".claude-profiles/work").join(FILE_NAME)
        );
        Ok(())
    }

    #[test]
    fn a_missing_file_holds_no_login() -> TestResult {
        let home = TempDir::new()?;
        assert_eq!(files(&home)?.read(&Slot::Active)?, None);
        Ok(())
    }

    #[test]
    fn a_write_replaces_the_file_with_mode_0600() -> TestResult {
        let home = TempDir::new()?;
        let files = files(&home)?;
        let slot = Slot::Parked("work".to_owned());
        files.write(&slot, &login("{\"first\":1}"))?;
        files.write(&slot, &login("{\"second\":2}"))?;
        assert_eq!(files.read(&slot)?, Some(login("{\"second\":2}")));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(files.path(&slot))?.permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
        let leftovers: Vec<_> = fs::read_dir(home.path().join(".claude-profiles/work"))?
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers, [FILE_NAME], "no temporary file is left");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_write_gives_the_file_a_new_inode() -> TestResult {
        use std::os::unix::fs::MetadataExt;
        let home = TempDir::new()?;
        let files = files(&home)?;
        files.write(&Slot::Active, &login("a"))?;
        let before = fs::metadata(files.path(&Slot::Active))?.ino();
        files.write(&Slot::Active, &login("b"))?;
        let after = fs::metadata(files.path(&Slot::Active))?.ino();
        assert_ne!(before, after, "a running window must see a new file");
        Ok(())
    }

    #[test]
    fn clear_removes_the_file_and_an_empty_slot_stays_empty() -> TestResult {
        let home = TempDir::new()?;
        let files = files(&home)?;
        files.write(&Slot::Active, &login("a"))?;
        files.clear(&Slot::Active)?;
        assert_eq!(files.read(&Slot::Active)?, None);
        files.clear(&Slot::Active)?;
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_file_is_refused() -> TestResult {
        use std::os::unix::fs::symlink;
        let home = TempDir::new()?;
        let files = files(&home)?;
        let elsewhere = home.path().join("elsewhere.json");
        fs::write(&elsewhere, "{}")?;
        symlink(&elsewhere, files.path(&Slot::Active))?;
        assert!(files.read(&Slot::Active).is_err());
        assert!(files.write(&Slot::Active, &login("x")).is_err());
        assert!(files.clear(&Slot::Active).is_err());
        assert_eq!(
            fs::read_to_string(&elsewhere)?,
            "{}",
            "the target is untouched"
        );
        Ok(())
    }

    #[test]
    fn credential_manager_follows_the_cached_flag_and_the_variable() -> TestResult {
        let dir = TempDir::new()?;
        let config = dir.path().join(".claude.json");
        assert!(
            !uses_credential_manager(&config, None),
            "no config: the file"
        );
        fs::write(
            &config,
            r#"{"cachedGrowthBookFeatures":{"tengu_windows_credman":false}}"#,
        )?;
        assert!(!uses_credential_manager(&config, None));
        fs::write(
            &config,
            r#"{"cachedGrowthBookFeatures":{"tengu_windows_credman":true}}"#,
        )?;
        assert!(uses_credential_manager(&config, None));
        fs::write(&config, "{}")?;
        assert!(uses_credential_manager(&config, Some(OsStr::new("1"))));
        assert!(!uses_credential_manager(&config, Some(OsStr::new("0"))));
        Ok(())
    }
}
