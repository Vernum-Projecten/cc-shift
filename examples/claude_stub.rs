// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! A stand-in for `claude` in the integration tests.
//!
//! The tests copy this program into the sandbox as `claude` (`claude.exe` on
//! Windows). It writes `CLAUDE_CONFIG_DIR` and each argument, one per line,
//! to `claude-called` in the home folder (`HOME`, else `USERPROFILE`), and
//! exits 0. It is a compiled program so that the same stub runs on every
//! system.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| io::Error::other("the stub claude found no home folder"))?;
    let mut record = String::new();
    let dir = env::var_os("CLAUDE_CONFIG_DIR").unwrap_or_default();
    writeln!(record, "CLAUDE_CONFIG_DIR={}", dir.to_string_lossy()).map_err(io::Error::other)?;
    for arg in env::args_os().skip(1) {
        writeln!(record, "arg={}", arg.to_string_lossy()).map_err(io::Error::other)?;
    }
    fs::write(PathBuf::from(home).join("claude-called"), record)
}
