//! Opt-in profiling of the analysis: per-rule cumulative time and the framework's phases.
//!
//! Set `MOTE_RULE_TIMINGS=1` (read once per process) and the plugin measures every
//! `check`/`finish` call, each phase of [`crate::JavaPlugin`]'s analysis, and the resolver's
//! internal [`Section`]s, accumulating the totals across files and threads. [`rule_timings`] /
//! [`phase_timings`] read them; the corpus test and the `dump` example print them with
//! [`report`]. Off by default: the only cost then is one flag test per file and one branch per
//! resolver entry point.
//!
//! Resolver work is shared through per-file caches, so the first rule that asks for a type pays
//! for it; the table shows where time goes, not an exact per-rule cost.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

static ENABLED: LazyLock<AtomicBool> = LazyLock::new(|| {
    let on = std::env::var("MOTE_RULE_TIMINGS").is_ok_and(|v| !v.is_empty() && v != "0");
    AtomicBool::new(on)
});

/// Whether timing is on (`MOTE_RULE_TIMINGS=1`, or [`set_enabled`]).
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Turn timing on or off for the rest of the process (overrides the environment variable).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Phases of one file's analysis, in the order they run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// tree-sitter parse (with the work budget).
    Parse,
    /// Rule instances, `Ctx` (imports, line index).
    Setup,
    /// The walk, excluding rule `check` calls: dispatch, metrics, comments, suppressions.
    Walk,
    /// Rule `check` calls.
    Check,
    /// Rule `finish` calls.
    Finish,
    /// Sorting and packaging the results.
    Collect,
}

const PHASES: [Phase; 6] = [
    Phase::Parse,
    Phase::Setup,
    Phase::Walk,
    Phase::Check,
    Phase::Finish,
    Phase::Collect,
];

/// Cumulative time of one rule.
#[derive(Clone, Debug)]
pub struct RuleTiming {
    pub rule: &'static str,
    /// Time in `check` calls.
    pub check: Duration,
    /// Time in `finish`.
    pub finish: Duration,
    /// Number of `check` calls.
    pub calls: u64,
}

impl RuleTiming {
    pub fn total(&self) -> Duration {
        self.check + self.finish
    }

    /// Total minus the clock's share of each timed `check` (see [`clock_overhead`]).
    pub fn net(&self, overhead: Duration) -> Duration {
        let clock = overhead.saturating_mul(self.calls.min(u32::MAX as u64) as u32);
        self.check.saturating_sub(clock) + self.finish
    }
}

#[derive(Default)]
struct Totals {
    /// Per rule (registry order): (rule id, check ns, finish ns, calls).
    rules: Vec<(&'static str, u64, u64, u64)>,
    phases: [u64; 6],
    files: u64,

    sections: [(u64, u64); SECTIONS],
}

static TOTALS: LazyLock<Mutex<Totals>> = LazyLock::new(|| Mutex::new(Totals::default()));

/// Framework sections timed (inclusive, outermost entry only) and counted when profiling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    /// Calls into the resolver's public API (`type_of`, `is_call_to`, …).
    Resolver,
    /// Simple type-name resolution.
    ResolveSimple,
    /// Variable lookup by name.
    LookupVar,
    /// Resolving a type node.
    TypeFromNode,
    /// Building the local-declaration index of a member.
    BuildScope,
    /// Building the facts about a class of this file.
    ClassInfo,
    /// Return type of a method call (candidate search and overload selection).
    MethodType,
    /// Subtype checks.
    Subtype,
    /// Supertype index for override checks, and the checks.
    Overrides,
    /// `ancestors` of a node off the walk stack (a descent from the root).
    AncestorsFromRoot,
    /// `ancestors` of a descendant of the node being checked (a descent from that node).
    AncestorsBelow,
}

pub(crate) const SECTIONS: usize = 11;

const ALL_SECTIONS: [Section; SECTIONS] = [
    Section::Resolver,
    Section::ResolveSimple,
    Section::LookupVar,
    Section::TypeFromNode,
    Section::BuildScope,
    Section::ClassInfo,
    Section::MethodType,
    Section::Subtype,
    Section::Overrides,
    Section::AncestorsFromRoot,
    Section::AncestorsBelow,
];

#[derive(Default)]
pub(crate) struct SectionState {
    depth: Cell<u32>,
    start: Cell<Option<Instant>>,
    ns: Cell<u64>,
    count: Cell<u64>,
}

/// Section times of one file, kept in the `Ctx` (only when profiling).
#[derive(Default)]
pub(crate) struct CtxProf {
    sections: [SectionState; SECTIONS],
}

/// Times one section until dropped (see `Ctx::section`).
pub(crate) struct SectionGuard<'a>(Option<&'a SectionState>);

impl<'a> SectionGuard<'a> {
    #[inline]
    pub(crate) fn new(prof: Option<&'a CtxProf>, s: Section) -> Self {
        let Some(p) = prof else {
            return SectionGuard(None);
        };
        let st = &p.sections[s as usize];
        let d = st.depth.get();
        if d == 0 {
            st.count.set(st.count.get() + 1);
            st.start.set(Some(Instant::now()));
        }
        st.depth.set(d + 1);
        SectionGuard(Some(st))
    }
}

impl Drop for SectionGuard<'_> {
    fn drop(&mut self) {
        if let Some(st) = self.0 {
            let d = st.depth.get().saturating_sub(1);
            st.depth.set(d);
            if d == 0 {
                if let Some(s) = st.start.take() {
                    st.ns.set(st.ns.get() + s.elapsed().as_nanos() as u64);
                }
            }
        }
    }
}

/// Per-file accumulator, merged into the process totals when the file is done.
pub(crate) struct FileTimes {
    pub(crate) check_ns: Vec<u64>,
    pub(crate) finish_ns: Vec<u64>,
    pub(crate) calls: Vec<u64>,
    pub(crate) phases: [u64; 6],
}

impl FileTimes {
    pub(crate) fn new(rules: usize) -> Self {
        FileTimes {
            check_ns: vec![0; rules],
            finish_ns: vec![0; rules],
            calls: vec![0; rules],
            phases: [0; 6],
        }
    }

    pub(crate) fn add_phase(&mut self, p: Phase, d: Duration) {
        if let Some(slot) = self.phases.get_mut(p as usize) {
            *slot += d.as_nanos() as u64;
        }
    }

    /// The walk is timed as a whole; account the `check` calls it contains separately.
    pub(crate) fn move_checks_out_of_walk(&mut self) {
        let checks: u64 = self.check_ns.iter().sum();
        let walk = Phase::Walk as usize;
        self.phases[walk] = self.phases[walk].saturating_sub(checks);
        self.phases[Phase::Check as usize] += checks;
    }

    pub(crate) fn merge(self, ids: &[&'static mote_core::RuleMeta], prof: Option<&CtxProf>) {
        let Ok(mut t) = TOTALS.lock() else { return };
        if let Some(p) = prof {
            for (a, b) in t.sections.iter_mut().zip(&p.sections) {
                a.0 += b.ns.get();
                a.1 += b.count.get();
            }
        }
        if t.rules.len() < ids.len() {
            let have = t.rules.len();
            t.rules
                .extend(ids.iter().skip(have).map(|m| (m.id, 0u64, 0u64, 0u64)));
        }
        for (i, slot) in t.rules.iter_mut().enumerate() {
            slot.1 += self.check_ns.get(i).copied().unwrap_or(0);
            slot.2 += self.finish_ns.get(i).copied().unwrap_or(0);
            slot.3 += self.calls.get(i).copied().unwrap_or(0);
        }
        for (a, b) in t.phases.iter_mut().zip(self.phases) {
            *a += b;
        }
        t.files += 1;
    }
}

/// Per-rule totals since the start (or the last [`reset`]), most expensive first.
pub fn rule_timings() -> Vec<RuleTiming> {
    let Ok(t) = TOTALS.lock() else {
        return Vec::new();
    };
    let mut out: Vec<RuleTiming> = t
        .rules
        .iter()
        .map(|(rule, c, f, n)| RuleTiming {
            rule,
            check: Duration::from_nanos(*c),
            finish: Duration::from_nanos(*f),
            calls: *n,
        })
        .collect();
    out.sort_by(|a, b| b.total().cmp(&a.total()).then(a.rule.cmp(b.rule)));
    out
}

/// Phase totals since the start (or the last [`reset`]).
pub fn phase_timings() -> Vec<(Phase, Duration)> {
    let Ok(t) = TOTALS.lock() else {
        return Vec::new();
    };
    PHASES
        .iter()
        .zip(t.phases)
        .map(|(p, ns)| (*p, Duration::from_nanos(ns)))
        .collect()
}

/// Clear the accumulated totals.
pub fn reset() {
    if let Ok(mut t) = TOTALS.lock() {
        *t = Totals::default();
    }
}

/// A printable report: phases, then the `top` most expensive rules.
pub fn report(top: usize) -> String {
    use std::fmt::Write;
    let files = TOTALS.lock().map(|t| t.files).unwrap_or(0);
    let mut s = String::new();
    let _ = writeln!(s, "timings over {files} file(s) (MOTE_RULE_TIMINGS):");
    for (p, d) in phase_timings() {
        let _ = writeln!(s, "  {:<10} {:9.1} ms", format!("{p:?}"), ms(d));
    }
    let sections = TOTALS
        .lock()
        .map(|t| t.sections)
        .unwrap_or([(0, 0); SECTIONS]);
    let _ = writeln!(
        s,
        "  framework sections (inclusive; inside Check/Finish/Walk):"
    );
    for (sec, (ns, n)) in ALL_SECTIONS.iter().zip(sections) {
        let _ = writeln!(
            s,
            "    {:<18} {:9.1} ms {n:>10} calls",
            format!("{sec:?}"),
            ms(Duration::from_nanos(ns))
        );
    }
    let overhead = clock_overhead();
    let mut rules = rule_timings();
    rules.sort_by(|a, b| {
        b.net(overhead)
            .cmp(&a.net(overhead))
            .then(a.rule.cmp(b.rule))
    });
    let total: Duration = rules.iter().map(RuleTiming::total).sum();
    let net: Duration = rules.iter().map(|r| r.net(overhead)).sum();
    let _ = writeln!(
        s,
        "  rules total {:9.1} ms, net of clock overhead ({:.0} ns per check) {:9.1} ms; top {top} by net:",
        ms(total),
        overhead.as_nanos(),
        ms(net)
    );
    let _ = writeln!(
        s,
        "  {:<44} {:>9} {:>9} {:>9} {:>10}",
        "rule", "net ms", "check ms", "finish ms", "calls"
    );
    for r in rules.iter().take(top) {
        let _ = writeln!(
            s,
            "  {:<44} {:9.1} {:9.1} {:9.1} {:>10}",
            r.rule,
            ms(r.net(overhead)),
            ms(r.check),
            ms(r.finish),
            r.calls
        );
    }
    s
}

/// Average time an empty timed interval reports: the clock's own share of every measured
/// `check` call, subtracted by [`RuleTiming::net`].
pub fn clock_overhead() -> Duration {
    let mut best = u64::MAX;
    for _ in 0..5 {
        let mut sum = 0u64;
        for _ in 0..10_000 {
            let start = Instant::now();
            sum += start.elapsed().as_nanos() as u64;
        }
        best = best.min(sum / 10_000);
    }
    Duration::from_nanos(best)
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
