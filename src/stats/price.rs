// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! Anthropic's public API prices per model, to weigh token counts into a
//! comparable cost.
//!
//! The cost is what the same tokens would cost at the Claude API's standard
//! prices. It is a comparison, not a bill: a subscription is not billed per
//! token, and fast mode, data residency and batch prices are not applied.

use crate::stats::Tokens;

/// The prices of one model, in millionths of a dollar per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Price {
    /// Base input tokens.
    pub input: u64,
    /// Cache writes with the 5-minute lifetime.
    pub write_5m: u64,
    /// Cache writes with the 1-hour lifetime.
    pub write_1h: u64,
    /// Cache hits and refreshes.
    pub read: u64,
    /// Output tokens.
    pub output: u64,
}

/// The prices of one model, with a second set above a prompt length when the
/// model has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelPrice {
    /// The model id as Claude Code records it.
    pub model: &'static str,
    /// The prices for a prompt up to the threshold, or for every prompt.
    pub standard: Price,
    /// The prompt length in tokens above which `long` applies, and its
    /// prices.
    pub long: Option<(u64, Price)>,
}

/// One dollar per million tokens, in the unit of [`Price`].
const DOLLAR: u64 = 1_000_000;

/// One cent per million tokens, in the unit of [`Price`].
const CENT: u64 = 10_000;

/// Builds a [`Price`] from cents per million tokens.
const fn cents(base: u64, short_write: u64, long_write: u64, hit: u64, out: u64) -> Price {
    Price {
        input: base * CENT,
        write_5m: short_write * CENT,
        write_1h: long_write * CENT,
        read: hit * CENT,
        output: out * CENT,
    }
}

/// Builds a [`ModelPrice`] without a long-prompt tier.
const fn flat(model: &'static str, standard: Price) -> ModelPrice {
    ModelPrice {
        model,
        standard,
        long: None,
    }
}

// NOTE: <https://platform.claude.com/docs/en/about-claude/pricing>, "Model
// pricing", read 2026-10-15; scripts/checks/api-prices.sh compares the page
// with the snapshot this table is tested against.
/// The price table, in cents per million tokens: input, 5-minute cache
/// write, 1-hour cache write, cache read, output.
pub const PRICES: [ModelPrice; 16] = [
    flat("claude-fable-5-1", cents(1000, 1250, 2000, 25, 5000)),
    flat("claude-mythos-5-1", cents(1000, 1250, 2000, 25, 5000)),
    flat("claude-fable-5", cents(1000, 1250, 2000, 100, 5000)),
    flat("claude-mythos-5", cents(1000, 1250, 2000, 100, 5000)),
    flat("claude-opus-5-5", cents(400, 500, 800, 20, 2000)),
    flat("claude-opus-5", cents(500, 625, 1000, 50, 2500)),
    flat("claude-opus-4-8", cents(500, 625, 1000, 50, 2500)),
    flat("claude-opus-4-7", cents(500, 625, 1000, 50, 2500)),
    flat("claude-opus-4-6", cents(500, 625, 1000, 50, 2500)),
    flat("claude-opus-4-5", cents(500, 625, 1000, 50, 2500)),
    flat("claude-sonnet-5-5", cents(200, 250, 400, 10, 1000)),
    flat("claude-sonnet-5", cents(200, 250, 400, 20, 1000)),
    flat("claude-sonnet-4-6", cents(300, 375, 600, 30, 1500)),
    flat("claude-sonnet-4-5", cents(300, 375, 600, 30, 1500)),
    ModelPrice {
        model: "claude-haiku-5-5",
        standard: Price {
            input: 100_000,
            write_5m: 125_000,
            write_1h: 200_000,
            read: 10_000,
            output: 500_000,
        },
        long: Some((
            100_000,
            Price {
                input: 500_000,
                write_5m: 625_000,
                write_1h: DOLLAR,
                read: 50_000,
                output: 2_500_000,
            },
        )),
    },
    flat("claude-haiku-4-5", cents(100, 125, 200, 10, 500)),
];

/// Returns the prices of `model`, or `None` when no price is known.
///
/// A model id with a date suffix (`-20251001`) has the prices of the id
/// without it.
#[must_use]
pub fn price_of(model: &str) -> Option<&'static ModelPrice> {
    let base = without_date(model);
    PRICES.iter().find(|p| p.model == base)
}

/// Returns `model` without a trailing `-YYYYMMDD`.
fn without_date(model: &str) -> &str {
    match model.rsplit_once('-') {
        Some((base, date)) if date.len() == 8 && date.bytes().all(|b| b.is_ascii_digit()) => base,
        _ => model,
    }
}

/// A cost in millionths of a millionth of a dollar, the unit that keeps
/// every product of a token count and a [`Price`] exact.
pub type Picodollars = u128;

/// Returns the cost of one response at `price`.
///
/// The prompt length (input, cache writes and cache reads together) picks
/// the long-prompt prices when the model has them and the prompt is over
/// the threshold.
#[must_use]
pub fn cost(price: &ModelPrice, tokens: &Tokens) -> Picodollars {
    let rates = match price.long {
        Some((threshold, long)) if tokens.prompt() > threshold => long,
        _ => price.standard,
    };
    let hour_writes = tokens.cache_write_1h.min(tokens.cache_write);
    let minute_writes = tokens.cache_write - hour_writes;
    [
        (tokens.input, rates.input),
        (minute_writes, rates.write_5m),
        (hour_writes, rates.write_1h),
        (tokens.cache_read, rates.read),
        (tokens.output, rates.output),
    ]
    .iter()
    .map(|(count, rate)| u128::from(*count) * u128::from(*rate))
    .sum()
}

/// One cent, in [`Picodollars`].
const PER_CENT: Picodollars = 10_000_000_000;

/// Half a cent, in [`Picodollars`], to round to the nearest cent.
const HALF_CENT: Picodollars = 5_000_000_000;

/// Returns a cost as dollars and cents, rounded to the nearest cent:
/// `$41.20`.
#[must_use]
pub fn dollars(cost: Picodollars) -> String {
    let cents = (cost + HALF_CENT).div_euclid(PER_CENT);
    format!("${}.{:02}", cents.div_euclid(100), cents.rem_euclid(100))
}

/// Returns a cost in dollars as a number, for JSON.
#[must_use]
pub fn dollars_number(cost: Picodollars) -> f64 {
    let cents = (cost + HALF_CENT).div_euclid(PER_CENT);
    let cents = u32::try_from(cents).map_or(f64::from(u32::MAX), f64::from);
    cents / 100.0
}

#[cfg(test)]
mod tests {
    use super::{DOLLAR, PRICES, Price, cost, dollars, price_of};
    use crate::stats::Tokens;

    /// The page's model table as last reviewed, one line per row.
    const SNAPSHOT: &str = include_str!("../../scripts/checks/api-prices.txt");

    /// Parses `$12.50`-style dollars from the snapshot into the [`Price`]
    /// unit.
    fn micros(text: &str) -> u64 {
        let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
        let mut fraction = fraction.to_owned();
        while fraction.len() < 6 {
            fraction.push('0');
        }
        whole.parse::<u64>().unwrap() * DOLLAR + fraction.parse::<u64>().unwrap()
    }

    fn snapshot_price(fields: &[&str]) -> Price {
        Price {
            input: micros(fields[1]),
            write_5m: micros(fields[2]),
            write_1h: micros(fields[3]),
            read: micros(fields[4]),
            output: micros(fields[5]),
        }
    }

    /// Returns the model id a row name of the pricing page stands for.
    fn id_of(name: &str) -> String {
        name.to_lowercase().replace([' ', '.'], "-")
    }

    #[test]
    fn the_table_matches_the_reviewed_page() {
        for model in &PRICES {
            let rows: Vec<Vec<&str>> = SNAPSHOT
                .lines()
                .map(|l| l.split('|').collect::<Vec<_>>())
                .filter(|f| id_of(f[0].split(" (").next().unwrap()) == model.model)
                .collect();
            assert!(!rows.is_empty(), "{} is not on the page", model.model);
            assert_eq!(
                model.standard,
                snapshot_price(&rows[0]),
                "{} standard",
                model.model
            );
            match model.long {
                None => assert_eq!(rows.len(), 1, "{}", model.model),
                Some((threshold, long)) => {
                    assert_eq!(threshold, 100_000, "{}", model.model);
                    assert!(rows[1][0].contains("over 100,000"), "{}", rows[1][0]);
                    assert_eq!(long, snapshot_price(&rows[1]), "{} long", model.model);
                }
            }
        }
    }

    #[test]
    fn every_current_page_model_has_a_price() {
        for line in SNAPSHOT.lines() {
            let name = line.split('|').next().unwrap();
            if name.contains("retired") {
                continue;
            }
            let id = id_of(name.split(" (").next().unwrap());
            assert!(price_of(&id).is_some(), "no price for {id} ({name})");
        }
    }

    #[test]
    fn a_dated_id_has_the_price_of_its_alias() {
        assert_eq!(
            price_of("claude-haiku-4-5-20251001").map(|p| p.model),
            Some("claude-haiku-4-5")
        );
        assert_eq!(price_of("claude-opus-9-9"), None);
        assert_eq!(price_of("<synthetic>"), None);
    }

    #[test]
    fn cost_weighs_each_kind() {
        let opus = price_of("claude-opus-5-5").unwrap();
        let tokens = Tokens {
            input: 1_000_000,
            output: 1_000_000,
            cache_write: 3_000_000,
            cache_write_1h: 1_000_000,
            cache_read: 10_000_000,
        };
        // $4 input + $20 output + 2 x $5 + 1 x $8 writes + 10 x $0.20 reads.
        assert_eq!(dollars(cost(opus, &tokens)), "$44.00");
    }

    #[test]
    fn a_long_haiku_prompt_pays_the_higher_prices() {
        let haiku = price_of("claude-haiku-5-5").unwrap();
        let short = Tokens {
            input: 100_000,
            output: 1_000_000,
            ..Tokens::default()
        };
        let long = Tokens {
            input: 100_001,
            output: 1_000_000,
            ..Tokens::default()
        };
        assert_eq!(dollars(cost(haiku, &short)), "$0.51");
        assert_eq!(dollars(cost(haiku, &long)), "$2.55");
    }

    #[test]
    fn dollars_round_to_the_cent() {
        assert_eq!(dollars(0), "$0.00");
        assert_eq!(dollars(5_000_000_000), "$0.01");
        assert_eq!(dollars(4_999_999_999), "$0.00");
        assert_eq!(dollars(41_204_000_000_000), "$41.20");
    }
}
