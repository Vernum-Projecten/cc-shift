//! `cs` launches Claude Code with one of several isolated account profiles.
//!
//! Each profile is a folder under `~/.claude-profiles/`, and `cs` points
//! Claude Code at it through `CLAUDE_CONFIG_DIR`. The binary in `main.rs`
//! only calls [`run`], so the whole run path is testable from `tests/`.

pub mod cli;

use std::ffi::OsString;
use std::io::Write;
use std::process::ExitCode;

use cli::Command;

/// The exit code for a command line that `cs` does not understand.
const USAGE_ERROR: u8 = 2;

/// Runs `cs` with the given arguments, without the program name.
///
/// Returns the process exit code: success after printing help, and `2` for a
/// command line `cs` does not understand.
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
        Command::Unknown(word) => {
            let mut stderr = std::io::stderr().lock();
            let _ignored: std::io::Result<()> = writeln!(
                stderr,
                "cs: unknown command '{}'\n\n{}",
                word.to_string_lossy(),
                cli::USAGE
            );
            ExitCode::from(USAGE_ERROR)
        }
    }
}
