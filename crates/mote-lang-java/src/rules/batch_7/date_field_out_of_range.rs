//! `java/date-field-out-of-range` — `LocalDate.of(2024, 13, 1)`, `LocalTime.of(24, 0)`,
//! `calendar.set(Calendar.MONTH, 12)`: date and time fields outside their range.

use super::util;
use crate::prelude::*;

pub static META: RuleMeta = RuleMeta {
    kind: RuleKind::Bug,
    quality: Quality::Reliability,
    severity: Severity::High,
    status: RuleStatus::Incubating,
    tags: &["dates", "api-misuse"],
    remediation_minutes: 2,
    description: r#"Detects constant date and time fields outside their valid range:

* `java.time` factories — `LocalDate.of`, `LocalDateTime.of`, `LocalTime.of`,
  `YearMonth.of`, `MonthDay.of`, `Month.of` — with a month outside 1–12, a day outside the
  month's days, an hour outside 0–23, a minute or second outside 0–59, or nanoseconds outside
  0–999,999,999;
* `java.util.Calendar` months above 11: `calendar.set(Calendar.MONTH, 12)`,
  `calendar.set(2024, 12, 1)`, `new GregorianCalendar(2024, 12, 31)`.

**Why it matters.** The `java.time` factories throw `DateTimeException` for such values, every
time. `Calendar` months are zero-based (January is 0, December is 11): writing the month
number people use (12 for December) makes a lenient calendar silently roll over into January
of the next year, which is the classic off-by-one in date code.

**How to fix.** Use the correct number, or better a named constant: `Month.DECEMBER`,
`Calendar.DECEMBER`.

```java
// Noncompliant
Calendar end = new GregorianCalendar(2024, 12, 31);
LocalTime closing = LocalTime.of(24, 0);

// Compliant
Calendar end = new GregorianCalendar(2024, Calendar.DECEMBER, 31);
LocalTime closing = LocalTime.MIDNIGHT;
```

Only integer literals are checked. Other `Calendar` fields are not reported: lenient
roll-over there is sometimes used on purpose (day 0 as "last day of the previous month")."#,
    ..RuleMeta::new(
        "java/date-field-out-of-range",
        "java",
        "Date or time field out of range",
    )
};

#[derive(Default)]
pub struct DateFieldOutOfRange {
    dates: util::Gate,
}

#[derive(Clone, Copy)]
enum F {
    Month,
    Day,
    Hour,
    Minute,
    Second,
    Nano,
    Year,
}

impl F {
    fn range(self) -> (i64, i64, &'static str) {
        match self {
            F::Month => (1, 12, "month"),
            F::Day => (1, 31, "day of month"),
            F::Hour => (0, 23, "hour"),
            F::Minute => (0, 59, "minute"),
            F::Second => (0, 59, "second"),
            F::Nano => (0, 999_999_999, "nano-of-second"),
            F::Year => (i64::MIN, i64::MAX, "year"),
        }
    }
}

/// The fields of a `java.time` factory, by class and arity.
fn java_time_fields(class: &str, n: usize) -> Option<&'static [F]> {
    use F::*;
    Some(match (class, n) {
        ("LocalDate", 3) => &[Year, Month, Day],
        ("LocalDateTime", 5) => &[Year, Month, Day, Hour, Minute],
        ("LocalDateTime", 6) => &[Year, Month, Day, Hour, Minute, Second],
        ("LocalDateTime", 7) => &[Year, Month, Day, Hour, Minute, Second, Nano],
        ("LocalTime", 2) => &[Hour, Minute],
        ("LocalTime", 3) => &[Hour, Minute, Second],
        ("LocalTime", 4) => &[Hour, Minute, Second, Nano],
        ("YearMonth", 2) => &[Year, Month],
        ("MonthDay", 2) => &[Month, Day],
        ("Month", 1) => &[Month],
        _ => return None,
    })
}

/// Days in a month (February counted as 29: the year may be a leap year).
fn max_day(month: i64) -> i64 {
    match month {
        2 => 29,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl<'t> JavaRule<'t> for DateFieldOutOfRange {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &'static [&'static str] {
        &["method_invocation", "object_creation_expression"]
    }

    fn call_names(&self) -> &'static [&'static str] {
        &["of", "set"]
    }

    fn check(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        if kind(node) == "object_creation_expression" {
            self.check_new(node, ctx);
            return;
        }
        let Some(name) = ctx.call_name(node) else {
            return;
        };
        if !self.dates.open_any(ctx, &["java.time", "Calendar"]) {
            return;
        }
        let n = util::arg_count(node);
        if name == "of" {
            let Some(recv) = node.field(Field::Object) else {
                return;
            };
            let class = util::last_segment(ctx.text(recv));
            let Some(fields) = java_time_fields(class, n) else {
                return;
            };
            // Literal values first (cheap), then the receiver type.
            let mut month = None;
            let mut bad: Option<(Node<'t>, String)> = None;
            for (i, f) in fields.iter().enumerate() {
                let Some(a) = util::arg(node, i) else {
                    return;
                };
                let Some(v) = util::int_value(ctx, a) else {
                    continue;
                };
                let (lo, hi, what) = f.range();
                if matches!(f, F::Month) && (lo..=hi).contains(&v) {
                    month = Some(v);
                }
                let hi = if matches!(f, F::Day) {
                    month.map(max_day).unwrap_or(hi)
                } else {
                    hi
                };
                if !(lo..=hi).contains(&v) {
                    bad = Some((a, format!("{v} is not a valid {what} ({lo}-{hi})")));
                    break;
                }
            }
            let Some((at, msg)) = bad else {
                return;
            };
            let fqn = format!("java.time.{class}");
            if !util::names_type(ctx, recv, &fqn) {
                return;
            }
            ctx.emit(
                at,
                format!("Fix this argument: {msg}; `{class}.of` throws DateTimeException"),
            );
            return;
        }
        // Calendar.set(MONTH, m) and set(year, month, day[, ...]).
        let month_arg = match n {
            2 => {
                let Some(f) = util::arg(node, 0) else {
                    return;
                };
                let text = ctx.text(f);
                if util::last_segment(text) != "MONTH" {
                    return;
                }
                if text.contains('.') && !text.ends_with("Calendar.MONTH") {
                    return;
                }
                util::arg(node, 1)
            }
            3 | 5 | 6 => util::arg(node, 1),
            _ => return,
        };
        let Some(m) = month_arg else {
            return;
        };
        let Some(v) = util::int_value(ctx, m) else {
            return;
        };
        if (0..=11).contains(&v) || v < 0 {
            return;
        }
        if !util::receiver_is(ctx, node, &["java.util.Calendar"]) {
            return;
        }
        ctx.emit(m, calendar_msg(v));
    }
}

fn calendar_msg(v: i64) -> String {
    format!("Use `Calendar.DECEMBER`-style constants: Calendar months are 0-11, so {v} rolls over into the next year")
}

impl DateFieldOutOfRange {
    fn check_new<'t>(&mut self, node: Node<'t>, ctx: &Ctx<'t>) {
        let Some(ty) = node.field(Field::Type) else {
            return;
        };
        if util::last_segment(ctx.text(ty)) != "GregorianCalendar" {
            return;
        }
        if !matches!(util::arg_count(node), 3 | 5 | 6) {
            return;
        }
        let Some(m) = util::arg(node, 1) else {
            return;
        };
        let Some(v) = util::int_value(ctx, m) else {
            return;
        };
        if v <= 11 {
            return;
        }
        if ctx.resolve_type_name(ctx.text(ty), ty).as_deref() != Some("java.util.GregorianCalendar")
        {
            return;
        }
        ctx.emit(m, calendar_msg(v));
    }
}
