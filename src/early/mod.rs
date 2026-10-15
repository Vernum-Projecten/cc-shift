//! Early start: starting some accounts' 5-hour windows before the workday.
//!
//! A 5-hour window starts with an account's first message and resets five
//! hours later. A window started at 06:00 resets at 11:00, inside the
//! workday, which gives an extra window that day. Each morning from Monday
//! to Friday a `launchd` agent runs `cc-shift early run`, which sends one
//! short message on a few accounts, picked in the order the selection rule
//! uses them. It is off until `cc-shift early on`.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod start;

use std::path::{Path, PathBuf};

use jiff::Timestamp;
use jiff::civil::{Time, Weekday};
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::usage::select::in_order;
use crate::usage::{Account, Usage};

/// The most accounts an early start can be set to start.
pub const MOST_ACCOUNTS: u8 = 20;

/// When the early start runs and how many accounts it starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EarlyStart {
    /// The hour of the run, local time, from 0 to 23.
    pub hour: u8,
    /// The minute of the run, from 0 to 59.
    pub minute: u8,
    /// How many accounts to start, from 1 to [`MOST_ACCOUNTS`].
    pub accounts: u8,
}

impl Default for EarlyStart {
    fn default() -> Self {
        Self {
            hour: 6,
            minute: 0,
            accounts: 2,
        }
    }
}

impl EarlyStart {
    /// Returns the time of the run as `HH:MM`.
    #[must_use]
    pub fn at(self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// Returns the count as it reads: `1 account`, `2 accounts`.
    #[must_use]
    pub fn count(self) -> String {
        if self.accounts == 1 {
            "1 account".to_owned()
        } else {
            format!("{} accounts", self.accounts)
        }
    }
}

/// Installs or replaces the system job that runs `binary early run` at the
/// set time on workdays, with `path` as its `PATH`.
///
/// # Errors
///
/// Returns the error of the job's file or of the system scheduler.
pub fn install(home: &Path, setting: EarlyStart, binary: &Path, path: &str) -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    return macos::launchd::install(home, setting, binary, path);
    #[cfg(target_os = "linux")]
    return linux::systemd::install(home, setting, binary, path);
}

/// Removes the system job; returns whether there was one.
///
/// # Errors
///
/// Returns the error of the job's file or of the system scheduler.
pub fn remove(home: &Path) -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    return macos::launchd::remove(home);
    #[cfg(target_os = "linux")]
    return linux::systemd::remove(home);
}

/// Returns the file of the system job: the `launchd` agent on macOS, the
/// systemd timer on Linux.
#[must_use]
pub fn job_path(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    return macos::launchd::agent_path(home);
    #[cfg(target_os = "linux")]
    return linux::systemd::timer_path(home);
}

/// Returns whether the system job is installed.
#[must_use]
pub fn installed(home: &Path) -> bool {
    job_path(home).is_file()
}

/// Returns the empty folder the early start runs `claude` from.
#[must_use]
pub fn workdir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    return home.join("Library/Caches/cc-shift/early-start");
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map_or_else(|| home.join(".cache"), PathBuf::from)
            .join("cc-shift/early-start")
    }
}

/// Parses a time of day written as `HH:MM`, from 00:00 to 23:59.
#[must_use]
pub fn parse_time(text: &str) -> Option<(u8, u8)> {
    let (hour, minute) = text.split_once(':')?;
    if hour.len() != 2 || minute.len() != 2 {
        return None;
    }
    let hour: u8 = hour.parse().ok().filter(|h| *h < 24)?;
    let minute: u8 = minute.parse().ok().filter(|m| *m < 60)?;
    Some((hour, minute))
}

/// Returns whether a day is a workday, Monday to Friday.
#[must_use]
pub fn is_workday(day: Weekday) -> bool {
    !matches!(day, Weekday::Saturday | Weekday::Sunday)
}

/// Returns the next run after `now`: the first workday at the set time, in
/// `zone`; `None` when the time cannot be placed in the zone.
#[must_use]
pub fn next_run(setting: EarlyStart, now: Timestamp, zone: &TimeZone) -> Option<Timestamp> {
    let time = Time::new(
        i8::try_from(setting.hour).ok()?,
        i8::try_from(setting.minute).ok()?,
        0,
        0,
    )
    .ok()?;
    let today = now.to_zoned(zone.clone()).date();
    (0..8_i64).find_map(|days| {
        let date = today.checked_add(jiff::Span::new().days(days)).ok()?;
        let run = date.to_datetime(time).to_zoned(zone.clone()).ok()?;
        (is_workday(date.weekday()) && run.timestamp() > now).then(|| run.timestamp())
    })
}

/// How long after the set time a run still starts windows, in seconds: a
/// window started later would reset in the evening.
pub const LATEST_SECONDS: i64 = 3 * 3_600;

/// Returns whether `now` is more than [`LATEST_SECONDS`] after today's set
/// time in `zone`, as when a sleeping or switched-off machine catches up.
#[must_use]
pub fn too_late(setting: EarlyStart, now: Timestamp, zone: &TimeZone) -> bool {
    let Ok(time) = Time::new(
        i8::try_from(setting.hour).unwrap_or(0),
        i8::try_from(setting.minute).unwrap_or(0),
        0,
        0,
    ) else {
        return false;
    };
    let today = now.to_zoned(zone.clone()).date();
    today
        .to_datetime(time)
        .to_zoned(zone.clone())
        .is_ok_and(|set| now.as_second() - set.timestamp().as_second() > LATEST_SECONDS)
}

/// Returns the accounts to start: the first `count` in the selection
/// order whose 5-hour window is not running at `now`.
///
/// Accounts that are full, out of week or logged out are never in the
/// selection order, so they are never started.
#[must_use]
pub fn pick(accounts: &[Account], count: u8, now: i64) -> Vec<&Account> {
    in_order(accounts, now)
        .into_iter()
        .filter(|a| !window_running(a, now))
        .take(usize::from(count))
        .collect()
}

/// Returns whether the account's 5-hour window is running at `now`.
fn window_running(account: &Account, now: i64) -> bool {
    matches!(
        &account.usage,
        Usage::Known {
            five_hour: Some(window),
            ..
        } if window.resets_at > now
    )
}

#[cfg(test)]
mod tests {
    use super::{EarlyStart, next_run, parse_time, pick};
    use crate::usage::{Account, Thresholds, Usage, Window};
    use jiff::Timestamp;
    use jiff::tz::TimeZone;

    /// Thursday 2026-10-15 12:00 UTC.
    const NOW: i64 = 1_792_065_600;

    fn account(name: &str, five: Option<Window>, seven_used: f64) -> Account {
        Account {
            name: name.to_owned(),
            logged_in: true,
            usage: Usage::Known {
                five_hour: five,
                seven_day: Some(Window {
                    used_percent: seven_used,
                    resets_at: NOW + 3 * 86_400,
                }),
            },
            thresholds: Thresholds::default(),
        }
    }

    fn names(picked: &[&Account]) -> Vec<String> {
        picked.iter().map(|a| a.name.clone()).collect()
    }

    #[test]
    fn times_are_two_digit_hours_and_minutes() {
        assert_eq!(parse_time("06:00"), Some((6, 0)));
        assert_eq!(parse_time("23:59"), Some((23, 59)));
        for bad in ["6:00", "24:00", "06:60", "0600", "06:0", "aa:bb", ""] {
            assert_eq!(parse_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn the_default_is_two_accounts_at_six() {
        let setting = EarlyStart::default();
        assert_eq!((setting.at(), setting.accounts), ("06:00".to_owned(), 2));
    }

    #[test]
    fn the_next_run_skips_the_weekend() {
        let setting = EarlyStart::default();
        let at = |s: i64| Timestamp::from_second(s).unwrap();
        let friday_six = 1_792_130_400;
        assert_eq!(
            next_run(setting, at(NOW), &TimeZone::UTC),
            Some(at(friday_six)),
            "Thursday noon runs next on Friday"
        );
        let monday_six = friday_six + 3 * 86_400;
        assert_eq!(
            next_run(setting, at(friday_six), &TimeZone::UTC),
            Some(at(monday_six)),
            "Friday's run is followed by Monday's"
        );
    }

    #[test]
    fn a_run_more_than_three_hours_late_is_too_late() {
        let setting = EarlyStart::default();
        let at = |s: i64| Timestamp::from_second(s).unwrap();
        let six = NOW - 6 * 3_600;
        assert!(!super::too_late(
            setting,
            at(six + 3 * 3_600),
            &TimeZone::UTC
        ));
        assert!(super::too_late(
            setting,
            at(six + 3 * 3_600 + 1),
            &TimeZone::UTC
        ));
        assert!(
            !super::too_late(setting, at(six - 3_600), &TimeZone::UTC),
            "early is fine"
        );
    }

    #[test]
    fn it_starts_the_first_accounts_in_the_selection_order() {
        let accounts = [
            account("extra", None, 40.0),
            account("spare", None, 10.0),
            account("work", None, 20.0),
        ];
        assert_eq!(names(&pick(&accounts, 2, NOW)), ["spare", "work"]);
    }

    #[test]
    fn it_skips_full_logged_out_and_running_accounts() {
        let running = Window {
            used_percent: 10.0,
            resets_at: NOW + 3_600,
        };
        let mut logged_out = account("gone", None, 0.0);
        logged_out.logged_in = false;
        let accounts = [
            account("busy", Some(running), 0.0),
            account("full", None, 100.0),
            logged_out,
            account("spare", None, 50.0),
        ];
        assert_eq!(names(&pick(&accounts, 3, NOW)), ["spare"]);
    }

    #[test]
    fn a_window_that_has_reset_counts_as_not_running() {
        let ended = Window {
            used_percent: 80.0,
            resets_at: NOW - 60,
        };
        let accounts = [account("work", Some(ended), 10.0)];
        assert_eq!(names(&pick(&accounts, 1, NOW)), ["work"]);
    }
}
