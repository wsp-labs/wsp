// SPDX-License-Identifier: AGPL-3.0-only
//! channels.ts:280-580: whose shell is whose, the kill guard and the port roots. Spec: spec/channels_ptys.md.
use std::cell::RefCell;
use std::collections::{BTreeSet, HashSet};
use std::rc::{Rc, Weak};

use async_trait::async_trait;
use futures::future::{ready, FutureExt, LocalBoxFuture};
use indexmap::{IndexMap, IndexSet};
use serde_json::{json, Map, Value};

use wsp_core::daemon_channel::{ChannelEnd, ChannelError, DaemonChannel};
use wsp_wire::types::daemon_contract::{APP_DATA_FOLDER, APP_LOCK_FILE};

use super::daemon::{frame_op, js_string, node_join, record_of};
use super::{ChannelsArea, GUEST_ROAD_EXTRA, PORT_ROOTS_RECHECK_MS, PTY_THREADS, WORKSPACE_FRAMES};
use crate::account::core_types::{AccountResult, Entry, Frame};
use crate::account::proc_scopes::ThreadTerminal;

/// `String(v)` of a frame's key, where a missing key reads "undefined".
fn js_str_opt(v: Option<&Value>) -> String {
    match v {
        None => "undefined".to_string(),
        Some(v) => js_string(v),
    }
}

fn is_ok(reply: &Frame) -> bool {
    reply.get("ok") == Some(&Value::Bool(true))
}

fn frame_of(v: Value) -> Frame {
    v.as_object().cloned().unwrap_or_default()
}

/// `{ id: frame.id, ok: false, error }`: the `id` is kept only where the frame named one.
pub(crate) fn refusal_reply(frame: &Frame, error: String) -> Frame {
    let mut reply = Map::new();
    if let Some(id) = frame.get("id") {
        reply.insert("id".to_string(), id.clone());
    }
    reply.insert("ok".to_string(), Value::Bool(false));
    reply.insert("error".to_string(), Value::String(error));
    reply
}

/// `{ id: frame.id, ok: true }`.
fn ok_reply(frame: &Frame) -> Frame {
    let mut reply = Map::new();
    if let Some(id) = frame.get("id") {
        reply.insert("id".to_string(), id.clone());
    }
    reply.insert("ok".to_string(), Value::Bool(true));
    reply
}

/// A workspace's stored tags or tokens as the string map the document holds (a non-object or a non-string value reads
/// as no entry).
pub(crate) fn string_map_of(kept: AccountResult<Option<Value>>) -> IndexMap<String, String> {
    let entries: Vec<(String, Value)> = match kept {
        Ok(Some(Value::Object(map))) => map.into_iter().collect(),
        Ok(Some(Value::Array(items))) => items.into_iter().enumerate().map(|(i, v)| (i.to_string(), v)).collect(),
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(|(key, value)| match value {
            Value::String(s) => Some((key, s)),
            _ => None,
        })
        .collect()
}

/// The document a string map is kept as.
pub(crate) fn string_map_value(map: &IndexMap<String, String>) -> Value {
    Value::Object(map.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
}

/// `pty.list`'s rows, each row's id as `String(row.id)`.
fn listed_ids(rows: &[Value]) -> HashSet<String> {
    rows.iter().map(|row| js_str_opt(row.get("id"))).collect()
}

impl ChannelsArea {
    /// Y1, channels.ts:299: `WORKSPACE_FRAMES` then the guest road's own.
    pub fn guest_road_frames() -> Vec<&'static str> {
        WORKSPACE_FRAMES.iter().chain(GUEST_ROAD_EXTRA.iter()).copied().collect()
    }

    /// Y2, channels.ts:312.
    pub fn held_to_owner(self: &Rc<Self>, owner: &str, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        let mine = self.pty_owners.borrow_mut().entry(owner.to_string()).or_default().clone();
        Rc::new(HeldToOwner { mine, channel })
    }

    /// Y4, channels.ts:340.
    pub fn app_main(&self) -> Vec<i64> {
        if !self.ctx.has_state_path() {
            return Vec::new();
        }
        let lock = node_join(&[&self.ctx.state_folder(), APP_DATA_FOLDER, APP_LOCK_FILE]);
        let Some(link) = self.ctx.read_link(&lock) else {
            return Vec::new();
        };
        let Some((name, digits)) = link.rsplit_once('-') else {
            return Vec::new();
        };
        if name.is_empty() || digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) || name != self.ctx.hostname() {
            return Vec::new();
        }
        let Ok(pid) = digits.parse::<i64>() else {
            return Vec::new();
        };
        if self.ctx.pid_alive(pid) {
            vec![pid]
        } else {
            Vec::new()
        }
    }

    /// Y3, channels.ts:358.
    pub fn kill_guarded(self: &Rc<Self>, channel: Rc<dyn DaemonChannel>, here: bool) -> Rc<dyn DaemonChannel> {
        Rc::new(KillGuarded { area: self.clone(), channel, here })
    }

    /// Y5, channels.ts:382: the workspace's tags, read once from the store.
    pub async fn tags_of(self: &Rc<Self>, workspace_id: &str) -> Rc<RefCell<IndexMap<String, String>>> {
        let held = self.tags_held.borrow().get(workspace_id).cloned();
        if let Some(held) = held {
            return held.await;
        }
        let ctx = self.ctx.clone();
        let id = workspace_id.to_string();
        let read: LocalBoxFuture<'static, Rc<RefCell<IndexMap<String, String>>>> = Box::pin(async move {
            let kept = ctx.store_get(PTY_THREADS, &id).await;
            Rc::new(RefCell::new(string_map_of(kept)))
        });
        let shared = read.shared();
        let token = self.seq();
        self.tags_held_seq.borrow_mut().insert(workspace_id.to_string(), token);
        self.tags_held.borrow_mut().insert(workspace_id.to_string(), shared.clone());
        let this = self.clone();
        let ws = workspace_id.to_string();
        let settled = shared.clone();
        self.ctx.spawn(Box::pin(async move {
            let tags = settled.await;
            if this.tags_held_seq.borrow().get(&ws) == Some(&token) {
                this.tags_read.borrow_mut().insert(ws, tags);
            }
        }));
        shared.await
    }

    /// Y8, channels.ts:398.
    pub fn thread_terminals(&self) -> Vec<ThreadTerminal> {
        let port_pids = self.port_pids.borrow();
        let tags_read = self.tags_read.borrow();
        let mut out = Vec::new();
        for (workspace, ptys) in port_pids.iter() {
            let tags = tags_read.get(workspace);
            for (pty, pid) in ptys.iter() {
                let thread = tags.and_then(|tags| tags.borrow().get(pty).cloned());
                if let Some(thread) = thread.filter(|thread| !thread.is_empty()) {
                    out.push(ThreadTerminal { thread, workspace: workspace.clone(), pid: *pid });
                }
            }
        }
        out
    }

    /// Y6, channels.ts:410: queues one write of the tags as they stand when it runs.
    pub fn keep_tags(self: &Rc<Self>, workspace_id: &str) {
        let prev = self.tag_writes.borrow().get(workspace_id).cloned();
        let token = self.seq();
        self.tag_writes_seq.borrow_mut().insert(workspace_id.to_string(), token);
        let this = self.clone();
        let ws = workspace_id.to_string();
        let write: LocalBoxFuture<'static, ()> = Box::pin(async move {
            if let Some(prev) = prev {
                prev.await;
            }
            let held = this.tags_held.borrow().get(&ws).cloned();
            if let Some(held) = held {
                let tags = held.await;
                let saved = if tags.borrow().is_empty() {
                    this.ctx.store_delete(PTY_THREADS, &ws).await
                } else {
                    let value = string_map_value(&tags.borrow());
                    this.ctx.store_put(PTY_THREADS, &ws, value).await
                };
                if let Err(e) = saved {
                    this.ctx.warn(&format!("whose shells are whose in {ws} was not kept for the next host: {}", e.message()));
                }
            }
            if this.tag_writes_seq.borrow().get(&ws) != Some(&token) {
                return;
            }
            this.tag_writes.borrow_mut().shift_remove(&ws);
            this.tag_writes_seq.borrow_mut().shift_remove(&ws);
            if !this.tags_going.borrow_mut().shift_remove(&ws) {
                return;
            }
            this.tags_held.borrow_mut().shift_remove(&ws);
            this.tags_held_seq.borrow_mut().shift_remove(&ws);
            this.tags_read.borrow_mut().shift_remove(&ws);
        });
        let shared = write.shared();
        self.tag_writes.borrow_mut().insert(workspace_id.to_string(), shared.clone());
        self.ctx.spawn(Box::pin(shared));
    }

    /// Y6: the workspace's last queued tag write, settled (None when none is out), for a caller that must see it land.
    pub fn tag_write(&self, workspace_id: &str) -> Option<futures::future::Shared<futures::future::LocalBoxFuture<'static, ()>>> {
        self.tag_writes.borrow().get(workspace_id).cloned()
    }

    /// Y7, channels.ts:429: a deleted workspace's tags are empty, and its document goes with the last write.
    pub(crate) fn tags_deleted(self: &Rc<Self>, workspace_id: &str) {
        let none: Rc<RefCell<IndexMap<String, String>>> = Rc::default();
        let token = self.seq();
        self.tags_held_seq.borrow_mut().insert(workspace_id.to_string(), token);
        self.tags_held.borrow_mut().insert(workspace_id.to_string(), ready(none.clone()).boxed_local().shared());
        self.tags_read.borrow_mut().insert(workspace_id.to_string(), none);
        self.tags_going.borrow_mut().insert(workspace_id.to_string());
        self.keep_tags(workspace_id);
    }

    /// Y5, channels.ts:440.
    pub fn thread_tagged(self: &Rc<Self>, workspace_id: &str, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        Rc::new(ThreadTagged { area: self.clone(), workspace_id: workspace_id.to_string(), channel })
    }

    /// Y9, channels.ts:472.
    pub async fn end_thread_shells(self: &Rc<Self>, entry: &Entry, thread_id: &str) -> AccountResult<()> {
        let workspace_id = record_of(entry).id;
        let tags = self.tags_of(&workspace_id).await;
        let snapshot: Vec<(String, String)> = tags.borrow().iter().map(|(pty, thread)| (pty.clone(), thread.clone())).collect();
        for (pty, thread) in snapshot {
            if thread != thread_id {
                continue;
            }
            let kill = frame_of(json!({ "op": "pty.kill", "ptyId": pty }));
            let killed = self
                .with_daemon(
                    entry,
                    Box::new(move |ask: crate::account::core_types::Ask| -> LocalBoxFuture<'static, AccountResult<Frame>> {
                        Box::pin(async move { ask(kill).await })
                    }),
                )
                .await;
            if let Err(e) = killed {
                self.ctx.warn(&format!("shell {pty} of a deleted thread was not ended: {}", e.message()));
            }
            self.shells_ended(&workspace_id, std::slice::from_ref(&pty)).await;
            tags.borrow_mut().shift_remove(&pty);
            self.keep_tags(&workspace_id);
        }
        Ok(())
    }

    /// Y2, channels.ts:483.
    pub fn shared_daemon_replaced(&self) {
        for mine in self.pty_owners.borrow().values() {
            mine.borrow_mut().clear();
        }
    }

    /// Y10, channels.ts:495.
    pub fn arm_roots_recheck(self: &Rc<Self>) {
        if self.ctx.roots_recheck().borrow().is_some() {
            return;
        }
        self.schedule_roots_tick();
    }

    fn schedule_roots_tick(self: &Rc<Self>) {
        let weak: Weak<ChannelsArea> = Rc::downgrade(self);
        let cancel = self.ctx.schedule(
            PORT_ROOTS_RECHECK_MS,
            Box::new(move || {
                if let Some(area) = weak.upgrade() {
                    area.roots_tick();
                }
            }),
        );
        *self.ctx.roots_recheck().borrow_mut() = Some(cancel);
    }

    fn roots_tick(self: &Rc<Self>) {
        let ids: Vec<String> = self.turn_groups.borrow().keys().cloned().collect();
        for id in ids {
            self.port_roots_moved(&id);
        }
        if !self.turn_groups.borrow().is_empty() {
            self.schedule_roots_tick();
            return;
        }
        *self.ctx.roots_recheck().borrow_mut() = None;
    }

    /// Y10, channels.ts:507.
    pub fn port_roots_of(&self, workspace_id: &str) -> Vec<i64> {
        let mut roots: BTreeSet<i64> = BTreeSet::new();
        let emptied = {
            let mut all = self.turn_groups.borrow_mut();
            match all.get_mut(workspace_id) {
                Some(groups) => {
                    let pids: Vec<i64> = groups.keys().copied().collect();
                    for pid in pids {
                        if self.ctx.group_exists(pid) {
                            roots.insert(pid);
                        } else {
                            groups.shift_remove(&pid);
                        }
                    }
                    groups.is_empty()
                }
                None => false,
            }
        };
        if emptied {
            self.turn_groups.borrow_mut().shift_remove(workspace_id);
        }
        if let Some(ptys) = self.port_pids.borrow().get(workspace_id) {
            roots.extend(ptys.values().copied());
        }
        roots.into_iter().collect()
    }

    /// Y10, channels.ts:521.
    pub fn port_roots_moved(&self, workspace_id: &str) {
        self.port_roots_of(workspace_id);
        let pushes: Vec<Rc<dyn Fn()>> = self.port_watchers.borrow().get(workspace_id).cloned().unwrap_or_default();
        for push in pushes {
            push();
        }
        self.ctx.proc_scopes_moved();
    }

    /// Y11, channels.ts:529. `ptys` is the workspace's entry of `port_pids`.
    pub fn rooted_ports(self: &Rc<Self>, workspace_id: &str, folder: &str, channel: Rc<dyn DaemonChannel>) -> Rc<dyn DaemonChannel> {
        let ws = workspace_id.to_string();
        let told: Rc<RefCell<Option<String>>> = Rc::default();
        let push: Rc<dyn Fn()> = {
            let weak = Rc::downgrade(self);
            let ws = ws.clone();
            let folder = folder.to_string();
            let channel = channel.clone();
            let told = told.clone();
            Rc::new(move || {
                let Some(area) = weak.upgrade() else {
                    return;
                };
                if told.borrow().is_none() {
                    return;
                }
                let Some(roots) = fresh_roots(&area, &ws, &told) else {
                    return;
                };
                let frame = frame_of(json!({ "id": null, "op": "ports.watch", "roots": roots, "folder": folder }));
                let channel = channel.clone();
                area.ctx.spawn(Box::pin(async move {
                    let _ = channel.send(frame).await;
                }));
            })
        };
        self.port_watchers.borrow_mut().entry(ws.clone()).or_default().push(push.clone());
        let stop: Rc<dyn Fn()> = {
            let watchers = self.port_watchers.clone();
            let ws = ws.clone();
            let push = push.clone();
            Rc::new(move || {
                if let Some(list) = watchers.borrow_mut().get_mut(&ws) {
                    list.retain(|p| !Rc::ptr_eq(p, &push));
                }
            })
        };
        {
            let stop = stop.clone();
            let channel = channel.clone();
            self.ctx.spawn(Box::pin(async move {
                channel.closed().await;
                stop();
            }));
        }
        Rc::new(RootedPorts { area: self.clone(), ws, folder: folder.to_string(), channel, told, stop })
    }
}

/// The roots a channel last named differ from the roots now: `None` where they are the same as it was told.
fn fresh_roots(area: &ChannelsArea, workspace_id: &str, told: &RefCell<Option<String>>) -> Option<Vec<i64>> {
    let roots = area.port_roots_of(workspace_id);
    let joined = roots.iter().map(|root| root.to_string()).collect::<Vec<_>>().join(",");
    if told.borrow().as_deref() == Some(joined.as_str()) {
        return None;
    }
    *told.borrow_mut() = Some(joined);
    Some(roots)
}

struct HeldToOwner {
    mine: Rc<RefCell<IndexSet<String>>>,
    channel: Rc<dyn DaemonChannel>,
}

#[async_trait(?Send)]
impl DaemonChannel for HeldToOwner {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let asked = js_str_opt(frame.get("ptyId"));
        if frame.get("ptyId").is_some() && !self.mine.borrow().contains(&asked) {
            return Ok(refusal_reply(&frame, format!("no such pty: {asked}")));
        }
        let op = frame_op(&frame).to_string();
        let listed_from: Vec<String> = if op == "pty.list" { self.mine.borrow().iter().cloned().collect() } else { Vec::new() };
        let reply = self.channel.send(frame).await?;
        if !is_ok(&reply) {
            return Ok(reply);
        }
        match op.as_str() {
            "pty.create" => {
                self.mine.borrow_mut().insert(js_str_opt(reply.get("ptyId")));
            }
            "pty.kill" => {
                self.mine.borrow_mut().shift_remove(&asked);
            }
            "pty.list" => {
                if let Some(rows) = reply.get("ptys").and_then(Value::as_array) {
                    let held = listed_ids(rows);
                    let mut mine = self.mine.borrow_mut();
                    for id in &listed_from {
                        if !held.contains(id) {
                            mine.shift_remove(id);
                        }
                    }
                    let kept: Vec<Value> = rows.iter().filter(|row| mine.contains(&js_str_opt(row.get("id")))).cloned().collect();
                    let mut out = reply.clone();
                    out.insert("ptys".to_string(), Value::Array(kept));
                    return Ok(out);
                }
            }
            _ => {}
        }
        Ok(reply)
    }

    fn close(&self) {
        self.channel.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.channel.closed().await
    }
}

struct KillGuarded {
    area: Rc<ChannelsArea>,
    channel: Rc<dyn DaemonChannel>,
    here: bool,
}

#[async_trait(?Send)]
impl DaemonChannel for KillGuarded {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        if frame_op(&frame) != "proc.kill" {
            return self.channel.send(frame).await;
        }
        let mut asked = frame;
        for key in ["protect", "apps", "agents"] {
            asked.shift_remove(key);
        }
        let agent_bins: Vec<Value> = self.area.ctx.agent_bins().into_iter().map(Value::String).collect();
        if !self.here {
            asked.insert("agentBins".to_string(), Value::Array(agent_bins));
            return self.channel.send(asked).await;
        }
        let mut agents: Vec<Value> = Vec::new();
        for row in self.area.ctx.session_rows() {
            if row.status != "running" {
                continue;
            }
            let Some(pid) = row.pid else {
                continue;
            };
            agents.push(json!(pid));
            if let Some(agent) = row.run.as_deref().and_then(|run| self.area.ctx.agent_pid_of(run)) {
                agents.push(json!(agent));
            }
        }
        asked.insert("protect".to_string(), json!([self.area.ctx.host_pid(), self.area.ctx.host_parent_pid()]));
        asked.insert("apps".to_string(), json!(self.area.app_main()));
        asked.insert("agents".to_string(), Value::Array(agents));
        asked.insert("agentBins".to_string(), Value::Array(agent_bins));
        self.channel.send(asked).await
    }

    fn close(&self) {
        self.channel.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.channel.closed().await
    }
}

struct ThreadTagged {
    area: Rc<ChannelsArea>,
    workspace_id: String,
    channel: Rc<dyn DaemonChannel>,
}

#[async_trait(?Send)]
impl DaemonChannel for ThreadTagged {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let tags = self.area.tags_of(&self.workspace_id).await;
        let thread = frame.get("thread").and_then(Value::as_str).map(str::to_string);
        if frame_op(&frame) == "pty.thread" {
            let pty = frame.get("ptyId").and_then(Value::as_str).map(str::to_string);
            let (Some(pty), Some(thread)) = (pty, thread) else {
                return Ok(refusal_reply(&frame, "pty.thread names a ptyId and a thread".to_string()));
            };
            tags.borrow_mut().insert(pty, thread);
            self.area.keep_tags(&self.workspace_id);
            self.area.port_roots_moved(&self.workspace_id);
            return Ok(ok_reply(&frame));
        }
        let op = frame_op(&frame).to_string();
        let killed_pty = js_str_opt(frame.get("ptyId"));
        let reply = self.channel.send(frame.clone()).await?;
        if !is_ok(&reply) {
            return Ok(reply);
        }
        if op == "pty.create" {
            if let Some(thread) = &thread {
                tags.borrow_mut().insert(js_str_opt(reply.get("ptyId")), thread.clone());
                self.area.keep_tags(&self.workspace_id);
            }
        }
        if op == "pty.kill" && tags.borrow_mut().shift_remove(&killed_pty).is_some() {
            self.area.keep_tags(&self.workspace_id);
        }
        let Some(rows) = reply.get("ptys").and_then(Value::as_array).cloned() else {
            return Ok(reply);
        };
        if op != "pty.list" {
            return Ok(reply);
        }
        let held = listed_ids(&rows);
        let gone: Vec<String> = tags.borrow().keys().filter(|id| !held.contains(*id)).cloned().collect();
        if !gone.is_empty() {
            for id in &gone {
                tags.borrow_mut().shift_remove(id);
            }
            self.area.keep_tags(&self.workspace_id);
        }
        let tagged: Vec<Value> = rows
            .into_iter()
            .map(|row| {
                let id = js_str_opt(row.get("id"));
                let named = tags.borrow().get(&id).cloned();
                match (named, row) {
                    (Some(thread), Value::Object(mut obj)) => {
                        obj.insert("thread".to_string(), Value::String(thread));
                        Value::Object(obj)
                    }
                    (_, row) => row,
                }
            })
            .collect();
        let mut out = reply;
        out.insert("ptys".to_string(), Value::Array(tagged));
        Ok(out)
    }

    fn close(&self) {
        self.channel.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.channel.closed().await
    }
}

struct RootedPorts {
    area: Rc<ChannelsArea>,
    ws: String,
    folder: String,
    channel: Rc<dyn DaemonChannel>,
    told: Rc<RefCell<Option<String>>>,
    stop: Rc<dyn Fn()>,
}

#[async_trait(?Send)]
impl DaemonChannel for RootedPorts {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        if frame_op(&frame) == "ports.watch" {
            *self.told.borrow_mut() = None;
            let roots = fresh_roots(&self.area, &self.ws, &self.told).unwrap_or_default();
            let mut sent = frame;
            sent.insert("roots".to_string(), json!(roots));
            sent.insert("folder".to_string(), Value::String(self.folder.clone()));
            return self.channel.send(sent).await;
        }
        let op = frame_op(&frame).to_string();
        let reply = self.channel.send(frame).await?;
        if op == "pty.create" && is_ok(&reply) {
            if let Some(pid) = reply.get("pid").and_then(Value::as_i64) {
                let pty = js_str_opt(reply.get("ptyId"));
                self.area.port_pids.borrow_mut().entry(self.ws.clone()).or_default().insert(pty, pid);
                self.area.port_roots_moved(&self.ws);
            }
        }
        if op == "pty.list" && is_ok(&reply) {
            if let Some(rows) = reply.get("ptys").and_then(Value::as_array) {
                let standing: HashSet<String> =
                    rows.iter().filter(|row| row.get("exited") != Some(&Value::Bool(true))).map(|row| js_str_opt(row.get("id"))).collect();
                let mut gone = 0_usize;
                if let Some(ptys) = self.area.port_pids.borrow_mut().get_mut(&self.ws) {
                    let ids: Vec<String> = ptys.keys().cloned().collect();
                    for id in ids {
                        if !standing.contains(&id) {
                            ptys.shift_remove(&id);
                            gone += 1;
                        }
                    }
                }
                if gone > 0 {
                    self.area.port_roots_moved(&self.ws);
                }
            }
        }
        Ok(reply)
    }

    fn close(&self) {
        (self.stop)();
        self.channel.close();
    }

    async fn closed(&self) -> ChannelEnd {
        self.channel.closed().await
    }
}
