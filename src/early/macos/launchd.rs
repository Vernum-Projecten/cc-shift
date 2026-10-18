// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The `launchd` agent that older versions installed to run the early start.
//!
//! A user agent lives in `~/Library/LaunchAgents` and is unloaded from the
//! user's `gui/<uid>` domain
//! (<https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html>).

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::Error;

/// The label of the agent, which is also its file name.
pub const LABEL: &str = "io.github.vernum-projecten.cc-shift.early-start";

/// Returns the path of the agent's file under `home`.
#[must_use]
pub fn agent_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// Unloads the agent and deletes its file; does nothing when there is
/// none. Returns whether a file was deleted.
///
/// # Errors
///
/// Returns [`Error::Settings`] when the file cannot be deleted.
pub fn remove(home: &Path) -> Result<bool, Error> {
    let file = agent_path(home);
    if !file.exists() {
        return Ok(false);
    }
    unload(home)?;
    fs::remove_file(&file)
        .map_err(|e| Error::Settings(format!("cannot delete {}: {e}", file.display())))?;
    Ok(true)
}

/// Unloads the agent if it is loaded.
fn unload(home: &Path) -> Result<(), Error> {
    let target = format!("{}/{LABEL}", domain(home)?);
    // NOTE: no specification governs this: our own design. `bootout` fails
    // when the agent is not loaded, which is the state it is meant to reach.
    let _unloaded: bool = launchctl(&["bootout", &target])?;
    Ok(())
}

/// Returns the user's `launchd` domain, `gui/<uid>`, taking the uid from
/// the owner of `home`.
fn domain(home: &Path) -> Result<String, Error> {
    let uid = fs::metadata(home)
        .map_err(|e| Error::Settings(format!("cannot read {}: {e}", home.display())))?
        .uid();
    Ok(format!("gui/{uid}"))
}

/// Runs `launchctl` and returns whether it succeeded.
fn launchctl(args: &[&str]) -> Result<bool, Error> {
    Command::new("/bin/launchctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .map_err(Error::Launch)
}

#[cfg(test)]
mod tests {
    use super::{LABEL, agent_path};
    use std::path::Path;

    #[test]
    fn the_agent_lives_in_the_users_launch_agents() {
        assert_eq!(
            agent_path(Path::new("/u")),
            Path::new("/u/Library/LaunchAgents").join(format!("{LABEL}.plist"))
        );
    }
}
