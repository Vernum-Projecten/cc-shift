//! `cc-shift remove <name>`: deletes a profile and its Keychain login.

use std::ffi::OsStr;
use std::fs;
use std::io::BufRead;

use crate::command::report;
use crate::error::Error;
use crate::login::SystemLogins;
use crate::login::{Logins, Slot};
use crate::store::profile::{ProfileName, ProfilesRoot};
use crate::store::state::StateFile;

/// What a removal found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Removed {
    /// Whether the profile had a parked login in the Keychain.
    pub had_login: bool,
}

/// Deletes profile `name`: its parked Keychain login, its folder and what
/// the state remembers about it.
///
/// # Errors
///
/// Returns [`Error::NoProfile`] when the profile does not exist,
/// [`Error::RemoveActive`] when it is the active account, and the Keychain
/// or file error that stopped the removal.
pub fn remove_profile(
    logins: &impl Logins,
    file: &StateFile,
    root: &ProfilesRoot,
    name: &ProfileName,
) -> Result<Removed, Error> {
    let dir = root.profile_dir(name);
    if !dir.is_dir() {
        return Err(Error::NoProfile(name.as_str().to_owned()));
    }
    let locked = file.lock()?;
    let mut state = locked.load()?;
    if state.active.as_deref() == Some(name.as_str()) {
        return Err(Error::RemoveActive(name.as_str().to_owned()));
    }
    let slot = Slot::Parked(name.as_str().to_owned());
    let had_login = logins.read(&slot)?.is_some();
    logins.clear(&slot)?;
    fs::remove_dir_all(&dir).map_err(|source| Error::State {
        path: dir.clone(),
        source,
    })?;
    state.usage.remove(name.as_str());
    state.logged_out.remove(name.as_str());
    locked.save(&state)?;
    Ok(Removed { had_login })
}

/// Asks for confirmation, then removes the profile and says what went.
///
/// # Errors
///
/// Returns the error of an invalid name, a missing `HOME`, or the removal.
pub fn run(raw: &OsStr) -> Result<(), Error> {
    let name = ProfileName::parse(raw)?;
    let root = ProfilesRoot::from_env()?;
    if !root.profile_dir(&name).is_dir() {
        return Err(Error::NoProfile(name.as_str().to_owned()));
    }
    report(&format!(
        "this deletes the profile '{}' and its stored login. Type yes to go on:",
        name.as_str()
    ));
    let mut answer = String::new();
    let _read: std::io::Result<usize> = std::io::stdin().lock().read_line(&mut answer);
    if answer.trim() != "yes" {
        report("nothing removed");
        return Ok(());
    }
    let logins = SystemLogins::for_user(&root)?;
    let removed = remove_profile(&logins, &StateFile::in_root(&root), &root, &name)?;
    if removed.had_login {
        report(&format!("removed '{}' and its login", name.as_str()));
    } else {
        report(&format!(
            "removed '{}'; it had no stored login",
            name.as_str()
        ));
    }
    Ok(())
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::remove_profile;
    use crate::error::Error;
    use crate::login::{Login, Logins, Slot};
    use crate::store::profile::{ProfileName, ProfilesRoot};
    use crate::store::state::{Recorded, StateFile};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::fs;
    use tempfile::TempDir;

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

    fn setup() -> Result<(TempDir, Memory, StateFile, ProfilesRoot), Box<dyn std::error::Error>> {
        let home = TempDir::new()?;
        let root = ProfilesRoot::from_home(Some(home.path().into()))?;
        for name in ["work", "spare"] {
            fs::create_dir_all(root.path().join(name))?;
        }
        let memory = Memory::default();
        memory.write(
            &Slot::Parked("spare".to_owned()),
            &Login::from_bytes(b"login".to_vec()),
        )?;
        let file = StateFile::in_root(&root);
        file.update(|s| {
            s.active = Some("work".to_owned());
            s.usage.insert("spare".to_owned(), Recorded::default());
        })?;
        Ok((home, memory, file, root))
    }

    fn name(raw: &str) -> Result<ProfileName, Error> {
        ProfileName::parse(OsStr::new(raw))
    }

    #[test]
    fn removes_the_folder_the_login_and_the_state() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = setup()?;
        let removed = remove_profile(&memory, &file, &root, &name("spare")?)?;
        assert!(removed.had_login, "had a login");
        assert!(!root.path().join("spare").exists(), "folder gone");
        assert_eq!(memory.read(&Slot::Parked("spare".to_owned()))?, None);
        assert!(!file.read()?.usage.contains_key("spare"), "state cleaned");
        Ok(())
    }

    #[test]
    fn refuses_the_active_account() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = setup()?;
        let result = remove_profile(&memory, &file, &root, &name("work")?);
        assert!(matches!(result, Err(Error::RemoveActive(_))), "{result:?}");
        assert!(root.path().join("work").is_dir(), "kept");
        Ok(())
    }

    #[test]
    fn reports_a_missing_profile() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = setup()?;
        let result = remove_profile(&memory, &file, &root, &name("other")?);
        assert!(matches!(result, Err(Error::NoProfile(_))), "{result:?}");
        Ok(())
    }

    #[test]
    fn says_when_there_was_no_login() -> Result<(), Box<dyn std::error::Error>> {
        let (_home, memory, file, root) = setup()?;
        memory.clear(&Slot::Parked("spare".to_owned()))?;
        let removed = remove_profile(&memory, &file, &root, &name("spare")?)?;
        assert!(!removed.had_login, "no login");
        Ok(())
    }
}
