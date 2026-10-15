//! Parsing of the `cc-shift` command line.

use std::ffi::OsString;

use crate::stats::{Options, Period};

/// The usage text printed by `cc-shift help` and after a command line `cc-shift` does
/// not understand.
pub const USAGE: &str = "\
Usage: cc-shift <command>

Commands:
  add <name>    Create a profile and start Claude Code in it to run /login
  setup         Install cc-shift in Claude Code once, so switching is automatic
  status        Show every account with its usage and reset times
  status --json The same as JSON, for scripts
  stats         Show today's tokens of all accounts per project and model
  stats [--since 2h|7d|30d | --all] [--project <name>] [--cost]
        [--per-account] [--json]
                Pick the period or one project, add the API cost or a split
                per account, or print JSON
  use <name>    Make an account active in every Claude Code window now
  refresh       Refresh parked logins and read their usage now
  doctor        Check the setup and say what to fix
  log           Show what cc-shift did: switches, refreshes, full accounts
                (--since 2h|7d|30d, --all, --verbose, --json)
  early         Show the early start: windows started before the workday
  early on [--at 06:00] [--accounts 2]
                Start that many windows at that time, Monday to Friday
  early off     Stop the early start
  remove <name> Delete a profile and its Keychain login
  completion zsh
                Print the zsh completion script
  uninstall [--all]
                Undo setup; --all also removes every profile and its login
  list          Print the profiles
  statusline    Record usage; run by Claude Code as its status line
  early run     Start the windows now; run by launchd
  hook rate-limit
                Switch after a usage limit; run by Claude Code as a hook
  version       Print the version (also --version, -V)
  help          Print this help
";

/// A parsed `cc-shift` command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Print the usage text.
    Help,
    /// Print the version.
    Version,
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
    /// Print token statistics across all accounts.
    Stats(Options),
    /// Refresh parked logins and read parked accounts' usage.
    Refresh,
    /// Check the setup and say what to fix.
    Doctor,
    /// Show the history.
    Log(crate::command::log::Options),
    /// Show, turn on or off, or run the early start.
    Early(crate::command::early::Action),
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
        "version" | "-V" | "--version" => bare("version", &rest, Command::Version),
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
        "stats" => stats(&flags),
        "uninstall" => match flags.as_slice() {
            [] => Command::Uninstall { all: false },
            ["--all"] => Command::Uninstall { all: true },
            _ => Command::Misuse("uninstall takes only --all".to_owned()),
        },
        "completion" => match flags.as_slice() {
            [shell] => Command::Completion((*shell).to_owned()),
            _ => Command::Misuse("completion takes one shell: zsh".to_owned()),
        },
        "log" => crate::command::log::options(&flags).map_or_else(Command::Misuse, Command::Log),
        "early" => {
            crate::command::early::action(&flags).map_or_else(Command::Misuse, Command::Early)
        }
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

/// Returns the options of `cc-shift stats`, or the misuse.
fn stats(flags: &[&str]) -> Command {
    let mut options = Options::default();
    let mut period = None;
    let mut rest = flags.iter();
    while let Some(flag) = rest.next() {
        match *flag {
            "--cost" => options.cost = true,
            "--json" => options.json = true,
            "--per-account" => options.per_account = true,
            "--all" if period.is_none() => period = Some(Period::All),
            "--since" if period.is_none() => match rest.next().and_then(|v| since(v)) {
                Some(p) => period = Some(p),
                None => {
                    return Command::Misuse(
                        "--since takes a number of hours or days, like 2h, 7d or 30d".to_owned(),
                    );
                }
            },
            "--all" | "--since" => {
                return Command::Misuse("stats takes one of --since and --all".to_owned());
            }
            "--project" => match rest.next() {
                Some(name) if !name.is_empty() && options.project.is_none() => {
                    options.project = Some((*name).to_owned());
                }
                _ => return Command::Misuse("--project takes one project name".to_owned()),
            },
            other => return Command::Misuse(format!("stats does not take '{other}'")),
        }
    }
    options.period = period.unwrap_or(Period::Today);
    Command::Stats(options)
}

/// Parses a `--since` value: a whole number of hours (`2h`) or days (`7d`),
/// at least one.
fn since(value: &str) -> Option<Period> {
    let unit = value.chars().last()?;
    let number: u32 = value.strip_suffix(unit)?.parse().ok().filter(|n| *n > 0)?;
    match unit {
        'h' => Some(Period::Hours(number)),
        'd' => Some(Period::Days(number)),
        _ => None,
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
    use crate::stats::{Options, Period};
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
    fn stats_counts_today_by_default() {
        assert_eq!(parse(args(&["stats"])), Command::Stats(Options::default()));
    }

    #[test]
    fn stats_options_combine_freely() {
        assert_eq!(
            parse(args(&[
                "stats",
                "--json",
                "--since",
                "30d",
                "--cost",
                "--project",
                "shop"
            ])),
            Command::Stats(Options {
                period: Period::Days(30),
                project: Some("shop".to_owned()),
                cost: true,
                json: true,
                per_account: false,
            })
        );
        assert_eq!(
            parse(args(&["stats", "--since", "2h"])),
            Command::Stats(Options {
                period: Period::Hours(2),
                ..Options::default()
            })
        );
        assert_eq!(
            parse(args(&["stats", "--all"])),
            Command::Stats(Options {
                period: Period::All,
                ..Options::default()
            })
        );
        assert_eq!(
            parse(args(&["stats", "--per-account"])),
            Command::Stats(Options {
                per_account: true,
                ..Options::default()
            })
        );
    }

    #[test]
    fn stats_refuses_bad_options() {
        for words in [
            &["stats", "--since"][..],
            &["stats", "--since", "7"],
            &["stats", "--since", "0d"],
            &["stats", "--since", "7w"],
            &["stats", "--since", "d"],
            &["stats", "--since", "7d", "--all"],
            &["stats", "--all", "--all"],
            &["stats", "--project"],
            &["stats", "--project", "a", "--project", "b"],
            &["stats", "--per-day"],
        ] {
            assert!(
                matches!(parse(args(words)), Command::Misuse(_)),
                "{words:?}"
            );
        }
    }

    #[test]
    fn early_takes_on_off_run_or_nothing() {
        use crate::command::early::Action;
        assert_eq!(parse(args(&["early"])), Command::Early(Action::Show));
        assert_eq!(
            parse(args(&["early", "on", "--at", "07:15"])),
            Command::Early(Action::On {
                at: Some((7, 15)),
                accounts: None
            })
        );
        assert!(matches!(
            parse(args(&["early", "on", "--at", "7"])),
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
