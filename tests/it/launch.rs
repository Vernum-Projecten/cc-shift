//! `cs <name> [args...]`: starts Claude Code with an existing profile.

use std::fs;
use std::io;

use crate::sandbox::Sandbox;

fn stderr(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn with_profile(name: &str) -> io::Result<Sandbox> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.root().join(name))?;
    Ok(sandbox)
}

#[test]
fn starts_claude_with_the_profile_folder() -> io::Result<()> {
    let sandbox = with_profile("member1")?;
    let out = sandbox.run(&["member1"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let dir = sandbox.root().join("member1");
    assert_eq!(
        sandbox.claude_call(),
        Some(format!("CLAUDE_CONFIG_DIR={}\n", dir.display()))
    );
    Ok(())
}

#[test]
fn passes_arguments_through_unchanged_and_in_order() -> io::Result<()> {
    let sandbox = with_profile("member1")?;
    let out = sandbox.run(&["member1", "--continue", "-p", "two words", "help"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let dir = sandbox.root().join("member1");
    assert_eq!(
        sandbox.claude_call(),
        Some(format!(
            "CLAUDE_CONFIG_DIR={}\narg=--continue\narg=-p\narg=two words\narg=help\n",
            dir.display()
        ))
    );
    Ok(())
}

#[test]
fn names_cs_add_for_a_missing_profile() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["claude9"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("cs add claude9"), "{}", stderr(&out));
    assert_eq!(sandbox.claude_call(), None, "claude not started");
    Ok(())
}

#[test]
fn refuses_an_invalid_name() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["a.b"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(
        stderr(&out).contains("invalid profile name"),
        "{}",
        stderr(&out)
    );
    Ok(())
}

#[test]
fn fails_without_home() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.cs().env_remove("HOME").arg("member1").output()?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("HOME is not set"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn fails_when_claude_is_not_on_path() -> io::Result<()> {
    let sandbox = Sandbox::without_claude()?;
    fs::create_dir_all(sandbox.root().join("member1"))?;
    let out = sandbox.run(&["member1"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(
        stderr(&out).contains("claude was not found"),
        "{}",
        stderr(&out)
    );
    Ok(())
}
