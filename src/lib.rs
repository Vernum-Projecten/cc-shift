//! `cs` launches Claude Code with one of several isolated account profiles.
//!
//! Each profile is a folder under `~/.claude-profiles/`, and `cs` points
//! Claude Code at it through `CLAUDE_CONFIG_DIR`. The binary in `main.rs`
//! only calls [`run`], so the whole run path is testable from `tests/`.

pub mod cli;
pub mod error;
pub mod launch;
pub mod profile;
pub mod select;
pub mod state;
pub mod statusline;
pub mod usage;

use std::convert::Infallible;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::process::ExitCode;

use cli::Command;
use error::Error;
use profile::{ProfileName, ProfilesRoot};
use state::StateFile;

/// The exit code for a command that failed.
const FAILURE: u8 = 1;

/// The exit code for a command line that `cs` does not understand.
const USAGE_ERROR: u8 = 2;

/// Runs `cs` with the given arguments, without the program name.
///
/// Returns the process exit code: success after printing help, `1` for a
/// failed command and `2` for a command line `cs` does not understand. A
/// command that starts Claude Code replaces this process and does not return.
#[must_use]
pub fn run<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    match cli::parse(args) {
        Command::Help => {
            // NOTE: no specification governs this: our own design. A failed
            // write to stdout (a closed pipe) is not an error for help text.
            let _ignored: std::io::Result<()> = std::io::stdout().write_all(cli::USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        Command::Misuse(reason) => {
            report(&format!("{reason}\n\n{}", cli::USAGE));
            ExitCode::from(USAGE_ERROR)
        }
        Command::List => match list() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                report(&error.to_string());
                ExitCode::from(FAILURE)
            }
        },
        Command::Add(name) => fail(add(&name)),
        Command::Statusline => {
            statusline_command();
            ExitCode::SUCCESS
        }
    }
}

/// Reports the error of a command that would otherwise have replaced `cs`.
fn fail(result: Result<Infallible, Error>) -> ExitCode {
    match result {
        Ok(never) => match never {},
        Err(error) => {
            report(&error.to_string());
            ExitCode::from(FAILURE)
        }
    }
}

/// Records usage from the status line input on stdin and prints the user's
/// own status line. It never fails: a `cs` problem must not break the
/// status line of a Claude Code window.
fn statusline_command() {
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        return;
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
    let chained = ProfilesRoot::from_env()
        .ok()
        .and_then(|root| statusline::record(&input, &StateFile::in_root(&root), now).ok())
        .flatten();
    if let Some(command) = chained {
        let line = statusline::chained(&command, &input);
        let _ignored: std::io::Result<()> = std::io::stdout().write_all(&line);
    }
}

/// Creates a profile when missing and starts Claude Code in it.
fn add(raw: &OsStr) -> Result<Infallible, Error> {
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

/// Prints every profile name on its own line, or how to create the first.
fn list() -> Result<(), Error> {
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

/// Writes one `cs:` message to stderr.
fn report(message: &str) {
    // NOTE: no specification governs this: our own design. If stderr itself
    // is closed there is nowhere left to report, so the write error is dropped.
    let _ignored: std::io::Result<()> = writeln!(std::io::stderr().lock(), "cs: {message}");
}
