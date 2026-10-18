// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! `cc-shift doctor`: checks that everything `cc-shift` needs is in place and
//! says what to fix.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{IsTerminal, Write};
use std::path::Path;
use std::process::Command;

use crate::claude_code::settings;
use crate::command::home;
use crate::early::EarlyStart;
use crate::error::Error;
use crate::login::SystemLogins;
use crate::login::account::AccountFiles;
use crate::login::{Logins, Slot};
use crate::render::Paint;
use crate::store::profile::ProfilesRoot;
use crate::store::state::{State, StateFile};

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// In place.
    Ok,
    /// Works, but worth fixing.
    Warn,
    /// Stops `cc-shift` from working.
    Fail,
}

/// The fix for a login cc-shift cannot read.
#[cfg(target_os = "macos")]
const UNREADABLE_LOGIN_FIX: &str =
    "allow cc-shift to use the Keychain item, or run cc-shift add again";
/// The fix for a login cc-shift cannot read.
#[cfg(target_os = "linux")]
const UNREADABLE_LOGIN_FIX: &str =
    "check that you can read the profile's .credentials.json, or run cc-shift add again";
/// The fix for a login cc-shift cannot read.
#[cfg(windows)]
const UNREADABLE_LOGIN_FIX: &str = "check that you can read the profile's .credentials.json and that Claude Code does not use Credential Manager, or run cc-shift add again";

/// One check and, when it did not pass, the command that fixes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    /// How it came out.
    pub level: Level,
    /// What was checked and found.
    pub text: String,
    /// The command or step that fixes it.
    pub fix: Option<String>,
}

impl Check {
    fn ok(text: impl Into<String>) -> Self {
        Self {
            level: Level::Ok,
            text: text.into(),
            fix: None,
        }
    }

    fn warn(text: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            level: Level::Warn,
            text: text.into(),
            fix: Some(fix.into()),
        }
    }

    fn fail(text: impl Into<String>, fix: impl Into<String>) -> Self {
        Self {
            level: Level::Fail,
            text: text.into(),
            fix: Some(fix.into()),
        }
    }
}

/// Everything the checks read.
#[derive(Debug)]
pub struct Setup<'a, L: Logins> {
    /// The Keychain.
    pub logins: &'a L,
    /// The `.claude.json` files.
    pub accounts: &'a AccountFiles,
    /// The state file.
    pub file: &'a StateFile,
    /// The profiles root.
    pub root: &'a ProfilesRoot,
    /// The home folder, for `~/.claude/settings.json`.
    pub home: &'a Path,
    /// The absolute path of this `cc-shift` binary.
    pub binary: &'a str,
    /// The version `claude --version` printed, or `None` when it did not run.
    pub claude: Option<String>,
    /// The `curl` cc-shift would run, or `None` when there is none.
    pub curl: Option<std::path::PathBuf>,
}

/// Runs every check, in the order they are shown.
#[must_use]
pub fn checks<L: Logins>(setup: &Setup<'_, L>) -> Vec<Check> {
    let mut out = vec![match &setup.claude {
        Some(version) => Check::ok(format!("claude is on PATH ({version})")),
        None => Check::fail(
            "claude is not on PATH",
            "install Claude Code: https://code.claude.com/docs",
        ),
    }];
    out.push(match &setup.curl {
        Some(curl) => Check::ok(format!("curl is installed ({})", curl.display())),
        None => Check::fail(
            "curl is not installed",
            if cfg!(windows) {
                "curl.exe ships with Windows 10 1803 and later; check %SystemRoot%\\System32\\curl.exe"
            } else {
                "install curl with the system's package manager"
            },
        ),
    });
    out.push(settings_check(setup));
    let state = match setup.file.read() {
        Ok(state) => {
            out.push(Check::ok("the state file is readable"));
            state
        }
        Err(error) => {
            out.push(Check::fail(
                error.to_string(),
                "move ~/.claude-profiles/state.json aside and run cc-shift setup",
            ));
            return out;
        }
    };
    if let Some(pending) = &state.pending {
        out.push(Check::warn(
            format!("a switch to '{}' was left unfinished", pending.to),
            format!("cc-shift use {}", pending.to),
        ));
    }
    out.extend(early_check(setup.home, state.early_start.as_ref()));
    let profiles = match setup.root.list() {
        Ok(profiles) => profiles,
        Err(error) => {
            out.push(Check::fail(error.to_string(), "check ~/.claude-profiles"));
            return out;
        }
    };
    match profiles.len() {
        0 => {
            out.push(Check::fail("no profiles yet", "cc-shift add <name>"));
            return out;
        }
        1 => out.push(Check::warn(
            "only one profile, so there is nothing to switch to",
            "cc-shift add <name>",
        )),
        n => out.push(Check::ok(format!("{n} profiles"))),
    }
    out.extend(account_checks(setup, &state, &profiles));
    if let Some(active) = &state.active {
        let same = setup.accounts.active_id().ok().flatten()
            == setup.accounts.profile_id(active).ok().flatten();
        out.push(if same {
            Check::ok(format!("~/.claude.json names the active account, {active}"))
        } else {
            Check::warn(
                format!("~/.claude.json names another account than the active one, {active}"),
                "send a message in Claude Code; the status line puts it right",
            )
        });
    }
    out
}

/// Checks that the scheduled job exists exactly while the early start is
/// on; nothing to say while it is off.
fn early_check(home: &Path, setting: Option<&EarlyStart>) -> Option<Check> {
    let installed = crate::early::installed(home);
    match (setting, installed) {
        (Some(setting), true) => Some(Check::ok(format!(
            "early start is on: {}, {} account(s)",
            setting.at(),
            setting.accounts
        ))),
        (Some(_), false) => Some(Check::warn(
            "early start is on, but its scheduled job is missing",
            "cc-shift early on",
        )),
        (None, true) => Some(Check::warn(
            "the early start is off, but its scheduled job is still installed",
            "cc-shift early off",
        )),
        (None, false) => None,
    }
}

/// Checks every profile's login and account, and that no two profiles are
/// signed in to the same account.
fn account_checks<L: Logins>(
    setup: &Setup<'_, L>,
    state: &State,
    profiles: &[String],
) -> Vec<Check> {
    let mut out = Vec::new();
    let mut ids: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in profiles {
        let active = state.active.as_deref() == Some(name.as_str());
        let slot = if active {
            Slot::Active
        } else {
            Slot::Parked(name.clone())
        };
        let login = setup.logins.read(&slot);
        let id = setup.accounts.profile_id(name);
        match (login, id) {
            (Ok(Some(_)), Ok(Some(id))) => {
                ids.entry(id).or_default().push(name.clone());
                if state.logged_out.contains(name) {
                    out.push(Check::warn(
                        format!("{name}: logged out"),
                        format!("cc-shift add {name}, then /login"),
                    ));
                } else {
                    out.push(Check::ok(format!(
                        "{name}: logged in{}",
                        if active { " (active)" } else { "" }
                    )));
                }
            }
            (Ok(None), _) => out.push(Check::fail(
                format!("{name}: no stored login"),
                format!("cc-shift add {name}, then /login"),
            )),
            (_, Ok(None) | Err(_)) => out.push(Check::fail(
                format!("{name}: no signed-in account"),
                format!("cc-shift add {name}, then /login"),
            )),
            (Err(error), _) => out.push(Check::fail(
                format!("{name}: {error}"),
                UNREADABLE_LOGIN_FIX,
            )),
        }
    }
    for names in ids.values().filter(|names| names.len() > 1) {
        out.push(Check::fail(
            format!("{} are signed in to the same account", names.join(" and ")),
            format!(
                "cc-shift add {}, then /login with its own account",
                names.last().map_or("", String::as_str)
            ),
        ));
    }
    out
}

/// Checks that the status line and the hook point at this binary.
fn settings_check<L: Logins>(setup: &Setup<'_, L>) -> Check {
    let path = settings::settings_path(setup.home);
    match settings::read_settings(&path).and_then(|s| settings::install(s, setup.binary)) {
        Ok(installed) if !installed.changed => {
            Check::ok("the status line and the rate-limit hook are installed")
        }
        Ok(_) => Check::fail(
            "the status line or the rate-limit hook is missing, out of date or points elsewhere",
            "cc-shift setup",
        ),
        Err(error) => Check::fail(error.to_string(), "fix ~/.claude/settings.json"),
    }
}

/// Renders the checks, one line each, with the fix under a failed one.
#[must_use]
pub fn render(checks: &[Check], paint: Paint) -> String {
    let mut out = String::new();
    for check in checks {
        let mark = match check.level {
            Level::Ok => paint.green("✓"),
            Level::Warn => paint.yellow("!"),
            Level::Fail => paint.red("✗"),
        };
        // NOTE: no specification governs this: our own design. Writing to a
        // `String` cannot fail.
        let _infallible: std::fmt::Result = writeln!(out, "{mark} {}", check.text);
        if let Some(fix) = &check.fix {
            out.push_str(&paint.dim(&format!("  → {fix}")));
            out.push('\n');
        }
    }
    out
}

/// Runs the checks against the real system and prints them.
///
/// # Errors
///
/// Returns [`Error::DoctorFailed`] when a check failed, so the exit code is
/// 1; a missing `HOME` stops the run with its own error.
pub fn run() -> Result<(), Error> {
    let home = home()?;
    let root = ProfilesRoot::from_home(Some(home.clone().into_os_string()))?;
    let logins = SystemLogins::for_user(&root)?;
    let accounts = AccountFiles::new(&home, &root);
    let file = StateFile::in_root(&root);
    let binary = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let claude = crate::claude_code::launch::without_window(&mut Command::new(
        crate::claude_code::launch::claude_program(),
    ))
    .arg("--version")
    .output()
    .ok()
    .filter(|out| out.status.success())
    .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());
    let setup = Setup {
        logins: &logins,
        accounts: &accounts,
        file: &file,
        root: &root,
        home: &home,
        binary: &binary,
        claude,
        curl: crate::anthropic::http::find_curl(
            crate::anthropic::http::system_curl().as_deref(),
            std::env::var_os("PATH").as_deref(),
        ),
    };
    let results = checks(&setup);
    let paint = Paint::unless_no_color(std::io::stdout().is_terminal());
    let _ignored: std::io::Result<()> =
        std::io::stdout().write_all(render(&results, paint).as_bytes());
    if results.iter().any(|c| c.level == Level::Fail) {
        return Err(Error::DoctorFailed);
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{Level, Setup, checks};
    use crate::claude_code::settings;
    use crate::error::Error;
    use crate::login::account::AccountFiles;
    use crate::login::{Login, Logins, Slot};
    use crate::store::profile::ProfilesRoot;
    use crate::store::state::StateFile;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    const BINARY: &str = "/usr/local/bin/cc-shift";

    #[derive(Default)]
    struct Memory(RefCell<HashMap<Slot, Login>>);

    impl Logins for Memory {
        fn read(&self, slot: &Slot) -> Result<Option<Login>, Error> {
            Ok(self.0.borrow().get(slot).cloned())
        }
        fn write(&self, slot: &Slot, login: &Login) -> Result<(), Error> {
            self.0.borrow_mut().insert(slot.clone(), login.clone());
            Ok(())
        }
        fn clear(&self, slot: &Slot) -> Result<(), Error> {
            self.0.borrow_mut().remove(slot);
            Ok(())
        }
    }

    /// A home with profiles signed in to the given account ids; the first is
    /// active, and the settings are installed.
    fn home(ids: &[(&str, &str)]) -> Result<(TempDir, Memory), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let memory = Memory::default();
        for (i, (name, id)) in ids.iter().enumerate() {
            let dir = home.path().join(".claude-profiles").join(name);
            fs::create_dir_all(&dir)?;
            fs::write(
                dir.join(".claude.json"),
                format!(r#"{{"oauthAccount": {{"accountUuid": "{id}"}}}}"#),
            )?;
            let slot = if i == 0 {
                Slot::Active
            } else {
                Slot::Parked((*name).to_owned())
            };
            memory.write(&slot, &Login::from_bytes(b"login".to_vec()))?;
        }
        if let Some((first, id)) = ids.first() {
            fs::write(
                home.path().join(".claude.json"),
                format!(r#"{{"oauthAccount": {{"accountUuid": "{id}"}}}}"#),
            )?;
            let root = ProfilesRoot::from_home(Some(home.path().into()))?;
            StateFile::in_root(&root).update(|s| s.active = Some((*first).to_owned()))?;
        }
        let path = settings::settings_path(home.path());
        let installed = settings::install(serde_json::json!({}), BINARY)?;
        settings::write_settings(&path, &installed.settings)?;
        Ok((home, memory))
    }

    fn run(home: &TempDir, memory: &Memory) -> Result<Vec<(Level, String)>, Error> {
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        let accounts = AccountFiles::new(home.path(), &root);
        let file = StateFile::in_root(&root);
        let setup = Setup {
            logins: memory,
            accounts: &accounts,
            file: &file,
            root: &root,
            home: home.path(),
            binary: BINARY,
            claude: Some("2.1.300 (Claude Code)".to_owned()),
            curl: Some(std::path::PathBuf::from("/usr/bin/curl")),
        };
        Ok(checks(&setup)
            .into_iter()
            .map(|c| (c.level, c.text))
            .collect())
    }

    #[test]
    fn a_complete_setup_passes() -> Result<(), Box<dyn std::error::Error>> {
        let (home, memory) = home(&[("work", "id-1"), ("spare", "id-2")])?;
        let results = run(&home, &memory)?;
        assert!(
            results.iter().all(|(level, _)| *level == Level::Ok),
            "{results:?}"
        );
        Ok(())
    }

    #[test]
    fn two_profiles_on_one_account_fail() -> Result<(), Box<dyn std::error::Error>> {
        let (home, memory) = home(&[("work", "id-1"), ("spare", "id-1")])?;
        let results = run(&home, &memory)?;
        assert!(
            results.contains(&(
                Level::Fail,
                "spare and work are signed in to the same account".to_owned()
            )),
            "{results:?}"
        );
        Ok(())
    }

    #[test]
    fn a_missing_login_fails() -> Result<(), Box<dyn std::error::Error>> {
        let (home, memory) = home(&[("work", "id-1"), ("spare", "id-2")])?;
        memory.clear(&Slot::Parked("spare".to_owned()))?;
        let results = run(&home, &memory)?;
        assert!(
            results.contains(&(Level::Fail, "spare: no stored login".to_owned())),
            "{results:?}"
        );
        Ok(())
    }

    #[test]
    fn missing_settings_fail() -> Result<(), Box<dyn std::error::Error>> {
        let (home, memory) = home(&[("work", "id-1"), ("spare", "id-2")])?;
        fs::write(home.path().join(".claude/settings.json"), "{}")?;
        let results = run(&home, &memory)?;
        assert!(
            results.iter().any(|(level, text)| *level == Level::Fail
                && text.contains("status line or the rate-limit hook")),
            "{results:?}"
        );
        Ok(())
    }

    #[test]
    fn other_account_details_warn() -> Result<(), Box<dyn std::error::Error>> {
        let (home, memory) = home(&[("work", "id-1"), ("spare", "id-2")])?;
        fs::write(
            home.path().join(".claude.json"),
            r#"{"oauthAccount": {"accountUuid": "id-2"}}"#,
        )?;
        let results = run(&home, &memory)?;
        assert!(
            results.iter().any(
                |(level, text)| *level == Level::Warn && text.contains("names another account")
            ),
            "{results:?}"
        );
        Ok(())
    }

    #[test]
    fn the_early_start_agent_must_match_the_setting() -> Result<(), Box<dyn std::error::Error>> {
        use super::early_check;
        use crate::early::EarlyStart;
        let home = TempDir::new()?;
        let on = EarlyStart::default();
        assert_eq!(
            early_check(home.path(), None),
            None,
            "off and absent: nothing to say"
        );
        let missing = early_check(home.path(), Some(&on)).ok_or("a check")?;
        assert_eq!(
            (missing.level, missing.fix.as_deref()),
            (Level::Warn, Some("cc-shift early on"))
        );
        let agent = crate::early::job_path(home.path());
        fs::create_dir_all(agent.parent().ok_or("a folder")?)?;
        fs::write(&agent, "")?;
        let installed = early_check(home.path(), Some(&on)).ok_or("a check")?;
        assert_eq!(installed.level, Level::Ok);
        let left = early_check(home.path(), None).ok_or("a check")?;
        assert_eq!(
            (left.level, left.fix.as_deref()),
            (Level::Warn, Some("cc-shift early off"))
        );
        Ok(())
    }
}
