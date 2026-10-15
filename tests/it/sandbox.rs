// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! A fake home folder and a stub `claude`, so tests never touch the real
//! profiles, Keychain or account.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// The variables that name the home folder: `HOME`, and on Windows also
/// `USERPROFILE`, which `std::env::home_dir` reads there
/// (<https://doc.rust-lang.org/std/env/fn.home_dir.html>).
const HOME_VARS: &[&str] = if cfg!(windows) {
    &["HOME", "USERPROFILE"]
} else {
    &["HOME"]
};

/// The variables a Windows process needs from the real environment after
/// `env_clear`: `SystemRoot` for the system libraries, and `PATHEXT` for
/// the lookup of a command by its bare name.
const KEPT_VARS: &[&str] = if cfg!(windows) {
    &["SystemRoot", "PATHEXT"]
} else {
    &[]
};

/// A temporary `HOME` and a `PATH` that holds only the stub `claude` and the
/// system tools the stub needs.
pub(crate) struct Sandbox {
    home: TempDir,
    bin: TempDir,
    path: OsString,
}

impl Sandbox {
    /// Creates a sandbox with the stub `claude` installed.
    pub(crate) fn new() -> io::Result<Self> {
        let sandbox = Self::without_claude()?;
        let stub = format!("claude{}", env::consts::EXE_SUFFIX);
        fs::copy(stub_program()?, sandbox.bin.path().join(stub))?;
        Ok(sandbox)
    }

    /// Creates a sandbox with no `claude` on `PATH`.
    pub(crate) fn without_claude() -> io::Result<Self> {
        let bin = TempDir::new()?;
        let path = search_path(bin.path())?;
        Ok(Self {
            home: TempDir::new()?,
            bin,
            path,
        })
    }

    /// Returns the fake home folder.
    pub(crate) fn home(&self) -> &Path {
        self.home.path()
    }

    /// Returns the profiles root inside the fake home.
    pub(crate) fn root(&self) -> PathBuf {
        self.home().join(".claude-profiles")
    }

    /// Returns a `cc-shift` command with the sandbox's home and `PATH`.
    pub(crate) fn cc_shift(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cc-shift"));
        command.env_clear();
        for name in KEPT_VARS {
            if let Some(value) = env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in HOME_VARS {
            command.env(name, self.home());
        }
        command.env("PATH", &self.path);
        command
    }

    /// Runs `cc-shift` with the given arguments.
    pub(crate) fn run(&self, args: &[&str]) -> io::Result<Output> {
        self.cc_shift().args(args).output()
    }

    /// Returns what the stub `claude` recorded, or `None` if it never ran.
    pub(crate) fn claude_call(&self) -> Option<String> {
        fs::read_to_string(self.home().join("claude-called")).ok()
    }
}

/// Returns the sandbox `PATH`: the stub's folder, then the system folders.
///
/// # Errors
///
/// Returns an error when a folder name holds the `PATH` separator.
fn search_path(bin: &Path) -> io::Result<OsString> {
    let mut dirs = vec![bin.to_path_buf()];
    if cfg!(windows) {
        if let Some(root) = env::var_os("SystemRoot") {
            dirs.push(PathBuf::from(root).join("System32"));
        }
    } else {
        dirs.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
    }
    env::join_paths(dirs).map_err(io::Error::other)
}

/// Returns the compiled stub `claude`, which Cargo builds as the example
/// `claude_stub` next to the `cc-shift` binary.
///
/// # Errors
///
/// Returns [`io::ErrorKind::NotFound`] when the example was not built, as
/// with `cargo nextest run --test it`, which builds no examples.
fn stub_program() -> io::Result<PathBuf> {
    let program = Path::new(env!("CARGO_BIN_EXE_cc-shift"))
        .parent()
        .map(|dir| {
            dir.join("examples")
                .join(format!("claude_stub{}", env::consts::EXE_SUFFIX))
        })
        .filter(|program| program.is_file());
    program.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "the claude_stub example is not built; run the whole test suite",
        )
    })
}
