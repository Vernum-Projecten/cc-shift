//! The errors `cs` reports to the user.

use std::io;
use std::path::PathBuf;

/// A failure that stops a `cs` command, printed as one short line.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `HOME` is unset or empty, so the profiles root cannot be found.
    #[error("HOME is not set, so ~/.claude-profiles cannot be found")]
    NoHome,
    /// A profile name with a character outside the allowed set.
    #[error(
        "invalid profile name '{0}': use letters, digits, '-' or '_', starting with a letter or digit"
    )]
    InvalidName(String),
    /// A profile name that is also a `cs` command.
    #[error("'{0}' is a cs command and cannot be a profile name")]
    ReservedName(String),
    /// No folder exists for the named profile.
    #[error("profile '{0}' does not exist; create it with 'cs add {0}'")]
    NoProfile(String),
    /// The profile folder could not be created.
    #[error("cannot create {}: {source}", path.display())]
    CreateFolder {
        /// The folder `cs` tried to create.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// No `claude` binary on `PATH`.
    #[error("claude was not found on PATH; install Claude Code first")]
    ClaudeNotFound,
    /// `claude` was found but could not be started.
    #[error("cannot start claude: {0}")]
    Launch(#[source] io::Error),
}
