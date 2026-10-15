// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! What `cc-shift` knows about each account's usage.
//!
//! Claude Code reports two windows per account, the 5-hour and the 7-day
//! window, each as a percentage used and a reset time
//! (<https://code.claude.com/docs/en/statusline>).

pub mod overview;
pub mod select;

use crate::store::state::{Recorded, State};

/// One usage window: how much of it is used, and when it resets.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Window {
    /// The percentage used, from 0 to 100.
    pub used_percent: f64,
    /// When the window resets, in Unix seconds.
    pub resets_at: i64,
}

impl Window {
    /// Returns the percentage used at `now`; a window that has reset is 0%.
    #[must_use]
    pub fn used_at(&self, now: i64) -> f64 {
        if self.resets_at <= now {
            0.0
        } else {
            self.used_percent
        }
    }
}

/// The usage of one account, as last read.
#[derive(Debug, Clone, PartialEq)]
pub enum Usage {
    /// Both windows as last read; a window Claude Code did not report is
    /// `None` and counts as unused.
    Known {
        /// The 5-hour window.
        five_hour: Option<Window>,
        /// The 7-day window.
        seven_day: Option<Window>,
    },
    /// No source could be read, with the reason shown to the user.
    Unknown(String),
}

impl Usage {
    /// Returns the window with the higher percentage used at `now`, if any
    /// window is reported.
    #[must_use]
    pub fn fuller_window(&self, now: i64) -> Option<Window> {
        let Self::Known {
            five_hour,
            seven_day,
        } = self
        else {
            return None;
        };
        match (five_hour, seven_day) {
            (Some(a), Some(b)) if b.used_at(now) > a.used_at(now) => Some(*b),
            (Some(a), _) => Some(*a),
            (None, b) => *b,
        }
    }

    /// Returns the highest percentage used at `now` across both windows, or
    /// `None` when the usage is unknown.
    #[must_use]
    pub fn used_at(&self, now: i64) -> Option<f64> {
        match self {
            Self::Unknown(_) => None,
            Self::Known { .. } => Some(self.fuller_window(now).map_or(0.0, |w| w.used_at(now))),
        }
    }
}

/// One account as the selection rule sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    /// The profile name.
    pub name: String,
    /// Whether the account has a working login.
    pub logged_in: bool,
    /// The account's usage as last read.
    pub usage: Usage,
    /// The percentages at which its windows count as full.
    pub thresholds: Thresholds,
}

impl Account {
    /// Returns whether either window is full at `now`, by its own threshold;
    /// `None` when the usage is unknown.
    #[must_use]
    pub fn full_at(&self, now: i64) -> Option<bool> {
        let Usage::Known {
            five_hour,
            seven_day,
        } = &self.usage
        else {
            return None;
        };
        let over = |w: &Option<Window>, limit: f64| w.is_some_and(|w| w.used_at(now) >= limit);
        Some(
            over(five_hour, self.thresholds.five_hour)
                || over(seven_day, self.thresholds.seven_day),
        )
    }
}

/// The percentages at which a window counts as full and `cc-shift` switches
/// away: 95% of the 5-hour window and 99% of the 7-day window by default.
///
/// The week switches later because its last percent is worth more than a
/// 5-hour window, and the rate-limit hook switches at once if a turn hits the
/// limit anyway. Both are set in `state.json` under `thresholds`.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Thresholds {
    /// The 5-hour window, in percent.
    pub five_hour: f64,
    /// The 7-day window, in percent.
    pub seven_day: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            five_hour: 95.0,
            seven_day: 99.0,
        }
    }
}

impl Thresholds {
    /// The lowest and highest threshold that may be set.
    pub const RANGE: std::ops::RangeInclusive<f64> = 50.0..=100.0;

    /// Returns the thresholds with a value outside [`Thresholds::RANGE`]
    /// replaced by its default.
    #[must_use]
    pub fn checked(self) -> Self {
        let default = Self::default();
        let pick = |value: f64, fallback: f64| {
            if Self::RANGE.contains(&value) {
                value
            } else {
                fallback
            }
        };
        Self {
            five_hour: pick(self.five_hour, default.five_hour),
            seven_day: pick(self.seven_day, default.seven_day),
        }
    }
}

/// Builds the selection view of every profile from the recorded state.
///
/// A profile with no usage read yet is unknown, and a profile whose refresh
/// was refused is logged out.
#[must_use]
pub fn accounts(profiles: &[String], state: &State) -> Vec<Account> {
    profiles
        .iter()
        .map(|name| Account {
            name: name.clone(),
            logged_in: !state.logged_out.contains(name),
            usage: state
                .usage
                .get(name)
                .map_or_else(|| Usage::Unknown("no usage read yet".to_owned()), usage_of),
            thresholds: state.thresholds.checked(),
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

/// Formats a number of seconds as the two largest units, for example `2h15m`
/// or `3d4h`.
#[must_use]
pub fn time_left(seconds: i64) -> String {
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
