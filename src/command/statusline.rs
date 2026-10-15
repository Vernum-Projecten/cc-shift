//! `cc-shift statusline`: run by Claude Code as its status line after every
//! reply.

use std::io::{Read, Write};

use crate::anthropic::parked;
use crate::claude_code::statusline;
use crate::command::{automatic, home, refresh, unix_now};
use crate::error::Error;
use crate::login::account::AccountFiles;
use crate::render::{self, Paint};
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::switching::auto::Trigger;
use crate::usage::overview::overview;

/// Records usage, switches when needed, and prints the status line.
///
/// The reading on stdin is recorded under the account the window really
/// uses. When it is proven to be the active account's and that account is
/// full, the switch runs. A background refresh starts when numbers are stale.
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
    let reading = statusline::record(&input, &file, now).unwrap_or_default();
    let active = file.read().ok().and_then(|state| state.active);
    if reading.proven && reading.account.is_some() && reading.account == active {
        // NOTE: Claude Code 2.1.300 can write a window's old account details
        // back into ~/.claude.json (#50), so they are put right on each run.
        if let (Some(name), Ok(home)) = (&active, home()) {
            let _restored: Result<(), Error> = AccountFiles::new(&home, &root).activate(name);
        }
        let _switched: Result<Option<String>, Error> = automatic(Trigger::Usage);
    }
    if parked::any_stale(&file, &root, now).unwrap_or(false) {
        refresh::start_in_background();
    }
    let mut line = reading
        .chained
        .map(|command| statusline::chained(&command, &input))
        .unwrap_or_default();
    while line.last().is_some_and(u8::is_ascii_whitespace) {
        line.pop();
    }
    if let (Ok(state), Ok(profiles)) = (file.read(), root.list()) {
        if !line.is_empty() {
            line.extend_from_slice(" │ ".as_bytes());
        }
        let view = overview(&profiles, &state, now);
        let text = render::line::line(
            &view,
            reading.account.as_deref(),
            now,
            Paint::unless_no_color(true),
        );
        line.extend_from_slice(text.as_bytes());
    }
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(&line);
}
