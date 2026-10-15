//! Parsing of the `cs` command line.

use std::ffi::OsString;

/// The usage text printed by `cs help` and after an unknown command.
pub const USAGE: &str = "\
Usage: cs <command>

Commands:
  help    Print this help
";

/// A parsed `cs` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Print the usage text.
    Help,
    /// A first word that names no command.
    Unknown(OsString),
}

/// Parses the arguments that follow the program name.
///
/// No argument, `help`, `-h` and `--help` all ask for help.
#[must_use]
pub fn parse<I>(args: I) -> Command
where
    I: IntoIterator<Item = OsString>,
{
    match args.into_iter().next() {
        None => Command::Help,
        Some(word) if word == "help" || word == "-h" || word == "--help" => Command::Help,
        Some(word) => Command::Unknown(word),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};
    use std::ffi::OsString;

    fn args(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn no_argument_asks_for_help() {
        assert_eq!(parse(args(&[])), Command::Help);
    }

    #[test]
    fn help_spellings_ask_for_help() {
        for word in ["help", "-h", "--help"] {
            assert_eq!(parse(args(&[word])), Command::Help, "{word}");
        }
    }

    #[test]
    fn other_word_is_unknown() {
        assert_eq!(
            parse(args(&["frobnicate"])),
            Command::Unknown(OsString::from("frobnicate"))
        );
    }
}
