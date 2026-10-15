// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Starting an account's 5-hour window with one short Claude Code message.
//!
//! Claude Code sends the message with the login of the config folder it
//! runs in, so `cc-shift` never touches a token here. The message runs on
//! Haiku with no tools, no hooks from the user's settings, no MCP servers
//! and no saved session, so it does nothing but start the window.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::error::Error;

/// How long the message may take before `claude` is stopped.
const TIMEOUT: Duration = Duration::from_secs(120);

/// How often a running `claude` is checked.
const POLL: Duration = Duration::from_millis(250);

/// Returns the arguments of the `claude` run that starts a window.
///
/// `--setting-sources project` with an empty working folder loads no
/// settings file, so no hook of the user's runs, and
/// `--no-session-persistence` keeps the message out of the transcripts
/// (<https://code.claude.com/docs/en/cli-reference>).
#[must_use]
pub fn arguments() -> [&'static str; 11] {
    [
        "-p",
        "Reply with one word: ok",
        "--model",
        "haiku",
        "--tools",
        "",
        "--setting-sources",
        "project",
        "--strict-mcp-config",
        "--no-session-persistence",
        "--disable-slash-commands",
    ]
}

/// Sends the message with the login of `config_dir`, or of `~/.claude` when
/// `None`, from the empty folder `workdir`.
///
/// # Errors
///
/// Returns [`Error::ClaudeNotFound`] when no `claude` is on `PATH`,
/// [`Error::Launch`] when it cannot be started or waited for, and
/// [`Error::EarlyStart`] when it fails or takes longer than two minutes.
pub fn start_window(config_dir: Option<&Path>, workdir: &Path) -> Result<(), Error> {
    std::fs::create_dir_all(workdir).map_err(|source| Error::CreateFolder {
        path: workdir.to_path_buf(),
        source,
    })?;
    let mut command = Command::new("claude");
    command
        .args(arguments())
        .current_dir(workdir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match config_dir {
        Some(dir) => command.env("CLAUDE_CONFIG_DIR", dir),
        None => command.env_remove("CLAUDE_CONFIG_DIR"),
    };
    let mut child = command.spawn().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            Error::ClaudeNotFound
        } else {
            Error::Launch(e)
        }
    })?;
    let began = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(Error::Launch)? {
            return if status.success() {
                Ok(())
            } else {
                Err(Error::EarlyStart(format!("claude exited with {status}")))
            };
        }
        if began.elapsed() > TIMEOUT {
            // NOTE: no specification governs this: our own design. A child
            // that cannot be stopped has already been reported as too slow.
            let _killed: io::Result<()> = child.kill();
            let _reaped: io::Result<std::process::ExitStatus> = child.wait();
            return Err(Error::EarlyStart(
                "claude took longer than two minutes".to_owned(),
            ));
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
mod tests {
    use super::arguments;

    #[test]
    fn the_message_runs_on_haiku_without_tools_hooks_or_a_session() {
        let args = arguments().join(" ");
        for part in [
            "-p ",
            "--model haiku",
            "--tools  --setting-sources project",
            "--strict-mcp-config",
            "--no-session-persistence",
        ] {
            assert!(args.contains(part), "{part} in {args}");
        }
    }
}
