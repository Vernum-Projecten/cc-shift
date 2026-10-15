//! `cs add <name>`: creates a profile and starts Claude Code in it for
//! `/login`.

use std::convert::Infallible;
use std::ffi::OsStr;

use crate::claude_code::launch;
use crate::command::report;
use crate::error::Error;
use crate::store::profile::{ProfileName, ProfilesRoot};

/// Creates the profile when missing and replaces `cs` with Claude Code in it.
///
/// # Errors
///
/// Returns the error of an invalid name, a missing `HOME`, a folder that
/// cannot be created, or a `claude` that cannot be started.
pub fn run(raw: &OsStr) -> Result<Infallible, Error> {
    let name = ProfileName::parse(raw)?;
    let root = ProfilesRoot::from_env()?;
    let dir = root.create(&name)?;
    report(&format!(
        "profile '{}' is at {}\nRun /login in Claude Code and sign in with the account for this profile, then quit.",
        name.as_str(),
        dir.display()
    ));
    launch::exec_claude(&dir)
}
