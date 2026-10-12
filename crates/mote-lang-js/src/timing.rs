//! Opt-in profiling of the analysis: per-rule cumulative time and the plugin's phases.
//!
//! Set `MOTE_RULE_TIMINGS=1` (read once per process) and the plugin measures every
//! [`run`](crate::JsRule::run) / [`run_once`](crate::JsRule::run_once) call and each phase of a
//! file's analysis, accumulating the totals across files and threads. [`rule_timings`] /
//! [`phase_timings`] read them; the corpus test prints them with [`report`]. Off by default: the
//! only cost then is one flag test per file.
//!
//! Intervals are read from the CPU's time-stamp counter where there is one (a few ns per read,
//! against ~30 ns for the OS clock on Windows), so the clock's share of millions of short `run`
//! calls stays small and [`RuleTiming::net`] can subtract it reliably.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// A clock reading in ticks (see [`ticks_per_ns`]).
#[inline]
pub(crate) fn stamp() -> u64 {
    #[cfg(target_arch = "x86_64")]
    {
        // SAFETY: `rdtsc` is available on every x86_64 CPU.
        unsafe { std::arch::x86_64::_rdtsc() }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);
        ORIGIN.elapsed().as_nanos() as u64
    }
}

/// Clock ticks per nanosecond, calibrated once against the OS clock.
fn ticks_per_ns() -> f64 {
    static CAL: LazyLock<f64> = LazyLock::new(|| {
        let (t0, s0) = (Instant::now(), stamp());
        while t0.elapsed() < Duration::from_millis(50) {
            std::hint::spin_loop();
        }
        let (ns, ticks) = (t0.elapsed().as_nanos() as f64, (stamp() - s0) as f64);
        if ns > 0.0 && ticks > 0.0 {
            ticks / ns
        } else {
            1.0
        }
    });
    *CAL
}

fn ticks_to_duration(ticks: u64) -> Duration {
    Duration::from_nanos((ticks as f64 / ticks_per_ns()) as u64)
}

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
    /// Oxc parse, line index, comments and suppressions.
    Parse,
    /// Oxc semantic model.
    Semantic,
    /// The facts pass (metrics, symbols, node-type presence).
    Facts,
    /// `applies_to` gates.
    Setup,
    /// The node walk, excluding rule `run` calls (dispatch).
    Walk,
    /// Rule `run` calls.
    Run,
    /// Rule `run_once` calls.
    RunOnce,
    /// Sorting the findings.
    Collect,
}

const PHASES: [Phase; 8] = [
    Phase::Parse,
    Phase::Semantic,
    Phase::Facts,
    Phase::Setup,
    Phase::Walk,
    Phase::Run,
    Phase::RunOnce,
    Phase::Collect,
];

/// Cumulative time of one rule.
#[derive(Clone, Debug)]
pub struct RuleTiming {
    pub rule: &'static str,
    /// Time in `run` calls.
    pub run: Duration,
    /// Time in `run_once`.
    pub run_once: Duration,
    /// Number of `run` calls.
    pub calls: u64,
    /// Time in `applies_to`.
    pub gate: Duration,
}

impl RuleTiming {
    pub fn total(&self) -> Duration {
        self.run + self.run_once + self.gate
    }

    /// Total minus the clock's share of each timed `run` (`overhead_ns` per call, see
    /// [`clock_overhead_ns`]).
    pub fn net(&self, overhead_ns: f64) -> Duration {
        let clock = Duration::from_nanos((overhead_ns * self.calls as f64) as u64);
        self.run.saturating_sub(clock) + self.run_once + self.gate
    }
}

#[derive(Default)]
struct Totals {
    /// Per rule (registry order): (rule id, run ns, run_once ns, calls, applies_to ns).
    rules: Vec<(&'static str, u64, u64, u64, u64)>,
    phases: [u64; 8],
    files: u64,
}

static TOTALS: LazyLock<Mutex<Totals>> = LazyLock::new(|| Mutex::new(Totals::default()));

/// Per-file accumulator, merged into the process totals when the file is done.
pub(crate) struct FileTimes {
    run_ns: Vec<u64>,
    once_ns: Vec<u64>,
    calls: Vec<u64>,
    gate_ns: Vec<u64>,
    phases: [u64; 8],
    /// End of the last lap ([`stamp`]).
    last: u64,
}

impl FileTimes {
    pub(crate) fn new(rules: usize) -> Self {
        FileTimes {
            run_ns: vec![0; rules],
            once_ns: vec![0; rules],
            calls: vec![0; rules],
            gate_ns: vec![0; rules],
            phases: [0; 8],
            last: stamp(),
        }
    }

    /// Time since the previous lap (or the start) goes to phase `p`.
    pub(crate) fn lap(&mut self, p: Phase) {
        let now = stamp();
        self.phases[p as usize] += now.wrapping_sub(self.last);
        self.last = now;
    }

    /// One `run` call of rule `index` that started at `start`.
    #[inline]
    pub(crate) fn add_run(&mut self, index: usize, start: u64) {
        self.run_ns[index] += stamp().wrapping_sub(start);
        self.calls[index] += 1;
    }

    /// The `applies_to` call of rule `index` that started at `start`.
    pub(crate) fn add_gate(&mut self, index: usize, start: u64) {
        self.gate_ns[index] += stamp().wrapping_sub(start);
    }

    /// The `run_once` call of rule `index` that started at `start`.
    pub(crate) fn add_run_once(&mut self, index: usize, start: u64) {
        self.once_ns[index] += stamp().wrapping_sub(start);
    }

    /// Add this file's times to the process totals. The walk was timed as a whole; the `run`
    /// calls it contains are accounted separately.
    pub(crate) fn merge(mut self, ids: &[&'static str]) {
        let runs: u64 = self.run_ns.iter().sum();
        let walk = Phase::Walk as usize;
        self.phases[walk] = self.phases[walk].saturating_sub(runs);
        self.phases[Phase::Run as usize] += runs;
        let Ok(mut t) = TOTALS.lock() else { return };
        if t.rules.len() < ids.len() {
            let have = t.rules.len();
            t.rules.extend(
                ids.iter()
                    .skip(have)
                    .map(|id| (*id, 0u64, 0u64, 0u64, 0u64)),
            );
        }
        for (i, slot) in t.rules.iter_mut().enumerate() {
            slot.1 += self.run_ns.get(i).copied().unwrap_or(0);
            slot.2 += self.once_ns.get(i).copied().unwrap_or(0);
            slot.3 += self.calls.get(i).copied().unwrap_or(0);
            slot.4 += self.gate_ns.get(i).copied().unwrap_or(0);
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
        .map(|(rule, r, o, n, g)| RuleTiming {
            rule,
            run: ticks_to_duration(*r),
            run_once: ticks_to_duration(*o),
            calls: *n,
            gate: ticks_to_duration(*g),
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
        .map(|(p, ticks)| (*p, ticks_to_duration(ticks)))
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
    let overhead = clock_overhead_ns();
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
        "  rules total {:9.1} ms, net of clock overhead ({:.1} ns per run) {:9.1} ms; top {top} by net:",
        ms(total),
        overhead,
        ms(net)
    );
    let _ = writeln!(
        s,
        "  {:<44} {:>9} {:>9} {:>9} {:>9} {:>10}",
        "rule", "net ms", "run ms", "once ms", "gate ms", "calls"
    );
    for r in rules.iter().take(top) {
        let _ = writeln!(
            s,
            "  {:<44} {:9.1} {:9.1} {:9.1} {:9.1} {:>10}",
            r.rule,
            ms(r.net(overhead)),
            ms(r.run),
            ms(r.run_once),
            ms(r.gate),
            r.calls
        );
    }
    s
}

/// Average time (ns) an empty timed interval reports: the clock's own share of every measured
/// `run` call, subtracted by [`RuleTiming::net`].
pub fn clock_overhead_ns() -> f64 {
    let mut best = u64::MAX;
    for _ in 0..5 {
        let mut sum = 0u64;
        for _ in 0..10_000 {
            let start = stamp();
            sum += stamp().wrapping_sub(start);
        }
        best = best.min(sum);
    }
    best as f64 / 10_000.0 / ticks_per_ns()
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}
