// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift refresh`: refreshes parked logins, reads the accounts' usage, and
//! switches when the active account turns out to be full.

use crate::anthropic::http::Curl;
use crate::anthropic::parked::{self, Report};
use crate::command::{automatic, report, unix_now};
use crate::error::Error;
use crate::login::SystemLogins;
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::switching::auto::Trigger;

/// Runs a refresh against the real Keychain and network and says what
/// happened per account.
///
/// # Errors
///
/// Returns the error of the profiles root or the state file.
pub fn run() -> Result<(), Error> {
    let (reports, switched) = refresh_and_switch()?;
    for item in reports {
        match item.usage {
            Ok(_) if item.refreshed => {
                report(&format!("{}: login refreshed, usage read", item.name));
            }
            Ok(true) => report(&format!("{}: usage read", item.name)),
            Ok(false) => report(&format!("{}: up to date", item.name)),
            Err(error) => report(&format!("{}: {error}", item.name)),
        }
    }
    if let Some(name) = switched {
        report(&format!("switched to {name}"));
    }
    Ok(())
}

/// Refreshes and reads usage as [`refresh_parked`] does, then switches when
/// the active account's usage was read and is at or past a threshold.
///
/// Returns the reports and the account switched to, if any. The status line
/// switches only after the main session's own replies, so this switch is
/// the one that runs while the main session waits on background subagents.
///
/// # Errors
///
/// Returns the error of [`refresh_parked`] or of the state file; an error of
/// the switch itself leaves the active account as it was and is not
/// returned.
pub fn refresh_and_switch() -> Result<(Vec<Report>, Option<String>), Error> {
    let reports = refresh_parked()?;
    let root = ProfilesRoot::from_env()?;
    let active = StateFile::in_root(&root).read()?.active;
    let switched = if active_was_read(&reports, active.as_deref()) {
        // NOTE: no specification governs this: our own design. A switch that
        // fails is tried again by the next refresh, the status line or the hook.
        automatic(Trigger::Usage).ok().flatten()
    } else {
        None
    };
    Ok((reports, switched))
}

/// Returns whether the reports hold a new usage reading of `active`.
fn active_was_read(reports: &[Report], active: Option<&str>) -> bool {
    reports
        .iter()
        .any(|r| Some(r.name.as_str()) == active && matches!(r.usage, Ok(true)))
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
    let logins = SystemLogins::for_user(&root)?;
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
        let _child: std::io::Result<std::process::Child> =
            crate::claude_code::launch::without_window(&mut std::process::Command::new(binary))
                .arg("refresh")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
    }
}

#[cfg(test)]
mod tests {
    use super::active_was_read;
    use crate::anthropic::parked::Report;
    use crate::error::Error;

    fn report(name: &str, usage: Result<bool, Error>) -> Report {
        Report {
            name: name.to_owned(),
            refreshed: false,
            usage,
        }
    }

    #[test]
    fn a_new_reading_of_the_active_account_counts() {
        let reports = [report("a", Ok(true)), report("b", Ok(false))];
        assert!(active_was_read(&reports, Some("a")));
    }

    #[test]
    fn a_reading_of_a_parked_account_does_not_count() {
        let reports = [report("a", Ok(false)), report("b", Ok(true))];
        assert!(!active_was_read(&reports, Some("a")));
    }

    #[test]
    fn a_failed_reading_of_the_active_account_does_not_count() {
        let reports = [report("a", Err(Error::Http("status 429".to_owned())))];
        assert!(!active_was_read(&reports, Some("a")));
        assert!(!active_was_read(&[report("a", Ok(true))], None));
    }
}
