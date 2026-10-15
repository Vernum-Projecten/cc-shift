//! `cc-shift use <name>`: makes an account the active account of `~/.claude`.

use std::ffi::OsStr;

use crate::command::{home, report};
use crate::error::Error;
use crate::login::account::AccountFiles;
use crate::login::keychain::SystemKeychain;
use crate::store::profile::{ProfileName, ProfilesRoot};
use crate::store::state::StateFile;
use crate::switching::transfer::{self, Outcome};

/// Moves an account's parked login into `~/.claude`, so every window
/// continues on it, and says what moved.
///
/// # Errors
///
/// Returns the error of an invalid name, a missing `HOME`, or a switch that
/// could not run.
pub fn run(raw: &OsStr) -> Result<(), Error> {
    let name = ProfileName::parse(raw)?;
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let logins = SystemKeychain::for_user(&root)?;
    let accounts = AccountFiles::new(&home, &root);
    let file = StateFile::in_root(&root);
    match transfer::switch_to(&logins, &accounts, &file, name.as_str())? {
        Outcome::AlreadyActive => {
            report(&format!("'{}' is already active", name.as_str()));
        }
        Outcome::Switched { from: Some(from) } => report(&format!(
            "'{}' is active; '{from}' is parked",
            name.as_str()
        )),
        Outcome::Switched { from: None } => report(&format!(
            "'{}' is active; the login that was in ~/.claude belonged to no profile and was replaced",
            name.as_str()
        )),
    }
    Ok(())
}
