// SPDX-License-Identifier: AGPL-3.0-only
//! `cargo xtask parity`: every tape replayed against a host, its answers normalized and diffed with the golden, each
//! difference failed unless `parity/accepted.tsv` names it, and the result printed as a table per op family with
//! coverage against the host's ops. A run fails on any difference left and on any family no tape sends an op of.

use crate::accepted::Accepted;
use crate::diff::{diff, Difference};
use crate::golden;
use crate::host::{Host, Replay};
use crate::normalize::{canonical, Facts};
use crate::seed::Order;
use crate::tape::{self, Door, Tape};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub jobs: usize,
    pub seed: Option<u64>,
    /// Only the tapes whose id holds this text; coverage is then not held to every family.
    pub only: Option<String>,
    /// Only these tapes, as a unit's run names them; coverage is then not held to every family either.
    pub ids: Option<BTreeSet<String>>,
}

impl Options {
    fn narrowed(&self) -> bool {
        self.only.is_some() || self.ids.is_some()
    }
}

#[derive(Clone, Debug)]
pub enum Verdict {
    Passed,
    /// Every difference is one an accepted line names: the line numbers.
    Accepted(Vec<usize>),
    Failed(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub tape: String,
    pub door: Door,
    pub ops: BTreeSet<String>,
    pub verdict: Verdict,
}

pub struct Report {
    pub host: String,
    pub outcomes: Vec<Outcome>,
    pub ops: Vec<String>,
    pub key: Option<String>,
    pub tree: Option<String>,
    pub unused: Vec<usize>,
    pub accepted: Accepted,
    pub orphans: Vec<String>,
    pub seed: Option<u64>,
    pub only: bool,
    /// How many of the tapes run are held back from implementers; none when no seed draws the set.
    pub held: Option<usize>,
}

/// The family of an op: the words before its first dot, or the op itself.
pub fn family(op: &str) -> &str {
    op.split('.').next().unwrap_or(op)
}

pub fn check(root: &Path, host: &dyn Host, opts: &Options) -> Result<Report, String> {
    let tapes = selected(root, opts)?;
    let ops = golden::read_ops(root)?;
    let accepted = Accepted::read(root)?;
    let replays = replay_all(host, &tapes, opts);
    let mut used = BTreeSet::new();
    let mut outcomes = Vec::new();
    for (tape, replay) in tapes.iter().zip(replays) {
        let verdict = match (golden::read(root, &tape.id)?, replay) {
            (_, Err(e)) => Verdict::Failed(vec![format!("the replay failed: {e}")]),
            (None, Ok(_)) => Verdict::Failed(vec!["no golden: run cargo xtask parity record".into()]),
            (Some(want), Ok(replay)) => judge(tape, &want, &replay, &accepted, &mut used),
        };
        outcomes.push(Outcome { tape: tape.id.clone(), door: tape.door, ops: tape.ops().into_iter().collect(), verdict });
    }
    let orphans = if opts.narrowed() {
        Vec::new()
    } else {
        golden::recorded(root).into_iter().filter(|g| !tapes.iter().any(|t| t.id == *g)).collect()
    };
    let unused = if opts.narrowed() { Vec::new() } else { (0..accepted.lines.len()).filter(|i| !used.contains(i)).collect() };
    Ok(Report {
        host: host.name(),
        outcomes,
        ops,
        key: golden::read_key(root),
        tree: golden::tree_key(root).ok(),
        unused,
        accepted,
        orphans,
        seed: opts.seed,
        only: opts.narrowed(),
        held: held_among(root, &tapes),
    })
}

/// How many of these tapes the held-back set takes, drawn over every tape the repository holds.
fn held_among(root: &Path, tapes: &[Tape]) -> Option<usize> {
    let seed = crate::held::seed(root).ok()?;
    let held = crate::held::held(&tape::load(root).ok()?, seed);
    Some(tapes.iter().filter(|t| held.contains(&t.id)).count())
}

fn selected(root: &Path, opts: &Options) -> Result<Vec<Tape>, String> {
    Ok(tape::load(root)?
        .into_iter()
        .filter(|t| opts.only.as_ref().is_none_or(|text| t.id.contains(text.as_str())))
        .filter(|t| opts.ids.as_ref().is_none_or(|ids| ids.contains(&t.id)))
        .collect())
}

/// A replay's answers held to the golden, and the tape's own written answers held to it as well.
fn judge(tape: &Tape, want: &Value, replay: &Replay, accepted: &Accepted, used: &mut BTreeSet<usize>) -> Verdict {
    let live = canonical(&replay.replies, &replay.events, &replay.facts);
    let mut found: Vec<(String, Difference)> = diff(want, &live).into_iter().map(|d| (String::new(), d)).collect();
    if let Some(written) = written(tape) {
        found.extend(diff(want, &written).into_iter().map(|d| ("the tape writes".to_string(), d)));
    }
    let mut lines = Vec::new();
    let mut failures = Vec::new();
    for (whose, d) in found {
        match accepted.accepts(&tape.id, &d.field) {
            Some(i) if whose.is_empty() => {
                used.insert(i);
                lines.push(accepted.lines[i].at);
            }
            _ => {
                let theirs = if whose.is_empty() { "live" } else { whose.as_str() };
                failures.push(format!("{}: golden {}, {theirs} {}", d.field, d.golden, d.live));
            }
        }
    }
    if !failures.is_empty() {
        Verdict::Failed(failures)
    } else if !lines.is_empty() {
        lines.dedup();
        Verdict::Accepted(lines)
    } else {
        Verdict::Passed
    }
}

/// The answers a tape writes down itself, in the golden's form, or none for a tape that writes none.
fn written(tape: &Tape) -> Option<Value> {
    if tape.outs.is_empty() {
        return None;
    }
    let (replies, events) = match tape.door {
        Door::Cli => (tape.outs.clone(), Vec::new()),
        Door::Ws | Door::Mcp => {
            let answer_to =
                |f: &Value| f.get("id").filter(|id| !id.is_null()).is_some() && f.get("op").is_none() && f.get("method").is_none();
            let mut replies = Vec::new();
            let mut pool: Vec<Option<&Value>> = tape.outs.iter().filter(|f| answer_to(f)).map(Some).collect();
            for frame in &tape.ins {
                let Some(id) = frame.get("id").filter(|id| !id.is_null()) else { continue };
                if tape.door == Door::Mcp && frame.get("method").is_none() {
                    continue;
                }
                let found = pool.iter_mut().find(|o| o.is_some_and(|o| o.get("id") == Some(id))).and_then(Option::take);
                replies.push(found.cloned().unwrap_or_else(|| serde_json::json!({ "parity": "no reply", "id": id.to_string() })));
            }
            (replies, tape.outs.iter().filter(|f| !answer_to(f)).cloned().collect())
        }
    };
    Some(canonical(&replies, &events, &Facts::default()))
}

fn replay_all(host: &dyn Host, tapes: &[Tape], opts: &Options) -> Vec<Result<Replay, String>> {
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<Option<Result<Replay, String>>>> = Mutex::new(vec![None; tapes.len()]);
    std::thread::scope(|s| {
        for _ in 0..opts.jobs.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(tape) = tapes.get(i) else { break };
                let mut order = Order::for_tape(opts.seed, &tape.id);
                let result = host.replay(tape, &mut order);
                results.lock().expect("results")[i] = Some(result);
            });
        }
    });
    results.into_inner().expect("results").into_iter().map(|r| r.expect("every tape replayed")).collect()
}

impl Report {
    pub fn failed(&self) -> usize {
        self.outcomes.iter().filter(|o| matches!(o.verdict, Verdict::Failed(_))).count()
    }

    /// The host's op families no tape sends an op of.
    pub fn empty_families(&self) -> Vec<String> {
        if self.only {
            return Vec::new();
        }
        let sent: BTreeSet<&str> = self.outcomes.iter().flat_map(|o| o.ops.iter().map(|op| family(op))).collect();
        let all: BTreeSet<&str> = self.ops.iter().map(|op| family(op)).collect();
        all.into_iter().filter(|f| !sent.contains(f)).map(str::to_string).collect()
    }

    pub fn ok(&self) -> bool {
        self.failed() == 0 && self.empty_families().is_empty() && !self.outcomes.is_empty()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        let stale = match (&self.key, &self.tree) {
            (Some(k), Some(t)) if k != t => format!(" (the TS tree is now {}: run cargo xtask parity record to move them)", short(t)),
            _ => String::new(),
        };
        let _ = writeln!(out, "parity: {} tapes against {}", self.outcomes.len(), self.host);
        let _ = writeln!(out, "goldens: TS tree {}{stale}", self.key.as_deref().map(short).unwrap_or("(none recorded)"));
        if let Some(seed) = self.seed {
            let _ = writeln!(out, "order: seeded, seed {seed}");
        }
        match self.held {
            Some(n) => {
                let _ = writeln!(out, "held back: {n} of these tapes, which cargo xtask unit never runs");
            }
            None => {
                let _ = writeln!(out, "held back: none ({} is missing)", crate::held::SEED_FILE);
            }
        }
        #[derive(Default)]
        struct Row {
            tapes: usize,
            passed: usize,
            accepted: usize,
            failed: usize,
            covered: BTreeSet<String>,
            ops: usize,
        }
        let mut rows: BTreeMap<String, Row> = BTreeMap::new();
        for op in &self.ops {
            rows.entry(family(op).to_string()).or_default().ops += 1;
        }
        let known: BTreeSet<&String> = self.ops.iter().collect();
        for o in &self.outcomes {
            let families: BTreeSet<String> = match o.door {
                Door::Ws => o.ops.iter().filter(|op| known.contains(op)).map(|op| family(op).to_string()).collect(),
                door => [format!("({} door)", door.word())].into(),
            };
            for f in families {
                let row = rows.entry(f.clone()).or_default();
                row.tapes += 1;
                match o.verdict {
                    Verdict::Passed => row.passed += 1,
                    Verdict::Accepted(_) => row.accepted += 1,
                    Verdict::Failed(_) => row.failed += 1,
                }
                row.covered.extend(o.ops.iter().filter(|op| known.contains(op) && family(op) == f).cloned());
            }
        }
        let _ = writeln!(out, "\n{:<18} {:>6} {:>7} {:>9} {:>7} {:>9}", "family", "tapes", "passed", "accepted", "failed", "ops");
        for (name, r) in &rows {
            let ops = if r.ops == 0 { "-".to_string() } else { format!("{}/{}", r.covered.len(), r.ops) };
            let _ = writeln!(out, "{name:<18} {:>6} {:>7} {:>9} {:>7} {ops:>9}", r.tapes, r.passed, r.accepted, r.failed);
        }
        let covered: BTreeSet<&String> = self.outcomes.iter().flat_map(|o| o.ops.iter()).filter(|op| known.contains(op)).collect();
        let missing: Vec<&String> = self.ops.iter().filter(|op| !covered.contains(op)).collect();
        let _ = writeln!(out, "\ncoverage: {} of {} host ops have a tape", covered.len(), self.ops.len());
        if !missing.is_empty() {
            let _ = writeln!(out, "no tape yet: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
        for o in &self.outcomes {
            if let Verdict::Failed(lines) = &o.verdict {
                for line in lines.iter().take(20) {
                    let _ = writeln!(out, "FAIL {} {line}", o.tape);
                }
                if lines.len() > 20 {
                    let _ = writeln!(out, "FAIL {} and {} more differences", o.tape, lines.len() - 20);
                }
            }
        }
        for i in &self.unused {
            let l = &self.accepted.lines[*i];
            let _ = writeln!(
                out,
                "note: {}:{} accepts a difference no tape has any more; take the line out",
                crate::accepted::ACCEPTED_FILE,
                l.at
            );
        }
        for g in &self.orphans {
            let _ = writeln!(out, "note: golden {g} answers no tape; cargo xtask parity record takes it out");
        }
        let empty = self.empty_families();
        if !empty.is_empty() {
            let _ = writeln!(out, "FAIL families no tape sends an op of: {}", empty.join(", "));
        }
        if self.outcomes.is_empty() {
            let _ = writeln!(out, "FAIL no tape ran");
        }
        let accepted = self.outcomes.iter().filter(|o| matches!(o.verdict, Verdict::Accepted(_))).count();
        let _ = writeln!(
            out,
            "\nresult: {} passed, {accepted} accepted, {} failed, {} families with no tape",
            self.outcomes.len() - accepted - self.failed(),
            self.failed(),
            empty.len()
        );
        if let (Some(seed), false) = (self.seed, self.failed() == 0) {
            let _ = writeln!(out, "seed {seed}: run again with --seed {seed}");
        }
        out
    }
}

fn short(key: &str) -> &str {
    &key[..key.len().min(12)]
}

pub struct Recorded {
    pub written: usize,
    pub removed: usize,
    pub failed: Vec<(String, String)>,
    pub key: String,
}

/// Every tape replayed against the TS host and its answers written as the golden, the TS tree's hash beside them;
/// goldens of tapes that are gone are taken out.
pub fn record(root: &Path, host: &dyn Host, opts: &Options, ops: &[String]) -> Result<Recorded, String> {
    let tapes = selected(root, opts)?;
    let replays = replay_all(host, &tapes, opts);
    let mut written = 0;
    let mut failed = Vec::new();
    for (tape, replay) in tapes.iter().zip(replays) {
        match replay {
            Ok(r) => {
                golden::write(root, &tape.id, &canonical(&r.replies, &r.events, &r.facts))?;
                written += 1;
            }
            Err(e) => failed.push((tape.id.clone(), e)),
        }
    }
    let mut removed = 0;
    if !opts.narrowed() {
        for g in golden::recorded(root) {
            if !tapes.iter().any(|t| t.id == g) {
                std::fs::remove_file(golden::path_of(root, &g)).map_err(|e| e.to_string())?;
                removed += 1;
            }
        }
    }
    let key = golden::tree_key(root)?;
    std::fs::write(root.join(golden::KEY_FILE), format!("{key}\n")).map_err(|e| e.to_string())?;
    golden::write_ops(root, ops)?;
    Ok(Recorded { written, removed, failed, key })
}
