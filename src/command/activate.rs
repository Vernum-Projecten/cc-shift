// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift use <name>`: makes an account the active account of `~/.claude`.

use std::ffi::OsStr;

use crate::command::{home, report, unix_now};
use crate::error::Error;
use crate::login::SystemLogins;
use crate::login::account::AccountFiles;
use crate::store::history::{self, Event, Reason};
use crate::store::profile::{ProfileName, ProfilesRoot};
use crate::store::state::StateFile;
use crate::switching::transfer::{self, Outcome};

/// When open windows pick up a switch: Claude Code caches the Keychain item
/// for about 30 seconds on macOS, and checks the login file before each
/// request on Linux and Windows.
#[cfg(target_os = "macos")]
const FOLLOW: &str = "within about 30 seconds";
#[cfg(any(target_os = "linux", windows))]
const FOLLOW: &str = "with their next message";

/// Moves an account's parked login into `~/.claude` and says what moved.
///
/// Running Claude Code windows re-read their login before each request, so
/// they follow: within about 30 seconds on macOS, where Claude Code caches
/// the Keychain item, and on their next request on Linux.
///
/// # Errors
///
/// Returns the error of an invalid name, a missing `HOME`, or a switch that
/// could not run.
pub fn run(raw: &OsStr) -> Result<(), Error> {
    let name = ProfileName::parse(raw)?;
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let logins = SystemLogins::for_user(&root)?;
    let accounts = AccountFiles::new(&home, &root);
    let file = StateFile::in_root(&root);
    let outcome = transfer::switch_to(&logins, &accounts, &file, name.as_str())?;
    if let Outcome::Switched { from } = &outcome {
        history::note(
            &root,
            Event::Switch {
                reason: Reason::ByHand,
                window: None,
                from: from.clone(),
                to: name.as_str().to_owned(),
                from_used: None,
                to_used: None,
            },
            unix_now(),
        );
    }
    match outcome {
        Outcome::AlreadyActive => {
            report(&format!("'{}' is already active", name.as_str()));
        }
        Outcome::Switched { from: Some(from) } => report(&format!(
            "'{}' is active; '{from}' is parked. Open Claude Code windows follow {FOLLOW}",
            name.as_str()
        )),
        Outcome::Switched { from: None } => report(&format!(
            "'{}' is active; the login that was in ~/.claude belonged to no profile and was replaced. Open Claude Code windows follow {FOLLOW}",
            name.as_str()
        )),
    }
    Ok(())
}
