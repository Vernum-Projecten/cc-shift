// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift statusline`: records usage and never breaks the status line.

use std::fs;
use std::io::{self, Write};
use std::process::{Output, Stdio};

use crate::sandbox::Sandbox;

const INPUT: &str =
    r#"{"rate_limits": {"five_hour": {"used_percentage": 12.5, "resets_at": 4102444800}}}"#;

fn run(sandbox: &Sandbox, input: &str, home: bool) -> io::Result<Output> {
    let mut command = sandbox.cc_shift();
    command.env("NO_COLOR", "1");
    if !home {
        command.env_remove("HOME");
    }
    let mut child = command
        .arg("statusline")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes())?;
    }
    child.wait_with_output()
}

fn write_state(sandbox: &Sandbox, json: &str) -> io::Result<()> {
    fs::create_dir_all(sandbox.root().join("member1"))?;
    fs::write(sandbox.root().join("state.json"), json)
}

#[test]
fn records_the_active_accounts_usage() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    write_state(&sandbox, r#"{"active": "member1"}"#)?;
    let out = run(&sandbox, INPUT, true)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    let state = fs::read_to_string(sandbox.root().join("state.json"))?;
    assert!(state.contains("\"member1\""), "{state}");
    assert!(state.contains("12.5"), "{state}");
    Ok(())
}

#[test]
fn prints_the_chained_status_line() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    write_state(
        &sandbox,
        r#"{"active": "member1", "chained_statusline": "printf 'my line'"}"#,
    )?;
    let out = run(&sandbox, INPUT, true)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with("my line │ member1 ▕█░░░░░░░░░▏ 12% · "),
        "the user's line, then the cc-shift part: {text}"
    );
    Ok(())
}

#[test]
fn shows_the_active_account_without_a_chained_line() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    write_state(&sandbox, r#"{"active": "member1"}"#)?;
    let out = run(&sandbox, INPUT, true)?;
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.starts_with("member1 ▕█░░░░░░░░░▏ 12% · "), "{text}");
    Ok(())
}

#[test]
fn succeeds_on_input_it_cannot_read() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = run(&sandbox, "not json", true)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert!(out.stderr.is_empty(), "nothing on stderr");
    Ok(())
}

#[test]
fn succeeds_with_a_damaged_state_file() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    write_state(&sandbox, "{ damaged")?;
    let out = run(&sandbox, INPUT, true)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert!(out.stderr.is_empty(), "nothing on stderr");
    Ok(())
}

// NOTE: <https://doc.rust-lang.org/std/env/fn.home_dir.html> falls back to the
// real profile folder on Windows, so the case runs where `HOME` is the only source.
#[cfg(unix)]
#[test]
fn succeeds_without_home() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = run(&sandbox, INPUT, false)?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    Ok(())
}
