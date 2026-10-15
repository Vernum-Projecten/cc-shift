//! `cs status`: every account with its usage, its reset times and which
//! account comes next.

use std::fmt::Write;

use crate::select::{self, Choice, SWITCH_THRESHOLD};
use crate::state::{Recorded, State};
use crate::usage::{Account, Usage, Window};

/// Builds the selection view of every profile from the recorded state.
///
/// A profile with no usage read yet is unknown. Every profile counts as
/// logged in until the Keychain check exists.
#[must_use]
pub fn accounts(profiles: &[String], state: &State) -> Vec<Account> {
    profiles
        .iter()
        .map(|name| Account {
            name: name.clone(),
            logged_in: true,
            usage: state
                .usage
                .get(name)
                .map_or_else(|| Usage::Unknown("no usage read yet".to_owned()), usage_of),
        })
        .collect()
}

/// Turns a recorded reading into usage; a reading with no time is unknown.
fn usage_of(recorded: &Recorded) -> Usage {
    if recorded.read_at.is_none() {
        return Usage::Unknown("no usage read yet".to_owned());
    }
    Usage::Known {
        five_hour: recorded.five_hour,
        seven_day: recorded.seven_day,
    }
}

/// Renders the status table at `now`.
#[must_use]
pub fn render(accounts: &[Account], active: Option<&str>, now: i64) -> String {
    let width = accounts
        .iter()
        .map(|a| a.name.len())
        .max()
        .unwrap_or(0)
        .max("ACCOUNT".len());
    let mut out = String::new();
    // NOTE: no specification governs this: our own design. Writing to a
    // `String` cannot fail, so the `fmt::Result` carries nothing.
    let _infallible: std::fmt::Result =
        writeln!(out, "  {:width$}  {:<16}  7-DAY", "ACCOUNT", "5-HOUR");
    for account in accounts {
        let marker = if Some(account.name.as_str()) == active {
            '*'
        } else {
            ' '
        };
        let detail = match (&account.usage, account.logged_in) {
            (_, false) => "logged out; run cs add again".to_owned(),
            (Usage::Unknown(reason), true) => format!("unknown: {reason}"),
            (
                Usage::Known {
                    five_hour,
                    seven_day,
                },
                true,
            ) => format!("{:<16}  {}", cell(*five_hour, now), cell(*seven_day, now)),
        };
        let _infallible: std::fmt::Result = writeln!(
            out,
            "{marker} {:width$}  {}",
            account.name,
            detail.trim_end()
        );
    }
    out.push('\n');
    out.push_str(&next_line(accounts, active, now));
    out
}

/// One window as `23% 2h15m`: the percentage used now and the time to reset.
fn cell(window: Option<Window>, now: i64) -> String {
    let Some(window) = window else {
        return "-".to_owned();
    };
    let used = window.used_at(now);
    let full = if used >= SWITCH_THRESHOLD {
        " full"
    } else {
        ""
    };
    if window.resets_at <= now {
        return "0% reset".to_owned();
    }
    format!("{used:.0}% {}{full}", duration(window.resets_at - now))
}

/// Formats seconds as the two largest units, for example `2h15m` or `3d4h`.
fn duration(seconds: i64) -> String {
    let minutes = seconds.div_euclid(60);
    let (days, hours, mins) = (
        minutes.div_euclid(1440),
        minutes.rem_euclid(1440).div_euclid(60),
        minutes.rem_euclid(60),
    );
    if days > 0 {
        format!("{days}d{hours}h")
    } else if hours > 0 {
        format!("{hours}h{mins}m")
    } else {
        format!("{mins}m")
    }
}

/// The closing line: which account `cs` would switch to next.
fn next_line(accounts: &[Account], active: Option<&str>, now: i64) -> String {
    match select::next(accounts, active, now) {
        Choice::Account(account) => format!("next: {}\n", account.name),
        Choice::NoneAvailable(Some((account, at))) => format!(
            "no account available; {} frees up in {}\n",
            account.name,
            duration(at - now)
        ),
        Choice::NoneAvailable(None) => "no account available\n".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{accounts, duration, render};
    use crate::state::{Recorded, State};
    use crate::usage::{Account, Usage, Window};

    const NOW: i64 = 1_000_000;

    fn known(name: &str, five: f64, five_in: i64, seven: f64, seven_in: i64) -> Account {
        Account {
            name: name.to_owned(),
            logged_in: true,
            usage: Usage::Known {
                five_hour: Some(Window {
                    used_percent: five,
                    resets_at: NOW + five_in,
                }),
                seven_day: Some(Window {
                    used_percent: seven,
                    resets_at: NOW + seven_in,
                }),
            },
        }
    }

    #[test]
    fn renders_every_account_and_the_next_one() {
        let mut out = known("member3", 0.0, 60, 0.0, 60);
        out.logged_in = false;
        let rows = [
            known("member1", 23.4, 8100, 41.0, 273_600),
            known("member2", 97.0, 600, 10.0, 90_000),
            out,
            Account {
                name: "member5".to_owned(),
                logged_in: true,
                usage: Usage::Unknown("no usage read yet".to_owned()),
            },
        ];
        assert_eq!(
            render(&rows, Some("member1"), NOW),
            "  ACCOUNT  5-HOUR            7-DAY\n\
             * member1  23% 2h15m         41% 3d4h\n\
             \x20 member2  97% 10m full      10% 1d1h\n\
             \x20 member3  logged out; run cs add again\n\
             \x20 member5  unknown: no usage read yet\n\
             \n\
             next: member5\n"
        );
    }

    #[test]
    fn says_when_the_first_account_frees_up() {
        let rows = [known("member1", 99.0, 3600, 0.0, 60)];
        assert!(
            render(&rows, None, NOW).ends_with("no account available; member1 frees up in 1h0m\n"),
            "{}",
            render(&rows, None, NOW)
        );
    }

    #[test]
    fn a_reset_window_shows_as_reset() {
        let rows = [known("member1", 80.0, -5, 10.0, 60)];
        assert!(
            render(&rows, None, NOW).contains("0% reset"),
            "{}",
            render(&rows, None, NOW)
        );
    }

    #[test]
    fn durations_use_the_two_largest_units() {
        assert_eq!(duration(59), "0m");
        assert_eq!(duration(8100), "2h15m");
        assert_eq!(duration(273_600), "3d4h");
    }

    #[test]
    fn a_profile_without_a_reading_is_unknown() {
        let mut state = State::default();
        state
            .usage
            .insert("member1".to_owned(), Recorded::default());
        let rows = accounts(&["member1".to_owned(), "member2".to_owned()], &state);
        assert!(rows.iter().all(|a| matches!(a.usage, Usage::Unknown(_))));
    }
}
