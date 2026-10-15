//! The `cc-shift` commands: parsing the command line and running each command.
//!
//! Every command has its own module. This module dispatches to them and
//! holds what they share: how a message reaches the user, the current time,
//! and the automatic switch against the real Keychain and files.

pub mod activate;
pub mod add;
pub mod completion;
pub mod doctor;
pub mod early;
pub mod help;
pub mod hook;
pub mod list;
pub mod log;
pub mod parse;
pub mod refresh;
pub mod remove;
pub mod setup;
pub mod stats;
pub mod status;
pub mod statusline;
pub mod uninstall;

use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::error::Error;
use crate::login::account::AccountFiles;
use crate::login::macos::keychain::SystemKeychain;
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::switching::auto::{self, Trigger};
use parse::Command;

/// The exit code for a command that failed.
const FAILURE: u8 = 1;

/// The exit code for a command line that `cc-shift` does not understand.
const USAGE_ERROR: u8 = 2;

/// Parses the arguments, runs the command and returns the exit code.
#[must_use]
pub fn run<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    let result = match parse::parse(args) {
        Command::Help => {
            help::run();
            Ok(())
        }
        Command::Version => {
            help::version();
            Ok(())
        }
        Command::Misuse(reason) => {
            report(&format!("{reason}\n\n{}", parse::USAGE));
            return ExitCode::from(USAGE_ERROR);
        }
        Command::Add(name) => add::run(&name).map(|never| match never {}),
        Command::List => list::run(),
        Command::Use(name) => activate::run(&name),
        Command::Setup => setup::run(),
        Command::Refresh => refresh::run(),
        Command::Doctor => doctor::run(),
        Command::Log(options) => log::run(&options),
        Command::Early(action) => early::run(action),
        Command::Completion(shell) => completion::run(&shell),
        Command::Remove(name) => remove::run(&name),
        Command::Uninstall { all } => uninstall::run(all),
        Command::Status { json } => status::run(json),
        Command::Stats(options) => stats::run(&options),
        Command::HookRateLimit => {
            hook::run();
            Ok(())
        }
        Command::Statusline => {
            statusline::run();
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error.to_string());
            ExitCode::from(FAILURE)
        }
    }
}

/// Writes one `cc-shift:` message to stderr.
pub fn report(message: &str) {
    // NOTE: no specification governs this: our own design. If stderr itself
    // is closed there is nowhere left to report, so the write error is dropped.
    let _ignored: std::io::Result<()> = writeln!(std::io::stderr().lock(), "cc-shift: {message}");
}

/// Returns the current time in Unix seconds.
#[must_use]
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Returns `$HOME`.
///
/// # Errors
///
/// Returns [`Error::NoHome`] when it is unset or empty.
pub fn home() -> Result<PathBuf, Error> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or(Error::NoHome)
}

/// Runs an automatic switch against the real Keychain and files.
///
/// # Errors
///
/// Returns the error that stopped the switch.
pub fn automatic(trigger: Trigger) -> Result<Option<String>, Error> {
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let logins = SystemKeychain::for_user(&root)?;
    let accounts = AccountFiles::new(&home, &root);
    let file = StateFile::in_root(&root);
    let context = auto::Context {
        logins: &logins,
        accounts: &accounts,
        file: &file,
        root: &root,
    };
    auto::run(&context, trigger, unix_now())
}
