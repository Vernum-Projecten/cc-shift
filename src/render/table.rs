//! The `cc-shift status` table.

use std::fmt::Write;

use crate::render::{Paint, bar, pad};
use crate::usage::overview::{Overview, Row, Status};
use crate::usage::{Window, time_left};

/// The width of one window cell: bar, percentage and time to reset.
const CELL: usize = 23;

/// Renders the status table: a summary line, then one row per account in
/// the order `cc-shift` uses them.
#[must_use]
pub fn table(view: &Overview, now: i64, paint: Paint) -> String {
    let width = view
        .rows
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0)
        .max("ACCOUNT".len());
    let mut out = summary(view, paint);
    out.push_str("\n\n");
    out.push_str(&paint.dim(&format!(
        "  {}  {}  {}  STATE",
        pad("ACCOUNT", width),
        pad("5-HOUR", CELL),
        pad("7-DAY", CELL)
    )));
    out.push('\n');
    for row in &view.rows {
        let marker = if row.status == Status::Active {
            "▶"
        } else {
            " "
        };
        let name = pad(&row.name, width);
        let name = if row.status == Status::Active {
            paint.bold(&name)
        } else {
            name
        };
        // NOTE: no specification governs this: our own design. Writing to a
        // `String` cannot fail.
        let _infallible: std::fmt::Result = writeln!(
            out,
            "{marker} {name}  {}  {}  {}",
            cell(row.five_hour, now, paint),
            cell(row.seven_day, now, paint),
            state(row, now, paint)
        );
    }
    out
}

/// The line above the table.
fn summary(view: &Overview, paint: Paint) -> String {
    let total = view.rows.len();
    let mut parts = vec![
        paint.bold("cc-shift"),
        format!("{total} account{}", if total == 1 { "" } else { "s" }),
        format!("{} free now", view.free),
    ];
    match &view.active {
        Some(active) => parts.push(format!("active {}", paint.bold(active))),
        None => parts.push(paint.yellow("not set up: run cc-shift setup")),
    }
    if let Some(next) = &view.next {
        parts.push(format!("next {next}"));
    }
    parts.join(" · ")
}

/// One window: bar, percentage and time to reset, or "not started".
fn cell(window: Option<Window>, now: i64, paint: Paint) -> String {
    let Some(w) = window else {
        return paint.dim(&pad("not started", CELL));
    };
    let plain_bar = bar(w.used_percent);
    let percent = format!("{:>3.0}%", w.used_percent);
    let time = pad(&time_left(w.resets_at - now), 6);
    format!(
        "{} {}  {time}",
        paint.level(w.used_percent, &plain_bar),
        paint.level(w.used_percent, &percent)
    )
}

/// The state column.
fn state(row: &Row, now: i64, paint: Paint) -> String {
    match &row.status {
        Status::Active => paint.green("active"),
        Status::Next => paint.bold("next"),
        Status::Ready => paint.dim("ready"),
        Status::Full(at) => paint.red(&format!("full · free in {}", time_left(at - now))),
        Status::WeekFull(at) => paint.red(&format!("week full · free in {}", time_left(at - now))),
        Status::LoggedOut => paint.red(&format!("logged out · run cc-shift add {}", row.name)),
        Status::Unknown(reason) => paint.yellow(&format!("unknown · {reason}")),
    }
}

#[cfg(test)]
mod tests {
    use super::table;
    use crate::render::Paint;
    use crate::usage::Thresholds;
    use crate::usage::Window;
    use crate::usage::overview::{Overview, Row, Status};

    const NOW: i64 = 1_000_000;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "fixtures pass the result straight into an optional window"
    )]
    fn window(used: f64, resets_in: i64) -> Option<Window> {
        Some(Window {
            used_percent: used,
            resets_at: NOW + resets_in,
        })
    }

    fn row(name: &str, status: Status, five: Option<Window>, seven: Option<Window>) -> Row {
        Row {
            name: name.to_owned(),
            status,
            five_hour: five,
            seven_day: seven,
            confirmed: true,
        }
    }

    #[test]
    fn renders_the_summary_and_every_state() {
        let view = Overview {
            rows: vec![
                row(
                    "work",
                    Status::Active,
                    window(15.0, 16_140),
                    window(66.0, 352_800),
                ),
                row(
                    "spare",
                    Status::Next,
                    window(10.0, 10_740),
                    window(64.0, 302_400),
                ),
                row(
                    "third",
                    Status::Full(NOW + 540),
                    window(100.0, 540),
                    window(49.0, 324_000),
                ),
                row(
                    "fourth",
                    Status::WeekFull(NOW + 59_340),
                    None,
                    window(100.0, 59_340),
                ),
                row("fifth", Status::LoggedOut, None, None),
            ],
            active: Some("work".to_owned()),
            next: Some("spare".to_owned()),
            free: 2,
            first_free: Some(("third".to_owned(), NOW + 540)),
            thresholds: Thresholds::default(),
        };
        assert_eq!(
            table(&view, NOW, Paint::new(false)),
            "cc-shift · 5 accounts · 2 free now · active work · next spare\n\
             \n\
             \x20 ACCOUNT  5-HOUR                   7-DAY                    STATE\n\
             ▶ work     ██░░░░░░░░  15%  4h29m   ███████░░░  66%  4d2h    active\n\
             \x20 spare    █░░░░░░░░░  10%  2h59m   ██████░░░░  64%  3d12h   next\n\
             \x20 third    ██████████ 100%  9m      █████░░░░░  49%  3d18h   full · free in 9m\n\
             \x20 fourth   not started              ██████████ 100%  16h29m  week full · free in 16h29m\n\
             \x20 fifth    not started              not started              logged out · run cc-shift add fifth\n"
        );
    }

    #[test]
    fn before_setup_it_says_so() {
        let view = Overview {
            rows: Vec::new(),
            active: None,
            next: None,
            free: 0,
            first_free: None,
            thresholds: Thresholds::default(),
        };
        assert!(
            table(&view, NOW, Paint::new(false))
                .starts_with("cc-shift · 0 accounts · 0 free now · not set up"),
            "{}",
            table(&view, NOW, Paint::new(false))
        );
    }
}
