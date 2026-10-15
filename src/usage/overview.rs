//! One view of every account, shared by `cc-shift status`, its JSON form and
//! the status line: each account's state, in the order `cc-shift` uses them.

use crate::store::state::State;
use crate::usage::select::{self, SWITCH_THRESHOLD};
use crate::usage::{Account, Usage, Window, accounts};

/// What an account is right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// The account in `~/.claude`.
    Active,
    /// The account `cc-shift` switches to next.
    Next,
    /// Available, later in the order.
    Ready,
    /// A 5-hour window is full; free again at the time given.
    Full(i64),
    /// The 7-day window is full; free again at the time given.
    WeekFull(i64),
    /// Its refresh was refused; it needs `/login` again.
    LoggedOut,
    /// No usage could be read, with the reason.
    Unknown(String),
}

/// One account in the overview.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The profile name.
    pub name: String,
    /// What the account is right now.
    pub status: Status,
    /// The 5-hour window, `None` when it has not started or has reset.
    pub five_hour: Option<Window>,
    /// The 7-day window, `None` when it has not started or has reset.
    pub seven_day: Option<Window>,
    /// Whether the numbers were read with the account's own token.
    pub confirmed: bool,
}

/// Every account, in the order `cc-shift` uses them.
#[derive(Debug, Clone, PartialEq)]
pub struct Overview {
    /// The active account first, then the available ones in the order the
    /// selection rule uses them, then the full ones by when they free up,
    /// then the logged-out ones.
    pub rows: Vec<Row>,
    /// The account in `~/.claude`.
    pub active: Option<String>,
    /// The account `cc-shift` switches to next.
    pub next: Option<String>,
    /// How many accounts are available now, the active one included.
    pub free: usize,
    /// The account that frees up first and when, when none but the active
    /// one is available.
    pub first_free: Option<(String, i64)>,
}

/// Builds the overview of `profiles` from the state at `now`.
#[must_use]
pub fn overview(profiles: &[String], state: &State, now: i64) -> Overview {
    let all = accounts(profiles, state);
    let active = state.active.clone();
    let order = select::in_order(&all, now);
    let next = order
        .iter()
        .find(|a| Some(&a.name) != active.as_ref())
        .map(|a| a.name.clone());
    let available = |a: &Account| order.iter().any(|o| o.name == a.name);
    let free = all.iter().filter(|a| available(a)).count();
    let row = |account: &Account, status: Status| Row {
        name: account.name.clone(),
        status,
        five_hour: running(&account.usage, now, |u| u.0),
        seven_day: running(&account.usage, now, |u| u.1),
        confirmed: state.usage.get(&account.name).is_some_and(|r| r.confirmed),
    };

    let mut rows = Vec::new();
    if let Some(account) = all.iter().find(|a| Some(&a.name) == active.as_ref()) {
        rows.push(row(account, Status::Active));
    }
    for account in order.iter().filter(|a| Some(&a.name) != active.as_ref()) {
        let status = if Some(&account.name) == next.as_ref() {
            Status::Next
        } else {
            match &account.usage {
                Usage::Unknown(reason) => Status::Unknown(reason.clone()),
                Usage::Known { .. } => Status::Ready,
            }
        };
        rows.push(row(account, status));
    }
    let mut full: Vec<(&Account, Status, i64)> = all
        .iter()
        .filter(|a| Some(&a.name) != active.as_ref() && !available(a))
        .map(|a| {
            let status = blocked(a, now);
            let until = match status {
                Status::Full(at) | Status::WeekFull(at) => at,
                _ => i64::MAX,
            };
            (a, status, until)
        })
        .collect();
    full.sort_by(|x, y| x.2.cmp(&y.2).then_with(|| x.0.name.cmp(&y.0.name)));
    let first_free = full
        .iter()
        .find(|(_, _, until)| *until != i64::MAX)
        .map(|(a, _, until)| (a.name.clone(), *until));
    rows.extend(full.into_iter().map(|(a, status, _)| row(a, status)));
    Overview {
        rows,
        active,
        next,
        free,
        first_free,
    }
}

/// Returns why an unavailable account cannot be used now.
fn blocked(account: &Account, now: i64) -> Status {
    if !account.logged_in {
        return Status::LoggedOut;
    }
    let Usage::Known {
        five_hour,
        seven_day,
    } = &account.usage
    else {
        return match &account.usage {
            Usage::Unknown(reason) => Status::Unknown(reason.clone()),
            Usage::Known { .. } => Status::Ready,
        };
    };
    let full = |w: &Option<Window>| w.filter(|w| w.used_at(now) >= SWITCH_THRESHOLD);
    match (full(five_hour), full(seven_day)) {
        (_, Some(week)) => Status::WeekFull(
            full(five_hour).map_or(week.resets_at, |f| f.resets_at.max(week.resets_at)),
        ),
        (Some(five), None) => Status::Full(five.resets_at),
        (None, None) => select::full_until(account, now).map_or(Status::Ready, Status::Full),
    }
}

/// Returns one window of a known usage when it is running at `now`.
fn running(
    usage: &Usage,
    now: i64,
    pick: impl Fn((Option<Window>, Option<Window>)) -> Option<Window>,
) -> Option<Window> {
    match usage {
        Usage::Known {
            five_hour,
            seven_day,
        } => pick((*five_hour, *seven_day)).filter(|w| w.resets_at > now),
        Usage::Unknown(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{Status, overview};
    use crate::store::state::{Recorded, State};
    use crate::usage::Window;

    const NOW: i64 = 1_000_000;

    fn reading(five: Option<(f64, i64)>, seven: Option<(f64, i64)>) -> Recorded {
        let window = |(used, resets_in): (f64, i64)| Window {
            used_percent: used,
            resets_at: NOW + resets_in,
        };
        Recorded {
            five_hour: five.map(window),
            seven_day: seven.map(window),
            read_at: Some(NOW),
            confirmed: true,
        }
    }

    fn state() -> State {
        let mut state = State {
            active: Some("c".to_owned()),
            ..State::default()
        };
        let usage = [
            ("a", reading(Some((10.0, 3600)), Some((60.0, 90_000)))),
            ("b", reading(None, Some((100.0, 50_000)))),
            ("c", reading(Some((15.0, 9000)), Some((66.0, 300_000)))),
            ("d", reading(Some((100.0, 600)), Some((49.0, 200_000)))),
            ("e", reading(None, Some((20.0, 400_000)))),
        ];
        for (name, recorded) in usage {
            state.usage.insert(name.to_owned(), recorded);
        }
        state.logged_out.insert("f".to_owned());
        state
    }

    fn names(profiles: &[&str]) -> Vec<String> {
        profiles.iter().map(|p| (*p).to_owned()).collect()
    }

    #[test]
    fn rows_follow_the_order_of_use() {
        let view = overview(&names(&["a", "b", "c", "d", "e", "f"]), &state(), NOW);
        let order: Vec<(&str, &Status)> = view
            .rows
            .iter()
            .map(|r| (r.name.as_str(), &r.status))
            .collect();
        assert_eq!(
            order,
            vec![
                ("c", &Status::Active),
                ("a", &Status::Next),
                ("e", &Status::Ready),
                ("d", &Status::Full(NOW + 600)),
                ("b", &Status::WeekFull(NOW + 50_000)),
                ("f", &Status::LoggedOut),
            ]
        );
        assert_eq!(view.next.as_deref(), Some("a"));
        assert_eq!(view.free, 3);
        assert_eq!(view.first_free, Some(("d".to_owned(), NOW + 600)));
    }

    #[test]
    fn a_window_that_has_not_started_is_none() {
        let view = overview(&names(&["e"]), &state(), NOW);
        let e = view
            .rows
            .first()
            .map(|r| (r.five_hour, r.seven_day.is_some()));
        assert_eq!(e, Some((None, true)));
    }
}
