//! `cs setup`: installs `cs` in Claude Code once, so switching is
//! automatic.

use crate::claude_code::settings;
use crate::command::{automatic, home, report};
use crate::error::Error;
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::switching::auto::Trigger;

/// Installs the status line and the hook in `~/.claude/settings.json`,
/// remembers the user's own status line to chain it, and makes the best
/// account active when none is.
///
/// # Errors
///
/// Returns the error of a missing `HOME`, settings that cannot be read or
/// written, or a switch that could not run.
pub fn run() -> Result<(), Error> {
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let cs_path = std::env::current_exe()
        .map_err(|e| Error::Settings(format!("cannot find the cs binary: {e}")))?;
    let path = settings::settings_path(&home);
    let installed = settings::install(settings::read_settings(&path)?, &cs_path.to_string_lossy())?;
    let file = StateFile::in_root(&root);
    if let Some(previous) = installed.previous_statusline.clone() {
        file.update(|state| state.chained_statusline = Some(previous))?;
    }
    if installed.changed {
        settings::write_settings(&path, &installed.settings)?;
        report(&format!(
            "installed the status line and the rate-limit hook in {}",
            path.display()
        ));
    } else {
        report("already set up");
    }
    if root.list()?.is_empty() {
        report("no profiles yet; add them with 'cs add <name>', then run 'cs setup' again");
        return Ok(());
    }
    if file.read()?.active.is_none() {
        report("the login now in ~/.claude belongs to no profile and is replaced");
    }
    match automatic(Trigger::Setup)? {
        Some(name) => report(&format!(
            "'{name}' is active; switching is automatic from now on"
        )),
        None => report("switching is automatic from now on"),
    }
    Ok(())
}
