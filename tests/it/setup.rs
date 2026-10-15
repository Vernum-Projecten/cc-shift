// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift setup`: installs the status line and the hook, keeping the rest.

use std::fs;
use std::io;
use std::process::Output;

use crate::sandbox::Sandbox;

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn settings(sandbox: &Sandbox) -> io::Result<String> {
    fs::read_to_string(sandbox.home().join(".claude/settings.json"))
}

#[test]
fn installs_into_a_fresh_home() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["setup"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = settings(&sandbox)?;
    assert!(text.contains(" statusline\""), "{text}");
    assert!(text.contains(" hook rate-limit\""), "{text}");
    assert!(text.contains("\"rate_limit\""), "{text}");
    Ok(())
}

#[test]
fn keeps_settings_and_chains_the_status_line() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.home().join(".claude"))?;
    fs::write(
        sandbox.home().join(".claude/settings.json"),
        r#"{"model": "opus", "statusLine": {"type": "command", "command": "my-line"}}"#,
    )?;
    let out = sandbox.run(&["setup"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        settings(&sandbox)?.contains("\"model\": \"opus\""),
        "model kept"
    );
    let state = fs::read_to_string(sandbox.root().join("state.json"))?;
    assert!(
        state.contains("\"chained_statusline\": \"my-line\""),
        "{state}"
    );
    Ok(())
}

#[test]
fn a_second_run_is_already_set_up() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    sandbox.run(&["setup"])?;
    let first = settings(&sandbox)?;
    let out = sandbox.run(&["setup"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).contains("already set up"), "{}", stderr(&out));
    assert_eq!(settings(&sandbox)?, first, "unchanged");
    Ok(())
}

#[test]
fn refuses_settings_it_cannot_parse() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.home().join(".claude"))?;
    fs::write(sandbox.home().join(".claude/settings.json"), "{ broken")?;
    let out = sandbox.run(&["setup"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("does not parse"), "{}", stderr(&out));
    assert_eq!(settings(&sandbox)?, "{ broken", "left as it was");
    Ok(())
}

#[test]
fn uninstall_puts_the_settings_back() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.home().join(".claude"))?;
    let original = r#"{"model": "opus", "statusLine": {"type": "command", "command": "my-line"}}"#;
    fs::write(sandbox.home().join(".claude/settings.json"), original)?;
    sandbox.run(&["setup"])?;
    let out = sandbox.run(&["uninstall"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let value: serde_json::Value =
        serde_json::from_str(&settings(&sandbox)?).map_err(io::Error::other)?;
    let expected: serde_json::Value = serde_json::from_str(original).map_err(io::Error::other)?;
    assert_eq!(value, expected);
    Ok(())
}

#[test]
fn uninstall_all_removes_nothing_without_yes() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.root().join("member1"))?;
    let mut child = sandbox
        .cc_shift()
        .args(["uninstall", "--all"])
        .stdin(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        io::Write::write_all(&mut stdin, b"no\n")?;
    }
    let out = child.wait_with_output()?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).contains("nothing removed"), "{}", stderr(&out));
    assert!(sandbox.root().join("member1").is_dir(), "profile kept");
    Ok(())
}
