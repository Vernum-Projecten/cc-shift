//! The macOS login keychain, reached through `/usr/bin/security`.
//!
//! Claude Code stores each config folder's login as a generic password whose
//! service is `Claude Code-credentials`, followed for a folder other than
//! `~/.claude` by `-` and the first 8 hex characters of the SHA-256 of the
//! folder's absolute path, with the macOS user name as the account
//! (observed on Claude Code 2.1.300, recorded on #6). Going through the same
//! `security` tool keeps each item's access rules as they are, and a login
//! is passed on stdin as hex, never as a command-line argument.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use sha2::{Digest, Sha256};

use crate::error::Error;
use crate::login::{Login, Logins, Slot};
use crate::store::profile::ProfilesRoot;

/// The service of the item of `~/.claude`.
const ACTIVE_SERVICE: &str = "Claude Code-credentials";

/// The exit code of `security` for an item that does not exist.
const NOT_FOUND: i32 = 44;

/// The login keychain of the current macOS user.
#[derive(Debug, Clone)]
pub struct SystemKeychain {
    account: String,
    root: PathBuf,
}

impl SystemKeychain {
    /// Returns the keychain of the user named by `$USER`, for the profiles
    /// in `root`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Keychain`] when `$USER` is unset or empty.
    pub fn for_user(root: &ProfilesRoot) -> Result<Self, Error> {
        let account = std::env::var("USER")
            .ok()
            .filter(|user| !user.is_empty())
            .ok_or_else(|| Error::Keychain {
                slot: "the login keychain".to_owned(),
                reason: "USER is not set".to_owned(),
            })?;
        Ok(Self {
            account,
            root: root.path().to_path_buf(),
        })
    }

    /// Returns the service name of a slot's item.
    #[must_use]
    pub fn service(&self, slot: &Slot) -> String {
        match slot {
            Slot::Active => ACTIVE_SERVICE.to_owned(),
            Slot::Parked(name) => service_of(&self.root.join(name).to_string_lossy()),
        }
    }

    fn fail(slot: &Slot, reason: impl Into<String>) -> Error {
        Error::Keychain {
            slot: slot.to_string(),
            reason: reason.into(),
        }
    }

    fn run(slot: &Slot, args: &[&str]) -> Result<Output, Error> {
        Command::new("/usr/bin/security")
            .args(args)
            .output()
            .map_err(|e| Self::fail(slot, format!("cannot run security: {e}")))
    }
}

/// Returns the service name of the item of a config folder at `path`.
#[must_use]
pub fn service_of(path: &str) -> String {
    let digest = Sha256::digest(path.as_bytes());
    format!(
        "{ACTIVE_SERVICE}-{}",
        hex(digest.get(..4).unwrap_or_default())
    )
}

/// Returns the `security -i` line that writes `login` to an item, with the
/// login as hex so no quoting can break it.
#[must_use]
pub fn add_command(account: &str, service: &str, login: &Login) -> String {
    format!(
        "add-generic-password -U -a \"{account}\" -s \"{service}\" -X {}\n",
        hex(login.as_bytes())
    )
}

/// Returns bytes as lowercase hex.
fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, byte| {
        // NOTE: no specification governs this: our own design. Writing to a
        // `String` cannot fail.
        let _infallible: std::fmt::Result = write!(out, "{byte:02x}");
        out
    })
}

impl Logins for SystemKeychain {
    fn read(&self, slot: &Slot) -> Result<Option<Login>, Error> {
        let service = self.service(slot);
        let out = Self::run(
            slot,
            &[
                "find-generic-password",
                "-a",
                &self.account,
                "-s",
                &service,
                "-w",
            ],
        )?;
        match out.status.code() {
            Some(0) => {
                let mut bytes = out.stdout;
                if bytes.last() == Some(&b'\n') {
                    bytes.pop();
                }
                Ok(Some(Login::from_bytes(bytes)))
            }
            Some(NOT_FOUND) => Ok(None),
            _ => Err(Self::fail(
                slot,
                String::from_utf8_lossy(&out.stderr).trim(),
            )),
        }
    }

    fn write(&self, slot: &Slot, login: &Login) -> Result<(), Error> {
        let service = self.service(slot);
        let mut child = Command::new("/usr/bin/security")
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| Self::fail(slot, format!("cannot run security: {e}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(add_command(&self.account, &service, login).as_bytes())
                .map_err(|e| Self::fail(slot, format!("cannot write to security: {e}")))?;
        }
        let out = child
            .wait_with_output()
            .map_err(|e| Self::fail(slot, format!("security did not finish: {e}")))?;
        // NOTE: no specification governs this: our own design. `security -i`
        // can exit 0 after a failed command, so the write is read back.
        if self.read(slot)?.as_ref() == Some(login) {
            Ok(())
        } else {
            Err(Self::fail(
                slot,
                format!(
                    "the login did not reach the keychain: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                ),
            ))
        }
    }

    fn clear(&self, slot: &Slot) -> Result<(), Error> {
        let service = self.service(slot);
        let out = Self::run(
            slot,
            &[
                "delete-generic-password",
                "-a",
                &self.account,
                "-s",
                &service,
            ],
        )?;
        match out.status.code() {
            Some(0 | NOT_FOUND) => Ok(()),
            _ => Err(Self::fail(
                slot,
                String::from_utf8_lossy(&out.stderr).trim(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{add_command, service_of};
    use crate::login::Login;

    #[test]
    fn a_folder_service_is_the_short_sha256_of_its_path() {
        assert_eq!(
            service_of("/Users/someone/.claude-profiles/claude1"),
            "Claude Code-credentials-93f6eaf7"
        );
    }

    #[test]
    fn a_write_sends_the_login_as_hex() {
        let login = Login::from_bytes(br#"{"a":"b c"}"#.to_vec());
        assert_eq!(
            add_command("someone", "Claude Code-credentials", &login),
            "add-generic-password -U -a \"someone\" -s \"Claude Code-credentials\" -X 7b2261223a22622063227d\n"
        );
    }
}
