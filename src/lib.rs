//! `cs` switches Claude Code between several accounts automatically.
//!
//! Each account has a profile folder under `~/.claude-profiles/` that holds
//! its parked login. The binary in `main.rs` only calls [`run`], so the
//! whole run path is testable from `tests/`.

pub mod account;
pub mod auto;
pub mod cli;
pub mod error;
pub mod keychain;
pub mod launch;
pub mod profile;
pub mod select;
pub mod setup;
pub mod state;
pub mod status;
pub mod statusline;
pub mod switch;
pub mod system_keychain;
pub mod usage;

use std::convert::Infallible;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::process::ExitCode;

use cli::Command;
use error::Error;
use profile::{ProfileName, ProfilesRoot};
use state::StateFile;

/// The exit code for a command that failed.
const FAILURE: u8 = 1;

/// The exit code for a command line that `cs` does not understand.
const USAGE_ERROR: u8 = 2;

/// Runs `cs` with the given arguments, without the program name.
///
/// Returns the process exit code: success after printing help, `1` for a
/// failed command and `2` for a command line `cs` does not understand.
/// `cs add` replaces this process with Claude Code and does not return.
#[must_use]
pub fn run<I>(args: I) -> ExitCode
where
    I: IntoIterator<Item = OsString>,
{
    match cli::parse(args) {
        Command::Help => {
            // NOTE: no specification governs this: our own design. A failed
            // write to stdout (a closed pipe) is not an error for help text.
            let _ignored: std::io::Result<()> = std::io::stdout().write_all(cli::USAGE.as_bytes());
            ExitCode::SUCCESS
        }
        Command::Misuse(reason) => {
            report(&format!("{reason}\n\n{}", cli::USAGE));
            ExitCode::from(USAGE_ERROR)
        }
        Command::List => match list() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                report(&error.to_string());
                ExitCode::from(FAILURE)
            }
        },
        Command::Add(name) => fail(add(&name)),
        Command::Use(name) => match use_command(&name) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                report(&error.to_string());
                ExitCode::from(FAILURE)
            }
        },
        Command::Setup => match setup_command() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                report(&error.to_string());
                ExitCode::from(FAILURE)
            }
        },
        Command::Status => match status() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                report(&error.to_string());
                ExitCode::from(FAILURE)
            }
        },
        Command::HookRateLimit => {
            // NOTE: no specification governs this: our own design. The hook
            // always exits 0, so a cs problem never blocks a Claude Code turn.
            let mut input = Vec::new();
            let _ignored: std::io::Result<usize> = std::io::stdin().read_to_end(&mut input);
            let _switched: Result<Option<String>, Error> = automatic(auto::Trigger::LimitHit);
            ExitCode::SUCCESS
        }
        Command::Statusline => {
            statusline_command();
            ExitCode::SUCCESS
        }
    }
}

/// Reports the error of a command that would otherwise have replaced `cs`.
fn fail(result: Result<Infallible, Error>) -> ExitCode {
    match result {
        Ok(never) => match never {},
        Err(error) => {
            report(&error.to_string());
            ExitCode::from(FAILURE)
        }
    }
}

/// Moves an account's parked login into `~/.claude`, so every window
/// continues on it.
fn use_command(raw: &OsStr) -> Result<(), Error> {
    let name = ProfileName::parse(raw)?;
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or(Error::NoHome)?;
    let root = ProfilesRoot::from_home(Some(home.clone()))?;
    let logins = system_keychain::SystemKeychain::for_user(&root)?;
    let accounts = account::AccountFiles::new(std::path::Path::new(&home), &root);
    match switch::switch_to(
        &logins,
        &accounts,
        &StateFile::in_root(&root),
        name.as_str(),
    )? {
        switch::Outcome::AlreadyActive => {
            report(&format!("'{}' is already active", name.as_str()));
        }
        switch::Outcome::Switched { from: Some(from) } => report(&format!(
            "'{}' is active; '{from}' is parked",
            name.as_str()
        )),
        switch::Outcome::Switched { from: None } => report(&format!(
            "'{}' is active; the login that was in ~/.claude belonged to no profile and was replaced",
            name.as_str()
        )),
    }
    Ok(())
}

/// Installs the status line and the hook in `~/.claude/settings.json`, and
/// remembers the user's own status line to chain it.
fn setup_command() -> Result<(), Error> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or(Error::NoHome)?;
    let root = ProfilesRoot::from_home(Some(home.clone()))?;
    let cs_path = std::env::current_exe()
        .map_err(|e| Error::Settings(format!("cannot find the cs binary: {e}")))?;
    let path = setup::settings_path(std::path::Path::new(&home));
    let installed = setup::install(setup::read_settings(&path)?, &cs_path.to_string_lossy())?;
    if let Some(previous) = installed.previous_statusline.clone() {
        StateFile::in_root(&root).update(|state| state.chained_statusline = Some(previous))?;
    }
    if installed.changed {
        setup::write_settings(&path, &installed.settings)?;
        report(&format!(
            "installed the status line and the rate-limit hook in {}",
            path.display()
        ));
    } else {
        report("already set up");
    }
    if root.list()?.is_empty() {
        report("no profiles yet; add them with 'cs add <name>', then run 'cs setup' again");
        return Ok(());
    }
    if StateFile::in_root(&root).read()?.active.is_none() {
        report("the login now in ~/.claude belongs to no profile and is replaced");
    }
    match automatic(auto::Trigger::Setup)? {
        Some(name) => report(&format!(
            "'{name}' is active; switching is automatic from now on"
        )),
        None => report("switching is automatic from now on"),
    }
    Ok(())
}

/// Runs an automatic switch against the real Keychain and files.
fn automatic(trigger: auto::Trigger) -> Result<Option<String>, Error> {
    let home = std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .ok_or(Error::NoHome)?;
    let root = ProfilesRoot::from_home(Some(home.clone()))?;
    let logins = system_keychain::SystemKeychain::for_user(&root)?;
    let accounts = account::AccountFiles::new(std::path::Path::new(&home), &root);
    let file = StateFile::in_root(&root);
    let context = auto::Context {
        logins: &logins,
        accounts: &accounts,
        file: &file,
        root: &root,
    };
    auto::run(&context, trigger, unix_now())
}

/// Prints every account with its usage and which account comes next.
fn status() -> Result<(), Error> {
    let root = ProfilesRoot::from_env()?;
    let profiles = root.list()?;
    if profiles.is_empty() {
        report("no profiles yet; create one with 'cs add <name>'");
        return Ok(());
    }
    let state = StateFile::in_root(&root).read()?;
    let accounts = status::accounts(&profiles, &state);
    let text = status::render(&accounts, state.active.as_deref(), unix_now());
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(text.as_bytes());
    Ok(())
}

/// Returns the current time in Unix seconds.
fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// Records usage from the status line input on stdin and prints the user's
/// own status line. It never fails: a `cs` problem must not break the
/// status line of a Claude Code window.
fn statusline_command() {
    let mut input = Vec::new();
    if std::io::stdin().read_to_end(&mut input).is_err() {
        return;
    }
    let now = unix_now();
    let Ok(root) = ProfilesRoot::from_env() else {
        return;
    };
    let file = StateFile::in_root(&root);
    let chained = statusline::record(&input, &file, now).ok().flatten();
    let _switched: Result<Option<String>, Error> = automatic(auto::Trigger::Usage);
    let mut line = chained
        .map(|command| statusline::chained(&command, &input))
        .unwrap_or_default();
    while line.last().is_some_and(u8::is_ascii_whitespace) {
        line.pop();
    }
    if let Ok(state) = file.read() {
        if !line.is_empty() {
            line.extend_from_slice(" │ ".as_bytes());
        }
        line.extend_from_slice(statusline::summary(&state, now).as_bytes());
    }
    let _ignored: std::io::Result<()> = std::io::stdout().write_all(&line);
}

/// Creates a profile when missing and starts Claude Code in it.
fn add(raw: &OsStr) -> Result<Infallible, Error> {
    let name = ProfileName::parse(raw)?;
    let root = ProfilesRoot::from_env()?;
    let dir = root.create(&name)?;
    report(&format!(
        "profile '{}' is at {}\nRun /login in Claude Code and sign in with the account for this profile, then quit.",
        name.as_str(),
        dir.display()
    ));
    launch::exec_claude(&dir)
}

/// Prints every profile name on its own line, or how to create the first.
fn list() -> Result<(), Error> {
    let names = ProfilesRoot::from_env()?.list()?;
    if names.is_empty() {
        report("no profiles yet; create one with 'cs add <name>'");
        return Ok(());
    }
    let mut stdout = std::io::stdout().lock();
    for name in names {
        // NOTE: no specification governs this: our own design. A closed pipe
        // (`cs list | head -1`) ends the listing without an error.
        if writeln!(stdout, "{name}").is_err() {
            break;
        }
    }
    Ok(())
}

/// Writes one `cs:` message to stderr.
fn report(message: &str) {
    // NOTE: no specification governs this: our own design. If stderr itself
    // is closed there is nowhere left to report, so the write error is dropped.
    let _ignored: std::io::Result<()> = writeln!(std::io::stderr().lock(), "cs: {message}");
}
