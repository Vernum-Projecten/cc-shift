// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Token statistics across all accounts, for `cc-shift stats`.
//!
//! Claude Code writes a transcript per session under
//! `<config>/projects/<project>/`. `transcript` finds those files in
//! `~/.claude` and in every profile and reads only the usage fields of each
//! response, `cache` keeps what was read per file, `summary` counts the
//! responses of a period per project, model and day, and `price` weighs the
//! counts by Anthropic's public API prices.

pub mod cache;
pub mod price;
pub mod summary;
pub mod transcript;

use serde::{Deserialize, Serialize};

/// The token counts of one response, or of many added together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Tokens {
    /// Input tokens that were neither written to nor read from the cache.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Input tokens written to the cache, both lifetimes together.
    pub cache_write: u64,
    /// The part of [`Tokens::cache_write`] written with the 1-hour lifetime.
    pub cache_write_1h: u64,
    /// Input tokens read from the cache.
    pub cache_read: u64,
}

impl Tokens {
    /// Adds `other` to these counts.
    pub fn add(&mut self, other: &Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
        self.cache_write_1h = self.cache_write_1h.saturating_add(other.cache_write_1h);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
    }

    /// Returns the length of the prompt: every input token, cached or not.
    #[must_use]
    pub fn prompt(&self) -> u64 {
        self.input
            .saturating_add(self.cache_write)
            .saturating_add(self.cache_read)
    }
}

/// One response from a transcript, with only the fields statistics need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    /// The message id, which identifies the response across files.
    pub id: String,
    /// The model that wrote the response.
    pub model: String,
    /// The session the response belongs to, when the line names one.
    pub session: Option<String>,
    /// The working folder of the session, when the line names one.
    pub cwd: Option<String>,
    /// When the response was written, in Unix seconds.
    pub at: i64,
    /// The token counts.
    pub tokens: Tokens,
}

impl Response {
    /// Returns whether this copy of a response counts instead of `kept`, an
    /// earlier copy with the same id.
    ///
    /// Claude Code can write one response on several lines and repeat it in
    /// a resumed session; the copy with the most output tokens counts, and
    /// the earlier one on a tie.
    #[must_use]
    pub fn replaces(&self, kept: &Self) -> bool {
        self.tokens.output > kept.tokens.output
    }
}

/// The time span `cc-shift stats` counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    /// Since the start of today, in local time.
    Today,
    /// The last given number of hours.
    Hours(u32),
    /// Today and the days before it, the given number of days in all.
    Days(u32),
    /// Everything in the transcripts.
    All,
}

impl Period {
    /// Returns the words the summary line uses for the period.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Today | Self::Days(1) => "today".to_owned(),
            Self::Hours(1) => "last hour".to_owned(),
            Self::Hours(n) => format!("last {n} hours"),
            Self::Days(n) => format!("last {n} days"),
            Self::All => "all time".to_owned(),
        }
    }

    /// Returns whether the output has a section per day.
    #[must_use]
    pub fn has_days(self) -> bool {
        matches!(self, Self::Days(_) | Self::All)
    }
}

/// What `cc-shift stats` was asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The time span to count.
    pub period: Period,
    /// The one project to show, by its shown name.
    pub project: Option<String>,
    /// Whether to show the cost at API prices.
    pub cost: bool,
    /// Whether to print JSON.
    pub json: bool,
    /// Whether to add the split per account.
    pub per_account: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            period: Period::Today,
            project: None,
            cost: false,
            json: false,
            per_account: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Period, Tokens};

    #[test]
    fn tokens_add_up() {
        let mut total = Tokens::default();
        let one = Tokens {
            input: 1,
            output: 2,
            cache_write: 3,
            cache_write_1h: 1,
            cache_read: 4,
        };
        total.add(&one);
        total.add(&one);
        assert_eq!(total.output, 4, "output");
        assert_eq!(total.cache_write_1h, 2, "1-hour writes");
        assert_eq!(total.prompt(), 16, "prompt");
    }

    #[test]
    fn periods_have_labels() {
        assert_eq!(Period::Today.label(), "today");
        assert_eq!(Period::Hours(2).label(), "last 2 hours");
        assert_eq!(Period::Days(7).label(), "last 7 days");
        assert_eq!(Period::All.label(), "all time");
        assert!(Period::Days(7).has_days(), "days");
        assert!(!Period::Hours(2).has_days(), "hours");
    }
}
