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
use crate::ui::{icon_menu, scroll_area, ActivePalette, Button, MenuAct};
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
    /// When the panel last drew the slate: a tab behind another or a shut panel keeps its screen but draws nothing.
    drawn_at: Option<Instant>,
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
    _watch: Option<Task<()>>,
    _holds: Option<Task<()>>,
    _release: Option<Subscription>,
}

/// How long after its last draw the slate counts as off screen: its hold goes and its clock stops asking.
const OFF_SCREEN: Duration = Duration::from_secs(2);

impl Screen for SlateScreen {
    const NAME: &'static str = "slate";
    const PLACE: Place = Place::Panel(Pane::Slate);

    fn new(host: Host, opened: Opened, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (holding_asks, mut asks) = mpsc::unbounded::<&'static str>();
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
            drawn_at: None,
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
            _watch: None,
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
        screen._watch = Some(cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(OFF_SCREEN / 2).await;
            // The right panel draws its tab cached, so a quiet slate is asked to draw again to show it is still on screen.
            let watched = this.update(cx, |this, cx| {
                if this.holding() {
                    cx.notify();
                }
                this.sync_hold();
            });
            if watched.is_err() {
                break;
            }
        }));
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

/// Whether a failure says no host was there to answer: a dial that reached nobody, or a socket that went, as wsp-mcp's
/// record words those.
fn offline(failure: &Failure) -> bool {
    static GONE: std::sync::OnceLock<Vec<Vec<String>>> = std::sync::OnceLock::new();
    let gone = GONE.get_or_init(|| {
        let words: Map<String, Value> =
            serde_json::from_str(include_str!("../../../../../../daemon/crates/wsp-mcp/record/words.json")).unwrap_or_default();
        [
            "noHostServing",
            "hostTokenMissing",
            "noAnswer",
            "noAnswerWithin",
            "hostClosed",
            "hostStopping",
            "notDelivered",
            "noHostAnswered",
            "hostExited",
        ]
        .into_iter()
        .filter_map(|key| words.get(key).and_then(Value::as_str))
        .map(|template| {
            template
                .split('{')
                .enumerate()
                .map(|(i, p)| if i == 0 { p } else { p.split_once('}').map_or("", |(_, rest)| rest) })
                .map(str::to_owned)
                .collect()
        })
        .collect()
    });
    failure.kind.as_deref() == Some("unreachable") || gone.iter().any(|words| said_in(&failure.message, words))
}

/// Whether `message` is a template's words in order with anything between them: `a {x} b` as `["a ", " b"]`.
fn said_in(message: &str, words: &[String]) -> bool {
    let Some((first, after)) = words.split_first() else { return false };
    let Some(mut rest) = message.strip_prefix(first.as_str()) else { return false };
    let Some((last, between)) = after.split_last() else { return rest.is_empty() };
    for part in between {
        match rest.find(part.as_str()) {
            Some(at) => rest = &rest[at + part.len()..],
            None => return false,
        }
    }
    rest.ends_with(last.as_str())
}

/// The right panel's pane a `pane` step names (`SLATE_PANE_KINDS`).
fn pane_of(kind: &str) -> Option<Pane> {
    Some(match kind {
        "preview" => Pane::Browser,
        "terminal" => Pane::Terminal,
        "diff" => Pane::Changes,
        "pr" => Pane::PullRequest,
        "files" => Pane::Files,
        "machine" => Pane::Computer,
        "processes" => Pane::Processes,
        "agents" => Pane::Agents,
        "slate" => Pane::Slate,
        _ => return None,
    })
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
        self.drawn_at.is_some_and(|at| at.elapsed() < OFF_SCREEN)
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
                    // An answer that moved nothing draws nothing, as the web redraws only the pieces whose reads moved.
                    if slate.resolved != resolved {
                        slate.resolved = resolved;
                        cx.notify();
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

    /// Writes one value the person typed or picked: drawn at once, then sent with `slates.state`.
    pub fn write(&mut self, path: String, value: Value, cx: &mut Context<Self>) {
        self.write_then(path, value, |_, _| {}, cx);
    }

    /// `write`, then `then` once the host has answered it, taken or not, as `StateSender` never fails a write: a pick's
    /// change event reads the value it wrote.
    pub fn write_then(
        &mut self,
        path: String,
        value: Value,
        then: impl FnOnce(&mut Self, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self.opened.thread_id.clone() else { return };
        if let Load::Drawn(slate) = &mut self.load {
            slate.queue(path.clone(), value.clone());
        }
        let sent = self.writes.entry(path.clone()).or_default();
        *sent += 1;
        let sent = *sent;
        cx.notify();
        let (asked, sending) = (path.clone(), value.clone());
        let asking = self.host.run(move |host| async move {
            let mut values = Map::new();
            values.insert(asked, sending);
            call::<protocol::SlatesStateRequest>(&host, json!({ "threadId": thread, "values": values })).await
        });
        cx.spawn(async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| {
                if let Load::Drawn(slate) = &mut this.load {
                    if this.writes.get(&path) == Some(&sent) {
                        this.writes.remove(&path);
                        // Words typed while it was out are still the field's to send.
                        if slate.mine.get(&path) == Some(&value) {
                            slate.queued.remove(&path);
                            if answer.is_ok() {
                                slate.mine.remove(&path);
                            }
                        }
                        if answer.is_ok() {
                            slate.record.set_own(&path, value);
                        }
                    }
                }
                this.resolve(cx);
                then(this, cx);
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
            Some((item, index)) => window_steps
                .iter()
                .flat_map(|step| {
                    let with =
                        step.get("with").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned);
                    ["target", "text"].into_iter().filter_map(|k| step.get(k)).flat_map(model::leaves).chain(with).collect::<Vec<_>>()
                })
                .map(|leaf| (format!("[{}] | map(_slate0: {})", model::literal(item), model::shifted(&leaf, *index)), leaf))
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
                        if let Some(said) = answer.get("said").and_then(Value::as_str).filter(|s| !s.is_empty()) {
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

    /// Keeps what the person typed drawn over a write from elsewhere until they keep it or take the other.
    pub fn keep_drawn(&mut self, path: String, value: Value, cx: &mut Context<Self>) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.queued.remove(&path);
            slate.mine.insert(path, value);
        }
        cx.notify();
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
                    Some(h) if consent::opens(&h) => {
                        self.open_link(h, cx);
                        None
                    }
                    _ => Some(Outcome::Refused("Only http, https and mailto links open from a slate.".into())),
                }
            }
            "copy" => {
                cx.write_to_clipboard(ClipboardItem::new_string(copied(value_of(step.get("text")))));
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
                    .map(|path| format!("> {path}: {}", rows.get(path).or_else(|| slate.resolved.get(path)).unwrap_or(&Value::Null)))
                    .collect();
                self.filling = Some(if with.is_empty() { text.to_owned() } else { format!("{text}\n\n{}", with.join("\n")) });
                cx.notify();
                None
            }
            "pane" => match step.get("kind").and_then(Value::as_str).and_then(pane_of) {
                Some(pane) if open_pane(pane, cx) => None,
                _ => Some(Outcome::Refused(words::NO_PANE.into())),
            },
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

    /// Keeps a section's fold where no state path keeps it.
    pub fn fold(&mut self, id: String, open: bool, cx: &mut Context<Self>) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.folds.insert(id, open);
            cx.notify();
        }
        self.resolve(cx);
    }

    /// Keeps a bar switch's pick, by the first list it switches.
    pub fn pick(&mut self, first: String, list: String, cx: &mut Context<Self>) {
        if let Load::Drawn(slate) = &mut self.load {
            slate.picks.insert(first, list);
            cx.notify();
        }
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
                    .when(refreshing, |el| el.child(crate::ui::spinner(cx).size(px(12.))))
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

/// A fill's block at the caret of the thread's composer: the open one, else its kept draft.
fn insert_in_composer(thread: &str, block: &str, window: &mut Window, cx: &mut App) {
    if !crate::screens::composer::insert(thread, block, window, cx) && cx.has_global::<crate::local_store::LocalStore>() {
        _ = cx.update_global(|s: &mut crate::local_store::LocalStore, _| {
            let (text, at) = crate::screens::composer::store::draft(s, thread).unwrap_or_default();
            let (text, caret) = crate::screens::composer::with_block(&text, at, block);
            crate::screens::composer::store::set_draft(s, thread, &text, caret)
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
            insert_in_composer(&thread, &text, window, cx);
        }
        if back {
            self.sync_hold();
            self.resolve(cx);
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
                    .child(scroll_area("slate-scroll", &self.scroll, column().pb(px(32.)).children(body), window, cx).flex_1().min_h_0())
                    .into_any_element()
            }
        };
        let overlays = consent::overlays(self, window, cx);
        div().key_context(KEY_CONTEXT).size_full().flex().flex_col().child(surface).children(overlays)
    }
}

/// What a copy step writes, as the web's `JSON.stringify(text ?? "")` does: a string as it is, nothing as `""`, and
/// anything else as JSON.
fn copied(text: Option<Value>) -> String {
    match text.unwrap_or(Value::Null) {
        Value::String(s) => s,
        Value::Null => "\"\"".to_owned(),
        other => other.to_string(),
    }
}

/// The panel's one column, 16 px in, capped at the settings pages' 760 px.
pub fn column() -> gpui_kit::Div {
    v_flex().w_full().max_w(px(760.)).mx_auto().px(px(16.))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui_kit::test]
    fn first_showings_are_kept_for_the_last_hundred_threads(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            assert!(first_showing("t0", cx));
            assert!(!first_showing("t0", cx), "a thread shown here opens once");
            for i in 1..=KEPT_THREADS {
                assert!(first_showing(&format!("t{i}"), cx));
            }
            assert_eq!(cx.global::<ShownHere>().0.len(), KEPT_THREADS);
            assert!(first_showing("t0", cx), "the oldest went past the hundredth");
        });
    }

    #[gpui_kit::test]
    fn kept_slates_are_the_last_hundred_threads(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            for i in 0..=KEPT_THREADS {
                let mut s = slate(json!({ "root": { "type": "column" } }));
                s.folds.insert("sec".into(), true);
                keep(&format!("t{i}"), &mut s, cx);
            }
            let kept = kept_threads(cx);
            assert_eq!(kept.len(), KEPT_THREADS);
            assert_eq!(kept.first().map(String::as_str), Some("t1"), "the oldest went past the hundredth");
        });
    }

    fn slate(pieces: Value) -> Slate {
        let record = json!({ "threadId": "t", "version": 1, "document": { "schema": model::SCHEMA, "root": "root", "pieces": pieces } });
        Slate::new(Record::read(&record).expect("a record"))
    }

    #[test]
    fn a_hidden_piece_asks_its_when_and_nothing_else() {
        let mut s = slate(json!({
            "root": { "type": "column", "children": ["shown", "hidden", "fold"] },
            "shown": { "type": "text", "props": { "value": { "bind": "$a + 1" } } },
            "hidden": { "type": "column", "when": "$show", "children": ["clock"] },
            "clock": { "type": "timeline", "props": { "items": { "bind": "$spans" } } },
            "fold": { "type": "section", "props": { "title": "F", "collapsible": true, "open": false }, "children": ["inside"] },
            "inside": { "type": "text", "props": { "value": { "bind": "$b + 1" } } },
        }));
        let asked = s.formulas();
        assert!(asked.contains(&"($show)".to_owned()) && asked.contains(&"($a + 1)".to_owned()), "{asked:?}");
        assert!(!asked.iter().any(|f| f == "time.now" || f.contains("$spans")), "a hidden timeline reads nothing: {asked:?}");
        assert!(!asked.contains(&"($b + 1)".to_owned()), "a folded section's body asks nothing: {asked:?}");
        s.resolved.insert("($show)".into(), json!(true));
        s.folds.insert("fold".into(), true);
        let asked = s.formulas();
        assert!(asked.contains(&"time.now".to_owned()) && asked.contains(&"($b + 1)".to_owned()), "shown and opened: {asked:?}");
    }

    #[test]
    fn a_copy_writes_what_the_web_writes() {
        assert_eq!(copied(Some(json!("plain"))), "plain");
        assert_eq!(copied(Some(Value::Null)), "\"\"");
        assert_eq!(copied(None), "\"\"");
        assert_eq!(copied(Some(json!({"a": [1, 2]}))), r#"{"a":[1,2]}"#);
    }

    #[test]
    fn a_batch_keeps_a_pieces_formulas_together() {
        let groups = vec![vec!["a".to_owned(); 150], vec!["b".to_owned(); 100], vec!["c".to_owned(); 30]];
        let batches = batched(groups, 200);
        assert_eq!(batches.iter().map(Vec::len).collect::<Vec<_>>(), [150, 130]);
        assert!(batches[1].iter().all(|f| f != "a"));
        assert_eq!(batched(vec![vec!["x".to_owned(); 450]], 200).iter().map(Vec::len).collect::<Vec<_>>(), [200, 200, 50]);
    }

    #[test]
    fn each_push_moves_the_sources_the_web_folds_it_into() {
        let moved = |frame: Value| match moved_by(&frame, "t") {
            Some(Change::Moved { sources, workspace }) => Some((sources, workspace)),
            _ => None,
        };
        let w = Some("w".to_owned());
        assert_eq!(moved(json!({"type": "workspace.status", "status": {"id": "w"}})), Some((vec!["git", "pr"], w.clone())));
        assert_eq!(moved(json!({"type": "workspace.cost", "workspaceId": "w"})), Some((vec!["cost"], w)));
        assert_eq!(moved(json!({"type": "usage.account", "key": "k"})), Some((vec!["usage"], None)));
        assert_eq!(moved(json!({"type": "session.done", "threadId": "t"})), Some((vec!["thread"], None)));
        assert_eq!(moved(json!({"type": "thread.head", "threadId": "t"})), Some((vec!["thread"], None)));
        assert_eq!(moved(json!({"type": "session.row", "threadId": "t"})), Some((vec!["thread"], None)));
        assert_eq!(moved(json!({"type": "session.delta", "threadId": "t"})), None, "a streaming reply moves no thread field");
        assert_eq!(moved(json!({"type": "session.done", "threadId": "other"})), None);
        assert_eq!(moved(json!({"type": "slate.values", "threadId": "t"})), None, "the slate's own pushes are read apart");
        assert_eq!(moved(json!({"type": "workspace.status", "status": {}})), None);
    }

    #[test]
    fn no_host_to_answer_reads_as_the_offline_sentence() {
        for gone in [
            Failure::new("no wsp host is serving /tmp/x/state/state.json; start one with wsp up --state /tmp/x/state/state.json"),
            Failure::new("the host closed the connection"),
            Failure::new("no host answered for /tmp/x/state.json within 20s"),
            Failure::of_kind("the host at ws://127.0.0.1:4 did not answer: Connection refused", "unreachable"),
        ] {
            assert_eq!(refusal(&gone), words::OFFLINE, "{}", gone.message);
        }
        let refused = Failure::new("Z800 thread-not-yours: a thread writes its own slate");
        assert_eq!(refusal(&refused), refused.message, "the host's own refusals stay its own");
        assert_eq!(refusal(&Failure::new("the host closed the connection twice")), "the host closed the connection twice");
    }
}
