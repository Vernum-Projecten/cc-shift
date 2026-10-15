//! The rule that picks the next account.

use std::cmp::Ordering;

use crate::usage::{Account, Usage};

/// The percentage at which a window counts as full and `cs` switches away.
pub const SWITCH_THRESHOLD: f64 = 95.0;

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
/// An account is available when it is logged in and both windows are below
/// [`SWITCH_THRESHOLD`]; a window whose reset time has passed counts as 0%.
/// Among available accounts with known usage, the one with the most usage
/// left in its fuller window wins, and on a tie the one whose fuller window
/// resets first, then the name, so the choice is the same every run.
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
        .filter(|a| {
            a.usage
                .used_at(now)
                .is_some_and(|used| used < SWITCH_THRESHOLD)
        })
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
            .filter_map(|a| available_from(a, now).map(|at| (a, at)))
            .min_by(|(a, at_a), (b, at_b)| at_a.cmp(at_b).then_with(|| a.name.cmp(&b.name))),
    )
}

/// Orders two available accounts: the better one first.
fn rank(a: &Account, b: &Account, now: i64) -> Ordering {
    let used = |x: &Account| x.usage.used_at(now).unwrap_or(f64::MAX);
    let reset = |x: &Account| x.usage.fuller_window(now).map_or(i64::MAX, |w| w.resets_at);
    used(a)
        .total_cmp(&used(b))
        .then_with(|| reset(a).cmp(&reset(b)))
        .then_with(|| a.name.cmp(&b.name))
}

/// Returns when a full account with known usage becomes available again.
fn available_from(account: &Account, now: i64) -> Option<i64> {
    let Usage::Known {
        five_hour,
        seven_day,
    } = &account.usage
    else {
        return None;
    };
    [five_hour, seven_day]
        .into_iter()
        .flatten()
        .filter(|w| w.used_at(now) >= SWITCH_THRESHOLD)
        .map(|w| w.resets_at)
        .max()
}

#[cfg(test)]
mod tests {
    use super::{Choice, SWITCH_THRESHOLD, next};
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
        }
    }

    fn unknown(name: &str) -> Account {
        Account {
            name: name.to_owned(),
            logged_in: true,
            usage: Usage::Unknown("no source".to_owned()),
        }
    }

    fn chosen<'a>(choice: &Choice<'a>) -> Option<&'a str> {
        match choice {
            Choice::Account(a) => Some(a.name.as_str()),
            Choice::NoneAvailable(_) => None,
        }
    }

    #[test]
    fn most_usage_left_wins() {
        let accounts = [
            account("a", window(60.0, 100), window(10.0, 1000)),
            account("b", window(20.0, 100), window(30.0, 1000)),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn the_fuller_window_decides() {
        let accounts = [
            account("a", window(10.0, 100), window(80.0, 1000)),
            account("b", window(50.0, 100), window(50.0, 1000)),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn a_tie_goes_to_the_earliest_reset() {
        let accounts = [
            account("a", window(40.0, 500), None),
            account("b", window(40.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn a_full_window_makes_an_account_unavailable() {
        let accounts = [
            account("a", window(10.0, 100), window(SWITCH_THRESHOLD, 1000)),
            account("b", window(90.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("b"));
    }

    #[test]
    fn a_window_that_has_reset_counts_as_unused() {
        let accounts = [
            account("a", window(100.0, 0), None),
            account("b", window(50.0, 100), None),
        ];
        assert_eq!(chosen(&next(&accounts, None, NOW)), Some("a"));
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
        fn the_choice_is_available_and_has_the_most_usage_left(accounts in any_accounts()) {
            if let Choice::Account(pick) = next(&accounts, None, NOW) {
                let used = pick.usage.used_at(NOW).unwrap_or(f64::MAX);
                prop_assert!(pick.logged_in, "chose a logged-out account");
                prop_assert!(used < SWITCH_THRESHOLD, "chose a full account");
                for other in accounts.iter().filter(|a| a.logged_in) {
                    if let Some(other_used) = other.usage.used_at(NOW) {
                        prop_assert!(
                            other_used >= SWITCH_THRESHOLD || used <= other_used,
                            "{} has more left than {}", other.name, pick.name
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
