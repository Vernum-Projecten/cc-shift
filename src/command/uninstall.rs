// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift uninstall`: undoes `cc-shift setup`, and with `--all` also
//! removes every profile.

use std::fs;
use std::io::BufRead;

use crate::claude_code::settings;
use crate::command::{home, report};
use crate::early;
use crate::error::Error;
use crate::login::SystemLogins;
use crate::login::{Logins, Slot};
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;

/// Undoes `cc-shift setup`, and with `all` removes every profile.
///
/// The status line and the hook leave `~/.claude/settings.json` and the
/// user's own status line comes back. With `all`, every profile folder and
/// its Keychain item are deleted after the user types `yes`.
///
/// The login in `~/.claude` stays, so Claude Code keeps working with the
/// account that was active.
///
/// # Errors
///
/// Returns the error of a missing `HOME`, settings that cannot be read or
/// written, or a profile or Keychain item that cannot be removed.
pub fn run(all: bool) -> Result<(), Error> {
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let file = StateFile::in_root(&root);
    let previous = if root.path().is_dir() {
        file.read()?.chained_statusline
    } else {
        None
    };
    let path = settings::settings_path(&home);
    let (cleaned, changed) =
        settings::uninstall(settings::read_settings(&path)?, previous.as_deref())?;
    if changed {
        settings::write_settings(&path, &cleaned)?;
        report(&format!(
            "removed the status line and the rate-limit hook from {}",
            path.display()
        ));
    } else {
        report("the status line and the hook were not installed");
    }
    if early::remove(&home)? {
        report("removed the scheduled early start job an older version installed");
    }
    if !all {
        report(
            "the profiles and their logins are kept; 'cc-shift uninstall --all' removes them too",
        );
        return Ok(());
    }
    let profiles = if root.path().is_dir() {
        root.list()?
    } else {
        Vec::new()
    };
    if profiles.is_empty() {
        report("there are no profiles to remove");
        return Ok(());
    }
    report(&format!(
        "this deletes {} profile(s) and their stored logins: {}. Type yes to go on:",
        profiles.len(),
        profiles.join(", ")
    ));
    let mut answer = String::new();
    let _read: std::io::Result<usize> = std::io::stdin().lock().read_line(&mut answer);
    if answer.trim() != "yes" {
        report("nothing removed");
        return Ok(());
    }
    let logins = SystemLogins::for_user(&root)?;
    for name in &profiles {
        logins.clear(&Slot::Parked(name.clone()))?;
        let dir = root.path().join(name);
        fs::remove_dir_all(&dir).map_err(|source| Error::State {
            path: dir.clone(),
            source,
        })?;
    }
    for leftover in ["state.json", ".lock", ".refresh-lock"] {
        // NOTE: no specification governs this: our own design. A file that
        // was never written is already gone.
        let _gone: std::io::Result<()> = fs::remove_file(root.path().join(leftover));
    }
    let _empty: std::io::Result<()> = fs::remove_dir(root.path());
    report(&format!(
        "removed {} profile(s); the login in ~/.claude stays, so Claude Code keeps working",
        profiles.len()
    ));
    Ok(())
}
