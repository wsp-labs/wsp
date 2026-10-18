// SPDX-License-Identifier: AGPL-3.0-only
//! The right panel's Slate tab, after `apps/web/src/slate/SlateSurface.tsx`: the open thread's slate, read with
//! `slates.get`, kept live by the host's `session.slate`, `slate.values` and `slate.run` pushes, and drawn piece by
//! piece. Every formula a drawn piece reads is answered by the host's `slates.resolve`, the one evaluator: asked again when a
//! value moves, and each second only while a drawn formula reads the clock. Presses, writes, approvals and the menu's
//! acts go through the host's own ops.

pub mod consent;
pub mod model;
pub mod pieces;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClipboardItem, Context, Entity, SharedString, Subscription, Task, Window};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use wsp_host_client::{protocol, Event, Failure, HostClient, Op};

use crate::screens::{Host, Opened, Pane, Parts, Place, Screen};
use crate::ui::{icon_menu, ActivePalette, Button, MenuAct};
use model::{Doc, Piece, Record, Row, Rows};

/// The panel's words, `SLATE_WORDS` in `SlateSurface.tsx`.
pub mod words {
    pub const NO_THREAD: &str = "Pick a thread to see its slate.";
    pub const NOTHING: &str = "Nothing here yet";
    pub const ASK: &str = "The agent builds this panel when there is something to watch, track, fill in or press. Ask for one.";
    pub const ASK_BUTTON: &str = "Ask for one";
    pub const ASK_PROMPT: &str = "Build a slate for this thread that shows ";
    pub const CLEARED: &str = "Cleared";
    pub const REWOUND: &str = "Rewound to before this slate existed";
    pub fn newer(schema: u64) -> String {
        format!("This slate needs a newer wsp (schema {schema})")
    }
    pub fn undo(version: u64) -> String {
        format!("Undo v{version}")
    }
    pub const CLEAR: &str = "Clear";
    pub const COPY: &str = "Copy as text";
    pub const STOP: &str = "Stop runs";
    pub const FORGET: &str = "Forget secrets";
    pub const FORGET_TITLE: &str = "Forget this slate's secrets?";
    pub const FORGET_BODY: &str = "A secret it kept does not come back. The slate asks you to type it again.";
    pub const MENU: &str = "Slate menu";
    pub const APPROVALS: &str = "Approvals";
    pub const TITLE: &str = "Slate";
}

/// The hold a drawn slate keeps on the host while the tab shows it (`SHOWN_HOLD`).
pub const SHOWN_HOLD: &str = "slate";
/// The most formulas one `slates.resolve` takes (`SlatesResolveParams`).
const RESOLVE_MAX: usize = 200;
/// The round trips the panel keeps for its report, newest last.
const TIMINGS_KEPT: usize = 64;
/// The lines a streaming run keeps (`LINES_KEPT` in `engine.ts`).
const LINES_KEPT: usize = 500;

/// One thread's slate as drawn: the host's record, what the host answered for each formula, and the lines its runs
/// streamed since they last started.
pub struct Slate {
    pub record: Record,
    /// Each formula text the panel asked, by its text, as the host last answered it; null where it failed.
    pub resolved: HashMap<String, Value>,
    pub lines: HashMap<String, Vec<String>>,
    /// Values the person typed or picked that the host has not answered for yet, by own path.
    pub mine: HashMap<String, Value>,
    /// Pieces whose press is out, which draw busy until it is answered.
    pub busy: HashSet<String>,
    /// What each piece's last press said or why it was refused, shown under it for two seconds.
    pub outcomes: HashMap<String, Outcome>,
    /// Runs refreshing whose streamed lines are still the last result's, until their own first line arrives.
    pub fresh: HashSet<String>,
}

impl Slate {
    pub fn new(record: Record) -> Slate {
        Slate {
            record,
            resolved: HashMap::new(),
            lines: HashMap::new(),
            mine: HashMap::new(),
            busy: HashSet::new(),
            outcomes: HashMap::new(),
            fresh: HashSet::new(),
        }
    }

    pub fn doc(&self) -> Option<&Doc> {
        self.record.doc.as_ref()
    }

    /// Whether `id` is the slate's one piece of this loud kind.
    pub fn is_loud(&self, kind: model::Loud, id: &str) -> bool {
        self.doc().is_some_and(|d| d.is_loud(kind, id))
    }

    /// Whether a piece shows now: it is in the document and its `when`, if any, answered truthy.
    pub fn shown(&self, id: &str) -> bool {
        let Some(piece) = self.doc().and_then(|d| d.piece(id)) else { return false };
        match &piece.when {
            None => true,
            Some(when) => truthy(self.resolved.get(&format!("({when})"))),
        }
    }

    /// A piece's props: literals as written, live ones as the host answered them, the ones read per row left out.
    pub fn props(&self, piece: &Piece) -> Map<String, Value> {
        let scoped = model::row_scoped(&piece.kind);
        let rows = !scoped.is_empty() && piece.prop("items").is_some();
        let mut out = Map::new();
        for (name, value) in &piece.props {
            if scoped.contains(&name.as_str()) || (rows && name == "items") {
                continue;
            }
            if let Some(v) = model::filled(value, &self.resolved) {
                out.insert(name.clone(), v);
            }
        }
        out
    }

    pub fn rows(&self, piece: &Piece) -> Option<Vec<Row>> {
        Rows::of(piece)?.read(&self.resolved)
    }

    /// The value at an own path as this window draws it: what the person typed over the host's copy.
    pub fn value(&self, path: &str) -> Option<&Value> {
        self.mine.get(path).or_else(|| self.record.get_own(path))
    }

    /// Every formula the drawn pieces read: each `when`, each live prop, and each repeating piece's rows.
    pub fn formulas(&self) -> Vec<String> {
        let Some(doc) = self.doc() else { return Vec::new() };
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        let mut add = |e: String| {
            if seen.insert(e.clone()) {
                out.push(e);
            }
        };
        for piece in doc.drawn() {
            if let Some(when) = &piece.when {
                add(format!("({when})"));
            }
            // A timeline with no `now` of its own reads the clock (`reads` in timeline.tsx).
            if piece.kind == "timeline" && piece.prop("now").is_none() {
                add("time.now".to_owned());
            }
            let scoped = model::row_scoped(&piece.kind);
            let rows = Rows::of(piece);
            for formula in rows.iter().flat_map(Rows::formulas) {
                add(formula);
            }
            for (name, value) in &piece.props {
                if scoped.contains(&name.as_str()) || (rows.is_some() && name == "items") {
                    continue;
                }
                for when in item_whens(value) {
                    add(format!("({when})"));
                }
                for formula in model::leaves(value) {
                    add(formula);
                }
            }
            // What a press opens, copies or carries into the composer is run here, so its value is asked with the rest.
            for step in piece.on.values().flat_map(|steps| steps.as_array().cloned().unwrap_or_else(|| vec![steps.clone()])) {
                for part in ["target", "text"]
                    .into_iter()
                    .filter_map(|k| step.get(k))
                    .filter(|_| matches!(step.get("do").and_then(Value::as_str), Some("open" | "copy")))
                {
                    for formula in model::leaves(part) {
                        add(formula);
                    }
                }
                if step.get("do").and_then(Value::as_str) == Some("fill") {
                    for path in step.get("with").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
                        add(path.to_owned());
                    }
                }
            }
        }
        out
    }

    /// `present`: the entries of a list prop the document wrote out whose own `when` answered truthy, as resolved; an
    /// entry whose `when` has not answered yet is left out. A list from a value has no whens and comes whole.
    pub fn present(&self, raw: Option<&Value>, resolved: &[Value]) -> Vec<Value> {
        let Some(Value::Array(raw)) = raw else { return resolved.to_vec() };
        if raw.len() != resolved.len() {
            return resolved.to_vec();
        }
        raw.iter()
            .zip(resolved)
            .filter(|(r, _)| match r.get("when").and_then(Value::as_str) {
                Some(when) => truthy(self.resolved.get(&format!("({when})"))),
                None => true,
            })
            .map(|(_, v)| v.clone())
            .collect()
    }
}

/// The own `when` of each entry of a list prop the document wrote out (`whenOf` in `paths.ts`).
fn item_whens(value: &Value) -> impl Iterator<Item = &str> {
    value.as_array().into_iter().flatten().filter_map(|item| item.get("when").and_then(Value::as_str))
}

/// `slateTruthy`: missing, false, 0, "" and an empty list are false.
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) | Some(Value::Bool(false)) => false,
        Some(Value::Number(n)) => n.as_f64() != Some(0.),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        _ => true,
    }
}

/// What a press, a submit or a change is raised with: a row's scope and a row action's number.
#[derive(Clone, Debug, Default)]
pub struct Raise {
    pub row: Option<(Value, usize)>,
    pub row_action: Option<usize>,
    /// A row action's own steps, which stand in for the piece's handler.
    pub steps: Option<Value>,
}

/// What the person sees under a piece for two seconds after a press: an outcome, or a refusal sentence.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Said(String),
    Refused(String),
}

/// Where the panel stands with its thread's record.
pub enum Load {
    /// No thread open: the panel's own empty line.
    NoThread,
    /// Asked and not answered, or the get failed: blank, as the web's tab is.
    Waiting,
    /// No slate, cleared, rewound or newer: the empty state.
    Empty(Box<Record>),
    Drawn(Box<Slate>),
}

pub struct SlateScreen {
    host: Host,
    pub opened: Opened,
    pub load: Load,
    /// The menu's last refusal, under the header until the next act.
    pub refused: Option<String>,
    /// The empty state's Undo refusal.
    pub empty_refused: Option<String>,
    pub consent: consent::Consent,
    /// Each `slates.resolve` round trip this panel made, newest last, for the report.
    pub timings: Vec<Duration>,
    /// Whether a drawn formula reads the clock, so the tick asks again each second.
    clocked: bool,
    /// The `slates.resolve` in flight; a newer ask drops it, and a tick waits for it.
    resolving: Option<Task<()>>,
    /// The answer the person's press or write is awaiting, counted so a stale get never lands over a newer one.
    loads: u64,
    /// Each path's latest write, so an earlier write's answer does not undo a later one drawn meanwhile.
    writes: HashMap<String, u64>,
    /// When the panel last drew the slate: a tab behind another or a shut panel keeps its screen but draws nothing.
    drawn_at: Option<Instant>,
    /// Whether this panel holds the host's shown hold for the thread.
    holding: bool,
    /// Words a press's fill puts in the composer on the next draw, which has the window the composer needs.
    filling: Option<String>,
    _follow: Option<Task<()>>,
    _pump: Option<Task<()>>,
    _tick: Option<Task<()>>,
    _watch: Option<Task<()>>,
    _release: Option<Subscription>,
}

/// How long after its last draw the slate counts as off screen: its hold goes and its clock stops asking.
const OFF_SCREEN: Duration = Duration::from_secs(2);

impl Screen for SlateScreen {
    const NAME: &'static str = "slate";
    const PLACE: Place = Place::Panel(Pane::Slate);

    fn new(host: Host, opened: Opened, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut screen = SlateScreen {
            host,
            load: if opened.thread_id.is_some() { Load::Waiting } else { Load::NoThread },
            opened,
            refused: None,
            empty_refused: None,
            consent: consent::Consent::default(),
            timings: Vec::new(),
            clocked: false,
            resolving: None,
            loads: 0,
            writes: HashMap::new(),
            drawn_at: None,
            holding: false,
            filling: None,
            _follow: None,
            _pump: None,
            _tick: None,
            _watch: None,
            _release: None,
        };
        screen.follow(cx);
        screen._watch = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(OFF_SCREEN).await;
            if this.update(cx, |this, cx| this.sync_hold(cx)).is_err() {
                break;
            }
        }));
        screen._release = Some(cx.on_release(|this, cx| {
            if let (true, Some(thread)) = (this.holding, this.opened.thread_id.clone()) {
                let letting_go = this.host.run(move |host| async move {
                    call::<protocol::SlatesUnsubscribeRequest>(&host, json!({ "threadId": thread, "sources": [SHOWN_HOLD] })).await
                });
                cx.background_spawn(async move {
                    let _ = letting_go.await;
                })
                .detach();
            }
        }));
        screen
    }

    fn parts(_this: &Entity<Self>, _window: &mut Window, _cx: &mut App) -> Parts {
        Parts::default()
    }
}

/// A typed op from loose params, as the sidebar sends them.
async fn call<O: Op + DeserializeOwned>(host: &HostClient, params: Value) -> Result<O::Reply, Failure> {
    let op: O = serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))?;
    host.call(op).await
}

/// What the follow loop hands the window.
enum Change {
    Record(Option<Box<Value>>),
    Values { revision: u64, values: Map<String, Value> },
    Lines { run: String, lines: Vec<String> },
}

/// The host's refusal as the person reads it: its own words, which lead with its code and name.
pub fn refusal(failure: &Failure) -> String {
    failure.message.clone()
}

impl SlateScreen {
    pub fn thread(&self) -> Option<&str> {
        self.opened.thread_id.as_deref()
    }

    pub fn slate(&self) -> Option<&Slate> {
        match &self.load {
            Load::Drawn(s) => Some(s),
            _ => None,
        }
    }

    pub fn record(&self) -> Option<&Record> {
        match &self.load {
            Load::Drawn(s) => Some(&s.record),
            Load::Empty(r) => Some(&**r),
            _ => None,
        }
    }

    pub fn host(&self) -> &Host {
        &self.host
    }

    /// Listens for the thread's pushes before the first read, so nothing pushed between the two is lost, and reads
    /// the record again on every push that moves the document.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let (tx, mut rx) = mpsc::unbounded::<Change>();
        let follow = self.host.run(move |host| async move {
            loop {
                let ended: Result<(), Failure> = async {
                    let mut events = host.events().await?;
                    let got = call::<protocol::SlatesGetRequest>(&host, json!({ "threadId": thread })).await?;
                    let slate = serde_json::to_value(&got.slate).ok().filter(|v| !v.is_null()).map(Box::new);
                    tx.unbounded_send(Change::Record(slate)).map_err(|_| Failure::new("gone"))?;
                    while let Some(event) = events.next().await {
                        let Ok(Event::Host(event)) = event else { continue };
                        let Ok(frame) = serde_json::to_value(&*event) else { continue };
                        if frame.get("threadId").and_then(Value::as_str) != Some(thread.as_str()) {
                            continue;
                        }
                        let change = match frame.get("type").and_then(Value::as_str) {
                            Some("session.slate") => {
                                let got = call::<protocol::SlatesGetRequest>(&host, json!({ "threadId": thread })).await?;
                                Change::Record(serde_json::to_value(&got.slate).ok().filter(|v| !v.is_null()).map(Box::new))
                            }
                            Some("slate.values") => Change::Values {
                                revision: frame.get("revision").and_then(Value::as_u64).unwrap_or_default(),
                                values: frame.get("values").and_then(Value::as_object).cloned().unwrap_or_default(),
                            },
                            Some("slate.run") => Change::Lines {
                                run: frame.get("run").and_then(Value::as_str).unwrap_or_default().to_owned(),
                                lines: frame
                                    .get("lines")
                                    .and_then(Value::as_array)
                                    .map(|l| l.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
                                    .unwrap_or_default(),
                            },
                            _ => continue,
                        };
                        tx.unbounded_send(change).map_err(|_| Failure::new("gone"))?;
                    }
                    Err(Failure::new("the host closed the socket"))
                }
                .await;
                if tx.is_closed() {
                    return Ok(());
                }
                if let Err(f) = ended {
                    if f.message == "gone" {
                        return Ok(());
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        self._follow = Some(cx.spawn(async move |_, _| {
            let _: Result<(), Failure> = follow.await;
        }));
        self._pump = Some(cx.spawn(async move |this, cx| {
            while let Some(change) = rx.next().await {
                if this.update(cx, |this, cx| this.take(change, cx)).is_err() {
                    break;
                }
            }
        }));
    }

    fn take(&mut self, change: Change, cx: &mut Context<Self>) {
        match change {
            Change::Record(slate) => self.take_record(slate.and_then(|s| Record::read(&s)), cx),
            Change::Values { revision, values } => {
                if let Load::Drawn(slate) = &mut self.load {
                    if revision <= slate.record.revision {
                        return;
                    }
                    slate.record.revision = revision;
                    for (path, value) in values {
                        restarted(slate, &path, &value);
                        slate.record.set_own(&path, value);
                    }
                    self.resolve(cx);
                }
            }
            Change::Lines { run, lines } => {
                if let Load::Drawn(slate) = &mut self.load {
                    let fresh = slate.fresh.remove(&run);
                    let kept = slate.lines.entry(run).or_default();
                    if fresh {
                        kept.clear();
                    }
                    kept.extend(lines);
                    let over = kept.len().saturating_sub(LINES_KEPT);
                    kept.drain(..over);
                    cx.notify();
                }
            }
        }
    }

    /// A record from the host: drawn if it holds a document this build reads, else the empty state.
    pub fn take_record(&mut self, record: Option<Record>, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let record = record.unwrap_or_else(|| Record { thread_id: thread.clone(), ..Record::default() });
        if let Load::Drawn(slate) = &self.load {
            // A read that left before a newer one landed: older in both counts, it is dropped whole.
            if record.revision < slate.record.revision && record.version < slate.record.version {
                return;
            }
        }
        let first = !matches!(self.load, Load::Drawn(_));
        if record.doc.is_none() {
            self.load = Load::Empty(Box::new(record));
            self._tick = None;
            self.clocked = false;
            self.resolving = None;
            self.sync_hold(cx);
            cx.notify();
            return;
        }
        let shown_once = record.shown_once;
        self.load = match std::mem::replace(&mut self.load, Load::Waiting) {
            Load::Drawn(mut slate) => {
                for (name, value) in &record.values {
                    if slate.record.values.get(name) != Some(value) {
                        restarted(&mut slate, &format!("${name}"), value);
                    }
                }
                slate.record = record;
                slate.mine.retain(|path, mine| slate.record.get_own(path) != Some(mine));
                Load::Drawn(slate)
            }
            _ => Load::Drawn(Box::new(Slate::new(record))),
        };
        if first {
            self.sync_hold(cx);
            if !shown_once {
                let asking = self
                    .host
                    .run(move |host| async move { call::<protocol::SlatesShownRequest>(&host, json!({ "threadId": thread })).await });
                cx.spawn(async move |_, _| {
                    let _ = asking.await;
                })
                .detach();
            }
        }
        self.resolve(cx);
    }

    fn on_screen(&self) -> bool {
        self.drawn_at.is_some_and(|at| at.elapsed() < OFF_SCREEN)
    }

    /// Takes the host's hold while a drawn slate is on screen, which keeps its `every` runs ticking, and lets it go
    /// once the slate is gone or off screen, as the web's tab does when it unmounts.
    fn sync_hold(&mut self, cx: &mut Context<Self>) {
        let take = matches!(self.load, Load::Drawn(_)) && self.on_screen();
        if take == self.holding {
            return;
        }
        self.holding = take;
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let asking = self.host.run(move |host| async move {
            let params = json!({ "threadId": thread, "sources": [SHOWN_HOLD] });
            if take {
                call::<protocol::SlatesSubscribeRequest>(&host, params).await
            } else {
                call::<protocol::SlatesUnsubscribeRequest>(&host, params).await
            }
        });
        cx.spawn(async move |_, _| {
            let _ = asking.await;
        })
        .detach();
    }

    /// Asks the host for every formula the drawn pieces read, in batches of 200 at once, and draws the answers.
    pub fn resolve(&mut self, cx: &mut Context<Self>) {
        let (Some(thread), Load::Drawn(slate)) = (self.opened.thread_id.clone(), &self.load) else { return };
        let formulas = slate.formulas();
        let clocked = formulas.iter().any(|f| model::reads_clock(f));
        if clocked != self.clocked {
            self.clocked = clocked;
            self._tick = clocked.then(|| {
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(Duration::from_secs(1)).await;
                    let ticked = this.update(cx, |this, cx| {
                        if this.resolving.is_none() && this.on_screen() {
                            this.resolve(cx);
                        }
                    });
                    if ticked.is_err() {
                        break;
                    }
                })
            });
        }
        if formulas.is_empty() {
            cx.notify();
            return;
        }
        let started = Instant::now();
        let asking = self.host.run(move |host| async move {
            let batches = formulas.chunks(RESOLVE_MAX).map(|batch| {
                let host = host.clone();
                let params = json!({ "threadId": thread, "paths": batch });
                async move { call::<protocol::SlatesResolveRequest>(&host, params).await }
            });
            let mut values = Map::new();
            for answer in futures::future::join_all(batches).await {
                values.extend(answer?.values);
            }
            Ok(values)
        });
        self.resolving = Some(cx.spawn(async move |this, cx| {
            let answer = asking.await;
            let took = started.elapsed();
            _ = this.update(cx, |this, cx| {
                this.resolving = None;
                this.timings.push(took);
                let over = this.timings.len().saturating_sub(TIMINGS_KEPT);
                this.timings.drain(..over);
                if let (Ok(values), Load::Drawn(slate)) = (answer, &mut this.load) {
                    slate.resolved = values.into_iter().collect();
                    cx.notify();
                }
            });
        }));
    }

    /// Reads the record again, as after an act the host does not push for this window.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        self.loads += 1;
        let loads = self.loads;
        let asking =
            self.host.run(move |host| async move { call::<protocol::SlatesGetRequest>(&host, json!({ "threadId": thread })).await });
        cx.spawn(async move |this, cx| {
            let Ok(got) = asking.await else { return };
            let slate = serde_json::to_value(&got.slate).ok().filter(|v| !v.is_null());
            _ = this.update(cx, |this, cx| {
                if loads == this.loads {
                    this.take_record(slate.and_then(|s| Record::read(&s)), cx);
                }
            });
        })
        .detach();
    }

    /// Writes one value the person typed or picked: drawn at once, then sent with `slates.state`.
    pub fn write(&mut self, path: String, value: Value, cx: &mut Context<Self>) {
        self.write_then(path, value, |_, _| {}, cx);
    }

    /// `write`, then `then` once the host has answered it: a pick's change event reads the value it wrote.
    pub fn write_then(
        &mut self,
        path: String,
        value: Value,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        if let Load::Drawn(slate) = &mut self.load {
            slate.mine.insert(path.clone(), value.clone());
        }
        let sent = self.writes.entry(path.clone()).or_default();
        *sent += 1;
        let sent = *sent;
        cx.notify();
        let asking = self.host.run(move |host| async move {
            let mut values = Map::new();
            values.insert(path.clone(), value.clone());
            call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": values })).await.map(|_| (path, value))
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                let took = answer.is_ok();
                if let (Ok((path, value)), Load::Drawn(slate)) = (answer, &mut this.load) {
                    if this.writes.get(&path) == Some(&sent) {
                        this.writes.remove(&path);
                        slate.record.set_own(&path, value);
                        slate.mine.remove(&path);
                    }
                }
                this.resolve(cx);
                if took {
                    then(this, cx);
                }
            });
        })
        .detach();
    }

    /// A press, a submit or a change: the host runs the piece's steps off its stored version through
    /// `slates.event` and answers an outcome, with a held run's ask when it held one.
    pub fn raise(&mut self, piece: String, event: &'static str, raise: Raise, cx: &mut Context<Self>) {
        let (Some(thread), Some(slate)) = (self.opened.thread_id.clone(), self.slate()) else { return };
        let version = slate.record.version;
        let steps = raise.steps.clone().or_else(|| slate.doc().and_then(|d| d.piece(&piece)).and_then(|p| p.on.get(event).cloned()));
        let steps: Vec<Value> = match steps {
            Some(Value::Array(a)) => a,
            Some(one @ Value::Object(_)) => vec![one],
            _ => Vec::new(),
        };
        let host_steps = steps.iter().any(|s| {
            matches!(s.get("do").and_then(Value::as_str), Some("set" | "toggle" | "start" | "cancel" | "send" | "steer" | "queue"))
        });
        let window_steps: Vec<Value> = steps
            .iter()
            .filter(|s| matches!(s.get("do").and_then(Value::as_str), Some("open" | "copy" | "fill" | "pane")))
            .cloned()
            .collect();
        if let Load::Drawn(slate) = &mut self.load {
            slate.busy.insert(piece.clone());
            slate.outcomes.remove(&piece);
        }
        cx.notify();
        let request = REQUESTS.with(|n| {
            let next = n.get() + 1;
            n.set(next);
            format!("{thread}:{version}:{piece}:{}:{next}", window_id())
        });
        let mut params = json!({ "threadId": thread, "version": version, "piece": piece, "event": event, "requestId": request });
        if let Some((item, index)) = &raise.row {
            params["scope"] = json!({ "item": item, "index": index });
        }
        if let Some(action) = raise.row_action {
            params["rowAction"] = json!(action);
        }
        // What a row's own open, copy or fill reads is asked with the row as the one element of a list, the only row
        // scope a formula sent to the host can carry.
        let row_asks: Vec<(String, String)> = match &raise.row {
            Some((item, _)) => window_steps
                .iter()
                .flat_map(|step| ["target", "text"].into_iter().filter_map(|k| step.get(k)).flat_map(model::leaves).collect::<Vec<_>>())
                .map(|leaf| (format!("[{}] | map(_slate0: {leaf})", model::literal(item)), leaf))
                .filter(|(formula, _)| formula.len() <= model::EXPR_CHARS)
                .collect(),
            None => Vec::new(),
        };
        let row_thread = thread.clone();
        let asking = self.host.run(move |host| async move {
            let answer = if host_steps {
                call::<protocol::SlatesEventRequest>(&host, params).await.map(|a| serde_json::to_value(a).ok())?
            } else {
                None
            };
            let mut rows = HashMap::new();
            if !row_asks.is_empty() {
                let paths: Vec<&String> = row_asks.iter().map(|(f, _)| f).collect();
                let read = call::<protocol::SlatesResolveRequest>(&host, json!({ "threadId": row_thread, "paths": paths })).await?;
                for (formula, leaf) in row_asks {
                    if let Some(v) = read.values.get(&formula).and_then(|v| v.get(0)).and_then(|v| v.get("_slate0")) {
                        rows.insert(leaf, v.clone());
                    }
                }
            }
            Ok((answer, rows))
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                if let Load::Drawn(slate) = &mut this.load {
                    slate.busy.remove(&piece);
                }
                match answer {
                    Err(failure) => this.set_outcome(&piece, Outcome::Refused(refusal(&failure))),
                    Ok((answer, rows)) => {
                        let answer = answer.unwrap_or(Value::Null);
                        if let Some(ask) = answer.get("ask").and_then(|a| serde_json::from_value::<model::Ask>(a.clone()).ok()) {
                            this.consent.asking = Some(ask);
                            // The held run's row reads the record's asks, which no push carries.
                            this.reload(cx);
                        }
                        if let Some(said) = answer.get("said").and_then(Value::as_str) {
                            this.set_outcome(&piece, Outcome::Said(said.to_owned()));
                        }
                        for step in &window_steps {
                            if let Some(outcome) = this.window_step(step, &rows, cx) {
                                let refused = matches!(outcome, Outcome::Refused(_));
                                this.set_outcome(&piece, outcome);
                                if refused {
                                    break;
                                }
                            }
                        }
                    }
                }
                this.fade(piece.clone(), cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn set_outcome(&mut self, piece: &str, outcome: Outcome) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.outcomes.insert(piece.to_owned(), outcome);
        }
    }

    /// Clears a piece's outcome line after two seconds.
    fn fade(&mut self, piece: String, cx: &mut Context<Self>) {
        let Some(shown) = self.slate().and_then(|s| s.outcomes.get(&piece)).cloned() else { return };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            _ = this.update(cx, |this, cx| {
                if let Load::Drawn(slate) = &mut this.load {
                    if slate.outcomes.get(&piece) != Some(&shown) {
                        return;
                    }
                    slate.outcomes.remove(&piece);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The steps the window runs itself: open a link, copy, fill the composer, open a pane.
    fn window_step(&mut self, step: &Value, rows: &HashMap<String, Value>, cx: &mut Context<Self>) -> Option<Outcome> {
        let slate = self.slate()?;
        let value_of = |v: Option<&Value>| {
            let v = v?;
            model::filled(v, rows).or_else(|| model::filled(v, &slate.resolved))
        };
        match step.get("do").and_then(Value::as_str)? {
            "open" => {
                let href = value_of(step.get("target")).and_then(|v| v.as_str().map(str::to_owned));
                match href {
                    Some(h) if h.starts_with("http://") || h.starts_with("https://") || h.starts_with("mailto:") => {
                        self.open_link(h, cx);
                        None
                    }
                    _ => Some(Outcome::Refused("Only http, https and mailto links open from a slate.".into())),
                }
            }
            "copy" => {
                let text = value_of(step.get("text")).map(|v| match v {
                    Value::String(s) => s,
                    other => other.to_string(),
                });
                cx.write_to_clipboard(ClipboardItem::new_string(text.unwrap_or_default()));
                Some(Outcome::Said("Copied".into()))
            }
            "fill" => {
                let text = step.get("text").and_then(Value::as_str).unwrap_or_default();
                let with: Vec<String> = step
                    .get("with")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|path| format!("> {path}: {}", slate.resolved.get(path).cloned().unwrap_or(Value::Null)))
                    .collect();
                self.filling = Some(if with.is_empty() { text.to_owned() } else { format!("{text}\n\n{}", with.join("\n")) });
                cx.notify();
                None
            }
            "pane" => Some(Outcome::Refused("That pane is not available here.".into())),
            _ => None,
        }
    }

    /// Cancels one of the slate's runs, as the output's Cancel and the menu's Stop runs do.
    pub fn cancel(&mut self, run: String, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let asking = self
            .host
            .run(move |host| async move { call::<protocol::SlatesCancelRequest>(&host, json!({ "threadId": thread, "run": run })).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                if let Err(f) = answer {
                    this.refused = Some(refusal(&f));
                }
                this.reload(cx);
            });
        })
        .detach();
    }

    /// The menu's write: undo or clear, the record read again after, a refusal under the header.
    pub fn menu_write(&mut self, text: &'static str, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        self.refused = None;
        let asking = self
            .host
            .run(move |host| async move { call::<protocol::SlatesWriteRequest>(&host, json!({ "threadId": thread, "text": text })).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                match answer {
                    Err(f) => this.refused = Some(refusal(&f)),
                    Ok(_) => this.reload(cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Copy as text: the sketch the agent reads.
    pub fn copy_sketch(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let asking = self.host.run(move |host| async move {
            call::<protocol::SlatesReadRequest>(&host, json!({ "threadId": thread, "sketch": true, "text": false })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| match answer {
                Ok(read) => cx.write_to_clipboard(ClipboardItem::new_string(read.text)),
                Err(f) => {
                    this.refused = Some(refusal(&f));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Stop runs: cancels every run whose record says it is running.
    pub fn stop_runs(&mut self, cx: &mut Context<Self>) {
        let Some(slate) = self.slate() else { return };
        let running: Vec<String> = slate
            .doc()
            .map(|d| d.runs.keys().filter(|r| slate.record.run_state(r) == Some("running")).cloned().collect())
            .unwrap_or_default();
        for run in running {
            self.cancel(run, cx);
        }
    }

    /// An approval's answer for one ask: once, for the thread, or a refusal; the record read again after.
    pub fn approve(&mut self, key: String, scope: &'static str, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self.host.run(move |host| async move {
            call::<protocol::SlatesApproveRequest>(&host, json!({ "threadId": thread, "key": key, "scope": scope })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }

    pub fn revoke(&mut self, key: String, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let Some(thread) = self.opened.thread_id.clone() else { return Task::ready(Err("No thread".into())) };
        let asking = self
            .host
            .run(move |host| async move { call::<protocol::SlatesRevokeRequest>(&host, json!({ "threadId": thread, "key": key })).await });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }

    /// Forget secrets: each secret written empty, which the host takes as cleared.
    pub fn forget_secrets(&mut self, cx: &mut Context<Self>) -> Task<Result<(), String>> {
        let (Some(thread), Some(slate)) = (self.opened.thread_id.clone(), self.slate()) else { return Task::ready(Ok(())) };
        let secrets = slate.doc().map(Doc::secrets).unwrap_or_default();
        let asking = self.host.run(move |host| async move {
            let values: Map<String, Value> = secrets.into_iter().map(|s| (s, Value::String(String::new()))).collect();
            call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": values })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await.map(|_| ()).map_err(|f| refusal(&f));
            _ = this.update(cx, |this, cx| this.reload(cx));
            answer
        })
    }

    fn menu_acts(&self, cx: &mut Context<Self>) -> Rc<Vec<MenuAct>> {
        let Some(slate) = self.slate() else { return Rc::new(Vec::new()) };
        let this = cx.entity().downgrade();
        let act = |title: String, held: bool, run: fn(&mut SlateScreen, &mut Context<SlateScreen>)| {
            let this = this.clone();
            MenuAct {
                group: 0,
                glyph: None,
                title: title.into(),
                destructive: false,
                held,
                why: None,
                keys: None,
                run: Rc::new(move |_, cx| {
                    _ = this.update(cx, run);
                }),
            }
        };
        let mut acts = vec![act(words::undo(slate.record.version), !slate.record.can_undo, |s, cx| s.menu_write("<undo />", cx))];
        acts.push(act(words::CLEAR.into(), false, |s, cx| s.menu_write("<clear />", cx)));
        let doc = slate.doc();
        if doc.is_some_and(|d| !d.runs.is_empty()) {
            acts.push(act(words::STOP.into(), false, |s, cx| s.stop_runs(cx)));
        }
        if doc.is_some_and(|d| !d.secrets().is_empty()) {
            acts.push(act(words::FORGET.into(), false, |s, cx| {
                s.consent.forgetting = Some(consent::Forget::default());
                cx.notify();
            }));
        }
        acts.push(act(words::APPROVALS.into(), false, |s, cx| {
            s.consent.standing = Some(consent::Standing::default());
            cx.notify();
        }));
        acts.push(act(words::COPY.into(), false, |s, cx| s.copy_sketch(cx)));
        Rc::new(acts)
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let slate = self.slate().expect("a drawn slate");
        let title = slate.doc().and_then(|d| d.title.clone()).unwrap_or_else(|| words::TITLE.to_owned());
        let acts = self.menu_acts(cx);
        let refreshing = consent::refreshing_under_root(slate);
        v_flex()
            .child(
                h_flex()
                    .mt(px(12.))
                    .when(self.refused.is_none(), |el| el.mb(px(16.)))
                    .h(px(28.))
                    .flex_none()
                    .gap(px(8.))
                    .debug_selector(|| "slate-header".into())
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(p.muted_foreground)
                            .child(SharedString::from(title)),
                    )
                    .when(refreshing, |el| el.child(crate::ui::spinner(cx)))
                    .child(div().debug_selector(|| "slate-menu".into()).child(icon_menu("slate-menu", words::MENU, acts))),
            )
            .children(self.refused.clone().map(|r| {
                div()
                    .mb(px(16.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(p.error_foreground)
                    .debug_selector(|| "slate-refused".into())
                    .child(r)
            }))
            .into_any_element()
    }

    fn empty(&self, record: Option<&Record>, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let Some(record) = record else {
            return empty_frame(div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(words::NO_THREAD), cx);
        };
        let first = match (record.newer, record.empty.as_deref()) {
            (Some(schema), _) => Some(words::newer(schema)),
            (None, Some("cleared")) => Some(words::CLEARED.to_owned()),
            (None, Some("rewound-before")) => Some(words::REWOUND.to_owned()),
            _ => None,
        };
        let mut lead = String::new();
        if let Some(first) = first {
            lead.push_str(&first);
            lead.push_str(". ");
        }
        if record.newer.is_none() {
            lead.push_str(words::ASK);
        }
        let mark = record.empty.clone().unwrap_or_else(|| if record.newer.is_some() { "newer".into() } else { "none".into() });
        let this = cx.entity().downgrade();
        let thread = record.thread_id.clone();
        let ask_for_one =
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| fill_composer(&thread, words::ASK_PROMPT, window, cx);
        let undo = record.can_undo.then(|| {
            let this = this.clone();
            div().debug_selector(|| "slate-empty-undo".into()).child(Button::ghost("slate-empty-undo", "Undo").xs().on_click(
                move |_, _, cx| {
                    _ = this.update(cx, |s, cx| {
                        s.empty_refused = None;
                        s.menu_write("<undo />", cx);
                    });
                },
            ))
        });
        let body = v_flex()
            .items_center()
            .gap(px(24.))
            .debug_selector(move || format!("slate-empty-{mark}"))
            .child(
                v_flex()
                    .max_w(px(384.))
                    .items_center()
                    .text_center()
                    .child(div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(words::NOTHING))
                    .child(div().mt(px(4.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(lead)),
            )
            .when(record.newer.is_none(), |el| {
                el.child(
                    h_flex().gap(px(8.)).child(Button::outline("slate-ask", words::ASK_BUTTON).xs().on_click(ask_for_one)).children(undo),
                )
            })
            .children(
                self.refused
                    .clone()
                    .or(self.empty_refused.clone())
                    .map(|r| div().text_size(px(13.)).line_height(px(20.)).text_color(p.error_foreground).child(r)),
            );
        empty_frame(body, cx)
    }
}

thread_local! {
    static REQUESTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static WINDOW: String = format!("{:x}", std::process::id() as u64 ^ rand_seed());
}

fn rand_seed() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64)
}

/// This window's own part of every request id: the host keeps ids from every window for minutes.
fn window_id() -> String {
    WINDOW.with(Clone::clone)
}

/// A run that starts again begins its streamed lines afresh; a refresh keeps the last lines until its own arrive.
fn restarted(slate: &mut Slate, path: &str, value: &Value) {
    let Some(name) = path.strip_prefix('$') else { return };
    if slate.doc().is_none_or(|d| !d.runs.contains_key(name)) {
        return;
    }
    if value.get("state").and_then(Value::as_str) != Some("running") {
        // A refresh that streamed nothing ends on its record's own lines.
        if slate.fresh.remove(name) {
            slate.lines.remove(name);
        }
        return;
    }
    if slate.record.run_state(name) == Some("running") {
        return;
    }
    if value.get("refreshing").and_then(Value::as_bool) == Some(true) {
        slate.fresh.insert(name.to_owned());
    } else {
        slate.lines.remove(name);
    }
}

/// Puts words in the thread's composer: the open one, else its kept draft, which it reads as it opens the thread.
fn fill_composer(thread: &str, text: &str, window: &mut Window, cx: &mut App) {
    if !crate::screens::composer::fill(thread, text, window, cx) && cx.has_global::<crate::local_store::LocalStore>() {
        _ = cx.update_global(|s: &mut crate::local_store::LocalStore, _| {
            crate::screens::composer::store::set_draft(s, thread, text, text.len())
        });
    }
}

/// The panel's own empty state, centred in the tab: `Empty` with its 24 px gap and `md:p-12`.
fn empty_frame(body: impl IntoElement, _cx: &App) -> AnyElement {
    v_flex().size_full().flex_1().items_center().justify_center().p(px(48.)).child(body).into_any_element()
}

impl Render for SlateScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let back = !self.on_screen();
        self.drawn_at = Some(Instant::now());
        if let (Some(text), Some(thread)) = (self.filling.take(), self.opened.thread_id.clone()) {
            fill_composer(&thread, &text, window, cx);
        }
        if back {
            self.sync_hold(cx);
            if self.clocked {
                self.resolve(cx);
            }
        }
        let surface = match &self.load {
            Load::NoThread => self.empty(None, cx),
            Load::Waiting => div().into_any_element(),
            Load::Empty(record) => {
                let record = record.clone();
                self.empty(Some(&record), cx)
            }
            Load::Drawn(_) => {
                let header = self.header(cx);
                let held = consent::held_rows(self, window, cx);
                let screen = cx.entity().downgrade();
                let slate = self.slate().expect("drawn");
                let body = slate.doc().and_then(|d| pieces::draw(slate, &d.root, &screen, window, cx));
                v_flex()
                    .size_full()
                    .min_h_0()
                    .debug_selector(|| "slate-surface".into())
                    .child(column().child(header))
                    .children(held)
                    .child(div().id("slate-scroll").flex_1().min_h_0().overflow_y_scroll().child(column().pb(px(32.)).children(body)))
                    .into_any_element()
            }
        };
        let overlays = consent::overlays(self, window, cx);
        div().size_full().flex().flex_col().child(surface).children(overlays)
    }
}

/// The panel's one column, 16 px in, capped at the settings pages' 760 px.
pub fn column() -> gpui_kit::Div {
    v_flex().w_full().max_w(px(760.)).mx_auto().px(px(16.))
}
