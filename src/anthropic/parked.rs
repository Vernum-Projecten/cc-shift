//! Keeping parked accounts ready: refreshing their logins and reading their
//! usage.
//!
//! A parked login has no Claude Code process to refresh it, so `cc-shift` does,
//! under the lock for the whole request: a refresh token works once, and a
//! switch must never move a login whose refresh is under way. A usage read
//! only needs the access token, so it holds the lock just to read the token
//! and to record the result. The active login is never touched here;
//! Claude Code refreshes it and the status line reports its usage.

use std::fs::File;

use crate::anthropic::http::Http;
use crate::anthropic::oauth::{self, Stored};
use crate::error::Error;
use crate::login::{Logins, Slot};
use crate::store::history::{self, Event};
use crate::store::profile::ProfilesRoot;
use crate::store::state::StateFile;

/// A parked account's usage is read again when it is older than this.
pub const STALE_SECONDS: i64 = 5 * 60;

/// What happened to one parked account.
#[derive(Debug)]
pub struct Report {
    /// The profile name.
    pub name: String,
    /// Whether its login was refreshed.
    pub refreshed: bool,
    /// The outcome of reading its usage; `Ok(false)` when it was fresh
    /// enough to skip.
    pub usage: Result<bool, Error>,
}

/// Refreshes every parked login that is close to expiry and reads the usage
/// of every account whose numbers are stale or unconfirmed, the active one
/// included: its access token is read, never refreshed.
///
/// Only one run happens at a time: a run that finds another one busy
/// returns no reports.
///
/// # Errors
///
/// Returns the error of the profiles root or the state file; a problem with
/// one account is in its report.
pub fn refresh_all(
    logins: &impl Logins,
    http: &impl Http,
    file: &StateFile,
    root: &ProfilesRoot,
    now: i64,
) -> Result<Vec<Report>, Error> {
    let Some(_running) = single_run(root)? else {
        return Ok(Vec::new());
    };
    let mut reports = Vec::new();
    for name in root.list()? {
        let refreshed = match refresh_if_due(logins, http, file, root, &name, now) {
            Ok(refreshed) => refreshed,
            Err(error) => {
                reports.push(Report {
                    name,
                    refreshed: false,
                    usage: Err(error),
                });
                continue;
            }
        };
        let usage = read_usage_if_stale(logins, http, file, root, &name, now);
        reports.push(Report {
            name,
            refreshed,
            usage,
        });
    }
    Ok(reports)
}

/// Returns whether any account's usage is stale or not yet confirmed with
/// its own token, so the status line knows to start a run.
///
/// # Errors
///
/// Returns the error of the profiles root or the state file.
pub fn any_stale(file: &StateFile, root: &ProfilesRoot, now: i64) -> Result<bool, Error> {
    let state = file.read()?;
    Ok(root.list()?.iter().any(|name| {
        !state.logged_out.contains(name)
            && state
                .usage
                .get(name)
                .filter(|r| r.confirmed)
                .and_then(|r| r.read_at)
                .is_none_or(|at| now - at >= STALE_SECONDS)
    }))
}

/// Refreshes a parked login when its access token is close to expiry, with
/// the lock held for the whole request. Returns whether it refreshed.
fn refresh_if_due(
    logins: &impl Logins,
    http: &impl Http,
    file: &StateFile,
    root: &ProfilesRoot,
    name: &str,
    now: i64,
) -> Result<bool, Error> {
    let locked = file.lock()?;
    let mut state = locked.load()?;
    if state.active.as_deref() == Some(name) || state.pending.is_some() {
        return Ok(false);
    }
    let slot = Slot::Parked(name.to_owned());
    let Some(login) = logins.read(&slot)? else {
        return Err(Error::NoParkedLogin(name.to_owned()));
    };
    let stored = Stored::of(&login)?;
    if !stored.expires_soon(now) {
        return Ok(false);
    }
    let response = http.send(&stored.refresh_request())?;
    match oauth::apply_refresh(name, &login, &response, now) {
        Ok(new) => {
            logins.write(&slot, &new)?;
            history::note(
                root,
                Event::Refresh {
                    account: name.to_owned(),
                },
                now,
            );
            if state.logged_out.remove(name) {
                locked.save(&state)?;
            }
            Ok(true)
        }
        Err(Error::LoggedOut(who)) => {
            let first = state.logged_out.insert(who.clone());
            locked.save(&state)?;
            if first {
                history::note(
                    root,
                    Event::LoggedOut {
                        account: who.clone(),
                    },
                    now,
                );
            }
            Err(Error::LoggedOut(who))
        }
        Err(other) => Err(other),
    }
}

/// Reads a parked account's usage when its numbers are stale. Returns
/// whether it read.
fn read_usage_if_stale(
    logins: &impl Logins,
    http: &impl Http,
    file: &StateFile,
    root: &ProfilesRoot,
    name: &str,
    now: i64,
) -> Result<bool, Error> {
    let stored = {
        let locked = file.lock()?;
        let state = locked.load()?;
        let fresh = state
            .usage
            .get(name)
            .filter(|r| r.confirmed)
            .and_then(|r| r.read_at)
            .is_some_and(|at| now - at < STALE_SECONDS);
        if fresh {
            return Ok(false);
        }
        let active = state.active.as_deref() == Some(name);
        let slot = if active {
            Slot::Active
        } else {
            Slot::Parked(name.to_owned())
        };
        let login = logins
            .read(&slot)?
            .ok_or_else(|| Error::NoParkedLogin(name.to_owned()))?;
        (Stored::of(&login)?, active)
    };
    let (stored, active) = stored;
    let response = http.send(&stored.usage_request())?;
    let recorded = match oauth::parse_usage(name, &response, now) {
        Ok(recorded) => recorded,
        Err(Error::LoggedOut(who)) if !active => {
            if file.update(|s| s.logged_out.insert(who.clone()))? {
                history::note(
                    root,
                    Event::LoggedOut {
                        account: who.clone(),
                    },
                    now,
                );
            }
            return Err(Error::LoggedOut(who));
        }
        Err(other) => return Err(other),
    };
    file.update(|s| {
        s.logged_out.remove(name);
        s.usage.insert(name.to_owned(), recorded);
    })?;
    Ok(true)
}

/// Takes the lock that keeps runs from overlapping; `None` when another run
/// holds it.
fn single_run(root: &ProfilesRoot) -> Result<Option<File>, Error> {
    let path = root.path().join(".refresh-lock");
    std::fs::create_dir_all(root.path()).map_err(|e| state_error(root.path(), e))?;
    let lock = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| state_error(&path, e))?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(lock)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(state_error(&path, e)),
    }
}

fn state_error(path: &std::path::Path, source: std::io::Error) -> Error {
    Error::State {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{STALE_SECONDS, any_stale, refresh_all};
    use crate::anthropic::http::{Http, Request, Response};
    use crate::error::Error;
    use crate::login::{Login, Logins, Slot};
    use crate::store::profile::ProfilesRoot;
    use crate::store::state::{Recorded, StateFile};
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    const NOW: i64 = 1_800_000_000;

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

    /// Answers the token endpoint with `token` and the usage endpoint with
    /// `usage`, and records the URLs it was asked for.
    struct Fake {
        token: Response,
        usage: Response,
        asked: RefCell<Vec<String>>,
    }

    impl Http for Fake {
        fn send(&self, request: &Request) -> Result<Response, Error> {
            self.asked.borrow_mut().push(request.url.clone());
            Ok(if request.url.contains("oauth/token") {
                self.token.clone()
            } else {
                self.usage.clone()
            })
        }
    }

    fn response(status: u16, body: &Value) -> Response {
        Response {
            status,
            body: body.to_string().into_bytes(),
        }
    }

    fn fake() -> Fake {
        Fake {
            token: response(
                200,
                &json!({"access_token": "new", "refresh_token": "new-r", "expires_in": 28_800}),
            ),
            usage: response(
                200,
                &json!({"five_hour": {"utilization": 12.0, "resets_at": "2027-01-15T10:00:00Z"}}),
            ),
            asked: RefCell::new(Vec::new()),
        }
    }

    fn login(expires_in: i64) -> Login {
        Login::from_bytes(
            json!({"claudeAiOauth": {
                "accessToken": "old", "refreshToken": "old-r",
                "expiresAt": (NOW + expires_in) * 1000, "scopes": ["user:inference"]
            }})
            .to_string()
            .into_bytes(),
        )
    }

    /// Profiles `a` (active) and `b` (parked, its token expiring in
    /// `expires_in` seconds).
    fn world(
        expires_in: i64,
    ) -> Result<(TempDir, Memory, StateFile, ProfilesRoot), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        for name in ["a", "b"] {
            fs::create_dir_all(home.path().join(".claude-profiles").join(name))?;
        }
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        let file = StateFile::in_root(&root);
        file.update(|s| s.active = Some("a".to_owned()))?;
        let memory = Memory::default();
        memory.write(&Slot::Active, &login(3600))?;
        memory.write(&Slot::Parked("b".to_owned()), &login(expires_in))?;
        Ok((home, memory, file, root))
    }

    fn parked_access(memory: &Memory) -> Result<String, Box<dyn std::error::Error>> {
        let login = memory
            .read(&Slot::Parked("b".to_owned()))?
            .ok_or("parked")?;
        let value: Value = serde_json::from_slice(login.as_bytes())?;
        Ok(value["claudeAiOauth"]["accessToken"]
            .as_str()
            .unwrap_or_default()
            .to_owned())
    }

    #[test]
    fn a_parked_login_close_to_expiry_is_refreshed() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(60)?;
        let reports = refresh_all(&memory, &fake(), &file, &root, NOW)?;
        let report = |name: &str| reports.iter().find(|r| r.name == name);
        assert!(report("b").is_some_and(|r| r.refreshed), "{reports:?}");
        assert!(report("a").is_some_and(|r| !r.refreshed), "{reports:?}");
        assert_eq!(parked_access(&memory)?, "new");
        Ok(())
    }

    #[test]
    fn a_login_with_time_left_is_not_refreshed() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(3600)?;
        let http = fake();
        refresh_all(&memory, &http, &file, &root, NOW)?;
        assert_eq!(parked_access(&memory)?, "old");
        assert!(
            !http
                .asked
                .borrow()
                .iter()
                .any(|u| u.contains("oauth/token")),
            "no refresh request"
        );
        Ok(())
    }

    #[test]
    fn the_active_login_is_never_touched() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(60)?;
        memory.write(&Slot::Active, &login(60))?;
        refresh_all(&memory, &fake(), &file, &root, NOW)?;
        let active = memory.read(&Slot::Active)?.ok_or("active")?;
        assert_eq!(active, login(60), "never refreshed or changed");
        assert!(
            file.read()?.usage.get("a").is_some_and(|r| r.confirmed),
            "its usage is read, with its own token"
        );
        Ok(())
    }

    #[test]
    fn usage_of_a_parked_account_is_recorded() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(3600)?;
        refresh_all(&memory, &fake(), &file, &root, NOW)?;
        let state = file.read()?;
        let b = state.usage.get("b").ok_or("b recorded")?;
        assert_eq!(b.five_hour.map(|w| w.used_percent), Some(12.0));
        assert_eq!(b.read_at, Some(NOW));
        Ok(())
    }

    #[test]
    fn fresh_usage_is_not_read_again() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(3600)?;
        file.update(|s| {
            for name in ["a", "b"] {
                s.usage.insert(
                    name.to_owned(),
                    Recorded {
                        read_at: Some(NOW - 60),
                        confirmed: true,
                        ..Recorded::default()
                    },
                );
            }
        })?;
        let http = fake();
        refresh_all(&memory, &http, &file, &root, NOW)?;
        assert!(http.asked.borrow().is_empty(), "{:?}", http.asked.borrow());
        assert!(!any_stale(&file, &root, NOW)?, "nothing stale");
        assert!(any_stale(&file, &root, NOW + STALE_SECONDS)?, "stale later");
        Ok(())
    }

    #[test]
    fn a_refused_refresh_marks_the_account_logged_out() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(60)?;
        let http = Fake {
            token: response(400, &json!({"error": "invalid_grant"})),
            ..fake()
        };
        let reports = refresh_all(&memory, &http, &file, &root, NOW)?;
        assert!(
            matches!(
                reports.iter().find(|r| r.name == "b").map(|r| &r.usage),
                Some(Err(Error::LoggedOut(_)))
            ),
            "{reports:?}"
        );
        assert!(file.read()?.logged_out.contains("b"), "marked");
        assert_eq!(parked_access(&memory)?, "old", "item unchanged");
        Ok(())
    }

    #[test]
    fn a_failing_server_leaves_the_login_as_it_was() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = world(60)?;
        let http = Fake {
            token: response(503, &json!({})),
            ..fake()
        };
        refresh_all(&memory, &http, &file, &root, NOW)?;
        assert_eq!(parked_access(&memory)?, "old");
        assert!(!file.read()?.logged_out.contains("b"), "not logged out");
        Ok(())
    }
}
