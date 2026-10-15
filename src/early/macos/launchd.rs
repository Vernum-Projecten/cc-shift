//! The `launchd` agent that runs `cc-shift early run` each workday.
//!
//! A user agent lives in `~/Library/LaunchAgents` and runs while the user is
//! logged in. With `StartCalendarInterval`, a run missed while the Mac was
//! asleep starts when it wakes; a run missed while it was off is skipped
//! (<https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/ScheduledJobs.html>).

use std::fmt::Write;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::early::EarlyStart;
use crate::error::Error;

/// The label of the agent, which is also its file name.
pub const LABEL: &str = "io.github.vernum-projecten.cc-shift.early-start";

/// Returns the path of the agent's file under `home`.
#[must_use]
pub fn agent_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// Returns the property list of the agent: `binary early run` at the set
/// time, Monday (1) to Friday (5), with `path` as its `PATH` so it finds
/// `claude`.
#[must_use]
pub fn plist(setting: EarlyStart, binary: &Path, path: &str) -> String {
    let mut days = String::new();
    for weekday in 1..=5 {
        let _infallible: std::fmt::Result = write!(
            days,
            "    <dict>\n      <key>Weekday</key><integer>{weekday}</integer>\n      \
             <key>Hour</key><integer>{}</integer>\n      \
             <key>Minute</key><integer>{}</integer>\n    </dict>\n",
            setting.hour, setting.minute
        );
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n  \
         <key>Label</key><string>{LABEL}</string>\n  \
         <key>ProgramArguments</key>\n  <array>\n    \
         <string>{}</string>\n    <string>early</string>\n    <string>run</string>\n  \
         </array>\n  \
         <key>EnvironmentVariables</key>\n  <dict>\n    \
         <key>PATH</key><string>{}</string>\n  </dict>\n  \
         <key>StartCalendarInterval</key>\n  <array>\n{days}  </array>\n  \
         <key>StandardOutPath</key><string>/dev/null</string>\n  \
         <key>StandardErrorPath</key><string>/dev/null</string>\n\
         </dict>\n</plist>\n",
        escape(&binary.to_string_lossy()),
        escape(path),
    )
}

/// Writes the agent and loads it, replacing a loaded one.
///
/// # Errors
///
/// Returns [`Error::CreateFolder`] or [`Error::Settings`] when the file
/// cannot be written, and [`Error::Settings`] when `launchctl` refuses it.
pub fn install(home: &Path, setting: EarlyStart, binary: &Path, path: &str) -> Result<(), Error> {
    let file = agent_path(home);
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder).map_err(|source| Error::CreateFolder {
            path: folder.to_path_buf(),
            source,
        })?;
    }
    unload(home)?;
    fs::write(&file, plist(setting, binary, path))
        .map_err(|e| Error::Settings(format!("cannot write {}: {e}", file.display())))?;
    let domain = domain(home)?;
    let loaded = launchctl(&["bootstrap", &domain, &file.to_string_lossy()])?;
    if loaded {
        Ok(())
    } else {
        Err(Error::Settings(format!(
            "launchctl could not load {}",
            file.display()
        )))
    }
}

/// Unloads the agent and deletes its file; nothing happens when there is
/// none. Returns whether a file was deleted.
///
/// # Errors
///
/// Returns [`Error::Settings`] when the file cannot be deleted.
pub fn remove(home: &Path) -> Result<bool, Error> {
    let file = agent_path(home);
    if !file.exists() {
        return Ok(false);
    }
    unload(home)?;
    fs::remove_file(&file)
        .map_err(|e| Error::Settings(format!("cannot delete {}: {e}", file.display())))?;
    Ok(true)
}

/// Unloads the agent if it is loaded.
fn unload(home: &Path) -> Result<(), Error> {
    let target = format!("{}/{LABEL}", domain(home)?);
    // NOTE: no specification governs this: our own design. `bootout` fails
    // when the agent is not loaded, which is the state it is meant to reach.
    let _unloaded: bool = launchctl(&["bootout", &target])?;
    Ok(())
}

/// Returns the user's `launchd` domain, `gui/<uid>`, taking the uid from
/// the owner of `home`.
fn domain(home: &Path) -> Result<String, Error> {
    let uid = fs::metadata(home)
        .map_err(|e| Error::Settings(format!("cannot read {}: {e}", home.display())))?
        .uid();
    Ok(format!("gui/{uid}"))
}

/// Runs `launchctl` and returns whether it succeeded.
fn launchctl(args: &[&str]) -> Result<bool, Error> {
    Command::new("/bin/launchctl")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .map_err(Error::Launch)
}

/// Escapes text for an XML string.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{LABEL, agent_path, plist};
    use crate::early::EarlyStart;
    use std::path::Path;

    #[test]
    fn the_agent_runs_early_run_at_the_set_time_on_workdays() {
        let setting = EarlyStart {
            hour: 5,
            minute: 30,
            accounts: 2,
        };
        let text = plist(
            setting,
            Path::new("/u/.cargo/bin/cc-shift"),
            "/u/bin:/usr/bin",
        );
        assert!(text.contains(&format!("<key>Label</key><string>{LABEL}</string>")));
        assert!(text.contains(
            "<string>/u/.cargo/bin/cc-shift</string>\n    <string>early</string>\n    <string>run</string>"
        ));
        assert!(text.contains("<key>PATH</key><string>/u/bin:/usr/bin</string>"));
        for weekday in 1..=5 {
            assert!(
                text.contains(&format!(
                    "<key>Weekday</key><integer>{weekday}</integer>\n      \
                     <key>Hour</key><integer>5</integer>\n      \
                     <key>Minute</key><integer>30</integer>"
                )),
                "{text}"
            );
        }
        assert!(!text.contains("<integer>6</integer>"), "no Saturday");
        assert!(
            !text.contains("<key>Weekday</key><integer>0</integer>"),
            "no Sunday"
        );
    }

    #[test]
    fn paths_are_escaped() {
        let text = plist(EarlyStart::default(), Path::new("/a&b/cc-shift"), "/x<y");
        assert!(
            text.contains("<string>/a&amp;b/cc-shift</string>"),
            "{text}"
        );
        assert!(text.contains("<string>/x&lt;y</string>"), "{text}");
    }

    #[test]
    fn the_agent_lives_in_the_users_launch_agents() {
        assert_eq!(
            agent_path(Path::new("/u")),
            Path::new("/u/Library/LaunchAgents").join(format!("{LABEL}.plist"))
        );
    }
}
