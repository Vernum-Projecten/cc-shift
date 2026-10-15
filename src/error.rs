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
    /// The profile folder could not be created.
    #[error("cannot create {}: {source}", path.display())]
    CreateFolder {
        /// The folder `cs` tried to create.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The profiles root exists but cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    ReadRoot {
        /// The profiles root.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The state file or its lock cannot be used.
    #[error("cannot use {}: {source}", path.display())]
    State {
        /// The file or folder `cs` tried to use.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The state file does not parse.
    #[error("{} is damaged: {source}", path.display())]
    StateFormat {
        /// The state file.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: serde_json::Error,
    },
    /// A Keychain slot could not be used.
    #[error("cannot use {slot}: {reason}")]
    Keychain {
        /// The slot, as [`crate::login::Slot`] names it.
        slot: String,
        /// Why, without any token content.
        reason: String,
    },
    /// The account to switch to has no parked login.
    #[error("'{0}' has no parked login; run 'cs add {0}' and /login")]
    NoParkedLogin(String),
    /// An HTTP request got no response.
    #[error("{0}")]
    Http(String),
    /// Anthropic's token endpoint refused a refresh: the login is revoked or
    /// expired.
    #[error("'{0}' is logged out; run 'cs add {0}' and /login")]
    LoggedOut(String),
    /// A stored login or a response is not in the expected form.
    #[error("{0}")]
    Format(String),
    /// A `.claude.json` with the account details cannot be used.
    #[error("{0}")]
    Account(String),
    /// `~/.claude/settings.json` cannot be read, parsed or written.
    #[error("{0}")]
    Settings(String),
    /// No `claude` binary on `PATH`.
    #[error("claude was not found on PATH; install Claude Code first")]
    ClaudeNotFound,
    /// `claude` was found but could not be started.
    #[error("cannot start claude: {0}")]
    Launch(#[source] io::Error),
}
