//! Claude Code's session transcripts: where they are and what `cc-shift`
//! reads from them.
//!
//! Claude Code keeps each session as a JSONL file under
//! `<config>/projects/<project>/`, with the transcripts of subagents in
//! folders below it. Of every line `cc-shift` reads only assistant responses,
//! and of those only the message id, the model, the usage counts, the time,
//! the session and the working folder; the message content is skipped
//! unread.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::stats::{Response, Tokens};

/// One transcript line, with only the fields statistics read.
#[derive(Debug, Deserialize)]
struct Line {
    #[serde(rename = "type")]
    kind: Option<String>,
    message: Option<Message>,
    timestamp: Option<String>,
    cwd: Option<String>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

/// The message of an assistant line, without its content.
#[derive(Debug, Deserialize)]
struct Message {
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

/// The usage counts of a response.
#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_creation_input_tokens: Option<u64>,
    cache_read_input_tokens: Option<u64>,
    cache_creation: Option<CacheCreation>,
}

/// The split of the cache writes by lifetime.
#[derive(Debug, Deserialize)]
struct CacheCreation {
    ephemeral_1h_input_tokens: Option<u64>,
}

/// Returns the responses in the text of one transcript, in file order.
///
/// A line that is not JSON, not an assistant response, or lacks the id,
/// the model, the time or one of the four counts is skipped: it is not a
/// usage record. A response repeated on several lines appears once, as the
/// copy [`Response::replaces`] picks, at the place of its first line.
#[must_use]
pub fn responses(text: &str) -> Vec<Response> {
    let mut found: Vec<Response> = Vec::new();
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for response in text.lines().filter_map(response) {
        if let Some(kept) = seen.get(&response.id).and_then(|i| found.get_mut(*i)) {
            if response.replaces(kept) {
                *kept = response;
            }
        } else {
            seen.insert(response.id.clone(), found.len());
            found.push(response);
        }
    }
    found
}

/// Returns the response on one line, if the line holds one.
fn response(line: &str) -> Option<Response> {
    // NOTE: no specification governs this: our own design. Every assistant
    // line names its type as the JSON string "assistant", so other lines skip
    // the full parse.
    if !line.contains("\"assistant\"") {
        return None;
    }
    // NOTE: no specification governs this: our own design. A line that does
    // not parse (one being written) carries no usage record, so it is absent.
    let parsed: Line = serde_json::from_str(line).ok()?;
    if parsed.kind.as_deref() != Some("assistant") {
        return None;
    }
    let message = parsed.message?;
    let usage = message.usage?;
    let cache_write = usage.cache_creation_input_tokens?;
    let tokens = Tokens {
        input: usage.input_tokens?,
        output: usage.output_tokens?,
        cache_write,
        cache_write_1h: usage
            .cache_creation
            .and_then(|c| c.ephemeral_1h_input_tokens)
            .unwrap_or(0)
            .min(cache_write),
        cache_read: usage.cache_read_input_tokens?,
    };
    // NOTE: no specification governs this: our own design. A response with
    // no tokens of any kind used nothing, so it is not a usage record.
    if tokens.prompt() == 0 && tokens.output == 0 {
        return None;
    }
    let at = parsed.timestamp?.parse::<jiff::Timestamp>().ok()?;
    Some(Response {
        id: message.id?,
        model: message.model?,
        session: parsed.session_id,
        cwd: parsed.cwd,
        at: at.as_second(),
        tokens,
    })
}

/// Returns every `.jsonl` file below `projects`, sorted by path.
///
/// Subfolders are searched too, so subagent transcripts are included.
/// Symbolic links are not followed. A missing folder holds no files.
///
/// # Errors
///
/// Returns the I/O error of a folder that exists but cannot be read.
pub fn files(projects: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let mut folders = vec![projects.to_path_buf()];
    while let Some(folder) = folders.pop() {
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let entry = entry?;
            let kind = entry.file_type()?;
            let path = entry.path();
            if kind.is_dir() {
                folders.push(path);
            } else if !kind.is_symlink() && path.extension().is_some_and(|e| e == "jsonl") {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{files, responses};
    use std::fs;

    const LINE: &str = r#"{"type":"assistant","timestamp":"2026-10-15T10:00:00.000Z","cwd":"/work/shop","sessionId":"s1","message":{"id":"msg_1","model":"claude-opus-5-5","content":[{"type":"text","text":"never read"}],"usage":{"input_tokens":10,"output_tokens":20,"cache_creation_input_tokens":30,"cache_read_input_tokens":40,"cache_creation":{"ephemeral_1h_input_tokens":25,"ephemeral_5m_input_tokens":5}}}}"#;

    #[test]
    fn reads_the_usage_of_an_assistant_line() {
        let found = responses(LINE);
        assert_eq!(found.len(), 1, "{found:?}");
        let r = &found[0];
        assert_eq!(r.id, "msg_1");
        assert_eq!(r.model, "claude-opus-5-5");
        assert_eq!(r.session.as_deref(), Some("s1"));
        assert_eq!(r.cwd.as_deref(), Some("/work/shop"));
        assert_eq!(r.at, 1_792_058_400);
        assert_eq!(
            (
                r.tokens.input,
                r.tokens.output,
                r.tokens.cache_write,
                r.tokens.cache_write_1h,
                r.tokens.cache_read
            ),
            (10, 20, 30, 25, 40)
        );
    }

    #[test]
    fn skips_other_lines() {
        let text = [
            r#"{"type":"user","message":{"role":"user","content":"assistant"}}"#,
            "not json \"assistant\"",
            r#"{"type":"assistant","timestamp":"2026-10-15T10:00:00Z","message":{"id":"m","model":"x"}}"#,
            r#"{"type":"assistant","message":{"id":"m","model":"x","usage":{"input_tokens":1,"output_tokens":1,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}}"#,
            "",
        ]
        .join("\n");
        assert_eq!(responses(&text), Vec::new());
    }

    #[test]
    fn a_response_on_several_lines_appears_once_with_the_most_output() {
        let more = LINE.replace(r#""output_tokens":20"#, r#""output_tokens":90"#);
        let other = LINE.replace("msg_1", "msg_2");
        let text = [LINE.to_owned(), more, LINE.to_owned(), other].join("\n");
        let found = responses(&text);
        let ids: Vec<(&str, u64)> = found
            .iter()
            .map(|r| (r.id.as_str(), r.tokens.output))
            .collect();
        assert_eq!(ids, [("msg_1", 90), ("msg_2", 20)]);
    }

    #[test]
    fn a_response_without_tokens_is_skipped() {
        let empty = LINE
            .replace(r#""input_tokens":10"#, r#""input_tokens":0"#)
            .replace(r#""output_tokens":20"#, r#""output_tokens":0"#)
            .replace(
                r#""cache_creation_input_tokens":30"#,
                r#""cache_creation_input_tokens":0"#,
            )
            .replace(
                r#""cache_read_input_tokens":40"#,
                r#""cache_read_input_tokens":0"#,
            );
        assert_eq!(responses(&empty), Vec::new());
    }

    #[test]
    fn a_missing_split_counts_every_write_as_five_minutes() {
        let line = LINE.replace(
            r#","cache_creation":{"ephemeral_1h_input_tokens":25,"ephemeral_5m_input_tokens":5}"#,
            "",
        );
        assert_eq!(responses(&line)[0].tokens.cache_write_1h, 0);
    }

    #[test]
    fn finds_sessions_and_subagent_transcripts() -> std::io::Result<()> {
        let dir = tempfile::TempDir::new()?;
        let project = dir.path().join("-work-shop");
        fs::create_dir_all(project.join("s1").join("subagents"))?;
        fs::write(project.join("s1.jsonl"), "")?;
        fs::write(project.join("s1").join("subagents").join("a.jsonl"), "")?;
        fs::write(project.join("notes.txt"), "")?;
        let found = files(dir.path())?;
        assert_eq!(
            found,
            vec![
                project.join("s1").join("subagents").join("a.jsonl"),
                project.join("s1.jsonl"),
            ]
        );
        assert_eq!(
            files(&dir.path().join("missing"))?,
            Vec::<std::path::PathBuf>::new()
        );
        Ok(())
    }
}
