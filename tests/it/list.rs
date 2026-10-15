//! `cs list`: prints the profiles, sorted.

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
fn prints_profiles_sorted_one_per_line() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    for name in ["primary", "member2", "member1"] {
        fs::create_dir_all(sandbox.root().join(name))?;
    }
    let out = sandbox.run(&["list"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out), "member1\nmember2\nprimary\n");
    Ok(())
}

#[test]
fn skips_files_in_the_root() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.root().join("member1"))?;
    fs::write(sandbox.root().join(".DS_Store"), "")?;
    let out = sandbox.run(&["list"])?;
    assert_eq!(stdout(&out), "member1\n");
    Ok(())
}

#[test]
fn explains_how_to_create_the_first_profile() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::create_dir_all(sandbox.root())?;
    let out = sandbox.run(&["list"])?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert_eq!(stdout(&out), "", "nothing listed");
    assert!(stderr(&out).contains("cs add <name>"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn treats_a_missing_root_as_no_profiles() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.run(&["list"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(stderr(&out).contains("no profiles yet"), "{}", stderr(&out));
    assert!(!sandbox.root().exists(), "list creates nothing");
    Ok(())
}

#[test]
fn fails_when_the_root_is_not_a_folder() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    fs::write(sandbox.root(), "a file where the root folder belongs")?;
    let out = sandbox.run(&["list"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("cannot read"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn fails_without_home() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = sandbox.cs().env_remove("HOME").arg("list").output()?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(stderr(&out).contains("HOME is not set"), "{}", stderr(&out));
    Ok(())
}
