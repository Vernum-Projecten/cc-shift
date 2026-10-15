// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The Task Scheduler task that runs `cc-shift early run` each workday.
//!
//! The task is imported from XML with `schtasks /Create /XML`, which a user
//! may do for a task that runs as themselves
//! (<https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/schtasks-create>).
//! `StartWhenAvailable` starts a run missed while the computer was off or
//! asleep as soon as it can
//! (<https://learn.microsoft.com/en-us/windows/win32/taskschd/taskschedulerschema-startwhenavailable-settingstype-element>).
//! The imported XML is kept as the record that the task is installed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::claude_code::launch::{system32, without_window};
use crate::early::EarlyStart;
use crate::error::Error;

/// The name of the task in Task Scheduler.
pub const TASK: &str = "cc-shift early start";

/// Returns the `cc-shift` folder of the user's local application data under
/// `home`: `%LOCALAPPDATA%\cc-shift`.
#[must_use]
pub fn data_folder(home: &Path) -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .filter(|v| !v.is_empty())
        .map_or_else(|| home.join("AppData").join("Local"), PathBuf::from)
        .join("cc-shift")
}

/// Returns the path of the XML the task was imported from.
#[must_use]
pub fn task_path(home: &Path) -> PathBuf {
    data_folder(home).join("early-start.xml")
}

/// Returns the task definition: `binary early run` Monday to Friday at the
/// set time, as the user who is logged on, with the least privilege.
#[must_use]
pub fn task_xml(setting: EarlyStart, binary: &Path, user: Option<&str>) -> String {
    let user_id = user.map_or_else(String::new, |u| {
        format!("      <UserId>{}</UserId>\n", escape(u))
    });
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n\
         <Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n\
         \x20 <RegistrationInfo>\n\
         \x20   <Description>cc-shift early start: start 5-hour windows before the workday</Description>\n\
         \x20 </RegistrationInfo>\n\
         \x20 <Triggers>\n\
         \x20   <CalendarTrigger>\n\
         \x20     <StartBoundary>2026-01-01T{at}:00</StartBoundary>\n\
         \x20     <Enabled>true</Enabled>\n\
         \x20     <ScheduleByWeek>\n\
         \x20       <DaysOfWeek><Monday /><Tuesday /><Wednesday /><Thursday /><Friday /></DaysOfWeek>\n\
         \x20       <WeeksInterval>1</WeeksInterval>\n\
         \x20     </ScheduleByWeek>\n\
         \x20   </CalendarTrigger>\n\
         \x20 </Triggers>\n\
         \x20 <Principals>\n\
         \x20   <Principal id=\"Author\">\n\
         {user_id}\
         \x20     <LogonType>InteractiveToken</LogonType>\n\
         \x20     <RunLevel>LeastPrivilege</RunLevel>\n\
         \x20   </Principal>\n\
         \x20 </Principals>\n\
         \x20 <Settings>\n\
         \x20   <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>\n\
         \x20   <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>\n\
         \x20   <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>\n\
         \x20   <StartWhenAvailable>true</StartWhenAvailable>\n\
         \x20   <ExecutionTimeLimit>PT15M</ExecutionTimeLimit>\n\
         \x20   <Enabled>true</Enabled>\n\
         \x20 </Settings>\n\
         \x20 <Actions Context=\"Author\">\n\
         \x20   <Exec>\n\
         \x20     <Command>{command}</Command>\n\
         \x20     <Arguments>early run</Arguments>\n\
         \x20   </Exec>\n\
         \x20 </Actions>\n\
         </Task>\n",
        at = setting.at(),
        command = escape(&binary.to_string_lossy()),
    )
}

/// Writes the task definition and imports it, replacing an earlier task.
///
/// # Errors
///
/// Returns [`Error::CreateFolder`] or [`Error::Settings`] when the
/// definition cannot be written or Task Scheduler refuses it.
pub fn install(home: &Path, setting: EarlyStart, binary: &Path) -> Result<(), Error> {
    let folder = data_folder(home);
    fs::create_dir_all(&folder).map_err(|source| Error::CreateFolder {
        path: folder.clone(),
        source,
    })?;
    let file = task_path(home);
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(domain), Ok(name)) => Some(format!("{domain}\\{name}")),
        (_, Ok(name)) => Some(name),
        _ => None,
    };
    let xml = task_xml(setting, binary, user.as_deref());
    fs::write(&file, utf16_with_bom(&xml))
        .map_err(|e| Error::Settings(format!("cannot write {}: {e}", file.display())))?;
    let file_arg = file.to_string_lossy();
    if schtasks(&["/Create", "/TN", TASK, "/XML", &file_arg, "/F"])? {
        Ok(())
    } else {
        Err(Error::Settings(format!(
            "Task Scheduler did not accept the task from {}",
            file.display()
        )))
    }
}

/// Deletes the task and its definition; nothing happens when there is none.
/// Returns whether one was removed.
///
/// # Errors
///
/// Returns [`Error::Settings`] when the definition cannot be deleted.
pub fn remove(home: &Path) -> Result<bool, Error> {
    let file = task_path(home);
    if !file.exists() {
        return Ok(false);
    }
    // NOTE: no specification governs this: our own design. A task that is
    // already gone from Task Scheduler leaves only its definition to delete.
    let _deleted: bool = schtasks(&["/Delete", "/TN", TASK, "/F"]).unwrap_or(false);
    fs::remove_file(&file)
        .map_err(|e| Error::Settings(format!("cannot delete {}: {e}", file.display())))?;
    Ok(true)
}

/// Runs `schtasks.exe` from the system folder without a console window and
/// returns whether it succeeded.
///
/// # Errors
///
/// Returns [`Error::Settings`] when `SystemRoot` is unset, so the system
/// folder is unknown, and [`Error::Launch`] when `schtasks.exe` cannot run.
fn schtasks(args: &[&str]) -> Result<bool, Error> {
    let program = system32()
        .ok_or_else(|| {
            Error::Settings(
                "SystemRoot is not set, so schtasks.exe cannot be found; open a new terminal"
                    .to_owned(),
            )
        })?
        .join("schtasks.exe");
    without_window(&mut Command::new(program))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .map_err(Error::Launch)
}

/// Encodes `text` as UTF-16 little-endian with a byte order mark, the form
/// the XML declaration names.
fn utf16_with_bom(text: &str) -> Vec<u8> {
    std::iter::once(0xFEFF_u16)
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Escapes text for an XML element.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::{task_xml, utf16_with_bom};
    use crate::early::EarlyStart;
    use std::path::Path;

    #[test]
    fn the_task_runs_early_run_on_workdays_and_catches_up() {
        let setting = EarlyStart {
            hour: 5,
            minute: 30,
            accounts: 2,
        };
        let xml = task_xml(
            setting,
            Path::new(r"C:\Users\u\.cargo\bin\cc-shift.exe"),
            Some(r"PC\u"),
        );
        for part in [
            "<StartBoundary>2026-01-01T05:30:00</StartBoundary>",
            "<DaysOfWeek><Monday /><Tuesday /><Wednesday /><Thursday /><Friday /></DaysOfWeek>",
            "<StartWhenAvailable>true</StartWhenAvailable>",
            "<LogonType>InteractiveToken</LogonType>",
            "<RunLevel>LeastPrivilege</RunLevel>",
            r"<UserId>PC\u</UserId>",
            r"<Command>C:\Users\u\.cargo\bin\cc-shift.exe</Command>",
            "<Arguments>early run</Arguments>",
        ] {
            assert!(xml.contains(part), "{part} in {xml}");
        }
        assert!(!xml.contains("Saturday") && !xml.contains("Sunday"));
    }

    #[test]
    fn the_definition_is_utf16_with_a_byte_order_mark() {
        assert_eq!(utf16_with_bom("<a"), [0xFF, 0xFE, b'<', 0, b'a', 0]);
    }

    #[test]
    fn paths_are_escaped() {
        let xml = task_xml(
            EarlyStart::default(),
            Path::new(r"C:\a&b\cc-shift.exe"),
            None,
        );
        assert!(
            xml.contains(r"<Command>C:\a&amp;b\cc-shift.exe</Command>"),
            "{xml}"
        );
        assert!(!xml.contains("<UserId>"));
    }
}
