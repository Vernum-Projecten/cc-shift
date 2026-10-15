// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift stats`: token totals from synthetic transcripts in a fake home.

use std::fs;
use std::io;
use std::path::Path;
use std::process::Output;

use crate::sandbox::Sandbox;

/// One assistant line with invented ids and counts, and content that must
/// never be read or stored.
fn line(id: &str, session: &str, cwd: &str, model: &str, output: u64) -> String {
    format!(
        r#"{{"type":"assistant","timestamp":"2026-10-15T10:00:00Z","cwd":"{cwd}","sessionId":"{session}","message":{{"id":"{id}","model":"{model}","content":[{{"type":"text","text":"SECRET-CONTENT"}}],"usage":{{"input_tokens":100,"output_tokens":{output},"cache_creation_input_tokens":1000,"cache_read_input_tokens":10000,"cache_creation":{{"ephemeral_1h_input_tokens":1000,"ephemeral_5m_input_tokens":0}}}}}}}}"#
    )
}

fn write(path: &Path, lines: &[String]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, lines.join("\n") + "\n")
}

/// Sessions in `~/.claude` and in a profile, a subagent transcript, and a
/// resumed session that repeats a response.
fn transcripts(sandbox: &Sandbox) -> io::Result<()> {
    let main = sandbox.home().join(".claude/projects/-w-shop");
    write(
        &main.join("s1.jsonl"),
        &[
            line("m1", "s1", "/w/client/shop", "claude-opus-5-5", 2000),
            line("m2", "s1", "/w/client/shop", "claude-opus-5-5", 3000),
            r#"{"type":"user","message":{"role":"user","content":"hello"}}"#.to_owned(),
        ],
    )?;
    write(
        &main.join("s1/subagents/agent-a.jsonl"),
        &[line(
            "m3",
            "s1",
            "/w/client/shop",
            "claude-sonnet-5-5",
            1000,
        )],
    )?;
    write(
        &main.join("s2.jsonl"),
        &[
            line("m2", "s2", "/w/client/shop", "claude-opus-5-5", 3000),
            line("m4", "s2", "/w/client/shop", "claude-opus-5-5", 500),
        ],
    )?;
    write(
        &sandbox.root().join("work/projects/-w-server-shop/s3.jsonl"),
        &[line("m5", "s3", "/w/server/shop", "local-model", 4000)],
    )
}

fn stats(sandbox: &Sandbox, args: &[&str]) -> io::Result<Output> {
    sandbox
        .cc_shift()
        .env("TZ", "UTC")
        .arg("stats")
        .args(args)
        .output()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn counts_every_account_once_per_response() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    transcripts(&sandbox)?;
    let out = stats(&sandbox, &["--all", "--json"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let value: serde_json::Value = serde_json::from_slice(&out.stdout).map_err(io::Error::other)?;
    assert_eq!(value["responses"], 5, "{value}");
    assert_eq!(value["sessions"], 3, "{value}");
    assert_eq!(value["total"]["output"], 10_500, "{value}");
    assert_eq!(value["total"]["cache_read"], 50_000, "{value}");
    assert_eq!(value["projects"][0]["name"], "client/shop", "{value}");
    assert_eq!(value["projects"][0]["output"], 6500, "{value}");
    assert_eq!(value["projects"][1]["name"], "server/shop", "{value}");
    assert_eq!(value["models"][0]["model"], "claude-opus-5-5", "{value}");
    assert_eq!(value["days"][0]["date"], "2026-10-15", "{value}");
    assert!(value["total"].get("cost_usd").is_none(), "{value}");
    Ok(())
}

#[test]
fn cost_names_the_model_without_a_price() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    transcripts(&sandbox)?;
    let out = stats(&sandbox, &["--all", "--cost"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(
        text.starts_with(
            "cc-shift stats · all time · all accounts together · 3 sessions · 5 responses\n"
        ),
        "{text}"
    );
    assert!(text.contains("≈ API COST"), "{text}");
    assert!(
        text.contains("  ? = no price known for local-model\n"),
        "{text}"
    );
    assert!(text.contains("\n  DAY "), "{text}");
    assert!(!text.contains("\u{1b}["), "no colour when piped: {text}");
    Ok(())
}

#[test]
fn the_cache_holds_no_content() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    transcripts(&sandbox)?;
    let out = stats(&sandbox, &["--all"])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let cache = fs::read_to_string(sandbox.root().join("stats-cache.json"))?;
    assert!(cache.contains("\"m5\""), "{cache}");
    assert!(!cache.contains("SECRET-CONTENT"), "{cache}");
    let again = stats(&sandbox, &["--all"])?;
    assert_eq!(again.stdout, out.stdout, "the cached run shows the same");
    Ok(())
}

#[test]
fn an_unknown_project_is_an_error() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    transcripts(&sandbox)?;
    let out = stats(&sandbox, &["--all", "--project", "nope"])?;
    assert_eq!(out.status.code(), Some(1), "exit code");
    assert!(
        stderr(&out).contains("no project 'nope'"),
        "{}",
        stderr(&out)
    );
    Ok(())
}

#[test]
fn a_bad_period_is_a_usage_error() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = stats(&sandbox, &["--since", "week"])?;
    assert_eq!(out.status.code(), Some(2), "exit code");
    assert!(stderr(&out).contains("--since takes"), "{}", stderr(&out));
    Ok(())
}

#[test]
fn no_transcripts_is_not_an_error() -> io::Result<()> {
    let sandbox = Sandbox::new()?;
    let out = stats(&sandbox, &[])?;
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(
        String::from_utf8_lossy(&out.stdout).ends_with("\n\n  no responses today\n"),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    Ok(())
}
