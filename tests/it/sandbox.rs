//! A fake home folder and a stub `claude`, so tests never touch the real
//! profiles, Keychain or account.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

/// The stub writes its config folder and each argument, one per line.
const STUB: &str = "#!/bin/sh\n\
{ printf 'CLAUDE_CONFIG_DIR=%s\\n' \"$CLAUDE_CONFIG_DIR\"; \
for a in \"$@\"; do printf 'arg=%s\\n' \"$a\"; done; } > \"$HOME/claude-called\"\n";

/// A temporary `HOME` and a `PATH` that holds only the stub `claude` and the
/// system tools the stub needs.
pub(crate) struct Sandbox {
    home: TempDir,
    bin: TempDir,
}

impl Sandbox {
    /// Creates a sandbox with the stub `claude` installed.
    pub(crate) fn new() -> io::Result<Self> {
        let sandbox = Self::without_claude()?;
        let stub = sandbox.bin.path().join("claude");
        fs::write(&stub, STUB)?;
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755))?;
        Ok(sandbox)
    }

    /// Creates a sandbox with no `claude` on `PATH`.
    pub(crate) fn without_claude() -> io::Result<Self> {
        Ok(Self {
            home: TempDir::new()?,
            bin: TempDir::new()?,
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

    /// Returns a `cs` command with the sandbox's `HOME` and `PATH`.
    pub(crate) fn cs(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cs"));
        command.env_clear().env("HOME", self.home()).env(
            "PATH",
            format!("{}:/usr/bin:/bin", self.bin.path().display()),
        );
        command
    }

    /// Runs `cs` with the given arguments.
    pub(crate) fn run(&self, args: &[&str]) -> io::Result<Output> {
        self.cs().args(args).output()
    }

    /// Returns what the stub `claude` recorded, or `None` if it never ran.
    pub(crate) fn claude_call(&self) -> Option<String> {
        fs::read_to_string(self.home().join("claude-called")).ok()
    }
}
