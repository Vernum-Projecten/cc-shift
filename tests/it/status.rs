//! `cc-shift status`: every account with its usage, the active one marked.

use std::fs;
use std::io;
use std::process::Output;

use crate::sandbox::Sandbox;

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn marks_the_active_account_and_shows_unknown_ones() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    for name in ["member1", "member2"] {
        fs::create_dir_all(sandbox.root().join(name))?;
    }
    fs::write(
        sandbox.root().join("state.json"),
        r#"{"active": "member1", "usage": {"member1": {
            "five_hour": {"used_percent": 40.0, "resets_at": 4102444800},
            "read_at": 1}}}"#,
    )?;
    let out = sandbox.run(&["status"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(
        text.starts_with("cc-shift · 2 accounts · 2 free now · active member1 · next member2\n"),
        "{text}"
    );
    assert!(text.contains("▶ member1  ████░░░░░░  40%"), "{text}");
    assert!(
        text.contains("member2  not started              not started              next"),
        "{text}"
    );
    assert!(!text.contains("\u{1b}["), "no colour when piped: {text}");
    Ok(())
}

#[test]
fn explains_how_to_create_the_first_profile() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["status"])?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert!(
        stderr(&out).contains("cc-shift add <name>"),
        "{}",
        stderr(&out)
    );
    Ok(())
}

#[test]
fn reports_a_damaged_state_file() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.root().join("member1"))?;
    fs::write(sandbox.root().join("state.json"), "{ damaged")?;
    let out = sandbox.run(&["status"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("is damaged"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn prints_json_with_the_same_overview() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    for name in ["member1", "member2"] {
        fs::create_dir_all(sandbox.root().join(name))?;
    }
    fs::write(
        sandbox.root().join("state.json"),
        r#"{"active": "member1", "usage": {"member1": {
            "five_hour": {"used_percent": 40.0, "resets_at": 4102444800},
            "read_at": 1, "confirmed": true}}}"#,
    )?;
    let out = sandbox.run(&["status", "--json"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(io::Error::other)?;
    assert_eq!(value["active"], "member1");
    assert_eq!(value["next"], "member2");
    assert_eq!(value["accounts"][0]["state"], "active");
    assert_eq!(
        value["accounts"][0]["five_hour"]["resets_at"],
        "2100-01-01T00:00:00Z"
    );
    assert_eq!(value["accounts"][1]["state"], "next");
    Ok(())
}
