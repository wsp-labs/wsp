// SPDX-License-Identifier: AGPL-3.0-only
//! channels.ts:1-280: the facts of a record and the roads to a daemon. Spec: spec/channels_daemon.md.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use futures::channel::oneshot;
use futures::future::{select, Either, FutureExt, LocalBoxFuture, Shared};
use indexmap::IndexMap;
use serde_json::Value;

use wsp_core::daemon_channel::{ChannelError, DaemonChannel};
use wsp_core::devices::MintMore;
use wsp_core::types::internal::{is_on_default_branch, DaemonRefusal};
use wsp_core::types::wiring::WorkspaceRecord;
use wsp_engine::machine::MachineState;
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place_more::PlaceSettings;
use wsp_wire::views::workspace::{agents_from, DaemonReachView, ProjectView, ThreadLit, WorkspaceAgentsPatch, WorkspacePhase};
use wsp_wire::views::workspace_more::{ProjectRef, ThreadScope, WorkspaceAgents};
use wsp_wire::wire::daemon_version::PIPE_DAEMON_VERSION;
use wsp_words::exit::{usage_refusal, HostError, Kind};
use wsp_words::place_state::{place_at_limit_line, place_spend_limit};
use wsp_words::projects::{no_such_project_line, runs_in_folder};
use wsp_words::workspace_state::{absent_computer, is_local_workspace, kind_words, machine_word};

use super::{ChannelsArea, HELLO_WAIT_MS, OP_TOKEN_OPS};
use crate::account::core_types::{AccountError, AccountResult, Ask, Entry, Frame, Served, Work};

/// `{ deadlineMs }` of `turnLimitOf` (channels.ts:89): `f64::INFINITY` for no limit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TurnWall {
    pub deadline_ms: f64,
}

/// What `stdioRoad` answers (`StdioRoad`, slate-mcp-on.ts): frames out, events in, the road's end.
pub struct StdioRoad {
    pub send: Served,
    /// Adds a listener; the answer takes it off.
    pub listen: Rc<dyn Fn(Rc<dyn Fn(Value)>) -> Rc<dyn Fn()>>,
    pub channel: Rc<dyn DaemonChannel>,
}

/// A frame's work on this computer's daemon: the `ask` and the home it runs in.
pub type HomeWork<T> = Box<dyn FnOnce(Ask, String) -> futures::future::LocalBoxFuture<'static, AccountResult<T>>>;

type Hello = Shared<LocalBoxFuture<'static, Option<i64>>>;

/// A channel error as the area's error: a door's refusal and a token's keep their kind, the rest are plain.
pub(crate) fn channel_failure(e: ChannelError) -> AccountError {
    match e.kind() {
        Some("refused") => HostError::with_kind(e.to_string(), Kind::Refused).into(),
        Some("reauth") => HostError::with_kind(e.to_string(), Kind::Reauth).into(),
        _ => HostError::new(e.to_string()).into(),
    }
}

/// An area error as a channel's error, for a channel that answers from a road of this host.
pub(crate) fn channel_refused(e: AccountError) -> ChannelError {
    ChannelError::Refused { message: e.message() }
}

/// `String(v)` for the values a frame carries.
pub(crate) fn js_string(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && f.abs() < 9.0e15 => (f as i64).to_string(),
            _ => n.to_string(),
        },
        Value::String(s) => s.clone(),
        Value::Array(items) => items.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// `frame.op`, or nothing where the frame names none.
pub(crate) fn frame_op(frame: &Frame) -> &str {
    frame.get("op").and_then(Value::as_str).unwrap_or("")
}

/// A reply as `replyOf` (channels.ts:208) passes it: `ok: true` whole, else the daemon's refusal with its code.
pub(crate) fn reply_frame(frame: &Frame, reply: Frame) -> AccountResult<Frame> {
    if reply.get("ok") == Some(&Value::Bool(true)) {
        return Ok(reply);
    }
    let code = match reply.get("code") {
        Some(Value::String(code)) => Some(code.clone()),
        _ => None,
    };
    let message = match reply.get("error") {
        None | Some(Value::Null) => format!("{} was refused", frame_op(frame)),
        Some(error) => js_string(error),
    };
    Err(DaemonRefusal { code, message }.into())
}

/// A road that sends each frame over a channel and answers its reply through `replyOf` (channels.ts:214).
pub(crate) fn ask_over(channel: Rc<dyn DaemonChannel>) -> Ask {
    Rc::new(move |frame: Frame| -> LocalBoxFuture<'static, AccountResult<Frame>> {
        let channel = channel.clone();
        Box::pin(async move {
            let sent = frame.clone();
            let reply = channel.send(frame).await.map_err(channel_failure)?;
            reply_frame(&sent, reply)
        })
    })
}

/// A road that sends each frame up a computer's link and answers its reply through `replyOf` (channels.ts:160).
pub(crate) fn ask_served(served: Served) -> Ask {
    Rc::new(move |frame: Frame| -> LocalBoxFuture<'static, AccountResult<Frame>> {
        let served = served.clone();
        Box::pin(async move {
            let sent = frame.clone();
            let reply = served(frame).await?;
            reply_frame(&sent, reply)
        })
    })
}

/// A copy of a workspace record, read off its entry without holding the entry's borrow.
pub(crate) fn record_of(entry: &Entry) -> WorkspaceRecord {
    entry.borrow().record.borrow().clone()
}

/// Node's `path.normalize` on a path with no drive: `.` and empty segments go, `..` climbs.
fn normalize(p: &str) -> String {
    let absolute = p.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => match out.last() {
                Some(last) if *last != ".." => {
                    out.pop();
                }
                _ if !absolute => out.push(".."),
                _ => {}
            },
            s => out.push(s),
        }
    }
    let mut body = out.join("/");
    if body.is_empty() && !absolute {
        body = ".".to_string();
    }
    if !body.is_empty() && p.ends_with('/') {
        body.push('/');
    }
    if absolute {
        format!("/{body}")
    } else {
        body
    }
}

/// Node's `path.join` over the non-empty parts.
pub(crate) fn node_join(parts: &[&str]) -> String {
    let joined: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if joined.is_empty() {
        return ".".to_string();
    }
    normalize(&joined.join("/"))
}

/// Node's `path.relative` between two absolute paths: `""` where they are the same folder.
pub(crate) fn node_relative(from: &str, to: &str) -> String {
    let segments = |p: &str| -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for seg in p.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    out.pop();
                }
                s => out.push(s.to_string()),
            }
        }
        out
    };
    let (from, to) = (segments(from), segments(to));
    let common = from.iter().zip(to.iter()).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); from.len() - common];
    parts.extend(to[common..].iter().cloned());
    parts.join("/")
}

impl ChannelsArea {
    /// D1, channels.ts:33.
    pub fn started_as(&self, workspace_id: &str, thread_id: &str) -> Option<String> {
        self.ctx.index_start(workspace_id, thread_id)
    }

    /// D1, channels.ts:37.
    pub fn cut_before(&self, workspace_id: &str, thread_id: &str) -> bool {
        self.ctx.index_cut(workspace_id, thread_id).unwrap_or(false)
    }

    /// D2, channels.ts:44. `fact` is the TS key (`cwd`, `permissionMode`, ...).
    pub fn resumed_fact(&self, workspace_id: &str, resume: &str, fact: &str) -> Option<String> {
        for row in self.ctx.session_rows().iter().rev() {
            if row.workspace_id == workspace_id && row.claude_session_id.as_deref() == Some(resume) {
                if let Some(value) = row.facts.get(fact) {
                    return Some(value.clone());
                }
            }
        }
        self.ctx.index_fact(workspace_id, resume, fact)
    }

    /// D2, channels.ts:55.
    pub fn folder_of(&self, workspace_id: &str, resume: &str) -> Option<String> {
        self.resumed_fact(workspace_id, resume, "cwd")
    }

    /// D2, channels.ts:61.
    pub fn access_of(&self, workspace_id: &str, thread_id: &str, resume: Option<&str>) -> Option<String> {
        self.ctx
            .thread_record(thread_id)
            .and_then(|record| record.permission_mode)
            .or_else(|| resume.and_then(|resume| self.resumed_fact(workspace_id, resume, "permissionMode")))
    }

    /// D3, channels.ts:70.
    pub fn home_of(&self, record: &WorkspaceRecord) -> Option<String> {
        self.ctx.home_dir(record)
    }

    /// D4, channels.ts:80.
    pub fn provider_of(&self, record: &WorkspaceRecord) -> Option<String> {
        match &record.place {
            Some(place) if self.ctx.has_door() => self.ctx.offer_of(place),
            Some(_) => None,
            None if kind_words(&record.kind).driven => Some(self.ctx.wired()),
            None => None,
        }
    }

    /// D4, channels.ts:84.
    pub fn place_id_of(&self, record: &WorkspaceRecord) -> Option<String> {
        if is_local_workspace(Some(&record.kind)) {
            return Some(HERE_PLACE_ID.to_string());
        }
        record.place.clone().or_else(|| self.provider_of(record))
    }

    /// D4, channels.ts:86.
    pub fn settings_at(&self, place_id: Option<&str>) -> Option<PlaceSettings> {
        place_id.filter(|_| self.ctx.has_door()).and_then(|place| self.ctx.settings_at(place))
    }

    /// D5, channels.ts:89.
    pub fn turn_limit_of(&self, record: &WorkspaceRecord) -> Option<TurnWall> {
        let place_id = self.place_id_of(record)?;
        if !self.ctx.has_door() {
            return None;
        }
        let limit = self.ctx.turn_limit_at(&place_id)?;
        Some(TurnWall { deadline_ms: limit.map_or(f64::INFINITY, |ms| ms as f64) })
    }

    /// D6, channels.ts:95.
    pub fn agents_held(&self, record: &WorkspaceRecord) -> WorkspaceAgents {
        match &record.agents {
            Some(agents) => agents.clone(),
            None => {
                let place_id = self.place_id_of(record);
                self.spawn_at(place_id.as_deref())
            }
        }
    }

    /// D6, channels.ts:97.
    pub fn spawn_at(&self, place_id: Option<&str>) -> WorkspaceAgents {
        let patch = match self.settings_at(place_id).and_then(|settings| settings.spawn) {
            Some(spawn) => WorkspaceAgentsPatch { max_machines: spawn.max_machines, max_depth: spawn.max_depth },
            None => WorkspaceAgentsPatch { max_machines: None, max_depth: None },
        };
        agents_from(None, &patch)
    }

    /// D7, channels.ts:101.
    pub fn project_held(&self, id: &str) -> Result<ProjectView, HostError> {
        let held = self.ctx.projects_held();
        match held.get(id) {
            Some(project) => Ok(project.clone()),
            None => {
                let names: Vec<String> = held.values().map(|project| project.name.clone()).collect();
                Err(HostError::new(no_such_project_line(id, &names)))
            }
        }
    }

    /// D7, channels.ts:109.
    pub fn ref_of(&self, project: &ProjectView) -> ProjectRef {
        ProjectRef { id: project.id.clone(), name: project.name.clone(), path: project.path.clone(), computer: project.computer.clone() }
    }

    /// D8, channels.ts:114.
    pub fn checkout_of(&self, record: &WorkspaceRecord) -> Result<String, HostError> {
        if let Some(sub) = &record.sub {
            return Ok(sub.folder.clone());
        }
        let project = self.project_held(&record.project)?;
        match &record.worktree {
            Some(tree) if tree.gone.is_none() => {
                let top = project.git.as_ref().map_or(project.path.as_str(), |git| git.top.as_str());
                let inside = node_relative(top, &project.path);
                Ok(node_join(&[&tree.path, &inside]))
            }
            _ => Ok(project.path),
        }
    }

    /// D9, channels.ts:124.
    pub fn computer_of(&self, entry: &Entry) -> String {
        let record = record_of(entry);
        match &record.place {
            Some(place) if self.ctx.has_door() => self.ctx.name_of(place),
            Some(place) => place.clone(),
            None => machine_word(&record.kind),
        }
    }

    /// D10, channels.ts:130.
    pub fn served_by_its_computer(&self, entry: &Entry) -> Option<Served> {
        let machine = entry.borrow().machine.clone();
        if !machine.offers().daemon_frame {
            return None;
        }
        Some(Rc::new(move |frame: Frame| -> LocalBoxFuture<'static, AccountResult<Frame>> {
            let machine = machine.clone();
            Box::pin(async move { machine.daemon_frame(frame).await.map_err(|e| AccountError::Host(HostError::new(e.to_string()))) })
        }))
    }

    /// D12, channels.ts:138.
    pub async fn own_daemon_channel(&self, entry: &Entry, on_event: Rc<dyn Fn(Value)>) -> AccountResult<Rc<dyn DaemonChannel>> {
        let attempt: AccountResult<Rc<dyn DaemonChannel>> = async {
            let reach = self.ctx.daemon_road(entry).await?;
            let name = record_of(entry).name;
            self.channel_over(&reach, &name, on_event).await
        }
        .await;
        match attempt {
            Ok(channel) => Ok(channel),
            Err(e) => {
                let running = matches!(record_of(entry).phase, WorkspacePhase::Running);
                if running {
                    let machine = entry.borrow().machine.clone();
                    if matches!(machine.state().await, Ok(MachineState::Paused)) {
                        self.ctx.adopt_pause(entry).await?;
                    }
                }
                Err(e)
            }
        }
    }

    /// D11, channels.ts:146.
    pub async fn channel_over(
        &self,
        reach: &DaemonReachView,
        name: &str,
        on_event: Rc<dyn Fn(Value)>,
    ) -> AccountResult<Rc<dyn DaemonChannel>> {
        let Some(token) = reach.daemon_token.as_deref() else {
            return Err(HostError::new(format!("{name} is not answering yet")).into());
        };
        self.ctx.open_channel(&reach.url, token, on_event).await
    }

    /// D14, channels.ts:158.
    pub async fn with_daemon<T: 'static>(self: &Rc<Self>, entry: &Entry, work: Work<T>) -> AccountResult<T> {
        if let Some(served) = self.served_by_its_computer(entry) {
            return work(ask_served(served)).await;
        }
        if !self.brokered(entry) {
            let channel = self.own_daemon_channel(entry, Rc::new(|_: Value| {})).await?;
            return self.over_channel(channel, work).await;
        }
        let (hello_tx, hello_rx) = oneshot::channel::<Option<i64>>();
        let hello_slot = Rc::new(RefCell::new(Some(hello_tx)));
        let on_event: Rc<dyn Fn(Value)> = Rc::new(move |event: Value| {
            if event.get("type").and_then(Value::as_str) != Some("daemon.hello") {
                return;
            }
            if let Some(tx) = hello_slot.borrow_mut().take() {
                let version = event.get("version").and_then(Value::as_f64).map(|v| v as i64);
                let _ = tx.send(version);
            }
        });
        let hello: Hello = hello_rx.map(|heard| heard.unwrap_or(None)).boxed_local().shared();
        let channel = self.own_daemon_channel(entry, on_event).await?;
        let this = self.clone();
        let entry = entry.clone();
        self.over_channel(channel, Box::new(move |ask: Ask| work(this.with_op_tokens(&entry, ask, hello)))).await
    }

    /// D15, channels.ts:172.
    pub fn brokered(&self, entry: &Entry) -> bool {
        let record = record_of(entry);
        self.ctx.broker_ready()
            && !is_local_workspace(Some(&record.kind))
            && !runs_in_folder(&record.kind)
            && self.served_by_its_computer(entry).is_none()
            && record.place.as_ref().is_none_or(|place| !(self.ctx.has_door() && self.ctx.joined(place)))
    }

    /// D16, channels.ts:180.
    pub fn thread_on(&self, workspace_id: &str, named: Option<&Value>) -> Option<String> {
        if let Some(Value::String(thread_id)) = named {
            if self.ctx.thread_record(thread_id).is_some_and(|record| record.workspace_id == workspace_id) {
                return Some(thread_id.clone());
            }
        }
        let mut newest = None;
        for thread_id in self.ctx.thread_ids() {
            if self.ctx.thread_record(&thread_id).is_some_and(|record| record.workspace_id == workspace_id) {
                newest = Some(thread_id);
            }
        }
        newest
    }

    /// D16, channels.ts:189: `ask` wrapped so each op of `OP_TOKEN_OPS` carries a token of its own.
    pub fn with_op_tokens(self: &Rc<Self>, entry: &Entry, ask: Ask, hello: Hello) -> Ask {
        let this = self.clone();
        let entry = entry.clone();
        Rc::new(move |frame: Frame| -> LocalBoxFuture<'static, AccountResult<Frame>> {
            let this = this.clone();
            let entry = entry.clone();
            let ask = ask.clone();
            let hello = hello.clone();
            Box::pin(async move { this.op_token_frame(&entry, ask, hello, frame).await })
        })
    }

    async fn op_token_frame(self: &Rc<Self>, entry: &Entry, ask: Ask, hello: Hello, frame: Frame) -> AccountResult<Frame> {
        let op = frame_op(&frame).to_string();
        if !OP_TOKEN_OPS.contains(&op.as_str()) {
            return ask(frame).await;
        }
        let record = record_of(entry);
        let Some(thread_id) = self.thread_on(&record.id, None) else {
            return Err(usage_refusal(
                &format!("{} has no thread yet, so wsp cannot reach GitHub for it.", record.name),
                "Start a thread on it first.",
            )
            .into());
        };
        let (fired_tx, fired_rx) = oneshot::channel::<()>();
        let cancel = self.ctx.schedule(
            HELLO_WAIT_MS,
            Box::new(move || {
                let _ = fired_tx.send(());
            }),
        );
        let heard = match select(hello.clone(), fired_rx).await {
            Either::Left((version, _)) => version,
            Either::Right(_) => None,
        };
        cancel();
        if !matches!(heard, Some(version) if version >= PIPE_DAEMON_VERSION) {
            return ask(frame).await;
        }
        let scope = ThreadScope {
            kind: ThreadLit,
            thread_id: thread_id.clone(),
            workspace_id: record.id.clone(),
            root_thread_id: self.ctx.root_of(&thread_id),
        };
        let road = self.ctx.turn_road(record.kind);
        let minted =
            self.ctx.device_door().mint(&format!("op {op}"), scope, self.ctx.now(), MintMore { road, ..MintMore::default() }).await?;
        let mut sent = frame;
        sent.insert("token".to_string(), Value::String(minted.device_token.clone()));
        let answer = ask(sent).await;
        if let Err(e) = self.ctx.device_door().revoke(&minted.device_id).await {
            self.ctx.warn(&format!("the token of {op} on {} was not taken away: {}", record.name, e.message));
        }
        answer
    }

    /// D13, channels.ts:208.
    pub fn reply_of(&self, frame: &Frame, reply: Frame) -> AccountResult<Frame> {
        reply_frame(frame, reply)
    }

    /// D13, channels.ts:212.
    pub async fn over_channel<T: 'static>(&self, channel: Rc<dyn DaemonChannel>, work: Work<T>) -> AccountResult<T> {
        let result = work(ask_over(channel.clone())).await;
        channel.close();
        result
    }

    /// D17, channels.ts:223. `need` defaults to `STDIO_DAEMON_VERSION` at the call sites that name none.
    pub async fn stdio_road(&self, entry: &Entry, need: i64) -> AccountResult<StdioRoad> {
        let listeners: Rc<RefCell<IndexMap<u64, Rc<dyn Fn(Value)>>>> = Rc::default();
        let next_id = Rc::new(Cell::new(0_u64));
        let heard: Rc<dyn Fn(Value)> = {
            let listeners = listeners.clone();
            Rc::new(move |event: Value| {
                let fns: Vec<Rc<dyn Fn(Value)>> = listeners.borrow().values().cloned().collect();
                for f in fns {
                    f(event.clone());
                }
            })
        };
        let listen: Rc<dyn Fn(Rc<dyn Fn(Value)>) -> Rc<dyn Fn()>> = {
            let listeners = listeners.clone();
            Rc::new(move |f: Rc<dyn Fn(Value)>| {
                let id = next_id.get();
                next_id.set(id + 1);
                listeners.borrow_mut().insert(id, f);
                let listeners = listeners.clone();
                let remove: Rc<dyn Fn()> = Rc::new(move || {
                    listeners.borrow_mut().shift_remove(&id);
                });
                remove
            })
        };
        let Some(served) = self.served_by_its_computer(entry) else {
            let channel = self.own_daemon_channel(entry, heard).await?;
            return Ok(StdioRoad { send: ask_over(channel.clone()), listen, channel });
        };
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
        if let Some(version) = version {
            if version < need {
                return Err(HostError::new(wsp_wire::wire::daemon_version::slate_server_behind_line(&self.ctx.name_of(&place))).into());
            }
        }
        let channel = match self.ctx.link_channel(&place, heard) {
            Some(channel) => channel,
            None => return Err(HostError::new(absent_computer(&self.ctx.name_of(&place), None).sentence).into()),
        };
        Ok(StdioRoad { send: ask_served(served), listen, channel })
    }

    /// D18, channels.ts:247.
    pub async fn on_this_computer<T: 'static>(&self, work: HomeWork<T>) -> AccountResult<T> {
        if !self.ctx.local_has_daemon() {
            return Err(DaemonRefusal {
                code: Some("no-host-cli".to_string()),
                message: "this host runs no daemon on this computer".to_string(),
            }
            .into());
        }
        let home = self.ctx.local_home_dir();
        let road = self.ctx.local_road().await?;
        let channel = self.channel_over(&road, wsp_words::words::computer::THIS_COMPUTER, Rc::new(|_: Value| {})).await?;
        self.over_channel(channel, Box::new(move |ask: Ask| work(ask, home))).await
    }

    /// D19, channels.ts:255.
    pub async fn place_guard(&self, place_id: &str) -> Result<(), HostError> {
        if !self.ctx.has_door() {
            return Ok(());
        }
        let rows = self.ctx.rows().await;
        let Some(row) = rows.iter().find(|row| row.id == place_id) else {
            return Ok(());
        };
        let Some(limit) = place_spend_limit(row) else {
            return Ok(());
        };
        let spend = self.ctx.spend_today(&rows, self.ctx.now()).await;
        let today = spend.iter().find(|(place, _)| place == place_id).and_then(|(_, usd)| *usd);
        if let Some(today) = today {
            if place_at_limit_line(row, Some(today)).is_some() {
                return Err(HostError::new(wsp_words::words::computer::spend_cap_refusal(&row.name, today, limit)));
            }
        }
        Ok(())
    }

    /// D20, channels.ts:268.
    pub async fn place_refuses(&self, place_id: Option<&str>) -> Result<(), HostError> {
        match self.blocked_line(place_id).await {
            Some(line) => Err(HostError::new(line)),
            None => Ok(()),
        }
    }

    /// D20, channels.ts:272.
    pub async fn blocked_line(&self, place_id: Option<&str>) -> Option<String> {
        let place = place_id?;
        if !self.ctx.has_door() {
            return None;
        }
        let report = self.ctx.report_of(place).await?;
        wsp_wire::wire::machine_link::place_blocked(&self.ctx.name_of(place), report.runs_workspaces, report.workspaces_blocked.as_deref())
    }

    /// D20, channels.ts:278.
    pub async fn copy_blocked(&self, entry: &Entry) -> Result<(), HostError> {
        let record = record_of(entry);
        if runs_in_folder(&record.kind) {
            return Ok(());
        }
        self.place_refuses(record.place.as_deref()).await
    }
}

/// Whether a daemon refusal is the default-branch one (`isOnDefaultBranch`).
pub(crate) fn is_default_branch_error(e: &AccountError) -> bool {
    matches!(e, AccountError::Refusal(refusal) if is_on_default_branch(refusal))
}
