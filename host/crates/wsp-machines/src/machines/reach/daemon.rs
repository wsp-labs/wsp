// SPDX-License-Identifier: AGPL-3.0-only
//! reach.ts:199-372, 381-474: the dial preamble, the wait on a woken daemon, the hello's version, the checkout read
//! and a lead's tree. Spec: spec/reach_daemon.md.
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use futures::FutureExt;
use serde_json::{json, Map, Value};
use tokio_util::sync::CancellationToken;

use wsp_core::clock::Cancel as Timer;
use wsp_core::reach::DaemonReach;
use wsp_core::types::events::CHECKOUT_TTL_MS;
use wsp_core::types::wiring::Joined;
use wsp_engine::machine::ExecOptions;
use wsp_wire::types::changes::Checkout;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::tree::{GitBranchCompareReply, TreeChild, TreeFact};
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::wire::daemon::{GitStatusReply, DETACHED_HEAD};
use wsp_wire::wire::daemon_events::DaemonEvent;
use wsp_words::exit::HostError;
use wsp_words::js::utf16::tail;
use wsp_words::js::js_trim;
use wsp_words::words::computer::WAKE_STOPPED;
use wsp_catalog::git_hosts::remote_host;

use super::ReachArea;
use crate::machines::area_error::{AreaError, AreaResult};
use crate::machines::machines::Entry;

/// reach.ts:212-213, 278, 319.
pub const ASK_DAEMON_MS: f64 = 5_000.0;
pub const ASK_AGAIN_MS: f64 = 500.0;
pub const START_DAEMON_AFTER_MS: f64 = 60_000.0;
pub const LOOK_EXITS_EVERY_MS: f64 = 10_000.0;

/// Stops a wait: the wake's stop, the caller's own reason, whichever is pulled first.
async fn stopped(first: Option<&CancellationToken>, second: Option<&CancellationToken>) {
    tokio::select! {
        _ = async { match first { Some(token) => token.cancelled().await, None => std::future::pending().await } } => {}
        _ = async { match second { Some(token) => token.cancelled().await, None => std::future::pending().await } } => {}
    }
}

fn stopped_error() -> AreaError {
    AreaError::Host(HostError::new(WAKE_STOPPED))
}

fn host_error(e: impl std::fmt::Display) -> AreaError {
    AreaError::Host(HostError::new(e.to_string()))
}

/// Where a wait on a daemon started a machine's daemon from, and whether the wait is over.
struct StartWatch {
    over: Cell<bool>,
    timer: RefCell<Option<Timer>>,
}

/// Where a wait on a daemon's exits looks from, and what it saw.
struct ExitWatch {
    over: Cell<bool>,
    timer: RefCell<Option<Timer>>,
    exited: RefCell<Option<String>>,
}

impl ReachArea {
    /// A future that fails once `deadline` passes on the context's clock (wiring.ts `until`, rule 2).
    async fn until<T>(&self, work: impl Future<Output = AreaResult<T>>, deadline: f64, what: &str) -> AreaResult<T> {
        let ms = (deadline - self.ctx.now()).max(0.0);
        let (fire, timer) = futures::channel::oneshot::channel::<()>();
        let cancel = self.ctx.schedule(
            ms,
            Box::new(move || {
                let _ = fire.send(());
            }),
        );
        let work = Box::pin(work);
        let timer = Box::pin(timer);
        match futures::future::select(work, timer).await {
            futures::future::Either::Left((done, _)) => {
                cancel();
                done
            }
            futures::future::Either::Right(_) => Err(AreaError::Host(HostError::new(format!("{what} timed out after {ms} ms")))),
        }
    }

    /// Fails with WAKE_STOPPED where either stop is pulled before the work settles.
    async fn or_stopped<T>(&self, work: impl Future<Output = AreaResult<T>>, quit: Option<&CancellationToken>, wake: Option<&CancellationToken>) -> AreaResult<T> {
        tokio::select! {
            biased;
            _ = stopped(quit, wake) => Err(stopped_error()),
            done = work => done,
        }
    }

    /// reach.ts:203 (rule 2). `None` where the guest holds no daemon token.
    pub(crate) async fn dial_daemon(&self, entry: &Entry, deadline: f64, on_event: Option<Rc<dyn Fn(DaemonEvent)>>, heartbeat_ms: Option<i64>) -> AreaResult<Option<Rc<dyn DaemonReach>>> {
        let (ws, machine) = {
            let held = entry.borrow();
            (Rc::clone(&held.ws), Arc::clone(&held.machine))
        };
        let reach = self.until(async { ws.daemon_reach().await.map_err(AreaError::from) }, deadline, "preview route").await?;
        let token = self.until(self.ctx.daemon_token_of(&machine), deadline, "daemon token").await?;
        let Some(token) = token else {
            return Ok(None);
        };
        let on_event: Rc<dyn Fn(DaemonEvent)> = on_event.unwrap_or_else(|| Rc::new(|_: DaemonEvent| {}));
        Ok(Some(self.ctx.connect_daemon(&reach.url, &token, on_event, heartbeat_ms)))
    }

    /// reach.ts:222 (rule 3). `None` is the daemon answering; `Some` the row's words for why not.
    pub(crate) async fn daemon_answer(&self, entry: &Entry, quit: Option<CancellationToken>) -> Option<String> {
        let (machine, wake) = {
            let held = entry.borrow();
            (Arc::clone(&held.machine), held.wake_stop.as_ref().map(|stop| stop.token().clone()))
        };
        let machine_id = machine.id().to_string();
        let answers_ms = match self.ctx.daemon_answers_ms(entry) {
            Ok(ms) => ms,
            Err(error) => return Some(format!("the daemon on {machine_id} could not be asked ({})", error.message())),
        };
        let quit = quit.as_ref();
        let wake = wake.as_ref();
        if machine.offers().daemon_answers {
            let deadline = self.ctx.now() + answers_ms;
            loop {
                let ask_ms = answers_ms.min(ASK_DAEMON_MS) as u64;
                let asked = self
                    .until(
                        self.or_stopped(async { machine.daemon_answers(Some(ask_ms)).await.map_err(AreaError::from) }, quit, wake),
                        deadline,
                        "daemon answer",
                    )
                    .await;
                match asked {
                    Ok(true) => return None,
                    Ok(false) => {}
                    Err(error) => return Some(format!("the daemon on {machine_id} could not be asked ({})", error.message())),
                }
                if self.ctx.now() >= deadline {
                    return Some(format!("nothing listens on the daemon's port inside {machine_id}"));
                }
                let pause = self.ctx.sleep(ASK_AGAIN_MS);
                if let Err(error) = self.or_stopped(async { pause.await; Ok(()) }, quit, wake).await {
                    return Some(format!("the daemon on {machine_id} could not be asked ({})", error.message()));
                }
            }
        }
        if !machine.offers().preview_url {
            return None;
        }
        let deadline = self.ctx.now() + answers_ms;
        let dialled = self.or_stopped(self.dial_daemon(entry, deadline, None, Some(answers_ms as i64)), quit, wake).await;
        let link = match dialled {
            Ok(link) => link,
            Err(error) => return Some(format!("daemon on {machine_id} did not answer within {answers_ms} ms ({})", error.message())),
        };
        let Some(link) = link else {
            let exec = self.until(async { machine.exec("true", ExecOptions::default()).await.map_err(AreaError::from) }, deadline, "guest exec");
            return match self.or_stopped(exec, quit, wake).await {
                Ok(_) => None,
                Err(error) => Some(format!("daemon on {machine_id} did not answer within {answers_ms} ms ({})", error.message())),
            };
        };
        let probed = async {
            self.until(async { link.ready().await.map_err(host_error) }, deadline, "daemon link").await?;
            self.until(async { link.request("ping", Map::new()).await.map(|_| ()).map_err(host_error) }, deadline, "daemon ping").await?;
            Ok::<(), AreaError>(())
        };
        let outcome = self.or_stopped(probed, quit, wake).await;
        link.close();
        match outcome {
            Ok(()) => None,
            Err(error) => Some(format!("daemon on {machine_id} did not answer within {answers_ms} ms ({})", error.message())),
        }
    }

    /// reach.ts:280 (rule 4).
    pub(crate) async fn starting_daemon(&self, entry: &Entry, quit: CancellationToken) -> Option<String> {
        let machine = Arc::clone(&entry.borrow().machine);
        if !machine.offers().start_daemon {
            return self.daemon_answer(entry, Some(quit)).await;
        }
        let began = self.ctx.now();
        let watch = Rc::new(StartWatch { over: Cell::new(false), timer: RefCell::new(None) });
        self.arm_start(entry, &watch, began);
        let answer = self.daemon_answer(entry, Some(quit)).await;
        watch.over.set(true);
        let timer = watch.timer.borrow_mut().take();
        if let Some(timer) = timer {
            timer();
        }
        answer
    }

    fn arm_start(&self, entry: &Entry, watch: &Rc<StartWatch>, began: f64) {
        let Some(area) = self.me.upgrade() else {
            return;
        };
        let entry = Rc::clone(entry);
        let watch_for_timer = Rc::clone(watch);
        let ctx = Rc::clone(&self.ctx);
        let timer = self.ctx.schedule(
            START_DAEMON_AFTER_MS,
            Box::new(move || {
                let machine = Arc::clone(&entry.borrow().machine);
                let (ws_id, machine_id) = (entry.borrow().record.borrow().id.clone(), machine.id().to_string());
                let waited = ((ctx.now() - began) / 1000.0).round();
                let late = format!("daemon on {machine_id} (workspace {ws_id}) had not answered {waited} s into the wait, so wsp started it");
                let start = Rc::clone(&watch_for_timer);
                let area = Rc::clone(&area);
                let entry_again = Rc::clone(&entry);
                ctx.spawn(Box::pin(async move {
                    let again = |area: &Rc<ReachArea>| {
                        if !start.over.get() {
                            area.arm_start(&entry_again, &start, began);
                        }
                    };
                    match machine.start_daemon().await {
                        Ok(read) => {
                            let tail_text = if read.exit_code == JsInt::from(0) { String::new() } else { tail(js_trim(&read.stderr), 200) };
                            let detail = if tail_text.is_empty() { String::new() } else { format!(": {tail_text}") };
                            area.ctx.warn(&format!("{late} (exit {}{detail})", read.exit_code));
                            if read.exit_code != JsInt::from(0) {
                                again(&area);
                            }
                        }
                        Err(error) => {
                            area.ctx.warn(&format!("{late}, and the start failed ({})", AreaError::from(error).message()));
                            again(&area);
                        }
                    }
                }));
            }),
        );
        *watch.timer.borrow_mut() = Some(timer);
    }

    /// reach.ts:321 (rule 5).
    pub async fn ping_daemon(&self, entry: &Entry) -> Option<String> {
        let machine = Arc::clone(&entry.borrow().machine);
        let quit = CancellationToken::new();
        let watch = Rc::new(ExitWatch { over: Cell::new(false), timer: RefCell::new(None), exited: RefCell::new(None) });
        if machine.offers().daemon_exits {
            self.look_exits(entry, &watch, &quit);
        }
        let fault = self.starting_daemon(entry, quit.clone()).await;
        watch.over.set(true);
        let timer = watch.timer.borrow_mut().take();
        if let Some(timer) = timer {
            timer();
        }
        let exited = watch.exited.borrow().clone();
        fault.map(|fault| exited.unwrap_or(fault))
    }

    fn look_exits(&self, entry: &Entry, watch: &Rc<ExitWatch>, quit: &CancellationToken) {
        let Some(area) = self.me.upgrade() else {
            return;
        };
        let entry = Rc::clone(entry);
        let watch_for_timer = Rc::clone(watch);
        let quit = quit.clone();
        let timer = self.ctx.schedule(
            LOOK_EXITS_EVERY_MS,
            Box::new(move || {
                let machine = Arc::clone(&entry.borrow().machine);
                let look = Rc::clone(&watch_for_timer);
                let area = Rc::clone(&area);
                let entry = Rc::clone(&entry);
                let quit = quit.clone();
                let spawn = area.ctx.clone();
                spawn.spawn(Box::pin(async move {
                    let said = machine.daemon_exits().await;
                    if look.over.get() {
                        return;
                    }
                    match said {
                        Ok(None) | Err(_) => area.look_exits(&entry, &look, &quit),
                        Ok(Some(said)) => {
                            *look.exited.borrow_mut() = Some(format!("the daemon on {} exits as it starts ({said})", machine.id()));
                            quit.cancel();
                        }
                    }
                }));
            }),
        );
        *watch.timer.borrow_mut() = Some(timer);
    }

    /// reach.ts:357 (rule 6).
    pub async fn hello_version(&self, entry: &Entry) -> Option<f64> {
        let machine = Arc::clone(&entry.borrow().machine);
        if !machine.offers().preview_url {
            return None;
        }
        let deadline = self.ctx.now() + self.ctx.daemon_hello_timeout_ms();
        let (announce, hello) = futures::channel::oneshot::channel::<f64>();
        let announce = RefCell::new(Some(announce));
        let on_event: Rc<dyn Fn(DaemonEvent)> = Rc::new(move |event: DaemonEvent| {
            if let DaemonEvent::DaemonHello { version, .. } = event {
                if let Some(announce) = announce.borrow_mut().take() {
                    let _ = announce.send(version.0 .0);
                }
            }
        });
        let link = match self.dial_daemon(entry, deadline, Some(on_event), None).await {
            Ok(Some(link)) => link,
            _ => return None,
        };
        let answered = self.until(async { hello.await.map_err(host_error) }, deadline, "daemon hello").await;
        link.close();
        answered.ok()
    }

    /// reach.ts:386 (rule 7).
    pub async fn read_checkout(&self, entry: &Entry, force: bool) -> Option<Checkout> {
        let held = entry.borrow().checkout.clone();
        if !force {
            if let Some(fact) = held.as_ref() {
                if self.ctx.now() - fact.read_at.0 < CHECKOUT_TTL_MS as f64 {
                    return Some(fact.clone());
                }
            }
        }
        let (phase, served) = {
            let held = entry.borrow();
            let phase = held.record.borrow().phase;
            (phase, self.ctx.served_by_its_computer(entry))
        };
        if phase != WorkspacePhase::Running && !served {
            return held;
        }
        let reading = entry.borrow().checkout_reading.clone();
        if let Some(reading) = reading {
            return reading.await.ok().flatten();
        }
        let Some(area) = self.me.upgrade() else {
            return held;
        };
        let owned = Rc::clone(entry);
        let future: LocalBoxFuture<'static, Result<Option<Checkout>, HostError>> = Box::pin(async move {
            area.read_checkout_now(&owned).await;
            owned.borrow_mut().checkout_reading = None;
            Ok(owned.borrow().checkout.clone())
        });
        let shared: Joined<Option<Checkout>> = future.shared();
        entry.borrow_mut().checkout_reading = Some(shared.clone());
        shared.await.ok().flatten()
    }

    async fn read_checkout_now(&self, entry: &Entry) {
        let cwd = {
            let held = entry.borrow();
            let record = held.record.borrow().clone();
            self.ctx.checkout_of(&record)
        };
        let frame = json!({ "op": "git.status", "cwd": cwd });
        let Ok(said) = self.ctx.with_daemon(entry, frame).await else {
            return;
        };
        let Ok(said) = serde_json::from_value::<GitStatusReply>(said) else {
            return;
        };
        let now = self.ctx.now();
        let head_is_oid = said.branch.oid.len() >= 7 && said.branch.oid.len() <= 40 && said.branch.oid.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
        let fact = Checkout {
            branch: said.branch.head.clone(),
            ahead: JsInt(said.branch.ahead),
            behind: JsInt(said.branch.behind),
            changed: JsInt(JsNumber(said.entries.iter().filter(|e| e.xy != "!!").count() as f64)),
            edits_unread: (said.edits_unread == Some(true)).then_some(true),
            counts_unknown: (said.counts_unknown == Some(true)).then_some(true),
            stashes: said.stashes,
            head: head_is_oid.then(|| said.branch.oid.clone()),
            read_at: JsNumber(now),
        };
        entry.borrow_mut().checkout = Some(fact.clone());
        let recorded = entry.borrow().record.borrow().checkout.clone();
        let moved = match recorded {
            None => true,
            Some(mut kept) => {
                kept.read_at = JsNumber(0.0);
                let mut read = fact.clone();
                read.read_at = JsNumber(0.0);
                kept != read
            }
        };
        if moved {
            entry.borrow().record.borrow_mut().checkout = Some(fact.clone());
            let record = entry.borrow().record.borrow().clone();
            let _ = self.ctx.persist(&record).await;
        }
        let _ = self.status_now(entry).await;
    }

    /// reach.ts:428 (rule 8).
    pub async fn read_tree(&self, lead: &Entry) -> Option<TreeFact> {
        let reading = lead.borrow().tree_reading.clone();
        if let Some(reading) = reading {
            return reading.await.ok().flatten();
        }
        let area = self.me.upgrade()?;
        let owned = Rc::clone(lead);
        let future: LocalBoxFuture<'static, Result<Option<TreeFact>, HostError>> = Box::pin(async move {
            let outcome = area.read_tree_now(&owned).await;
            owned.borrow_mut().tree_reading = None;
            Ok(outcome)
        });
        let shared: Joined<Option<TreeFact>> = future.shared();
        lead.borrow_mut().tree_reading = Some(shared.clone());
        shared.await.ok().flatten()
    }

    async fn read_tree_now(&self, lead: &Entry) -> Option<TreeFact> {
        let (lead_id, lead_project, lead_base) = {
            let held = lead.borrow();
            let record = held.record.borrow();
            (record.id.clone(), record.project.clone(), record.base.clone())
        };
        let children: Vec<Entry> = self
            .ctx
            .live_all()
            .into_iter()
            .filter(|child| {
                let child = child.borrow();
                let parent = child.record.borrow().parent_workspace_id.clone();
                parent.as_deref() == Some(lead_id.as_str()) && !child.creating
            })
            .collect();
        if children.is_empty() {
            if lead.borrow().tree.is_some() {
                lead.borrow_mut().tree = None;
                let _ = self.status_now(lead).await;
            }
            return None;
        }
        let project = self.ctx.project_held(&lead_project);
        let read = self.read_checkout(lead, false).await.map(|fact| fact.branch);
        let lead_branch = read.or(lead_base).or(project.base.clone()).unwrap_or(project.default_branch.clone());
        let mut rows: Vec<TreeChild> = Vec::new();
        for child in &children {
            rows.push(self.tree_child_of(child, &lead_branch).await);
        }
        let fact = TreeFact { lead_branch, children: rows, read_at: JsNumber(self.ctx.now()) };
        lead.borrow_mut().tree = Some(fact.clone());
        let _ = self.status_now(lead).await;
        Some(fact)
    }

    /// reach.ts:456 (rule 9).
    pub(crate) async fn tree_child_of(&self, child: &Entry, lead_branch: &str) -> TreeChild {
        let (record, machine_tree, branch_held) = {
            let held = child.borrow();
            let record = held.record.borrow().clone();
            let tree = record.tree.clone();
            let base = record.worktree.as_ref().and_then(|tree| tree.branch.clone());
            (record, tree, base)
        };
        let checkout_branch = self.read_checkout(child, false).await.map(|fact| fact.branch);
        let branch = checkout_branch.or(branch_held).or(record.base.clone()).unwrap_or_default();
        let thread = self
            .ctx
            .session_entries()
            .into_iter()
            .find(|entry| entry.view.workspace_id == record.id && entry.view.thread_id.is_some())
            .and_then(|entry| entry.view.thread_id.clone());
        let remote = self.ctx.project_held(&record.project).remote;
        let asked: Option<GitBranchCompareReply> = if branch.is_empty() || branch == DETACHED_HEAD || remote_host(&remote).is_none() {
            None
        } else {
            let base = machine_tree_base(&record).unwrap_or_else(|| lead_branch.to_string());
            let head = branch.clone();
            let frame = move |home: &str| json!({ "op": "git.branchCompare", "cwd": home, "remote": remote, "base": base, "head": head });
            match self.ctx.on_this_computer(Box::new(frame)).await {
                Ok(reply) => serde_json::from_value(reply).ok(),
                Err(_) => None,
            }
        };
        let mut row: Map<String, Value> = Map::new();
        row.insert("workspaceId".to_string(), json!(record.id));
        if let Some(thread) = thread {
            row.insert("threadId".to_string(), json!(thread));
        }
        row.insert("branch".to_string(), json!(branch));
        if let Some(asked) = asked {
            row.insert("pushed".to_string(), json!(asked.pushed));
            if asked.pushed {
                if let Some(ahead) = asked.ahead_by {
                    row.insert("aheadOfLead".to_string(), json!(ahead));
                }
                if let Some(behind) = asked.behind_by {
                    row.insert("behindLead".to_string(), json!(behind));
                }
            }
        }
        if let Some(Value::Object(kept)) = machine_tree.and_then(|tree| serde_json::to_value(tree).ok()) {
            for (key, value) in kept {
                row.insert(key, value);
            }
        }
        serde_json::from_value(Value::Object(row)).unwrap_or_else(|_| TreeChild {
            merged: None,
            conflicts: None,
            push_refused: None,
            workspace_id: record.id.clone(),
            thread_id: None,
            branch: String::new(),
            pushed: None,
            ahead_of_lead: None,
            behind_lead: None,
        })
    }

    /// reach.ts:471 (rule 10).
    pub fn read_lead_of(&self, child: &Entry) {
        let parent = child.borrow().record.borrow().parent_workspace_id.clone();
        let Some(lead_id) = parent else {
            return;
        };
        let Some(lead) = self.ctx.live(&lead_id) else {
            return;
        };
        let Some(area) = self.me.upgrade() else {
            return;
        };
        self.ctx.spawn(Box::pin(async move {
            let _ = area.read_tree(&lead).await;
        }));
    }
}

fn machine_tree_base(record: &wsp_core::types::wiring::WorkspaceRecord) -> Option<String> {
    record.tree.as_ref().and_then(|tree| serde_json::to_value(tree).ok()).and_then(|tree| tree.get("merged").and_then(|merged| merged.get("head")).and_then(Value::as_str).map(str::to_string))
}
