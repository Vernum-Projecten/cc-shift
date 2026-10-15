// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift log`: the history of switches, refreshes and full accounts.

use std::io::{IsTerminal, Write};

use jiff::tz::TimeZone;

use crate::command::unix_now;
use crate::error::Error;
use crate::render::{Paint, log};
use crate::store::history::History;
use crate::store::profile::ProfilesRoot;

/// The period and form of `cc-shift log`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// How far back, in seconds; `None` for the whole history.
    pub since: Option<i64>,
    /// How the period reads ("24 hours", "7 days", "all time").
    pub label: String,
    /// Whether to show the diagnostic events too.
    pub verbose: bool,
    /// Whether to print JSON lines instead of the screen.
    pub json: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            since: Some(24 * 3600),
            label: "24 hours".to_owned(),
            verbose: false,
            json: false,
        }
    }
}

/// Parses the arguments after `log`, or returns the misuse.
///
/// # Errors
///
/// Returns the message for an unknown flag or a duration that does not
/// parse.
pub fn options(flags: &[&str]) -> Result<Options, String> {
    let mut options = Options::default();
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        match *flag {
            "--verbose" => options.verbose = true,
            "--json" => options.json = true,
            "--all" => {
                options.since = None;
                "all time".clone_into(&mut options.label);
            }
            "--since" => {
                let value = rest
                    .next()
                    .ok_or("--since needs a duration such as 2h or 7d")?;
                let (seconds, label) = duration(value)
                    .ok_or_else(|| format!("'{value}' is not a duration such as 2h, 7d or 30d"))?;
                options.since = Some(seconds);
                options.label = label;
            }
            other => return Err(format!("log does not know '{other}'")),
        }
    }
    Ok(options)
}

/// Parses `90m`, `2h` or `7d` into seconds and a label.
fn duration(text: &str) -> Option<(i64, String)> {
    let split = text.len().checked_sub(1)?;
    let (number, unit) = (text.get(..split)?, text.get(split..)?);
    let count: i64 = number.parse().ok().filter(|n| *n > 0)?;
    let (seconds, word) = match unit {
        "m" => (60, "minute"),
        "h" => (3600, "hour"),
        "d" => (86_400, "day"),
        _ => return None,
    };
    let plural = if count == 1 { "" } else { "s" };
    Some((
        count.checked_mul(seconds)?,
        format!("{count} {word}{plural}"),
    ))
}

/// Prints the history for the chosen period.
///
/// # Errors
///
/// Returns the error of a missing `HOME` or a history file that cannot be
/// read.
pub fn run(options: &Options) -> Result<(), Error> {
    let root = ProfilesRoot::from_env()?;
    let now = unix_now();
    let entries: Vec<_> = History::in_root(&root)
        .read(options.since.map(|back| now - back))?
        .into_iter()
        .filter(|entry| options.verbose || !entry.event.is_diagnostic())
        .collect();
    let mut out = std::io::stdout().lock();
    if options.json {
        for entry in &entries {
            if let Ok(line) = serde_json::to_string(entry) {
                // NOTE: no specification governs this: our own design. A
                // closed pipe ends the output without an error.
                if writeln!(out, "{line}").is_err() {
                    break;
                }
            }
        }
        return Ok(());
    }
    let paint = Paint::unless_no_color(std::io::stdout().is_terminal());
    let text = log::log(&entries, &options.label, now, &TimeZone::system(), paint);
    let _ignored: std::io::Result<()> = out.write_all(text.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Options, options};

    #[test]
    fn the_default_is_the_last_24_hours() {
        assert_eq!(options(&[]), Ok(Options::default()));
    }

    #[test]
    fn since_takes_minutes_hours_and_days() {
        let week = options(&["--since", "7d"]);
        assert_eq!(
            week.map(|o| (o.since, o.label)),
            Ok((Some(604_800), "7 days".to_owned()))
        );
        let hour = options(&["--since", "1h", "--verbose", "--json"]);
        assert_eq!(
            hour.map(|o| (o.since, o.label, o.verbose, o.json)),
            Ok((Some(3600), "1 hour".to_owned(), true, true))
        );
    }

    #[test]
    fn all_reads_the_whole_history() {
        assert_eq!(options(&["--all"]).map(|o| o.since), Ok(None));
    }

    #[test]
    fn a_bad_duration_or_flag_is_refused() {
        assert!(options(&["--since", "7w"]).is_err(), "unknown unit");
        assert!(options(&["--since"]).is_err(), "missing value");
        assert!(options(&["--yesterday"]).is_err(), "unknown flag");
    }
}
