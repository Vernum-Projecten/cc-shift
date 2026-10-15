// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Where logins live: the slot of `~/.claude` and one parked slot per profile.
//!
//! On macOS a slot is a Keychain item, on Linux and Windows a
//! `.credentials.json` file.
//! Every access goes through [`Logins`], so the switch logic is tested
//! against an in-memory store and never touches a real login.

pub mod account;
#[cfg(any(target_os = "linux", windows))]
pub mod files;
#[cfg(target_os = "macos")]
pub mod macos;

use std::fmt;

use crate::error::Error;

/// The login store of this system: the Keychain on macOS.
#[cfg(target_os = "macos")]
pub type SystemLogins = macos::keychain::SystemKeychain;

/// The login store of this system: the login files on Linux and Windows.
#[cfg(any(target_os = "linux", windows))]
pub type SystemLogins = files::CredentialFiles;

/// One slot that can hold a login.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The slot of `~/.claude`, which every window uses.
    Active,
    /// The parked slot of one profile.
    Parked(String),
}

impl fmt::Display for Slot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => f.write_str("the login of ~/.claude"),
            Self::Parked(name) => write!(f, "the parked login of '{name}'"),
        }
    }
}

/// A login as Claude Code stores it, kept opaque.
///
/// Its `Debug` output never shows the content, and it has no `Display` or
/// serialization, so it cannot reach a log, the terminal or the state file.
#[derive(Clone, PartialEq, Eq)]
pub struct Login(Vec<u8>);

impl Login {
    /// Wraps the stored bytes of a login.
    #[must_use]
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Returns the stored bytes, for writing them back to a slot.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Login {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Login(<redacted>)")
    }
}

/// Reads, writes and clears logins in their slots.
pub trait Logins {
    /// Returns the login in `slot`, or `None` when the slot is empty.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LoginStore`] when the slot cannot be read.
    fn read(&self, slot: &Slot) -> Result<Option<Login>, Error>;

    /// Puts `login` in `slot`, replacing what was there.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LoginStore`] when the slot cannot be written.
    fn write(&self, slot: &Slot, login: &Login) -> Result<(), Error>;

    /// Empties `slot`; an empty slot stays empty.
    ///
    /// # Errors
    ///
    /// Returns [`Error::LoginStore`] when the slot cannot be cleared.
    fn clear(&self, slot: &Slot) -> Result<(), Error>;
}

#[cfg(test)]
mod tests {
    use super::{Login, Slot};

    #[test]
    fn debug_output_never_shows_the_login() {
        let login = Login::from_bytes(b"secret-token".to_vec());
        assert_eq!(format!("{login:?}"), "Login(<redacted>)");
    }

    #[test]
    fn slots_name_themselves_without_content() {
        assert_eq!(
            Slot::Parked("member1".to_owned()).to_string(),
            "the parked login of 'member1'"
        );
    }
}
