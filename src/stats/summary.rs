//! Counting the responses of a period per project, model and day.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Span, Timestamp};

use crate::error::Error;
use crate::stats::price::{self, Picodollars};
use crate::stats::{Options, Period, Response, Tokens};

/// The cost of a group of responses at API prices.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cost {
    /// The cost of the responses whose model has a known price.
    pub known: Picodollars,
    /// Whether some responses are of a model without a known price.
    pub unpriced: bool,
    /// Whether some responses are of a model with a known price.
    pub priced: bool,
}

impl Cost {
    /// Adds the cost of one response of `model`.
    fn add(&mut self, model: &str, tokens: &Tokens) {
        match price::price_of(model) {
            Some(price) => {
                self.known = self.known.saturating_add(price::cost(price, tokens));
                self.priced = true;
            }
            None => self.unpriced = true,
        }
    }

    /// Adds another group's cost.
    fn merge(&mut self, other: &Self) {
        self.known = self.known.saturating_add(other.known);
        self.unpriced |= other.unpriced;
        self.priced |= other.priced;
    }
}

/// One project or one model with its counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The shown name: a folder name, a parent and folder name, or a model.
    pub name: String,
    /// The working folder of a project; `None` for a model.
    pub path: Option<String>,
    /// The token counts.
    pub tokens: Tokens,
    /// The cost at API prices.
    pub cost: Cost,
}

/// One local day with its counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Day {
    /// The date in local time.
    pub date: Date,
    /// Whether the date is today.
    pub today: bool,
    /// The token counts.
    pub tokens: Tokens,
    /// The cost at API prices.
    pub cost: Cost,
}

/// Everything `cc-shift stats` shows for a period.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// The period counted.
    pub period: Period,
    /// The start of the period, in Unix seconds; `None` for all time.
    pub from: Option<i64>,
    /// When the summary was made, in Unix seconds.
    pub to: i64,
    /// The one project shown, by its shown name.
    pub project: Option<String>,
    /// The number of sessions with a response in the period.
    pub sessions: usize,
    /// The number of responses in the period, each counted once.
    pub responses: usize,
    /// The projects, most output first.
    pub projects: Vec<Row>,
    /// The models, most output first.
    pub models: Vec<Row>,
    /// The days with a response, oldest first.
    pub days: Vec<Day>,
    /// The counts of all responses together.
    pub total: Tokens,
    /// The cost of all responses together.
    pub total_cost: Cost,
    /// The models in the period without a known price, sorted.
    pub unpriced: Vec<String>,
}

/// Counts `responses` for the period and project of `options`.
///
/// A response repeated in several lines or files (a resumed session repeats
/// earlier ones) is counted once, by its message id; of the copies, the one
/// with the most output tokens counts. Days and today are in `zone`.
///
/// # Errors
///
/// Returns [`Error::Period`] when the start of the period cannot be worked
/// out, and [`Error::NoProject`] when `options` names a project with no
/// response in the period.
pub fn summarize(
    responses: Vec<Response>,
    options: &Options,
    now: Timestamp,
    zone: &TimeZone,
) -> Result<Summary, Error> {
    let from = start(options.period, now, zone)?;
    let mut unique: BTreeMap<String, Response> = BTreeMap::new();
    for response in responses {
        if from.is_some_and(|from| response.at < from) {
            continue;
        }
        match unique.get(&response.id) {
            Some(kept) if !response.replaces(kept) => {}
            _ => {
                unique.insert(response.id.clone(), response);
            }
        }
    }
    let names = project_names(unique.values());
    let mut kept: Vec<(&Response, &str)> = unique
        .values()
        .map(|r| (r, project_name(&names, r)))
        .collect();
    if let Some(wanted) = &options.project {
        kept.retain(|(_, name)| name == wanted);
        if kept.is_empty() {
            return Err(Error::NoProject {
                name: wanted.clone(),
                period: options.period.label(),
            });
        }
    }
    let today = now.to_zoned(zone.clone()).date();
    let mut projects: BTreeMap<&str, Row> = BTreeMap::new();
    let mut models: BTreeMap<&str, Row> = BTreeMap::new();
    let mut days: BTreeMap<Date, Day> = BTreeMap::new();
    let mut sessions = BTreeSet::new();
    let mut total = Tokens::default();
    let mut total_cost = Cost::default();
    for (response, name) in &kept {
        let mut cost = Cost::default();
        cost.add(&response.model, &response.tokens);
        let project = projects.entry(name).or_insert_with(|| Row {
            name: (*name).to_owned(),
            path: response.cwd.clone(),
            tokens: Tokens::default(),
            cost: Cost::default(),
        });
        project.tokens.add(&response.tokens);
        project.cost.merge(&cost);
        let model = models.entry(&response.model).or_insert_with(|| Row {
            name: response.model.clone(),
            path: None,
            tokens: Tokens::default(),
            cost: Cost::default(),
        });
        model.tokens.add(&response.tokens);
        model.cost.merge(&cost);
        let date = Timestamp::from_second(response.at)
            .map_err(Error::Period)?
            .to_zoned(zone.clone())
            .date();
        let day = days.entry(date).or_insert_with(|| Day {
            date,
            today: date == today,
            tokens: Tokens::default(),
            cost: Cost::default(),
        });
        day.tokens.add(&response.tokens);
        day.cost.merge(&cost);
        if let Some(session) = &response.session {
            sessions.insert(session.as_str());
        }
        total.add(&response.tokens);
        total_cost.merge(&cost);
    }
    let unpriced = models
        .values()
        .filter(|m| m.cost.unpriced)
        .map(|m| m.name.clone())
        .collect();
    Ok(Summary {
        period: options.period,
        from,
        to: now.as_second(),
        project: options.project.clone(),
        sessions: sessions.len(),
        responses: kept.len(),
        projects: by_output(projects.into_values().collect()),
        models: by_output(models.into_values().collect()),
        days: days.into_values().collect(),
        total,
        total_cost,
        unpriced,
    })
}

/// Returns the start of `period` in Unix seconds, or `None` for all time.
fn start(period: Period, now: Timestamp, zone: &TimeZone) -> Result<Option<i64>, Error> {
    let today = now
        .to_zoned(zone.clone())
        .start_of_day()
        .map_err(Error::Period)?;
    let at = match period {
        Period::All => return Ok(None),
        Period::Today => today.timestamp(),
        Period::Hours(n) => now
            .checked_sub(SignedDuration::from_hours(i64::from(n)))
            .map_err(Error::Period)?,
        Period::Days(n) => today
            .checked_sub(Span::new().days(i64::from(n.saturating_sub(1))))
            .map_err(Error::Period)?
            .timestamp(),
    };
    Ok(Some(at.as_second()))
}

/// Sorts rows by output, most first, then by name.
fn by_output(mut rows: Vec<Row>) -> Vec<Row> {
    rows.sort_by(|a, b| {
        b.tokens
            .output
            .cmp(&a.tokens.output)
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

/// The shown name of each working folder.
type Names = BTreeMap<Option<String>, String>;

/// Returns the shown name of every working folder of `responses`.
///
/// A project is shown by its folder name. Two folders with the same name get
/// their parent folder added, and the full path when that is still the same.
fn project_names<'a>(responses: impl Iterator<Item = &'a Response>) -> Names {
    let folders: BTreeSet<Option<&str>> = responses.map(|r| r.cwd.as_deref()).collect();
    let short = |cwd: &str, parts: usize| -> String {
        let components: Vec<String> = Path::new(cwd)
            .components()
            .filter(|c| matches!(c, Component::Normal(_)))
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        let skip = components.len().saturating_sub(parts);
        components
            .into_iter()
            .skip(skip)
            .collect::<Vec<_>>()
            .join("/")
    };
    let mut names = Names::new();
    for parts in [1, 2] {
        let mut taken: BTreeMap<String, usize> = BTreeMap::new();
        for cwd in folders.iter().flatten() {
            *taken.entry(short(cwd, parts)).or_default() += 1;
        }
        for cwd in folders.iter().flatten() {
            let name = short(cwd, parts);
            let key = Some((*cwd).to_owned());
            if !names.contains_key(&key) && taken.get(&name) == Some(&1) {
                names.insert(key, name);
            }
        }
    }
    for cwd in folders.iter().flatten() {
        names
            .entry(Some((*cwd).to_owned()))
            .or_insert_with(|| (*cwd).to_owned());
    }
    names.insert(None, "(no folder)".to_owned());
    names
}

/// Returns the shown name of the project of `response`.
fn project_name<'a>(names: &'a Names, response: &Response) -> &'a str {
    names
        .get(&response.cwd)
        .map_or("(no folder)", String::as_str)
}

#[cfg(test)]
mod tests {
    use super::summarize;
    use crate::error::Error;
    use crate::stats::{Options, Period, Response, Tokens};
    use jiff::Timestamp;
    use jiff::tz::{Offset, TimeZone};

    /// 2026-10-15 12:00 UTC.
    const NOW: i64 = 1_792_065_600;

    fn response(id: &str, cwd: &str, model: &str, at: i64, output: u64) -> Response {
        Response {
            id: id.to_owned(),
            model: model.to_owned(),
            session: Some(format!("session-{cwd}")),
            cwd: Some(cwd.to_owned()),
            at,
            tokens: Tokens {
                input: 1,
                output,
                cache_write: 2,
                cache_write_1h: 0,
                cache_read: 3,
            },
        }
    }

    fn options(period: Period) -> Options {
        Options {
            period,
            ..Options::default()
        }
    }

    fn now() -> Timestamp {
        Timestamp::from_second(NOW).unwrap()
    }

    #[test]
    fn a_repeated_response_counts_once() {
        let responses = vec![
            response("m1", "/w/shop", "claude-opus-5-5", NOW - 60, 10),
            response("m1", "/w/shop", "claude-opus-5-5", NOW - 60, 30),
            response("m1", "/w/shop", "claude-opus-5-5", NOW - 60, 20),
        ];
        let s = summarize(responses, &options(Period::Today), now(), &TimeZone::UTC).unwrap();
        assert_eq!(s.responses, 1, "{s:?}");
        assert_eq!(s.total.output, 30, "the most output counts");
    }

    #[test]
    fn folders_with_the_same_name_get_their_parent() {
        let responses = vec![
            response("a", "/w/client/app", "m", NOW, 1),
            response("b", "/w/server/app", "m", NOW, 2),
            response("c", "/w/shop", "m", NOW, 3),
        ];
        let s = summarize(responses, &options(Period::Today), now(), &TimeZone::UTC).unwrap();
        let names: Vec<&str> = s.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["shop", "server/app", "client/app"]);
    }

    #[test]
    fn today_starts_at_local_midnight() {
        let zone = TimeZone::fixed(Offset::from_hours(2).unwrap());
        // 2026-10-14 22:30 UTC is 00:30 on the 15th at +02:00.
        let responses = vec![
            response("a", "/w/shop", "m", NOW - 13 * 3600 - 1800, 5),
            response("b", "/w/shop", "m", NOW - 15 * 3600, 7),
        ];
        let s = summarize(responses.clone(), &options(Period::Today), now(), &zone).unwrap();
        assert_eq!(s.total.output, 5, "only the response after local midnight");
        let utc = summarize(responses, &options(Period::Today), now(), &TimeZone::UTC).unwrap();
        assert_eq!(utc.total.output, 0, "both are before midnight UTC");
    }

    #[test]
    fn periods_filter_by_time() {
        let responses = vec![
            response("a", "/w/shop", "m", NOW - 3600, 1),
            response("b", "/w/shop", "m", NOW - 3 * 3600, 10),
            response("c", "/w/shop", "m", NOW - 3 * 86_400, 100),
            response("d", "/w/shop", "m", NOW - 40 * 86_400, 1000),
        ];
        let total = |period| {
            summarize(responses.clone(), &options(period), now(), &TimeZone::UTC)
                .unwrap()
                .total
                .output
        };
        assert_eq!(total(Period::Hours(2)), 1);
        assert_eq!(total(Period::Today), 11);
        assert_eq!(total(Period::Days(7)), 111);
        assert_eq!(total(Period::All), 1111);
    }

    #[test]
    fn days_are_counted_and_today_marked() {
        let responses = vec![
            response("a", "/w/shop", "m", NOW, 1),
            response("b", "/w/shop", "m", NOW - 86_400, 10),
        ];
        let s = summarize(responses, &options(Period::Days(7)), now(), &TimeZone::UTC).unwrap();
        let days: Vec<(String, bool, u64)> = s
            .days
            .iter()
            .map(|d| (d.date.to_string(), d.today, d.tokens.output))
            .collect();
        assert_eq!(
            days,
            [
                ("2026-10-14".to_owned(), false, 10),
                ("2026-10-15".to_owned(), true, 1)
            ]
        );
    }

    #[test]
    fn one_project_can_be_chosen() {
        let responses = vec![
            response("a", "/w/shop", "m", NOW, 1),
            response("b", "/w/blog", "m", NOW, 10),
        ];
        let mut chosen = options(Period::Today);
        chosen.project = Some("blog".to_owned());
        let s = summarize(responses.clone(), &chosen, now(), &TimeZone::UTC).unwrap();
        assert_eq!(s.total.output, 10);
        chosen.project = Some("nope".to_owned());
        let missing = summarize(responses, &chosen, now(), &TimeZone::UTC);
        assert!(
            matches!(missing, Err(Error::NoProject { .. })),
            "{missing:?}"
        );
    }

    #[test]
    fn a_model_without_a_price_is_named() {
        let responses = vec![
            response("a", "/w/shop", "claude-opus-5-5", NOW, 1),
            response("b", "/w/shop", "local-model", NOW, 10),
        ];
        let s = summarize(responses, &options(Period::Today), now(), &TimeZone::UTC).unwrap();
        assert_eq!(s.unpriced, ["local-model"]);
        assert!(s.total_cost.unpriced && s.total_cost.priced, "{s:?}");
        assert!(s.projects[0].cost.unpriced, "{s:?}");
    }
}
