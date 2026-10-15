//! `cc-shift status --json`: the overview as JSON, for scripts and widgets.
//!
//! The shape is part of the public interface: a change to it is a changelog
//! entry. Times are RFC 3339 in UTC. No token or account email appears.

use serde::Serialize;

use crate::usage::Window;
use crate::usage::overview::{Overview, Status};

/// The whole document.
#[derive(Debug, Serialize)]
struct Document {
    active: Option<String>,
    next: Option<String>,
    free: usize,
    total: usize,
    first_free: Option<FirstFree>,
    accounts: Vec<Account>,
}

/// The account that frees up first, when none is free.
#[derive(Debug, Serialize)]
struct FirstFree {
    account: String,
    at: String,
}

/// One account.
#[derive(Debug, Serialize)]
struct Account {
    name: String,
    state: &'static str,
    free_at: Option<String>,
    reason: Option<String>,
    five_hour: Option<WindowJson>,
    seven_day: Option<WindowJson>,
    confirmed: bool,
}

/// One usage window.
#[derive(Debug, Serialize)]
struct WindowJson {
    used_percent: f64,
    resets_at: String,
    seconds_left: i64,
}

/// Renders the overview as pretty-printed JSON with a final newline.
#[must_use]
pub fn json(view: &Overview, now: i64) -> String {
    let document = Document {
        active: view.active.clone(),
        next: view.next.clone(),
        free: view.free,
        total: view.rows.len(),
        first_free: view.first_free.as_ref().map(|(account, at)| FirstFree {
            account: account.clone(),
            at: rfc3339(*at),
        }),
        accounts: view
            .rows
            .iter()
            .map(|row| {
                let (state, free_at, reason) = match &row.status {
                    Status::Active => ("active", None, None),
                    Status::Next => ("next", None, None),
                    Status::Ready => ("ready", None, None),
                    Status::Full(at) => ("full", Some(rfc3339(*at)), None),
                    Status::WeekFull(at) => ("week_full", Some(rfc3339(*at)), None),
                    Status::LoggedOut => ("logged_out", None, None),
                    Status::Unknown(why) => ("unknown", None, Some(why.clone())),
                };
                Account {
                    name: row.name.clone(),
                    state,
                    free_at,
                    reason,
                    five_hour: row.five_hour.map(|w| window(w, now)),
                    seven_day: row.seven_day.map(|w| window(w, now)),
                    confirmed: row.confirmed,
                }
            })
            .collect(),
    };
    let mut text = serde_json::to_string_pretty(&document).unwrap_or_else(|_| "{}".to_owned());
    text.push('\n');
    text
}

fn window(w: Window, now: i64) -> WindowJson {
    WindowJson {
        used_percent: w.used_percent,
        resets_at: rfc3339(w.resets_at),
        seconds_left: (w.resets_at - now).max(0),
    }
}

/// Formats Unix seconds as RFC 3339 in UTC.
fn rfc3339(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds).map_or_else(|_| seconds.to_string(), |t| t.to_string())
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::json;
    use crate::usage::Window;
    use crate::usage::overview::{Overview, Row, Status};
    use serde_json::{Value, json};

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn the_overview_as_json() -> Result<(), serde_json::Error> {
        let view = Overview {
            rows: vec![
                Row {
                    name: "work".to_owned(),
                    status: Status::Active,
                    five_hour: Some(Window {
                        used_percent: 23.0,
                        resets_at: NOW + 3600,
                    }),
                    seven_day: None,
                    confirmed: true,
                },
                Row {
                    name: "old".to_owned(),
                    status: Status::WeekFull(NOW + 7200),
                    five_hour: None,
                    seven_day: None,
                    confirmed: false,
                },
            ],
            active: Some("work".to_owned()),
            next: None,
            free: 1,
            first_free: Some(("old".to_owned(), NOW + 7200)),
        };
        let value: Value = serde_json::from_str(&json(&view, NOW))?;
        assert_eq!(
            value,
            json!({
                "active": "work",
                "next": null,
                "free": 1,
                "total": 2,
                "first_free": {"account": "old", "at": "2027-01-15T10:00:00Z"},
                "accounts": [
                    {
                        "name": "work",
                        "state": "active",
                        "free_at": null,
                        "reason": null,
                        "five_hour": {
                            "used_percent": 23.0,
                            "resets_at": "2027-01-15T09:00:00Z",
                            "seconds_left": 3600
                        },
                        "seven_day": null,
                        "confirmed": true
                    },
                    {
                        "name": "old",
                        "state": "week_full",
                        "free_at": "2027-01-15T10:00:00Z",
                        "reason": null,
                        "five_hour": null,
                        "seven_day": null,
                        "confirmed": false
                    }
                ]
            })
        );
        Ok(())
    }
}
