// SPDX-License-Identifier: AGPL-3.0-only
//! `cargo xtask parity`: every tape replayed against a host, its answers normalized and diffed with the golden, each
//! difference failed unless `parity/accepted.tsv` names it, and the result printed as a table per op family, then one
//! row per host op counting its success answers apart from its refusals and closes. A run fails on any difference
//! left, on any family no tape sends an op of, and on a door, or a tool on either door, with no tape.

use crate::accepted::Accepted;
use crate::computer;
use crate::diff::{diff, Difference};
use crate::golden;
use crate::host::{Host, Replay};
use crate::normalize::{canonical, Facts};
use crate::seed::Order;
use crate::tape::{self, Door, Tape};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
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
    /// The held-back folder, whose tapes run beside the repository's and whose goldens are kept there.
    pub held: Option<PathBuf>,
    /// Record rewrites the goldens of tapes recorded on another computer and marks them as recorded on this one.
    pub move_here: bool,
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
    /// Recorded on the named computer, and its answers read the computer: not run on this one.
    Skipped(String),
}

/// How a host met one request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Answer {
    /// It did what was asked: `ok: true`, a tool's result that is no error, a command line's exit 0.
    Success,
    /// It answered no: `ok: false`, an error or a tool result marked one, a command line's other exit.
    Refused,
    /// No answer came, as for a request sent after the host closed the socket.
    Closed,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub tape: String,
    pub door: Door,
    pub ops: BTreeSet<String>,
    /// Each request's op (websocket), tool (tool server) or command line words, and how the replay met it.
    pub answers: Vec<(String, Answer)>,
    pub verdict: Verdict,
}

pub struct Report {
    pub host: String,
    pub outcomes: Vec<Outcome>,
    pub ops: Vec<String>,
    pub tools: Vec<String>,
    pub key: Option<String>,
    pub held_key: Option<String>,
    pub tree: Option<String>,
    pub unused: Vec<usize>,
    pub accepted: Accepted,
    pub orphans: Vec<String>,
    pub seed: Option<u64>,
    pub only: bool,
    /// How many of the tapes run came from the held-back folder; none when no folder was given.
    pub held: Option<usize>,
    pub computer: String,
}

/// The family of an op: the words before its first dot, or the op itself.
pub fn family(op: &str) -> &str {
    op.split('.').next().unwrap_or(op)
}

/// The goldens folder a tape's golden is kept in.
fn goldens_of(root: &Path, opts: &Options, tape: &Tape) -> PathBuf {
    match (&opts.held, tape.held) {
        (Some(dir), true) => dir.join("goldens"),
        _ => golden::dir(root),
    }
}

/// Whether a tape was recorded on another computer and its answers read the computer, so it is not run here.
fn elsewhere(tape: &Tape, here: &str) -> Option<String> {
    tape.computer.as_ref().filter(|c| c.as_str() != here).cloned()
}

pub fn check(root: &Path, host: &dyn Host, opts: &Options) -> Result<Report, String> {
    let tapes = selected(root, opts)?;
    let ops = golden::read_ops(root)?;
    let tools = golden::read_tools(root)?;
    let accepted = Accepted::read(root)?;
    let here = computer::name();
    let runnable: Vec<Tape> = tapes.iter().filter(|t| elsewhere(t, &here).is_none()).cloned().collect();
    let mut replays = replay_all(host, &runnable, opts).into_iter();
    let mut used = BTreeSet::new();
    let mut outcomes = Vec::new();
    for tape in &tapes {
        let ops = tape.ops().into_iter().collect();
        if let Some(other) = elsewhere(tape, &here) {
            outcomes.push(Outcome { tape: tape.id.clone(), door: tape.door, ops, answers: Vec::new(), verdict: Verdict::Skipped(other) });
            continue;
        }
        let replay = replays.next().expect("a replay for every tape run");
        let answers = replay.as_ref().map(|r| answers(tape, &r.replies, &tools)).unwrap_or_default();
        let verdict = match (golden::read(&goldens_of(root, opts, tape), &tape.id)?, replay) {
            (_, Err(e)) => Verdict::Failed(vec![format!("the replay failed: {e}")]),
            (None, Ok(_)) => Verdict::Failed(vec!["no golden: run cargo xtask parity record".into()]),
            (Some(want), Ok(replay)) => judge(tape, &want, &replay, &accepted, &mut used),
        };
        outcomes.push(Outcome { tape: tape.id.clone(), door: tape.door, ops, answers, verdict });
    }
    let orphans = if opts.narrowed() {
        Vec::new()
    } else {
        let mut dirs = vec![(golden::dir(root), false)];
        if let Some(dir) = &opts.held {
            dirs.push((dir.join("goldens"), true));
        }
        let tapes = &tapes;
        dirs.iter()
            .flat_map(|(dir, held)| golden::recorded(dir).into_iter().filter(move |g| !tapes.iter().any(|t| t.id == *g && t.held == *held)))
            .collect()
    };
    let unused = if opts.narrowed() { Vec::new() } else { (0..accepted.lines.len()).filter(|i| !used.contains(i)).collect() };
    Ok(Report {
        host: host.name(),
        outcomes,
        ops,
        tools,
        key: golden::read_key(&golden::dir(root)),
        held_key: opts.held.as_ref().and_then(|d| golden::read_key(&d.join("goldens"))),
        tree: golden::tree_key(root).ok(),
        unused,
        accepted,
        orphans,
        seed: opts.seed,
        only: opts.narrowed(),
        held: opts.held.as_ref().map(|_| tapes.iter().filter(|t| t.held).count()),
        computer: here,
    })
}

/// The words of a command line that name its verb: the longest run of its first words, flags left out, that a tool
/// names (`thread_read` is `wsp thread read`), or its first word when none does.
pub fn verb_of(argv: &[Value], tools: &[String]) -> String {
    let words: Vec<&str> = argv.iter().filter_map(Value::as_str).filter(|w| !w.starts_with('-')).collect();
    (1..=words.len().min(3))
        .rev()
        .map(|n| words[..n].join(" "))
        .find(|v| tools.iter().any(|t| t.replace('_', " ") == *v))
        .unwrap_or_else(|| words.first().map(|w| w.to_string()).unwrap_or_default())
}

/// Each request of a tape and how the replay met it, read off the replies in the order the requests went.
fn answers(tape: &Tape, replies: &[Value], tools: &[String]) -> Vec<(String, Answer)> {
    let closed = |r: &Value| r.get("parity").is_some();
    match tape.door {
        Door::Ws => tape
            .ins
            .iter()
            .filter(|f| f.get("id").is_some_and(|id| !id.is_null()))
            .zip(replies)
            .filter_map(|(f, r)| {
                let op = f.get("op").and_then(Value::as_str)?.to_string();
                let answer = if closed(r) {
                    Answer::Closed
                } else if r.get("ok") == Some(&Value::Bool(false)) || r.get("error").is_some() {
                    Answer::Refused
                } else {
                    Answer::Success
                };
                Some((op, answer))
            })
            .collect(),
        Door::Mcp => tape
            .ins
            .iter()
            .filter(|f| f.get("method").is_some() && f.get("id").is_some_and(|id| !id.is_null()))
            .zip(replies)
            .filter(|(f, _)| f.get("method").and_then(Value::as_str) == Some("tools/call"))
            .filter_map(|(f, r)| {
                let tool = f.pointer("/params/name").and_then(Value::as_str)?.to_string();
                let answer = if closed(r) {
                    Answer::Closed
                } else if r.get("error").is_some() || r.pointer("/result/isError") == Some(&Value::Bool(true)) {
                    Answer::Refused
                } else {
                    Answer::Success
                };
                Some((tool, answer))
            })
            .collect(),
        Door::Cli => tape
            .ins
            .iter()
            .zip(replies)
            .map(|(f, r)| {
                let verb = verb_of(f.get("argv").and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default(), tools);
                let answer = match r.get("exit").and_then(Value::as_i64) {
                    _ if closed(r) => Answer::Closed,
                    Some(0) => Answer::Success,
                    _ => Answer::Refused,
                };
                (verb, answer)
            })
            .collect(),
    }
}

fn selected(root: &Path, opts: &Options) -> Result<Vec<Tape>, String> {
    let mut tapes = tape::load(root)?;
    if let Some(dir) = &opts.held {
        let held = tape::load_held(dir)?;
        if let Some(both) = held.iter().find(|h| tapes.iter().any(|t| t.id == h.id)) {
            return Err(format!("{} is both in the repository and in {}", both.id, dir.display()));
        }
        tapes.extend(held);
        tapes.sort_by(|a, b| a.id.cmp(&b.id));
    }
    Ok(tapes
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
    if tape.outs.is_empty() || tape.recorded {
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

/// How many requests of one op, tool or verb met each answer.
#[derive(Clone, Copy, Default)]
struct Tally {
    tapes: usize,
    success: usize,
    refused: usize,
    closed: usize,
}

impl Report {
    pub fn failed(&self) -> usize {
        self.outcomes.iter().filter(|o| matches!(o.verdict, Verdict::Failed(_))).count()
    }

    pub fn skipped(&self) -> usize {
        self.outcomes.iter().filter(|o| matches!(o.verdict, Verdict::Skipped(_))).count()
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

    /// The command line and the tool server with no tape at all, then each tool with no tape on the tool server or
    /// no tape of its verb on the command line: the three are one contract, so each holds a tape of every tool.
    pub fn empty_doors(&self) -> Vec<String> {
        if self.only {
            return Vec::new();
        }
        let mut out = Vec::new();
        for door in [Door::Cli, Door::Mcp] {
            if !self.outcomes.iter().any(|o| o.door == door) {
                out.push(format!("the {} door has no tape", door.word()));
            }
        }
        let reached = |door: Door| -> BTreeSet<String> {
            let mut seen = BTreeSet::new();
            for o in self.outcomes.iter().filter(|o| o.door == door) {
                seen.extend(o.answers.iter().map(|(name, _)| name.clone()));
            }
            seen
        };
        let (mcp, cli) = (reached(Door::Mcp), reached(Door::Cli));
        let no_tool: Vec<&str> = self.tools.iter().filter(|t| !mcp.contains(*t)).map(String::as_str).collect();
        let no_verb: Vec<String> = self.tools.iter().map(|t| t.replace('_', " ")).filter(|v| !cli.contains(v)).collect();
        if !no_tool.is_empty() {
            out.push(format!("no tool server tape calls: {}", no_tool.join(", ")));
        }
        if !no_verb.is_empty() {
            out.push(format!("no command line tape runs: {}", no_verb.join(", ")));
        }
        out
    }

    pub fn ok(&self) -> bool {
        self.failed() == 0 && self.empty_families().is_empty() && self.empty_doors().is_empty() && !self.outcomes.is_empty()
    }

    fn tallies(&self, door: Door) -> BTreeMap<String, Tally> {
        let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
        for o in self.outcomes.iter().filter(|o| o.door == door) {
            let mut counted = BTreeSet::new();
            for (name, answer) in &o.answers {
                let t = tallies.entry(name.clone()).or_default();
                if counted.insert(name.clone()) {
                    t.tapes += 1;
                }
                match answer {
                    Answer::Success => t.success += 1,
                    Answer::Refused => t.refused += 1,
                    Answer::Closed => t.closed += 1,
                }
            }
            // A skipped tape still holds its ops; it has no answers to count.
            if matches!(o.verdict, Verdict::Skipped(_)) {
                for op in &o.ops {
                    tallies.entry(op.clone()).or_default().tapes += 1;
                }
            }
        }
        tallies
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        let stale = |key: &Option<String>| match (key, &self.tree) {
            (Some(k), Some(t)) if k != t => format!(" (the TS tree is now {}: run cargo xtask parity record to move them)", short(t)),
            _ => String::new(),
        };
        let _ = writeln!(out, "parity: {} tapes against {}", self.outcomes.len(), self.host);
        let _ = writeln!(out, "goldens: TS tree {}{}", self.key.as_deref().map(short).unwrap_or("(none recorded)"), stale(&self.key));
        if let Some(seed) = self.seed {
            let _ = writeln!(out, "order: seeded, seed {seed}");
        }
        match self.held {
            Some(n) => {
                let _ = writeln!(
                    out,
                    "held back: {n} of these tapes, their goldens at TS tree {}{}",
                    self.held_key.as_deref().map(short).unwrap_or("(none recorded)"),
                    stale(&self.held_key)
                );
            }
            None => {
                let _ = writeln!(out, "held back: none run (the reviewer passes --held <dir>)");
            }
        }
        #[derive(Default)]
        struct Row {
            tapes: usize,
            passed: usize,
            accepted: usize,
            failed: usize,
            skipped: usize,
            answered: BTreeSet<String>,
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
                    Verdict::Skipped(_) => row.skipped += 1,
                }
                row.answered.extend(
                    o.answers
                        .iter()
                        .filter(|(op, a)| *a == Answer::Success && known.contains(op) && family(op) == f)
                        .map(|(op, _)| op.clone()),
                );
            }
        }
        let _ = writeln!(
            out,
            "\n{:<18} {:>6} {:>7} {:>9} {:>7} {:>8} {:>9}",
            "family", "tapes", "passed", "accepted", "failed", "skipped", "answered"
        );
        for (name, r) in &rows {
            let ops = if r.ops == 0 { "-".to_string() } else { format!("{}/{}", r.answered.len(), r.ops) };
            let _ = writeln!(out, "{name:<18} {:>6} {:>7} {:>9} {:>7} {:>8} {ops:>9}", r.tapes, r.passed, r.accepted, r.failed, r.skipped);
        }
        let ws = self.tallies(Door::Ws);
        let _ = writeln!(out, "\n{:<34} {:>6} {:>8} {:>8} {:>7}", "op", "tapes", "success", "refused", "closed");
        for op in &self.ops {
            let t = ws.get(op).copied().unwrap_or_default();
            let _ = writeln!(out, "{op:<34} {:>6} {:>8} {:>8} {:>7}", t.tapes, t.success, t.refused, t.closed);
        }
        let covered: BTreeSet<&String> = self.outcomes.iter().flat_map(|o| o.ops.iter()).filter(|op| known.contains(op)).collect();
        let missing: Vec<&String> = self.ops.iter().filter(|op| !covered.contains(op)).collect();
        let answered: BTreeSet<&String> = self.ops.iter().filter(|op| ws.get(*op).is_some_and(|t| t.success > 0)).collect();
        let _ = writeln!(
            out,
            "\ncoverage: {} of {} host ops have a tape, {} of them an answer on one",
            covered.len(),
            self.ops.len(),
            answered.len()
        );
        let unanswered: Vec<&str> =
            self.ops.iter().filter(|op| covered.contains(op) && !answered.contains(op)).map(String::as_str).collect();
        if !unanswered.is_empty() {
            let _ = writeln!(out, "sent but only ever refused or closed: {}", unanswered.join(", "));
        }
        if !missing.is_empty() {
            let _ = writeln!(out, "no tape yet: {}", missing.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "));
        }
        if !self.tools.is_empty() {
            let (mcp, cli) = (self.tallies(Door::Mcp), self.tallies(Door::Cli));
            let succeeded = |tallies: &BTreeMap<String, Tally>, name: &str| tallies.get(name).is_some_and(|t| t.success > 0);
            let _ = writeln!(
                out,
                "tools: {} of {} answered on the tool server, {} of their verbs on the command line",
                self.tools.iter().filter(|t| succeeded(&mcp, t)).count(),
                self.tools.len(),
                self.tools.iter().filter(|t| succeeded(&cli, &t.replace('_', " "))).count()
            );
        }
        let mut skipped: BTreeMap<&str, usize> = BTreeMap::new();
        for o in &self.outcomes {
            if let Verdict::Skipped(c) = &o.verdict {
                *skipped.entry(c.as_str()).or_default() += 1;
            }
        }
        for (c, n) in &skipped {
            let _ = writeln!(
                out,
                "skipped: {n} tapes recorded on {c}, whose answers read the computer the host runs on; this is {}",
                self.computer
            );
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
        for door in self.empty_doors() {
            let _ = writeln!(out, "FAIL {door}");
        }
        if self.outcomes.is_empty() {
            let _ = writeln!(out, "FAIL no tape ran");
        }
        let accepted = self.outcomes.iter().filter(|o| matches!(o.verdict, Verdict::Accepted(_))).count();
        let _ = writeln!(
            out,
            "\nresult: {} passed, {accepted} accepted, {} failed, {} skipped, {} families with no tape",
            self.outcomes.len() - accepted - self.failed() - self.skipped(),
            self.failed(),
            self.skipped(),
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
    /// Tapes recorded on another computer whose goldens were left as they are: the tape and that computer.
    pub kept: Vec<(String, String)>,
    /// Tapes whose answers read this computer, marked in their meta as recorded on it.
    pub marked: Vec<String>,
    /// A narrowed record leaves `KEY` where it was, so the goldens it did not touch still read as from that tree.
    pub key_kept: Option<String>,
}

/// Every tape replayed against the TS host and its answers written as the golden, the TS tree's hash beside them;
/// goldens of tapes that are gone are taken out. A tape recorded on another computer whose answers read it is left
/// alone unless `move_here`; a tape whose answers read this computer is marked as recorded on it.
pub fn record(root: &Path, host: &dyn Host, opts: &Options, ops: &[String]) -> Result<Recorded, String> {
    let mut dirs = vec![golden::dir(root)];
    if let Some(dir) = &opts.held {
        dirs.push(dir.join("goldens"));
    }
    // Asked before the replays rather than after them: a run of every tape takes minutes.
    for dir in &dirs {
        let probe = dir.join(".writable");
        std::fs::create_dir_all(dir)
            .and_then(|_| std::fs::write(&probe, b""))
            .and_then(|_| std::fs::remove_file(&probe))
            .map_err(|e| format!("{} cannot be written: {e}", dir.display()))?;
    }
    let here = computer::name();
    let tapes = selected(root, opts)?;
    let kept: Vec<(String, String)> =
        tapes.iter().filter_map(|t| elsewhere(t, &here).filter(|_| !opts.move_here).map(|c| (t.id.clone(), c))).collect();
    let runnable: Vec<Tape> = tapes.iter().filter(|t| !kept.iter().any(|(id, _)| *id == t.id)).cloned().collect();
    let replays = replay_all(host, &runnable, opts);
    let mut written = 0;
    let mut failed = Vec::new();
    let mut marked = Vec::new();
    for (tape, replay) in runnable.iter().zip(replays) {
        match replay {
            Ok(r) => {
                let golden = canonical(&r.replies, &r.events, &r.facts);
                golden::write(&goldens_of(root, opts, tape), &tape.id, &golden)?;
                written += 1;
                let reads = computer::reads(tape, &golden);
                let mark = reads.then_some(here.as_str());
                if mark != tape.computer.as_deref() {
                    if let Some(file) = tape_file(root, opts, tape) {
                        computer::mark(&file, mark)?;
                    }
                }
                if reads {
                    marked.push(tape.id.clone());
                }
            }
            Err(e) => failed.push((tape.id.clone(), e)),
        }
    }
    let mut removed = 0;
    if !opts.narrowed() {
        for (dir, held) in dirs.iter().zip([false, true]) {
            for g in golden::recorded(dir) {
                if !tapes.iter().any(|t| t.id == g && t.held == held) {
                    std::fs::remove_file(golden::path_of(dir, &g)).map_err(|e| e.to_string())?;
                    removed += 1;
                }
            }
        }
    }
    let key = golden::tree_key(root)?;
    let mut key_kept = None;
    for dir in &dirs {
        match golden::read_key(dir) {
            Some(old) if opts.narrowed() && old != key => key_kept = Some(old),
            _ => golden::write_key(dir, &key)?,
        }
    }
    golden::write_ops(root, ops)?;
    Ok(Recorded { written, removed, failed, key, kept, marked, key_kept })
}

/// The file a recorded tape was read from; none for a spec example, which its markdown file holds.
fn tape_file(root: &Path, opts: &Options, tape: &Tape) -> Option<PathBuf> {
    let file = match (&opts.held, tape.held) {
        (Some(dir), true) => dir.join("tapes"),
        _ => root.join(tape::TAPES_DIR),
    }
    .join(format!("{}.tape", tape.id));
    file.is_file().then_some(file)
}
