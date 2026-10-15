// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! How `cc-shift` shows usage: the `cc-shift status` table, the status line
//! and the `cc-shift stats` tables, with bars and colour.
//!
//! Colour follows the `NO_COLOR` convention (<https://no-color.org>): any
//! non-empty `NO_COLOR` turns it off.

pub mod json;
pub mod line;
pub mod log;
pub mod stats;
pub mod stats_json;
pub mod table;

/// Paints text with ANSI colours, or leaves it plain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paint {
    color: bool,
}

impl Paint {
    /// Returns a painter that colours when `color` is true.
    #[must_use]
    pub fn new(color: bool) -> Self {
        Self { color }
    }

    /// Returns a painter that colours unless `NO_COLOR` is set and not empty.
    #[must_use]
    pub fn unless_no_color(wanted: bool) -> Self {
        let no_color = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        Self::new(wanted && !no_color)
    }

    fn wrap(self, code: &str, text: &str) -> String {
        if self.color {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }

    /// Returns `text` in bold.
    #[must_use]
    pub fn bold(self, text: &str) -> String {
        self.wrap("1", text)
    }

    /// Returns `text` dimmed.
    #[must_use]
    pub fn dim(self, text: &str) -> String {
        self.wrap("2", text)
    }

    /// Returns `text` in cyan, the colour of the logo.
    #[must_use]
    pub fn cyan(self, text: &str) -> String {
        self.wrap("36", text)
    }

    /// Returns `text` in green.
    #[must_use]
    pub fn green(self, text: &str) -> String {
        self.wrap("32", text)
    }

    /// Returns `text` in yellow.
    #[must_use]
    pub fn yellow(self, text: &str) -> String {
        self.wrap("33", text)
    }

    /// Returns `text` in red.
    #[must_use]
    pub fn red(self, text: &str) -> String {
        self.wrap("31", text)
    }

    /// Returns `text` in the colour for a window that is `percent` used:
    /// green below 70, yellow below 90, red from 90.
    #[must_use]
    pub fn level(self, percent: f64, text: &str) -> String {
        if percent < 70.0 {
            self.green(text)
        } else if percent < 90.0 {
            self.yellow(text)
        } else {
            self.red(text)
        }
    }
}

/// The number of cells in a usage bar.
pub const BAR_CELLS: usize = 10;

/// Returns a bar of [`BAR_CELLS`] cells for a window that is `percent` used.
#[must_use]
pub fn bar(percent: f64) -> String {
    let cells = (0..BAR_CELLS)
        .filter(|cell| {
            let edge = f64::from(u32::try_from(*cell).unwrap_or(0)) * 10.0 + 5.0;
            percent >= edge
        })
        .count();
    let mut text = "█".repeat(cells);
    text.push_str(&"░".repeat(BAR_CELLS - cells));
    text
}

/// Pads `text` with spaces to `width` characters, counting characters, not
/// bytes.
#[must_use]
pub fn pad(text: &str, width: usize) -> String {
    let len = text.chars().count();
    let mut out = text.to_owned();
    out.push_str(&" ".repeat(width.saturating_sub(len)));
    out
}

#[cfg(test)]
mod tests {
    use super::{Paint, bar, pad};

    #[test]
    fn a_bar_rounds_to_the_nearest_cell() {
        assert_eq!(bar(0.0), "░░░░░░░░░░");
        assert_eq!(bar(15.0), "██░░░░░░░░");
        assert_eq!(bar(94.0), "█████████░");
        assert_eq!(bar(100.0), "██████████");
    }

    #[test]
    fn plain_paint_adds_nothing() {
        assert_eq!(Paint::new(false).red("full"), "full");
        assert_eq!(Paint::new(true).red("full"), "\u{1b}[31mfull\u{1b}[0m");
    }

    #[test]
    fn padding_counts_characters() {
        assert_eq!(pad("██", 4), "██  ");
    }
}
