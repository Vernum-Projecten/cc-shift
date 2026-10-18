// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The scheduled job of the early start, which older versions installed.
//!
//! The early start was a job that started some accounts' 5-hour windows
//! before the workday: a `launchd` agent on macOS, a systemd user timer on
//! Linux, and a Task Scheduler task on Windows. It is removed, and
//! `cc-shift setup` and `cc-shift uninstall` delete a job an older version
//! left, so nothing runs a command that no longer exists.

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(windows)]
pub mod windows;

use std::path::{Path, PathBuf};

use crate::error::Error;

/// Removes the job and the empty folder it ran `claude` from; returns
/// whether there was a job.
///
/// # Errors
///
/// Returns the error of the job's file or of the system scheduler.
pub fn remove(home: &Path) -> Result<bool, Error> {
    #[cfg(target_os = "macos")]
    let removed = macos::launchd::remove(home)?;
    #[cfg(target_os = "linux")]
    let removed = linux::systemd::remove(home)?;
    #[cfg(windows)]
    let removed = windows::task::remove(home)?;
    let folder = workdir(home);
    match std::fs::remove_dir_all(&folder) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(Error::Settings(format!(
                "cannot delete {}: {e}",
                folder.display()
            )));
        }
    }
    Ok(removed)
}

/// Returns the file of the job: the `launchd` agent on macOS, the systemd
/// timer on Linux, and the imported task definition on Windows.
#[must_use]
pub fn job_path(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    return macos::launchd::agent_path(home);
    #[cfg(target_os = "linux")]
    return linux::systemd::timer_path(home);
    #[cfg(windows)]
    return windows::task::task_path(home);
}

/// Returns whether a job an older version installed is still there.
#[must_use]
pub fn installed(home: &Path) -> bool {
    job_path(home).is_file()
}

/// Returns the empty folder the early start ran `claude` from.
fn workdir(home: &Path) -> PathBuf {
    #[cfg(target_os = "macos")]
    return home.join("Library/Caches/cc-shift/early-start");
    #[cfg(target_os = "linux")]
    {
        std::env::var_os("XDG_CACHE_HOME")
            .filter(|v| !v.is_empty())
            .map_or_else(|| home.join(".cache"), PathBuf::from)
            .join("cc-shift/early-start")
    }
    #[cfg(windows)]
    return windows::task::data_folder(home).join("early-start");
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{installed, job_path, remove, workdir};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn nothing_to_remove_is_no_job() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        assert!(!installed(home.path()));
        assert!(!remove(home.path())?, "no job");
        Ok(())
    }

    #[test]
    fn a_left_job_and_its_folder_are_removed() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let job = job_path(home.path());
        fs::create_dir_all(job.parent().ok_or("a parent")?)?;
        fs::write(&job, "left by an older version")?;
        fs::create_dir_all(workdir(home.path()))?;
        assert!(installed(home.path()));
        assert!(remove(home.path())?, "a job");
        assert!(!job.exists(), "job deleted");
        assert!(!workdir(home.path()).exists(), "folder deleted");
        Ok(())
    }
}
