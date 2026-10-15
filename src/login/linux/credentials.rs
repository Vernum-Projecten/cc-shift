// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The Linux login files: `.credentials.json` in each config folder.
//!
//! On Linux, Claude Code keeps a config folder's login in
//! `<folder>/.credentials.json` with mode 0600 and uses no keyring
//! (<https://code.claude.com/docs/en/authentication#credential-management>).
//! A running window checks the file's modification time before each request
//! and drops its cached login when it changed (observed in the 2.1.300 Linux
//! build, recorded on #85), so a login is written to a fresh temporary file
//! and renamed into place: the window then sees a new file and follows.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::login::{Login, Logins, Slot};
use crate::store::profile::ProfilesRoot;

/// The name of the login file in a config folder.
pub const FILE_NAME: &str = ".credentials.json";

/// The mode of a login file: readable and writable by its owner only.
const MODE: u32 = 0o600;

/// The login files of `~/.claude` and of every profile.
#[derive(Debug, Clone)]
pub struct CredentialFiles {
    active: PathBuf,
    root: PathBuf,
}

impl CredentialFiles {
    /// Returns the login files of the user named by `$HOME`, for the
    /// profiles in `root`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoHome`] when `$HOME` is unset or empty.
    pub fn for_user(root: &ProfilesRoot) -> Result<Self, Error> {
        let home = std::env::var_os("HOME")
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
            active: active.join(FILE_NAME),
            root: root.to_path_buf(),
        }
    }

    /// Returns the path of a slot's file.
    #[must_use]
    pub fn path(&self, slot: &Slot) -> PathBuf {
        match slot {
            Slot::Active => self.active.clone(),
            Slot::Parked(name) => self.root.join(name).join(FILE_NAME),
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
}

impl Logins for CredentialFiles {
    fn read(&self, slot: &Slot) -> Result<Option<Login>, Error> {
        let path = self.path(slot);
        Self::refuse_link(slot, &path)?;
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(Login::from_bytes(bytes))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Self::fail(slot, &path, e)),
        }
    }

    fn write(&self, slot: &Slot, login: &Login) -> Result<(), Error> {
        let path = self.path(slot);
        Self::refuse_link(slot, &path)?;
        let folder = path
            .parent()
            .ok_or_else(|| Self::fail(slot, &path, "has no folder"))?;
        let temporary = folder.join(format!("{FILE_NAME}.cc-shift-{}", std::process::id()));
        // NOTE: no specification governs this: our own design. A file with this
        // name is left from an earlier run with the same process id.
        let _stale: io::Result<()> = fs::remove_file(&temporary);
        let written = write_new(&temporary, login.as_bytes())
            .and_then(|()| fs::rename(&temporary, &path))
            .and_then(|()| File::open(folder)?.sync_all());
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
        Self::refuse_link(slot, &path)?;
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Self::fail(slot, &path, e)),
        }
    }
}

/// Creates `path` with mode 0600, writes `bytes` and flushes them to disk;
/// fails when `path` already exists.
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(MODE)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{CredentialFiles, FILE_NAME};
    use crate::login::{Login, Logins, Slot};
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
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
        let mode = fs::metadata(files.path(&slot))?.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let leftovers: Vec<_> = fs::read_dir(home.path().join(".claude-profiles/work"))?
            .filter_map(Result::ok)
            .map(|e| e.file_name())
            .collect();
        assert_eq!(leftovers, [FILE_NAME], "no temporary file is left");
        Ok(())
    }

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

    #[test]
    fn a_linked_file_is_refused() -> TestResult {
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
}
