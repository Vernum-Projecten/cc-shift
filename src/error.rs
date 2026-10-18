// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The errors `cc-shift` reports to the user.

use std::io;
use std::path::PathBuf;

/// A failure that stops a `cc-shift` command, printed as one short line.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// `HOME` is unset or empty, so the profiles root cannot be found.
    #[error(
        "the home folder is not set (HOME, or USERPROFILE on Windows), so ~/.claude-profiles cannot be found"
    )]
    NoHome,
    /// A profile name with a character outside the allowed set.
    #[error(
        "invalid profile name '{0}': use letters, digits, '-' or '_', starting with a letter or digit"
    )]
    InvalidName(String),
    /// A profile name that is also a `cc-shift` command.
    #[error("'{0}' is a cc-shift command and cannot be a profile name")]
    ReservedName(String),
    /// A profile name that Windows reserves for a device, such as `con`.
    #[error("'{0}' is a device name on Windows and cannot be a profile name")]
    DeviceName(String),
    /// A new profile name that differs from an existing profile only in case.
    #[error(
        "'{name}' differs from the profile '{existing}' only in case; use 'cc-shift add {existing}' or another name"
    )]
    NameCase {
        /// The name that was asked for.
        name: String,
        /// The existing profile with the same name in another case.
        existing: String,
    },
    /// The profile folder could not be created.
    #[error("cannot create {}: {source}", path.display())]
    CreateFolder {
        /// The folder `cc-shift` tried to create.
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
        /// The file or folder `cc-shift` tried to use.
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
    /// A login slot (a Keychain item or a login file) could not be used.
    #[error("cannot use {slot}: {reason}")]
    LoginStore {
        /// The slot, as [`crate::login::Slot`] names it.
        slot: String,
        /// Why, without any token content.
        reason: String,
    },
    /// The account to switch to has no parked login.
    #[error("'{0}' has no parked login; run 'cc-shift add {0}' and /login")]
    NoParkedLogin(String),
    /// Completion was asked for a shell `cc-shift` has no script for.
    #[error("no completion for '{0}'; cc-shift completes zsh and powershell")]
    Completion(String),
    /// No profile folder exists for the name.
    #[error("there is no profile '{0}'; 'cc-shift list' shows them")]
    NoProfile(String),
    /// The profile to remove is the active account.
    #[error(
        "'{0}' is the active account; make another one active first with 'cc-shift use <name>'"
    )]
    RemoveActive(String),
    /// `cc-shift doctor` found a check that failed.
    #[error("doctor found a problem; the lines above say how to fix it")]
    DoctorFailed,
    /// An HTTP request got no response.
    #[error("{0}")]
    Http(String),
    /// Anthropic's token endpoint refused a refresh: the login is revoked or
    /// expired.
    #[error("'{0}' is logged out; run 'cc-shift add {0}' and /login")]
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
    /// A transcript or a transcript folder cannot be read.
    #[error("cannot read {}: {source}", path.display())]
    StatsRead {
        /// The file or folder.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The statistics cache cannot be written.
    #[error("cannot write {}: {source}", path.display())]
    StatsCache {
        /// The cache file.
        path: PathBuf,
        /// The underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The start of a statistics period cannot be worked out.
    #[error("cannot work out the period: {0}")]
    Period(#[source] jiff::Error),
    /// `cc-shift stats --project` names a project with no response in the
    /// period.
    #[error("no project '{name}' has responses ({period}); 'cc-shift stats' shows the names")]
    NoProject {
        /// The name asked for.
        name: String,
        /// The period, as the summary line names it.
        period: String,
    },
    /// No `claude` binary on `PATH`.
    #[error("claude was not found on PATH; install Claude Code first")]
    ClaudeNotFound,
    /// `claude` was found but could not be started.
    #[error("cannot start claude: {0}")]
    Launch(#[source] io::Error),
    /// Claude Code held its refresh lock for longer than a switch waits.
    #[error("Claude Code is refreshing the login in {}; the switch is retried later", .0.display())]
    RefreshLocked(PathBuf),
    /// Claude Code on Windows keeps this folder's login in Credential Manager.
    #[error(
        "Claude Code keeps the login of {} in Windows Credential Manager, which cc-shift cannot switch; nothing was moved",
        .0.display()
    )]
    CredentialManager(PathBuf),
}
