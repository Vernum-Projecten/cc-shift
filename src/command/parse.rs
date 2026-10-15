//! Parsing of the `cc-shift` command line.

use std::ffi::OsString;

/// The usage text printed by `cc-shift help` and after a command line `cc-shift` does
/// not understand.
pub const USAGE: &str = "\
Usage: cc-shift <command>

Commands:
  add <name>    Create a profile and start Claude Code in it to run /login
  setup         Install cc-shift in Claude Code once, so switching is automatic
  status        Show every account with its usage and reset times
  status --json The same as JSON, for scripts
  use <name>    Make an account active in every Claude Code window now
  refresh       Refresh parked logins and read their usage now
  doctor        Check the setup and say what to fix
  remove <name> Delete a profile and its Keychain login
  completion zsh
                Print the zsh completion script
  uninstall [--all]
                Undo setup; --all also removes every profile and its login
  list          Print the profiles
  statusline    Record usage; run by Claude Code as its status line
  hook rate-limit
                Switch after a usage limit; run by Claude Code as a hook
  help          Print this help
";

/// A parsed `cc-shift` command line.
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
    /// Print every account with its usage, as JSON when `json` is set.
    Status {
        /// Whether to print JSON instead of the table.
        json: bool,
    },
    /// Refresh parked logins and read parked accounts' usage.
    Refresh,
    /// Check the setup and say what to fix.
    Doctor,
    /// Delete a profile and its Keychain login.
    Remove(OsString),
    /// Print the completion script for a shell.
    Completion(String),
    /// Undo setup; with `all`, also remove every profile.
    Uninstall {
        /// Whether to remove the profiles and their logins too.
        all: bool,
    },
    /// A turn ended on a usage limit; run by Claude Code's `StopFailure`
    /// hook.
    HookRateLimit,
    /// Record usage from the status line input; run by Claude Code.
    Statusline,
    /// A command line `cc-shift` does not understand, with the reason.
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
    let rest: Vec<OsString> = args.collect();
    let flags: Vec<&str> = rest.iter().map(|a| a.to_str().unwrap_or("")).collect();
    match first.to_str().unwrap_or("") {
        "help" | "-h" | "--help" => Command::Help,
        "add" => one_name("add", rest).map_or_else(|misuse| misuse, Command::Add),
        "use" => one_name("use", rest).map_or_else(|misuse| misuse, Command::Use),
        "remove" => one_name("remove", rest).map_or_else(|misuse| misuse, Command::Remove),
        "setup" => bare("setup", &rest, Command::Setup),
        "doctor" => bare("doctor", &rest, Command::Doctor),
        "refresh" => bare("refresh", &rest, Command::Refresh),
        "statusline" => bare("statusline", &rest, Command::Statusline),
        "list" => bare("list", &rest, Command::List),
        "status" => match flags.as_slice() {
            [] => Command::Status { json: false },
            ["--json"] => Command::Status { json: true },
            _ => Command::Misuse("status takes only --json".to_owned()),
        },
        "uninstall" => match flags.as_slice() {
            [] => Command::Uninstall { all: false },
            ["--all"] => Command::Uninstall { all: true },
            _ => Command::Misuse("uninstall takes only --all".to_owned()),
        },
        "completion" => match flags.as_slice() {
            [shell] => Command::Completion((*shell).to_owned()),
            _ => Command::Misuse("completion takes one shell: zsh".to_owned()),
        },
        "hook" => match flags.as_slice() {
            ["rate-limit"] => Command::HookRateLimit,
            _ => Command::Misuse("hook takes one event: rate-limit".to_owned()),
        },
        _ => {
            let kind = if first.as_encoded_bytes().starts_with(b"-") {
                "option"
            } else {
                "command"
            };
            Command::Misuse(format!("unknown {kind} '{}'", first.to_string_lossy()))
        }
    }
}

/// Returns the one profile name a command takes, or the misuse.
fn one_name(command: &str, rest: Vec<OsString>) -> Result<OsString, Command> {
    let mut rest = rest.into_iter();
    match (rest.next(), rest.next()) {
        (Some(name), None) => Ok(name),
        (None, _) => Err(Command::Misuse(format!("{command} needs a profile name"))),
        (Some(_), Some(extra)) => Err(Command::Misuse(format!(
            "{command} takes one profile name, not '{}'",
            extra.to_string_lossy()
        ))),
    }
}

/// Returns `command` when no argument follows it, or the misuse.
fn bare(name: &str, rest: &[OsString], command: Command) -> Command {
    match rest.first() {
        None => command,
        Some(extra) => Command::Misuse(format!(
            "{name} takes no arguments, not '{}'",
            extra.to_string_lossy()
        )),
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
    fn status_takes_an_optional_json_flag() {
        assert_eq!(parse(args(&["status"])), Command::Status { json: false });
        assert_eq!(
            parse(args(&["status", "--json"])),
            Command::Status { json: true }
        );
        assert!(matches!(
            parse(args(&["status", "--yaml"])),
            Command::Misuse(_)
        ));
    }

    #[test]
    fn uninstall_takes_an_optional_all_flag() {
        assert_eq!(
            parse(args(&["uninstall"])),
            Command::Uninstall { all: false }
        );
        assert_eq!(
            parse(args(&["uninstall", "--all"])),
            Command::Uninstall { all: true }
        );
        assert!(matches!(
            parse(args(&["uninstall", "--force"])),
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
