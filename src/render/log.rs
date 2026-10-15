// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The `cc-shift log` screen: the history, oldest first, with a summary.

use std::fmt::Write;

use jiff::tz::TimeZone;

use crate::render::{Paint, pad};
use crate::store::history::{Entry, Event, ReadingOutcome, Reason, WindowKind};
use crate::usage::time_left;

/// Renders `entries` (already filtered to the period) as the log screen.
///
/// `label` names the period ("24 hours", "7 days", "all time"); times are
/// shown in `zone`, and `now` decides what "today" is.
#[must_use]
pub fn log(entries: &[Entry], label: &str, now: i64, zone: &TimeZone, paint: Paint) -> String {
    let mut out = format!(
        "{} · {}\n\n",
        paint.bold("cc-shift history"),
        heading(label)
    );
    if entries.is_empty() {
        out.push_str(&paint.dim("  nothing happened in this period"));
        out.push('\n');
    } else {
        out.push_str(&paint.dim("  TIME          EVENT        DETAILS"));
        out.push('\n');
        for entry in entries {
            let at = entry.seconds().unwrap_or(now);
            let (name, details) = describe(&entry.event, at, zone, paint);
            // NOTE: no specification governs this: our own design. Writing to
            // a `String` cannot fail.
            let _infallible: std::fmt::Result = writeln!(
                out,
                "  {}  {}  {details}",
                pad(&clock(at, now, zone), 12),
                pad(name, 11)
            );
        }
    }
    out.push('\n');
    out.push_str(&summary(entries, label, now));
    out.push('\n');
    out
}

/// The period as it reads in the heading.
fn heading(label: &str) -> String {
    if label == "all time" {
        label.to_owned()
    } else {
        format!("last {label}")
    }
}

/// Formats a time: `today 11:47`, or `Tue 13 16:40` for another day.
fn clock(at: i64, now: i64, zone: &TimeZone) -> String {
    let zoned = |seconds: i64| {
        jiff::Timestamp::from_second(seconds)
            .ok()
            .map(|t| t.to_zoned(zone.clone()))
    };
    match (zoned(at), zoned(now)) {
        (Some(at), Some(now)) if at.date() == now.date() => {
            format!("today {}", at.strftime("%H:%M"))
        }
        (Some(at), _) => {
            format!(
                "{} {} {}",
                at.strftime("%a"),
                at.day(),
                at.strftime("%H:%M")
            )
        }
        (None, _) => at.to_string(),
    }
}

/// The event column and the coloured details of one event.
fn describe(event: &Event, at: i64, zone: &TimeZone, paint: Paint) -> (&'static str, String) {
    match event {
        Event::Setup { active } => (
            "setup",
            match active {
                Some(name) => format!("{name} active · status line and hook installed"),
                None => "status line and hook installed".to_owned(),
            },
        ),
        Event::Switch {
            reason,
            window,
            from,
            to,
            from_used,
            to_used,
        } => {
            let route = match from {
                Some(from) => format!("{from} → {to}"),
                None => to.clone(),
            };
            match reason {
                Reason::Threshold => {
                    let which = match window {
                        Some(WindowKind::SevenDay) => "week",
                        _ => "5h",
                    };
                    let mut text = format!("{route} · {which} at {:.0}%", from_used.unwrap_or(0.0));
                    if let Some(used) = to_used {
                        let _infallible: std::fmt::Result =
                            write!(text, " · {to} had {used:.0}% used");
                    }
                    ("switch", paint.green(&text))
                }
                Reason::LimitHit => (
                    "switch",
                    paint.yellow(&format!("{route} · limit hit (hook)")),
                ),
                Reason::ByHand => ("use", format!("{route} · by hand")),
                Reason::Setup => ("switch", format!("{route} · setup")),
            }
        }
        Event::Refresh { account } => {
            ("refresh", paint.dim(&format!("{account}: login refreshed")))
        }
        Event::LoggedOut { account } => (
            "logged out",
            paint.red(&format!(
                "{account}: refresh refused · run cc-shift add {account}"
            )),
        ),
        Event::AllFull {
            staying_on,
            first_free,
            free_at,
        } => {
            let mut text = format!("staying on {staying_on}");
            if let (Some(first), Some(free_at)) = (first_free, free_at) {
                let _infallible: std::fmt::Result =
                    write!(text, " · {first} frees up in {}", time_left(free_at - at));
            }
            ("all full", paint.red(&text))
        }
        Event::FreeAgain {
            account,
            after_seconds,
        } => (
            "free again",
            paint.green(&format!(
                "{account} is free again after {}",
                time_left(*after_seconds)
            )),
        ),
        Event::EarlyStart { started, wanted } => (
            "early start",
            early_start(started, *wanted, at, zone, paint),
        ),
        Event::EarlyStartSkipped { reason } => {
            ("early start", paint.yellow(&format!("skipped · {reason}")))
        }
        Event::EarlyStartFailed { account, reason } => (
            "early start",
            paint.red(&format!("{account}: not started · {reason}")),
        ),
        Event::Reading { account, outcome } => (
            "reading",
            paint.dim(&match (outcome, account) {
                (ReadingOutcome::Elsewhere, Some(name)) => {
                    format!("recorded under {name} (matched its reset times)")
                }
                _ => "dropped a reading that matched no account".to_owned(),
            }),
        ),
    }
}

/// The details of an early start: the accounts started and when their
/// windows reset, five hours later.
fn early_start(started: &[String], wanted: u8, at: i64, zone: &TimeZone, paint: Paint) -> String {
    if started.is_empty() {
        return paint.yellow("no account was free to start");
    }
    let resets = jiff::Timestamp::from_second(at + 5 * 3_600).map_or_else(
        |_| "?".to_owned(),
        |t| t.to_zoned(zone.clone()).strftime("%H:%M").to_string(),
    );
    let mut text = format!("{} · windows reset at {resets}", started.join(", "));
    if started.len() < usize::from(wanted) {
        let _infallible: std::fmt::Result =
            write!(text, " · {} of {wanted} started", started.len());
    }
    paint.green(&text)
}

/// The closing line: switches, and how often and long all accounts were
/// full.
fn summary(entries: &[Entry], label: &str, now: i64) -> String {
    let switches = entries
        .iter()
        .filter(|e| matches!(e.event, Event::Switch { .. }))
        .count();
    let mut full_count = 0_usize;
    let mut full_seconds = 0_i64;
    let mut open: Option<i64> = None;
    for entry in entries {
        match entry.event {
            Event::AllFull { .. } => {
                full_count += 1;
                open = entry.seconds();
            }
            Event::FreeAgain { after_seconds, .. } => {
                full_seconds += after_seconds;
                open = None;
            }
            _ => {}
        }
    }
    if let Some(since) = open {
        full_seconds += now - since;
    }
    let mut line = format!(
        "  {label} · {switches} switch{}",
        if switches == 1 { "" } else { "es" }
    );
    if full_count > 0 {
        let _infallible: std::fmt::Result = write!(
            line,
            " · all accounts full {full_count}× for {}",
            time_left(full_seconds)
        );
    }
    line
}

#[cfg(test)]
mod tests {
    use super::log;
    use crate::render::Paint;
    use crate::store::history::{Entry, Event, Reason, WindowKind};
    use jiff::tz::TimeZone;

    /// 2026-10-15 18:30 UTC, a Thursday.
    const NOW: i64 = 1_792_089_000;
    const HOUR: i64 = 3600;

    fn at(event: Event, hours_ago: i64) -> Entry {
        Entry::new(event, NOW - hours_ago * HOUR)
    }

    #[test]
    fn renders_the_approved_screen() {
        let entries = vec![
            at(
                Event::Setup {
                    active: Some("work".to_owned()),
                },
                9,
            ),
            at(
                Event::Switch {
                    reason: Reason::Threshold,
                    window: Some(WindowKind::FiveHour),
                    from: Some("work".to_owned()),
                    to: "spare".to_owned(),
                    from_used: Some(95.0),
                    to_used: Some(10.0),
                },
                6,
            ),
            at(
                Event::Switch {
                    reason: Reason::LimitHit,
                    window: None,
                    from: Some("spare".to_owned()),
                    to: "extra".to_owned(),
                    from_used: None,
                    to_used: None,
                },
                4,
            ),
            at(
                Event::LoggedOut {
                    account: "old".to_owned(),
                },
                3,
            ),
            at(
                Event::AllFull {
                    staying_on: "extra".to_owned(),
                    first_free: Some("work".to_owned()),
                    free_at: Some(NOW - 2 * HOUR + 4200),
                },
                2,
            ),
            Entry::new(
                Event::FreeAgain {
                    account: "work".to_owned(),
                    after_seconds: 4200,
                },
                NOW - 2 * HOUR + 4200,
            ),
            at(
                Event::Switch {
                    reason: Reason::ByHand,
                    window: None,
                    from: Some("work".to_owned()),
                    to: "spare".to_owned(),
                    from_used: None,
                    to_used: None,
                },
                0,
            ),
        ];
        assert_eq!(
            log(&entries, "24 hours", NOW, &TimeZone::UTC, Paint::new(false)),
            "cc-shift history · last 24 hours\n\
             \n\
             \x20 TIME          EVENT        DETAILS\n\
             \x20 today 09:30   setup        work active · status line and hook installed\n\
             \x20 today 12:30   switch       work → spare · 5h at 95% · spare had 10% used\n\
             \x20 today 14:30   switch       spare → extra · limit hit (hook)\n\
             \x20 today 15:30   logged out   old: refresh refused · run cc-shift add old\n\
             \x20 today 16:30   all full     staying on extra · work frees up in 1h10m\n\
             \x20 today 17:40   free again   work is free again after 1h10m\n\
             \x20 today 18:30   use          work → spare · by hand\n\
             \n\
             \x20 24 hours · 3 switches · all accounts full 1× for 1h10m\n"
        );
    }

    #[test]
    fn an_older_day_shows_its_weekday_and_date() {
        let entries = vec![at(
            Event::Refresh {
                account: "extra".to_owned(),
            },
            50,
        )];
        let text = log(&entries, "7 days", NOW, &TimeZone::UTC, Paint::new(false));
        assert!(text.contains("  Tue 13 16:30  refresh"), "{text}");
        assert!(text.ends_with("  7 days · 0 switches\n"), "{text}");
    }

    #[test]
    fn an_empty_period_says_so() {
        let text = log(&[], "2 hours", NOW, &TimeZone::UTC, Paint::new(false));
        assert!(text.contains("nothing happened in this period"), "{text}");
    }

    #[test]
    fn an_early_start_names_the_accounts_and_when_they_reset() {
        let six = NOW - 12 * HOUR - 30 * 60;
        let entries = vec![
            Entry::new(
                Event::EarlyStartFailed {
                    account: "extra".to_owned(),
                    reason: "claude exited with exit status: 1".to_owned(),
                },
                six,
            ),
            Entry::new(
                Event::EarlyStart {
                    started: vec!["work".to_owned()],
                    wanted: 2,
                },
                six,
            ),
            Entry::new(
                Event::EarlyStart {
                    started: Vec::new(),
                    wanted: 2,
                },
                six + HOUR,
            ),
        ];
        let text = log(&entries, "24 hours", NOW, &TimeZone::UTC, Paint::new(false));
        for line in [
            "  today 06:00   early start  extra: not started · claude exited with exit status: 1\n",
            "  today 06:00   early start  work · windows reset at 11:00 · 1 of 2 started\n",
            "  today 07:00   early start  no account was free to start\n",
        ] {
            assert!(text.contains(line), "{line}\n{text}");
        }
    }
}
