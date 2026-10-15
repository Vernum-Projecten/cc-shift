// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The account details Claude Code shows and switches on.
//!
//! Next to the Keychain token, Claude Code keeps the signed-in account in
//! `oauthAccount` of the config folder's `.claude.json`: `~/.claude.json`
//! for `~/.claude`, `<profile>/.claude.json` for a profile. `/login` writes
//! both, and open windows follow when both change (observed on Claude Code
//! 2.1.300, #24). A switch copies the profile's `oauthAccount` into
//! `~/.claude.json` and changes nothing else in that file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::error::Error;
use crate::store::profile::ProfilesRoot;

/// The `.claude.json` files of `~/.claude` and of every profile.
#[derive(Debug, Clone)]
pub struct AccountFiles {
    active: PathBuf,
    config: PathBuf,
    root: PathBuf,
}

impl AccountFiles {
    /// Returns the account files for `home` and its profiles root.
    #[must_use]
    pub fn new(home: &Path, root: &ProfilesRoot) -> Self {
        Self {
            active: home.join(".claude.json"),
            config: home.join(".claude"),
            root: root.path().to_path_buf(),
        }
    }

    /// Returns the config folder every window uses, `~/.claude`.
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        &self.config
    }

    /// Returns the `oauthAccount` of a profile, written by its `/login`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Account`] when the profile's `.claude.json` cannot
    /// be read or holds no `oauthAccount`.
    pub fn of_profile(&self, name: &str) -> Result<Value, Error> {
        let path = self.root.join(name).join(".claude.json");
        let file = read_json(&path)?.ok_or_else(|| missing(&path))?;
        file.get("oauthAccount")
            .filter(|account| account.is_object())
            .cloned()
            .ok_or_else(|| missing(&path))
    }

    /// Returns the account id (`oauthAccount.accountUuid`) `~/.claude.json`
    /// names, if any.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Account`] when the file cannot be read or parsed.
    pub fn active_id(&self) -> Result<Option<String>, Error> {
        Ok(read_json(&self.active)?.and_then(|file| file.get("oauthAccount").and_then(account_id)))
    }

    /// Returns the account id of profile `name`, if it is signed in.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Account`] when the profile's `.claude.json` cannot be
    /// read or holds no `oauthAccount`.
    pub fn profile_id(&self, name: &str) -> Result<Option<String>, Error> {
        Ok(account_id(&self.of_profile(name)?))
    }

    /// Makes `~/.claude.json` name the account of profile `name`.
    ///
    /// Only `oauthAccount` changes; every other key keeps its value and its
    /// place. The file is replaced in one rename and stays readable by the
    /// user alone.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Account`] when a file cannot be read, parsed or
    /// written.
    pub fn activate(&self, name: &str) -> Result<(), Error> {
        let account = self.of_profile(name)?;
        let mut file =
            read_json(&self.active)?.unwrap_or_else(|| Value::Object(serde_json::Map::new()));
        let Value::Object(map) = &mut file else {
            return Err(Error::Account(format!(
                "{} is not a JSON object",
                self.active.display()
            )));
        };
        if map.get("oauthAccount") == Some(&account) {
            return Ok(());
        }
        map.insert("oauthAccount".to_owned(), account);
        write_json(&self.active, &file)
    }
}

/// Returns the `accountUuid` of an `oauthAccount` object.
fn account_id(account: &Value) -> Option<String> {
    account
        .get("accountUuid")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Reads a JSON file; a missing file is `None`.
fn read_json(path: &Path) -> Result<Option<Value>, Error> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| Error::Account(format!("{} does not parse: {e}", path.display()))),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::Account(format!(
            "cannot read {}: {e}",
            path.display()
        ))),
    }
}

/// Replaces a JSON file in one rename, readable and writable by the user
/// alone, as Claude Code keeps it: mode 0600 on Unix, and on Windows the
/// access rules the file takes from the user's folder.
fn write_json(path: &Path, value: &Value) -> Result<(), Error> {
    let fail =
        |e: &dyn std::fmt::Display| Error::Account(format!("cannot write {}: {e}", path.display()));
    let text = serde_json::to_vec_pretty(value).map_err(|e| fail(&e))?;
    let temporary = path.with_extension("json.cc-shift-tmp");
    fs::write(&temporary, text).map_err(|e| fail(&e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600)).map_err(|e| fail(&e))?;
    }
    fs::rename(&temporary, path).map_err(|e| fail(&e))
}

fn missing(path: &Path) -> Error {
    Error::Account(format!(
        "{} has no signed-in account; run /login in that profile",
        path.display()
    ))
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::AccountFiles;
    use crate::error::Error;
    use crate::store::profile::ProfilesRoot;
    use std::fs;
    use tempfile::TempDir;

    fn files(home: &TempDir) -> Result<AccountFiles, Error> {
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        Ok(AccountFiles::new(home.path(), &root))
    }

    fn profile(home: &TempDir, name: &str, email: &str) -> std::io::Result<()> {
        let dir = home.path().join(".claude-profiles").join(name);
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join(".claude.json"),
            format!(r#"{{"numStartups": 3, "oauthAccount": {{"emailAddress": "{email}"}}}}"#),
        )
    }

    #[test]
    fn copies_only_the_account() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        profile(&home, "b", "b@example.org")?;
        fs::write(
            home.path().join(".claude.json"),
            r#"{"zeta": 1, "oauthAccount": {"emailAddress": "a@example.org"}, "alpha": 2}"#,
        )?;
        files(&home)?.activate("b")?;
        let text = fs::read_to_string(home.path().join(".claude.json"))?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        assert_eq!(value["oauthAccount"]["emailAddress"], "b@example.org");
        assert_eq!(
            (value["zeta"].clone(), value["alpha"].clone()),
            (1.into(), 2.into())
        );
        let zeta = text.find("zeta").ok_or("zeta")?;
        let alpha = text.find("alpha").ok_or("alpha")?;
        assert!(zeta < alpha, "key order kept: {text}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(home.path().join(".claude.json"))?
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "user only");
        }
        Ok(())
    }

    #[test]
    fn creates_the_file_when_missing() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        profile(&home, "b", "b@example.org")?;
        files(&home)?.activate("b")?;
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(home.path().join(".claude.json"))?)?;
        assert_eq!(value["oauthAccount"]["emailAddress"], "b@example.org");
        Ok(())
    }

    #[test]
    fn a_profile_without_a_login_is_refused() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        fs::create_dir_all(home.path().join(".claude-profiles/b"))?;
        assert!(matches!(
            files(&home)?.activate("b"),
            Err(Error::Account(_))
        ));
        assert!(
            !home.path().join(".claude.json").exists(),
            "nothing written"
        );
        Ok(())
    }
}
