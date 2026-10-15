//! Parsing of the `cs` command line.

use std::ffi::OsString;

/// The usage text printed by `cs help` and after a command line `cs` does
/// not understand.
pub const USAGE: &str = "\
Usage: cs <command>

Commands:
  add <name>    Create a profile and start Claude Code in it to run /login
  setup         Install cs in Claude Code once, so switching is automatic
  status        Show every account with its usage and reset times
  use <name>    Make an account active in every Claude Code window now
  list          Print the profiles
  statusline    Record usage; run by Claude Code as its status line
  help          Print this help
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
    /// Install the status line and the hook in `~/.claude/settings.json`.
    Setup,
    /// Make an account the active account of `~/.claude`.
    Use(OsString),
    /// Print every account with its usage.
    Status,
    /// Record usage from the status line input; run by Claude Code.
    Statusline,
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
    if first == "use" {
        return match (args.next(), args.next()) {
            (Some(name), None) => Command::Use(name),
            (None, _) => Command::Misuse("use needs a profile name".to_owned()),
            (Some(_), Some(extra)) => Command::Misuse(format!(
                "use takes one profile name, not '{}'",
                extra.to_string_lossy()
            )),
        };
    }
    if first == "setup" {
        return match args.next() {
            None => Command::Setup,
            Some(extra) => Command::Misuse(format!(
                "setup takes no arguments, not '{}'",
                extra.to_string_lossy()
            )),
        };
    }
    if first == "status" {
        return match args.next() {
            None => Command::Status,
            Some(extra) => Command::Misuse(format!(
                "status takes no arguments, not '{}'",
                extra.to_string_lossy()
            )),
        };
    }
    if first == "statusline" {
        return match args.next() {
            None => Command::Statusline,
            Some(extra) => Command::Misuse(format!(
                "statusline takes no arguments, not '{}'",
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
    let kind = if first.as_encoded_bytes().starts_with(b"-") {
        "option"
    } else {
        "command"
    };
    Command::Misuse(format!("unknown {kind} '{}'", first.to_string_lossy()))
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
            parse(args(&["add", "member1"])),
            Command::Add(OsString::from("member1"))
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
    fn use_takes_one_name() {
        assert_eq!(
            parse(args(&["use", "member1"])),
            Command::Use(OsString::from("member1"))
        );
        assert!(matches!(parse(args(&["use"])), Command::Misuse(_)));
        assert!(matches!(
            parse(args(&["use", "a", "b"])),
            Command::Misuse(_)
        ));
    }

    #[test]
    fn list_takes_no_arguments() {
        assert_eq!(parse(args(&["list"])), Command::List);
        assert!(matches!(parse(args(&["list", "x"])), Command::Misuse(_)));
    }

    #[test]
    fn other_word_is_unknown_command() {
        assert_eq!(
            parse(args(&["member1"])),
            Command::Misuse("unknown command 'member1'".to_owned())
        );
    }

    #[test]
    fn leading_dash_is_misuse() {
        assert!(matches!(parse(args(&["--frobnicate"])), Command::Misuse(_)));
    }
}
