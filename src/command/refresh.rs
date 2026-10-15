//! `cc-shift refresh`: refreshes parked logins and reads parked accounts' usage.

use crate::anthropic::http::Curl;
use crate::anthropic::parked::{self, Report};
use crate::command::{report, unix_now};
use crate::error::Error;
use crate::login::keychain::SystemKeychain;
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;

/// Runs a refresh against the real Keychain and network and says what
/// happened per account.
///
/// # Errors
///
/// Returns the error of the profiles root or the state file.
pub fn run() -> Result<(), Error> {
    for item in refresh_parked()? {
        match item.usage {
            Ok(_) if item.refreshed => {
                report(&format!("{}: login refreshed, usage read", item.name));
            }
            Ok(true) => report(&format!("{}: usage read", item.name)),
            Ok(false) => report(&format!("{}: up to date", item.name)),
            Err(error) => report(&format!("{}: {error}", item.name)),
        }
    }
    Ok(())
}

/// Refreshes the parked logins and reads the parked accounts' usage against
/// the real Keychain and network.
///
/// # Errors
///
/// Returns the error of a missing `HOME`, the Keychain, the profiles root
/// or the state file.
pub fn refresh_parked() -> Result<Vec<Report>, Error> {
    let root = ProfilesRoot::from_env()?;
    let logins = SystemKeychain::for_user(&root)?;
    parked::refresh_all(
        &logins,
        &Curl,
        &StateFile::in_root(&root),
        &root,
        unix_now(),
    )
}

/// Starts `cc-shift refresh` as a detached process; a run that is already going
/// makes the new one return at once.
pub fn start_in_background() {
    if let Ok(binary) = std::env::current_exe() {
        let _child: std::io::Result<std::process::Child> = std::process::Command::new(binary)
            .arg("refresh")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}
