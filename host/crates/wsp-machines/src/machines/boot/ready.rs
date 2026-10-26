// SPDX-License-Identifier: AGPL-3.0-only
//! boot.ts:328-621 at 65f6b89c7: the host's start. Spec: spec/boot_ready.md.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use futures::channel::oneshot;
use futures::future::{FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};

use wsp_core::types::internal::{
    read_asked, read_road, read_scope, read_steered, restart_cut_line, Reopened, SessionEntry,
    StoredBuilder, TurnLive, BUILDERS, HELD_STARTS, OWNER, PROJECTS, READS, READS_ID,
    RESTARTED_REASON, SESSIONS, TRANSCRIPTS, WORKSPACES,
};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{ForkedFrom, SessionView};
use wsp_wire::views::session_events::TurnStatus;
use wsp_wire::views::workspace::{ProjectSource, ProjectView};
use wsp_wire::wire::events_union::EventUnion;
use wsp_words::exit::HostError;
use wsp_words::projects::{copies_folder, kind_for_computer};
use wsp_words::words::thread::{GoneSeenBy, RUN_GONE_LINE};

use super::load::{Asked, LoadReads, LOAD_READS_MS};
use super::{BootArea, CardAsk, HeldEnd, ScopedDevice};
use crate::machines::machines::{Entry, Row, ThreadOn};

/// The one start every caller of `ready()` awaits; its failure is shared as its words.
pub type Ready =
    futures::future::Shared<futures::future::LocalBoxFuture<'static, Result<(), String>>>;

/// A JSON value's string member, or none for a missing or non-string one.
fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// A JSON value's number member, or none for a missing or non-number one.
fn number(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

/// A value read back into the type it is, or the host error its read gave.
fn parse<T: DeserializeOwned>(v: Value) -> Result<T, HostError> {
    serde_json::from_value(v).map_err(|e| HostError::new(e.to_string()))
}

/// Waits for a future or the clock's deadline, whichever comes first, and cancels the deadline when the future wins.
async fn settle_by<F: std::future::Future<Output = ()>>(
    clock: &Rc<dyn wsp_core::types::pending::Clock>,
    until: i64,
    fut: F,
) {
    let (tx, rx) = oneshot::channel::<()>();
    let cancel = clock.schedule(
        Box::new(move || {
            let _ = tx.send(());
        }),
        (until - clock.now()).max(0),
    );
    futures::pin_mut!(fut);
    futures::future::select(fut, rx).await;
    cancel();
}

/// A call the TS makes runs to its first wait the moment it is made: the context's own turn is taken at the call, and the
/// caller goes on one turn later, as `await` does.
async fn at_call<F: std::future::Future>(fut: F) -> F::Output {
    let mut fut = Box::pin(fut);
    let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
    for _ in 0..2 {
        if let std::task::Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
            tokio::task::yield_now().await;
            return out;
        }
    }
    fut.await
}

impl BootArea {
    /// `ready()` (boot.ts:329): the one start, joined; spec rules 1-14 in order.
    pub fn ready(self: &Rc<Self>) -> Ready {
        if let Some(ready) = self.hydrated.borrow().clone() {
            return ready;
        }
        let area = self.clone();
        let start: Ready = async move { area.start().await.map_err(|e| e.message) }
            .boxed_local()
            .shared();
        *self.hydrated.borrow_mut() = Some(start.clone());
        tokio::task::spawn_local(start.clone());
        start
    }

    /// Spawns `fut` in the background, warning with `line` and its failure's words when it fails.
    fn spawn_warned<F>(self: &Rc<Self>, fut: F, line: String)
    where
        F: std::future::Future<Output = Result<(), HostError>> + 'static,
    {
        let area = self.clone();
        tokio::task::spawn_local(async move {
            if let Err(e) = fut.await {
                area.rt.warn(&format!("{line}: {}", e.message));
            }
        });
    }

    /// Starts `fut` in the background, warning with `line` and its failure's words when it fails. The call runs as a
    /// call the TS makes without awaiting does: polled in place, with a turn given to it, before the start goes on.
    async fn detach<F>(self: &Rc<Self>, fut: F, line: String)
    where
        F: std::future::Future<Output = Result<(), HostError>> + 'static,
    {
        let mut fut: LocalBoxFuture<'static, Result<(), HostError>> = fut.boxed_local();
        for _ in 0..2 {
            let polled = futures::future::poll_fn(|cx| match fut.as_mut().poll(cx) {
                std::task::Poll::Ready(out) => std::task::Poll::Ready(Some(out)),
                std::task::Poll::Pending => std::task::Poll::Ready(None),
            })
            .await;
            match polled {
                Some(Ok(())) => return,
                Some(Err(e)) => {
                    self.rt.warn(&format!("{line}: {}", e.message));
                    return;
                }
                None => tokio::task::yield_now().await,
            }
        }
        let area = self.clone();
        tokio::task::spawn_local(async move {
            if let Err(e) = fut.await {
                area.rt.warn(&format!("{line}: {}", e.message));
            }
        });
    }

    /// `persistSessions` started and not awaited, its failure swallowed by the chain (boot.ts:529).
    fn persist_later(self: &Rc<Self>, workspace_id: String) {
        let area = self.clone();
        let mut write = async move { area.rt.persist_sessions(&workspace_id).await }.boxed_local();
        let mut cx = std::task::Context::from_waker(futures::task::noop_waker_ref());
        if write.as_mut().poll(&mut cx).is_pending() && write.as_mut().poll(&mut cx).is_pending() {
            tokio::task::spawn_local(write);
        }
    }

    /// `revokeButShells` (boot.ts:36): every device given that no shell holds is revoked.
    async fn revoke_but_shells(&self, devices: &[ScopedDevice]) -> Result<(), HostError> {
        let shells = at_call(self.rt.shell_held_devices()).await?;
        for device in devices {
            if !shells.contains(&device.id) {
                at_call(self.rt.revoke_device(&device.id)).await?;
            }
        }
        tokio::task::yield_now().await;
        Ok(())
    }

    /// The start itself, in the order of boot.ts:331-620.
    async fn start(self: &Rc<Self>) -> Result<(), HostError> {
        self.load_owner_and_reads().await?;
        self.rt.migrate_copies().await?;
        self.rt.place_door_load().await?;
        self.load_projects().await?;
        self.rt.setups_load().await?;
        let load = Rc::new(LoadReads {
            until: self.rt.clock().now() + LOAD_READS_MS as i64,
            late: RefCell::default(),
        });
        let mut to_sync: Vec<Entry> = Vec::new();
        for raw in self.rt.store().list(WORKSPACES).await? {
            if let Some(entry) = self
                .hydrate_workspace(raw, None, Some(load.clone()), None)
                .await?
            {
                to_sync.push(entry);
            }
        }
        self.move_transcripts().await?;
        self.rt.move_blobs();
        for id in self.rt.store().keys(WORKSPACES).await? {
            if !self.rt.has_transcript_index(&id) {
                self.rt.load_index(&id).await?;
            }
        }
        let mut sessions = self.read_session_documents().await?;
        for ws in sessions.recut.clone() {
            let area = self.clone();
            self.detach(
                async move {
                    area.rt.persist_sessions(&ws).await;
                    Ok(())
                },
                String::new(),
            )
            .await;
        }
        let landing = self.begin_late_loads(&load);
        for id in self.rt.live().borrow().keys().cloned().collect::<Vec<_>>() {
            let area = self.clone();
            let line = format!("workspace {id} was not read again");
            self.spawn_warned(
                async move { area.reread_held(&id, GoneSeenBy::RecordLoad).await },
                line,
            );
        }
        let works = self.start_workspace_work(&mut sessions, &landing, &to_sync);
        let mut started = self.finish_cards(&sessions, &landing);
        started.extend(works.into_iter().map(|work| work.boxed_local()));
        let area = self.clone();
        let load = load.clone();
        let tail = async move {
            at_call(area.rt.resume_queue()).await?;
            area.end_held_starts().await?;
            let stale: Vec<ScopedDevice> = at_call(area.rt.devices())
                .await?
                .into_iter()
                .filter(|d| {
                    d.thread_id
                        .as_ref()
                        .is_some_and(|t| !area.rt.thread_runs(t))
                })
                .collect();
            if !stale.is_empty() {
                area.revoke_but_shells(&stale).await?;
            }
            at_call(area.rt.revoke_asides()).await?;
            area.rt.arm_subs();
            area.detach(
                area.rt_deliver_owed(),
                "the lines owed to threads were not sent".to_string(),
            )
            .await;
            area.admit_builders(&load).await?;
            area.detach(
                area.rt_move_old_copies(),
                "the move off old copies stopped".to_string(),
            )
            .await;
            area.rt.arm_sweep();
            area.detach(
                area.rt_slates_ready(),
                "the slates were not loaded".to_string(),
            )
            .await;
            Ok(())
        }
        .boxed_local();
        start_all(started, tail).await
    }

    /// `rt.deliverOwed()` as a future of its own (boot.ts:580).
    fn rt_deliver_owed(self: &Rc<Self>) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let area = self.clone();
        async move { area.rt.deliver_owed().await }.boxed_local()
    }

    /// `moveOldCopies` started and not awaited (boot.ts:600).
    fn rt_move_old_copies(self: &Rc<Self>) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let area = self.clone();
        async move { area.rt.move_old_copies().await }.boxed_local()
    }

    /// `slates.ready()` started and not awaited (boot.ts:617).
    fn rt_slates_ready(self: &Rc<Self>) -> LocalBoxFuture<'static, Result<(), HostError>> {
        let area = self.clone();
        async move { area.rt.slates_ready().await }.boxed_local()
    }

    /// The owner and the reads stamp (boot.ts:331-340).
    async fn load_owner_and_reads(&self) -> Result<(), HostError> {
        let stored = self.rt.store().get(OWNER, "id").await?;
        match stored
            .as_ref()
            .and_then(|v| text(v, "id"))
            .filter(|id| !id.is_empty())
        {
            Some(id) => self.rt.set_owner(&id),
            None => {
                let owner = format!("h_{}", hex::encode(self.rt.ids().bytes(4)));
                self.rt.set_owner(&owner);
                self.rt
                    .store()
                    .put(OWNER, "id", json!({ "id": owner }))
                    .await?;
            }
        }
        let since = self.rt.store().get(READS, READS_ID).await?;
        match since.as_ref().and_then(|v| number(v, "at")) {
            Some(at) => self.rt.set_reads_since(at),
            None => {
                let now = self.rt.clock().now();
                self.rt.set_reads_since(now as f64);
                self.rt
                    .store()
                    .put(READS, READS_ID, json!({ "at": now }))
                    .await?;
            }
        }
        Ok(())
    }

    /// Every project held, and the git top backfill of a local host (boot.ts:354-368).
    async fn load_projects(&self) -> Result<(), HostError> {
        for mut raw in self.rt.store().list(PROJECTS).await? {
            let path = text(&raw, "path").unwrap_or_default();
            if let Some(source) = raw.get_mut("source").and_then(Value::as_object_mut) {
                if source.get("kind").and_then(Value::as_str) == Some("folder") {
                    source
                        .entry("path".to_string())
                        .or_insert_with(|| json!(path));
                }
            }
            let project: ProjectView = parse(raw)?;
            self.rt
                .projects_held()
                .borrow_mut()
                .insert(project.id.clone(), project);
        }
        if !self.rt.local_wired() {
            return Ok(());
        }
        let wanted: Vec<ProjectView> = self
            .rt
            .projects_held()
            .borrow()
            .values()
            .filter(|p| {
                p.git.is_none()
                    && matches!(p.source, ProjectSource::Folder(_))
                    && copies_folder(&kind_for_computer(&p.computer, None))
            })
            .cloned()
            .collect();
        let jobs = wanted.into_iter().map(|project| {
            let rt = self.rt.clone();
            async move {
                let Ok(top) = rt.git_top_of(&project.path).await else {
                    return Ok(());
                };
                let mut value = serde_json::to_value(&project).unwrap_or(Value::Null);
                value["git"] = json!({ "top": top });
                rt.remember_project(parse(value)?).await
            }
        });
        futures::future::try_join_all(jobs).await?;
        Ok(())
    }

    /// `moveTranscript` for each state-file transcript; one whose files did not read stays (boot.ts:284-294).
    async fn move_transcripts(&self) -> Result<(), HostError> {
        let mut moved: Vec<String> = Vec::new();
        for record in self.rt.store().list(TRANSCRIPTS).await? {
            let id = text(&record, "workspaceId").unwrap_or_default();
            match self.rt.move_transcript(record).await {
                Ok(()) => moved.push(id),
                Err(e) => self.rt.warn(&format!(
                    "{}, so it stays in the state file for the next boot to move",
                    e.message
                )),
            }
        }
        for id in moved {
            self.rt.store().delete(TRANSCRIPTS, &id).await?;
        }
        Ok(())
    }

    /// The `sessions` documents of the live workspaces, read into the thread records and the rows (boot.ts:296-416).
    /// Returns the rows that are running (`left`) and the ones whose card is still to read (`unread`).
    async fn read_session_documents(self: &Rc<Self>) -> Result<SessionDocuments, HostError> {
        let mut out = SessionDocuments::default();
        for raw in self.rt.store().list(SESSIONS).await? {
            let ws = text(&raw, "workspaceId").unwrap_or_default();
            if !self.rt.live().borrow().contains_key(&ws) {
                continue;
            }
            let viewed: IndexMap<String, String> = raw
                .get("viewed")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
            if !viewed.is_empty() {
                self.rt
                    .viewed_marks()
                    .borrow_mut()
                    .insert(ws.clone(), viewed);
            }
            for (thread_id, held) in raw
                .get("threads")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let Some(harness) = text(held, "harness") else {
                    continue;
                };
                self.store_thread_record(&ws, thread_id, held, harness)?;
            }
            let Some(rows) = raw.get("sessions").and_then(Value::as_array) else {
                self.rt.warn(&format!(
                    "sessions document for {ws} has no rows array, read as empty"
                ));
                continue;
            };
            for row in rows {
                self.read_session_row(&ws, row, &mut out)?;
            }
        }
        Ok(out)
    }

    /// One thread record of a sessions document, read back with the shape the start keeps (boot.ts:325-352).
    fn store_thread_record(
        self: &Rc<Self>,
        ws: &str,
        thread_id: &str,
        held: &Value,
        harness: String,
    ) -> Result<(), HostError> {
        let mut record = Map::new();
        record.insert("workspaceId".into(), json!(ws));
        record.insert("harness".into(), json!(harness));
        for key in ["permissionMode", "parentThreadId", "rootThreadId"] {
            if let Some(v) = text(held, key) {
                record.insert(key.into(), json!(v));
            }
        }
        for key in [
            "readAt",
            "settledAt",
            "settleNamedAt",
            "pinnedAt",
            "order",
            "foldedAt",
            "snoozedUntil",
        ] {
            if let Some(v) = number(held, key) {
                record.insert(key.into(), json!(v));
            }
        }
        if let Some(v) = text(held, "replaces") {
            record.insert("replaces".into(), json!(v));
        }
        if let Some(section) = held
            .get("section")
            .and_then(|s| parse::<wsp_wire::views::session::ThreadPlacement>(s.clone()).ok())
        {
            record.insert(
                "section".into(),
                serde_json::to_value(section).unwrap_or(Value::Null),
            );
        }
        if let Some(owed) = held
            .get("resumeAt")
            .and_then(Value::as_str)
            .or_else(|| held.get("cutOwed").and_then(Value::as_str))
        {
            record.insert("cutOwed".into(), json!(owed));
        }
        if let Some(from) = held
            .get("forkedFrom")
            .and_then(|f| parse::<ForkedFrom>(f.clone()).ok())
        {
            record.insert(
                "forkedFrom".into(),
                serde_json::to_value(from).unwrap_or(Value::Null),
            );
        }
        if let (Some(at), Some(turn)) = (
            held.get("limitResume").and_then(|l| number(l, "at")),
            held.get("limitResume").and_then(|l| text(l, "turnId")),
        ) {
            record.insert("limitResume".into(), json!({ "at": at, "turnId": turn }));
        }
        if let (Some(before), Some(at)) = (
            held.get("rewound").and_then(|r| text(r, "before")),
            held.get("rewound").and_then(|r| number(r, "at")),
        ) {
            record.insert("rewound".into(), json!({ "before": before, "at": at }));
        }
        if let Some(worked) = held.get("worked").and_then(Value::as_bool) {
            record.insert("worked".into(), json!(worked));
        }
        if let Some(ended) = held
            .get("ended")
            .and_then(|e| parse::<TurnStatus>(e.clone()).ok())
        {
            record.insert(
                "ended".into(),
                serde_json::to_value(ended).unwrap_or(Value::Null),
            );
        }
        let on: ThreadOn = parse(Value::Object(record))?;
        let snoozed = on.record.snoozed_until.as_ref().map(|j| j.0);
        let armed = on.record.limit_resume.as_ref().map(|l| l.at.0);
        self.rt
            .thread_records()
            .borrow_mut()
            .insert(thread_id.to_string(), on);
        if let Some(until) = snoozed {
            self.rt.wake_at(thread_id, until);
        }
        if let Some(at) = armed {
            self.rt.resume_on_reset(thread_id, at);
        }
        Ok(())
    }

    /// One row of a sessions document: its view, its turn's fields, and where it goes (boot.ts:383-446).
    fn read_session_row(
        self: &Rc<Self>,
        ws: &str,
        row: &Value,
        out: &mut SessionDocuments,
    ) -> Result<(), HostError> {
        let Some(obj) = row.as_object() else {
            return Ok(());
        };
        let turn_id = text(row, "turnId").unwrap_or_default();
        let mut view_obj = obj.clone();
        for key in [
            "turnId",
            "notify",
            "notifyBy",
            "notifyRoad",
            "reply",
            "told",
            "toldLast",
            "steered",
            "waited",
            "waitingSince",
            "run",
            "from",
            "asked",
            "turnToken",
            "scopeDeviceId",
            "snapshot",
        ] {
            view_obj.remove(key);
        }
        if let Some(prompt) = text(row, "prompt") {
            if prompt.encode_utf16().count() > wsp_words::words::turn::OPENING_KEPT {
                view_obj.insert(
                    "prompt".into(),
                    json!(wsp_words::words::turn::kept_opening(&prompt)),
                );
                out.recut.insert(ws.to_string());
            }
        }
        let view: SessionView = parse(Value::Object(view_obj))?;
        let notify = match row.get("notify") {
            Some(Value::String(one)) => Some(vec![one.clone()]),
            Some(Value::Array(many)) => Some(
                many.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect(),
            ),
            _ => None,
        };
        let steered = row.get("steered").and_then(read_steered);
        let has_live = row.get("reply").is_some()
            || number(row, "told").is_some()
            || steered.is_some()
            || number(row, "waited").is_some()
            || number(row, "waitingSince").is_some();
        let from = number(row, "from")
            .filter(|f| f.fract() == 0.0 && *f >= 0.0 && *f <= 9_007_199_254_740_991.0)
            .map(|f| f as u64);
        let turn_live = has_live.then(|| TurnLive {
            reply: row
                .get("reply")
                .and_then(|r| parse::<TurnStatus>(r.clone()).ok()),
            told: number(row, "told").map(|t| t as u64),
            told_as: None,
            steered,
            waited: number(row, "waited").map(JsNumber),
            waiting_since: number(row, "waitingSince").map(JsNumber),
            failure: None,
        });
        let entry = SessionEntry {
            view: view.clone(),
            turn_id: turn_id.clone(),
            replaces: None,
            notify,
            notify_by: read_scope(row.get("notifyBy")),
            notify_road: read_road(row.get("notifyRoad")),
            turn_token: text(row, "turnToken"),
            scope_device_id: text(row, "scopeDeviceId"),
            handle: None,
            end: None,
            turn_live,
            run: text(row, "run"),
            from,
            asked: row.get("asked").and_then(read_asked),
            snapshot: text(row, "snapshot"),
            pid: None,
            launch: None,
            calls: None,
            unanswered: false,
            queued: false,
        };
        let running = view.status == wsp_wire::views::session::SessionStatus::Running;
        let snapshot = entry.snapshot.is_some();
        let row_rc: Row = Rc::new(RefCell::new(entry));
        let weak: Weak<RefCell<SessionEntry>> = Rc::downgrade(&row_rc);
        let area = self.clone();
        row_rc.borrow_mut().end = Some(Rc::new(
            move |reason: &str, stopped: Option<bool>, unreached: Option<bool>| {
                let Some(row) = weak.upgrade() else { return };
                if row.borrow().view.status != wsp_wire::views::session::SessionStatus::Running {
                    return;
                }
                let ws = row.borrow().view.workspace_id.clone();
                let line = reason.to_string();
                area.rt
                    .settle_cut(&row, reason, &move |_| line.clone(), stopped, unreached);
                area.persist_later(ws);
            },
        ));
        if running {
            out.left.push(row_rc.clone());
        } else if snapshot {
            out.unread.push(row_rc.clone());
        }
        self.rt
            .sessions()
            .borrow_mut()
            .insert(view.id.clone(), row_rc);
        Ok(())
    }

    /// The late loads begun for the records the load held (boot.ts:418).
    fn begin_late_loads(
        self: &Rc<Self>,
        load: &Rc<LoadReads>,
    ) -> IndexMap<String, futures::future::Shared<LocalBoxFuture<'static, Option<Entry>>>> {
        let late: Vec<(String, Asked)> = load
            .late
            .borrow()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        late.into_iter()
            .map(|(id, read)| {
                let area = self.clone();
                let key = id.clone();
                let fut = async move { area.load_late(&id, read).await }
                    .boxed_local()
                    .shared();
                (key, fut)
            })
            .collect()
    }

    /// Each unread card, read once its workspace's late load lands (boot.ts:421-440).
    fn finish_cards(
        self: &Rc<Self>,
        sessions: &SessionDocuments,
        landing: &IndexMap<String, futures::future::Shared<LocalBoxFuture<'static, Option<Entry>>>>,
    ) -> Vec<LocalBoxFuture<'static, ()>> {
        let mut cards = Vec::new();
        for row in &sessions.unread {
            let (ws, thread, cwd, claude, id, started, turn, snapshot) = {
                let e = row.borrow();
                (
                    e.view.workspace_id.clone(),
                    e.view.thread_id.clone(),
                    e.view.cwd.clone(),
                    e.view.claude_session_id.clone(),
                    e.view.id.clone(),
                    e.view.started_at.as_ref().map(|t| t.0),
                    e.turn_id.clone(),
                    e.snapshot.clone().unwrap_or_default(),
                )
            };
            let landed = landing.get(&ws).cloned();
            let area = self.clone();
            let row = row.clone();
            let line = format!("what {turn} changed was not read");
            let card = async move {
                if let Some(landed) = landed {
                    landed.await;
                }
                if let Err(e) = read_card(
                    &area,
                    &ws,
                    &row,
                    CardOf {
                        thread,
                        cwd,
                        claude,
                        id,
                        turn,
                        started,
                        snapshot,
                    },
                )
                .await
                {
                    area.rt.warn(&format!("{line}: {}", e.message));
                }
            };
            cards.push(card.boxed_local());
        }
        cards
    }

    /// The per-workspace work of boot.ts:442-473: its late load, the worktree mount, the re-open, the sweep and the
    /// daemon sync, held in `bootWork` until it is done.
    fn start_workspace_work(
        self: &Rc<Self>,
        sessions: &mut SessionDocuments,
        landing: &IndexMap<String, futures::future::Shared<LocalBoxFuture<'static, Option<Entry>>>>,
        to_sync: &[Entry],
    ) -> Vec<futures::future::Shared<LocalBoxFuture<'static, ()>>> {
        let mut started = Vec::new();
        let held_now: Vec<Entry> = self.rt.live().borrow().values().cloned().collect();
        for held in held_now {
            let id = held.borrow().record.borrow().id.clone();
            let late = landing.get(&id).cloned();
            let left: Vec<Row> = sessions
                .left
                .iter()
                .filter(|s| s.borrow().view.workspace_id == id)
                .cloned()
                .collect();
            let synced = to_sync.iter().any(|e| Rc::ptr_eq(e, &held));
            let area = self.clone();
            let work = {
                let id = id.clone();
                let held = held.clone();
                async move {
                    let (entry, synced_now) = match &late {
                        None => {
                            tokio::task::yield_now().await;
                            (Some(held.clone()), synced)
                        }
                        Some(late) => {
                            let landed = late.clone().await;
                            let entry = landed
                                .clone()
                                .or_else(|| area.rt.live().borrow().get(&id).cloned());
                            let synced_now = landed.is_some();
                            (entry, synced_now)
                        }
                    };
                    let Some(entry) = entry else {
                        return Ok::<(), HostError>(());
                    };
                    let made =
                        serde_json::to_value(entry.borrow().record.borrow().worktree.clone())
                            .ok()
                            .and_then(|w| w.get("made").and_then(Value::as_bool))
                            == Some(true);
                    if made {
                        at_call(area.rt.worktree_mounted(&entry)).await?;
                    }
                    area.reopen(left, false).await?;
                    tokio::task::yield_now().await;
                    at_call(area.rt.sweep_runs(&entry)).await?;
                    if synced_now {
                        area.rt.sync_daemon(&entry);
                    }
                    Ok(())
                }
            };
            let area = self.clone();
            let shared: futures::future::Shared<LocalBoxFuture<'static, ()>> = async move {
                if let Err(e) = work.await {
                    area.rt.warn(&format!(
                        "the turns and runs on {id} were not settled: {}",
                        e.message
                    ));
                }
                area.rt.boot_work().borrow_mut().shift_remove(&id);
            }
            .boxed_local()
            .shared();
            let id_key = held.borrow().record.borrow().id.clone();
            self.rt
                .boot_work()
                .borrow_mut()
                .insert(id_key, shared.clone());
            started.push(shared);
        }
        started
    }

    /// The held starts a restart ends with the restart named (boot.ts:487-507).
    async fn end_held_starts(&self) -> Result<(), HostError> {
        let held = self.rt.store().list(HELD_STARTS).await?;
        tokio::task::yield_now().await;
        for h in held {
            let turn_id = text(&h, "turnId").unwrap_or_default();
            self.rt.store().delete(HELD_STARTS, &turn_id).await?;
            tokio::task::yield_now().await;
            let ws = text(&h, "workspaceId").unwrap_or_default();
            if !self.rt.live().borrow().contains_key(&ws) {
                continue;
            }
            let reason =
                wsp_words::place_state::cap_restarted_line(&text(&h, "place").unwrap_or_default());
            let session = text(&h, "sessionId").unwrap_or_default();
            let mut view = json!({ "id": turn_id, "workspaceId": ws, "harness": text(&h, "harness").unwrap_or_default(), "status": "failed" });
            if let Some(thread) = text(&h, "threadId") {
                view["threadId"] = json!(thread);
            }
            if session != turn_id {
                view["claudeSessionId"] = json!(session);
            }
            let view: SessionView = parse(view)?;
            let notify: Option<Vec<String>> = h.get("notify").and_then(Value::as_array).map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            });
            if let Some(notify) = notify {
                let result = parse(json!({ "status": "failed", "error": reason }))?;
                self.rt.notify_end(HeldEnd {
                    view,
                    turn_id: turn_id.clone(),
                    notify,
                    notify_by: read_scope(h.get("notifyBy")),
                    notify_road: read_road(h.get("notifyRoad")),
                    result,
                });
            }
            let mut event = json!({
                "type": "session.end",
                "workspaceId": ws,
                "sessionId": session,
                "turnId": turn_id,
                "exitCode": null,
                "sawResult": false,
                "reason": reason,
                "unstarted": true,
            });
            if let Some(thread) = text(&h, "threadId") {
                event["threadId"] = json!(thread);
            }
            if let Some(prompt) = text(&h, "prompt") {
                event["prompt"] = json!(prompt);
            }
            self.rt.record(parse::<EventUnion>(event)?);
        }
        Ok(())
    }

    /// The builders listed and each admitted, waited no later than the load's deadline (boot.ts:594-616).
    async fn admit_builders(self: &Rc<Self>, load: &Rc<LoadReads>) -> Result<(), HostError> {
        let mut admissions = Vec::new();
        for raw in self.rt.store().list(BUILDERS).await? {
            let stored: StoredBuilder = parse(raw)?;
            let area = self.clone();
            admissions.push(async move {
                let id = stored.id.clone();
                if let Err(e) = area.admit_once(stored).await {
                    area.rt.warn(&format!("builder {id} was not read, so the next look at the builders reads it again: {}", e.message));
                }
            });
        }
        let admitted = futures::future::join_all(admissions);
        settle_by(&self.rt.clock(), load.until, async move {
            admitted.await;
        })
        .await;
        Ok(())
    }

    /// `reopen` (boot.ts:525), spec rule 10: the rows left running, re-opened by their run handles.
    pub async fn reopen(self: &Rc<Self>, rows: Vec<Row>, again: bool) -> Result<(), HostError> {
        for row in &rows {
            row.borrow_mut().unanswered = false;
        }
        let answers = futures::future::join_all(rows.iter().map(|row| {
            let area = self.clone();
            let row = row.clone();
            async move {
                let answer = at_call(area.rt.reattach(&row, again)).await;
                (row, answer)
            }
        }))
        .await;
        for _ in 0..if answers.is_empty() { 1 } else { 2 } {
            tokio::task::yield_now().await;
        }
        let mut cut: IndexSet<String> = IndexSet::new();
        for (row, answer) in &answers {
            let (started, thread) = {
                let e = row.borrow();
                (
                    e.view.started_at.as_ref().map(|t| t.0),
                    e.view.thread_id.clone(),
                )
            };
            match answer {
                Reopened::Cannot => {
                    self.rt.settle_cut(
                        row,
                        RESTARTED_REASON,
                        &move |ended| restart_cut_line(ended - started.unwrap_or(ended)),
                        None,
                        None,
                    );
                }
                Reopened::Gone => {
                    self.rt.settle_cut(
                        row,
                        RUN_GONE_LINE,
                        &|_| RUN_GONE_LINE.to_string(),
                        None,
                        None,
                    );
                }
                Reopened::Attached | Reopened::Unreached => {}
            }
            if matches!(answer, Reopened::Cannot | Reopened::Gone) {
                if let Some(thread) = thread {
                    cut.insert(thread);
                }
            }
        }
        let persisted: IndexSet<String> = answers
            .iter()
            .filter(|(_, a)| *a != Reopened::Unreached)
            .map(|(r, _)| r.borrow().view.workspace_id.clone())
            .collect();
        for ws in persisted {
            self.persist_later(ws);
        }
        let unanswered: Vec<Row> = answers
            .iter()
            .filter(|(_, a)| *a == Reopened::Unreached)
            .map(|(r, _)| r.clone())
            .collect();
        for row in &unanswered {
            row.borrow_mut().unanswered = true;
        }
        if !unanswered.is_empty() {
            self.ask_again(unanswered);
        }
        if cut.is_empty() {
            return Ok(());
        }
        let stale: Vec<ScopedDevice> = at_call(self.rt.devices())
            .await?
            .into_iter()
            .filter(|d| {
                d.thread_id
                    .as_ref()
                    .is_some_and(|t| cut.contains(t) && !self.rt.thread_runs(t))
            })
            .collect();
        if !stale.is_empty() {
            self.revoke_but_shells(&stale).await?;
        }
        Ok(())
    }

    /// `askAgain` (boot.ts:548): the rows asked again one link window on.
    pub fn ask_again(self: &Rc<Self>, rows: Vec<Row>) {
        let area = self.clone();
        self.rt.clock().schedule(
            Box::new(move || {
                if area.rt.closing() {
                    return;
                }
                let still: Vec<Row> = rows
                    .into_iter()
                    .filter(|s| {
                        let (id, running) = {
                            let e = s.borrow();
                            (
                                e.view.id.clone(),
                                e.view.status == wsp_wire::views::session::SessionStatus::Running,
                            )
                        };
                        running
                            && area
                                .rt
                                .sessions()
                                .borrow()
                                .get(&id)
                                .is_some_and(|held| Rc::ptr_eq(held, s))
                    })
                    .collect();
                let (ready, waiting): (Vec<Row>, Vec<Row>) = still.into_iter().partition(|s| {
                    let ws = s.borrow().view.workspace_id.clone();
                    area.rt.live().borrow().get(&ws).is_some_and(|e| {
                        e.borrow().record.borrow().phase
                            == wsp_wire::views::workspace::WorkspacePhase::Running
                    })
                });
                if !waiting.is_empty() {
                    area.ask_again(waiting);
                }
                if !ready.is_empty() {
                    let again = area.clone();
                    tokio::task::spawn_local(async move {
                        if let Err(e) = again.reopen(ready, true).await {
                            again.rt.warn(&format!(
                                "the turns left running were not asked after again: {}",
                                e.message
                            ));
                        }
                    });
                }
            }),
            wsp_wire::wire::limits::LINK_RETRY_WINDOW_MS,
        );
    }
}

/// The fields a card read is asked with: its row's thread, cwd, session, turn, start and snapshot.
struct CardOf {
    thread: Option<String>,
    cwd: Option<String>,
    claude: Option<String>,
    id: String,
    turn: String,
    started: Option<f64>,
    snapshot: String,
}

/// One unread card's read: the turn's changes are read unless the turn is already written, then the snapshot goes
/// (boot.ts:421-440).
async fn read_card(
    area: &Rc<BootArea>,
    ws: &str,
    row: &Row,
    card: CardOf,
) -> Result<(), HostError> {
    let CardOf {
        thread,
        cwd,
        claude,
        id,
        turn,
        started,
        snapshot,
    } = card;
    let Some(entry) = area.rt.live().borrow().get(ws).cloned() else {
        return Ok(());
    };
    let read = match (thread, cwd) {
        (Some(thread), Some(cwd)) => {
            if area.rt.turn_changes_written(ws, &thread, &turn).await? {
                true
            } else {
                let ask = CardAsk {
                    session_id: claude.unwrap_or(id),
                    turn_id: turn.clone(),
                    thread_id: thread,
                    cwd,
                    from: snapshot,
                    started_at: started.unwrap_or_else(|| area.rt.clock().now() as f64),
                };
                area.rt.read_turn_changes(&entry, ask).await?
            }
        }
        _ => true,
    };
    row.borrow_mut().snapshot = None;
    if read || !area.rt.closing() {
        area.rt.persist_sessions(ws).await;
    }
    Ok(())
}

/// Starts several background calls the way the TS starts calls it does not await: they and the start's own steps are
/// polled in the order given each turn, as one microtask queue runs them, and what is left when the start ends goes on
/// as one task.
async fn start_all(
    futs: Vec<LocalBoxFuture<'static, ()>>,
    mut tail: LocalBoxFuture<'static, Result<(), HostError>>,
) -> Result<(), HostError> {
    let mut live: Vec<Option<LocalBoxFuture<'static, ()>>> = futs.into_iter().map(Some).collect();
    let out = futures::future::poll_fn(|cx| {
        for slot in live.iter_mut() {
            if slot
                .as_mut()
                .is_some_and(|fut| fut.as_mut().poll(cx).is_ready())
            {
                *slot = None;
            }
        }
        tail.as_mut().poll(cx)
    })
    .await;
    if live.iter().any(Option::is_some) {
        tokio::task::spawn_local(futures::future::poll_fn(move |cx| {
            for slot in live.iter_mut() {
                if slot
                    .as_mut()
                    .is_some_and(|fut| fut.as_mut().poll(cx).is_ready())
                {
                    *slot = None;
                }
            }
            if live.iter().all(Option::is_none) {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        }));
    }
    out
}

/// The rows a start reads out of the sessions documents: the ones running and the ones whose card is still read.
#[derive(Default)]
pub(crate) struct SessionDocuments {
    pub(crate) left: Vec<Row>,
    pub(crate) unread: Vec<Row>,
    pub(crate) recut: IndexSet<String>,
}
