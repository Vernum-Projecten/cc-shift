//! Starting Claude Code with a profile's config folder.

use std::convert::Infallible;
use std::ffi::OsString;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use crate::error::Error;

/// Replaces the `cs` process with `claude`, using `config_dir` as its
/// `CLAUDE_CONFIG_DIR`.
///
/// `claude` is looked up on `PATH`, and `args` reach it unchanged and in
/// order. Claude Code reads `CLAUDE_CONFIG_DIR` as the folder for its
/// configuration and login state
/// (<https://docs.anthropic.com/en/docs/claude-code/settings>).
///
/// # Errors
///
/// Returns [`Error::ClaudeNotFound`] when no `claude` is on `PATH`, and
/// [`Error::Launch`] when it cannot be started. On success this function
/// never returns.
pub fn exec_claude(config_dir: &Path, args: &[OsString]) -> Result<Infallible, Error> {
    let failure = Command::new("claude")
        .env("CLAUDE_CONFIG_DIR", config_dir)
        .args(args)
        .exec();
    if failure.kind() == io::ErrorKind::NotFound {
        Err(Error::ClaudeNotFound)
    } else {
        Err(Error::Launch(failure))
    }
}
