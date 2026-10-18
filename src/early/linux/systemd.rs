// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The systemd user timer that older versions installed to run the early
//! start.
//!
//! A user unit lives in `$XDG_CONFIG_HOME/systemd/user` (`~/.config` by
//! default)
//! (<https://www.freedesktop.org/software/systemd/man/latest/systemd.unit.html>).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::Error;

/// The name of the units, without their suffix.
pub const UNIT: &str = "cc-shift-early-start";

/// Returns the folder of the user's own units under `home`.
#[must_use]
pub fn units_folder(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map_or_else(|| home.join(".config"), PathBuf::from)
        .join("systemd/user")
}

/// Returns the path of the timer unit under `home`.
#[must_use]
pub fn timer_path(home: &Path) -> PathBuf {
    units_folder(home).join(format!("{UNIT}.timer"))
}

/// Stops and disables the timer and deletes both units; does nothing when
/// there is no timer. Returns whether one was removed.
///
/// # Errors
///
/// Returns [`Error::Settings`] when a unit cannot be deleted.
pub fn remove(home: &Path) -> Result<bool, Error> {
    let timer_file = timer_path(home);
    if !timer_file.exists() {
        return Ok(false);
    }
    let timer_unit = format!("{UNIT}.timer");
    // NOTE: no specification governs this: our own design. Without a running
    // user systemd there is nothing loaded to stop, and the files still go.
    let _stopped: bool = systemctl(&["disable", "--now", &timer_unit]).unwrap_or(false);
    let _cleaned: bool = systemctl(&["clean", "--what=state", &timer_unit]).unwrap_or(false);
    for file in [
        timer_file,
        units_folder(home).join(format!("{UNIT}.service")),
    ] {
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::Settings(format!(
                    "cannot delete {}: {e}",
                    file.display()
                )));
            }
        }
    }
    let _reloaded: bool = systemctl(&["daemon-reload"]).unwrap_or(false);
    Ok(true)
}

/// Runs `systemctl --user` and returns whether it succeeded; `false` when
/// there is no `systemctl`.
fn systemctl(args: &[&str]) -> Result<bool, Error> {
    match Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => Ok(status.success()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::Launch(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::{UNIT, timer_path};
    use std::path::Path;

    #[test]
    fn the_timer_is_a_user_unit() {
        let path = timer_path(Path::new("/home/u"));
        assert!(
            path.ends_with(format!("systemd/user/{UNIT}.timer")),
            "{}",
            path.display()
        );
    }
}
