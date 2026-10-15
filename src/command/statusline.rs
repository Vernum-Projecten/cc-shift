//! `cc-shift statusline`: run by Claude Code as its status line after every
//! reply.

use std::io::{Read, Write};

use crate::anthropic::parked;
use crate::claude_code::statusline;
use crate::command::{automatic, refresh, unix_now};
use crate::error::Error;
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::switching::auto::Trigger;

/// Records usage from the status line input on stdin, switches when the
/// active account is full, starts a background refresh when parked numbers
/// are stale, and prints the status line.
///
/// It never fails: a `cc-shift` problem must not break the status line of a
/// Claude Code window.
pub fn run() {
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        return;
    }
    let now = unix_now();
    let Ok(root) = ProfilesRoot::from_env() else {
        return;
    };
    let file = StateFile::in_root(&root);
    let chained = statusline::record(&input, &file, now).ok().flatten();
    let _switched: Result<Option<String>, Error> = automatic(Trigger::Usage);
    if parked::any_stale(&file, &root, now).unwrap_or(false) {
        refresh::start_in_background();
    }
    let mut line = chained
        .map(|command| statusline::chained(&command, &input))
        .unwrap_or_default();
    while line.last().is_some_and(u8::is_ascii_whitespace) {
        line.pop();
    }
    if let Ok(state) = file.read() {
        if !line.is_empty() {
            line.extend_from_slice(" │ ".as_bytes());
        }
        line.extend_from_slice(statusline::summary(&state, now).as_bytes());
    }
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(&line);
}
