// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The Task Scheduler task that older versions installed to run the early
//! start.
//!
//! The task is deleted with `schtasks /Delete`
//! (<https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-delete>),
//! and the XML it was imported from, kept as the record that it is
//! installed, goes with it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::claude_code::launch::{system32, without_window};
use crate::error::Error;

/// The name of the task in Task Scheduler.
pub const TASK: &str = "cc-shift early start";

/// Returns the `cc-shift` folder of the user's local application data under
/// `home`: `%LOCALAPPDATA%\cc-shift`.
#[must_use]
pub fn data_folder(home: &Path) -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map_or_else(|| home.join("AppData").join("Local"), PathBuf::from)
        .join("cc-shift")
}

/// Returns the path of the XML the task was imported from.
#[must_use]
pub fn task_path(home: &Path) -> PathBuf {
    data_folder(home).join("early-start.xml")
}

/// Deletes the task and its definition; does nothing when there is no
/// definition. Returns whether there was one.
///
/// # Errors
///
/// Returns [`Error::Settings`] when the definition cannot be deleted.
pub fn remove(home: &Path) -> Result<bool, Error> {
    let file = task_path(home);
    if !file.exists() {
        return Ok(false);
    }
    // NOTE: no specification governs this: our own design. A task that is
    // already gone from Task Scheduler leaves only its definition to delete.
    let _deleted: bool = schtasks(&["/Delete", "/TN", TASK, "/F"]).unwrap_or(false);
    fs::remove_file(&file)
        .map_err(|e| Error::Settings(format!("cannot delete {}: {e}", file.display())))?;
    Ok(true)
}

/// Runs `schtasks.exe` from the system folder without a console window and
/// returns whether it succeeded.
///
/// # Errors
///
/// Returns [`Error::Settings`] when `SystemRoot` is unset, so the system
/// folder is unknown, and [`Error::Launch`] when `schtasks.exe` cannot run.
fn schtasks(args: &[&str]) -> Result<bool, Error> {
    let program = system32()
        .ok_or_else(|| {
            Error::Settings(
                "SystemRoot is not set, so schtasks.exe cannot be found; open a new terminal"
                    .to_owned(),
            )
        })?
        .join("schtasks.exe");
    without_window(&mut Command::new(program))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .map_err(Error::Launch)
}
