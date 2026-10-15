// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift` switches Claude Code between several accounts automatically.
//!
//! Each account has a profile folder under `~/.claude-profiles/` that holds
//! its parked login. The binary in `main.rs` only calls [`run`], so the
//! whole run path is testable from `tests/`.
//!
//! The modules follow the domain: `command` is the command line, `store`
//! the files `cc-shift` owns, `usage` what is known about each account, `login`
//! the logins (the Keychain on macOS, the login files on Linux) and the
//! account details, `switching` moving between
//! accounts, `anthropic` the calls to Anthropic, `claude_code` the places
//! where `cc-shift` plugs into Claude Code, and `stats` the token statistics
//! read from Claude Code's transcripts, and `early` the early start.

// NOTE: no specification governs this: our own design. cc-shift knows where
// Claude Code keeps logins on macOS, Linux and Windows, so other builds stop here.
#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
compile_error!("cc-shift works on macOS, Linux and Windows only");

pub mod anthropic;
pub mod claude_code;
pub mod command;
pub mod early;
pub mod error;
pub mod login;
pub mod render;
pub mod stats;
pub mod store;
pub mod switching;
pub mod usage;

use std::ffi::OsString;
use std::process::ExitCode;

/// Runs `cc-shift` with the given arguments, without the program name.
///
/// Returns the process exit code: success after printing help, `1` for a
/// failed command and `2` for a command line `cc-shift` does not understand.
/// `cc-shift add` replaces this process with Claude Code and does not return.
#[must_use]
pub fn run<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    command::run(args)
}
