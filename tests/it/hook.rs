// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift hook rate-limit`: never blocks a Claude Code turn.

use std::io::{self, Write};
use std::process::Stdio;

use crate::sandbox::Sandbox;

fn hook(sandbox: &Sandbox, home: bool) -> io::Result<std::process::Output> {
    let mut command = sandbox.cc_shift();
    if !home {
        command.env_remove("HOME");
    }
    let mut child = command
        .args(["hook", "rate-limit"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(br#"{"hook_event_name": "StopFailure", "error": "rate_limit"}"#)?;
    }
    child.wait_with_output()
}

#[test]
fn succeeds_before_setup() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = hook(&sandbox, true)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert!(out.stderr.is_empty(), "nothing on stderr");
    assert!(
        !sandbox.root().join("state.json").exists(),
        "nothing recorded"
    );
    Ok(())
}

// NOTE: <https://doc.rust-lang.org/std/env/fn.home_dir.html> falls back to the
// real profile folder on Windows, so the case runs where `HOME` is the only source.
#[cfg(unix)]
#[test]
fn succeeds_without_home() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = hook(&sandbox, false)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    Ok(())
}

#[test]
fn another_event_is_misuse() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["hook", "something-else"])?;
    assert_eq!(out.status.code(), Some(2), "exit code");
    Ok(())
}
