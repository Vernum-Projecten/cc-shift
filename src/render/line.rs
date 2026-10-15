// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The `cc-shift` part of the Claude Code status line.

use crate::render::{Paint, bar};
use crate::usage::Thresholds;
use crate::usage::overview::{Overview, Row};
use crate::usage::time_left;

/// Renders the status line for the account the window really uses
/// (`window`, else the active account), for example
/// `work ▕██░░░░░░░░▏ 15% · 4h29m │ 7d 66% │ next spare · 2/7 free`.
///
/// Right after a switch, when the window still reports the account just
/// left, it adds `moving to` the active account. When no other account is
/// free, it names the one that frees up first.
#[must_use]
pub fn line(view: &Overview, window: Option<&str>, now: i64, paint: Paint) -> String {
    let Some(active) = view.active.as_deref() else {
        return paint.yellow("cc-shift: run cc-shift setup");
    };
    let shown = window.unwrap_or(active);
    let row = view.rows.iter().find(|r| r.name == shown);
    let mut parts = Vec::new();

    let five = match row.and_then(|r| r.five_hour) {
        Some(w) => format!(
            "{} {} · {}",
            paint.level(w.used_percent, &format!("▕{}▏", bar(w.used_percent))),
            paint.level(w.used_percent, &format!("{:.0}%", w.used_percent)),
            time_left(w.resets_at - now)
        ),
        None => paint.dim("5h not started"),
    };
    parts.push(format!("{} {five}", paint.bold(shown)));
    if let Some(w) = row.and_then(|r| r.seven_day) {
        parts.push(format!(
            "7d {}",
            paint.level(w.used_percent, &format!("{:.0}%", w.used_percent))
        ));
    }
    if shown != active {
        parts.push(paint.yellow(&format!("moving to {active}")));
    } else if let Some(next) = &view.next {
        parts.push(format!(
            "next {next} · {}/{} free",
            view.free,
            view.rows.len()
        ));
    } else if shown_full(row, view.thresholds, now) {
        match &view.first_free {
            Some((first, at)) => parts.push(paint.red(&format!(
                "all full · {first} free in {}",
                time_left(at - now)
            ))),
            None => parts.push(paint.red("all full")),
        }
    }
    parts.join(" │ ")
}

/// Returns whether either window of the shown account is full, by its own
/// threshold.
fn shown_full(row: Option<&Row>, limits: Thresholds, now: i64) -> bool {
    row.is_some_and(|r| {
        [
            (r.five_hour, limits.five_hour),
            (r.seven_day, limits.seven_day),
        ]
        .into_iter()
        .any(|(w, limit)| w.is_some_and(|w| w.used_at(now) >= limit))
    })
}

#[cfg(test)]
mod tests {
    use super::line;
    use crate::render::Paint;
    use crate::usage::Thresholds;
    use crate::usage::Window;
    use crate::usage::overview::{Overview, Row, Status};

    const NOW: i64 = 1_000_000;

    fn view(next: Option<&str>, free: usize) -> Overview {
        Overview {
            rows: vec![
                Row {
                    name: "work".to_owned(),
                    status: Status::Active,
                    five_hour: Some(Window {
                        used_percent: 15.0,
                        resets_at: NOW + 16_140,
                    }),
                    seven_day: Some(Window {
                        used_percent: 66.0,
                        resets_at: NOW + 352_800,
                    }),
                    confirmed: true,
                },
                Row {
                    name: "spare".to_owned(),
                    status: Status::Full(NOW + 540),
                    five_hour: None,
                    seven_day: None,
                    confirmed: true,
                },
            ],
            active: Some("work".to_owned()),
            next: next.map(str::to_owned),
            free,
            first_free: Some(("spare".to_owned(), NOW + 540)),
            thresholds: Thresholds::default(),
        }
    }

    #[test]
    fn shows_the_account_its_windows_and_the_next() {
        assert_eq!(
            line(&view(Some("spare"), 2), None, NOW, Paint::new(false)),
            "work ▕██░░░░░░░░▏ 15% · 4h29m │ 7d 66% │ next spare · 2/2 free"
        );
    }

    #[test]
    fn says_when_the_window_is_still_on_the_previous_account() {
        assert_eq!(
            line(&view(None, 1), Some("spare"), NOW, Paint::new(false)),
            "spare 5h not started │ moving to work"
        );
    }

    #[test]
    fn names_the_first_to_free_up_when_all_are_full() {
        let mut full = view(None, 0);
        if let Some(work) = full.rows.first_mut() {
            work.five_hour = Some(Window {
                used_percent: 97.0,
                resets_at: NOW + 16_140,
            });
        }
        assert_eq!(
            line(&full, None, NOW, Paint::new(false)),
            "work ▕██████████▏ 97% · 4h29m │ 7d 66% │ all full · spare free in 9m"
        );
    }

    #[test]
    fn the_only_account_with_room_shows_no_next() {
        assert_eq!(
            line(&view(None, 1), None, NOW, Paint::new(false)),
            "work ▕██░░░░░░░░▏ 15% · 4h29m │ 7d 66%"
        );
    }

    #[test]
    fn before_setup_it_says_so() {
        let mut empty = view(None, 0);
        empty.active = None;
        assert_eq!(
            line(&empty, None, NOW, Paint::new(false)),
            "cc-shift: run cc-shift setup"
        );
    }
}
