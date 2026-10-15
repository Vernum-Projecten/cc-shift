// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift status`: every account with its usage, its reset times and its
//! state, in the order `cc-shift` uses them.

use std::io::{IsTerminal, Write};

use crate::anthropic::parked::Report;
use crate::command::{refresh, report, unix_now};
use crate::error::Error;
use crate::render::{self, Paint, table};
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;
use crate::usage::overview::overview;

/// Refreshes the accounts' numbers, then prints the status table, or the
/// same overview as JSON when `json` is set.
///
/// Colour is used only on a terminal, and never when `NO_COLOR` is set.
///
/// # Errors
///
/// Returns the error of a missing `HOME`, the profiles root or the state
/// file.
pub fn run(json: bool) -> Result<(), Error> {
    let root = ProfilesRoot::from_env()?;
    let profiles = root.list()?;
    if profiles.is_empty() && !json {
        report("no profiles yet; create one with 'cc-shift add <name>'");
        return Ok(());
    }
    // NOTE: no specification governs this: our own design. A refresh that
    // fails leaves the last numbers, which the table still shows.
    let _reports: Result<Vec<Report>, Error> = refresh::refresh_parked();
    let state = StateFile::in_root(&root).read()?;
    let now = unix_now();
    let view = overview(&profiles, &state, now);
    let text = if json {
        render::json::json(&view, now)
    } else {
        let paint = Paint::unless_no_color(std::io::stdout().is_terminal());
        let mut text = table::table(&view, now, paint);
        if let Some(setting) = &state.early_start {
            text.push_str(&table::early_line(*setting, paint));
        }
        text
    };
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(text.as_bytes());
    Ok(())
}
