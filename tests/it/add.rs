//! `cs add <name>`: creates the profile folder and starts Claude Code in it.

use std::fs;
use std::io;

use crate::sandbox::Sandbox;

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn creates_folder_and_starts_claude_in_it() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["add", "claude1"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let dir = sandbox.root().join("claude1");
    assert!(dir.is_dir(), "profile folder exists");
    assert_eq!(
        sandbox.claude_call(),
        Some(format!("CLAUDE_CONFIG_DIR={}\n", dir.display()))
    );
    Ok(())
}

#[test]
fn reminds_to_run_login() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["add", "claude1"])?;
    assert!(stderr(&out).contains("/login"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn reuses_an_existing_profile() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let dir = sandbox.root().join("claude1");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("settings.json"), "{}")?;
    let out = sandbox.run(&["add", "claude1"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        dir.join("settings.json").is_file(),
        "existing files are kept"
    );
    assert!(sandbox.claude_call().is_some(), "claude started");
    Ok(())
}

#[test]
fn refuses_an_invalid_name() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["add", "a/b"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(
        stderr(&out).contains("invalid profile name"),
        "{}",
        stderr(&out)
    );
    assert!(!sandbox.root().exists(), "nothing created");
    assert_eq!(sandbox.claude_call(), None, "claude not started");
    Ok(())
}

#[test]
fn refuses_a_command_word() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["add", "list"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("cs command"), "{}", stderr(&out));
    assert_eq!(sandbox.claude_call(), None, "claude not started");
    Ok(())
}

#[test]
fn needs_a_name() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["add"])?;
    assert_eq!(out.status.code(), Some(2), "exit code");
    assert!(stderr(&out).contains("Usage: cs"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn fails_without_home() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox
        .cs()
        .env_remove("HOME")
        .args(["add", "claude1"])
        .output()?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("HOME is not set"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn fails_when_the_folder_cannot_be_created() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::write(sandbox.root(), "a file where the root folder belongs")?;
    let out = sandbox.run(&["add", "claude1"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("cannot create"), "{}", stderr(&out));
    assert_eq!(sandbox.claude_call(), None, "claude not started");
    Ok(())
}

#[test]
fn fails_when_claude_is_not_on_path() -> io::Result<()> {
    let sandbox = Sandbox::without_claude()?;
    let out = sandbox.run(&["add", "claude1"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(
        stderr(&out).contains("claude was not found"),
        "{}",
        stderr(&out)
    );
    Ok(())
}
