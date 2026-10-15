// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift log`: the history, as a screen and as JSON lines.

use std::fs;
use std::io;
use std::process::Output;

use crate::sandbox::Sandbox;

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn with_history(sandbox: &Sandbox) -> io::Result<()> {
    fs::create_dir_all(sandbox.root())?;
    fs::write(
        sandbox.root().join("history.jsonl"),
        "{\"at\":\"2026-01-01T09:00:00Z\",\"event\":\"refresh\",\"account\":\"spare\"}\n\
         {\"at\":\"2026-01-01T10:00:00Z\",\"event\":\"reading\",\"account\":null,\"outcome\":\"dropped\"}\n\
         not json\n",
    )
}

#[test]
fn shows_the_whole_history_without_diagnostics() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    with_history(&sandbox)?;
    let out = sandbox
        .cc_shift()
        .env("TZ", "UTC")
        .args(["log", "--all"])
        .output()?;
    assert_eq!(out.status.code(), Some(0), "exit code");
    let text = stdout(&out);
    assert!(text.starts_with("cc-shift history · all time\n"), "{text}");
    assert!(
        text.contains("refresh      spare: login refreshed"),
        "{text}"
    );
    assert!(!text.contains("reading"), "diagnostics hidden: {text}");
    assert!(text.ends_with("  all time · 0 switches\n"), "{text}");
    Ok(())
}

#[test]
fn verbose_json_prints_every_event_as_a_line() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    with_history(&sandbox)?;
    let out = sandbox.run(&["log", "--all", "--verbose", "--json"])?;
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 2, "{text}");
    assert!(text.contains("\"event\":\"reading\""), "{text}");
    Ok(())
}

#[test]
fn a_bad_flag_is_misuse() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    assert_eq!(
        sandbox.run(&["log", "--since", "7w"])?.status.code(),
        Some(2)
    );
    Ok(())
}
