// SPDX-License-Identifier: AGPL-3.0-only
//! channels.ts:580-934: the served and copy channels, shell tokens, where a child starts, one repository. Spec:
//! spec/channels_copy.md.
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::{join_all, select, Either, FutureExt, LocalBoxFuture, Shared};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_core::types::internal::{default_branch_refused, ChildStart};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::exec_detached::put::INLINE_EXEC_MS;
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_machines::machines::brokered::TERMINAL_ENV;
use wsp_wire::types::bring_back::GitPushReply;
use wsp_wire::types::check::parse;
use wsp_wire::types::env::THREAD_ENV;
use wsp_wire::types::tree::GitStartOnReply;
use wsp_wire::views::workspace::{Caller, ProjectSource, ProjectView};
use wsp_wire::wire::daemon::{GitStatusReply, DETACHED_HEAD};
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;
use wsp_words::projects::{copies_folder, kind_for_computer, runs_in_folder};
use wsp_words::tree::{child_started_line, fork_needs_push_line, pushed_for_child_line, uncommitted_stayed};
use wsp_words::words::computer::{branch_unread_refusal, DefaultBranchRoad};
use wsp_words::workspace_state::is_local_workspace;

use super::daemon::{channel_refused, frame_op, is_default_branch_error, js_string, record_of};
use super::ptys::{string_map_of, string_map_value};
use super::{
    ChannelsArea, ChannelsCtx, BLOCKED_READS, COMPUTER_PROCS, COMPUTER_WATCHES, GUEST_EVENTS, ORIGIN_READ_MS, PTY_EVENTS, PTY_TOKENS,
};
use crate::account::box_panels::BoxPanels;
use crate::account::core_types::{AccountResult, Ask, Entry, Frame, Served};

/// `"fork" | "run"`: the road a child starts by (channels.ts:845).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChildRoad {
    Fork,
    Run,
}

/// What `terminalToken` answers (channels.ts:672).
#[derive(Clone, Debug, PartialEq)]
pub struct ShellToken {
    pub device_id: String,
    pub env: IndexMap<String, String>,
}

/// `{ id: null, ok: false, code: "unsupported", error }`.
fn unsupported(error: String) -> Frame {
    let mut reply = Map::new();
    reply.insert("id".to_string(), Value::Null);
    reply.insert("ok".to_string(), Value::Bool(false));
    reply.insert("code".to_string(), Value::String("unsupported".to_string()));
    reply.insert("error".to_string(), Value::String(error));
    reply
}

/// `{ id: null, ...reply }`: the answer under a null id, keeping the reply's own keys in their order.
fn with_null_id(reply: Frame) -> Frame {
    let mut out = Map::new();
    out.insert("id".to_string(), Value::Null);
    for (key, value) in reply {
        out.insert(key, value);
    }
    out
}

fn is_ok(reply: &Frame) -> bool {
    reply.get("ok") == Some(&Value::Bool(true))
}

fn frame_of(v: Value) -> Frame {
    v.as_object().cloned().unwrap_or_default()
}

fn js_str_opt(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(v) => js_string(v),
    }
}

fn first_line(text: &str) -> &str {
    js_trim(text).split('\n').next().unwrap_or("")
}

/// Polls a future once, now, so the calls it makes before it first waits are made in this call; the rest goes to the host.
fn poll_now(ctx: &dyn ChannelsCtx, mut run: LocalBoxFuture<'static, ()>) {
    let waker = futures::task::noop_waker();
    let mut cx = std::task::Context::from_waker(&waker);
    if run.as_mut().poll(&mut cx).is_pending() {
        ctx.spawn(run);
    }
}

fn default_road(road: ChildRoad) -> DefaultBranchRoad {
    match road {
        ChildRoad::Fork => DefaultBranchRoad::Fork,
        ChildRoad::Run => DefaultBranchRoad::Run,
    }
}

/// The channel a client of a served workspace drives: frames go up the computer's link with the workspace named on
/// them, and the events come off that link (channels.ts:587-667).
struct ServedChannel {
    ctx: Rc<dyn ChannelsCtx>,
    served: Served,
    place: String,
    entry_name: String,
    carries: Vec<String>,
    panels: Option<Rc<dyn BoxPanels>>,
    named: Rc<RefCell<IndexSet<String>>>,
    attached: Rc<RefCell<IndexSet<String>>>,
    link: Rc<dyn DaemonChannel>,
}

impl ServedChannel {
    /// What this channel now holds on the far end, off a frame it sent and the answer to it (channels.ts:625-638).
    fn hold(&self, op: &str, frame: &Frame, reply: &Frame) {
        if op == "pty.create" {
            self.named.borrow_mut().insert(js_str_opt(reply.get("ptyId")));
            return;
        }
        if op == "pty.list" {
            if let Some(rows) = reply.get("ptys").and_then(Value::as_array) {
                for row in rows {
                    self.named.borrow_mut().insert(js_str_opt(row.get("id")));
                }
            }
            return;
        }
        let pty = js_str_opt(frame.get("ptyId"));
        if op == "pty.attach" {
            self.named.borrow_mut().insert(pty.clone());
            self.attached.borrow_mut().insert(pty.clone());
        }
        if op == "pty.detach" || op == "pty.kill" {
            self.attached.borrow_mut().shift_remove(&pty);
        }
    }
}

#[async_trait(?Send)]
impl DaemonChannel for ServedChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let op = frame_op(&frame).to_string();
        if let Some(panels) = &self.panels {
            if panels.answers(&op) {
                let reply = panels.send(frame.clone()).await.map_err(channel_refused)?;
                if is_ok(&reply) {
                    self.hold(&op, &frame, &reply);
                }
                return Ok(with_null_id(reply));
            }
        }
        if COMPUTER_WATCHES.contains(&op.as_str()) {
            return Ok(unsupported(wsp_wire::wire::machine_link::place_watches_itself_line(&self.ctx.name_of(&self.place))));
        }
        if COMPUTER_PROCS.contains(&op.as_str()) {
            return Ok(unsupported(wsp_wire::wire::machine_link::fork_procs_unread_line(&self.entry_name, &self.ctx.name_of(&self.place))));
        }
        if !self.carries.iter().any(|carried| carried == &op) {
            return Ok(unsupported(wsp_wire::wire::machine_link::fork_op_refused_line(
                &op,
                &self.entry_name,
                &self.ctx.name_of(&self.place),
            )));
        }
        let reply = (self.served)(frame.clone()).await.map_err(channel_refused)?;
        if is_ok(&reply) {
            self.hold(&op, &frame, &reply);
        }
        Ok(with_null_id(reply))
    }

    fn close(&self) {
        let detach: Vec<String> = self.attached.borrow().iter().cloned().collect();
        self.attached.borrow_mut().clear();
        for pty in detach {
            let served = self.served.clone();
            let frame = frame_of(json!({ "op": "pty.detach", "ptyId": pty }));
            poll_now(
                &*self.ctx,
                Box::pin(async move {
                    let _ = served(frame).await;
                }),
            );
        }
        if let Some(panels) = &self.panels {
            panels.close();
        }
        self.link.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.link.closed().await
    }
}

/// A copy's channel with the shell token and thread bookkeeping of a pane's `pty.create` (channels.ts:791-825).
struct CopyChannel {
    area: Rc<ChannelsArea>,
    entry: Entry,
    workspace_id: String,
    opened: Rc<dyn DaemonChannel>,
    mints: bool,
}

#[async_trait(?Send)]
impl DaemonChannel for CopyChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        if frame_op(&frame) != "pty.create" {
            let op = frame_op(&frame).to_string();
            let reply = self.opened.send(frame.clone()).await?;
            if self.mints && (op == "pty.kill" || op == "pty.list") {
                self.area.shells_answered(&self.workspace_id, &frame, &reply).await;
            }
            return Ok(reply);
        }
        let mut asked = frame;
        let thread = asked.shift_remove("thread");
        let cwd = match asked.get("cwd") {
            Some(cwd) if !cwd.is_null() => cwd.clone(),
            _ => Value::String(self.area.ctx.thread_folder(&self.entry).await.map_err(channel_refused)?),
        };
        let minted = if self.area.brokered(&self.entry) {
            self.area.terminal_token(&self.entry, thread.as_ref()).await.map_err(channel_refused)?
        } else {
            None
        };
        let mut sent = asked.clone();
        sent.insert("cwd".to_string(), cwd);
        if let Some(minted) = &minted {
            let mut env = Map::new();
            // A JS object lists its index keys first, whatever order they were set in.
            if let Some(Value::Array(items)) = asked.get("env") {
                for (i, value) in items.iter().enumerate() {
                    env.insert(i.to_string(), value.clone());
                }
            }
            for (key, value) in self.area.ctx.thread_env(&self.entry) {
                env.insert(key, Value::String(value));
            }
            if let Some(Value::Object(given)) = asked.get("env") {
                for (key, value) in given {
                    env.insert(key.clone(), value.clone());
                }
            }
            for (key, value) in &minted.env {
                env.insert(key.clone(), Value::String(value.clone()));
            }
            sent.insert("env".to_string(), Value::Object(env));
        }
        let reply = match self.opened.send(sent).await {
            Ok(reply) => reply,
            Err(e) => {
                if let Some(minted) = &minted {
                    self.area.revoke_shell_tokens(std::slice::from_ref(&minted.device_id)).await;
                }
                return Err(e);
            }
        };
        let Some(minted) = minted else {
            return Ok(reply);
        };
        let going = self.entry.borrow().deleting.is_some() || !self.area.ctx.live_has(&self.workspace_id);
        match reply.get("ptyId").and_then(Value::as_str) {
            Some(pty) if is_ok(&reply) && !going => self.area.shell_opened(&self.workspace_id, pty, &minted.device_id).await,
            _ => self.area.revoke_shell_tokens(std::slice::from_ref(&minted.device_id)).await,
        }
        Ok(reply)
    }

    fn close(&self) {
        self.opened.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.opened.closed().await
    }
}

/// A copy on a computer that cannot run workspaces: only the reads and the hello reach the client (channels.ts:806-812).
struct BlockedChannel {
    channel: Rc<dyn DaemonChannel>,
    said: String,
}

#[async_trait(?Send)]
impl DaemonChannel for BlockedChannel {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        if BLOCKED_READS.contains(&frame_op(&frame)) {
            return self.channel.send(frame).await;
        }
        Ok(unsupported(self.said.clone()))
    }

    fn close(&self) {
        self.channel.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.channel.closed().await
    }
}

impl ChannelsArea {
    /// C1-C5, channels.ts:587. `carries` is the client's frame list.
    pub async fn served_channel(
        self: &Rc<Self>,
        entry: &Entry,
        served: Served,
        on_event: Rc<dyn Fn(Value)>,
        carries: &[&str],
    ) -> AccountResult<Rc<dyn DaemonChannel>> {
        let record = record_of(entry);
        let place = match (&record.place, self.ctx.has_door()) {
            (Some(place), true) => place.clone(),
            _ => {
                return Err(
                    HostError::new(wsp_wire::wire::machine_link::place_serves_daemon_line(&record.name, &self.computer_of(entry))).into()
                )
            }
        };
        let version = self.ctx.report_of(&place).await.map(|report| report.daemon_version.get());
        let machine_id = entry.borrow().machine.id().to_string();
        let checkout = self.checkout_of(&record)?;
        let named: Rc<RefCell<IndexSet<String>>> = Rc::default();
        let attached: Rc<RefCell<IndexSet<String>>> = Rc::default();
        let panels: Option<Rc<dyn BoxPanels>> =
            if runs_in_folder(&record.kind) { Some(self.box_panels.open(entry, &place, served.clone(), &checkout)) } else { None };
        let heard: Rc<dyn Fn(Value)> = {
            let panels = panels.clone();
            let named = named.clone();
            let attached = attached.clone();
            let on_event = on_event.clone();
            let machine_id = machine_id.clone();
            Rc::new(move |event: Value| {
                if panels.as_ref().is_some_and(|panels| panels.heard(&event)) {
                    on_event(event);
                    return;
                }
                let kind = js_str_opt(event.get("type"));
                if GUEST_EVENTS.contains(&kind.as_str()) {
                    if event.get("machineId") == Some(&Value::String(machine_id.clone())) {
                        on_event(event);
                    }
                    return;
                }
                let pty = js_str_opt(event.get("ptyId"));
                if !PTY_EVENTS.contains(&kind.as_str()) || !named.borrow().contains(&pty) {
                    return;
                }
                if kind == "pty.exit" {
                    attached.borrow_mut().shift_remove(&pty);
                }
                on_event(event);
            })
        };
        let Some(link) = self.ctx.link_channel(&place, heard) else {
            return Err(HostError::new(wsp_words::workspace_state::absent_computer(&self.ctx.name_of(&place), None).sentence).into());
        };
        {
            let link = link.clone();
            let panels = panels.clone();
            self.ctx.spawn(Box::pin(async move {
                link.closed().await;
                if let Some(panels) = panels {
                    panels.close();
                }
            }));
        }
        let mut hello = Map::new();
        hello.insert("type".to_string(), Value::String("daemon.hello".to_string()));
        hello.insert("root".to_string(), Value::String(checkout));
        if let Some(version) = version {
            hello.insert("version".to_string(), json!(version));
        }
        on_event(Value::Object(hello));
        Ok(Rc::new(ServedChannel {
            ctx: self.ctx.clone(),
            served,
            place,
            entry_name: record.name,
            carries: carries.iter().map(|op| op.to_string()).collect(),
            panels,
            named,
            attached,
            link,
        }))
    }

    /// C6, channels.ts:672. `named` is the frame's `thread` value, whatever its type.
    pub async fn terminal_token(self: &Rc<Self>, entry: &Entry, named: Option<&Value>) -> AccountResult<Option<ShellToken>> {
        let record = record_of(entry);
        let Some(thread_id) = self.thread_on(&record.id, named) else {
            return Ok(None);
        };
        let launch = self.ctx.thread_launch(entry, &thread_id, &self.ctx.root_of(&thread_id)).await?;
        let Some(scoped) = launch.scoped else {
            return Ok(None);
        };
        let token = scoped.device_token.clone();
        let mut env = launch.env;
        for (key, value) in self.ctx.brokered_env(&token, entry) {
            env.insert(key, value);
        }
        for (key, value) in self.shell_sign_ins(entry, &thread_id, &token).await? {
            env.insert(key, value);
        }
        env.insert(THREAD_ENV.to_string(), thread_id);
        Ok(Some(ShellToken { device_id: scoped.device_id, env }))
    }

    /// C7, channels.ts:685.
    pub async fn shell_sign_ins(
        self: &Rc<Self>,
        entry: &Entry,
        thread_id: &str,
        token: &str,
    ) -> Result<IndexMap<String, String>, HostError> {
        let harness = self
            .ctx
            .thread_record(thread_id)
            .map(|record| record.harness)
            .filter(|harness| !harness.is_empty())
            .or_else(|| self.ctx.latest_harness(thread_id));
        let Some(harness) = harness else {
            return Ok(IndexMap::new());
        };
        let Some(sign_in) = self.ctx.terminal_sign_in(&harness, entry, token)? else {
            return Ok(IndexMap::new());
        };
        let line = self.ctx.terminal_line(&harness, sign_in.program.as_deref());
        self.write_terminal(entry, &harness, &line).await;
        let mut env = sign_in.env;
        env.insert(TERMINAL_ENV.to_string(), harness);
        Ok(env)
    }

    /// C8, channels.ts:699.
    pub async fn write_terminal(self: &Rc<Self>, entry: &Entry, own: &str, line: &str) {
        let (workspace_id, machine) = {
            let live = entry.borrow();
            let id = live.record.borrow().id.clone();
            (id, live.machine.clone())
        };
        let machine_id = machine.id().to_string();
        let written = format!("{machine_id} {own}");
        let run = self.write_run(machine, workspace_id.clone(), machine_id, own.to_string(), line.to_string(), written.clone());
        if self.terminals_written.borrow().get(&workspace_id).is_some_and(|set| set.contains(&written)) {
            poll_now(&*self.ctx, run);
            return;
        }
        let (timer_tx, timer_rx) = oneshot::channel::<()>();
        let cancel = self.ctx.schedule(
            INLINE_EXEC_MS as i64,
            Box::new(move || {
                let _ = timer_tx.send(());
            }),
        );
        match select(run, timer_rx).await {
            Either::Left(_) => {}
            Either::Right((_, rest)) => self.ctx.spawn(rest),
        }
        cancel();
    }

    fn write_run(
        self: &Rc<Self>,
        machine: Arc<dyn Machine>,
        workspace_id: String,
        machine_id: String,
        own: String,
        line: String,
        written: String,
    ) -> LocalBoxFuture<'static, ()> {
        let this = self.clone();
        Box::pin(async move {
            let options = ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() };
            let (code, stderr) = match machine.exec(&line, options).await {
                Ok(result) => (result.exit_code.get(), result.stderr),
                Err(e) => (-1, e.to_string()),
            };
            this.terminals_written.borrow_mut().entry(workspace_id.clone()).or_default().insert(written);
            if code != 0 {
                this.ctx.warn(&format!(
                    "the terminal programs for {own} on {machine_id} (workspace {workspace_id}) were not written, so its terminals keep the last ones there: exit {code}, {}",
                    first_line(&stderr)
                ));
            }
        })
    }

    /// C9, channels.ts:717: the workspace's shell tokens, read once from the store.
    pub async fn shell_tokens_of(self: &Rc<Self>, workspace_id: &str) -> Rc<RefCell<IndexMap<String, String>>> {
        let held = self.shell_tokens.borrow().get(workspace_id).cloned();
        if let Some(held) = held {
            return held;
        }
        let kept = self.ctx.store_get(PTY_TOKENS, workspace_id).await;
        let read = Rc::new(RefCell::new(string_map_of(kept)));
        self.shell_tokens.borrow_mut().entry(workspace_id.to_string()).or_insert(read).clone()
    }

    /// C9, channels.ts:747.
    pub async fn shell_opened(self: &Rc<Self>, workspace_id: &str, pty_id: &str, device_id: &str) {
        let held = self.shell_tokens_of(workspace_id).await;
        held.borrow_mut().insert(pty_id.to_string(), device_id.to_string());
        self.keep_shell_tokens(workspace_id).await;
    }

    /// C9, channels.ts:751.
    pub async fn shells_ended(self: &Rc<Self>, workspace_id: &str, pty_ids: &[String]) {
        let held = self.shell_tokens_of(workspace_id).await;
        let mut gone: Vec<String> = Vec::new();
        for pty in pty_ids {
            if let Some(device) = held.borrow_mut().shift_remove(pty) {
                gone.push(device);
            }
        }
        if gone.is_empty() {
            return;
        }
        self.keep_shell_tokens(workspace_id).await;
        self.revoke_shell_tokens(&gone).await;
    }

    /// C9, channels.ts:764.
    pub async fn shells_answered(self: &Rc<Self>, workspace_id: &str, frame: &Frame, reply: &Frame) {
        if !is_ok(reply) {
            return;
        }
        if frame_op(frame) == "pty.kill" {
            let pty = js_str_opt(frame.get("ptyId"));
            self.shells_ended(workspace_id, &[pty]).await;
            return;
        }
        if frame_op(frame) != "pty.list" {
            return;
        }
        let Some(rows) = reply.get("ptys").and_then(Value::as_array) else {
            return;
        };
        let held = self.shell_tokens_of(workspace_id).await;
        if held.borrow().is_empty() {
            return;
        }
        let standing: HashSet<String> =
            rows.iter().filter(|row| row.get("exited") != Some(&Value::Bool(true))).map(|row| js_str_opt(row.get("id"))).collect();
        let gone: Vec<String> = held.borrow().keys().filter(|id| !standing.contains(*id)).cloned().collect();
        self.shells_ended(workspace_id, &gone).await;
    }

    /// C9, channels.ts:774.
    pub async fn shell_held_devices(self: &Rc<Self>) -> AccountResult<BTreeSet<String>> {
        let ids = self.ctx.store_keys(PTY_TOKENS).await?;
        let mut devices = BTreeSet::new();
        for id in ids.iter().filter(|id| self.ctx.live_has(id)) {
            let held = self.shell_tokens_of(id).await;
            devices.extend(held.borrow().values().cloned());
        }
        Ok(devices)
    }

    /// C10-C12, channels.ts:791.
    pub async fn copy_channel(
        self: &Rc<Self>,
        entry: &Entry,
        on_event: Rc<dyn Fn(Value)>,
        carries: &[&str],
    ) -> AccountResult<Rc<dyn DaemonChannel>> {
        let record = record_of(entry);
        let said = if runs_in_folder(&record.kind) { None } else { self.blocked_line(record.place.as_deref()).await };
        let workspace_id = record.id.clone();
        let served = self.served_by_its_computer(entry);
        let mints = served.is_none() && !is_local_workspace(Some(&record.kind)) && !runs_in_folder(&record.kind);
        let heard: Rc<dyn Fn(Value)> = {
            let area = self.clone();
            let workspace_id = workspace_id.clone();
            let on_event = on_event.clone();
            let silenced = said.is_some();
            Rc::new(move |event: Value| {
                if mints && js_str_opt(event.get("type")) == "pty.exit" {
                    let pty = js_str_opt(event.get("ptyId"));
                    let ctx = area.ctx.clone();
                    let area = area.clone();
                    let workspace_id = workspace_id.clone();
                    ctx.spawn(Box::pin(async move {
                        area.shells_ended(&workspace_id, &[pty]).await;
                    }));
                }
                if !silenced || event.get("type").and_then(Value::as_str) == Some("daemon.hello") {
                    on_event(event);
                }
            })
        };
        let opened: Rc<dyn DaemonChannel> = match &served {
            None => self.own_daemon_channel(entry, heard).await?,
            Some(served) => self.served_channel(entry, served.clone(), heard, carries).await?,
        };
        let channel: Rc<dyn DaemonChannel> = Rc::new(CopyChannel { area: self.clone(), entry: entry.clone(), workspace_id, opened, mints });
        match said {
            None => Ok(channel),
            Some(said) => Ok(Rc::new(BlockedChannel { channel, said })),
        }
    }

    /// C13, channels.ts:845.
    pub async fn lead_start(self: &Rc<Self>, lead: &Entry, child: &str, road: ChildRoad) -> AccountResult<ChildStart> {
        self.copy_blocked(lead).await?;
        let record = record_of(lead);
        let project = self.project_held(&record.project)?;
        let started: Option<String> = record.base.clone().or_else(|| project.base.clone()).or_else(|| Some(project.default_branch.clone()));
        let cwd = self.checkout_of(&record)?;
        let status_cwd = cwd.clone();
        let status = self
            .with_daemon(
                lead,
                Box::new(move |ask: Ask| -> LocalBoxFuture<'static, AccountResult<Frame>> {
                    Box::pin(async move { ask(frame_of(json!({ "op": "git.status", "cwd": status_cwd }))).await })
                }),
            )
            .await
            .map_err(|e| HostError::new(branch_unread_refusal(&record.name, &e.message())))?;
        let read: GitStatusReply = parse(&Value::Object(status)).map_err(|e| HostError::new(e.issue_text()))?;
        let branch = read.branch.head.clone();
        if read.default_branch.as_deref() == Some(branch.as_str()) && (read.branch.ahead.0 > 0.0 || read.counts_unknown == Some(true)) {
            return Err(default_branch_refused(default_road(road), &record.name, &branch).into());
        }
        let changed = read.entries.iter().filter(|entry| entry.xy != "!!").count();
        let stayed: Vec<String> = if changed > 0 { vec![uncommitted_stayed(changed as f64, &record.name)] } else { Vec::new() };
        if branch.is_empty() || branch == DETACHED_HEAD || started.as_deref() == Some(branch.as_str()) {
            return Ok(ChildStart { base: started, on_leads: false, lines: stayed });
        }
        let upstream = read.branch.upstream.clone();
        let tracks_itself = upstream.as_deref().is_some_and(|up| up.ends_with(&format!("/{branch}")));
        let tracks_base = match (&upstream, &started) {
            (Some(up), Some(base)) => up.ends_with(&format!("/{base}")),
            _ => false,
        };
        let holds_new =
            read.branch.ahead.0 > 0.0 || read.counts_unknown == Some(true) || (upstream.is_some() && !tracks_itself && !tracks_base);
        if !holds_new {
            return Ok(if tracks_itself {
                ChildStart { base: Some(branch), on_leads: true, lines: stayed }
            } else {
                ChildStart { base: started, on_leads: false, lines: stayed }
            });
        }
        let mut push = frame_of(json!({ "op": "git.push", "cwd": cwd }));
        if let Some(base) = &started {
            push.insert("base".to_string(), Value::String(base.clone()));
        }
        let pushed = self
            .with_daemon(
                lead,
                Box::new(move |ask: Ask| -> LocalBoxFuture<'static, AccountResult<Frame>> { Box::pin(async move { ask(push).await }) }),
            )
            .await
            .map_err(|e| {
                if is_default_branch_error(&e) {
                    default_branch_refused(default_road(road), &record.name, &branch)
                } else {
                    HostError::new(fork_needs_push_line(&record.name, &e.message()))
                }
            })?;
        let pushed: GitPushReply = parse(&Value::Object(pushed)).map_err(|e| HostError::new(e.issue_text()))?;
        self.ctx.read_checkout_soon(lead);
        let mut lines = vec![pushed_for_child_line(&pushed.branch, child)];
        lines.extend(stayed);
        Ok(ChildStart { base: Some(pushed.branch), on_leads: true, lines })
    }

    /// C14, channels.ts:884.
    pub async fn start_child_on(self: &Rc<Self>, child: &Entry, branch: &str, lead: &str) -> AccountResult<String> {
        match self.put_child_on(child, branch).await {
            Ok(line) => Ok(line),
            Err(e) => {
                let record = record_of(child);
                if let Err(d) = self.ctx.delete_workspace(&record.id).await {
                    self.ctx.warn(&format!("{} was not put on {branch} and was not deleted: {}", record.name, d.message()));
                }
                Err(HostError::new(fork_needs_push_line(lead, &e.message())).into())
            }
        }
    }

    async fn put_child_on(self: &Rc<Self>, child: &Entry, branch: &str) -> AccountResult<String> {
        let record = record_of(child);
        let cwd = self.checkout_of(&record)?;
        let frame = frame_of(json!({ "op": "git.startOn", "cwd": cwd, "branch": branch }));
        let said = self
            .with_daemon(
                child,
                Box::new(move |ask: Ask| -> LocalBoxFuture<'static, AccountResult<Frame>> { Box::pin(async move { ask(frame).await }) }),
            )
            .await?;
        let put: GitStartOnReply = parse(&Value::Object(said)).map_err(|e| HostError::new(e.issue_text()))?;
        if record.worktree.is_some() {
            let cell = child.borrow().record.clone();
            let mut record_mut = cell.borrow_mut();
            if let Some(tree) = record_mut.worktree.as_mut() {
                tree.branch = Some(put.branch.clone());
            }
        }
        let persisted: WorkspaceRecord = record_of(child);
        self.ctx.persist(&persisted).await?;
        self.ctx.read_checkout_soon(child);
        Ok(child_started_line(&persisted.name, &put.branch))
    }

    /// C15, channels.ts:899.
    pub fn repository_of(remote: &str) -> Option<String> {
        let host = wsp_catalog::git_hosts::remote_host(remote)?;
        let repo = wsp_words::start::owner_repo_of(remote)?.to_lowercase();
        Some(format!("{host}/{repo}"))
    }

    /// C15, channels.ts:910. The TS holds the read per caller object for one request; this reads every call (open
    /// point 1 in the report: a borrowed `Caller` has no identity that outlives the request safely).
    pub fn origin_now(&self, project: &ProjectView, _caller: Option<&Caller>) -> String {
        if !matches!(project.source, ProjectSource::Folder(_)) || !copies_folder(&kind_for_computer(&project.computer, None)) {
            return String::new();
        }
        let (status, remote) = self.ctx.git_origin(&project.path, ORIGIN_READ_MS);
        if status == Some(0) {
            js_trim(&remote).to_string()
        } else {
            String::new()
        }
    }

    /// C15, channels.ts:923.
    pub fn same_repository(&self, a: &ProjectView, b: &ProjectView, caller: Option<&Caller>) -> bool {
        let Some(theirs) = Self::repository_of(&b.remote) else {
            return false;
        };
        Self::repository_of(&a.remote).as_deref() == Some(theirs.as_str())
            || Self::repository_of(&self.origin_now(a, caller)).as_deref() == Some(theirs.as_str())
    }

    /// C9: writes the shell tokens of a workspace one at a time, each the map as it stands when it runs.
    async fn keep_shell_tokens(self: &Rc<Self>, workspace_id: &str) {
        let prev = self.shell_writes.borrow().get(workspace_id).map(|(_, write)| write.clone());
        let token = self.seq();
        let this = self.clone();
        let ws = workspace_id.to_string();
        let write: LocalBoxFuture<'static, ()> = Box::pin(async move {
            if let Some(prev) = prev {
                prev.await;
            }
            let held = this.shell_tokens_of(&ws).await;
            let saved = if held.borrow().is_empty() {
                this.ctx.store_delete(PTY_TOKENS, &ws).await
            } else {
                let value = string_map_value(&held.borrow());
                this.ctx.store_put(PTY_TOKENS, &ws, value).await
            };
            if let Err(e) = saved {
                this.ctx.warn(&format!("which shells in {ws} hold a token was not kept for the next host: {}", e.message()));
            }
            if this.shell_writes.borrow().get(&ws).map(|(seq, _)| *seq) == Some(token) {
                this.shell_writes.borrow_mut().shift_remove(&ws);
            }
        });
        let shared: Shared<LocalBoxFuture<'static, ()>> = write.shared();
        self.shell_writes.borrow_mut().insert(workspace_id.to_string(), (token, shared.clone()));
        self.ctx.spawn(Box::pin(shared.clone()));
        shared.await;
    }

    /// C9, channels.ts:735 and :760: every revoke is asked before any answer is waited on; a revoke that fails warns and
    /// the shell's end stands.
    async fn revoke_shell_tokens(&self, device_ids: &[String]) {
        let door = self.ctx.device_door();
        let answers = join_all(device_ids.iter().map(|device_id| door.revoke(device_id))).await;
        for answer in answers {
            if let Err(e) = answer {
                self.ctx.warn(&format!("the token of a shell was not taken away: {}", e.message));
            }
        }
    }

    /// C9, channels.ts:818-832: a workspace deleted forgets what its terminals wrote and ends its shells.
    pub(crate) fn shells_deleted(self: &Rc<Self>, workspace_id: &str) {
        self.terminals_written.borrow_mut().shift_remove(workspace_id);
        let this = self.clone();
        let ws = workspace_id.to_string();
        self.ctx.spawn(Box::pin(async move {
            let held = this.shell_tokens_of(&ws).await;
            let ids: Vec<String> = held.borrow().keys().cloned().collect();
            this.shells_ended(&ws, &ids).await;
            this.shell_tokens.borrow_mut().shift_remove(&ws);
        }));
    }
}
