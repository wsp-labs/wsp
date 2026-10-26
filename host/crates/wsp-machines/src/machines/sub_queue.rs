// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/machines/sub-queue.ts at 65f6b89c7: the queue every sub-thread's spawn goes through.
//! Spec: spec/sub_queue.md.
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::{ready, LocalBoxFuture};
use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};

use wsp_engine::box_backend::BoxLimits;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_state::CLOUD_CAP_DEFAULT;
use wsp_wire::types::sub_thread::SUB_DROPPED_LINE;
use wsp_wire::views::session::{SessionStatus, SessionView, ThreadCapWait, ThreadCapWaitWhy};
use wsp_wire::views::workspace::{road_of, scope_of, Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin, WorkspaceViewQueue, WorkspaceViewQueueWhy};
use wsp_words::date_parse::date_parse;
use wsp_words::exit::HostError;
use wsp_words::js_date::iso_string;
use wsp_words::place_state::{cap_wait_line, deleted_before_start_line};
use wsp_words::place_word::provider_key_name;
use wsp_words::sub_thread::sub_waiting_refusal;
use wsp_words::words::turn::{kept_opening, listed_failure};

use crate::machines::area_error::{AreaError, AreaResult};
use crate::machines::machines::Entry;
use wsp_core::clock::Cancel;
use wsp_core::context::Joined;
use wsp_core::types::internal::{SessionEntry, SubStartOpts, SubStartRecord, SubTree};
use wsp_core::types::wiring::WorkspaceRecord;

/// sub-queue.ts:21. How often a held spawn reads its cloud's limits again for a slot.
pub const QUEUE_SLOT_READ_MS: f64 = 30_000.0;
/// sub-queue.ts:22. How often for the next minute of starts.
pub const QUEUE_MINUTE_READ_MS: f64 = 10_000.0;
/// sub-queue.ts:25. How long a queued record waits for its first turn.
pub const QUEUE_START_WAIT_MS: f64 = 30_000.0;
/// sub-queue.ts:29. What of a start rides the written record, in this order.
pub const KEPT: [&str; 14] = ["prompt", "harness", "model", "effort", "permissionMode", "access", "contextWindow", "fast", "startedBy", "requestId", "attempt", "title", "cwd", "attachments"];

/// The handle a started turn answers (`SessionHandle`): what the queue hands its caller.
#[derive(Clone, Debug, PartialEq)]
pub struct StartedTurn {
    pub id: String,
    pub workspace_id: String,
    pub turn_id: String,
}

/// What `onHeld` is told once a held start is written down (sub-queue.ts:205).
#[derive(Clone, Debug)]
pub struct HeldTold {
    pub view: SessionView,
    pub turn_id: String,
    pub wait: ThreadCapWait,
}

/// The start a queued record's first turn asks (sub-queue.ts:624 `at`): its options as the TS object, the
/// `onHeld` callback beside them since it never rides the written record.
pub struct HoldAsk {
    pub workspace_id: String,
    pub thread_id: String,
    pub tree: IndexMap<String, Value>,
    pub o: serde_json::Map<String, Value>,
    pub on_held: Option<Rc<dyn Fn(HeldTold)>>,
    pub notify: Option<Vec<String>>,
}

/// What the queue reads off the runtime's context (sub-queue.ts:50 and the `ctx.` calls below it).
#[async_trait(?Send)]
pub trait SubQueueCtx {
    fn live(&self, id: &str) -> Option<Entry>;
    fn live_all(&self) -> Vec<Entry>;
    fn session(&self, turn_id: &str) -> Option<SessionEntry>;
    fn set_session(&self, turn_id: &str, entry: SessionEntry);
    /// Changes the row in place, keeping its map position.
    fn update_session(&self, turn_id: &str, f: &mut dyn FnMut(&mut SessionEntry));
    fn remove_session(&self, turn_id: &str);
    fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String>;
    fn holds_slot(&self, record: &WorkspaceRecord) -> bool;
    /// `placeDoor?.nameOf(placeId)`.
    fn place_name_of(&self, place_id: &str) -> Option<String>;
    /// `placeDoor?.machinesAt(placeId)`.
    fn machines_at(&self, place_id: &str) -> Option<f64>;
    /// `backendFor(record).limits()` where the backend offers it; `Ok(None)` where it does not.
    async fn limits_of(&self, record: &WorkspaceRecord) -> AreaResult<Option<BoxLimits>>;
    /// `ctx.view(record)` as the TS object.
    fn view(&self, record: &WorkspaceRecord) -> Value;
    async fn persist(&self, record: &WorkspaceRecord) -> AreaResult<()>;
    async fn persist_sessions(&self, workspace_id: &str) -> AreaResult<()>;
    fn emit(&self, event: Value);
    /// `ctx.record(event)`: a transcript event, stamped and emitted.
    fn record(&self, event: Value);
    /// `attach(record, absentMachine("", "sandbox", () => throw said))`.
    fn attach_absent(&self, record: &Rc<RefCell<WorkspaceRecord>>, said: &str) -> Entry;
    /// `attach(record, deadMachine(""))`.
    fn attach_dead(&self, record: &Rc<RefCell<WorkspaceRecord>>) -> Entry;
    /// The record's shared cell, for a record live or just read.
    fn record_cell(&self, workspace_id: &str) -> Option<Rc<RefCell<WorkspaceRecord>>>;
    /// `ctx.drop(id)`: the record goes.
    async fn drop_workspace(&self, workspace_id: &str) -> AreaResult<()>;
    /// `notifyEnd({view, turnId}, notify, tellAs({notifyBy?, notifyRoad?}), {status: "failed", error})`.
    fn notify_failed(&self, view: &SessionView, turn_id: &str, notify: &[String], by: Option<&ThreadScope>, road: Option<&Value>, error: &str);
    fn push_head(&self, thread_id: &str);
    async fn rows_sent(&self);
    async fn put_start(&self, record: &SubStartRecord) -> AreaResult<()>;
    async fn delete_start(&self, workspace_id: &str) -> AreaResult<()>;
    async fn list_starts(&self) -> AreaResult<Vec<SubStartRecord>>;
    /// The sub-spawn area's `standUpQueued`.
    async fn stand_up_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, posted: Rc<dyn Fn()>, waited: bool) -> AreaResult<()>;
    /// `sessionsApi.start(workspaceId, o, origin)`.
    async fn start_turn(&self, workspace_id: &str, o: serde_json::Map<String, Value>, origin: Option<Caller>) -> AreaResult<StartedTurn>;
    async fn cut_off(&self, entry: &Entry);
    async fn unfork(&self, entry: &Entry) -> AreaResult<()>;
    fn now(&self) -> f64;
    /// A new UUID (the `Ids` trait).
    fn uuid(&self) -> String;
    fn schedule(&self, ms: f64, f: Box<dyn FnOnce()>) -> Cancel;
    fn spawn(&self, f: LocalBoxFuture<'static, ()>);
    fn warn(&self, line: &str);
    fn on_event(&self, kind: &str, f: Rc<dyn Fn(&Value)>);
}

/// One callback a held start answers through, taken once.
pub(crate) type Answer<A> = Box<dyn FnOnce(A)>;

/// sub-queue.ts:32 `Held`: one held spawn's start, with the caller still waiting where there is one.
pub(crate) struct Held {
    pub record: SubStartRecord,
    /// The machine's name, which a delete of its record names.
    pub name: String,
    pub started: RefCell<Option<Answer<StartedTurn>>>,
    pub failed: RefCell<Option<Answer<HostError>>>,
    pub tell: RefCell<Option<Answer<ThreadCapWait>>>,
    pub written: Cell<bool>,
    pub left: RefCell<Option<Box<dyn FnOnce()>>>,
    pub gone: wsp_core::context::Joined,
    pub stopped: RefCell<Option<String>>,
}

/// A claim of a slot and a start (sub-queue.ts:106).
pub struct StartClaim {
    pub posted: Rc<dyn Fn()>,
    pub release: Rc<dyn Fn()>,
}

/// sub-queue.ts:49 `subQueueArea(ctx)`.
pub struct SubQueueArea {
    pub(crate) ctx: Rc<dyn SubQueueCtx>,
    pub(crate) me: Weak<SubQueueArea>,
    pub(crate) held: RefCell<IndexMap<String, Rc<Held>>>,
    pub(crate) last_wait: RefCell<IndexMap<String, ThreadCapWait>>,
    pub(crate) starting: RefCell<IndexMap<String, (String, bool)>>,
    pub(crate) refused_until: RefCell<IndexMap<String, f64>>,
    pub(crate) queued_at: RefCell<IndexMap<String, f64>>,
    pub(crate) pumping: RefCell<Option<wsp_core::context::Joined>>,
    pub(crate) again: Cell<bool>,
    pub(crate) timer: RefCell<Option<Cancel>>,
}

fn fire<A>(cell: &RefCell<Option<Answer<A>>>, arg: A) {
    let f = cell.borrow_mut().take();
    if let Some(f) = f {
        f(arg);
    }
}

fn fire0(cell: &RefCell<Option<Box<dyn FnOnce()>>>) {
    let f = cell.borrow_mut().take();
    if let Some(f) = f {
        f();
    }
}

fn record_of(entry: &Entry) -> Rc<RefCell<WorkspaceRecord>> {
    entry.borrow().record.clone()
}

fn snapshot(record: &RefCell<WorkspaceRecord>) -> WorkspaceRecord {
    record.borrow().clone()
}

fn id_of(entry: &Entry) -> String {
    entry.borrow().record.borrow().id.clone()
}

fn since_of(entry: &Entry) -> String {
    entry.borrow().record.borrow().queue.as_ref().map(|q| q.since.clone()).unwrap_or_default()
}

fn name_of(entry: &Entry) -> String {
    entry.borrow().record.borrow().name.clone()
}

fn default_machines() -> f64 {
    CLOUD_CAP_DEFAULT.iter().find(|(key, _)| *key == "machines").map_or(3.0, |(_, value)| *value)
}

fn queue_why_of(why: Option<ThreadCapWaitWhy>) -> WorkspaceViewQueueWhy {
    match why {
        Some(ThreadCapWaitWhy::Minute) => WorkspaceViewQueueWhy::Minute,
        _ => WorkspaceViewQueueWhy::Slot,
    }
}

fn cap_why_of(why: WorkspaceViewQueueWhy) -> ThreadCapWaitWhy {
    match why {
        WorkspaceViewQueueWhy::Minute => ThreadCapWaitWhy::Minute,
        WorkspaceViewQueueWhy::Slot => ThreadCapWaitWhy::Slot,
    }
}

fn capped_event(workspace_id: &str, turn_id: &str, thread_id: &str, wait: &ThreadCapWait) -> Value {
    let mut event = serde_json::Map::new();
    event.insert("type".into(), json!("session.capped"));
    event.insert("workspaceId".into(), json!(workspace_id));
    event.insert("sessionId".into(), json!(turn_id));
    event.insert("turnId".into(), json!(turn_id));
    event.insert("threadId".into(), json!(thread_id));
    if let Ok(Value::Object(fields)) = serde_json::to_value(wait) {
        event.extend(fields);
    }
    Value::Object(event)
}

fn new_row(view: SessionView, turn_id: String) -> SessionEntry {
    SessionEntry {
        view,
        turn_id,
        replaces: None,
        notify: None,
        notify_by: None,
        notify_road: None,
        turn_token: None,
        scope_device_id: None,
        handle: None,
        end: None,
        turn_live: None,
        run: None,
        from: None,
        asked: None,
        snapshot: None,
        pid: None,
        launch: None,
        calls: None,
        unanswered: false,
        queued: false,
    }
}

/// The start options a held record keeps: the KEPT keys the first turn asked, in KEPT order.
fn opts_from(asked: &serde_json::Map<String, Value>) -> Result<SubStartOpts, HostError> {
    let kept: serde_json::Map<String, Value> = KEPT.iter().filter_map(|key| asked.get(*key).map(|v| (key.to_string(), v.clone()))).collect();
    serde_json::from_value(Value::Object(kept)).map_err(|e| HostError::new(e.to_string()))
}

fn options_map(opts: &SubStartOpts) -> serde_json::Map<String, Value> {
    match serde_json::to_value(opts) {
        Ok(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    }
}

impl SubQueueArea {
    /// sub-queue.ts:49. Builds the area and registers its `"*"` listener (rule 23).
    pub fn new(ctx: Rc<dyn SubQueueCtx>) -> Rc<SubQueueArea> {
        Rc::new_cyclic(|me: &Weak<SubQueueArea>| {
            let bus = me.clone();
            ctx.on_event(
                "*",
                Rc::new(move |event: &Value| {
                    if let Some(area) = bus.upgrade() {
                        area.on_bus(event);
                    }
                }),
            );
            SubQueueArea {
                ctx,
                me: me.clone(),
                held: RefCell::new(IndexMap::new()),
                last_wait: RefCell::new(IndexMap::new()),
                starting: RefCell::new(IndexMap::new()),
                refused_until: RefCell::new(IndexMap::new()),
                queued_at: RefCell::new(IndexMap::new()),
                pumping: RefCell::new(None),
                again: Cell::new(false),
                timer: RefCell::new(None),
            }
        })
    }

    /// sub-queue.ts:66 (rule 2).
    pub(crate) fn queued(&self) -> Vec<Entry> {
        let mut found: Vec<(String, String, Entry)> = Vec::new();
        for entry in self.ctx.live_all() {
            let (phase, since, id) = {
                let record = record_of(&entry);
                let rec = record.borrow();
                (rec.phase, rec.queue.as_ref().map(|q| q.since.clone()), rec.id.clone())
            };
            if let (WorkspacePhase::Queued, Some(since)) = (phase, since) {
                found.push((since, id, entry));
            }
        }
        found.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        found.into_iter().map(|(_, _, entry)| entry).collect()
    }

    fn place_of(&self, entry: &Entry) -> Option<String> {
        let snap = snapshot(&record_of(entry));
        self.ctx.place_id_of(&snap)
    }

    fn is_held(&self, workspace_id: &str, h: &Rc<Held>) -> bool {
        self.held.borrow().get(workspace_id).is_some_and(|x| Rc::ptr_eq(x, h))
    }

    fn is_live_as(&self, workspace_id: &str, entry: &Entry) -> bool {
        self.ctx.live(workspace_id).is_some_and(|live| Rc::ptr_eq(&live, entry))
    }

    /// sub-queue.ts:70 (rule 3).
    pub fn slots(&self, place_id: &str) -> ThreadCapWait {
        let mut running = 0usize;
        for entry in self.ctx.live_all() {
            let snap = snapshot(&record_of(&entry));
            if self.ctx.place_id_of(&snap).as_deref() == Some(place_id) && self.ctx.holds_slot(&snap) {
                running += 1;
            }
        }
        let starting: Vec<(String, String)> = self.starting.borrow().iter().map(|(id, (place, _))| (id.clone(), place.clone())).collect();
        let starting_here = starting.iter().filter(|(id, place)| place == place_id && self.ctx.live(id).is_none()).count();
        ThreadCapWait {
            place_id: place_id.to_string(),
            place: self.ctx.place_name_of(place_id).unwrap_or_else(|| provider_key_name(place_id)),
            running: JsInt(JsNumber((running + starting_here) as f64)),
            at_once: JsInt(JsNumber(self.ctx.machines_at(place_id).unwrap_or_else(default_machines))),
            why: Some(ThreadCapWaitWhy::Slot),
        }
    }

    /// sub-queue.ts:82 (rule 4).
    pub fn wait_on(&self, place_id: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait> {
        let mine = self.slots(place_id);
        if mine.running.as_f64() >= mine.at_once.as_f64() {
            return Some(mine);
        }
        if let (Some(at_once), Some(active)) = (limits.and_then(|l| l.machines_at_once), limits.and_then(|l| l.active_boxes)) {
            if active >= at_once {
                return Some(ThreadCapWait { running: JsInt(JsNumber(active)), at_once: JsInt(JsNumber(at_once)), ..mine });
            }
        }
        if let Some(minute) = limits.and_then(|l| l.starts.as_ref()).map(|starts| &starts.minute) {
            let starting_here = self.starting.borrow().values().filter(|(place, posted)| place == place_id && !posted).count();
            let used = minute.used + starting_here as f64;
            if used >= minute.limit {
                return Some(ThreadCapWait { running: JsInt(JsNumber(used)), at_once: JsInt(JsNumber(minute.limit)), why: Some(ThreadCapWaitWhy::Minute), ..mine });
            }
        }
        None
    }

    /// sub-queue.ts:96 (rule 5).
    pub fn queue_wait(&self, place_id: &str, limits: Option<&BoxLimits>) -> Option<ThreadCapWait> {
        if let Some(wait) = self.wait_on(place_id, limits) {
            return Some(wait);
        }
        let ahead = self.queued().into_iter().find(|entry| self.place_of(entry).as_deref() == Some(place_id) && self.last_wait.borrow().contains_key(&id_of(entry)))?;
        self.last_wait.borrow().get(&id_of(&ahead)).cloned()
    }

    /// sub-queue.ts:106 (rule 6).
    pub fn claim_start(&self, workspace_id: &str, place_id: &str) -> StartClaim {
        self.starting.borrow_mut().insert(workspace_id.to_string(), (place_id.to_string(), false));
        let (posted_me, posted_id) = (self.me.clone(), workspace_id.to_string());
        let posted: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(area) = posted_me.upgrade() {
                if let Some(claim) = area.starting.borrow_mut().get_mut(&posted_id) {
                    claim.1 = true;
                }
            }
        });
        let (release_me, release_id) = (self.me.clone(), workspace_id.to_string());
        let release: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(area) = release_me.upgrade() {
                area.starting.borrow_mut().shift_remove(&release_id);
            }
        });
        StartClaim { posted, release }
    }

    /// sub-queue.ts:119 (rule 7).
    pub fn cloud_full(&self, place_id: &str, limits: Option<&BoxLimits>) -> ThreadCapWait {
        match limits.and_then(|l| l.machines_at_once) {
            None => self.slots(place_id),
            Some(at_once) => ThreadCapWait { running: JsInt(JsNumber(at_once)), at_once: JsInt(JsNumber(at_once)), ..self.slots(place_id) },
        }
    }

    /// sub-queue.ts:127 (rule 8).
    pub async fn enqueue(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<ThreadCapWait>, refused: bool) -> AreaResult<()> {
        let line: Vec<f64> = self.queued().iter().map(|entry| date_parse(&since_of(entry))).collect();
        let now = self.ctx.now();
        let at = if refused && !line.is_empty() { line[0] - 1.0 } else { now.max(line.last().copied().unwrap_or(0.0) + 1.0) };
        let since = iso_string(at).unwrap_or_default();
        let id = record.borrow().id.clone();
        if refused {
            self.refused_until.borrow_mut().insert(id.clone(), now + QUEUE_SLOT_READ_MS);
        }
        self.queued_at.borrow_mut().insert(id.clone(), now);
        {
            let mut rec = record.borrow_mut();
            rec.phase = WorkspacePhase::Queued;
            rec.queue = Some(WorkspaceViewQueue { since, why: queue_why_of(wait.as_ref().and_then(|w| w.why)) });
            rec.machine_id = String::new();
        }
        if let Some(wait) = &wait {
            self.last_wait.borrow_mut().insert(id, wait.clone());
        }
        self.attach_queued(record, wait.as_ref());
        self.persist_cell(record).await?;
        let view = self.ctx.view(&snapshot(record));
        self.ctx.emit(json!({ "type": "workspace.created", "workspace": view }));
        drop(self.pump_queue());
        Ok(())
    }

    async fn persist_cell(&self, record: &RefCell<WorkspaceRecord>) -> AreaResult<()> {
        self.ctx.persist(&snapshot(record)).await
    }

    /// sub-queue.ts:143 (rule 9).
    pub fn attach_queued(&self, record: &Rc<RefCell<WorkspaceRecord>>, wait: Option<&ThreadCapWait>) {
        let said = match wait {
            None => format!("{} is waiting for a machine", record.borrow().name),
            Some(wait) => cap_wait_line(wait),
        };
        self.ctx.attach_absent(record, &said);
    }

    /// sub-queue.ts:151 (rule 10).
    pub(crate) fn show_row(&self, h: &Held, wait: Option<&ThreadCapWait>) {
        let r = &h.record;
        let mut view = serde_json::Map::new();
        view.insert("id".into(), json!(r.turn_id));
        view.insert("workspaceId".into(), json!(r.workspace_id));
        view.insert("harness".into(), json!(r.opts.harness.clone().unwrap_or_default()));
        view.insert("status".into(), json!("running"));
        let started_by = r.opts.started_by.as_ref().and_then(|by| serde_json::to_value(by).ok()).unwrap_or_else(|| json!("person"));
        view.insert("startedBy".into(), started_by);
        view.insert("threadId".into(), json!(r.thread_id));
        if let Ok(Value::Object(tree)) = serde_json::to_value(&r.tree) {
            view.extend(tree);
        }
        view.insert("prompt".into(), json!(kept_opening(&r.opts.prompt)));
        view.insert("startedAt".into(), json!(date_parse(&r.at)));
        if let Some(title) = &r.opts.title {
            view.insert("harnessTitle".into(), json!(title));
            view.insert("titleSource".into(), json!("person"));
        }
        if let Some(wait) = wait {
            view.insert("capped".into(), serde_json::to_value(wait).unwrap_or(Value::Null));
        }
        let view: SessionView = serde_json::from_value(Value::Object(view)).expect("a held start's row reads as a session view");
        let mut row = new_row(view, r.turn_id.clone());
        row.queued = true;
        row.launch = Some(h.gone.clone());
        row.notify = r.notify.clone();
        row.notify_by = r.by.clone();
        row.notify_road = r.road;
        self.ctx.set_session(&r.turn_id, row);
        self.ctx.emit(json!({ "type": "session.held", "workspaceId": r.workspace_id, "threadId": r.thread_id }));
        if let Some(wait) = wait {
            self.ctx.record(capped_event(&r.workspace_id, &r.turn_id, &r.thread_id, wait));
        }
    }

    /// sub-queue.ts:172.
    pub(crate) fn hold_of(&self, record: SubStartRecord) -> Rc<Held> {
        let (tx, rx) = oneshot::channel::<()>();
        let gone: Joined = async move {
            let _ = rx.await;
        }
        .boxed_local()
        .shared();
        let name = self.ctx.live(&record.workspace_id).map_or_else(|| record.workspace_id.clone(), |entry| name_of(&entry));
        let left: Box<dyn FnOnce()> = Box::new(move || {
            let _ = tx.send(());
        });
        let ws = record.workspace_id.clone();
        let h = Rc::new(Held {
            record,
            name,
            started: RefCell::new(None),
            failed: RefCell::new(None),
            tell: RefCell::new(None),
            written: Cell::new(false),
            left: RefCell::new(Some(left)),
            gone,
            stopped: RefCell::new(None),
        });
        self.held.borrow_mut().insert(ws.clone(), h.clone());
        let wait = self.last_wait.borrow().get(&ws).cloned();
        self.show_row(&h, wait.as_ref());
        h
    }

    /// sub-queue.ts:183 (rule 11).
    pub fn hold_queued(&self, at: HoldAsk, origin: Option<&Caller>) -> LocalBoxFuture<'static, Result<StartedTurn, HostError>> {
        let ws = at.workspace_id.clone();
        if self.held.borrow().contains_key(&ws) {
            let name = self.ctx.live(&ws).map_or_else(|| ws.clone(), |entry| name_of(&entry));
            return ready(Err(sub_waiting_refusal(&name))).boxed_local();
        }
        if self.ctx.live(&ws).is_none() {
            return ready(Err(HostError::new(format!("{ws} is not live")))).boxed_local();
        }
        let opts = match opts_from(&at.o) {
            Ok(opts) => opts,
            Err(e) => return ready(Err(e)).boxed_local(),
        };
        let tree = match serde_json::from_value::<SubTree>(Value::Object(at.tree.into_iter().collect())) {
            Ok(tree) => tree,
            Err(e) => return ready(Err(HostError::new(e.to_string()))).boxed_local(),
        };
        let record = SubStartRecord {
            workspace_id: ws.clone(),
            thread_id: at.thread_id,
            turn_id: self.ctx.uuid(),
            at: iso_string(self.ctx.now()).unwrap_or_default(),
            opts,
            tree,
            notify: at.notify,
            by: scope_of(origin),
            road: road_of(origin),
        };
        let (tx, rx) = oneshot::channel::<Result<StartedTurn, HostError>>();
        let slot = Rc::new(RefCell::new(Some(tx)));
        let h = self.hold_of(record.clone());
        let started_slot = slot.clone();
        let started: Box<dyn FnOnce(StartedTurn)> = Box::new(move |turn| {
            let sender = started_slot.borrow_mut().take();
            if let Some(sender) = sender {
                let _ = sender.send(Ok(turn));
            }
        });
        let failed_slot = slot;
        let failed: Box<dyn FnOnce(HostError)> = Box::new(move |e| {
            let sender = failed_slot.borrow_mut().take();
            if let Some(sender) = sender {
                let _ = sender.send(Err(e));
            }
        });
        let tell_ctx = self.ctx.clone();
        let turn = record.turn_id.clone();
        let on_held = at.on_held;
        let tell: Box<dyn FnOnce(ThreadCapWait)> = Box::new(move |wait| {
            if let Some(on_held) = on_held {
                if let Some(row) = tell_ctx.session(&turn) {
                    on_held(HeldTold { view: row.view, turn_id: turn.clone(), wait });
                }
            }
        });
        *h.started.borrow_mut() = Some(started);
        *h.failed.borrow_mut() = Some(failed);
        *h.tell.borrow_mut() = Some(tell);
        let me = self.me.clone();
        self.ctx.spawn(
            async move {
                let Some(area) = me.upgrade() else { return };
                match area.ctx.put_start(&record).await {
                    Ok(()) => {
                        h.written.set(true);
                        let wait = area.last_wait.borrow().get(&ws).cloned();
                        if let Some(wait) = wait {
                            fire(&h.tell, wait);
                        }
                        drop(area.pump_queue());
                    }
                    Err(e) => {
                        let line = format!("the held start was not written down: {}", e.message());
                        if let Err(x) = area.drop_held(&h, &line, true).await {
                            area.ctx.warn(&format!("workspace {ws}: the held start was not dropped: {}", x.message()));
                        }
                    }
                }
            }
            .boxed_local(),
        );
        async move {
            match rx.await {
                Ok(answer) => answer,
                Err(_) => Err(HostError::new("the held start was dropped")),
            }
        }
        .boxed_local()
    }

    /// sub-queue.ts:220 (rule 12).
    pub(crate) fn forget(&self, workspace_id: &str) {
        self.held.borrow_mut().shift_remove(workspace_id);
        self.last_wait.borrow_mut().shift_remove(workspace_id);
        self.refused_until.borrow_mut().shift_remove(workspace_id);
        self.queued_at.borrow_mut().shift_remove(workspace_id);
    }

    /// sub-queue.ts:228 `tellFailed`: the thread's targets hear a spawn that failed as its turn's end.
    fn tell_failed(&self, r: &SubStartRecord, view: &SessionView, reason: &str) {
        let Some(notify) = &r.notify else { return };
        let road = r.road.map(|road| serde_json::to_value(road).unwrap_or(Value::Null));
        self.ctx.notify_failed(view, &r.turn_id, notify, r.by.as_ref(), road.as_ref(), reason);
    }

    /// sub-queue.ts:234 (rule 14).
    pub(crate) async fn drop_held(&self, h: &Rc<Held>, reason: &str, failed_spawn: bool) -> AreaResult<()> {
        let r = h.record.clone();
        if !self.is_held(&r.workspace_id, h) {
            return Ok(());
        }
        self.forget(&r.workspace_id);
        if failed_spawn {
            if let Some(row) = self.ctx.session(&r.turn_id) {
                self.tell_failed(&r, &row.view, reason);
            }
        }
        self.ctx.remove_session(&r.turn_id);
        fire0(&h.left);
        fire(&h.failed, HostError::new(reason));
        self.ctx.delete_start(&r.workspace_id).await?;
        self.ctx.drop_workspace(&r.workspace_id).await
    }

    /// sub-queue.ts:250 (rule 15).
    pub(crate) async fn fail_first(&self, h: &Rc<Held>, record: &Rc<RefCell<WorkspaceRecord>>, e: AreaError) -> AreaResult<()> {
        let r = h.record.clone();
        let why = e.failure_words();
        self.forget(&r.workspace_id);
        {
            let mut rec = record.borrow_mut();
            rec.phase = WorkspacePhase::Released;
            rec.machine_id = String::new();
            if let Some(sub) = rec.sub.as_mut() {
                sub.opening = Some(r.opts.prompt.clone());
                if let Some(spawn) = sub.spawn.as_mut() {
                    spawn.asked = Some(r.opts.prompt.clone());
                }
            }
        }
        self.ctx.attach_dead(record);
        self.persist_cell(record).await?;
        if self.ctx.session(&r.turn_id).is_some() {
            let now = self.ctx.now();
            let mut failed_view = None;
            self.ctx.update_session(&r.turn_id, &mut |row| {
                row.view.status = SessionStatus::Failed;
                row.view.failure = Some(listed_failure(Some(&why)));
                row.view.ended_at = Some(JsNumber(now));
                row.view.capped = None;
                row.launch = None;
                failed_view = Some(row.view.clone());
            });
            if let Some(view) = failed_view {
                self.tell_failed(&r, &view, &why);
            }
            self.ctx.persist_sessions(&r.workspace_id).await?;
        }
        self.ctx.push_head(&r.thread_id);
        self.ctx.rows_sent().await;
        fire0(&h.left);
        self.ctx.delete_start(&r.workspace_id).await?;
        fire(&h.failed, e.into_host());
        Ok(())
    }

    /// sub-queue.ts:280 (rule 16).
    pub async fn drop_queued(&self, turn_id: &str) -> bool {
        let found = self.held.borrow().values().find(|h| h.record.turn_id == turn_id).cloned();
        let Some(h) = found else { return false };
        if let Err(e) = self.stop(&h, SUB_DROPPED_LINE).await {
            self.ctx.warn(&format!("the held start {turn_id} was not stopped: {}", e.message()));
        }
        true
    }

    /// sub-queue.ts:287 `stop`: a record whose machine is being made only marks the stop; else the start drops now.
    async fn stop(&self, h: &Rc<Held>, reason: &str) -> AreaResult<()> {
        let has_queue = self.ctx.live(&h.record.workspace_id).is_some_and(|entry| record_of(&entry).borrow().queue.is_some());
        if has_queue {
            self.drop_held(h, reason, false).await
        } else {
            *h.stopped.borrow_mut() = Some(reason.to_string());
            Ok(())
        }
    }

    /// sub-queue.ts:294 (rule 17). `None` for a record the queue holds no start of.
    pub fn delete_queued(&self, workspace_id: &str) -> Option<LocalBoxFuture<'static, ()>> {
        let h = self.held.borrow().get(workspace_id).cloned()?;
        let me = self.me.clone();
        let line = deleted_before_start_line(&h.name);
        Some(
            async move {
                if let Some(area) = me.upgrade() {
                    if let Err(e) = area.stop(&h, &line).await {
                        area.ctx.warn(&format!("the held start was not stopped: {}", e.message()));
                    }
                }
                let gone = h.gone.clone();
                gone.await;
            }
            .boxed_local(),
        )
    }

    /// sub-queue.ts:305 (rule 18).
    pub fn pump_queue(&self) -> wsp_core::context::Joined {
        if let Some(running) = self.pumping.borrow().clone() {
            self.again.set(true);
            return running;
        }
        let me = self.me.clone();
        let pumping: LocalBoxFuture<'static, ()> = async move {
            let Some(area) = me.upgrade() else { return };
            let outcome = area.pump_loop().await;
            *area.pumping.borrow_mut() = None;
            if let Err(e) = outcome {
                area.ctx.warn(&format!("the queue of held spawns did not move: {}", e.failure_words()));
            }
        }
        .boxed_local();
        let shared = pumping.shared();
        *self.pumping.borrow_mut() = Some(shared.clone());
        self.ctx.spawn(shared.clone().boxed_local());
        shared
    }

    async fn pump_loop(&self) -> AreaResult<()> {
        loop {
            self.again.set(false);
            while self.pump_once().await? {}
            if !self.again.get() {
                return Ok(());
            }
        }
    }

    /// sub-queue.ts:324 (rule 19): replaces the one timer.
    fn arm(&self, ms: f64) {
        let previous = self.timer.borrow_mut().take();
        if let Some(previous) = previous {
            previous();
        }
        let me = self.me.clone();
        let cancel = self.ctx.schedule(
            ms,
            Box::new(move || {
                if let Some(area) = me.upgrade() {
                    area.timer.borrow_mut().take();
                    drop(area.pump_queue());
                }
            }),
        );
        *self.timer.borrow_mut() = Some(cancel);
    }

    fn disarm(&self) {
        let previous = self.timer.borrow_mut().take();
        if let Some(previous) = previous {
            previous();
        }
    }

    /// sub-queue.ts:332-333: a record's drop time, from when it was queued here or its `since`.
    fn due_of(&self, entry: &Entry) -> f64 {
        let queued = self.queued_at.borrow().get(&id_of(entry)).copied().unwrap_or_else(|| date_parse(&since_of(entry)));
        queued + QUEUE_START_WAIT_MS
    }

    /// sub-queue.ts:332 (rule 20).
    pub(crate) async fn pump_once(&self) -> AreaResult<bool> {
        let line = self.queued();
        let startless: Vec<Entry> = line.iter().filter(|entry| !self.held.borrow().contains_key(&id_of(entry))).cloned().collect();
        for entry in &startless {
            if self.ctx.now() < self.due_of(entry) {
                continue;
            }
            let id = id_of(entry);
            self.ctx.warn(&format!("workspace {id}: its first turn never came, so its queued spawn is dropped"));
            self.forget(&id);
            self.ctx.drop_workspace(&id).await?;
            return Ok(true);
        }
        let head = line.iter().find_map(|entry| self.held.borrow().get(&id_of(entry)).cloned().map(|h| (entry.clone(), h)));
        let Some((head, h)) = head else {
            self.disarm();
            if let Some(earliest) = startless.iter().map(|entry| self.due_of(entry)).reduce(f64::min) {
                self.arm(earliest - self.ctx.now());
            }
            return Ok(false);
        };
        let head_id = id_of(&head);
        let now = self.ctx.now();
        let until = self.refused_until.borrow().get(&head_id).copied();
        if let Some(until) = until {
            if now < until {
                self.arm(until - now);
                return Ok(false);
            }
        }
        self.refused_until.borrow_mut().shift_remove(&head_id);
        let snap = snapshot(&record_of(&head));
        let place_id = self.ctx.place_id_of(&snap).unwrap_or_default();
        let limits = self.ctx.limits_of(&snap).await.ok().flatten();
        if !self.is_live_as(&head_id, &head) || !self.is_held(&head_id, &h) {
            return Ok(true);
        }
        let Some(wait) = self.wait_on(&place_id, limits.as_ref()) else {
            let me = self.me.clone();
            let mut begun = async move {
                if let Some(area) = me.upgrade() {
                    area.let_through(head, h).await;
                }
            }
            .boxed_local();
            if (&mut begun).now_or_never().is_none() {
                self.ctx.spawn(begun);
            }
            return Ok(true);
        };
        for entry in line.iter().filter(|entry| self.place_of(entry).as_deref() == Some(place_id.as_str())) {
            let id = id_of(entry);
            let Some(x) = self.held.borrow().get(&id).cloned() else { continue };
            let said = self.last_wait.borrow().get(&id).cloned();
            if said.as_ref().is_some_and(|said| said.running == wait.running && said.at_once == wait.at_once && said.why == wait.why) {
                continue;
            }
            self.last_wait.borrow_mut().insert(id.clone(), wait.clone());
            if x.written.get() {
                fire(&x.tell, wait.clone());
            }
            let turn = x.record.turn_id.clone();
            self.ctx.update_session(&turn, &mut |row| row.view.capped = Some(wait.clone()));
            let record = record_of(entry);
            let differs = record.borrow().queue.as_ref().is_none_or(|q| Some(cap_why_of(q.why)) != wait.why);
            if differs {
                let since = since_of(entry);
                record.borrow_mut().queue = Some(WorkspaceViewQueue { since, why: queue_why_of(wait.why) });
                self.persist_cell(&record).await?;
            }
            self.ctx.record(capped_event(&id, &turn, &x.record.thread_id, &wait));
        }
        let next = if wait.why == Some(ThreadCapWaitWhy::Minute) { QUEUE_MINUTE_READ_MS } else { QUEUE_SLOT_READ_MS };
        self.arm(next);
        Ok(false)
    }

    /// sub-queue.ts:388 (rule 21).
    pub(crate) async fn let_through(&self, entry: Entry, h: Rc<Held>) {
        let record = record_of(&entry);
        let id = id_of(&entry);
        let waited = self.last_wait.borrow().contains_key(&id);
        record.borrow_mut().queue = None;
        self.last_wait.borrow_mut().shift_remove(&id);
        let place_id = self.place_of(&entry).unwrap_or_default();
        let claim = self.claim_start(&id, &place_id);
        let turn = h.record.turn_id.clone();
        if self.ctx.session(&turn).is_some() {
            self.ctx.update_session(&turn, &mut |row| row.view.capped = None);
            self.ctx.emit(json!({ "type": "session.held", "workspaceId": id, "threadId": h.record.thread_id }));
        }
        let waiting_view = self.ctx.session(&turn).map(|row| row.view);
        let attempt: AreaResult<Option<StartedTurn>> = async {
            self.persist_cell(&record).await?;
            self.ctx.stand_up_queued(&record, claim.posted.clone(), waited).await?;
            (claim.release)();
            if !self.is_held(&id, &h) {
                return Ok(None);
            }
            let stopped = h.stopped.borrow().clone();
            if let Some(reason) = stopped {
                if let Some(standing) = self.ctx.live(&id) {
                    self.ctx.cut_off(&standing).await;
                    if let Err(e) = self.ctx.unfork(&standing).await {
                        self.ctx.warn(&format!("workspace {id}: its machine was not deleted after the stop: {}", e.failure_words()));
                    }
                }
                self.drop_held(&h, &reason, false).await?;
                return Ok(None);
            }
            self.forget(&id);
            self.ctx.remove_session(&turn);
            fire0(&h.left);
            self.ctx.delete_start(&id).await?;
            let r = h.record.clone();
            let origin = match (&r.by, r.road) {
                (Some(by), road) => Some(Caller::Scoped { origin: road.unwrap_or(WorkspaceOrigin::Here), by: by.clone() }),
                (None, road) => road.map(Caller::Road),
            };
            let mut o = options_map(&r.opts);
            if let Some(notify) = &r.notify {
                o.insert("notify".into(), json!(notify));
            }
            o.insert("as".into(), json!({ "threadId": r.thread_id, "turnId": r.turn_id }));
            self.ctx.start_turn(&id, o, origin).await.map(Some)
        }
        .await;
        match attempt {
            Ok(None) => {}
            Ok(Some(handle)) => {
                drop(self.pump_queue());
                fire(&h.started, handle);
                return;
            }
            Err(e) => self.let_through_failed(&entry, &h, &claim, e, waiting_view).await,
        }
        drop(self.pump_queue());
    }

    /// The catch of sub-queue.ts:413-441: a stop drops, a cap refusal queues again, any other failure fails the first
    /// start or tells the caller.
    async fn let_through_failed(&self, entry: &Entry, h: &Rc<Held>, claim: &StartClaim, e: AreaError, waiting_view: Option<SessionView>) {
        (claim.release)();
        let id = id_of(entry);
        let record = record_of(entry);
        let stopped = h.stopped.borrow().clone();
        if let Some(reason) = stopped {
            if self.is_held(&id, h) {
                if let Err(x) = self.drop_held(h, &reason, false).await {
                    self.ctx.warn(&format!("workspace {id}: the stopped start was not dropped: {}", x.message()));
                }
                return;
            }
        }
        if e.is_cap_refusal() && self.is_held(&id, h) && h.stopped.borrow().is_none() {
            let snap = snapshot(&record);
            let place_id = self.ctx.place_id_of(&snap).unwrap_or_default();
            let limits = self.ctx.limits_of(&snap).await.ok().flatten();
            let wait = self.cloud_full(&place_id, limits.as_ref());
            if let Err(x) = self.enqueue(&record, Some(wait.clone()), true).await {
                self.ctx.warn(&format!("workspace {id}: the refused spawn was not queued again: {}", x.message()));
            }
            let turn = h.record.turn_id.clone();
            self.ctx.update_session(&turn, &mut |row| row.view.capped = Some(wait.clone()));
            return;
        }
        let why = e.failure_words();
        self.ctx.warn(&format!("workspace {id}: the held spawn did not start: {why}"));
        if self.is_held(&id, h) {
            if let Err(x) = self.fail_first(h, &record, e).await {
                self.ctx.warn(&format!("workspace {id}: the failed first start was not recorded: {}", x.message()));
            }
        } else {
            if let Some(view) = waiting_view {
                self.tell_failed(&h.record, &view, &why);
            }
            fire(&h.failed, e.into_host());
        }
    }

    /// sub-queue.ts:454 (rule 22).
    pub async fn resume_queue(&self) -> AreaResult<()> {
        let kept: IndexMap<String, SubStartRecord> = self.ctx.list_starts().await?.into_iter().map(|r| (r.workspace_id.clone(), r)).collect();
        for id in kept.keys() {
            let queued = self.ctx.live(id).is_some_and(|entry| record_of(&entry).borrow().phase == WorkspacePhase::Queued);
            if !queued {
                self.ctx.delete_start(id).await?;
            }
        }
        let queued_now: Vec<Entry> = self.ctx.live_all().into_iter().filter(|entry| record_of(entry).borrow().phase == WorkspacePhase::Queued).collect();
        for entry in queued_now {
            let id = id_of(&entry);
            let Some(r) = kept.get(&id).cloned() else {
                self.ctx.drop_workspace(&id).await?;
                continue;
            };
            let cell = record_of(&entry);
            let needs_queue = cell.borrow().queue.is_none();
            if needs_queue {
                cell.borrow_mut().queue = Some(WorkspaceViewQueue { since: r.at.clone(), why: WorkspaceViewQueueWhy::Slot });
                self.persist_cell(&cell).await?;
            }
            let place_id = self.place_of(&entry).unwrap_or_default();
            let limits = self.ctx.limits_of(&snapshot(&cell)).await.ok().flatten();
            let wait = self.wait_on(&place_id, limits.as_ref()).unwrap_or_else(|| self.slots(&place_id));
            self.last_wait.borrow_mut().insert(id, wait);
            self.hold_of(r);
        }
        drop(self.pump_queue());
        Ok(())
    }

    /// sub-queue.ts:478-495 (rule 23): the bus listener.
    fn on_bus(&self, event: &Value) {
        let kind = event["type"].as_str().unwrap_or("");
        let id = event["workspaceId"].as_str().map(str::to_string);
        if kind == "workspace.deleted" {
            if let Some(id) = &id {
                self.queued_at.borrow_mut().shift_remove(id);
            }
        }
        if self.held.borrow().is_empty() {
            return;
        }
        if kind == "workspace.deleted" {
            if let Some(id) = &id {
                let own = self.held.borrow().get(id).cloned();
                if let Some(own) = own {
                    self.forget(id);
                    fire0(&own.left);
                    fire(&own.failed, HostError::new(deleted_before_start_line(&own.name)));
                    let ctx = self.ctx.clone();
                    let gone_id = id.clone();
                    self.ctx.spawn(
                        async move {
                            if let Err(e) = ctx.delete_start(&gone_id).await {
                                ctx.warn(&format!("workspace {gone_id}: its held start was not deleted: {}", e.message()));
                            }
                        }
                        .boxed_local(),
                    );
                }
                let held: Vec<Rc<Held>> = self.held.borrow().values().cloned().collect();
                for h in held {
                    let lead = self.ctx.live(&h.record.workspace_id).and_then(|entry| record_of(&entry).borrow().lead.clone());
                    if lead.is_some_and(|lead| lead.workspace_id == *id) {
                        let me = self.me.clone();
                        self.ctx.spawn(
                            async move {
                                if let Some(area) = me.upgrade() {
                                    if let Err(e) = area.drop_held(&h, "its lead's folder was deleted", false).await {
                                        area.ctx.warn(&format!("the held start was not dropped: {}", e.message()));
                                    }
                                }
                            }
                            .boxed_local(),
                        );
                    }
                }
            }
        }
        if matches!(kind, "workspace.napped" | "workspace.deleted" | "workspace.gone" | "workspace.released" | "place.changed") {
            drop(self.pump_queue());
        }
    }
}
