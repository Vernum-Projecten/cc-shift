// SPDX-FileCopyrightText: 2026 Vernum Projecten B.V.
// SPDX-License-Identifier: Apache-2.0

//! The `cc-shift stats` tables.
//!
//! Output is the column everything is sorted and shared by. Input, cache
//! writes and cache reads stand beside it, never added into one total,
//! because cache reads dwarf the rest.

use crate::render::{Paint, pad};
use crate::stats::price;
use crate::stats::summary::{Cost, Row, Summary};
use crate::stats::{Period, Tokens};

/// The cells of the share bar in a table with the four token kinds.
const WIDE_BAR: usize = 15;

/// The cells of the share bar in a table with output only.
const NARROW_BAR: usize = 16;

/// The columns beside the share, with the width each takes at least.
const KINDS: [(&str, usize); 3] = [("INPUT", 8), ("CACHE WRITE", 13), ("CACHE READ", 13)];

/// The header of the cost column.
const COST: &str = "≈ API COST";

/// One line of a table before layout.
struct Line {
    name: String,
    output: String,
    share: Option<u32>,
    extra: Vec<String>,
}

/// A table: its name header, bar width, extra columns and lines.
struct Table {
    heading: &'static str,
    bar: usize,
    extra: Vec<(&'static str, usize)>,
    lines: Vec<Line>,
}

/// Renders the summary as the `cc-shift stats` screen.
///
/// Without a project: the summary line, the projects with the four token
/// kinds and a total, the models, and with a period of days the days. With
/// a project: the summary line, the project's models with the four kinds,
/// and its days. `cost` adds the cost column and a line naming each model
/// without a known price.
#[must_use]
pub fn stats(summary: &Summary, cost: bool, paint: Paint) -> String {
    let mut out = header(summary, paint);
    out.push('\n');
    if summary.responses == 0 {
        let when = match summary.period {
            Period::Today | Period::Days(1) => "today".to_owned(),
            Period::All => "yet".to_owned(),
            period => format!("in the {}", period.label()),
        };
        out.push_str("\n  no responses ");
        out.push_str(&when);
        out.push('\n');
        return out;
    }
    let mut sections = Vec::new();
    if summary.project.is_some() {
        sections.push(kinds_table("MODEL", &summary.models, summary, cost, paint));
        sections.push(days_table(summary, paint));
    } else {
        sections.push(kinds_table(
            "PROJECT",
            &summary.projects,
            summary,
            cost,
            paint,
        ));
        sections.push(output_table(summary, paint));
        if summary.period.has_days() {
            sections.push(days_table(summary, paint));
        }
    }
    if !summary.accounts.is_empty() || summary.unsplit.output > 0 {
        sections.push(accounts_table(summary, paint));
    }
    for section in sections {
        out.push('\n');
        out.push_str(&section);
    }
    out
}

/// The line above the tables.
fn header(summary: &Summary, paint: Paint) -> String {
    let mut parts = vec![paint.bold("cc-shift stats"), summary.period.label()];
    if let Some(project) = &summary.project {
        parts.push(format!("project {project}"));
    }
    parts.push("all accounts together".to_owned());
    parts.push(plural(summary.sessions, "session"));
    parts.push(plural(summary.responses, "response"));
    parts.join(" · ")
}

/// Returns `count` with `word`, plural unless it is one.
fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        format!("1 {word}")
    } else {
        format!("{count} {word}s")
    }
}

/// The rows with the four token kinds and a total, with the cost column when
/// asked for.
fn kinds_table(
    heading: &'static str,
    rows: &[Row],
    summary: &Summary,
    cost: bool,
    paint: Paint,
) -> String {
    let mut extra = KINDS.to_vec();
    if cost {
        extra.push((COST, 0));
    }
    let kinds = |tokens: &Tokens, row_cost: Option<&Cost>| {
        let mut cells = vec![
            large(tokens.input),
            large(tokens.cache_write),
            large(tokens.cache_read),
        ];
        if let Some(c) = row_cost.filter(|_| cost) {
            cells.push(money(c));
        }
        cells
    };
    let mut lines: Vec<Line> = rows
        .iter()
        .map(|row| Line {
            name: row.name.clone(),
            output: small(row.tokens.output),
            share: Some(share(row.tokens.output, summary.total.output)),
            extra: kinds(&row.tokens, Some(&row.cost)),
        })
        .collect();
    let total_cost = Cost {
        unpriced: false,
        ..summary.total_cost
    };
    lines.push(Line {
        name: "total".to_owned(),
        output: small(summary.total.output),
        share: None,
        extra: kinds(&summary.total, Some(&total_cost)),
    });
    let mut text = layout(
        &Table {
            heading,
            bar: WIDE_BAR,
            extra,
            lines,
        },
        paint,
    );
    if cost && !summary.unpriced.is_empty() {
        text.push_str("  ? = no price known for ");
        text.push_str(&summary.unpriced.join(", "));
        text.push('\n');
    }
    text
}

/// The models with output and share only.
fn output_table(summary: &Summary, paint: Paint) -> String {
    share_table("MODEL", &summary.models, summary, paint)
}

/// The accounts with output and share, and the output from before the
/// history began, which no account can be named for.
fn accounts_table(summary: &Summary, paint: Paint) -> String {
    let mut text = String::new();
    if !summary.accounts.is_empty() {
        text.push_str(&share_table("ACCOUNT", &summary.accounts, summary, paint));
    }
    if summary.unsplit.output > 0 {
        text.push_str(if summary.accounts.is_empty() {
            "  per account: "
        } else {
            "  "
        });
        text.push_str(&small(summary.unsplit.output));
        text.push_str(" (before the history began: not split)\n");
    }
    text
}

/// The rows with output and share only.
fn share_table(heading: &'static str, rows: &[Row], summary: &Summary, paint: Paint) -> String {
    let lines = rows
        .iter()
        .map(|row| Line {
            name: row.name.clone(),
            output: small(row.tokens.output),
            share: Some(share(row.tokens.output, summary.total.output)),
            extra: Vec::new(),
        })
        .collect();
    layout(
        &Table {
            heading,
            bar: NARROW_BAR,
            extra: Vec::new(),
            lines,
        },
        paint,
    )
}

/// The days with output and share, today marked.
fn days_table(summary: &Summary, paint: Paint) -> String {
    let lines = summary
        .days
        .iter()
        .map(|day| Line {
            name: if day.today {
                format!("{} today", day.date)
            } else {
                day.date.to_string()
            },
            output: small(day.tokens.output),
            share: Some(share(day.tokens.output, summary.total.output)),
            extra: Vec::new(),
        })
        .collect();
    layout(
        &Table {
            heading: "DAY",
            bar: NARROW_BAR,
            extra: Vec::new(),
            lines,
        },
        paint,
    )
}

/// Lays a table out in columns sized to their longest cell.
fn layout(table: &Table, paint: Paint) -> String {
    let name_width = table
        .lines
        .iter()
        .map(|l| l.name.chars().count())
        .chain([table.heading.chars().count()])
        .max()
        .unwrap_or(0);
    let output_width = table
        .lines
        .iter()
        .map(|l| l.output.chars().count())
        .chain([5])
        .max()
        .unwrap_or(5);
    let output_column = (output_width + 1).max("OUTPUT".len());
    let share_width = table.bar + 5;
    let value_widths: Vec<usize> = (0..table.extra.len())
        .map(|i| {
            table
                .lines
                .iter()
                .filter_map(|l| l.extra.get(i))
                .map(|c| c.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let column_widths: Vec<usize> = table
        .extra
        .iter()
        .zip(&value_widths)
        .map(|((head, least), value)| (*least).max(head.chars().count().max(*value) + 2))
        .collect();
    let mut head = format!(
        "  {}   {}   {}",
        pad(table.heading, name_width),
        pad("OUTPUT", output_column),
        pad("SHARE", share_width)
    );
    let gap = if table.extra.is_empty() { "" } else { "   " };
    head.push_str(gap);
    for ((name, _), width) in table.extra.iter().zip(&column_widths) {
        head.push_str(&pad(name, *width));
    }
    let mut out = paint.dim(head.trim_end());
    out.push('\n');
    for line in &table.lines {
        let output = format!("{:>output_width$}", line.output);
        let share = match line.share {
            Some(percent) => format!("{} {percent:>3}%", bar(percent, table.bar)),
            None => String::new(),
        };
        let mut text = format!(
            "  {}   {}   {}",
            pad(&line.name, name_width),
            pad(&output, output_column),
            pad(&share, share_width)
        );
        text.push_str(gap);
        for ((cell, width), value) in line.extra.iter().zip(&column_widths).zip(&value_widths) {
            text.push_str(&pad(&format!("{cell:>value$}"), *width));
        }
        out.push_str(text.trim_end());
        out.push('\n');
    }
    out
}

/// Returns `part` as a whole percentage of `whole`, rounded half up.
fn share(part: u64, whole: u64) -> u32 {
    if whole == 0 {
        return 0;
    }
    let doubled = u128::from(part) * 200 + u128::from(whole);
    let percent = doubled.div_euclid(u128::from(whole) * 2);
    u32::try_from(percent.min(100)).unwrap_or(100)
}

/// Returns a bar of `cells` cells for a share of `percent`, the number of
/// full cells rounded to the nearest.
fn bar(percent: u32, cells: usize) -> String {
    let filled = f64::from(percent) / 100.0 * f64::from(u32::try_from(cells).unwrap_or(0));
    let full = (0..cells)
        .filter(|cell| filled >= f64::from(u32::try_from(*cell).unwrap_or(0)) + 0.5)
        .count();
    let mut text = "█".repeat(full);
    text.push_str(&"░".repeat(cells - full));
    text
}

/// Formats an output count: `512`, `70K`, `910K`, `1.2M`, `1.8B`.
fn small(count: u64) -> String {
    if count < 1_000 {
        return count.to_string();
    }
    let thousands = (count + 500).div_euclid(1_000);
    if thousands < 1_000 {
        format!("{thousands}K")
    } else {
        millions(count)
    }
}

/// Formats an input or cache count, in millions from a tenth of a million:
/// `30K`, `0.5M`, `17.6M`.
fn large(count: u64) -> String {
    if count < 95_000 {
        small(count)
    } else {
        millions(count)
    }
}

/// Formats a count in millions with one decimal, or in billions from a
/// thousand million: `17.6M`, `1.8B`.
fn millions(count: u64) -> String {
    let tenths = (count + 50_000).div_euclid(100_000);
    if tenths < 10_000 {
        return format!("{}.{}M", tenths.div_euclid(10), tenths.rem_euclid(10));
    }
    let tenths = (count + 50_000_000).div_euclid(100_000_000);
    format!("{}.{}B", tenths.div_euclid(10), tenths.rem_euclid(10))
}

/// Formats a cost: `$41.20`, `?` with no known price, and `+?` after the
/// known part when part of it has no price.
fn money(cost: &Cost) -> String {
    match (cost.priced, cost.unpriced) {
        (false, true) => "?".to_owned(),
        (true, true) => format!("{}+?", price::dollars(cost.known)),
        _ => price::dollars(cost.known),
    }
}

#[cfg(test)]
mod tests {
    use super::{bar, large, share, small, stats};
    use crate::render::Paint;
    use crate::stats::summary::{Cost, Day, Row, Summary};
    use crate::stats::{Period, Tokens};
    use jiff::civil::date;

    fn tokens(output: u64, input: u64, write: u64, read: u64) -> Tokens {
        Tokens {
            input,
            output,
            cache_write: write,
            cache_write_1h: 0,
            cache_read: read,
        }
    }

    fn cost(cents: u128) -> Cost {
        Cost {
            known: cents * 10_000_000_000,
            unpriced: false,
            priced: true,
        }
    }

    fn row(name: &str, tokens: Tokens, cost: Cost) -> Row {
        Row {
            name: name.to_owned(),
            path: None,
            tokens,
            cost,
        }
    }

    /// The numbers of the approved screen.
    fn approved() -> Summary {
        Summary {
            period: Period::Today,
            from: Some(0),
            to: 0,
            project: None,
            sessions: 14,
            responses: 312,
            projects: vec![
                row(
                    "backend-api",
                    tokens(410_000, 500_000, 2_900_000, 17_600_000),
                    cost(4120),
                ),
                row(
                    "website",
                    tokens(280_000, 300_000, 2_000_000, 12_400_000),
                    cost(2790),
                ),
                row(
                    "infra",
                    tokens(150_000, 200_000, 1_100_000, 7_000_000),
                    cost(1410),
                ),
                row(
                    "scratch",
                    tokens(70_000, 100_000, 400_000, 2_800_000),
                    cost(550),
                ),
            ],
            models: vec![
                row("claude-opus-5-5", tokens(843_000, 0, 0, 0), cost(0)),
                row("claude-sonnet-5-5", tokens(67_000, 0, 0, 0), cost(0)),
            ],
            days: vec![Day {
                date: date(2026, 10, 15),
                today: true,
                tokens: tokens(910_000, 0, 0, 0),
                cost: cost(0),
            }],
            total: tokens(910_000, 1_100_000, 6_400_000, 39_800_000),
            total_cost: cost(8870),
            unpriced: Vec::new(),
            accounts: Vec::new(),
            unsplit: Tokens::default(),
        }
    }

    const PROJECTS: &str = "\
cc-shift stats · today · all accounts together · 14 sessions · 312 responses

  PROJECT       OUTPUT   SHARE                  INPUT   CACHE WRITE  CACHE READ   ≈ API COST
  backend-api    410K    ███████░░░░░░░░  45%   0.5M    2.9M         17.6M        $41.20
  website        280K    █████░░░░░░░░░░  31%   0.3M    2.0M         12.4M        $27.90
  infra          150K    ██░░░░░░░░░░░░░  16%   0.2M    1.1M          7.0M        $14.10
  scratch         70K    █░░░░░░░░░░░░░░   8%   0.1M    0.4M          2.8M         $5.50
  total          910K                           1.1M    6.4M         39.8M        $88.70

  MODEL               OUTPUT   SHARE
  claude-opus-5-5      843K    ███████████████░  93%
  claude-sonnet-5-5     67K    █░░░░░░░░░░░░░░░   7%
";

    #[test]
    fn renders_the_approved_screen_with_cost() {
        assert_eq!(stats(&approved(), true, Paint::new(false)), PROJECTS);
    }

    #[test]
    fn without_cost_there_is_no_cost_column() {
        let text = stats(&approved(), false, Paint::new(false));
        let mut expected = String::new();
        for l in PROJECTS.lines() {
            let l = l.split("  ≈ API COST").next().unwrap();
            let l = l.split("        $").next().unwrap();
            let l = l.split("         $").next().unwrap();
            expected.push_str(l.trim_end());
            expected.push('\n');
        }
        assert_eq!(text, expected);
    }

    #[test]
    fn a_period_of_days_adds_the_days() {
        let mut summary = approved();
        summary.period = Period::Days(7);
        let text = stats(&summary, false, Paint::new(false));
        assert!(
            text.starts_with("cc-shift stats · last 7 days · all accounts"),
            "{text}"
        );
        assert!(
            text.ends_with(
                "\n\n  DAY                OUTPUT   SHARE\n  \
                 2026-10-15 today    910K    ████████████████ 100%\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn an_unpriced_model_shows_a_question_mark() {
        let mut summary = approved();
        summary.projects[3].cost = Cost {
            known: 0,
            unpriced: true,
            priced: false,
        };
        summary.projects[2].cost.unpriced = true;
        summary.total_cost.unpriced = true;
        summary.unpriced = vec!["local-model".to_owned()];
        let text = stats(&summary, true, Paint::new(false));
        assert!(text.contains("2.8M               ?\n"), "{text}");
        assert!(text.contains("7.0M        $14.10+?\n"), "{text}");
        assert!(text.contains("39.8M          $88.70\n"), "{text}");
        assert!(
            text.contains("$88.70\n  ? = no price known for local-model\n\n  MODEL"),
            "{text}"
        );
    }

    #[test]
    fn a_project_shows_its_models_and_days() {
        let mut summary = approved();
        summary.project = Some("website".to_owned());
        summary.models = vec![row(
            "claude-opus-5-5",
            tokens(280_000, 300_000, 2_000_000, 12_400_000),
            cost(2790),
        )];
        summary.total = tokens(280_000, 300_000, 2_000_000, 12_400_000);
        summary.total_cost = cost(2790);
        summary.days[0].tokens = summary.total;
        assert_eq!(
            stats(&summary, false, Paint::new(false)),
            "\
cc-shift stats · today · project website · all accounts together · 14 sessions · 312 responses

  MODEL             OUTPUT   SHARE                  INPUT   CACHE WRITE  CACHE READ
  claude-opus-5-5    280K    ███████████████ 100%   0.3M    2.0M         12.4M
  total              280K                           0.3M    2.0M         12.4M

  DAY                OUTPUT   SHARE
  2026-10-15 today    280K    ████████████████ 100%
"
        );
    }

    #[test]
    fn nothing_in_the_period_says_so() {
        let mut summary = approved();
        summary.responses = 0;
        summary.sessions = 0;
        assert_eq!(
            stats(&summary, false, Paint::new(false)),
            "cc-shift stats · today · all accounts together · 0 sessions · 0 responses\n\n  no responses today\n"
        );
    }

    #[test]
    fn colour_only_dims_the_headers_and_bolds_the_title() {
        let text = stats(&approved(), false, Paint::new(true));
        assert!(
            text.starts_with("\u{1b}[1mcc-shift stats\u{1b}[0m · today"),
            "{text}"
        );
        assert!(text.contains("\u{1b}[2m  PROJECT"), "{text}");
    }

    #[test]
    fn counts_use_k_and_m() {
        assert_eq!(small(512), "512");
        assert_eq!(small(69_600), "70K");
        assert_eq!(small(999_499), "999K");
        assert_eq!(small(999_500), "1.0M");
        assert_eq!(small(12_345_678), "12.3M");
        assert_eq!(large(30_000), "30K");
        assert_eq!(large(500_000), "0.5M");
        assert_eq!(large(17_649_999), "17.6M");
        assert_eq!(large(999_949_999), "999.9M");
        assert_eq!(large(999_950_000), "1.0B");
        assert_eq!(large(23_034_700_000), "23.0B");
    }

    #[test]
    fn shares_and_bars_round() {
        assert_eq!(share(1, 3), 33);
        assert_eq!(share(1, 2), 50);
        assert_eq!(share(0, 0), 0);
        assert_eq!(bar(45, 15), "███████░░░░░░░░");
        assert_eq!(bar(100, 16), "████████████████");
        assert_eq!(bar(0, 4), "░░░░");
    }

    #[test]
    fn per_account_adds_the_accounts_and_what_came_before_the_history() {
        let mut summary = approved();
        summary.accounts = vec![
            row("work", tokens(600_000, 0, 0, 0), cost(0)),
            row("spare", tokens(250_000, 0, 0, 0), cost(0)),
        ];
        summary.unsplit = tokens(60_000, 0, 0, 0);
        let text = stats(&summary, false, Paint::new(false));
        assert!(
            text.ends_with(
                "\n  ACCOUNT   OUTPUT   SHARE
  work       600K    ███████████░░░░░  66%
  spare      250K    ████░░░░░░░░░░░░  27%
  60K (before the history began: not split)
"
            ),
            "{text}"
        );
    }

    #[test]
    fn per_account_after_the_history_began_has_no_extra_line() {
        let mut summary = approved();
        summary.accounts = vec![row("work", tokens(910_000, 0, 0, 0), cost(0))];
        let text = stats(&summary, false, Paint::new(false));
        assert!(
            text.ends_with(
                "\n  ACCOUNT   OUTPUT   SHARE\n  work       910K    ████████████████ 100%\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn per_account_before_any_switch_says_so_without_a_table() {
        let mut summary = approved();
        summary.unsplit = tokens(910_000, 0, 0, 0);
        let text = stats(&summary, false, Paint::new(false));
        assert!(
            text.ends_with("\n  per account: 910K (before the history began: not split)\n"),
            "{text}"
        );
    }
}
