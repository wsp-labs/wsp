// SPDX-License-Identifier: AGPL-3.0-only
//! threads_owed: lines into threads, kept until a turn takes them, and the cut end. threads.ts:24-62, 586-955.
//! Spec: spec/threads_owed.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::{pending, select_all, FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wsp_core::console::warn;
use wsp_core::types::api::StartOptions;
use wsp_core::types::events::{EventListener, Unlisten};
use wsp_core::types::internal::{read_road, read_scope, write_lines, ThreadStamps, ATTACHMENTS};
use wsp_core::types::wiring::SessionHandle;
use wsp_frames::True;
use wsp_wire::types::attachments::{Attachment, AttachmentRecord};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::session::{SessionOrigin, SessionStatus, SessionView};
use wsp_wire::views::session_events::{SessionDoneEvent, SessionEndEvent, SessionNotifyEvent};
use wsp_wire::views::session_events::{TurnResult, TurnStatus};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_wire::views::workspace::Caller;
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::events_union::EventUnion;
use wsp_wire::wire::runtime::{SessionStartOutcome, DEVICE_OPS};
use wsp_words::attachments::attachment_key;
use wsp_words::exit::HostError;
use wsp_words::words::turn::{notify_line, turn_lines, unread_line, NotifyLength};
use wsp_words::workspace_state::{send_refusal, workspace_state, SendRefusalKind, WorkspaceStateInput};

use super::tree::{computer_silent, TellAs};
use super::{Row, ThreadsArea};

pub use wsp_core::types::internal::{NOTIFY_OWED, OWED_FOR_MS, OWED_RETRY_MAX_MS, OWED_RETRY_MS};
pub use wsp_wire::views::session_events::NOTIFY_ME;
/// threads.ts:25 `PersonRow`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonRow {
    pub workspace_id: String,
    pub session_id: String,
    pub turn_id: String,
    pub text: String,
}

/// threads.ts:33 `OwedImages`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OwedImages {
    pub under: String,
    pub records: Vec<AttachmentRecord>,
}

/// threads.ts:31 `Owed`: O1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Owed {
    pub id: String,
    pub from: String,
    pub notify: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by: Option<ThreadScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub road: Option<WorkspaceOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_by: Option<SessionOrigin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_person: Option<PersonRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tries: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub images: Option<OwedImages>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub awaits_result: Option<String>,
}

/// threads.ts:45 `readOwed`: O1.
pub fn read_owed(raw: &Value) -> Option<Owed> {
    let text_of = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_string);
    Some(Owed {
        id: text_of("id")?,
        from: text_of("from")?,
        notify: text_of("notify")?,
        text: text_of("text")?,
        by: read_scope(raw.get("by")),
        road: read_road(raw.get("road")),
        started_by: raw.get("startedBy").and_then(|v| serde_json::from_value::<SessionOrigin>(v.clone()).ok()),
        to_person: raw.get("toPerson").and_then(read_person_row),
        since: raw.get("since").and_then(Value::as_i64),
        request_id: text_of("requestId"),
        tries: raw.get("tries").and_then(Value::as_u64).and_then(|t| u32::try_from(t).ok()),
        next: raw.get("next").and_then(Value::as_i64),
        images: raw.get("images").and_then(read_owed_images),
        awaits_result: text_of("awaitsResult"),
    })
}

/// threads.ts:31 `toPerson`: all four strings, else dropped.
fn read_person_row(raw: &Value) -> Option<PersonRow> {
    let text_of = |key: &str| raw.get(key).and_then(Value::as_str).map(str::to_string);
    Some(PersonRow {
        workspace_id: text_of("workspaceId")?,
        session_id: text_of("sessionId")?,
        turn_id: text_of("turnId")?,
        text: text_of("text")?,
    })
}

/// threads.ts:24 `readOwedImages`: a string `under` and a non-empty list of records.
fn read_owed_images(raw: &Value) -> Option<OwedImages> {
    let under = raw.get("under")?.as_str()?.to_string();
    let records = serde_json::from_value::<Vec<AttachmentRecord>>(raw.get("records")?.clone()).ok()?;
    (!records.is_empty()).then_some(OwedImages { under, records })
}

/// The handle a try is settled on: the row id and turn id, and its finish where it has one (threads.ts:745).
#[derive(Clone)]
pub struct TryHandle {
    pub id: String,
    pub turn_id: String,
    pub finished: Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, TurnResult>>>,
    pub tells_unread: Option<bool>,
}

impl TryHandle {
    /// A handle a start answered.
    pub fn of(handle: &dyn SessionHandle) -> TryHandle {
        TryHandle { id: handle.id(), turn_id: handle.turn_id(), finished: Some(handle.finished()), tells_unread: handle.tells_unread() }
    }
}

/// The first eight characters of an id, as the warnings cut them (`slice(0, 8)`).
fn cut8(text: &str) -> &str {
    text.char_indices().nth(8).map_or(text, |(at, _)| &text[..at])
}

fn status_word(status: TurnStatus) -> &'static str {
    match status {
        TurnStatus::Completed => "completed",
        TurnStatus::Interrupted => "interrupted",
        TurnStatus::Failed => "failed",
    }
}

fn turn_result(status: TurnStatus, error: Option<String>, unreached: bool) -> TurnResult {
    TurnResult {
        status,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error,
        unreached: unreached.then_some(True),
        refusal: None,
        limit: None,
    }
}

fn session_status_of(status: TurnStatus) -> SessionStatus {
    match status {
        TurnStatus::Completed => SessionStatus::Completed,
        TurnStatus::Interrupted => SessionStatus::Interrupted,
        TurnStatus::Failed => SessionStatus::Failed,
    }
}

fn turn_status_of(status: SessionStatus) -> TurnStatus {
    match status {
        SessionStatus::Running | SessionStatus::Completed => TurnStatus::Completed,
        SessionStatus::Interrupted => TurnStatus::Interrupted,
        SessionStatus::Failed => TurnStatus::Failed,
    }
}

/// The `session.notify` event a line records (O12, O17, O19).
fn notify_event(workspace_id: &str, session_id: &str, turn_id: &str, thread_id: &str, notify: &str, text: &str) -> SessionEvent {
    SessionEvent::SessionNotifyEvent(SessionNotifyEvent {
        workspace_id: workspace_id.into(),
        session_id: session_id.into(),
        at: None,
        turn_id: Some(turn_id.into()),
        thread_id: Some(thread_id.into()),
        pos: None,
        copied_from: None,
        notify: notify.into(),
        text: text.into(),
    })
}

fn bare_line(id: String, from: &str, notify: &str, text: String) -> Owed {
    Owed {
        id,
        from: from.into(),
        notify: notify.into(),
        text,
        by: None,
        road: None,
        started_by: None,
        to_person: None,
        since: None,
        request_id: None,
        tries: None,
        next: None,
        images: None,
        awaits_result: None,
    }
}

/// Whether `sent` is the line the area holds under its id, and the host is not closing (threads.ts:685).
fn carried_by(area: &ThreadsArea, sent: &Rc<RefCell<Owed>>) -> bool {
    !area.ctx.closing() && is_held(area, sent)
}

fn is_held(area: &ThreadsArea, sent: &Rc<RefCell<Owed>>) -> bool {
    let id = sent.borrow().id.clone();
    area.sending.borrow().get(&id).is_some_and(|held| Rc::ptr_eq(held, sent))
}

/// Takes a line out of the sending map and the store, then tells the person what it carried (O10).
fn fall(area: &ThreadsArea, sent: &Rc<RefCell<Owed>>) {
    let owed = sent.borrow().clone();
    area.owed_taken(&owed);
    area.tell_person(&owed);
}

/// One line's delivery: the line as it was handed in, the copy being sent, and the parent it goes to (threads.ts:677).
struct Delivery {
    area: Rc<ThreadsArea>,
    line: Owed,
    sent: Rc<RefCell<Owed>>,
    since: i64,
    request_id: String,
    deadline: i64,
    parent_workspace: String,
    parent_harness: String,
    woke: bool,
}

impl Delivery {
    fn carried(&self) -> bool {
        carried_by(&self.area, &self.sent)
    }
    fn fell(&self) {
        fall(&self.area, &self.sent);
    }
    /// O11.
    fn taken(&self) {
        if !is_held(&self.area, &self.sent) {
            return;
        }
        let owed = self.sent.borrow().clone();
        self.area.owed_taken(&owed);
        if self.line.started_by.is_some() {
            return;
        }
        let mut stamps = ThreadStamps::new();
        stamps.insert("readAt".into(), json!(self.area.ctx.clock().now()));
        let marking = self.area.ctx.mark(vec![self.line.from.clone()], stamps, None);
        let from = cut8(&self.line.from).to_string();
        tokio::task::spawn_local(async move {
            if let Err(e) = marking.await {
                warn(&format!("thread {from} was not marked read: {}", e.message));
            }
        });
    }
    /// O4: holds the line for the parent's wake where its workspace refuses a send now.
    fn hold_for_wake(&self) -> bool {
        let phase = self.area.ctx.entry(&self.parent_workspace).map(|entry| {
            let phase = entry.borrow().record.borrow().phase.clone();
            phase
        });
        let Some(phase) = phase else {
            return false;
        };
        let state = workspace_state(&WorkspaceStateInput { phase, machine_state: None, reach: None, wake_refused: None });
        if send_refusal(&SendRefusalKind::State(state), None, None).is_none() {
            return false;
        }
        self.area.held_lines.borrow_mut().entry(self.parent_workspace.clone()).or_default().push(self.sent.clone());
        true
    }
    /// O9: gives up at the hour, else goes again on the clock, spaced out as the tries keep failing.
    fn try_again(&self, said: &str) {
        let now = self.area.ctx.clock().now();
        let left = self.deadline - now;
        if left <= 0 {
            return self.give_up(said);
        }
        let tries = {
            let mut sent = self.sent.borrow_mut();
            let tries = sent.tries.unwrap_or(0) + 1;
            sent.tries = Some(tries);
            tries
        };
        let doubled = 2_i64.checked_pow(tries - 1).unwrap_or(i64::MAX);
        let wait = (OWED_RETRY_MS as i64).saturating_mul(doubled).min(OWED_RETRY_MAX_MS as i64).min(left);
        self.sent.borrow_mut().next = Some(now + wait);
        let owed = self.sent.borrow().clone();
        tokio::task::spawn_local(self.area.keep_owed(&owed));
        warn(&format!(
            "thread {} ended, but its line did not reach thread {} yet, and goes again in {} s: {said}",
            cut8(&self.line.from),
            cut8(&self.line.notify),
            wait as f64 / 1000.0
        ));
        self.deliver_later(wait);
    }
    /// Delivers the sent copy again once `wait` ms have passed, if it is still carried.
    fn deliver_later(&self, wait: i64) {
        let (area, sent) = (self.area.clone(), self.sent.clone());
        let _ = self.area.ctx.clock().schedule(
            Box::new(move || {
                if carried_by(&area, &sent) {
                    let owed = sent.borrow().clone();
                    area.deliver(owed, false);
                }
            }),
            wait,
        );
    }
    fn give_up(&self, said: &str) {
        warn(&format!(
            "thread {} ended, but its line did not reach thread {} within {} minutes: {said}",
            cut8(&self.line.from),
            cut8(&self.line.notify),
            OWED_FOR_MS / 60_000
        ));
        self.fell();
    }
    /// O8: settles a try the parent's turn carries.
    async fn settle_try(self: Rc<Self>, handle: TryHandle, dark: bool) {
        let me = self.clone();
        self.area.prompt_waits.borrow_mut().insert(
            self.request_id.clone(),
            Rc::new(move || {
                if me.carried() {
                    me.taken();
                }
            }),
        );
        if self.area.took_request(&self.parent_workspace, &self.request_id) {
            self.taken();
        }
        self.try_end(&handle, dark).await;
        self.area.prompt_waits.borrow_mut().shift_remove(&self.request_id);
        self.area.dark_tries.borrow_mut().shift_remove(&handle.turn_id);
    }
    async fn try_end(self: &Rc<Self>, handle: &TryHandle, dark: bool) {
        let at_hour = Rc::new(Cell::new(false));
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let cut = Rc::new(RefCell::new(Some(tx)));
        if dark {
            let cut = cut.clone();
            self.area.dark_tries.borrow_mut().insert(
                handle.turn_id.clone(),
                Rc::new(move || {
                    if let Some(tx) = cut.borrow_mut().take() {
                        let _ = tx.send(turn_result(TurnStatus::Interrupted, None, false));
                    }
                }),
            );
        }
        let left = self.deadline - self.area.ctx.clock().now();
        let stop = if left > 0 || dark {
            let me = self.clone();
            let at_hour = at_hour.clone();
            let cut = cut.clone();
            let row_id = handle.id.clone();
            Some(self.area.ctx.clock().schedule(
                Box::new(move || {
                    if !me.carried() || me.area.took_request(&me.parent_workspace, &me.request_id) {
                        return;
                    }
                    at_hour.set(true);
                    if dark {
                        if let Some(tx) = cut.borrow_mut().take() {
                            let said = "nothing answered for the turn carrying it since the host restarted".to_string();
                            let _ = tx.send(turn_result(TurnStatus::Failed, Some(said), false));
                        }
                        return;
                    }
                    let stopping = me.area.ctx.stop_try(&row_id);
                    let (from, notify) = (cut8(&me.line.from).to_string(), cut8(&me.line.notify).to_string());
                    tokio::task::spawn_local(async move {
                        if let Err(e) = stopping.await {
                            warn(&format!("the try of the line of thread {from} into thread {notify} was not stopped: {}", e.message));
                        }
                    });
                }),
                left.max(0),
            ))
        } else {
            None
        };
        let mut ends: Vec<LocalBoxFuture<'static, TurnResult>> = vec![cut_short(rx)];
        if let Some(finished) = &handle.finished {
            ends.push(finished.clone().boxed_local());
        }
        if let Some(row) = self.area.session_row(&handle.id) {
            if row.borrow().turn_id == handle.turn_id {
                ends.push(self.area.row_ended(row));
            }
        }
        let (ended, _, _) = select_all(ends).await;
        if let Some(stop) = stop {
            stop();
        }
        if !self.carried() {
            return;
        }
        if self.area.took_request(&self.parent_workspace, &self.request_id) {
            return self.taken();
        }
        let said = ended.error.clone().unwrap_or_else(|| format!("the turn ended {} before its agent started", status_word(ended.status)));
        if at_hour.get() {
            return self.give_up(&said);
        }
        if ended.status == TurnStatus::Interrupted {
            warn(&format!(
                "thread {} ended, but the turn carrying its line into thread {} was stopped",
                cut8(&self.line.from),
                cut8(&self.line.notify)
            ));
            return self.fell();
        }
        if self.hold_for_wake() {
            return;
        }
        if computer_silent(self.area.failure_of(&handle.id, &handle.turn_id).as_ref()) {
            return self.try_again(&said);
        }
        warn(&format!("thread {} ended, but its line did not reach thread {}: {said}", cut8(&self.line.from), cut8(&self.line.notify)));
        self.fell();
    }
    /// The task a delivery runs after its first put (O7).
    async fn run(self: Rc<Self>, kept: Option<Shared<LocalBoxFuture<'static, ()>>>, as_who: Option<Caller>) {
        if let Some(kept) = kept {
            kept.await;
        }
        let awaits = self.sent.borrow().awaits_result.clone();
        if let Some(workspace) = awaits {
            let tail = self.area.ctx.sub_line(&workspace, self.since).await;
            if !self.carried() {
                return;
            }
            if let Some(tail) = tail {
                let text = format!("{}\n{tail}", self.sent.borrow().text);
                self.sent.borrow_mut().text = text;
            }
            self.sent.borrow_mut().awaits_result = None;
            let owed = self.sent.borrow().clone();
            self.area.keep_owed(&owed).await;
        }
        if self.line.request_id.is_some() {
            let boot = self.area.ctx.boot_work(&self.parent_workspace);
            if let Some(boot) = boot {
                boot.await;
            }
            if !self.carried() {
                return;
            }
            if self.area.took_request(&self.parent_workspace, &self.request_id) {
                return self.taken();
            }
            if let Some(row_id) = self.area.carrying(&self.line.notify, &self.request_id) {
                let out = self.area.session_row(&row_id).map(|row| {
                    let r = row.borrow();
                    (
                        r.view.id.clone(),
                        r.turn_id.clone(),
                        r.asked.as_ref().and_then(|a| a.request_id.clone()),
                        r.handle.clone(),
                        r.launch.clone(),
                    )
                });
                if let Some((view_id, turn_id, asked_request, handle, launch)) = out {
                    let steered = asked_request.as_deref() != Some(self.request_id.as_str());
                    if let (false, Some(handle)) = (steered, &handle) {
                        let settle = TryHandle::of(&**handle);
                        return self.settle_try(settle, false).await;
                    }
                    if !steered && launch.is_none() {
                        return self.settle_try(TryHandle { id: view_id, turn_id, finished: None, tells_unread: None }, true).await;
                    }
                    let over = match (&handle, &launch) {
                        (Some(handle), _) => {
                            handle.finished().await;
                            true
                        }
                        (None, Some(launch)) => {
                            launch.clone().await;
                            false
                        }
                        (None, None) => {
                            let (tx, rx) = oneshot::channel::<()>();
                            let _ = self.area.ctx.clock().schedule(
                                Box::new(move || {
                                    let _ = tx.send(());
                                }),
                                OWED_RETRY_MS as i64,
                            );
                            let _ = rx.await;
                            false
                        }
                    };
                    if !self.carried() {
                        return;
                    }
                    if steered && over {
                        if handle.as_ref().and_then(|h| h.tells_unread()) == Some(true) {
                            return self.taken();
                        }
                        warn(&format!(
                            "thread {} ended, and nothing says whether thread {} read its line",
                            cut8(&self.line.from),
                            cut8(&self.line.notify)
                        ));
                        return self.fell();
                    }
                    let owed = self.sent.borrow().clone();
                    return self.area.deliver(owed, false);
                }
            }
        }
        let tries = self.sent.borrow().tries.unwrap_or(0);
        if tries > 0 && !self.woke {
            let now = self.area.ctx.clock().now();
            if now >= self.deadline {
                return self.give_up("its last try failed");
            }
            let next = self.sent.borrow().next;
            if let Some(next) = next {
                if next > now {
                    return self.deliver_later(next - now);
                }
            }
        }
        let attachments = match &self.line.images {
            Some(images) => self.area.kept_images(&self.parent_workspace, &self.line.notify, images).await,
            None => Vec::new(),
        };
        let now = self.area.ctx.clock().now();
        let options = StartOptions {
            prompt: self.sent.borrow().text.clone(),
            attachments: (!attachments.is_empty()).then_some(attachments),
            harness: Some(self.parent_harness.clone()),
            thread: Some(self.line.notify.clone()),
            started_by: Some(self.line.started_by.clone().unwrap_or(SessionOrigin::Agent)),
            request_id: Some(self.request_id.clone()),
            owed: true,
            asks_until_stopped: self.deadline > now,
            wakes_lead: self.line.started_by.is_none(),
            ..StartOptions::default()
        };
        match self.area.ctx.start(&self.parent_workspace, options, as_who).await {
            Ok(handle) => {
                if handle.outcome() == SessionStartOutcome::Steered {
                    return self.taken();
                }
                let settle = TryHandle::of(&*handle);
                self.settle_try(settle, false).await
            }
            Err(e) => {
                if self.hold_for_wake() {
                    return;
                }
                if computer_silent(Some(&e)) {
                    return self.try_again(&e.message);
                }
                warn(&format!(
                    "thread {} ended, but its line did not reach thread {}: {}",
                    cut8(&self.line.from),
                    cut8(&self.line.notify),
                    e.message
                ));
                self.fell();
            }
        }
    }
}

/// The try's cut: resolves once the cut sender fires, and never where it was dropped unfired.
fn cut_short(rx: oneshot::Receiver<TurnResult>) -> LocalBoxFuture<'static, TurnResult> {
    async move {
        match rx.await {
            Ok(result) => result,
            Err(_) => pending().await,
        }
    }
    .boxed_local()
}

/// Lowercase base64 with padding, the RFC 4648 alphabet, as `Buffer.toString("base64")` writes it.
fn base64_of(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

impl ThreadsArea {
    /// Keeps a line in the store as it stands (threads.ts:600, keepOwed).
    fn keep_owed(&self, line: &Owed) -> LocalBoxFuture<'static, ()> {
        let store = self.ctx.store();
        let id = line.id.clone();
        let value = serde_json::to_value(line).unwrap_or(Value::Null);
        let warned = format!("the line of thread {} into thread {} was not kept", cut8(&line.from), cut8(&line.notify));
        async move {
            if let Err(e) = store.put(NOTIFY_OWED, &id, value).await {
                warn(&format!("{warned}: {}", e.message));
            }
        }
        .boxed_local()
    }
    /// The line leaves the sending map and the store (threads.ts:586, owedTaken).
    fn owed_taken(&self, line: &Owed) {
        self.sending.borrow_mut().shift_remove(&line.id);
        let store = self.ctx.store();
        let id = line.id.clone();
        let warned = format!("the line of thread {} into thread {} stays owed", cut8(&line.from), cut8(&line.notify));
        tokio::task::spawn_local(async move {
            if let Err(e) = store.delete(NOTIFY_OWED, &id).await {
                warn(&format!("{warned}: {}", e.message));
            }
        });
    }
    /// The row a line's person is told through (threads.ts:603, tellPerson).
    fn tell_person(&self, line: &Owed) {
        let Some(row) = line.to_person.clone() else {
            return;
        };
        self.ctx.record(notify_event(&row.workspace_id, &row.session_id, &row.turn_id, &line.from, NOTIFY_ME, &row.text));
        let held: Vec<Rc<RefCell<Owed>>> = self.sending.borrow().values().cloned().collect();
        for other in held {
            let same = {
                let other = other.borrow();
                other.id != line.id && other.to_person.as_ref().is_some_and(|p| p.turn_id == row.turn_id && p.text == row.text)
            };
            if !same {
                continue;
            }
            other.borrow_mut().to_person = None;
            let saved = other.borrow().clone();
            tokio::task::spawn_local(self.keep_owed(&saved));
        }
    }
    fn session_row(&self, row_id: &str) -> Option<Row> {
        let sessions = self.ctx.sessions();
        let row = sessions.borrow().get(row_id).cloned();
        row
    }
    /// The end a try's row reaches for its turn (threads.ts:645, O16). The listener stays until the turn ends.
    fn row_ended(&self, row: Row) -> LocalBoxFuture<'static, TurnResult> {
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let tx = Rc::new(RefCell::new(Some(tx)));
        let stop: Rc<RefCell<Option<Unlisten>>> = Rc::default();
        let turn = row.borrow().turn_id.clone();
        let (stop_in, tx_in) = (stop.clone(), tx.clone());
        let listener: EventListener = Rc::new(move |e: &EventUnion| {
            let EventUnion::SessionEnd(end) = e else {
                return Ok(());
            };
            if end.event.turn_id.as_deref() != Some(turn.as_str()) {
                return Ok(());
            }
            let off = stop_in.borrow_mut().take();
            if let Some(off) = off {
                off();
            }
            let interrupted = row.borrow().view.status == SessionStatus::Interrupted;
            let status = if interrupted { TurnStatus::Interrupted } else { TurnStatus::Failed };
            let result = turn_result(status, end.event.reason.clone(), false);
            if let Some(tx) = tx_in.borrow_mut().take() {
                let _ = tx.send(result);
            }
            Ok(())
        });
        let off = self.ctx.bus().on("session.end", listener);
        *stop.borrow_mut() = Some(off);
        async move {
            match rx.await {
                Ok(result) => result,
                Err(_) => pending().await,
            }
        }
        .boxed_local()
    }

    /// threads.ts:677 `deliver`: O2 to O16. Synchronous up to its detached task.
    pub fn deliver(&self, line: Owed, woke: bool) {
        let Some(area) = self.me.upgrade() else {
            return;
        };
        let now = self.ctx.clock().now();
        let since = line.since.unwrap_or(now);
        let request_id = match &line.request_id {
            Some(id) => id.clone(),
            None => self.ctx.ids().uuid(),
        };
        let sent = Rc::new(RefCell::new(Owed { since: Some(since), request_id: Some(request_id.clone()), ..line.clone() }));
        self.sending.borrow_mut().insert(line.id.clone(), sent.clone());
        let deadline = since + OWED_FOR_MS as i64;
        let kept = if line.request_id.is_none() || line.since.is_none() {
            let put = self.keep_owed(&sent.borrow()).shared();
            tokio::task::spawn_local(put.clone());
            Some(put)
        } else {
            None
        };
        let parent = match self.latest_on(&line.notify) {
            Some(parent) if Self::tellable(Some(&parent)) => parent,
            _ => {
                warn(&format!("thread {} ended, but thread {} has no session to tell", cut8(&line.from), cut8(&line.notify)));
                fall(&area, &sent);
                return;
            }
        };
        let d = Rc::new(Delivery {
            area: area.clone(),
            line: line.clone(),
            sent: sent.clone(),
            since,
            request_id,
            deadline,
            parent_workspace: parent.workspace_id.clone(),
            parent_harness: parent.harness.clone(),
            woke,
        });
        if d.hold_for_wake() {
            return;
        }
        if line.road == Some(WorkspaceOrigin::Paired) && !DEVICE_OPS.contains(&"sessions.start") {
            warn(&format!(
                "thread {} ended, but its line was registered from a paired computer, which starts no turn on thread {}",
                cut8(&line.from),
                cut8(&line.notify)
            ));
            d.fell();
            return;
        }
        let as_who = match &line.by {
            None => line.road.clone().map(Caller::Road),
            Some(by) => Some(Caller::Scoped { origin: line.road.clone().unwrap_or(WorkspaceOrigin::Here), by: by.clone() }),
        };
        tokio::task::spawn_local(d.run(kept, as_who));
    }
    /// threads.ts:622: O13.
    pub fn took_request(&self, workspace_id: &str, request_id: &str) -> bool {
        let Some(taken) = self.ctx.taken(workspace_id, request_id) else {
            return false;
        };
        if taken.outcome == wsp_core::types::internal::TakenOutcome::Steered {
            return true;
        }
        let sessions = self.ctx.sessions();
        let sessions = sessions.borrow();
        !sessions.values().any(|row| {
            let row = row.borrow();
            row.turn_id == taken.turn_id && row.asked.as_ref().and_then(|a| a.awaits_prompt) == Some(true)
        })
    }
    /// threads.ts:629: O13. The row's id names it in `sessions`.
    pub fn line_try(&self, row_id: &str) -> bool {
        let Some(row) = self.session_row(row_id) else {
            return false;
        };
        let (request, workspace_id, turn_id, awaits) = {
            let r = row.borrow();
            let Some(asked) = r.asked.as_ref() else {
                return false;
            };
            if asked.owed != Some(true) {
                return false;
            }
            let Some(request) = asked.request_id.clone() else {
                return false;
            };
            (request, r.view.workspace_id.clone(), r.turn_id.clone(), asked.awaits_prompt == Some(true))
        };
        let taken_turn = self.ctx.taken(&workspace_id, &request).map(|t| t.turn_id);
        taken_turn.as_deref() != Some(turn_id.as_str()) || awaits
    }
    /// threads.ts:637: a try's agent now holds the request's prompt.
    pub fn prompt_held(&self, request_id: &str) {
        let wait = self.prompt_waits.borrow().get(request_id).cloned();
        if let Some(wait) = wait {
            wait();
        }
    }
    /// threads.ts:641: the person stopped a dark try's turn.
    pub fn try_stopped(&self, turn_id: &str) {
        let stop = self.dark_tries.borrow().get(turn_id).cloned();
        if let Some(stop) = stop {
            stop();
        }
    }
    /// threads.ts:655: O14. The row id.
    pub fn carrying(&self, thread_id: &str, request_id: &str) -> Option<String> {
        let sessions = self.ctx.sessions();
        let sessions = sessions.borrow();
        sessions.iter().find_map(|(id, row)| {
            let s = row.borrow();
            let asked = s.asked.as_ref().and_then(|a| a.request_id.as_deref()) == Some(request_id);
            let steered = s
                .turn_live
                .as_ref()
                .and_then(|l| l.steered.as_ref())
                .is_some_and(|m| m.values().any(|st| st.request_id.as_deref() == Some(request_id)));
            let hit = s.view.thread_id.as_deref() == Some(thread_id) && s.view.status == SessionStatus::Running && (asked || steered);
            hit.then(|| id.clone())
        })
    }
    /// threads.ts:852: O15.
    pub async fn kept_images(&self, workspace_id: &str, thread_id: &str, images: &OwedImages) -> Vec<Attachment> {
        let writes = self.ctx.kept_writes(workspace_id);
        if let Some(writes) = writes {
            writes.await;
        }
        let store = self.ctx.store();
        let mut out = Vec::new();
        for (index, record) in images.records.iter().enumerate() {
            let Some(key) = attachment_key(thread_id, Some(&images.under), index as f64) else {
                continue;
            };
            let Some(bytes) = store.get_blob(ATTACHMENTS, &key).await.ok().flatten() else {
                continue;
            };
            out.push(Attachment { media_type: record.media_type.clone(), bytes: base64_of(&bytes), name: record.name.clone() });
        }
        out
    }
    /// threads.ts:870: O17.
    pub fn send_back(&self, s: &Row, ids: &[String], stopped: bool) {
        let (thread_id, workspace_id, session_id, turn_id, turn_live) = {
            let r = s.borrow();
            let Some(thread_id) = r.view.thread_id.clone() else {
                return;
            };
            let session_id = r.view.claude_session_id.clone().unwrap_or_else(|| r.view.id.clone());
            (thread_id, r.view.workspace_id.clone(), session_id, r.turn_id.clone(), r.turn_live.clone())
        };
        for id in ids {
            {
                let mut outs = self.steers_out.borrow_mut();
                if let Some(out) = outs.get_mut(id) {
                    out.ended = Some((s.clone(), stopped));
                    continue;
                }
            }
            let steered = turn_live.as_ref().and_then(|l| l.steered.as_ref()).and_then(|m| m.get(id)).cloned();
            let Some(steered) = steered else {
                continue;
            };
            let to_person = PersonRow {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                turn_id: turn_id.clone(),
                text: unread_line(&steered.prompt),
            };
            if stopped {
                self.ctx.record(notify_event(&workspace_id, &session_id, &turn_id, &thread_id, NOTIFY_ME, &to_person.text));
                continue;
            }
            let images = match (steered.request_id.clone(), steered.attachments.clone()) {
                (Some(under), Some(records)) => Some(OwedImages { under, records }),
                _ => None,
            };
            let line = Owed {
                to_person: Some(to_person),
                by: steered.by.clone(),
                road: steered.road.clone(),
                started_by: Some(steered.started_by.clone()),
                images,
                text: steered.prompt.clone(),
                ..bare_line(format!("{turn_id}:unread:{id}"), &thread_id, &thread_id, String::new())
            };
            self.deliver(line, false);
        }
    }
    /// threads.ts:905: O19. `open`: a reply given over work still running.
    pub fn notify_end(&self, s: &Row, notify: &[String], named: &TellAs, result: &TurnResult, open: bool) {
        let (thread_id, workspace_id, session_id, turn_id, told) = {
            let r = s.borrow();
            let Some(thread_id) = r.view.thread_id.clone() else {
                return;
            };
            let session_id = r.view.claude_session_id.clone().unwrap_or_else(|| r.view.id.clone());
            (thread_id, r.view.workspace_id.clone(), session_id, r.turn_id.clone(), r.turn_live.as_ref().and_then(|l| l.told).unwrap_or(0))
        };
        let reachable: Vec<String> = notify
            .iter()
            .filter(|target| target.as_str() == NOTIFY_ME || Self::tellable(self.latest_on(target).as_ref()))
            .cloned()
            .collect();
        let targets: Vec<String> = if reachable.len() == notify.len() {
            notify.to_vec()
        } else {
            let mut set: IndexSet<String> = reachable.into_iter().collect();
            set.insert(NOTIFY_ME.to_string());
            set.into_iter().collect()
        };
        let to_person = PersonRow {
            workspace_id: workspace_id.clone(),
            session_id: session_id.clone(),
            turn_id: turn_id.clone(),
            text: notify_line(&thread_id, result, Some(NotifyLength::Tail)),
        };
        let sub = if !open && self.ctx.entry(&workspace_id).is_some_and(|e| e.borrow().record.borrow().sub.is_some()) {
            Some(workspace_id.clone())
        } else {
            None
        };
        let me_told = targets.iter().any(|target| target == NOTIFY_ME);
        for target in &targets {
            let length = if target == NOTIFY_ME { NotifyLength::Tail } else { NotifyLength::Whole };
            let text = notify_line(&thread_id, result, Some(length));
            self.ctx.record(notify_event(&workspace_id, &session_id, &turn_id, &thread_id, target, &text));
            if target == NOTIFY_ME {
                continue;
            }
            let line = Owed {
                by: named.by.clone(),
                road: named.road.clone(),
                to_person: if me_told { None } else { Some(to_person.clone()) },
                awaits_result: sub.clone(),
                ..bare_line(format!("{turn_id}:{told}:{target}"), &thread_id, target, text)
            };
            self.deliver(line, false);
        }
    }
    /// threads.ts:925: O20.
    pub async fn deliver_owed(&self) -> Result<(), HostError> {
        let store = self.ctx.store();
        for raw in store.list(NOTIFY_OWED).await? {
            let Some(line) = read_owed(&raw) else {
                continue;
            };
            if !self.sending.borrow().contains_key(&line.id) {
                self.deliver(line, false);
            }
        }
        Ok(())
    }
    /// threads.ts:936: O21. Changes the row `row_id` names in `sessions`; `cut_line` gets the end's time.
    pub fn settle_cut(&self, row_id: &str, reason: &str, cut_line: &dyn Fn(i64) -> String, stopped: bool, unreached: bool) {
        let Some(row) = self.session_row(row_id) else {
            return;
        };
        let reply = {
            let mut s = row.borrow_mut();
            let reply = s.turn_live.as_ref().and_then(|l| l.reply);
            s.snapshot = None;
            reply
        };
        let ended_at = self.ctx.clock().now();
        let status = match reply {
            Some(reply) => session_status_of(reply),
            None if stopped => SessionStatus::Interrupted,
            None => SessionStatus::Failed,
        };
        let (workspace_id, thread_id, session_id, turn_id, notify, awaits_prompt) = {
            let mut s = row.borrow_mut();
            s.view.status = status;
            if reply.is_none() {
                let lines = turn_lines(&turn_result(turn_status_of(status), None, false), Some(reason));
                write_lines(&mut s.view, lines.last_line, lines.failure);
            }
            let session_id = s.view.claude_session_id.clone().unwrap_or_else(|| s.view.id.clone());
            (
                s.view.workspace_id.clone(),
                s.view.thread_id.clone(),
                session_id,
                s.turn_id.clone(),
                s.notify.clone(),
                s.asked.as_ref().and_then(|a| a.awaits_prompt) == Some(true),
            )
        };
        self.ctx.port_roots_moved(&workspace_id);
        let view: SessionView = {
            let mut s = row.borrow_mut();
            s.view.ended_at = Some(JsNumber(ended_at as f64));
            s.view.clone()
        };
        if status == SessionStatus::Failed {
            self.ctx.end_snooze_for(&view);
        }
        row.borrow_mut().view.asking = None;
        if let Some(thread) = &thread_id {
            self.lead_asks.borrow_mut().shift_remove(thread);
        }
        let cut =
            turn_result(if stopped { TurnStatus::Interrupted } else { TurnStatus::Failed }, Some(cut_line(ended_at)), stopped && unreached);
        if reply.is_none() {
            if let Some(notify) = notify {
                if !self.line_try(row_id) {
                    let (by, road) = {
                        let s = row.borrow();
                        (s.notify_by.clone(), s.notify_road.clone())
                    };
                    self.notify_end(&row, &notify, &TellAs { by, road }, &cut, false);
                }
            }
        }
        if reply.is_none() && stopped {
            self.ctx.record(SessionEvent::SessionDoneEvent(SessionDoneEvent {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                at: None,
                turn_id: Some(turn_id.clone()),
                thread_id: thread_id.clone(),
                pos: None,
                copied_from: None,
                result: cut,
            }));
        }
        self.ctx.record(SessionEvent::SessionEndEvent(SessionEndEvent {
            workspace_id,
            session_id,
            at: None,
            turn_id: Some(turn_id),
            thread_id,
            pos: None,
            copied_from: None,
            exit_code: None,
            saw_result: reply.is_some() || stopped,
            reason: Some(reason.to_string()),
            unstarted: None,
            prompt: None,
            promptless: awaits_prompt.then_some(True),
        }));
    }
}
