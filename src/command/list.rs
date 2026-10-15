//! `cs list`: prints the profile names.

use std::io::Write;

use crate::command::report;
use crate::error::Error;
use crate::store::profile::ProfilesRoot;

/// Prints every profile name on its own line, or how to create the first.
///
/// # Errors
///
/// Returns the error of a missing `HOME` or an unreadable profiles root.
pub fn run() -> Result<(), Error> {
    let names = ProfilesRoot::from_env()?.list()?;
    if names.is_empty() {
        report("no profiles yet; create one with 'cs add <name>'");
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    for name in names {
        // NOTE: no specification governs this: our own design. A closed pipe
        // (`cs list | head -1`) ends the listing without an error.
        if writeln!(stdout, "{name}").is_err() {
            break;
        }
    }
    Ok(())
}
