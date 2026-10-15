// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Starting Claude Code with a profile's config folder, and the other
//! programs `cc-shift` runs.

use std::convert::Infallible;
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::process::Command;

use crate::error::Error;

/// Replaces the `cc-shift` process with `claude`, using `config_dir` as its
/// `CLAUDE_CONFIG_DIR`.
///
/// `claude` is looked up on `PATH` (see [`claude_program`]). Claude Code
/// reads `CLAUDE_CONFIG_DIR` as the folder for its configuration and login
/// state (<https://code.claude.com/docs/en/settings>). Windows has no `exec`,
/// so there `cc-shift` runs `claude`, waits for it and exits with its code.
///
/// # Errors
///
/// Returns [`Error::ClaudeNotFound`] when no `claude` is on `PATH`, and
/// [`Error::Launch`] when it cannot be started. On success this function
/// never returns.
#[cfg_attr(
    windows,
    expect(
        clippy::exit,
        reason = "Windows has no exec, so cc-shift add ends with the exit code of claude as exec would"
    )
)]
pub fn exec_claude(config_dir: &Path) -> Result<Infallible, Error> {
    let mut command = Command::new(claude_program());
    command.env("CLAUDE_CONFIG_DIR", config_dir);
    #[cfg(unix)]
    let failure = std::os::unix::process::CommandExt::exec(&mut command);
    #[cfg(windows)]
    let failure = match command.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => e,
    };
    if failure.kind() == io::ErrorKind::NotFound {
        Err(Error::ClaudeNotFound)
    } else {
        Err(Error::Launch(failure))
    }
}

/// Returns the program to run for `claude`.
///
/// On Windows a program is found on `PATH` only as `claude.exe`, so an npm
/// install's `claude.cmd` is looked up by hand; elsewhere it is `claude`.
#[must_use]
pub fn claude_program() -> OsString {
    if cfg!(windows) {
        let path = std::env::var_os("PATH").unwrap_or_default();
        if let Some(found) = find_program(&path, &["claude.exe", "claude.cmd"]) {
            return found.into_os_string();
        }
    }
    OsString::from("claude")
}

/// Returns the first of `names` in the folders of `path`, trying every name
/// in a folder before the next folder.
#[must_use]
pub fn find_program(path: &std::ffi::OsStr, names: &[&str]) -> Option<std::path::PathBuf> {
    std::env::split_paths(path).find_map(|folder| {
        names
            .iter()
            .map(|name| folder.join(name))
            .find(|candidate| candidate.is_file())
    })
}

/// Returns the Windows system folder, `%SystemRoot%\System32`, where Windows
/// keeps `curl.exe` and `schtasks.exe`; `None` when `SystemRoot` is unset or
/// empty, which a working Windows never leaves it.
#[must_use]
pub fn system32() -> Option<std::path::PathBuf> {
    std::env::var_os("SystemRoot")
        .filter(|root| !root.is_empty())
        .map(|root| std::path::PathBuf::from(root).join("System32"))
}

/// Makes `command` start without a console window on Windows, where a
/// console program started from the status line, a hook or a scheduled task
/// would otherwise open one; nothing changes elsewhere.
pub fn without_window(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        /// `CREATE_NO_WINDOW`, from the process creation flags
        /// (<https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags>).
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::os::windows::process::CommandExt::creation_flags(command, CREATE_NO_WINDOW);
    }
    command
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::find_program;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn programs_are_found_by_folder_then_by_name() -> Result<(), Box<dyn std::error::Error>> {
        let dir = TempDir::new()?;
        let (first, second) = (dir.path().join("a"), dir.path().join("b"));
        fs::create_dir_all(&first)?;
        fs::create_dir_all(&second)?;
        fs::write(first.join("claude.cmd"), "")?;
        fs::write(second.join("claude.exe"), "")?;
        let path = std::env::join_paths([&first, &second])?;
        assert_eq!(
            find_program(&path, &["claude.exe", "claude.cmd"]),
            Some(first.join("claude.cmd")),
            "the earlier folder wins"
        );
        assert_eq!(find_program(&path, &["none"]), None);
        Ok(())
    }
}
