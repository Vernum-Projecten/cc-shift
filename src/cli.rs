//! Parsing of the `cs` command line.

use std::ffi::OsString;

/// The usage text printed by `cs help` and after a command line `cs` does
/// not understand.
pub const USAGE: &str = "\
Usage: cs <name> [claude arguments...]
       cs <command>

Commands:
  add <name>    Create a profile and start Claude Code in it to run /login
  list          Print the profiles
  help          Print this help

cs <name> starts Claude Code with that profile; every argument after the
name goes to claude unchanged.
";

/// A parsed `cs` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Print the usage text.
    Help,
    /// Create a profile and start Claude Code in it.
    Add(OsString),
    /// Print the profiles.
    List,
    /// Start Claude Code with a profile, passing the rest of the arguments.
    Launch {
        /// The profile name as typed.
        name: OsString,
        /// The arguments for `claude`, unchanged and in order.
        args: Vec<OsString>,
    },
    /// A command line `cs` does not understand, with the reason.
    Misuse(String),
}

/// Parses the arguments that follow the program name.
///
/// No argument, `help`, `-h` and `--help` all ask for help.
#[must_use]
pub fn parse<I>(args: I) -> Command
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Command::Help;
    };
    if first == "help" || first == "-h" || first == "--help" {
        return Command::Help;
    }
    if first == "add" {
        return match (args.next(), args.next()) {
            (Some(name), None) => Command::Add(name),
            (None, _) => Command::Misuse("add needs a profile name".to_owned()),
            (Some(_), Some(extra)) => Command::Misuse(format!(
                "add takes one profile name, not '{}'",
                extra.to_string_lossy()
            )),
        };
    }
    if first == "list" {
        return match args.next() {
            None => Command::List,
            Some(extra) => Command::Misuse(format!(
                "list takes no arguments, not '{}'",
                extra.to_string_lossy()
            )),
        };
    }
    if first.as_encoded_bytes().starts_with(b"-") {
        return Command::Misuse(format!("unknown option '{}'", first.to_string_lossy()));
    }
    Command::Launch {
        name: first,
        args: args.collect(),
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
    fn add_takes_one_name() {
        assert_eq!(
            parse(args(&["add", "claude1"])),
            Command::Add(OsString::from("claude1"))
        );
    }

    #[test]
    fn add_without_name_is_misuse() {
        assert!(matches!(parse(args(&["add"])), Command::Misuse(_)));
    }

    #[test]
    fn add_with_two_names_is_misuse() {
        assert!(matches!(
            parse(args(&["add", "a", "b"])),
            Command::Misuse(_)
        ));
    }

    #[test]
    fn list_takes_no_arguments() {
        assert_eq!(parse(args(&["list"])), Command::List);
        assert!(matches!(parse(args(&["list", "x"])), Command::Misuse(_)));
    }

    #[test]
    fn other_word_launches_with_the_rest() {
        assert_eq!(
            parse(args(&["claude1", "--continue", "-p", "hi"])),
            Command::Launch {
                name: OsString::from("claude1"),
                args: args(&["--continue", "-p", "hi"]),
            }
        );
    }

    #[test]
    fn leading_dash_is_misuse() {
        assert!(matches!(parse(args(&["--frobnicate"])), Command::Misuse(_)));
    }
}
