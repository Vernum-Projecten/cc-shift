//! Runs the built `cc-shift` binary and checks its exit codes and output.

use std::process::Command;

fn cc_shift() -> Command {
    Command::new(env!("CARGO_BIN_EXE_cc-shift"))
}

#[test]
fn help_prints_usage_and_succeeds() -> std::io::Result<()> {
    let out = cc_shift().arg("help").output()?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    assert!(
        String::from_utf8_lossy(&out.stdout).starts_with("Usage: cc-shift"),
        "stdout starts with the usage line"
    );
    Ok(())
}

#[test]
fn unknown_option_prints_usage_to_stderr_and_exits_2() -> std::io::Result<()> {
    let out = cc_shift().arg("--frobnicate").output()?;
    assert_eq!(out.status.code(), Some(2), "exit code");
    assert!(out.stdout.is_empty(), "nothing on stdout");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unknown option '--frobnicate'"),
        "stderr names the word: {stderr}"
    );
    assert!(
        stderr.contains("Usage: cc-shift"),
        "stderr carries the usage"
    );
    Ok(())
}
