//! What `cs` knows about each account's usage.
//!
//! Claude Code reports two windows per account, the 5-hour and the 7-day
//! window, each as a percentage used and a reset time
//! (<https://code.claude.com/docs/en/statusline>).

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
}
