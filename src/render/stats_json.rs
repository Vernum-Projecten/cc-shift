//! `cc-shift stats --json`: the statistics as one JSON document.
//!
//! The shape is part of the public interface: a change to it is a changelog
//! entry. Times are RFC 3339 in UTC and days are local dates. The cost
//! fields appear only with `--cost`.

use serde_json::{Map, Value, json};

use crate::stats::price;
use crate::stats::summary::{Cost, Summary};
use crate::stats::{Period, Tokens};

/// Renders the summary as pretty-printed JSON with a final newline.
#[must_use]
pub fn stats_json(summary: &Summary, cost: bool) -> String {
    let (kind, count) = match summary.period {
        Period::Today => ("today", None),
        Period::Hours(n) => ("hours", Some(n)),
        Period::Days(n) => ("days", Some(n)),
        Period::All => ("all", None),
    };
    let row = |name_key: &str, name: &str, path: Option<&str>, tokens: &Tokens, c: &Cost| {
        let mut fields = Map::new();
        fields.insert(name_key.to_owned(), json!(name));
        if name_key == "name" {
            fields.insert("path".to_owned(), json!(path));
        }
        kinds(&mut fields, tokens, cost.then_some(c));
        Value::Object(fields)
    };
    let mut document = Map::new();
    document.insert(
        "period".to_owned(),
        json!({
            "kind": kind,
            "count": count,
            "label": summary.period.label(),
            "from": summary.from.map(rfc3339),
            "to": rfc3339(summary.to),
        }),
    );
    document.insert("project".to_owned(), json!(summary.project));
    document.insert("sessions".to_owned(), json!(summary.sessions));
    document.insert("responses".to_owned(), json!(summary.responses));
    let mut total = Map::new();
    kinds(
        &mut total,
        &summary.total,
        cost.then_some(&summary.total_cost),
    );
    document.insert("total".to_owned(), Value::Object(total));
    document.insert(
        "projects".to_owned(),
        summary
            .projects
            .iter()
            .map(|p| row("name", &p.name, p.path.as_deref(), &p.tokens, &p.cost))
            .collect(),
    );
    document.insert(
        "models".to_owned(),
        summary
            .models
            .iter()
            .map(|m| row("model", &m.name, None, &m.tokens, &m.cost))
            .collect(),
    );
    document.insert(
        "days".to_owned(),
        summary
            .days
            .iter()
            .map(|d| {
                let mut fields = Map::new();
                fields.insert("date".to_owned(), json!(d.date.to_string()));
                fields.insert("today".to_owned(), json!(d.today));
                kinds(&mut fields, &d.tokens, cost.then_some(&d.cost));
                Value::Object(fields)
            })
            .collect(),
    );
    if cost {
        document.insert("unpriced_models".to_owned(), json!(summary.unpriced));
    }
    let mut text =
        serde_json::to_string_pretty(&Value::Object(document)).unwrap_or_else(|_| "{}".to_owned());
    text.push('\n');
    text
}

/// Adds the four token kinds and, when given, the cost to `fields`.
///
/// `cost_usd` is the cost of the responses with a known price, or `null`
/// when none has one; `cost_complete` says whether every response had one.
fn kinds(fields: &mut Map<String, Value>, tokens: &Tokens, cost: Option<&Cost>) {
    fields.insert("output".to_owned(), json!(tokens.output));
    fields.insert("input".to_owned(), json!(tokens.input));
    fields.insert("cache_write".to_owned(), json!(tokens.cache_write));
    fields.insert("cache_read".to_owned(), json!(tokens.cache_read));
    if let Some(c) = cost {
        let usd = c.priced.then(|| price::dollars_number(c.known));
        fields.insert("cost_usd".to_owned(), json!(usd));
        fields.insert("cost_complete".to_owned(), json!(!c.unpriced));
    }
}

/// Formats Unix seconds as RFC 3339 in UTC.
fn rfc3339(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds).map_or_else(|_| seconds.to_string(), |t| t.to_string())
}

#[cfg(test)]
mod tests {
    use super::stats_json;
    use crate::stats::summary::{Cost, Day, Row, Summary};
    use crate::stats::{Period, Tokens};
    use jiff::civil::date;
    use serde_json::{Value, json};

    fn tokens(output: u64) -> Tokens {
        Tokens {
            input: 1,
            output,
            cache_write: 2,
            cache_write_1h: 1,
            cache_read: 3,
        }
    }

    fn summary() -> Summary {
        let priced = Cost {
            known: 41_200_000_000_000,
            unpriced: false,
            priced: true,
        };
        let unpriced = Cost {
            known: 0,
            unpriced: true,
            priced: false,
        };
        Summary {
            period: Period::Days(7),
            from: Some(1_791_504_000),
            to: 1_792_065_600,
            project: None,
            sessions: 2,
            responses: 3,
            projects: vec![Row {
                name: "shop".to_owned(),
                path: Some("/w/shop".to_owned()),
                tokens: tokens(30),
                cost: Cost {
                    unpriced: true,
                    ..priced
                },
            }],
            models: vec![
                Row {
                    name: "claude-opus-5-5".to_owned(),
                    path: None,
                    tokens: tokens(20),
                    cost: priced,
                },
                Row {
                    name: "local-model".to_owned(),
                    path: None,
                    tokens: tokens(10),
                    cost: unpriced,
                },
            ],
            days: vec![Day {
                date: date(2026, 10, 15),
                today: true,
                tokens: tokens(30),
                cost: priced,
            }],
            total: tokens(30),
            total_cost: priced,
            unpriced: vec!["local-model".to_owned()],
        }
    }

    #[test]
    fn without_cost_there_are_no_cost_fields() {
        let value: Value = serde_json::from_str(&stats_json(&summary(), false)).unwrap();
        assert_eq!(
            value,
            json!({
                "period": {
                    "kind": "days",
                    "count": 7,
                    "label": "last 7 days",
                    "from": "2026-10-09T00:00:00Z",
                    "to": "2026-10-15T12:00:00Z"
                },
                "project": null,
                "sessions": 2,
                "responses": 3,
                "total": {"output": 30, "input": 1, "cache_write": 2, "cache_read": 3},
                "projects": [{
                    "name": "shop", "path": "/w/shop",
                    "output": 30, "input": 1, "cache_write": 2, "cache_read": 3
                }],
                "models": [
                    {"model": "claude-opus-5-5", "output": 20, "input": 1, "cache_write": 2, "cache_read": 3},
                    {"model": "local-model", "output": 10, "input": 1, "cache_write": 2, "cache_read": 3}
                ],
                "days": [{
                    "date": "2026-10-15", "today": true,
                    "output": 30, "input": 1, "cache_write": 2, "cache_read": 3
                }]
            })
        );
    }

    #[test]
    fn with_cost_each_group_has_its_cost() {
        let value: Value = serde_json::from_str(&stats_json(&summary(), true)).unwrap();
        assert_eq!(value["total"]["cost_usd"], json!(41.2));
        assert_eq!(value["total"]["cost_complete"], json!(true));
        assert_eq!(value["projects"][0]["cost_complete"], json!(false));
        assert_eq!(value["models"][1]["cost_usd"], Value::Null);
        assert_eq!(value["unpriced_models"], json!(["local-model"]));
    }
}
