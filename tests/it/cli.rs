//! Runs the built `cs` binary and checks its exit codes and output.

use std::process::Command;

fn cs() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cs"))
}

#[test]
fn help_prints_usage_and_succeeds() -> std::io::Result<()> {
    let out = cs().arg("help").output()?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("Usage: cs"),
        "stdout starts with the usage line"
    );
    Ok(())
}

#[test]
fn unknown_option_prints_usage_to_stderr_and_exits_2() -> std::io::Result<()> {
    let out = cs().arg("--frobnicate").output()?;
    assert_eq!(out.status.code(), Some(2), "exit code");
    assert!(out.stdout.is_empty(), "nothing on stdout");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown option '--frobnicate'"),
        "stderr names the word: {stderr}"
    );
    assert!(stderr.contains("Usage: cs"), "stderr carries the usage");
    Ok(())
}
