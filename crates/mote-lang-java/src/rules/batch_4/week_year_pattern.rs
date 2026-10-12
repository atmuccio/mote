//! `java/week-year-pattern` — `YYYY` (week-based year) in a calendar date pattern.

use super::util::{self, F};
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    tags: &["date-time"],
    remediation_minutes: 2,
    description: r#"Detects a date pattern passed to `new SimpleDateFormat(..)`,
`SimpleDateFormat.applyPattern(..)`, or `DateTimeFormatter.ofPattern(..)` that uses `Y`
(week-based year) together with month (`M`/`L`) or day-of-month (`d`) fields, and no week
field (`w`); and the reverse mix, a week of year (`w`) with the calendar year (`y`, or `u` in
`java.time`) and no month or day field (`yyyy-'W'ww`).

**Why it matters.** `Y` is the year of the *week* a day belongs to, not the calendar year.
They differ for a few days around New Year: with `YYYY-MM-dd`, 30 December 2024 is
formatted as `2025-12-30`. The bug passes every test run during the rest of the year and
then corrupts dates, file names, and reports in late December or early January. The reverse
mix fails on the same days: 30 December 2024 is in week 1 of 2025, so `yyyy-'W'ww` prints
`2024-W01`.

**How to fix.** Use `y` (`yyyy-MM-dd`, or `uuuu-MM-dd` with `java.time`). Keep `Y` only for
ISO week dates, which also use `w` (`YYYY-'W'ww`), and write week-based dates with `Y`.

```java
// Noncompliant
DateTimeFormatter f = DateTimeFormatter.ofPattern("YYYY-MM-dd");

// Compliant
DateTimeFormatter f = DateTimeFormatter.ofPattern("yyyy-MM-dd");
```

Quoted text in the pattern (`'Year' YYYY`) is ignored; only string literals are checked."#,
    ..RuleMeta::new(
        "java/week-year-pattern",
        "java",
        "Week-based year in a calendar date pattern",
    )
};

#[derive(Default)]
pub struct WeekYearPattern {
    date_formats: util::Gate,
}

/// What is wrong with a date pattern's year field, if anything.
#[derive(Clone, Copy, PartialEq, Eq)]
enum YearMistake {
    /// `Y` (week-based year) with month or day-of-month fields and no `w` (week of year).
    WeekYear,
    /// `w` with the calendar year (`y`, or `u` in `java.time`) and no month or day field.
    CalendarYearWithWeek,
}

/// Whether a date pattern mixes the week-based and calendar year systems. `u_is_year`: `u` is
/// the year in `java.time` patterns (it is the day-of-week number in `SimpleDateFormat`).
fn year_mistake(pattern: &str, u_is_year: bool) -> Option<YearMistake> {
    let (mut week_year, mut calendar_year, mut week) = (false, false, false);
    let (mut month_or_day, mut day_of_year) = (false, false);
    let mut quoted = false;
    for c in pattern.chars() {
        if c == '\'' {
            // `''` toggles twice, which is a literal quote either way.
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match c {
            'Y' => week_year = true,
            'y' => calendar_year = true,
            'u' if u_is_year => calendar_year = true,
            'M' | 'L' | 'd' => month_or_day = true,
            'D' => day_of_year = true,
            'w' => week = true,
            _ => {}
        }
    }
    if week_year && month_or_day && !week {
        Some(YearMistake::WeekYear)
    } else if week && !week_year && calendar_year && !month_or_day && !day_of_year {
        Some(YearMistake::CalendarYearWithWeek)
    } else {
        None
    }
}

impl<'t> JavaRule<'t> for WeekYearPattern {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["object_creation_expression", "method_invocation"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["ofPattern", "applyPattern"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if !self
            .date_formats
            .open_any(ctx, &["SimpleDateFormat", "ofPattern", "applyPattern"])
        {
            return;
        }
        // Cheap name tests first.
        let candidate = if kind(node) == "object_creation_expression" {
            util::fld(node, F::Type).is_some_and(|t| ctx.text(t).ends_with("SimpleDateFormat"))
        } else {
            matches!(util::name_text(ctx, node), "ofPattern" | "applyPattern")
        };
        if !candidate {
            return;
        }
        let Some(first) = util::args(node).next() else {
            return;
        };
        if !syntax::is_string_literal(first) {
            return;
        }
        let text = ctx.text(first);
        if !text.contains('Y') && !text.contains('w') {
            return;
        }
        let Some(pattern) = util::string_value(ctx, first) else {
            return;
        };
        let u_is_year =
            kind(node) == "method_invocation" && util::name_text(ctx, node) == "ofPattern";
        let Some(mistake) = year_mistake(&pattern, u_is_year) else {
            return;
        };
        let ok = if kind(node) == "object_creation_expression" {
            util::fld(node, F::Type).is_some_and(|t| {
                ctx.text(t).ends_with("SimpleDateFormat")
                    && util::type_fqn(ctx, t).as_deref() == Some("java.text.SimpleDateFormat")
            })
        } else {
            let Some(recv) = util::fld(node, F::Object) else {
                return;
            };
            match util::name_text(ctx, node) {
                "ofPattern" => util::names_type(ctx, recv, "java.time.format.DateTimeFormatter"),
                "applyPattern" => ctx
                    .type_of(recv)
                    .is_some_and(|t| ctx.is_subtype_of(&t, "java.text.SimpleDateFormat")),
                _ => false,
            }
        };
        if !ok {
            return;
        }
        let message = match mistake {
            YearMistake::WeekYear => "Use `y` (calendar year) instead of `Y` (week-based year) in this date pattern; they differ around New Year",
            YearMistake::CalendarYearWithWeek => "Use `Y` (week-based year) with `w` (week of year) in this date pattern; with the calendar year, weeks that span New Year get the wrong year",
        };
        ctx.emit(first, message);
    }
}
