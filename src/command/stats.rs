//! `cc-shift stats`: token statistics across all accounts together, per
//! project, model and day.

use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use jiff::Timestamp;
use jiff::tz::TimeZone;

use crate::command::home;
use crate::error::Error;
use crate::render::Paint;
use crate::render::stats::stats;
use crate::render::stats_json::stats_json;
use crate::stats::Options;
use crate::stats::cache::StatsCache;
use crate::stats::summary::{summarize, timeline};
use crate::stats::transcript;
use crate::store::history::History;
use crate::store::profile::ProfilesRoot;

/// Reads the transcripts of `~/.claude` and of every profile, then prints
/// the statistics `options` ask for.
///
/// Days and today are in the system's time zone. Colour is used only on a
/// terminal, and never when `NO_COLOR` is set.
///
/// # Errors
///
/// Returns the error of a missing `HOME`, an unreadable profiles root,
/// transcript or folder, a cache that cannot be written, a history that
/// cannot be read with `--per-account`, or a project with no response in the
/// period.
pub fn run(options: &Options) -> Result<(), Error> {
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let mut folders = vec![home.join(".claude").join("projects")];
    for profile in root.list()? {
        folders.push(root.path().join(profile).join("projects"));
    }
    let mut files: Vec<PathBuf> = Vec::new();
    for folder in &folders {
        let found = transcript::files(folder).map_err(|source| Error::StatsRead {
            path: folder.clone(),
            source,
        })?;
        files.extend(found);
    }
    let responses = StatsCache::in_root(&root).responses(&files)?;
    let changes = if options.per_account {
        timeline(&History::in_root(&root).read(None)?)
    } else {
        Vec::new()
    };
    let summary = summarize(
        responses,
        options,
        &changes,
        Timestamp::now(),
        &TimeZone::system(),
    )?;
    let text = if options.json {
        stats_json(&summary, options.cost, options.per_account)
    } else {
        let paint = Paint::unless_no_color(std::io::stdout().is_terminal());
        stats(&summary, options.cost, paint)
    };
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(text.as_bytes());
    Ok(())
}
