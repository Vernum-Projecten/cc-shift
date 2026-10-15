// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The rule that picks the next account.

use std::cmp::Ordering;

use crate::usage::{Account, Usage};

/// The outcome of the selection rule.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice<'a> {
    /// The account to make active.
    Account(&'a Account),
    /// No account is available. Holds the account that becomes available
    /// first and when, if any account with known usage is logged in.
    NoneAvailable(Option<(&'a Account, i64)>),
}

/// Picks the account to make active at `now`, never the one named `leaving`.
///
/// An account is available when it is logged in and each window is below its
/// own threshold ([`crate::usage::Thresholds`]); a window whose reset time
/// has passed counts as 0%.
///
/// The rule spends what expires first. Usage left in a window when it resets
/// is lost, and the 5-hour window only starts with an account's first
/// message, so an untouched account loses nothing by waiting. Among
/// available accounts with known usage:
///
/// 1. an account whose 5-hour window is running goes first, the one that
///    resets soonest first;
/// 2. then an account with no running 5-hour window, the one whose 7-day
///    window resets soonest first, then the one with the most 7-day usage
///    left;
/// 3. then the name, so the choice is the same every run.
///
/// Accounts with unknown usage are chosen, by name, only when no account
/// with known usage is available.
#[must_use]
pub fn next<'a>(accounts: &'a [Account], leaving: Option<&str>, now: i64) -> Choice<'a> {
    let candidates = || {
        accounts
            .iter()
            .filter(move |a| a.logged_in && Some(a.name.as_str()) != leaving)
    };
    let known_best = candidates()
        .filter(|a| a.full_at(now) == Some(false))
        .min_by(|a, b| rank(a, b, now));
    if let Some(account) = known_best {
        return Choice::Account(account);
    }
    let unknown_first = candidates()
        .filter(|a| matches!(a.usage, Usage::Unknown(_)))
        .min_by(|a, b| a.name.cmp(&b.name));
    if let Some(account) = unknown_first {
        return Choice::Account(account);
    }
    Choice::NoneAvailable(
        candidates()
            .filter_map(|a| full_until(a, now).map(|at| (a, at)))
            .min_by(|(a, at_a), (b, at_b)| at_a.cmp(at_b).then_with(|| a.name.cmp(&b.name))),
    )
}

/// Returns the available accounts in the order the rule uses them: the
/// accounts with known usage by [`next`]'s ranking, then the unknown ones by
/// name.
#[must_use]
pub fn in_order(accounts: &[Account], now: i64) -> Vec<&Account> {
    let mut known: Vec<&Account> = accounts
        .iter()
        .filter(|a| a.logged_in && a.full_at(now) == Some(false))
        .collect();
    known.sort_by(|a, b| rank(a, b, now));
    let mut unknown: Vec<&Account> = accounts
        .iter()
        .filter(|a| a.logged_in && matches!(a.usage, Usage::Unknown(_)))
        .collect();
    unknown.sort_by(|a, b| a.name.cmp(&b.name));
    known.extend(unknown);
    known
}

/// Orders two available accounts: the one whose usage expires first first.
fn rank(a: &Account, b: &Account, now: i64) -> Ordering {
    let running = |x: &Account| running_five_hour(x, now);
    match (running(a), running(b)) {
        (Some(at_a), Some(at_b)) => at_a.cmp(&at_b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => {
            let week = |x: &Account| seven_day(x, now);
            let (reset_a, used_a) = week(a);
            let (reset_b, used_b) = week(b);
            reset_a
                .cmp(&reset_b)
                .then_with(|| used_a.total_cmp(&used_b))
        }
    }
    .then_with(|| a.name.cmp(&b.name))
}

/// Returns when the account's running 5-hour window resets, or `None` when
/// no 5-hour window is running.
fn running_five_hour(account: &Account, now: i64) -> Option<i64> {
    match &account.usage {
        Usage::Known {
            five_hour: Some(window),
            ..
        } if window.resets_at > now => Some(window.resets_at),
        _ => None,
    }
}

/// Returns when the account's 7-day window resets (the far future when none
/// is running) and how much of it is used.
fn seven_day(account: &Account, now: i64) -> (i64, f64) {
    match &account.usage {
        Usage::Known {
            seven_day: Some(window),
            ..
        } if window.resets_at > now => (window.resets_at, window.used_percent),
        _ => (i64::MAX, 0.0),
    }
}

/// Returns when a full account with known usage becomes available again, or
/// `None` when no window is full or the usage is unknown.
#[must_use]
pub fn full_until(account: &Account, now: i64) -> Option<i64> {
    let Usage::Known {
        five_hour,
        seven_day,
    } = &account.usage
    else {
        return None;
    };
    let limits = account.thresholds;
    [(five_hour, limits.five_hour), (seven_day, limits.seven_day)]
        .into_iter()
        .filter_map(|(w, limit)| w.filter(|w| w.used_at(now) >= limit))
        .map(|w| w.resets_at)
        .max()
}

#[cfg(test)]
mod tests {
    use super::{Choice, next, running_five_hour};
    use crate::usage::Thresholds;
    use crate::usage::{Account, Usage, Window};
    use proptest::prelude::{Strategy, any, prop, prop_assert, prop_assert_eq, proptest};

    const NOW: i64 = 1_000_000;

    #[expect(
        clippy::unnecessary_wraps,
        reason = "fixtures pass the result straight into an optional window"
    )]
    fn window(used: f64, resets_in: i64) -> Option<Window> {
        Some(Window {
            used_percent: used,
            resets_at: NOW + resets_in,
        })
    }

    fn account(name: &str, five: Option<Window>, seven: Option<Window>) -> Account {
        Account {
            name: name.to_owned(),
            logged_in: true,
            usage: Usage::Known {
                five_hour: five,
                seven_day: seven,
            },
            thresholds: Thresholds::default(),
        }
    }

    fn unknown(name: &str) -> Account {
        Account {
            name: name.to_owned(),
            logged_in: true,
            usage: Usage::Unknown("no source".to_owned()),
            thresholds: Thresholds::default(),
        }
    }

    fn chosen<'a>(choice: &Choice<'a>) -> Option<&'a str> {
        match choice {
            Choice::Account(a) => Some(a.name.as_str()),
            Choice::NoneAvailable(_) => None,
        }
    }

    #[test]
    fn a_running_window_goes_before_a_fresh_account() {
        let accounts = [
            account("a", None, window(10.0, 400_000)),
            account("b", window(60.0, 3600), window(40.0, 500_000)),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn the_running_window_that_resets_soonest_goes_first() {
        let accounts = [
            account("a", window(10.0, 7200), None),
            account("b", window(80.0, 600), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn a_fresh_account_whose_week_resets_soonest_goes_first() {
        let accounts = [
            account("a", None, window(10.0, 500_000)),
            account("b", None, window(70.0, 100_000)),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn between_equal_weeks_the_most_left_goes_first() {
        let accounts = [
            account("a", None, window(50.0, 100_000)),
            account("b", None, window(20.0, 100_000)),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn a_window_that_has_reset_makes_a_full_account_fresh() {
        let accounts = [
            account("a", window(100.0, 0), None),
            account("b", window(99.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("a"));
    }

    #[test]
    fn the_week_switches_at_99_and_the_5_hour_window_at_95() {
        let accounts = [
            account("a", window(94.0, 100), window(98.0, 1000)),
            account("b", window(95.0, 100), None),
            account("c", None, window(99.0, 1000)),
        ];
        let available: Vec<&str> = super::in_order(&accounts, NOW)
            .iter()
            .map(|a| a.name.as_str())
            .collect();
        assert_eq!(available, vec!["a"]);
    }

    #[test]
    fn a_week_threshold_of_100_uses_the_week_to_the_end() {
        let mut nearly = account("a", None, window(99.5, 1000));
        nearly.thresholds = Thresholds {
            five_hour: 95.0,
            seven_day: 100.0,
        };
        assert_eq!(chosen(&next(&[nearly.clone()], None, NOW)), Some("a"));
        nearly.usage = Usage::Known {
            five_hour: None,
            seven_day: window(100.0, 1000),
        };
        assert_eq!(chosen(&next(&[nearly], None, NOW)), None);
    }

    #[test]
    fn a_threshold_outside_50_to_100_falls_back_to_its_default() {
        let checked = Thresholds {
            five_hour: 120.0,
            seven_day: 10.0,
        }
        .checked();
        assert_eq!(checked, Thresholds::default());
        let kept = Thresholds {
            five_hour: 90.0,
            seven_day: 100.0,
        };
        assert_eq!(kept.checked(), kept);
    }

    #[test]
    fn a_full_window_makes_an_account_unavailable() {
        let accounts = [
            account("a", window(10.0, 100), window(99.0, 1000)),
            account("b", window(90.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn the_account_being_left_is_never_chosen() {
        let accounts = [
            account("a", window(0.0, 100), None),
            account("b", window(50.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, Some("a"), NOW)), Some("b"));
    }

    #[test]
    fn a_logged_out_account_is_never_chosen() {
        let mut out = account("a", window(0.0, 100), None);
        out.logged_in = false;
        let accounts = [out, account("b", window(50.0, 100), None)];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn unknown_usage_comes_after_every_known_account() {
        let accounts = [unknown("a"), account("b", window(90.0, 100), None)];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn unknown_usage_is_chosen_when_nothing_known_is_available() {
        let accounts = [
            unknown("b"),
            unknown("a"),
            account("c", window(99.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("a"));
    }

    #[test]
    fn with_none_available_the_first_reset_is_reported() {
        let accounts = [
            account("a", window(99.0, 300), None),
            account("b", window(96.0, 200), window(97.0, 5000)),
            account("c", window(100.0, 100), None),
        ];
        let choice = next(&accounts, None, NOW);
        let Choice::NoneAvailable(Some((first, at))) = choice else {
            panic!("expected no account available, got {choice:?}");
        };
        assert_eq!((first.name.as_str(), at), ("c", NOW + 100));
    }

    #[test]
    fn with_no_accounts_nothing_is_chosen() {
        assert_eq!(next(&[], None, NOW), Choice::NoneAvailable(None));
    }

    fn any_window() -> impl Strategy<Value = Option<Window>> {
        prop::option::of(
            (0.0..=100.0_f64, -1000_i64..10_000).prop_map(|(used, resets_in)| Window {
                used_percent: used,
                resets_at: NOW + resets_in,
            }),
        )
    }

    fn any_accounts() -> impl Strategy<Value = Vec<Account>> {
        prop::collection::vec((any_window(), any_window(), any::<bool>()), 0..8).prop_map(|rows| {
            rows.into_iter()
                .enumerate()
                .map(|(i, (five, seven, logged_in))| Account {
                    logged_in,
                    ..account(&format!("p{i}"), five, seven)
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn the_choice_is_available_and_expires_first(accounts in any_accounts()) {
            if let Choice::Account(pick) = next(&accounts, None, NOW) {
                prop_assert!(pick.logged_in, "chose a logged-out account");
                prop_assert!(pick.full_at(NOW) != Some(true), "chose a full account");
                let pick_running = running_five_hour(pick, NOW);
                for other in accounts.iter().filter(|a| a.logged_in) {
                    let available = other.full_at(NOW) == Some(false);
                    if let (true, Some(other_reset)) = (available, running_five_hour(other, NOW)) {
                        prop_assert!(
                            pick_running.is_some_and(|at| at <= other_reset),
                            "{} expires before {}", other.name, pick.name
                        );
                    }
                }
            }
        }

        #[test]
        fn the_choice_is_the_same_every_run(accounts in any_accounts()) {
            prop_assert_eq!(next(&accounts, None, NOW), next(&accounts, None, NOW));
        }
    }
}
