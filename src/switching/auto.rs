// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Switching on its own: when to leave the active account, and where to go.
//!
//! The status line switches when a reading proven to be the active
//! account's reaches the threshold, the rate-limit hook when a turn hits a
//! limit, and `cc-shift setup` makes the best account active. No automatic
//! switch follows another within [`SETTLE_SECONDS`].

use crate::error::Error;
use crate::login::Logins;
use crate::login::account::AccountFiles;
use crate::store::history::{self, Event, Reason, WindowKind};
use crate::store::profile::ProfilesRoot;
use crate::store::state::{Recorded, StateFile};
use crate::switching::transfer::{self, Outcome};
use crate::usage::select::{self, Choice};
use crate::usage::{Account, FIVE_HOURS, Usage, Window};

/// How long after a switch no automatic switch follows.
///
/// The status line still reports a window's last response, made before the
/// switch, and a second window's rate-limit hook may belong to the account
/// just left.
pub const SETTLE_SECONDS: i64 = 5 * 60;

/// Why `cc-shift` considers a switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trigger {
    /// The status line recorded new usage for the active account.
    Usage,
    /// A turn ended on a usage limit.
    LimitHit,
    /// `cc-shift setup` runs and no account is active yet.
    Setup,
}

/// Everything a switch reaches.
#[derive(Debug)]
pub struct Context<'a, L: Logins> {
    /// The Keychain.
    pub logins: &'a L,
    /// The `.claude.json` files.
    pub accounts: &'a AccountFiles,
    /// The state file.
    pub file: &'a StateFile,
    /// The profiles root.
    pub root: &'a ProfilesRoot,
}

/// Switches when `trigger` calls for it, and returns the account made
/// active, if any.
///
/// A profile that turns out to have no parked login or no signed-in account
/// is skipped and the next one is tried. With no account available nothing
/// changes.
///
/// # Errors
///
/// Returns the state, Keychain or account error that stopped a switch.
pub fn run<L: Logins>(
    context: &Context<'_, L>,
    trigger: Trigger,
    now: i64,
) -> Result<Option<String>, Error> {
    let state = context.file.read()?;
    let leaving = state.active.clone();
    match (trigger, &leaving) {
        (Trigger::Setup, Some(_)) | (Trigger::Usage | Trigger::LimitHit, None) => {
            return Ok(None);
        }
        (Trigger::LimitHit, Some(active)) => {
            if state
                .switched_at
                .is_some_and(|at| now - at < SETTLE_SECONDS)
            {
                return Ok(None);
            }
            mark_full(context.file, active, now)?;
        }
        (Trigger::Usage, Some(active)) => {
            if state
                .switched_at
                .is_some_and(|at| now - at < SETTLE_SECONDS)
            {
                return Ok(None);
            }
            let profiles = context.root.list()?;
            let view = crate::usage::accounts(&profiles, &state);
            let full = view
                .iter()
                .find(|a| &a.name == active)
                .and_then(|a| a.full_at(now));
            if full != Some(true) {
                return Ok(None);
            }
        }
        (Trigger::Setup, None) => {}
    }
    let state = context.file.read()?;
    let mut view = crate::usage::accounts(&context.root.list()?, &state);
    loop {
        let name = match select::next(&view, leaving.as_deref(), now) {
            Choice::Account(account) => account.name.clone(),
            Choice::NoneAvailable(first) => {
                if trigger != Trigger::Setup && state.all_full_since.is_none() {
                    context.file.update(|s| s.all_full_since = Some(now))?;
                    if let Some(active) = &leaving {
                        history::note(
                            context.root,
                            Event::AllFull {
                                staying_on: active.clone(),
                                first_free: first.map(|(a, _)| a.name.clone()),
                                free_at: first.map(|(_, at)| at),
                            },
                            now,
                        );
                    }
                }
                return Ok(None);
            }
        };
        match transfer::switch_to(context.logins, context.accounts, context.file, &name) {
            Ok(Outcome::Switched { .. }) => {
                context.file.update(|s| {
                    s.switched_at = Some(now);
                    s.all_full_since = None;
                })?;
                if trigger != Trigger::Setup {
                    let left = view.iter().find(|a| Some(&a.name) == leaving.as_ref());
                    let (window, from_used) = left.map_or((None, None), |a| full_window(a, now));
                    let to_used = view
                        .iter()
                        .find(|a| a.name == name)
                        .and_then(|a| a.usage.used_at(now));
                    history::note(
                        context.root,
                        Event::Switch {
                            reason: if trigger == Trigger::LimitHit {
                                Reason::LimitHit
                            } else {
                                Reason::Threshold
                            },
                            window,
                            from: leaving.clone(),
                            to: name.clone(),
                            from_used,
                            to_used,
                        },
                        now,
                    );
                }
                return Ok(Some(name));
            }
            Ok(Outcome::AlreadyActive) => return Ok(None),
            Err(Error::NoParkedLogin(_) | Error::Account(_)) => {
                for account in view.iter_mut().filter(|a| a.name == name) {
                    account.logged_in = false;
                }
            }
            Err(other) => return Err(other),
        }
    }
}

/// Returns the window of `account` that reached its threshold and how full it
/// is, the 5-hour window first.
fn full_window(account: &Account, now: i64) -> (Option<WindowKind>, Option<f64>) {
    let Usage::Known {
        five_hour,
        seven_day,
    } = &account.usage
    else {
        return (None, None);
    };
    let limits = account.thresholds;
    let used = |w: &Option<Window>| w.map(|w| w.used_at(now));
    if used(five_hour).is_some_and(|u| u >= limits.five_hour) {
        (Some(WindowKind::FiveHour), used(five_hour))
    } else if used(seven_day).is_some_and(|u| u >= limits.seven_day) {
        (Some(WindowKind::SevenDay), used(seven_day))
    } else {
        (None, account.usage.used_at(now))
    }
}

/// Records the active account as full: its 5-hour window at 100%, or its
/// week when only the week is at or past its threshold. A window with no
/// reading becomes a 5-hour window from now.
///
/// The 5-hour window runs out far more often, and a week at 85% with a
/// 5-hour window at 80% is a 5-hour limit, so the percentages alone do not
/// tell which limit was hit.
fn mark_full(file: &StateFile, active: &str, now: i64) -> Result<(), Error> {
    file.update(|state| {
        let limits = state.thresholds.checked();
        let recorded = state
            .usage
            .entry(active.to_owned())
            .or_insert_with(Recorded::default);
        let used = |w: Option<Window>| w.map_or(0.0, |w| w.used_at(now));
        let week_only = used(recorded.seven_day) >= limits.seven_day
            && used(recorded.five_hour) < limits.five_hour;
        let window = if week_only {
            &mut recorded.seven_day
        } else {
            &mut recorded.five_hour
        };
        let resets_at = window
            .filter(|w| w.resets_at > now)
            .map_or(now + FIVE_HOURS, |w| w.resets_at);
        *window = Some(Window {
            used_percent: 100.0,
            resets_at,
        });
        recorded.read_at = Some(now);
    })
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn, reason = "test assertions")]
mod tests {
    use super::{Context, Trigger, run};
    use crate::error::Error;
    use crate::login::account::AccountFiles;
    use crate::login::{Login, Logins, Slot};
    use crate::store::profile::ProfilesRoot;
    use crate::store::state::{Recorded, StateFile};
    use crate::usage::Window;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    const NOW: i64 = 1_000_000;

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

    struct World {
        home: TempDir,
        memory: Memory,
        root: ProfilesRoot,
        file: StateFile,
        accounts: AccountFiles,
    }

    impl World {
        /// Profiles `a`, `b` and `c`, all signed in; `active` holds its login
        /// in `~/.claude` and the others are parked.
        fn new(active: Option<&str>) -> Result<Self, Box<dyn std::error::Error>> {
            let home = TempDir::new()?;
            let memory = Memory::default();
            for name in ["a", "b", "c"] {
                let dir = home.path().join(".claude-profiles").join(name);
                fs::create_dir_all(&dir)?;
                fs::write(
                    dir.join(".claude.json"),
                    format!(r#"{{"oauthAccount": {{"emailAddress": "{name}@example.org"}}}}"#),
                )?;
                let slot = if Some(name) == active {
                    Slot::Active
                } else {
                    Slot::Parked(name.to_owned())
                };
                memory
                    .0
                    .borrow_mut()
                    .insert(slot, Login::from_bytes(name.as_bytes().to_vec()));
            }
            let root = ProfilesRoot::from_home(Some(home.path().into()))?;
            let file = StateFile::in_root(&root);
            let accounts = AccountFiles::new(home.path(), &root);
            file.update(|s| s.active = active.map(str::to_owned))?;
            Ok(Self {
                home,
                memory,
                root,
                file,
                accounts,
            })
        }

        fn context(&self) -> Context<'_, Memory> {
            Context {
                logins: &self.memory,
                accounts: &self.accounts,
                file: &self.file,
                root: &self.root,
            }
        }

        fn usage(&self, name: &str, used: f64) -> Result<(), Error> {
            self.file.update(|s| {
                s.usage.insert(
                    name.to_owned(),
                    Recorded {
                        five_hour: Some(Window {
                            used_percent: used,
                            resets_at: NOW + 3600,
                        }),
                        seven_day: None,
                        read_at: Some(NOW),
                        confirmed: true,
                    },
                );
            })
        }

        fn active(&self) -> Result<Option<String>, Error> {
            Ok(self.file.read()?.active)
        }
    }

    #[test]
    fn usage_below_the_threshold_stays() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        world.usage("a", 94.0)?;
        assert_eq!(run(&world.context(), Trigger::Usage, NOW)?, None);
        assert_eq!(world.active()?.as_deref(), Some("a"));
        Ok(())
    }

    #[test]
    fn usage_at_the_threshold_switches_by_the_selection_rule()
    -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        world.usage("a", 95.0)?;
        world.usage("b", 60.0)?;
        assert_eq!(
            run(&world.context(), Trigger::Usage, NOW)?.as_deref(),
            Some("b"),
            "b has known usage; c has none read yet"
        );
        assert_eq!(world.active()?.as_deref(), Some("b"));
        assert!(
            world.home.path().join(".claude.json").exists(),
            "details set"
        );
        Ok(())
    }

    #[test]
    fn a_hit_limit_marks_full_and_switches() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        world.usage("a", 50.0)?;
        world.usage("b", 10.0)?;
        assert_eq!(
            run(&world.context(), Trigger::LimitHit, NOW)?.as_deref(),
            Some("b")
        );
        let state = world.file.read()?;
        let a = state.usage.get("a").ok_or("a recorded")?;
        assert_eq!(a.five_hour.map(|w| w.used_percent), Some(100.0));
        assert_eq!(a.five_hour.map(|w| w.resets_at), Some(NOW + 3600));
        Ok(())
    }

    /// Records `name` with a 5-hour and a 7-day window.
    fn both(world: &World, name: &str, five: f64, seven: f64) -> Result<(), Error> {
        world.file.update(|s| {
            s.usage.insert(
                name.to_owned(),
                Recorded {
                    five_hour: Some(Window {
                        used_percent: five,
                        resets_at: NOW + 3600,
                    }),
                    seven_day: Some(Window {
                        used_percent: seven,
                        resets_at: NOW + 86_400,
                    }),
                    read_at: Some(NOW),
                    confirmed: true,
                },
            );
        })
    }

    #[test]
    fn a_hit_with_a_fuller_week_marks_the_five_hour_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        both(&world, "a", 80.0, 85.0)?;
        world.usage("b", 10.0)?;
        run(&world.context(), Trigger::LimitHit, NOW)?;
        let state = world.file.read()?;
        let a = state.usage.get("a").ok_or("a recorded")?;
        assert_eq!(a.five_hour.map(|w| w.used_percent), Some(100.0));
        assert_eq!(a.seven_day.map(|w| w.used_percent), Some(85.0), "week kept");
        Ok(())
    }

    #[test]
    fn a_hit_with_only_the_week_at_its_threshold_marks_the_week()
    -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        both(&world, "a", 30.0, 99.5)?;
        world.usage("b", 10.0)?;
        run(&world.context(), Trigger::LimitHit, NOW)?;
        let state = world.file.read()?;
        let a = state.usage.get("a").ok_or("a recorded")?;
        assert_eq!(a.seven_day.map(|w| w.used_percent), Some(100.0));
        assert_eq!(a.five_hour.map(|w| w.used_percent), Some(30.0), "5h kept");
        Ok(())
    }

    #[test]
    fn a_hit_with_only_a_week_reading_marks_a_five_hour_window()
    -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        world.file.update(|s| {
            s.usage.insert(
                "a".to_owned(),
                Recorded {
                    seven_day: Some(Window {
                        used_percent: 60.0,
                        resets_at: NOW + 86_400,
                    }),
                    read_at: Some(NOW),
                    confirmed: true,
                    ..Recorded::default()
                },
            );
        })?;
        world.usage("b", 10.0)?;
        run(&world.context(), Trigger::LimitHit, NOW)?;
        let state = world.file.read()?;
        let a = state.usage.get("a").ok_or("a recorded")?;
        assert_eq!(
            a.five_hour,
            Some(Window {
                used_percent: 100.0,
                resets_at: NOW + crate::usage::FIVE_HOURS
            })
        );
        Ok(())
    }

    #[test]
    fn a_second_hit_right_after_a_switch_is_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        world.usage("b", 10.0)?;
        world.usage("c", 50.0)?;
        run(&world.context(), Trigger::LimitHit, NOW)?;
        assert_eq!(run(&world.context(), Trigger::LimitHit, NOW + 5)?, None);
        assert_eq!(world.active()?.as_deref(), Some("b"));
        let state = world.file.read()?;
        assert_eq!(
            state
                .usage
                .get("b")
                .and_then(|r| r.five_hour)
                .map(|w| w.used_percent),
            Some(10.0),
            "b not marked full"
        );
        Ok(())
    }

    #[test]
    fn a_profile_without_a_login_is_skipped() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        world.usage("a", 99.0)?;
        world.usage("b", 0.0)?;
        world.usage("c", 50.0)?;
        world.memory.clear(&Slot::Parked("b".to_owned()))?;
        assert_eq!(
            run(&world.context(), Trigger::Usage, NOW)?.as_deref(),
            Some("c")
        );
        Ok(())
    }

    #[test]
    fn with_none_available_nothing_changes() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(Some("a"))?;
        for name in ["a", "b", "c"] {
            world.usage(name, 99.0)?;
        }
        assert_eq!(run(&world.context(), Trigger::Usage, NOW)?, None);
        assert_eq!(world.active()?.as_deref(), Some("a"));
        Ok(())
    }

    #[test]
    fn setup_activates_the_best_account_once() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(None)?;
        world
            .memory
            .write(&Slot::Active, &Login::from_bytes(b"unowned".to_vec()))?;
        assert_eq!(
            run(&world.context(), Trigger::Setup, NOW)?.as_deref(),
            Some("a")
        );
        assert_eq!(run(&world.context(), Trigger::Setup, NOW)?, None);
        Ok(())
    }

    #[test]
    fn nothing_happens_before_setup() -> Result<(), Box<dyn std::error::Error>> {
        let world = World::new(None)?;
        assert_eq!(run(&world.context(), Trigger::LimitHit, NOW)?, None);
        assert_eq!(world.active()?, None);
        Ok(())
    }
}
