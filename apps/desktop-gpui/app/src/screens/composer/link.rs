// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's road to the host, through `wsp-host-client` alone: it reads the agents and their choices
//! (`harnesses.list`), the thread's rows (`sessions.list`) and its transcript live, and answers the composer's events
//! with `sessions.start`, `sessions.steer`, `sessions.interrupt`, `preferences.set` with `sessions.access`, and the
//! daemon's `fs.files` for the mention menu.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{AsyncWindowContext, Context, Entity, Subscription, Task, WeakEntity, Window};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{self, SessionEvent, SessionView};
use wsp_host_client::{Failure, HostClient, Live, Op};

use super::model::{Agent, Command, Hold, Picks};
use super::{Composer, ComposerEvent, Outgoing};

/// The thread the composer writes into: its workspace, and its id once it has one.
#[derive(Clone, Debug)]
pub struct Target {
    pub workspace_id: String,
    pub thread_id: Option<String>,
}

/// What the host said about the thread when it was opened.
pub struct Opened {
    pub agents: Vec<Agent>,
    pub rows: Vec<SessionView>,
    pub history: Vec<SessionEvent>,
}

enum Arrival {
    /// The transcript as it stood when the thread was followed, which may already hold a turn this link started.
    History(Vec<SessionEvent>),
    Event(Box<SessionEvent>),
    Rows(Vec<SessionView>),
    Failed(String),
}

pub struct Link {
    host: Arc<HostClient>,
    runtime: Arc<Runtime>,
    target: Target,
    composer: WeakEntity<Composer>,
    rows: Vec<SessionView>,
    /// The turn the thread runs now, as its transcript says: the agent's session id in its events.
    running_session: Option<String>,
    _follow: Option<(tokio::task::AbortHandle, Task<()>)>,
    _events: Subscription,
}

impl Drop for Link {
    fn drop(&mut self) {
        if let Some((task, _)) = &self._follow {
            task.abort();
        }
    }
}

/// One op on the runtime, its reply back on the window's thread.
async fn call<O: Op + Send + 'static>(runtime: &Runtime, host: &Arc<HostClient>, op: O) -> Result<O::Reply, Failure>
where
    O::Reply: Send + 'static,
{
    let host = host.clone();
    runtime.spawn(async move { host.call(op).await }).await.unwrap_or_else(|e| Err(Failure::new(e.to_string())))
}

/// A typed request built from the JSON the web app sends, so a field the record names in camelCase is written as such.
fn request<O: Op + serde::de::DeserializeOwned>(params: Value) -> Result<O, Failure> {
    serde_json::from_value(params).map_err(|e| Failure::new(format!("{}: {e}", O::NAME)))
}

/// A request id per send, as the web app mints one for each `sessions.start` and `sessions.steer`.
fn request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("gpui-{now}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

fn kind(event: &SessionEvent) -> (String, Value) {
    let value = serde_json::to_value(event).unwrap_or_default();
    (value.get("type").and_then(Value::as_str).unwrap_or_default().to_owned(), value)
}

/// Whether the transcript holds a message, whether its last turn still runs (a `session.start` with no
/// `session.end` after it), that turn's session id, and the commands its agent announced last.
pub fn transcript_state(events: &[SessionEvent]) -> (bool, Option<String>, Vec<Command>) {
    let mut any = false;
    let mut running = None;
    let mut commands = Vec::new();
    for event in events {
        let (kind, value) = kind(event);
        match kind.as_str() {
            "session.start" => {
                any = true;
                running = Some(value.get("sessionId").and_then(Value::as_str).unwrap_or_default().to_owned());
                if let Some(slash) = value.pointer("/harness/slashCommands").and_then(Value::as_array) {
                    commands = slash
                        .iter()
                        .filter_map(Value::as_str)
                        .map(|name| {
                            let name = name.trim_start_matches('/');
                            let source = name.split_once(':').map(|(source, _)| source.to_owned());
                            Command { name: name.to_owned(), description: None, source }
                        })
                        .collect();
                }
            }
            "session.end" => running = None,
            _ => {}
        }
    }
    (any, running, commands)
}

/// The agent and the choices the thread ran with, off its newest row.
pub fn ran(rows: &[SessionView]) -> Picks {
    let Some(row) = rows.last() else { return Picks::default() };
    Picks {
        agent: Some(row.harness.clone()),
        model: row.model.clone(),
        effort: row.effort.clone(),
        window: row.context_window.clone(),
        access: row.permission_mode.clone(),
    }
}

/// The host's own session id for the turn the events call `event_session` (ChatComposer.tsx, the stop's lookup):
/// the row it names that is running, else the last it names, else the event's id for the runtime to answer, else
/// the thread's newest row.
pub fn session_id(rows: &[SessionView], event_session: Option<&str>) -> Option<String> {
    let named: Vec<&SessionView> =
        rows.iter().filter(|r| event_session.is_some_and(|s| r.id == s || r.claude_session_id.as_deref() == Some(s))).collect();
    let running = |r: &&SessionView| serde_json::to_value(r.status).ok().and_then(|v| v.as_str().map(|s| s == "running")).unwrap_or(false);
    named
        .iter()
        .find(|r| running(r))
        .or(named.last())
        .map(|r| r.id.clone())
        .or_else(|| event_session.map(str::to_owned))
        .or_else(|| rows.iter().rev().find(|r| running(r)).or(rows.last()).map(|r| r.id.clone()))
}

impl Link {
    /// Reads what the composer needs for `target`, from the agents to the transcript.
    pub async fn read(host: &Arc<HostClient>, runtime: &Runtime, target: &Target) -> Result<Opened, Failure> {
        let listed = call(runtime, host, request::<protocol::HarnessesListRequest>(json!({ "workspaceId": target.workspace_id }))?).await?;
        let agents = listed.harnesses.iter().filter_map(Agent::read).collect();
        let (rows, history) = match &target.thread_id {
            Some(thread) => {
                let rows = rows_of(host, runtime, &target.workspace_id, thread).await?;
                let history = {
                    let (host, ws, thread) = (host.clone(), target.workspace_id.clone(), thread.clone());
                    runtime
                        .spawn(async move { host.open(&ws, &thread).await.map(|t| t.history) })
                        .await
                        .unwrap_or_else(|e| Err(Failure::new(e.to_string())))?
                };
                (rows, history)
            }
            None => (Vec::new(), Vec::new()),
        };
        Ok(Opened { agents, rows, history })
    }

    /// Ties `composer` to the host for `target`: shows what `opened` holds, follows the thread live, and answers every
    /// event the composer emits.
    pub fn new(
        host: Arc<HostClient>,
        runtime: Arc<Runtime>,
        target: Target,
        opened: Opened,
        composer: Entity<Composer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (any, running, commands) = transcript_state(&opened.history);
        let picks = ran(&opened.rows);
        let computer = opened.rows.last().and_then(|r| r.computer_name.clone());
        let no_agents = opened.agents.is_empty();
        composer.update(cx, |c, cx| {
            c.set_catalog(opened.agents, picks, commands, cx);
            c.set_turn(any, running.is_some(), cx);
            c.set_place(computer, None, cx);
            c.set_hold(no_agents.then_some(Hold::NoAgents), cx);
        });
        let events = cx.subscribe_in(&composer, window, Self::on_event);
        let mut link = Link {
            host,
            runtime,
            target,
            composer: composer.downgrade(),
            rows: opened.rows,
            running_session: running,
            _follow: None,
            _events: events,
        };
        link.follow(window, cx);
        link
    }

    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(thread) = self.target.thread_id.clone() else { return };
        let (host, ws) = (self.host.clone(), self.target.workspace_id.clone());
        let (tx, mut rx) = mpsc::unbounded();
        // Follows again after a lost socket, a second, then up to five, apart, reading the transcript whole each time.
        let task = self.runtime.spawn(async move {
            let mut wait = Duration::from_secs(1);
            loop {
                let followed: Result<(), Failure> = async {
                    let mut live = host.open(&ws, &thread).await?;
                    wait = Duration::from_secs(1);
                    _ = tx.unbounded_send(Arrival::History(std::mem::take(&mut live.history)));
                    while let Some(next) = live.next().await {
                        let Ok(Live::Event(event)) = next else { continue };
                        let ends = matches!(kind(&event).0.as_str(), "session.start" | "session.end");
                        _ = tx.unbounded_send(Arrival::Event(event));
                        if ends {
                            if let Ok(listed) = host.call(request::<protocol::SessionsListRequest>(json!({ "workspaceId": ws }))?).await {
                                _ = tx.unbounded_send(Arrival::Rows(thread_rows(listed.sessions, &thread)));
                            }
                        }
                    }
                    Err(Failure::new("the host closed the socket"))
                }
                .await;
                if let Err(failure) = followed {
                    if tx.unbounded_send(Arrival::Failed(failure.message)).is_err() {
                        return;
                    }
                }
                tokio::time::sleep(wait).await;
                wait = (wait * 2).min(Duration::from_secs(5));
            }
        });
        let receive = cx.spawn_in(window, async move |this, cx: &mut AsyncWindowContext| {
            while let Some(arrival) = rx.next().await {
                if this.update_in(cx, |this, window, cx| this.arrive(arrival, window, cx)).is_err() {
                    break;
                }
            }
        });
        self._follow = Some((task.abort_handle(), receive));
    }

    fn arrive(&mut self, arrival: Arrival, _: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        match arrival {
            Arrival::History(events) => {
                let (any, running, _) = transcript_state(&events);
                self.running_session = running.clone();
                composer.update(cx, |c, cx| {
                    if c.hold() == Some(&Hold::HostDown) {
                        c.set_hold(None, cx);
                    }
                    c.set_turn(any, running.is_some(), cx)
                });
            }
            Arrival::Event(event) => {
                let (kind, value) = kind(&event);
                match kind.as_str() {
                    "session.start" => {
                        self.running_session = value.get("sessionId").and_then(Value::as_str).map(str::to_owned);
                        let commands = transcript_state(std::slice::from_ref(&event)).2;
                        composer.update(cx, |c, cx| {
                            if !commands.is_empty() {
                                let (agents, ran) = (c.agents.clone(), c.ran.clone());
                                c.set_catalog(agents, ran, commands, cx);
                            }
                            c.set_turn(true, true, cx);
                        });
                    }
                    "session.end" => {
                        self.running_session = None;
                        composer.update(cx, |c, cx| c.set_turn(true, false, cx));
                    }
                    _ => {}
                }
            }
            Arrival::Rows(rows) => {
                self.rows = rows;
                let picks = ran(&self.rows);
                composer.update(cx, |c, cx| {
                    let (agents, commands) = (c.agents.clone(), c.commands.clone());
                    c.set_catalog(agents, picks, commands, cx);
                });
            }
            Arrival::Failed(failure) => {
                eprintln!("desktop-gpui: composer: {failure}");
                composer.update(cx, |c, cx| c.set_hold(Some(Hold::HostDown), cx));
            }
        }
    }

    fn started(&mut self, session: SessionView, window: &mut Window, cx: &mut Context<Self>) {
        if self.target.thread_id.is_some() {
            return;
        }
        self.target.thread_id = session.thread_id.clone().or_else(|| Some(session.id.clone()));
        self.rows = vec![session];
        self.follow(window, cx);
    }

    fn on_event(&mut self, composer: &Entity<Composer>, event: &ComposerEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (host, runtime, target) = (self.host.clone(), self.runtime.clone(), self.target.clone());
        let weak = composer.downgrade();
        let session = session_id(&self.rows, self.running_session.as_deref());
        match event.clone() {
            ComposerEvent::Start(out) => {
                let params = start_params(&target, &out, !self.rows.is_empty());
                cx.spawn_in(window, async move |this, cx| {
                    let started = match request::<protocol::SessionsStartRequest>(params) {
                        Ok(op) => call(&runtime, &host, op).await,
                        Err(e) => Err(e),
                    };
                    match started {
                        // A send that opened the thread names it; from here the link follows it as any other.
                        Ok(started) => {
                            _ = weak.update(cx, |c, cx| c.sent(cx));
                            _ = this.update_in(cx, |link, window, cx| link.started(started.session, window, cx));
                        }
                        Err(failure) => {
                            eprintln!("desktop-gpui: sessions.start: {}", failure.message);
                            _ = weak.update_in(cx, |c, window, cx| c.refused_send(out, window, cx));
                        }
                    }
                })
                .detach();
            }
            ComposerEvent::Steer(out) => {
                let Some(session) = session else {
                    composer.update(cx, |c, cx| c.steered(false, cx));
                    return;
                };
                let params =
                    json!({ "sessionId": session, "prompt": out.prompt, "requestId": request_id(), "attachments": attachments(&out) });
                cx.spawn_in(window, async move |_, cx| {
                    let taken = match request::<protocol::SessionsSteerRequest>(params) {
                        Ok(op) => call(&runtime, &host, op).await.map(|r| r.outcome),
                        Err(e) => Err(e),
                    };
                    let taken = matches!(&taken, Ok(outcome) if serde_json::to_value(outcome).ok().and_then(|v| v.as_str().map(|s| s == "accepted")).unwrap_or(false));
                    _ = weak.update(cx, |c, cx| c.steered(taken, cx));
                })
                .detach();
            }
            ComposerEvent::Stop => {
                let Some(session) = session else {
                    composer.update(cx, |c, cx| c.stopped(cx));
                    return;
                };
                cx.spawn_in(window, async move |_, cx| {
                    // Stop stays pressed until the turn ends; only a stop the host did not take lets go of it.
                    let asked = match request::<protocol::SessionsInterruptRequest>(json!({ "sessionId": session })) {
                        Ok(op) => call(&runtime, &host, op).await.map(|_| ()),
                        Err(e) => Err(e),
                    };
                    if let Err(failure) = asked {
                        eprintln!("desktop-gpui: sessions.interrupt: {}", failure.message);
                        _ = weak.update(cx, |c, cx| c.stopped(cx));
                    }
                })
                .detach();
            }
            ComposerEvent::Access(mode) => {
                let patch = json!({ "patch": { "access": { target.workspace_id.clone(): mode } } });
                cx.spawn_in(window, async move |_, _| {
                    if let Ok(op) = request::<protocol::PreferencesSetRequest>(patch) {
                        _ = call(&runtime, &host, op).await;
                    }
                    if let Some(session) = session {
                        if let Ok(op) = request::<protocol::SessionsAccessRequest>(json!({ "sessionId": session, "permissionMode": mode }))
                        {
                            _ = call(&runtime, &host, op).await;
                        }
                    }
                })
                .detach();
            }
            ComposerEvent::Files => {
                let cwd = self.rows.last().and_then(|r| r.cwd.clone());
                cx.spawn_in(window, async move |_, cx| {
                    let files = list_files(&runtime, &host, &target.workspace_id, cwd).await.unwrap_or_else(|failure| {
                        eprintln!("desktop-gpui: fs.files: {}", failure.message);
                        Vec::new()
                    });
                    _ = weak.update_in(cx, |c, window, cx| c.set_folder_files(files, window, cx));
                })
                .detach();
            }
        }
    }
}

/// Every row of one thread, oldest first.
pub async fn rows_of(host: &Arc<HostClient>, runtime: &Runtime, workspace_id: &str, thread: &str) -> Result<Vec<SessionView>, Failure> {
    let listed = call(runtime, host, request::<protocol::SessionsListRequest>(json!({ "workspaceId": workspace_id }))?).await?;
    Ok(thread_rows(listed.sessions, thread))
}

/// One thread's rows out of a workspace's, oldest first.
fn thread_rows(sessions: Vec<SessionView>, thread: &str) -> Vec<SessionView> {
    let mut rows: Vec<SessionView> = sessions.into_iter().filter(|r| r.thread_id.as_deref() == Some(thread)).collect();
    rows.sort_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.)));
    rows
}

/// The files of the thread's folder, as the mention menu lists them: the daemon's `fs.files` over a channel.
pub async fn list_files(
    runtime: &Runtime,
    host: &Arc<HostClient>,
    workspace_id: &str,
    cwd: Option<String>,
) -> Result<Vec<String>, Failure> {
    let opened = call(runtime, host, request::<protocol::DaemonOpenRequest>(json!({ "workspaceId": workspace_id }))?).await?;
    let channel = serde_json::to_value(&opened).ok().and_then(|v| v.get("channel").and_then(Value::as_str).map(str::to_owned));
    let channel = channel.ok_or_else(|| Failure::new("daemon.open answered no channel"))?;
    let frame = json!({ "op": "fs.files", "cwd": cwd.unwrap_or_default() });
    let sent = call(runtime, host, request::<protocol::DaemonSendRequest>(json!({ "channel": channel, "frame": frame }))?).await;
    if let Ok(op) = request::<protocol::DaemonCloseRequest>(json!({ "channel": channel })) {
        _ = call(runtime, host, op).await;
    }
    let reply = sent?.reply;
    if reply.get("ok").and_then(Value::as_bool) == Some(false) {
        return Err(Failure::new(reply.get("error").and_then(Value::as_str).unwrap_or("fs.files failed").to_owned()));
    }
    Ok(reply
        .get("files")
        .and_then(Value::as_array)
        .map(|f| f.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default())
}

fn attachments(out: &Outgoing) -> Value {
    if out.files.is_empty() {
        return Value::Null;
    }
    out.files
        .iter()
        .map(|f| json!({ "mediaType": f.media_type, "bytes": base64::engine::general_purpose::STANDARD.encode(f.bytes.as_slice()), "name": f.name }))
        .collect()
}

/// `sessions.start` as the web app's `start()` writes it: a thread that has run keeps its agent and access, so only
/// the model and reasoning go with it; a new thread carries everything.
pub fn start_params(target: &Target, out: &Outgoing, ran: bool) -> Value {
    let mut params = json!({ "workspaceId": target.workspace_id, "prompt": out.prompt, "requestId": request_id() });
    let set = |params: &mut Value, key: &str, value: Option<&str>| {
        if let Some(value) = value {
            params[key] = Value::from(value);
        }
    };
    set(&mut params, "thread", target.thread_id.as_deref());
    let r = &out.picks;
    if !ran {
        set(&mut params, "harness", r.agent.as_ref().map(|a| a.harness.as_str()));
        set(&mut params, "permissionMode", r.access.as_ref().map(|a| a.value.as_str()));
    }
    set(&mut params, "model", r.model.as_ref().map(|m| m.value.as_str()));
    set(&mut params, "effort", r.effort.as_ref().map(|e| e.value.as_str()));
    if r.model.is_some() {
        set(&mut params, "contextWindow", r.window.as_ref().map(|w| w.value.as_str()));
    }
    let files = attachments(out);
    if !files.is_null() {
        params["attachments"] = files;
    }
    params
}

#[cfg(test)]
mod tests {
    use super::super::model::{Choice, File, Resolved};
    use super::*;

    fn choice(v: &str) -> Choice {
        Choice { value: v.into(), label: v.into(), ..Default::default() }
    }

    #[test]
    fn a_send_on_a_thread_that_ran_keeps_its_agent_and_access() {
        let target = Target { workspace_id: "w".into(), thread_id: Some("t".into()) };
        let picks = Resolved {
            agent: Some(Agent { harness: "claude".into(), ..Default::default() }),
            model: Some(choice("opus")),
            effort: Some(choice("high")),
            window: Some(choice("1m")),
            access: Some(choice("plan")),
            ..Default::default()
        };
        let file = File { name: "a.txt".into(), media_type: "text/plain".into(), bytes: Arc::new(b"hi".to_vec()) };
        let out = Outgoing { prompt: "go".into(), files: vec![file], picks };
        let ran = start_params(&target, &out, true);
        assert_eq!(ran["thread"], "t");
        assert_eq!(ran["model"], "opus");
        assert_eq!(ran["contextWindow"], "1m");
        assert!(ran.get("harness").is_none() && ran.get("permissionMode").is_none());
        assert_eq!(ran["attachments"][0]["bytes"], "aGk=");
        let fresh = start_params(&target, &out, false);
        assert_eq!(fresh["harness"], "claude");
        assert_eq!(fresh["permissionMode"], "plan");
        request::<protocol::SessionsStartRequest>(fresh).expect("the params read as the recorded request");
    }
}
