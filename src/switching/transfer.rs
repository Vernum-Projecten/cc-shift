//! Making an account active by moving its login, never copying it.
//!
//! A refresh token works once, so two live copies of one login can get it
//! revoked. Under the lock a switch saves the login of `~/.claude` back to
//! the account that was active, writes the next account's parked login into
//! `~/.claude`, and clears that parked copy. Each step is recorded in the
//! state before it runs, and every step can run again safely, so a switch
//! that stops halfway is finished by the next one.
//!
//! The save and the write are one step. Run again after a stop between
//! them, it saves `~/.claude` once more, so tokens Claude Code refreshed in
//! the meantime are kept. The same step makes `~/.claude.json` name the
//! next account, which open windows need to follow the switch.

use crate::error::Error;
use crate::login::account::AccountFiles;
use crate::login::{Logins, Slot};
use crate::store::state::{Locked, Pending, State, StateFile, Step};

/// What a switch did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The account was already active; nothing moved.
    AlreadyActive,
    /// The account is now active.
    Switched {
        /// The account that was active before, if `cc-shift` knew one.
        from: Option<String>,
    },
}

/// Makes `to` the active account.
///
/// A switch left unfinished by an earlier run is finished first. When `cc-shift`
/// knows no active account yet, the login now in `~/.claude` belongs to no
/// profile and is replaced.
///
/// # Errors
///
/// Returns [`Error::NoParkedLogin`] or [`Error::Account`] before anything
/// moves when `to` has no parked login or no signed-in account, and the
/// error of a step that fails; the recorded step lets the next switch
/// finish the move.
pub fn switch_to(
    logins: &impl Logins,
    accounts: &AccountFiles,
    file: &StateFile,
    to: &str,
) -> Result<Outcome, Error> {
    let locked = file.lock()?;
    let mut state = locked.load()?;
    finish(logins, accounts, &locked, &mut state)?;
    if state.active.as_deref() == Some(to) {
        return Ok(Outcome::AlreadyActive);
    }
    if logins.read(&Slot::Parked(to.to_owned()))?.is_none() {
        return Err(Error::NoParkedLogin(to.to_owned()));
    }
    accounts.of_profile(to)?;
    let from = state.active.clone();
    state.pending = Some(Pending {
        from: from.clone(),
        to: to.to_owned(),
        step: Step::Move,
    });
    locked.save(&state)?;
    finish(logins, accounts, &locked, &mut state)?;
    Ok(Outcome::Switched { from })
}

/// Runs the remaining steps of a recorded switch, saving the state after
/// each one.
///
/// # Errors
///
/// Returns the state, Keychain or account error of the step that failed.
pub fn finish(
    logins: &impl Logins,
    accounts: &AccountFiles,
    locked: &Locked<'_>,
    state: &mut State,
) -> Result<(), Error> {
    while let Some(pending) = state.pending.clone() {
        let parked = Slot::Parked(pending.to.clone());
        match pending.step {
            Step::Move => {
                let next = logins
                    .read(&parked)?
                    .ok_or_else(|| Error::NoParkedLogin(pending.to.clone()))?;
                let current = logins.read(&Slot::Active)?;
                if current.as_ref() != Some(&next) {
                    if let (Some(from), Some(login)) = (&pending.from, &current) {
                        logins.write(&Slot::Parked(from.clone()), login)?;
                    }
                    logins.write(&Slot::Active, &next)?;
                }
                accounts.activate(&pending.to)?;
                advance(locked, state, Step::ClearParked)?;
            }
            Step::ClearParked => {
                logins.clear(&parked)?;
                state.active = Some(pending.to);
                state.pending = None;
                locked.save(state)?;
            }
        }
    }
    Ok(())
}

/// Records the next step of the pending switch.
fn advance(locked: &Locked<'_>, state: &mut State, step: Step) -> Result<(), Error> {
    if let Some(pending) = &mut state.pending {
        pending.step = step;
    }
    locked.save(state)
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{Outcome, switch_to};
    use crate::error::Error;
    use crate::login::account::AccountFiles;
    use crate::login::{Login, Logins, Slot};
    use crate::store::profile::ProfilesRoot;
    use crate::store::state::StateFile;
    use std::cell::{Cell, RefCell};
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    /// Account files with a signed-in `.claude.json` for profiles `a` and `b`.
    fn accounts(home: &TempDir) -> Result<AccountFiles, Box<dyn std::error::Error>> {
        for name in ["a", "b"] {
            let dir = home.path().join(".claude-profiles").join(name);
            fs::create_dir_all(&dir)?;
            fs::write(
                dir.join(".claude.json"),
                format!(r#"{{"oauthAccount": {{"emailAddress": "{name}@example.org"}}}}"#),
            )?;
        }
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        Ok(AccountFiles::new(home.path(), &root))
    }

    /// Returns the email `~/.claude.json` names.
    fn active_email(home: &TempDir) -> Result<String, Box<dyn std::error::Error>> {
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(home.path().join(".claude.json"))?)?;
        Ok(value["oauthAccount"]["emailAddress"]
            .as_str()
            .unwrap_or_default()
            .to_owned())
    }

    /// An in-memory Keychain that can fail every call after the first `n`,
    /// to stop a switch at any point.
    #[derive(Default)]
    struct Memory {
        slots: RefCell<HashMap<Slot, Login>>,
        calls_left: Cell<Option<usize>>,
    }

    impl Memory {
        fn with(slots: &[(Slot, &str)]) -> Self {
            let memory = Self::default();
            for (slot, content) in slots {
                memory
                    .slots
                    .borrow_mut()
                    .insert(slot.clone(), Login::from_bytes(content.as_bytes().to_vec()));
            }
            memory
        }

        fn call(&self, slot: &Slot) -> Result<(), Error> {
            match self.calls_left.get() {
                Some(0) => Err(Error::LoginStore {
                    slot: slot.to_string(),
                    reason: "stopped by the test".to_owned(),
                }),
                Some(n) => {
                    self.calls_left.set(Some(n - 1));
                    Ok(())
                }
                None => Ok(()),
            }
        }

        fn content(&self, slot: &Slot) -> Option<String> {
            self.slots
                .borrow()
                .get(slot)
                .map(|l| String::from_utf8_lossy(l.as_bytes()).into_owned())
        }

        fn places_of(&self, content: &str) -> usize {
            self.slots
                .borrow()
                .values()
                .filter(|l| l.as_bytes() == content.as_bytes())
                .count()
        }
    }

    impl Logins for Memory {
        fn read(&self, slot: &Slot) -> Result<Option<Login>, Error> {
            self.call(slot)?;
            Ok(self.slots.borrow().get(slot).cloned())
        }

        fn write(&self, slot: &Slot, login: &Login) -> Result<(), Error> {
            self.call(slot)?;
            self.slots.borrow_mut().insert(slot.clone(), login.clone());
            Ok(())
        }

        fn clear(&self, slot: &Slot) -> Result<(), Error> {
            self.call(slot)?;
            self.slots.borrow_mut().remove(slot);
            Ok(())
        }
    }

    fn parked(name: &str) -> Slot {
        Slot::Parked(name.to_owned())
    }

    fn state_file(home: &TempDir) -> Result<StateFile, Error> {
        Ok(StateFile::in_root(&ProfilesRoot::from_home(Some(
            home.path().into(),
        ))?))
    }

    /// Two accounts with `a` active: its live login is in `~/.claude`.
    fn a_active(home: &TempDir) -> Result<(Memory, StateFile), Error> {
        let memory = Memory::with(&[(Slot::Active, "login-a"), (parked("b"), "login-b")]);
        let file = state_file(home)?;
        file.update(|state| state.active = Some("a".to_owned()))?;
        Ok((memory, file))
    }

    #[test]
    fn moves_both_logins() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let (memory, file) = a_active(&home)?;
        let outcome = switch_to(&memory, &accounts(&home)?, &file, "b")?;
        assert_eq!(
            outcome,
            Outcome::Switched {
                from: Some("a".to_owned())
            }
        );
        assert_eq!(memory.content(&Slot::Active).as_deref(), Some("login-b"));
        assert_eq!(memory.content(&parked("a")).as_deref(), Some("login-a"));
        assert_eq!(memory.content(&parked("b")), None);
        assert_eq!(active_email(&home)?, "b@example.org");
        let state = file.read()?;
        assert_eq!(state.active.as_deref(), Some("b"));
        assert_eq!(state.pending, None);
        Ok(())
    }

    #[test]
    fn saves_the_tokens_claude_code_refreshed() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let (memory, file) = a_active(&home)?;
        memory.write(
            &Slot::Active,
            &Login::from_bytes(b"login-a-refreshed".to_vec()),
        )?;
        switch_to(&memory, &accounts(&home)?, &file, "b")?;
        assert_eq!(
            memory.content(&parked("a")).as_deref(),
            Some("login-a-refreshed")
        );
        Ok(())
    }

    #[test]
    fn switching_to_the_active_account_does_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let (memory, file) = a_active(&home)?;
        assert_eq!(
            switch_to(&memory, &accounts(&home)?, &file, "a")?,
            Outcome::AlreadyActive
        );
        assert_eq!(memory.content(&Slot::Active).as_deref(), Some("login-a"));
        Ok(())
    }

    #[test]
    fn an_account_without_a_parked_login_moves_nothing() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let (memory, file) = a_active(&home)?;
        let result = switch_to(&memory, &accounts(&home)?, &file, "c");
        assert!(matches!(result, Err(Error::NoParkedLogin(_))), "{result:?}");
        assert_eq!(memory.content(&Slot::Active).as_deref(), Some("login-a"));
        assert_eq!(file.read()?.pending, None);
        Ok(())
    }

    #[test]
    fn the_first_switch_replaces_the_unowned_login() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let memory = Memory::with(&[(Slot::Active, "login-x"), (parked("b"), "login-b")]);
        let file = state_file(&home)?;
        assert_eq!(
            switch_to(&memory, &accounts(&home)?, &file, "b")?,
            Outcome::Switched { from: None }
        );
        assert_eq!(memory.content(&Slot::Active).as_deref(), Some("login-b"));
        assert_eq!(memory.places_of("login-x"), 0);
        Ok(())
    }

    #[test]
    fn a_refresh_during_a_stopped_switch_is_kept() -> Result<(), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let (memory, file) = a_active(&home)?;
        // Stops after the old login is saved and before the new one is
        // written: the pre-check read, two reads, and the save.
        memory.calls_left.set(Some(4));
        assert!(
            switch_to(&memory, &accounts(&home)?, &file, "b").is_err(),
            "the switch stops"
        );
        memory.calls_left.set(None);
        memory.write(
            &Slot::Active,
            &Login::from_bytes(b"login-a-refreshed".to_vec()),
        )?;
        switch_to(&memory, &accounts(&home)?, &file, "b")?;
        assert_eq!(
            memory.content(&parked("a")).as_deref(),
            Some("login-a-refreshed")
        );
        assert_eq!(memory.content(&Slot::Active).as_deref(), Some("login-b"));
        Ok(())
    }

    #[test]
    fn a_switch_stopped_at_any_call_is_finished_by_the_next_run()
    -> Result<(), Box<dyn std::error::Error>> {
        for stop_after in 0..8 {
            let home = TempDir::new()?;
            let (memory, file) = a_active(&home)?;
            memory.calls_left.set(Some(stop_after));
            let first = switch_to(&memory, &accounts(&home)?, &file, "b");
            memory.calls_left.set(None);
            let second = switch_to(&memory, &accounts(&home)?, &file, "b")?;
            if first.is_ok() {
                assert_eq!(second, Outcome::AlreadyActive, "stop after {stop_after}");
            }
            assert_eq!(
                memory.content(&Slot::Active).as_deref(),
                Some("login-b"),
                "stop after {stop_after}"
            );
            for login in ["login-a", "login-b"] {
                assert_eq!(
                    memory.places_of(login),
                    1,
                    "{login} after a stop after {stop_after}"
                );
            }
            assert_eq!(file.read()?.pending, None, "stop after {stop_after}");
        }
        Ok(())
    }
}
