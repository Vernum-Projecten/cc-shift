//! `cs setup`: installs the status line and the hook, keeping the rest.

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
