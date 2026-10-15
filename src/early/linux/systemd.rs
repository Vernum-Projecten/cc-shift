//! The systemd user timer that runs `cc-shift early run` each workday.
//!
//! A user unit lives in `$XDG_CONFIG_HOME/systemd/user` (`~/.config` by
//! default) and runs while the user has a session. With `Persistent=true`, a
//! run missed while the machine was off starts at the next boot or login; a
//! timer does not wake a suspended machine
//! (<https://www.freedesktop.org/software/systemd/man/latest/systemd.timer.html>).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::early::EarlyStart;
use crate::error::Error;

/// The name of the units, without their suffix.
pub const UNIT: &str = "cc-shift-early-start";

/// Returns the folder of the user's own units under `home`.
#[must_use]
pub fn units_folder(home: &Path) -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map_or_else(|| home.join(".config"), PathBuf::from)
        .join("systemd/user")
}

/// Returns the path of the timer unit under `home`.
#[must_use]
pub fn timer_path(home: &Path) -> PathBuf {
    units_folder(home).join(format!("{UNIT}.timer"))
}

/// Returns the service unit: `binary early run` once, with `path` as its
/// `PATH` so it finds `claude`.
#[must_use]
pub fn service(binary: &Path, path: &str) -> String {
    format!(
        "[Unit]\n\
         Description=cc-shift early start: start 5-hour windows before the workday\n\
         \n\
         [Service]\n\
         Type=oneshot\n\
         Environment=\"PATH={}\"\n\
         ExecStart=\"{}\" early run\n",
        quote(path),
        quote(&binary.to_string_lossy()),
    )
}

/// Returns the timer unit: Monday to Friday at the set time, and a missed
/// run caught up at the next boot or login.
#[must_use]
pub fn timer(setting: EarlyStart) -> String {
    format!(
        "[Unit]\n\
         Description=cc-shift early start, Monday to Friday at {at}\n\
         \n\
         [Timer]\n\
         OnCalendar=Mon..Fri *-*-* {at}:00\n\
         Persistent=true\n\
         \n\
         [Install]\n\
         WantedBy=timers.target\n",
        at = setting.at(),
    )
}

/// Writes both units and enables the timer, replacing an earlier one.
///
/// # Errors
///
/// Returns [`Error::EarlyStart`] when there is no user systemd, and
/// [`Error::Settings`] when a unit cannot be written or enabled.
pub fn install(home: &Path, setting: EarlyStart, binary: &Path, path: &str) -> Result<(), Error> {
    if !systemctl(&["show-environment"])? {
        return Err(Error::EarlyStart(
            "the early start needs a systemd user session, and none is running".to_owned(),
        ));
    }
    let folder = units_folder(home);
    fs::create_dir_all(&folder).map_err(|source| Error::CreateFolder {
        path: folder.clone(),
        source,
    })?;
    let write = |name: String, text: String| {
        let file = folder.join(name);
        fs::write(&file, text)
            .map_err(|e| Error::Settings(format!("cannot write {}: {e}", file.display())))
    };
    write(format!("{UNIT}.service"), service(binary, path))?;
    write(format!("{UNIT}.timer"), timer(setting))?;
    let timer_unit = format!("{UNIT}.timer");
    let enabled = systemctl(&["daemon-reload"])?
        && systemctl(&["enable", &timer_unit])?
        && systemctl(&["restart", &timer_unit])?;
    if enabled {
        Ok(())
    } else {
        Err(Error::Settings(format!(
            "systemctl --user could not enable {timer_unit}"
        )))
    }
}

/// Disables the timer and deletes both units; nothing happens when there is
/// no timer. Returns whether one was removed.
///
/// # Errors
///
/// Returns [`Error::Settings`] when a unit cannot be deleted.
pub fn remove(home: &Path) -> Result<bool, Error> {
    let timer_file = timer_path(home);
    if !timer_file.exists() {
        return Ok(false);
    }
    let timer_unit = format!("{UNIT}.timer");
    // NOTE: no specification governs this: our own design. Without a running
    // user systemd there is nothing loaded to stop, and the files still go.
    let _stopped: bool = systemctl(&["disable", "--now", &timer_unit]).unwrap_or(false);
    let _cleaned: bool = systemctl(&["clean", "--what=state", &timer_unit]).unwrap_or(false);
    for file in [
        timer_file,
        units_folder(home).join(format!("{UNIT}.service")),
    ] {
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::Settings(format!(
                    "cannot delete {}: {e}",
                    file.display()
                )));
            }
        }
    }
    let _reloaded: bool = systemctl(&["daemon-reload"]).unwrap_or(false);
    Ok(true)
}

/// Runs `systemctl --user` and returns whether it succeeded; `false` when
/// there is no `systemctl`.
fn systemctl(args: &[&str]) -> Result<bool, Error> {
    match Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => Ok(status.success()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::Launch(e)),
    }
}

/// Quotes a value for a double-quoted systemd setting: `\` and `"` are
/// escaped, and `%` is doubled so systemd does not expand it.
fn quote(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%")
}

#[cfg(test)]
mod tests {
    use super::{UNIT, quote, service, timer, timer_path};
    use crate::early::EarlyStart;
    use std::path::Path;

    #[test]
    fn the_timer_runs_on_workdays_at_the_set_time_and_catches_up() {
        let text = timer(EarlyStart {
            hour: 5,
            minute: 30,
            accounts: 2,
        });
        assert!(
            text.contains("OnCalendar=Mon..Fri *-*-* 05:30:00\n"),
            "{text}"
        );
        assert!(text.contains("Persistent=true\n"), "{text}");
        assert!(text.contains("WantedBy=timers.target\n"), "{text}");
    }

    #[test]
    fn the_service_runs_early_run_once_with_the_path() {
        let text = service(
            Path::new("/home/u/.cargo/bin/cc-shift"),
            "/home/u/bin:/usr/bin",
        );
        assert!(text.contains("Type=oneshot\n"), "{text}");
        assert!(
            text.contains("Environment=\"PATH=/home/u/bin:/usr/bin\"\n"),
            "{text}"
        );
        assert!(
            text.contains("ExecStart=\"/home/u/.cargo/bin/cc-shift\" early run\n"),
            "{text}"
        );
    }

    #[test]
    fn values_are_quoted_for_systemd() {
        assert_eq!(quote(r#"a "b" \c 100%"#), r#"a \"b\" \\c 100%%"#);
    }

    #[test]
    fn the_timer_is_a_user_unit() {
        let path = timer_path(Path::new("/home/u"));
        assert!(
            path.ends_with(format!("systemd/user/{UNIT}.timer")),
            "{}",
            path.display()
        );
    }
}
