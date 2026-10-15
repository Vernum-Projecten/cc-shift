//! Profile names and the profiles root, `~/.claude-profiles/`.

use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::error::Error;

/// The words `cc-shift` uses as commands, which can never name a profile.
pub const RESERVED: [&str; 13] = [
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
];

/// The name of the profiles root inside the home folder.
const ROOT_NAME: &str = ".claude-profiles";

/// A validated profile name.
///
/// A name uses only ASCII letters, digits, `-` and `_`, starts with a letter
/// or digit, and is not one of the [`RESERVED`] command words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileName(String);

impl ProfileName {
    /// Validates a profile name from the command line.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidName`] for an empty name, a name that is not
    /// UTF-8, a name with another character or a leading `-` or `_`, and
    /// [`Error::ReservedName`] for a command word.
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
        Ok(Self(name.to_owned()))
    }

    /// Returns the name as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
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

    /// Finds the profiles root inside `$HOME`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoHome`] when `HOME` is unset or empty.
    pub fn from_env() -> Result<Self, Error> {
        Self::from_home(std::env::var_os("HOME"))
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
    use super::{ProfileName, ProfilesRoot, RESERVED};
    use crate::error::Error;
    use std::ffi::OsStr;
    use std::path::Path;

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
