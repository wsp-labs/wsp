// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_table: slate-runs.ts:140-163, 210-242, 613-983 at 65f6b89c7. Spec: spec/runs_table.md.
//!
//! State is `RunsInner` behind one `Rc`, each field in a `RefCell`; no borrow is held across a call to `deps`, a
//! road, a kill or a release (rule 3.3): `on_record` re-enters the slate area, which may call `start` again.
use std::cell::{Cell, RefCell};
use std::future::ready;
use std::path::Path;
use std::rc::{Rc, Weak};

use indexmap::IndexMap;
use serde_json::{Map, Value};
use tokio::task::spawn_local;
use wsp_core::clock::{Cancel, Clock};
use wsp_wire::types::env::THREAD_ENV;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::types::run_block::RUN_OUTPUT_MAX_CHARS;
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;
use wsp_words::js::{js_trim, number_string};
use wsp_words::run_block::run_output_tail;

use super::parts::{
    as_text, files_unwritten, is_env_name, last_result, run_key, running_record, spent, timeout_of, CmdRunDecl, Outcome, RunAsk, RunBy,
    RunInput, RunInputs, RunRecord, RunResult, RunStart, RunStartAnswer, RunState, BACKED_OFF, BACKOFF_AFTER, BACKOFF_MS,
    DEFAULT_TIMEOUT_S, HELD_APPROVAL, HELD_ASLEEP, HELD_BUDGET, HELD_BUSY, HELD_PRESSED, LINES_KEPT, RUNNING_MAX, SECRET_DOTS,
    TIMER_FLOOR_S,
};
use super::parts::{Reshape, ReshapeAnswer};
use super::road::{local_road, reshape_result, ReshapeStart, RoadEnd, RoadEvents, RoadStart, RunRoad, Stream};
use super::secrets::{create_secrets, DefaultApprovals, RunApprovals, Secrets, SlateSecrets};
use crate::pending::slate::SLATE_SECRET_IN_ARGS;

pub trait SlateRunsDeps {
    /// The person's login environment on this computer.
    fn env(&self) -> IndexMap<String, String>;
    /// Every change of a run's record, scrubbed.
    fn on_record(&self, thread_id: &str, run: &str, record: RunRecord);
    /// Each line a `stream` run prints, scrubbed.
    fn on_line(&self, _thread_id: &str, _run: &str, _line: &str, _stream: Stream) {}
    /// A timer wants its run started.
    fn on_timer(&self, _thread_id: &str, _run: &str) {}
    /// None keeps approvals in memory (`DefaultApprovals`).
    fn approvals(&self) -> Option<Rc<dyn RunApprovals>> {
        None
    }
    /// `slates.secrets.json`; none keeps nothing on disk.
    fn secrets_file(&self) -> Option<std::path::PathBuf> {
        None
    }
    /// The slate's own folder with its files written: none when the host gives no folder; `Err` with the message
    /// when the files could not be written.
    fn dir(&self, _thread_id: &str) -> Option<Result<String, String>> {
        None
    }
    /// Where a run of this thread runs when that is not this computer.
    fn road(&self, _thread_id: &str, _on: &str) -> Option<Rc<dyn RunRoad>> {
        None
    }
    /// Names a command's process as the thread's slate run until the answer is called.
    fn own(&self, _thread_id: &str, _pid: u32, _kind: &str) -> Option<Rc<dyn Fn()>> {
        None
    }
    /// Why a run `on: host` of this thread does not start.
    fn host_refused(&self, _thread_id: &str, _run: &str) -> Option<String> {
        None
    }
    fn clock(&self) -> Rc<dyn Clock>;
}

/// One declared timer.
#[derive(Clone, Debug, PartialEq)]
pub struct TimerDecl {
    pub run: String,
    /// Seconds.
    pub every: f64,
    pub key: String,
    pub always: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApproveScope {
    Once,
    Always,
}

pub trait SlateRuns {
    fn key(&self, decl: &CmdRunDecl) -> String;
    fn start(&self, req: RunStart) -> RunStartAnswer;
    fn approve(&self, thread_id: &str, run: &str, scope: ApproveScope) -> Option<RunStartAnswer>;
    fn deny(&self, thread_id: &str, run: &str);
    fn revoke(&self, thread_id: &str, key: &str);
    fn allowed(&self, thread_id: &str) -> Vec<String>;
    fn held(&self, thread_id: &str) -> Vec<RunAsk>;
    fn cancel(&self, thread_id: &str, run: &str);
    /// `quiet` drops the completions and writes nothing; else each records `why` (or `cancelled`).
    fn stop_all(&self, thread_id: &str, quiet: bool, why: Option<&str>);
    fn timers(&self, thread_id: &str, list: Vec<TimerDecl>);
    fn shown(&self, thread_id: &str, shown: bool);
    fn release(&self, thread_id: &str);
    /// `timeout_s` none is 60.
    fn reshape(&self, thread_id: &str, cmd: &str, input: &str, folder: &str, timeout_s: Option<f64>) -> Reshape;
    fn secrets(&self) -> Rc<dyn SlateSecrets>;
    /// The TS `drop`.
    fn drop_thread(&self, thread_id: &str, keep: bool);
    fn close(&self);
}

/// Why a start is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HeldFor {
    Approval,
    Busy,
    Budget,
    Pressed,
}

/// One run's live entry.
pub(crate) struct Live {
    pub record: RunRecord,
    pub gen: u64,
    pub result: Option<RunResult>,
    pub kill: Option<Rc<dyn Fn()>>,
    pub reshaping: Option<Rc<dyn Fn()>>,
    pub held: Option<(RunStart, HeldFor)>,
    pub starts: Vec<i64>,
    pub pressed: Option<Vec<i64>>,
}

impl Live {
    fn new(last: Option<&RunRecord>) -> Live {
        let mut record = Map::new();
        record.insert("state".into(), "idle".into());
        record.insert("runs".into(), last.map_or(0, RunRecord::runs).into());
        Live {
            record: RunRecord(record),
            gen: 0,
            result: last.and_then(last_result),
            kill: None,
            reshaping: None,
            held: None,
            starts: Vec::new(),
            pressed: None,
        }
    }
}

pub(crate) type LiveRef = Rc<RefCell<Live>>;

pub(crate) struct Timer {
    pub every: f64,
    pub key: String,
    pub always: bool,
    pub handle: Option<Cancel>,
    pub failures: i64,
    pub failed_at: Option<i64>,
}

#[derive(Default)]
pub(crate) struct ThreadRuns {
    pub runs: IndexMap<String, LiveRef>,
    pub queue: Vec<String>,
    pub timers: IndexMap<String, Timer>,
    pub shown: bool,
}

/// The table's state.
pub(crate) struct RunsInner {
    pub deps: Rc<dyn SlateRunsDeps>,
    pub clock: Rc<dyn Clock>,
    pub approvals: Rc<dyn RunApprovals>,
    pub secrets: Rc<Secrets>,
    pub local: Rc<dyn RunRoad>,
    pub threads: RefCell<IndexMap<String, ThreadRuns>>,
}

/// `createSlateRuns`' answer.
pub struct SlateRunsTable {
    pub(crate) inner: Rc<RunsInner>,
}

pub fn create_slate_runs(deps: Rc<dyn SlateRunsDeps>) -> Rc<dyn SlateRuns> {
    let clock = deps.clock();
    let approvals = deps.approvals().unwrap_or_else(|| Rc::new(DefaultApprovals::default()));
    let secrets = create_secrets(deps.secrets_file(), clock.clone());
    let local = local_road(clock.clone());
    Rc::new(SlateRunsTable { inner: Rc::new(RunsInner { deps, clock, approvals, secrets, local, threads: RefCell::new(IndexMap::new()) }) })
}

struct Captured {
    out: String,
    err: String,
    exit: Option<i64>,
    cut: bool,
}

struct CtxInner {
    inner: Weak<RunsInner>,
    thread: String,
    run: String,
    gen: u64,
    l: LiveRef,
    stream: bool,
    then: Option<String>,
    folder: String,
    timeout: f64,
    started_at: i64,
    runs: i64,
    lines: RefCell<Vec<String>>,
    over: Cell<bool>,
    release: RefCell<Option<Rc<dyn Fn()>>>,
    road: Option<Rc<dyn RunRoad>>,
    road_env: Option<IndexMap<String, String>>,
}

impl RoadEvents for Rc<CtxInner> {
    fn line(&self, stream: Stream, text: &str) {
        let Some(inner) = self.inner.upgrade() else { return };
        let clean = inner.secrets.scrub(&self.thread, text);
        {
            let mut lines = self.lines.borrow_mut();
            lines.push(clean.clone());
            if lines.len() > LINES_KEPT {
                lines.remove(0);
            }
        }
        let current = self.l.borrow().gen == self.gen;
        if current {
            inner.deps.on_line(&self.thread, &self.run, &clean, stream);
        }
    }
    fn end(&self, o: RoadEnd) {
        self.over.set(true);
        let release = self.release.borrow_mut().take();
        if let Some(release) = release {
            release();
        }
        command_ended(self, o);
    }
}

fn outcome_fields(state: RunState, why: Option<String>, exit: Option<i64>, err: String, json: Option<Value>) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("state".into(), state.as_str().into());
    if let Some(why) = why {
        m.insert("why".into(), why.into());
    }
    m.insert("exit".into(), exit.map_or(Value::Null, Value::from));
    m.insert("err".into(), err.into());
    if let Some(json) = json {
        m.insert("json".into(), json);
    }
    m
}

fn command_ended(ctx: &Rc<CtxInner>, o: RoadEnd) {
    let Some(inner) = ctx.inner.upgrade() else { return };
    if ctx.l.borrow().gen != ctx.gen {
        return;
    }
    ctx.l.borrow_mut().kill = None;
    let raw = inner.secrets.scrub(&ctx.thread, &o.out);
    let out = run_output_tail(&raw, RUN_OUTPUT_MAX_CHARS as usize);
    let err = run_output_tail(&inner.secrets.scrub(&ctx.thread, &o.err), RUN_OUTPUT_MAX_CHARS as usize);
    let json = if js_trim(&out).is_empty() { None } else { serde_json::from_str::<Value>(&out).ok() };
    let exit = if o.timed_out { Some(EXEC_DEADLINE_EXIT) } else { o.code };
    let why = if let Some(error) = &o.error {
        Some(inner.secrets.scrub(&ctx.thread, error))
    } else if o.timed_out {
        Some(format!("timed out after {} s", number_string(ctx.timeout)))
    } else if exit == Some(0) {
        None
    } else if exit.is_none() {
        Some(format!("ended by {}", o.signal.as_deref().unwrap_or("a signal")))
    } else {
        Some(format!("exited with {}", exit.unwrap_or_default()))
    };
    let ok = exit == Some(0) && o.error.is_none();
    if !ok || ctx.then.is_none() {
        let state = if ok { RunState::Done } else { RunState::Failed };
        let fields = outcome_fields(state, why, exit, err, json);
        finish(ctx, &inner, state, fields, out, o.cut);
        return;
    }
    let then = ctx.then.clone().unwrap_or_default();
    let shaping = match &ctx.road {
        Some(road) => {
            let secrets = inner.secrets.clone();
            let thread = ctx.thread.clone();
            road.reshape(ReshapeStart {
                cmd: then,
                input: raw,
                cwd: ctx.folder.clone(),
                env: ctx.road_env.clone().unwrap_or_default(),
                timeout_s: DEFAULT_TIMEOUT_S as f64,
                scrub: Rc::new(move |text: &str| secrets.scrub(&thread, text)),
            })
        }
        None => inner.reshape_local(&ctx.thread, &then, &raw, &ctx.folder, DEFAULT_TIMEOUT_S as f64),
    };
    ctx.l.borrow_mut().reshaping = Some(shaping.kill.clone());
    let captured = Captured { out, err: err.clone(), exit, cut: o.cut };
    let ctx = ctx.clone();
    let done = shaping.done;
    spawn_local(async move {
        let answer = done.await;
        reshaped(&ctx, captured, answer);
    });
}

fn reshaped(ctx: &Rc<CtxInner>, captured: Captured, answer: ReshapeAnswer) {
    let Some(inner) = ctx.inner.upgrade() else { return };
    if ctx.l.borrow().gen != ctx.gen {
        return;
    }
    ctx.l.borrow_mut().reshaping = None;
    let (state, fields) = match answer {
        ReshapeAnswer::Json(json) => (RunState::Done, outcome_fields(RunState::Done, None, captured.exit, captured.err, Some(json))),
        ReshapeAnswer::Failed { why, exit, err } => (RunState::Failed, outcome_fields(RunState::Failed, Some(why), exit, err, None)),
    };
    finish(ctx, &inner, state, fields, captured.out, captured.cut);
}

fn finish(ctx: &Rc<CtxInner>, inner: &Rc<RunsInner>, state: RunState, mut fields: Map<String, Value>, out: String, cut: bool) {
    let ended_at = inner.clock.now();
    let why = fields.get("why").and_then(Value::as_str).map(str::to_string);
    if let Some(said) = inner.streak(&ctx.thread, &ctx.run, state, why.as_deref()) {
        fields.insert("why".into(), said.into());
    }
    fields.insert("out".into(), out.into());
    if ctx.stream {
        fields.insert("lines".into(), Value::Array(ctx.lines.borrow().iter().map(|l| Value::String(l.clone())).collect()));
    }
    fields.insert("startedAt".into(), ctx.started_at.into());
    fields.insert("endedAt".into(), ended_at.into());
    fields.insert("ms".into(), (ended_at - ctx.started_at).into());
    fields.insert("runs".into(), ctx.runs.into());
    if cut {
        fields.insert("cut".into(), true.into());
    }
    inner.write(&ctx.thread, &ctx.run, &ctx.l, RunRecord(fields));
    inner.next(&ctx.thread);
}

fn held_record(why: &str, runs: i64) -> RunRecord {
    let mut m = Map::new();
    m.insert("state".into(), "held".into());
    m.insert("why".into(), why.into());
    m.insert("runs".into(), runs.into());
    RunRecord(m)
}

fn running_in(t: &ThreadRuns) -> usize {
    t.runs.values().filter(|l| l.borrow().record.state() == Some(RunState::Running)).count()
}

fn resolve(base: &str, rel: &str) -> String {
    let joined = if rel.starts_with('/') { rel.to_string() } else { format!("{base}/{rel}") };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    format!("/{}", parts.join("/"))
}

fn interval_ms(every: f64) -> i64 {
    (every.max(TIMER_FLOOR_S as f64) * 1_000.0) as i64
}

impl RunsInner {
    fn with_thread<R>(&self, thread_id: &str, f: impl FnOnce(&mut ThreadRuns) -> R) -> R {
        let mut threads = self.threads.borrow_mut();
        f(threads.entry(thread_id.to_string()).or_default())
    }

    fn live_of(&self, thread_id: &str, run: &str) -> Option<LiveRef> {
        self.threads.borrow().get(thread_id).and_then(|t| t.runs.get(run).cloned())
    }

    fn live(&self, thread_id: &str, run: &str, last: Option<&RunRecord>) -> LiveRef {
        self.with_thread(thread_id, |t| t.runs.entry(run.to_string()).or_insert_with(|| Rc::new(RefCell::new(Live::new(last)))).clone())
    }

    fn write(&self, thread_id: &str, run: &str, l: &LiveRef, record: RunRecord) -> RunRecord {
        {
            let mut live = l.borrow_mut();
            let result = if record.state() == Some(RunState::Running) { None } else { last_result(&record) };
            live.record = record.clone();
            if let Some(result) = result {
                live.result = Some(result);
            }
        }
        self.deps.on_record(thread_id, run, record.clone());
        record
    }

    fn login_env(&self, thread_id: &str) -> Result<IndexMap<String, String>, String> {
        let mut env: IndexMap<String, String> = self.deps.env().into_iter().filter(|(name, _)| !name.starts_with("WSP_")).collect();
        if let Some(dir) = self.deps.dir(thread_id) {
            match dir {
                Ok(dir) => {
                    env.insert("SLATE_DIR".into(), dir);
                }
                Err(message) => return Err(files_unwritten(&message)),
            }
        }
        Ok(env)
    }

    fn running(&self, thread_id: &str) -> usize {
        self.threads.borrow().get(thread_id).map_or(0, running_in)
    }

    fn shown(&self, thread_id: &str, input: &RunInput) -> String {
        match input {
            RunInput::Secret(name) => {
                if self.secrets.handle(thread_id, name).set {
                    SECRET_DOTS.to_string()
                } else {
                    format!("{SECRET_DOTS} (not filled)")
                }
            }
            RunInput::Value(Value::Null) => "(unset)".to_string(),
            RunInput::Value(value) => as_text(value),
        }
    }

    fn ask(&self, req: &RunStart) -> RunAsk {
        let inputs: RunInputs = (req.inputs)();
        let env = inputs
            .env
            .as_ref()
            .map_or_else(IndexMap::new, |m| m.iter().map(|(name, input)| (name.clone(), self.shown(&req.thread_id, input))).collect());
        let args = inputs.args.as_deref().unwrap_or(&[]).iter().map(|input| self.shown(&req.thread_id, input)).collect();
        let stdin = inputs.stdin.as_ref().map(|input| {
            let shown = self.shown(&req.thread_id, input);
            shown.split('\n').next().unwrap_or("").to_string()
        });
        RunAsk {
            run: req.run.clone(),
            key: run_key(&req.decl),
            cmd: req.decl.cmd.clone(),
            env,
            args,
            stdin,
            folder: match &req.decl.cwd {
                None => req.folder.clone(),
                Some(cwd) => resolve(&req.folder, cwd),
            },
            on: req.decl.on.clone().unwrap_or_else(|| "thread".into()),
            timeout: JsNumber(timeout_of(&req.decl)),
            confirm: req.decl.confirm.clone(),
            then: req.decl.then.clone(),
            files: req.decl.files.clone(),
        }
    }

    fn streak(&self, thread_id: &str, run: &str, state: RunState, why: Option<&str>) -> Option<String> {
        let now = self.clock.now();
        let why = why.map(str::to_string);
        self.with_thread(thread_id, |t| {
            let Some(timer) = t.timers.get_mut(run) else { return why };
            if state == RunState::Done {
                timer.failures = 0;
            }
            if state != RunState::Failed {
                return why;
            }
            timer.failures += 1;
            timer.failed_at = Some(now);
            if timer.always && timer.failures >= BACKOFF_AFTER {
                Some(format!("{}; {BACKED_OFF}", why.as_deref().unwrap_or("it failed")))
            } else {
                why
            }
        })
    }

    fn hold(&self, req: RunStart, l: &LiveRef, why: HeldFor, beside: bool) -> RunStartAnswer {
        let thread = req.thread_id.clone();
        let run = req.run.clone();
        l.borrow_mut().held = Some((req.clone(), why));
        if why == HeldFor::Busy {
            self.with_thread(&thread, |t| {
                if !t.queue.contains(&run) {
                    t.queue.push(run.clone());
                }
            });
        }
        let asks = why == HeldFor::Approval;
        if beside {
            let record = l.borrow().record.clone();
            let ask = asks.then(|| self.ask(&req));
            return RunStartAnswer { outcome: Outcome::Held, record, ask };
        }
        let text = match why {
            HeldFor::Approval => HELD_APPROVAL,
            HeldFor::Busy => HELD_BUSY,
            HeldFor::Pressed => HELD_PRESSED,
            HeldFor::Budget => HELD_BUDGET,
        };
        let runs = l.borrow().record.runs();
        let record = self.write(&thread, &run, l, held_record(text, runs));
        let ask = asks.then(|| self.ask(&req));
        RunStartAnswer { outcome: Outcome::Held, record, ask }
    }

    fn fail(&self, req: &RunStart, l: &LiveRef, why: &str) -> RunStartAnswer {
        let ended_at = self.clock.now();
        let said = self.streak(&req.thread_id, &req.run, RunState::Failed, Some(why)).unwrap_or_else(|| why.to_string());
        let runs = l.borrow().record.runs();
        let mut m = Map::new();
        m.insert("state".into(), "failed".into());
        m.insert("why".into(), said.into());
        m.insert("exit".into(), Value::Null);
        m.insert("runs".into(), runs.into());
        m.insert("endedAt".into(), ended_at.into());
        let record = self.write(&req.thread_id, &req.run, l, RunRecord(m));
        RunStartAnswer { outcome: Outcome::Failed, record, ask: None }
    }

    fn next(self: &Rc<Self>, thread_id: &str) {
        loop {
            let taken: Option<Option<LiveRef>> = self.with_thread(thread_id, |t| {
                if t.queue.is_empty() || running_in(t) >= RUNNING_MAX {
                    return None;
                }
                let run = t.queue.remove(0);
                Some(t.runs.get(&run).cloned())
            });
            let Some(taken) = taken else { return };
            let Some(l) = taken else { continue };
            let req = {
                let mut live = l.borrow_mut();
                if !matches!(live.held, Some((_, HeldFor::Busy))) {
                    continue;
                }
                live.held.take().map(|(req, _)| req)
            };
            if let Some(req) = req {
                self.launch(req, &l);
            }
        }
    }

    fn launch(self: &Rc<Self>, req: RunStart, l: &LiveRef) -> RunStartAnswer {
        let thread = req.thread_id.clone();
        let run = req.run.clone();
        let decl = req.decl.clone();
        if l.borrow().record.state() == Some(RunState::Running) {
            self.stop(&thread, &run, l, None);
        }
        if self.running(&thread) >= RUNNING_MAX {
            return self.hold(req, l, HeldFor::Busy, false);
        }
        let inputs = (req.inputs)();
        if inputs.args.as_deref().unwrap_or(&[]).iter().any(|input| matches!(input, RunInput::Secret(_))) {
            return self.fail(&req, l, SLATE_SECRET_IN_ARGS);
        }
        let road = self.deps.road(&thread, decl.on.as_deref().unwrap_or("thread"));
        let road_env = road
            .as_ref()
            .map(|road| IndexMap::from([("SLATE_DIR".to_string(), road.slate_dir()), (THREAD_ENV.to_string(), thread.clone())]));
        let mut env = match &road_env {
            Some(env) => env.clone(),
            None => match self.login_env(&thread) {
                Ok(env) => env,
                Err(why) => return self.fail(&req, l, &why),
            },
        };
        if let Some(map) = &inputs.env {
            for (name, input) in map {
                if !is_env_name(name) {
                    return self.fail(&req, l, &format!("{name} is not a name an environment variable can have"));
                }
                let text = match input {
                    RunInput::Secret(secret) => self.secrets.plaintext(&thread, secret),
                    RunInput::Value(Value::Null) => None,
                    RunInput::Value(value) => Some(as_text(value)),
                };
                match text {
                    None => {
                        env.shift_remove(name);
                    }
                    Some(text) => {
                        env.insert(name.clone(), text);
                    }
                }
            }
        }
        let args: Vec<String> = inputs
            .args
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|input| match input {
                RunInput::Value(value) => as_text(value),
                RunInput::Secret(_) => String::new(),
            })
            .collect();
        let stdin = inputs.stdin.as_ref().map(|input| match input {
            RunInput::Secret(secret) => self.secrets.plaintext(&thread, secret).unwrap_or_default(),
            RunInput::Value(value) => as_text(value),
        });
        let cwd = match &decl.cwd {
            None => req.folder.clone(),
            Some(cwd) => resolve(&req.folder, cwd),
        };
        if road.is_none() && !Path::new(&cwd).is_dir() {
            return self.fail(&req, l, &format!("the folder {cwd} does not exist"));
        }
        let timeout = timeout_of(&decl);

        let gen = {
            let mut live = l.borrow_mut();
            live.gen += 1;
            live.gen
        };
        let started_at = self.clock.now();
        let runs = l.borrow().record.runs() + 1;
        let running = running_record(l.borrow().result.as_ref(), runs, started_at);
        let record = self.write(&thread, &run, l, running);

        let ctx = Rc::new(CtxInner {
            inner: Rc::downgrade(self),
            thread: thread.clone(),
            run: run.clone(),
            gen,
            l: l.clone(),
            stream: decl.stream == Some(true),
            then: decl.then.clone(),
            folder: cwd.clone(),
            timeout,
            started_at,
            runs,
            lines: RefCell::new(Vec::new()),
            over: Cell::new(false),
            release: RefCell::new(None),
            road: road.clone(),
            road_env,
        });
        let start = RoadStart { cmd: decl.cmd.clone(), args, cwd, env, stdin, timeout_s: timeout, stream: ctx.stream };
        let events: Rc<dyn RoadEvents> = Rc::new(ctx.clone());
        let started = match &road {
            Some(road) => road.start(start, events),
            None => self.local.start(start, events),
        };
        if !ctx.over.get() {
            if let Some(pid) = started.pid {
                let release = self.deps.own(&thread, pid, "slate");
                *ctx.release.borrow_mut() = release;
            }
        }
        let still_running = l.borrow().gen == gen && l.borrow().record.state() == Some(RunState::Running);
        if still_running {
            l.borrow_mut().kill = Some(started.kill.clone());
        }
        RunStartAnswer { outcome: Outcome::Running, record, ask: None }
    }

    /// `why` none writes nothing.
    fn stop(&self, thread_id: &str, run: &str, l: &LiveRef, why: Option<&str>) {
        let (was, kill, reshaping) = {
            let mut live = l.borrow_mut();
            let was = live.record.state();
            live.gen += 1;
            let kill = live.kill.take();
            let reshaping = live.reshaping.take();
            live.held = None;
            (was, kill, reshaping)
        };
        self.with_thread(thread_id, |t| t.queue.retain(|r| r != run));
        if let Some(kill) = kill {
            kill();
        }
        if let Some(kill) = reshaping {
            kill();
        }
        let Some(why) = why else { return };
        if was != Some(RunState::Running) && was != Some(RunState::Held) {
            return;
        }
        let ended_at = self.clock.now();
        let (runs, started_at) = {
            let live = l.borrow();
            (live.record.runs(), live.record.started_at())
        };
        let mut m = Map::new();
        m.insert("state".into(), "cancelled".into());
        m.insert("why".into(), why.into());
        m.insert("exit".into(), Value::Null);
        m.insert("runs".into(), runs.into());
        if let (Some(started), true) = (started_at, was == Some(RunState::Running)) {
            m.insert("startedAt".into(), started.into());
            m.insert("ms".into(), (ended_at - started).into());
        }
        m.insert("endedAt".into(), ended_at.into());
        self.write(thread_id, run, l, RunRecord(m));
    }

    fn arm(self: &Rc<Self>, thread_id: &str, run: &str, ms: i64) -> Cancel {
        let weak = Rc::downgrade(self);
        let (thread_id, run) = (thread_id.to_string(), run.to_string());
        self.clock.schedule(
            Box::new(move || {
                if let Some(inner) = weak.upgrade() {
                    inner.interval(&thread_id, &run, ms);
                }
            }),
            ms,
        )
    }

    fn interval(self: &Rc<Self>, thread_id: &str, run: &str, ms: i64) {
        let armed = self.with_thread(thread_id, |t| t.timers.get(run).is_some_and(|timer| timer.handle.is_some()));
        if !armed {
            return;
        }
        let handle = self.arm(thread_id, run, ms);
        let stored = self.with_thread(thread_id, |t| match t.timers.get_mut(run) {
            Some(timer) if timer.handle.is_some() => {
                timer.handle = Some(handle.clone());
                true
            }
            _ => false,
        });
        if stored {
            self.fire(thread_id, run);
        } else {
            handle();
        }
    }

    fn fire(&self, thread_id: &str, run: &str) {
        let now = self.clock.now();
        let skip = self.with_thread(thread_id, |t| match t.timers.get(run) {
            Some(timer) => timer.always && timer.failures >= BACKOFF_AFTER && timer.failed_at.is_some_and(|at| now - at < BACKOFF_MS),
            None => true,
        });
        if !skip {
            self.deps.on_timer(thread_id, run);
        }
    }

    fn tick(self: &Rc<Self>, thread_id: &str) {
        let (arm, disarm) = self.with_thread(thread_id, |t| {
            let shown = t.shown;
            let mut arm = Vec::new();
            let mut disarm = Vec::new();
            for (run, timer) in t.timers.iter_mut() {
                let on = timer.always || shown;
                if on && timer.handle.is_none() {
                    arm.push((run.clone(), interval_ms(timer.every)));
                } else if !on {
                    if let Some(handle) = timer.handle.take() {
                        disarm.push(handle);
                    }
                }
            }
            (arm, disarm)
        });
        for handle in disarm {
            handle();
        }
        for (run, ms) in arm {
            let handle = self.arm(thread_id, &run, ms);
            let stored = self.with_thread(thread_id, |t| match t.timers.get_mut(&run) {
                Some(timer) if timer.handle.is_none() => {
                    timer.handle = Some(handle.clone());
                    true
                }
                _ => false,
            });
            if stored {
                self.fire(thread_id, &run);
            } else {
                handle();
            }
        }
    }

    fn reshape_local(&self, thread_id: &str, cmd: &str, input: &str, folder: &str, timeout_s: f64) -> Reshape {
        let env = match self.login_env(thread_id) {
            Ok(env) => env,
            Err(why) => {
                return Reshape {
                    done: Box::pin(ready(ReshapeAnswer::Failed { why, exit: None, err: String::new() })),
                    kill: Rc::new(|| {}),
                }
            }
        };
        let cwd = if Path::new(folder).exists() {
            folder.to_string()
        } else {
            std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_default()
        };
        let secrets = self.secrets.clone();
        let thread = thread_id.to_string();
        reshape_result(
            ReshapeStart {
                cmd: cmd.to_string(),
                input: input.to_string(),
                cwd,
                env,
                timeout_s,
                scrub: Rc::new(move |text: &str| secrets.scrub(&thread, text)),
            },
            self.clock.clone(),
        )
    }

    fn start(self: &Rc<Self>, req: RunStart) -> RunStartAnswer {
        let l = self.live(&req.thread_id, &req.run, req.last.as_ref());
        let thread = req.thread_id.clone();
        let run = req.run.clone();
        if req.asleep && req.by == RunBy::Timer {
            if l.borrow().record.state() == Some(RunState::Running) {
                let record = l.borrow().record.clone();
                return RunStartAnswer { outcome: Outcome::Noop, record, ask: None };
            }
            l.borrow_mut().held = None;
            let runs = l.borrow().record.runs();
            let mut m = l.borrow().result.clone().unwrap_or_default();
            m.insert("state".into(), "held".into());
            m.insert("why".into(), HELD_ASLEEP.into());
            m.insert("runs".into(), runs.into());
            let record = self.write(&thread, &run, &l, RunRecord(m));
            return RunStartAnswer { outcome: Outcome::Held, record, ask: None };
        }
        let refused = if req.decl.on.as_deref() == Some("host") { self.deps.host_refused(&thread, &run) } else { None };
        if let Some(why) = refused {
            return self.fail(&req, &l, &why);
        }
        let beside = l.borrow().record.state() == Some(RunState::Running);
        if beside && req.decl.once == Some(true) {
            let record = l.borrow().record.clone();
            return RunStartAnswer { outcome: Outcome::Noop, record, ask: None };
        }
        l.borrow_mut().held = None;
        let person = req.by == RunBy::Person;
        let now = self.clock.now();
        let times = {
            let live = l.borrow();
            if person {
                spent(live.pressed.as_deref(), now)
            } else {
                spent(Some(live.starts.as_slice()), now)
            }
        };
        let Some(times) = times else {
            let why = if person { HeldFor::Pressed } else { HeldFor::Budget };
            return self.hold(req, &l, why, beside);
        };
        {
            let mut live = l.borrow_mut();
            if person {
                live.pressed = Some(times);
            } else {
                live.starts = times;
            }
        }
        if req.decl.confirm.is_some() || !self.approvals.has(&thread, &run_key(&req.decl)) {
            return self.hold(req, &l, HeldFor::Approval, beside);
        }
        self.launch(req, &l)
    }

    fn drop_timers(t: ThreadRuns) {
        for timer in t.timers.into_values() {
            if let Some(handle) = timer.handle {
                handle();
            }
        }
    }
}

impl SlateRuns for SlateRunsTable {
    fn key(&self, decl: &CmdRunDecl) -> String {
        run_key(decl)
    }
    fn start(&self, req: RunStart) -> RunStartAnswer {
        self.inner.start(req)
    }
    fn approve(&self, thread_id: &str, run: &str, scope: ApproveScope) -> Option<RunStartAnswer> {
        let l = self.inner.live_of(thread_id, run)?;
        let req = {
            let mut live = l.borrow_mut();
            if !matches!(live.held, Some((_, HeldFor::Approval))) {
                return None;
            }
            live.held.take()?.0
        };
        if scope == ApproveScope::Always {
            self.inner.approvals.allow(thread_id, &run_key(&req.decl));
        }
        Some(self.inner.launch(req, &l))
    }
    fn deny(&self, thread_id: &str, run: &str) {
        let Some(l) = self.inner.live_of(thread_id, run) else { return };
        let (held, busy) = {
            let live = l.borrow();
            (live.held.is_some(), live.kill.is_some() || live.reshaping.is_some())
        };
        if !held {
            return;
        }
        if busy {
            l.borrow_mut().held = None;
            self.inner.with_thread(thread_id, |t| t.queue.retain(|r| r != run));
            return;
        }
        self.inner.stop(thread_id, run, &l, Some("you said not to run it"));
    }
    fn revoke(&self, thread_id: &str, key: &str) {
        self.inner.approvals.revoke(thread_id, key);
    }
    fn allowed(&self, thread_id: &str) -> Vec<String> {
        self.inner.approvals.list(thread_id)
    }
    fn held(&self, thread_id: &str) -> Vec<RunAsk> {
        let reqs: Vec<RunStart> = self.inner.threads.borrow().get(thread_id).map_or_else(Vec::new, |t| {
            t.runs
                .values()
                .filter_map(|l| {
                    let live = l.borrow();
                    let req = match &live.held {
                        Some((req, HeldFor::Approval)) => Some(req.clone()),
                        _ => None,
                    };
                    req
                })
                .collect()
        });
        reqs.iter().map(|req| self.inner.ask(req)).collect()
    }
    fn cancel(&self, thread_id: &str, run: &str) {
        let Some(l) = self.inner.live_of(thread_id, run) else { return };
        self.inner.stop(thread_id, run, &l, Some("cancelled"));
        self.inner.next(thread_id);
    }
    fn stop_all(&self, thread_id: &str, quiet: bool, why: Option<&str>) {
        let runs: Vec<(String, LiveRef)> = {
            let threads = self.inner.threads.borrow();
            let Some(t) = threads.get(thread_id) else { return };
            t.runs.iter().map(|(run, l)| (run.clone(), l.clone())).collect()
        };
        for (run, l) in runs {
            let why = if quiet { None } else { Some(why.unwrap_or("cancelled")) };
            self.inner.stop(thread_id, &run, &l, why);
        }
    }
    fn timers(&self, thread_id: &str, list: Vec<TimerDecl>) {
        let cancels = self.inner.with_thread(thread_id, |t| {
            let mut kept: IndexMap<String, Timer> = IndexMap::new();
            let mut cancels = Vec::new();
            for (run, was) in std::mem::take(&mut t.timers) {
                let same = list.iter().any(|n| n.run == run && n.key == was.key && n.every == was.every && n.always == was.always);
                if same {
                    kept.insert(run, was);
                } else if let Some(handle) = was.handle {
                    cancels.push(handle);
                }
            }
            for timer in &list {
                kept.entry(timer.run.clone()).or_insert_with(|| Timer {
                    every: timer.every,
                    key: timer.key.clone(),
                    always: timer.always,
                    handle: None,
                    failures: 0,
                    failed_at: None,
                });
            }
            t.timers = kept;
            cancels
        });
        for cancel in cancels {
            cancel();
        }
        self.inner.tick(thread_id);
    }
    fn shown(&self, thread_id: &str, shown: bool) {
        self.inner.with_thread(thread_id, |t| t.shown = shown);
        self.inner.tick(thread_id);
    }
    fn release(&self, thread_id: &str) {
        if let Some(t) = self.inner.threads.borrow().get(thread_id) {
            for l in t.runs.values() {
                l.borrow_mut().starts.clear();
            }
        }
    }
    fn reshape(&self, thread_id: &str, cmd: &str, input: &str, folder: &str, timeout_s: Option<f64>) -> Reshape {
        self.inner.reshape_local(thread_id, cmd, input, folder, timeout_s.unwrap_or(DEFAULT_TIMEOUT_S as f64))
    }
    fn secrets(&self) -> Rc<dyn SlateSecrets> {
        self.inner.secrets.clone()
    }
    fn drop_thread(&self, thread_id: &str, keep: bool) {
        let runs: Option<Vec<(String, LiveRef)>> =
            self.inner.threads.borrow().get(thread_id).map(|t| t.runs.iter().map(|(run, l)| (run.clone(), l.clone())).collect());
        if let Some(runs) = runs {
            if !keep {
                for (run, l) in runs {
                    self.inner.stop(thread_id, &run, &l, None);
                }
            }
            let removed = self.inner.threads.borrow_mut().shift_remove(thread_id);
            if let Some(t) = removed {
                RunsInner::drop_timers(t);
            }
        }
        for key in self.inner.approvals.list(thread_id) {
            self.inner.approvals.revoke(thread_id, &key);
        }
        self.inner.secrets.forget(thread_id, None);
    }
    fn close(&self) {
        let threads: Vec<(String, Vec<(String, LiveRef)>)> = self
            .inner
            .threads
            .borrow()
            .iter()
            .map(|(id, t)| (id.clone(), t.runs.iter().map(|(run, l)| (run.clone(), l.clone())).collect()))
            .collect();
        for (thread_id, runs) in &threads {
            for (run, l) in runs {
                self.inner.stop(thread_id, run, l, None);
            }
        }
        let removed = std::mem::take(&mut *self.inner.threads.borrow_mut());
        for (_, t) in removed {
            RunsInner::drop_timers(t);
        }
    }
}
