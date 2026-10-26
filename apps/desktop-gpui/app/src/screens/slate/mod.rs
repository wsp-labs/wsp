// SPDX-License-Identifier: AGPL-3.0-only
//! The right panel's Slate tab, after `apps/web/src/slate/SlateSurface.tsx`: the open thread's slate, read with
//! `slates.get`, kept live by the host's `session.slate`, `slate.values` and `slate.run` pushes, and drawn piece by
//! piece. Every formula a drawn piece reads is answered by the host's `slates.resolve`, the one evaluator: asked again when a
//! value moves, and each second only while a drawn formula reads the clock. Presses, writes, approvals and the menu's
//! acts go through the host's own ops.

mod acts;
pub mod consent;
pub mod frame_check;
pub mod model;
pub mod pieces;
mod view;

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::{prelude::*, App, Context, Entity, Subscription, Task, Window};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use wsp_host_client::{protocol, Event, Failure, HostClient, Op};

use crate::screens::{Host, Opened, Pane, Parts, Place, Screen};
use model::{Doc, Piece, Record, Row, Rows};
pub use view::column;

/// The panel's words, `SLATE_WORDS` in `SlateSurface.tsx`.
pub mod words {
    pub const NO_THREAD: &str = "Pick a thread to see its slate.";
    pub const NOTHING: &str = "Nothing here yet";
    pub const ASK: &str = "The agent builds this panel when there is something to watch, track, fill in or press. Ask for one.";
    pub const ASK_BUTTON: &str = "Ask for one";
    pub const ASK_HELD: &str = "Answer the prompt in the thread first.";
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
    pub const NO_PANE: &str = "That pane is not available here.";
    pub const OFFLINE: &str = "Not connected to wsp. The slate will catch up when it is.";
    pub const TITLE: &str = "Slate";
}

/// The hold a drawn slate keeps on the host while the tab shows it (`SHOWN_HOLD`).
pub const SHOWN_HOLD: &str = "slate";
/// The key context the slate draws in, which tells its own fields from the rest of the window's.
pub const KEY_CONTEXT: &str = "Slate";
/// The most formulas one `slates.resolve` takes (`SlatesResolveParams`).
const RESOLVE_MAX: usize = 200;
/// The least time between two asks, so a slate redraws at most ten times a second (`MIN_FLUSH_GAP_MS` in `engine.ts`).
const ASK_GAP: Duration = Duration::from_millis(100);
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
    /// The paths of `mine` a field or a pick queued for the host and whose write has not landed; only these go as the
    /// panel goes, never a value kept drawn while the person chooses between theirs and the host's.
    pub queued: HashSet<String>,
    /// Pieces whose press is out, which draw busy until it is answered.
    pub busy: HashSet<String>,
    /// What each piece's last press said or why it was refused, shown under it for two seconds.
    pub outcomes: HashMap<String, Outcome>,
    /// Runs refreshing whose streamed lines are still the last result's, until their own first line arrives.
    pub fresh: HashSet<String>,
    /// Sections folded or opened where no state path keeps it, by piece id, for the window's life.
    pub folds: HashMap<String, bool>,
    /// Each bar switch's pick, by the first list it switches, for the window's life.
    pub picks: HashMap<String, String>,
}

impl Slate {
    pub fn new(record: Record) -> Slate {
        Slate {
            record,
            resolved: HashMap::new(),
            lines: HashMap::new(),
            mine: HashMap::new(),
            queued: HashSet::new(),
            busy: HashSet::new(),
            outcomes: HashMap::new(),
            fresh: HashSet::new(),
            folds: HashMap::new(),
            picks: HashMap::new(),
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

    /// What the person typed or picked at `path`, drawn at once and sent to the host by a write or as the panel goes.
    pub fn queue(&mut self, path: String, value: Value) {
        self.queued.insert(path.clone());
        self.mine.insert(path, value);
    }

    /// The value at an own path as this window draws it: what the person typed over the host's copy.
    pub fn value(&self, path: &str) -> Option<&Value> {
        self.mine.get(path).or_else(|| self.record.get_own(path))
    }

    /// Every formula the shown pieces read: each `when`, each live prop, and each repeating piece's rows.
    pub fn formulas(&self) -> Vec<String> {
        self.formula_groups().into_iter().flatten().collect()
    }

    /// [`Self::formulas`] one group per piece, each formula once. A piece whose `when` has not answered truthy asks only
    /// its `when`, as the web draws nothing of it, and a folded section asks nothing for its body.
    pub fn formula_groups(&self) -> Vec<Vec<String>> {
        let Some(doc) = self.doc() else { return Vec::new() };
        let mut groups = Vec::new();
        let mut seen = HashSet::new();
        let mut walked = HashSet::new();
        let mut stack = vec![doc.root.as_str()];
        while let Some(id) = stack.pop() {
            if !walked.insert(id) {
                continue;
            }
            let Some(piece) = doc.piece(id) else { continue };
            let mut out = Vec::new();
            let mut add = |e: String| {
                if seen.insert(e.clone()) {
                    out.push(e);
                }
            };
            if let Some(when) = &piece.when {
                add(format!("({when})"));
                if !truthy(self.resolved.get(&format!("({when})"))) {
                    groups.push(out);
                    continue;
                }
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
            groups.push(out);
            if self.folded(piece) {
                continue;
            }
            if let model::Fallback::Piece(f) = &piece.fallback {
                stack.push(f);
            }
            stack.extend(piece.children.iter().rev().map(String::as_str));
        }
        groups
    }

    /// A section folded shut, as `section` in `pieces/layout.rs` decides it: by its state path, else the window's own fold.
    fn folded(&self, piece: &Piece) -> bool {
        let raw = piece.prop("open");
        let resolved = |v: Option<&Value>| v.and_then(|v| model::filled(v, &self.resolved)).filter(|v| !v.is_null());
        if piece.kind != "section" || (raw.is_none() && resolved(piece.prop("collapsible")) != Some(Value::Bool(true))) {
            return false;
        }
        let asked = resolved(raw).is_none_or(|v| truthy(Some(&v)));
        let open = if pieces::layout::two_way(raw).is_some() { asked } else { self.folds.get(&piece.id).copied().unwrap_or(asked) };
        !open
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
    /// How often the clock asks again: each second while a drawn formula reads the time, each 30 s while it reads only
    /// the day or the zone.
    tick: Option<Duration>,
    /// When the last ask went, which the next waits a tenth of a second after (`MIN_FLUSH_GAP_MS`).
    asked_at: Option<Instant>,
    spacing: Option<Task<()>>,
    /// The sources outside the slate a drawn formula reads, whose pushes ask again.
    sources: Vec<&'static str>,
    /// The one `slates.resolve` in flight; an ask meanwhile marks `again`, and is made once it lands.
    resolving: Option<Task<()>>,
    again: bool,
    /// The one `slates.get` in flight; an ask meanwhile marks `get_again`, read once it lands.
    getting: Option<Task<()>>,
    get_again: bool,
    /// The record reads this panel made, for its tests.
    pub gets: u64,
    /// Each path's latest write, so an earlier write's answer does not undo a later one drawn meanwhile.
    writes: HashMap<String, u64>,
    /// The token the window's frame keeps while it draws the slate, and the watch on its release: a tab behind another
    /// or a shut panel keeps its screen but draws nothing, and the first frame without it drops the token.
    drawn: Option<(gpui_kit::EntityId, Subscription)>,
    /// The holds this panel wants on the host for its thread: the shown hold and each held source a drawn formula reads.
    held: HashSet<&'static str>,
    /// The holds the host answered taking and has not let go of; a hold dies with the socket that took it.
    on_host: HashSet<&'static str>,
    /// Each source whose hold may differ from the one wanted, for the one task that takes and lets go of holds in order.
    holding_asks: mpsc::UnboundedSender<&'static str>,
    /// The hold call out now, its source and whether it takes: the host acts on it even when the panel goes first.
    asking: Option<(&'static str, bool)>,
    /// Whether the host's socket is up, as the follow loop last found it.
    live: bool,
    /// Words a press's fill puts in the composer on the next draw, which has the window the composer needs.
    filling: Option<String>,
    scroll: gpui_kit::ScrollHandle,
    _follow: Option<Task<()>>,
    _pump: Option<Task<()>>,
    _tick: Option<Task<()>>,
    _holds: Option<Task<()>>,
    _release: Option<Subscription>,
}

impl Screen for SlateScreen {
    const NAME: &'static str = "slate";
    const PLACE: Place = Place::Panel(Pane::Slate);

    fn new(host: Host, opened: Opened, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (holding_asks, mut asks) = mpsc::unbounded::<&'static str>();
        // A prompt that docks in the thread's composer, or leaves it, turns Ask for one off or on again.
        cx.observe_global::<crate::screens::composer::Docked>(|_, cx| cx.notify()).detach();
        let mut screen = SlateScreen {
            host,
            load: if opened.thread_id.is_some() { Load::Waiting } else { Load::NoThread },
            opened,
            refused: None,
            empty_refused: None,
            consent: consent::Consent::default(),
            timings: Vec::new(),
            tick: None,
            asked_at: None,
            spacing: None,
            sources: Vec::new(),
            resolving: None,
            again: false,
            getting: None,
            get_again: false,
            gets: 0,
            writes: HashMap::new(),
            drawn: None,
            held: HashSet::new(),
            on_host: HashSet::new(),
            holding_asks,
            asking: None,
            live: true,
            filling: None,
            scroll: gpui_kit::ScrollHandle::default(),
            _follow: None,
            _pump: None,
            _tick: None,
            _holds: None,
            _release: None,
        };
        // One call at a time, each for what is wanted when its turn comes, so a take and a later drop reach the host in
        // order and a call that fails is made again by the next sync rather than counted as done.
        screen._holds = Some(cx.spawn(async move |this, cx| {
            while let Some(source) = asks.next().await {
                let Ok(Some((take, host, thread))) = this.update(cx, |this, _| {
                    let take = this.held.contains(source);
                    let thread = this.opened.thread_id.clone()?;
                    let out = (take != this.on_host.contains(source)).then(|| (take, this.host.clone(), thread));
                    this.asking = out.as_ref().map(|_| (source, take));
                    out
                }) else {
                    continue;
                };
                let asked = host
                    .run(move |host| async move {
                        let params = json!({ "threadId": thread, "sources": [source] });
                        if take {
                            call::<protocol::SlatesSubscribeRequest>(&host, params).await.map(|_| ())
                        } else {
                            call::<protocol::SlatesUnsubscribeRequest>(&host, params).await.map(|_| ())
                        }
                    })
                    .await;
                let landed = this.update(cx, |this, _| {
                    this.asking = None;
                    if asked.is_ok() && this.live {
                        if take {
                            this.on_host.insert(source);
                        } else {
                            this.on_host.remove(source);
                        }
                    }
                });
                if landed.is_err() {
                    break;
                }
            }
        }));
        screen.consent.seen = consent::shown_on(screen.opened.thread_id.as_deref(), cx);
        screen.follow(cx);
        screen._release = Some(cx.on_release(|this, cx| {
            let Some(thread) = this.opened.thread_id.clone() else { return };
            this.held.clear();
            let mut held = std::mem::take(&mut this.on_host);
            match this.asking.take() {
                Some((source, true)) => held.insert(source),
                Some((source, false)) => held.remove(source),
                None => false,
            };
            // What the person typed and the host has not taken goes now, as the web's sender sends it after a switch.
            let unsent: Map<String, Value> = match &this.load {
                Load::Drawn(slate) => slate
                    .mine
                    .iter()
                    .filter(|(path, mine)| {
                        slate.queued.contains(*path)
                            && !slate.doc().is_some_and(|d| d.is_secret(path.trim_start_matches('$')))
                            && slate.record.get_own(path) != Some(*mine)
                    })
                    .map(|(path, mine)| (path.clone(), mine.clone()))
                    .collect(),
                _ => Map::new(),
            };
            if let Load::Drawn(slate) = &mut this.load {
                keep(&thread, slate, cx);
            }
            let letting_go = this.host.run(move |host| async move {
                if !unsent.is_empty() {
                    let _ = call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": unsent })).await;
                }
                for source in held {
                    let _ = call::<protocol::SlatesUnsubscribeRequest>(&host, json!({ "threadId": thread, "sources": [source] })).await;
                }
                Ok(())
            });
            cx.background_spawn(async move {
                let _: Result<(), Failure> = letting_go.await;
            })
            .detach();
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
    /// The document moved, or the socket is new: the record is read again.
    Reload,
    Values {
        revision: u64,
        values: Map<String, Value>,
    },
    Lines {
        run: String,
        lines: Vec<String>,
    },
    /// The socket went, or came back.
    Live(bool),
    /// A push moved what these sources read, for a workspace where it names one.
    Moved {
        sources: Vec<&'static str>,
        workspace: Option<String>,
    },
}

/// The sources a host push moves for a slate on `thread`, as the web's store folds each push in (`useStore.ts`): a
/// workspace's status its git and pull request, its cost, the account's usage, and the thread's row, its turn's end and
/// its `thread.*` events the thread; a streaming reply's deltas move no field the thread source reads.
fn moved_by(frame: &Value, thread: &str) -> Option<Change> {
    let kind = frame.get("type").and_then(Value::as_str).unwrap_or_default();
    let workspace = |w: Option<&Value>| w.and_then(Value::as_str).map(str::to_owned);
    let (sources, workspace) = match kind {
        "workspace.status" => (vec!["git", "pr"], workspace(frame.get("status").and_then(|s| s.get("id")))),
        "workspace.cost" => (vec!["cost"], workspace(frame.get("workspaceId"))),
        "usage.account" => (vec!["usage"], None),
        "session.slate" | "slate.values" | "slate.run" => return None,
        _ if (matches!(kind, "session.row" | "session.done") || kind.starts_with("thread."))
            && frame.get("threadId").and_then(Value::as_str) == Some(thread) =>
        {
            (vec!["thread"], None)
        }
        _ => return None,
    };
    if kind.starts_with("workspace.") && workspace.is_none() {
        return None;
    }
    Some(Change::Moved { sources, workspace })
}

/// The host's refusal as the person reads it: its own words, which lead with its code and name; with no host to answer,
/// the web's offline sentence, never the host's socket or state path.
pub fn refusal(failure: &Failure) -> String {
    if offline(failure) {
        words::OFFLINE.to_owned()
    } else {
        failure.message.clone()
    }
}

/// wsp-mcp's words for a host that was not there to answer, by their keys in its `record/words.json`.
pub(crate) const GONE: [(&str, &str); 9] = [
    ("noHostServing", "no wsp host is serving {state}; start one with wsp up --state {state}"),
    ("hostTokenMissing", "the host's token file is missing: {path}"),
    ("noAnswer", "the host at {where} did not answer: {why}"),
    ("noAnswerWithin", "the host at {where} did not answer: nothing came back within {ms} ms"),
    ("hostClosed", "the host closed the connection"),
    ("hostStopping", "the host is restarting; the turn goes on and its reply lands in the thread"),
    ("notDelivered", "the host stopped before it took the message, so it was not delivered; send it again"),
    ("noHostAnswered", "no host answered for {state} within 20s"),
    ("hostExited", "the host for {state} exited with {ended} before it served; its log is {log}"),
];

/// Whether a failure says no host was there to answer: a dial that reached nobody, or a socket that went, as wsp-mcp's
/// record words those.
pub(crate) fn offline(failure: &Failure) -> bool {
    failure.kind.as_deref() == Some("unreachable") || GONE.iter().any(|(_, template)| said_in(&failure.message, &pieces(template)))
}

/// A template's words between its `{..}` holes: `a {x} b` as `["a ", " b"]`.
fn pieces(template: &str) -> Vec<&str> {
    template.split('{').enumerate().map(|(i, p)| if i == 0 { p } else { p.split_once('}').map_or("", |(_, rest)| rest) }).collect()
}

/// Whether `message` is a template's words in order with anything between them.
fn said_in(message: &str, words: &[&str]) -> bool {
    let Some((first, after)) = words.split_first() else { return false };
    let Some(mut rest) = message.strip_prefix(first) else { return false };
    let Some((last, between)) = after.split_last() else { return rest.is_empty() };
    for part in between {
        match rest.find(part) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last)
}

/// Opens the right panel on `pane`'s tab where this build mounts a screen for it; false where it mounts none.
pub fn open_pane(pane: Pane, cx: &mut App) -> bool {
    if !crate::screens::screen(pane.name()).is_some_and(|e| e.place == Place::Panel(pane)) {
        return false;
    }
    crate::screens::navigate(crate::screens::Route::Screen(pane.name().into()), cx);
    true
}

/// Opens the open thread's Slate tab, as the transcript's "Updated the slate" line does.
pub fn open_tab(cx: &mut App) {
    open_pane(Pane::Slate, cx);
}

/// Whether `thread` has a slate no window has shown yet, which the panel opens its tab on once (`showOnce` in
/// `store.ts`): a document this build draws, never shown, and not opened by this window already.
pub fn never_shown(host: &Host, thread: String) -> impl std::future::Future<Output = bool> + 'static {
    let asking = host.run(move |host| async move {
        let got = call::<protocol::SlatesGetRequest>(&host, json!({ "threadId": thread })).await?;
        Ok(serde_json::to_value(&got.slate).ok().filter(|v| !v.is_null()).and_then(|v| Record::read(&v)))
    });
    async move { matches!(asking.await, Ok(Some(record)) if record.doc.is_some() && record.newer.is_none() && !record.shown_once) }
}

/// Notes that this window opened `thread`'s tab for its first showing; false where it already did, since an answer
/// read before the host stored the showing must not open the tab a second time.
pub fn first_showing(thread: &str, cx: &mut App) -> bool {
    let shown = &mut cx.default_global::<ShownHere>().0;
    if shown.iter().any(|t| t == thread) {
        return false;
    }
    shown.push(thread.to_owned());
    let over = shown.len().saturating_sub(KEPT_THREADS);
    shown.drain(..over);
    true
}

/// The threads this window opened the Slate tab on for their first showing, the last hundred, oldest first.
#[derive(Default)]
struct ShownHere(Vec<String>);

impl gpui_kit::Global for ShownHere {}

/// Each thread the agent writes a slate on, off the host's pushes, through every reconnect; the follow lives as long as
/// the future, which need not be polled.
pub fn agent_writes(host: &Host) -> (impl std::future::Future<Output = Result<(), Failure>> + 'static, mpsc::UnboundedReceiver<String>) {
    let (tx, rx) = mpsc::unbounded::<String>();
    let follow = host.run(move |host| async move {
        loop {
            if let Ok(mut events) = host.events().await {
                while let Some(event) = events.next().await {
                    let Ok(Event::Host(event)) = event else { continue };
                    let Ok(frame) = serde_json::to_value(&*event) else { continue };
                    if frame.get("type").and_then(Value::as_str) != Some("session.slate")
                        || frame.get("by").and_then(Value::as_str) != Some("agent")
                    {
                        continue;
                    }
                    if let Some(thread) = frame.get("threadId").and_then(Value::as_str) {
                        tx.unbounded_send(thread.to_owned()).map_err(|_| Failure::new("gone"))?;
                    }
                }
            }
            if tx.is_closed() {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    (follow, rx)
}

/// Requests packed into batches of at most `max` formulas, each piece's group whole where it fits in one, so a list and
/// the fields read off it are answered off the same values.
fn batched(groups: Vec<Vec<String>>, max: usize) -> Vec<Vec<String>> {
    let mut out: Vec<Vec<String>> = Vec::new();
    for group in groups {
        for part in group.chunks(max) {
            match out.last_mut() {
                Some(last) if last.len() + part.len() <= max => last.extend_from_slice(part),
                _ => out.push(part.to_vec()),
            }
        }
    }
    out
}

/// What a thread's slate keeps in this window past the panel a thread switch drops, as `slateBundle` keeps a thread's
/// engine for the window's life: its folds and picks, and what the person typed that the host has not taken.
struct Kept {
    folds: HashMap<String, bool>,
    picks: HashMap<String, String>,
    /// Each queued value with the host's value at its path as the panel went, which it is dropped for once moved.
    mine: HashMap<String, (Value, Option<Value>)>,
}

/// The kept slates, newest last.
#[derive(Default)]
struct KeptSlates(Vec<(String, Kept)>);

impl gpui_kit::Global for KeptSlates {}

/// How many threads keep their slate's folds, picks and typed values; the oldest goes past it, as the panel keeps the
/// tabs of a hundred threads.
pub const KEPT_THREADS: usize = 100;

/// Keeps `slate`'s window state for `thread` as its panel goes.
fn keep(thread: &str, slate: &mut Slate, cx: &mut App) {
    let queued = std::mem::take(&mut slate.queued);
    let mine = std::mem::take(&mut slate.mine)
        .into_iter()
        .filter(|(path, _)| queued.contains(path))
        .map(|(path, mine)| {
            let seen = slate.record.get_own(&path).cloned();
            (path, (mine, seen))
        })
        .collect();
    let kept = Kept { folds: std::mem::take(&mut slate.folds), picks: std::mem::take(&mut slate.picks), mine };
    let all = &mut cx.default_global::<KeptSlates>().0;
    all.retain(|(t, _)| t != thread);
    if !kept.folds.is_empty() || !kept.picks.is_empty() || !kept.mine.is_empty() {
        all.push((thread.to_owned(), kept));
    }
    let over = all.len().saturating_sub(KEPT_THREADS);
    all.drain(..over);
}

/// Gives a slate drawn again on `thread` what its last panel kept.
fn kept(thread: &str, slate: &mut Slate, cx: &mut App) {
    let Some(all) = cx.try_global::<KeptSlates>() else { return };
    let Some(at) = all.0.iter().position(|(t, _)| t == thread) else { return };
    let (_, kept) = cx.default_global::<KeptSlates>().0.remove(at);
    slate.folds = kept.folds;
    slate.picks = kept.picks;
    slate.mine = kept
        .mine
        .into_iter()
        .filter(|(path, (mine, seen))| {
            let now = slate.record.get_own(path);
            now == seen.as_ref() && now != Some(mine)
        })
        .map(|(path, (mine, _))| (path, mine))
        .collect();
}

/// The threads [`KeptSlates`] holds now, oldest first.
pub fn kept_threads(cx: &App) -> Vec<String> {
    cx.try_global::<KeptSlates>().map(|k| k.0.iter().map(|(t, _)| t.clone()).collect()).unwrap_or_default()
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

    /// Whether the host answered taking the panel's shown hold for its thread.
    pub fn holding(&self) -> bool {
        self.on_host.contains(SHOWN_HOLD)
    }

    /// Every hold the host answered taking for the panel and still has, the shown hold among them.
    pub fn holds(&self) -> Vec<&'static str> {
        let mut held: Vec<&'static str> = self.on_host.iter().copied().collect();
        held.sort_unstable();
        held
    }

    /// Whether the host's socket is up as the panel last found it.
    pub fn live(&self) -> bool {
        self.live
    }

    /// Listens for the thread's pushes before the first read, so nothing pushed between the two is lost, and reads
    /// the record again on every push that moves the document; a socket that comes back is read again the same way.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        let (tx, mut rx) = mpsc::unbounded::<Change>();
        let follow = self.host.run(move |host| async move {
            let mut live = true;
            loop {
                let ended: Result<(), Failure> = async {
                    let mut events = host.events().await?;
                    if !live {
                        live = true;
                        tx.unbounded_send(Change::Live(true)).map_err(|_| Failure::new("gone"))?;
                    }
                    tx.unbounded_send(Change::Reload).map_err(|_| Failure::new("gone"))?;
                    while let Some(event) = events.next().await {
                        let Ok(Event::Host(event)) = event else { continue };
                        let Ok(frame) = serde_json::to_value(&*event) else { continue };
                        if let Some(moved) = moved_by(&frame, &thread) {
                            tx.unbounded_send(moved).map_err(|_| Failure::new("gone"))?;
                        }
                        if frame.get("threadId").and_then(Value::as_str) != Some(thread.as_str()) {
                            continue;
                        }
                        let change = match frame.get("type").and_then(Value::as_str) {
                            Some("session.slate") => Change::Reload,
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
                if live {
                    live = false;
                    if tx.unbounded_send(Change::Live(false)).is_err() {
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
            Change::Reload => self.reload(cx),
            Change::Values { revision, values } => {
                let Load::Drawn(slate) = &mut self.load else { return };
                // A run newly held has an ask the record does not carry yet, and an empty push says the asks changed.
                let asked = values.is_empty()
                    || values.iter().any(|(path, value)| {
                        path.strip_prefix('$').is_some_and(|run| {
                            slate.doc().is_some_and(|d| d.runs.contains_key(run))
                                && value.get("state").and_then(Value::as_str) == Some("held")
                                && !slate.record.asks.iter().any(|a| a.run == run)
                        })
                    });
                if revision > slate.record.revision {
                    slate.record.revision = revision;
                    for (path, value) in values {
                        restarted(slate, &path, &value);
                        slate.record.set_own(&path, value);
                    }
                    self.resolve(cx);
                }
                if asked {
                    self.reload(cx);
                }
            }
            Change::Live(live) => {
                self.live = live;
                if !live {
                    // The host let go of every hold this socket took as it closed.
                    self.held.clear();
                    self.on_host.clear();
                    self.resolving = None;
                    self.again = false;
                }
                self.sync_hold();
                cx.notify();
            }
            Change::Moved { sources, workspace } => {
                let here = self.opened.workspace_id.as_deref().or(self.record().and_then(|r| r.workspace_id.as_deref()));
                if self.on_screen()
                    && workspace.is_none_or(|w| here == Some(w.as_str()))
                    && sources.iter().any(|m| self.sources.contains(m))
                {
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
        let mut record = record.unwrap_or_else(|| Record { thread_id: thread.clone(), ..Record::default() });
        if let Load::Drawn(slate) = &self.load {
            // An older read keeps the values pushed since, as the web's `setRecord` does; older in version too, it is dropped.
            if record.revision < slate.record.revision {
                if record.version < slate.record.version {
                    return;
                }
                record.revision = slate.record.revision;
                record.values.extend(slate.record.values.iter().map(|(k, v)| (k.clone(), v.clone())));
            }
        }
        let first = !matches!(self.load, Load::Drawn(_));
        if record.doc.is_none() {
            self.load = Load::Empty(Box::new(record));
            self._tick = None;
            self.tick = None;
            self.sources.clear();
            self.resolving = None;
            self.again = false;
            self.sync_hold();
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
            _ => {
                let mut slate = Slate::new(record);
                kept(&thread, &mut slate, cx);
                Load::Drawn(Box::new(slate))
            }
        };
        if first {
            self.sync_hold();
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
        self.drawn.is_some()
    }

    /// Keeps a token in the frame the slate draws in; the first frame that leaves the slate out drops it, and the slate
    /// lets its holds go and its clock stops asking.
    fn mark_drawn(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let token = window.use_keyed_state("slate-drawn", cx, |_, _| ());
        let id = token.entity_id();
        if self.drawn.as_ref().is_some_and(|(held, _)| *held == id) {
            return;
        }
        let gone = cx.observe_release(&token, move |this, _, _| {
            if this.drawn.as_ref().is_some_and(|(held, _)| *held == id) {
                this.drawn = None;
                this.sync_hold();
            }
        });
        self.drawn = Some((id, gone));
    }

    /// Takes the host's holds while a drawn slate is on screen and the socket is up: the shown hold, which keeps its
    /// `every` runs ticking, and each held source a drawn formula reads. Lets them go once the slate is gone or off
    /// screen, as the web's tab does when it unmounts; a socket that comes back is asked for each again.
    fn sync_hold(&mut self) {
        let want: HashSet<&'static str> = if matches!(self.load, Load::Drawn(_)) && self.on_screen() && self.live {
            std::iter::once(SHOWN_HOLD).chain(self.sources.iter().copied().filter(|s| model::HELD_SOURCES.contains(s))).collect()
        } else {
            HashSet::new()
        };
        let moved: Vec<&'static str> = want.symmetric_difference(&self.on_host).copied().collect();
        self.held = want;
        for source in moved {
            let _ = self.holding_asks.unbounded_send(source);
        }
    }

    /// Asks the host for every formula the shown pieces read, in batches of up to 200 that keep each piece's formulas
    /// together, and draws the answers. One ask is in flight at a time: an ask meanwhile is made once it lands, as is
    /// one for formulas the answers newly show.
    pub fn resolve(&mut self, cx: &mut Context<Self>) {
        let (Some(thread), Load::Drawn(slate)) = (self.opened.thread_id.clone(), &self.load) else { return };
        if self.resolving.is_some() {
            self.again = true;
            return;
        }
        if let Some(wait) = self.asked_at.map(|at| ASK_GAP.saturating_sub(at.elapsed())).filter(|wait| !wait.is_zero()) {
            if self.spacing.is_none() {
                let waiting = self.host.run(move |_| async move {
                    tokio::time::sleep(wait).await;
                    Ok(())
                });
                self.spacing = Some(cx.spawn(async move |this, cx| {
                    let _ = waiting.await;
                    _ = this.update(cx, |this, cx| {
                        this.spacing = None;
                        this.resolve(cx);
                    });
                }));
            }
            return;
        }
        let groups = slate.formula_groups();
        let formulas: Vec<String> = groups.iter().flatten().cloned().collect();
        let none = Map::new();
        let derived = slate.doc().map_or(&none, |d| &d.derived);
        let tick = model::tick(&formulas, derived);
        let sources = model::sources(&formulas, derived);
        if sources != self.sources {
            self.sources = sources;
            self.sync_hold();
        }
        if tick != self.tick {
            self.tick = tick;
            self._tick = tick.map(|every| {
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor().timer(every).await;
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
        self.asked_at = Some(started);
        let batches = batched(groups, RESOLVE_MAX);
        let asking = self.host.run(move |host| async move {
            let batches = batches.into_iter().map(|batch| {
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
                let mut more = std::mem::take(&mut this.again);
                if let (Ok(values), Load::Drawn(slate)) = (answer, &mut this.load) {
                    let mut resolved: HashMap<String, Value> = values.into_iter().collect();
                    // A formula the host left out reads null, so it is not asked again for nothing.
                    for formula in formulas {
                        resolved.entry(formula).or_insert(Value::Null);
                    }
                    // As the web redraws only the pieces whose reads moved: an answer that moved nothing draws nothing.
                    if slate.resolved != resolved {
                        slate.resolved = resolved;
                        cx.notify_in_place();
                    }
                    more |= slate.formulas().iter().any(|f| !slate.resolved.contains_key(f));
                }
                if more {
                    this.resolve(cx);
                }
            });
        }));
    }

    /// Reads the record again, as after an act the host does not push for this window: one read in flight, and one
    /// more once it lands for every ask meanwhile, which may have come after the host wrote what the first read missed.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        if self.getting.is_some() {
            self.get_again = true;
            return;
        }
        let asking =
            self.host.run(move |host| async move { call::<protocol::SlatesGetRequest>(&host, json!({ "threadId": thread })).await });
        self.getting = Some(cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                this.getting = None;
                this.gets += 1;
                if let Ok(got) = answer {
                    let slate = serde_json::to_value(&got.slate).ok().filter(|v| !v.is_null());
                    this.take_record(slate.and_then(|s| Record::read(&s)), cx);
                }
                if std::mem::take(&mut this.get_again) {
                    this.reload(cx);
                }
            });
        }));
    }
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

#[cfg(test)]
mod tests;
