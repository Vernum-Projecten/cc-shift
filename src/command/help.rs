// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift help` and `cc-shift --version`.

use std::io::{IsTerminal, Write};

use crate::command::parse::USAGE;
use crate::render::Paint;

/// The running version, from the crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The logo: "cc-shift" in figlet's standard font.
const LOGO: [&str; 5] = [
    "                   _     _  __ _",
    "  ___ ___      ___| |__ (_)/ _| |_",
    " / __/ __|____/ __| '_ \\| | |_| __|",
    "| (_| (_|_____\\__ \\ | | | |  _| |_",
    " \\___\\___|    |___/_| |_|_|_|  \\__|",
];

/// Returns the version line, `cc-shift <version>`.
#[must_use]
pub fn version_line() -> String {
    format!("cc-shift {VERSION}\n")
}

/// Returns the help text: the logo with the version and tagline under it
/// when `logo` is set, then the usage.
#[must_use]
pub fn help_text(logo: bool, paint: Paint) -> String {
    if !logo {
        return USAGE.to_owned();
    }
    let mut out = String::new();
    for line in LOGO {
        out.push_str(&paint.cyan(line));
        out.push('\n');
    }
    out.push_str(&paint.dim(&format!(
        "{VERSION} · switches Claude Code accounts on its own"
    )));
    out.push('\n');
    out.push('\n');
    out.push_str(USAGE);
    out
}

/// Prints the help: with the logo on a terminal, coloured unless `NO_COLOR`
/// is set; plain usage text when piped.
pub fn run() {
    let terminal = std::io::stdout().is_terminal();
    let text = help_text(terminal, Paint::unless_no_color(terminal));
    // NOTE: no specification governs this: our own design. A failed write to
    // stdout (a closed pipe) is not an error for help text.
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(text.as_bytes());
}

/// Prints the version line.
pub fn version() {
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(version_line().as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{LOGO, VERSION, help_text, version_line};
    use crate::command::parse::USAGE;
    use crate::render::Paint;

    #[test]
    fn the_version_line_names_the_crate_version() {
        assert_eq!(version_line(), format!("cc-shift {VERSION}\n"));
    }

    #[test]
    fn piped_help_is_the_plain_usage() {
        assert_eq!(help_text(false, Paint::new(false)), USAGE);
    }

    #[test]
    fn terminal_help_opens_with_the_logo_and_the_version_under_it() {
        let text = help_text(true, Paint::new(false));
        let expected = format!(
            "{}\n{VERSION} · switches Claude Code accounts on its own\n\n{USAGE}",
            LOGO.join("\n")
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn no_help_line_is_wider_than_the_usage() {
        let text = help_text(true, Paint::new(false));
        let usage = USAGE.lines().map(|l| l.chars().count()).max().unwrap();
        for line in text.lines() {
            assert!(line.chars().count() <= usage, "{line}");
        }
    }
}
