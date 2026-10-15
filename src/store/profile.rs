// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Profile names and the profiles root, `~/.claude-profiles/`.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::Error;

/// The words `cc-shift` uses as commands, which can never name a profile.
pub const RESERVED: [&str; 17] = [
    "add",
    "setup",
    "status",
    "use",
    "list",
    "statusline",
    "hook",
    "help",
    "remove",
    "refresh",
    "doctor",
    "uninstall",
    "completion",
    "log",
    "version",
    "stats",
    "early",
];

/// The name of the profiles root inside the home folder.
const ROOT_NAME: &str = ".claude-profiles";

/// Returns whether Windows reserves `name` for a device: `con`, `prn`, `aux`,
/// `nul`, `com1` to `com9` or `lpt1` to `lpt9`, in any case.
///
/// A folder cannot carry such a name on Windows
/// (<https://learn.microsoft.com/windows/win32/fileio/naming-a-file>). The
/// superscript forms that page also lists are not ASCII, so no profile name
/// can hold them.
#[must_use]
pub fn is_windows_device_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if ["con", "prn", "aux", "nul"].contains(&lower.as_str()) {
        return true;
    }
    let number = lower
        .strip_prefix("com")
        .or_else(|| lower.strip_prefix("lpt"));
    matches!(number.map(str::as_bytes), Some([b'1'..=b'9']))
}

/// A validated profile name.
///
/// A name uses only ASCII letters, digits, `-` and `_`, starts with a letter
/// or digit, and is not one of the [`RESERVED`] command words. On Windows it
/// is also not a device name ([`is_windows_device_name`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileName(String);

impl ProfileName {
    /// Validates a profile name from the command line.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] for an empty name, a name that is not
    /// UTF-8, a name with another character or a leading `-` or `_`,
    /// [`Error::ReservedName`] for a command word, and, on Windows,
    /// [`Error::DeviceName`] for a device name.
    pub fn parse(raw: &OsStr) -> Result<Self, Error> {
        let Some(name) = raw.to_str() else {
            return Err(Error::InvalidName(raw.to_string_lossy().into_owned()));
        };
        let starts_well = name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric());
        let allowed = name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !starts_well || !allowed {
            return Err(Error::InvalidName(name.to_owned()));
        }
        if RESERVED.contains(&name) {
            return Err(Error::ReservedName(name.to_owned()));
        }
        if cfg!(windows) && is_windows_device_name(name) {
            return Err(Error::DeviceName(name.to_owned()));
        }
        Ok(Self(name.to_owned()))
    }

    /// Returns the name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Returns the home folder from the environment.
///
/// That is `HOME` on macOS and Linux, and `USERPROFILE` on Windows, where
/// Claude Code keeps `.claude` and `.claude.json`
/// (<https://code.claude.com/docs/en/authentication#credential-management>).
#[must_use]
pub fn home_var() -> Option<OsString> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
}

/// The folder that holds one subfolder per profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilesRoot(PathBuf);

impl ProfilesRoot {
    /// Finds the profiles root inside the given home folder.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoHome`] when `home` is missing or empty.
    pub fn from_home(home: Option<OsString>) -> Result<Self, Error> {
        match home {
            Some(home) if !home.is_empty() => Ok(Self(PathBuf::from(home).join(ROOT_NAME))),
            _ => Err(Error::NoHome),
        }
    }

    /// Finds the profiles root inside the home folder, see [`home_var`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoHome`] when the home folder is unset or empty.
    pub fn from_env() -> Result<Self, Error> {
        Self::from_home(home_var())
    }

    /// Returns the path of the profiles root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Returns the config folder of one profile.
    #[must_use]
    pub fn profile_dir(&self, name: &ProfileName) -> PathBuf {
        self.0.join(name.as_str())
    }

    /// Returns the names of all profiles, sorted.
    ///
    /// Every folder directly inside the root is a profile; files are
    /// skipped. A missing root means there are no profiles.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ReadRoot`] when the root exists but cannot be read.
    pub fn list(&self) -> Result<Vec<String>, Error> {
        let read_error = |source| Error::ReadRoot {
            path: self.0.clone(),
            source,
        };
        let entries = match fs::read_dir(&self.0) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(read_error(error)),
        };
        let mut names = Vec::new();
        for entry in entries {
            let entry = entry.map_err(read_error)?;
            if entry.path().is_dir() {
                names.push(entry.file_name().to_string_lossy().into_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Checks that `name` is not an existing profile's name in another case,
    /// unless a profile of exactly that name exists too.
    ///
    /// Windows treats names that differ only in case as the same folder
    /// (<https://learn.microsoft.com/windows/win32/fileio/naming-a-file>:
    /// "Do not assume case sensitivity"), so `cc-shift` refuses such a name on
    /// every system, and one profile keeps one spelling wherever it is used.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NameCase`] when a profile differs from `name` only in
    /// case, and [`Error::ReadRoot`] when the root cannot be read.
    pub fn check_case(&self, name: &ProfileName) -> Result<(), Error> {
        // NOTE: no specification governs this: our own design; a root that is
        // not a folder holds no profiles, and `create` then reports it.
        if !self.0.is_dir() {
            return Ok(());
        }
        let names = self.list()?;
        if names.iter().any(|existing| existing == name.as_str()) {
            return Ok(());
        }
        let twin = names
            .into_iter()
            .find(|existing| existing.eq_ignore_ascii_case(name.as_str()));
        match twin {
            Some(existing) => Err(Error::NameCase {
                name: name.as_str().to_owned(),
                existing,
            }),
            None => Ok(()),
        }
    }

    /// Creates a profile's config folder, and the root, when missing.
    ///
    /// # Errors
    ///
    /// Returns [`Error::CreateFolder`] when a folder cannot be created.
    pub fn create(&self, name: &ProfileName) -> Result<PathBuf, Error> {
        let path = self.profile_dir(name);
        fs::create_dir_all(&path).map_err(|source| Error::CreateFolder {
            path: path.clone(),
            source,
        })?;
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::{ProfileName, ProfilesRoot, RESERVED, is_windows_device_name};
    use crate::error::Error;
    use std::ffi::OsStr;
    use std::path::Path;
    use tempfile::TempDir;

    fn parse(raw: &str) -> Result<ProfileName, Error> {
        ProfileName::parse(OsStr::new(raw))
    }

    #[test]
    fn accepts_letters_digits_dash_underscore() {
        for name in ["primary", "member1", "a-b", "a_b", "7"] {
            assert!(parse(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn refuses_empty_name() {
        assert!(matches!(parse(""), Err(Error::InvalidName(_))));
    }

    #[test]
    fn refuses_leading_dash_or_underscore() {
        for name in ["-x", "_x"] {
            assert!(matches!(parse(name), Err(Error::InvalidName(_))), "{name}");
        }
    }

    #[test]
    fn refuses_other_characters() {
        for name in ["a/b", "a b", "..", "a.b", "é"] {
            assert!(matches!(parse(name), Err(Error::InvalidName(_))), "{name}");
        }
    }

    #[test]
    fn refuses_every_command_word() {
        for name in RESERVED {
            assert!(matches!(parse(name), Err(Error::ReservedName(_))), "{name}");
        }
    }

    #[test]
    fn device_names_are_known_in_any_case() {
        for name in [
            "con", "CON", "Prn", "aux", "nUl", "com1", "COM9", "lpt1", "Lpt9",
        ] {
            assert!(is_windows_device_name(name), "{name}");
        }
    }

    #[test]
    fn other_names_are_no_device_names() {
        for name in [
            "console", "com", "com0", "com10", "lpt0", "lpt", "comx", "nul1", "work", "c0n",
        ] {
            assert!(!is_windows_device_name(name), "{name}");
        }
    }

    #[cfg(windows)]
    #[test]
    fn refuses_a_device_name_on_windows() {
        for name in ["con", "CON", "Aux", "com1", "LPT9"] {
            assert!(matches!(parse(name), Err(Error::DeviceName(_))), "{name}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn accepts_a_device_name_elsewhere() {
        for name in ["con", "aux", "com1"] {
            assert!(parse(name).is_ok(), "{name}");
        }
    }

    #[test]
    #[expect(clippy::panic_in_result_fn, reason = "test assertions")]
    fn refuses_a_name_that_differs_only_in_case() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        root.create(&parse("work")?)?;
        let checked = root.check_case(&parse("Work")?);
        assert!(
            matches!(&checked, Err(Error::NameCase { name, existing }) if name == "Work" && existing == "work"),
            "{checked:?}"
        );
        Ok(())
    }

    #[test]
    #[expect(clippy::panic_in_result_fn, reason = "test assertions")]
    fn accepts_the_same_name_and_a_new_name() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        root.create(&parse("work")?)?;
        assert!(root.check_case(&parse("work")?).is_ok(), "same name");
        assert!(root.check_case(&parse("home")?).is_ok(), "new name");
        Ok(())
    }

    #[test]
    #[expect(clippy::panic_in_result_fn, reason = "test assertions")]
    fn accepts_any_name_without_a_root() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        assert!(root.check_case(&parse("Work")?).is_ok(), "no profiles");
        Ok(())
    }

    #[test]
    fn root_needs_a_home() {
        assert!(matches!(ProfilesRoot::from_home(None), Err(Error::NoHome)));
        assert!(matches!(
            ProfilesRoot::from_home(Some("".into())),
            Err(Error::NoHome)
        ));
    }

    #[test]
    #[expect(clippy::panic_in_result_fn, reason = "test assertions")]
    fn root_is_inside_home() -> Result<(), Error> {
        let root = ProfilesRoot::from_home(Some("/Users/someone".into()))?;
        assert_eq!(root.path(), Path::new("/Users/someone/.claude-profiles"));
        Ok(())
    }
}
