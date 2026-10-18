// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's road to the host, through `wsp-host-client` alone: it reads the agents and their choices
//! (`harnesses.list`), the thread's rows (`sessions.list`) and its transcript live, and answers the composer's events
//! with `sessions.start`, `sessions.steer`, `sessions.interrupt`, `preferences.set` with `sessions.access`, and the
//! daemon's `fs.files` for the mention menu.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine as _;
use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{AsyncWindowContext, Context, Entity, Subscription, Task, WeakEntity, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{self, SessionEvent, SessionView};
use wsp_host_client::{Failure, Live, Op};

use super::model::{Agent, Command, Drawer, Hold, HostItem, Picks, Skill};
use super::{Composer, ComposerEvent, Outgoing};
use crate::screens::Host;

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
    host: Host,
    target: Target,
    composer: WeakEntity<Composer>,
    rows: Vec<SessionView>,
    /// The turn the thread runs now, as its transcript says: the agent's session id in its events.
    running_session: Option<String>,
    /// The thread's sent prompts by id, oldest first, for the box's Up and Down.
    prompts: Vec<(String, String)>,
    /// The step list of the turn running now, as its last `session.plan` gave it.
    plan: Option<Value>,
    /// The thread followed: the host task, stopped when this is dropped, and its arrivals read on the window's thread.
    _follow: Option<(Task<()>, Task<()>)>,
    _events: Subscription,
}

/// One op on the host's runtime, its reply back on the window's thread.
async fn call<O: Op + Send + 'static>(host: &Host, op: O) -> Result<O::Reply, Failure>
where
    O::Reply: Send + 'static,
{
    host.run(move |client| async move { client.call(op).await }).await
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

/// The thread's sent prompts by id, oldest first: each turn's and each steer's.
pub fn prompts_of(events: &[SessionEvent]) -> Vec<(String, String)> {
    events
        .iter()
        .filter_map(|e| {
            let (kind, value) = kind(e);
            if !matches!(kind.as_str(), "session.start" | "session.steer") {
                return None;
            }
            let prompt = value.get("prompt")?.as_str()?.to_owned();
            let id = value.get("pos").map(Value::to_string).or_else(|| value.get("turnId").map(Value::to_string)).unwrap_or_default();
            Some((id, prompt))
        })
        .collect()
}

/// The running turn's last step list, read off its `session.plan` events.
pub fn plan_of(events: &[SessionEvent]) -> Option<Value> {
    let mut plan = None;
    for e in events {
        match kind(e) {
            (k, _) if k == "session.start" => plan = None,
            (k, v) if k == "session.plan" => plan = v.get("steps").cloned(),
            _ => {}
        }
    }
    plan
}

/// `n word`, the word plural past one.
fn plural(n: i64, word: &str) -> String {
    if n == 1 {
        format!("1 {word}")
    } else {
        format!("{n} {word}s")
    }
}

/// The clock time `at` (ms since the epoch) reads on this computer, with its weekday when it is not today
/// (usage.ts `clockWord`).
pub fn clock_word(at: f64, now: std::time::SystemTime) -> String {
    let local = |secs: i64| {
        let t = secs as libc::time_t;
        // SAFETY: localtime_r writes only into the tm it is handed.
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        unsafe { libc::localtime_r(&t, &mut tm) };
        tm
    };
    let then = local((at / 1000.) as i64);
    let today = local(now.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0));
    let time = format!("{:02}:{:02}", then.tm_hour, then.tm_min);
    if (then.tm_year, then.tm_yday) == (today.tm_year, today.tm_yday) {
        return time;
    }
    let day = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][then.tm_wday.rem_euclid(7) as usize];
    format!("{day} {time}")
}

/// What waits around the composer, off the thread's newest row and the running turn's step list (ChatComposer.tsx's
/// `drawer`, `capRunningLine` and `LIMIT_WORDS`).
pub fn drawer_of(rows: &[SessionView], plan: Option<&Value>, running: bool, agents: &[Agent]) -> Drawer {
    let Some(row) = rows.last() else { return Drawer::default() };
    let row_value = serde_json::to_value(row).unwrap_or_default();
    let agent = agents.iter().find(|a| a.harness == row.harness).map_or(row.harness.clone(), |a| a.label.clone());
    let failed = row_value.get("status").and_then(Value::as_str) == Some("failed");
    let limit = row_value.get("limit").filter(|_| failed).map(|limit| {
        let name = if row.resume_at.is_some() { "Resumes at the reset" } else { "Usage limit reached" }.to_owned();
        let line = match limit.get("resetsAt").and_then(Value::as_f64) {
            Some(at) => format!("{agent} resumes at {}", clock_word(at, std::time::SystemTime::now())),
            None => format!("{agent} stopped this turn and did not say when its plan resets."),
        };
        (name, line)
    });
    let slot = row_value.get("capped").map(|c| {
        let place = c.get("place").and_then(Value::as_str).unwrap_or_default();
        let (on, of) = (c.get("running").and_then(Value::as_i64).unwrap_or(0), c.get("atOnce").and_then(Value::as_i64).unwrap_or(0));
        match c.get("why").and_then(Value::as_str) {
            Some("minute") => format!("{place}'s starts this minute are used: {on} of {of}"),
            Some("slot") => format!("{place} is running {on} of {}", plural(of, "machine")),
            _ => format!("{place} is running {on} of {}", plural(of, "thread")),
        }
    });
    let tasks = plan.filter(|_| running).and_then(Value::as_array).and_then(|steps| {
        let state = |s: &Value| s.get("state").and_then(Value::as_str).unwrap_or_default().to_owned();
        let current = steps.iter().find(|s| state(s) == "working").or_else(|| steps.iter().find(|s| state(s) == "pending"))?;
        let done = steps.iter().filter(|s| state(s) == "done").count();
        Some((current.get("text").and_then(Value::as_str).unwrap_or_default().to_owned(), format!("{done}/{}", steps.len())))
    });
    Drawer { question: row.asking.clone().filter(|_| running), limit, slot, tasks }
}

/// The skills `agents.read` reports that the thread's agent reads (composerMenuSearch.ts `skillsForHarness`).
pub fn skills_of(report: &Value, harness: &str) -> Vec<Skill> {
    let rows = report.get("skills").and_then(Value::as_array).cloned().unwrap_or_default();
    rows.iter()
        .filter(|s| {
            s.get("paths").and_then(Value::as_array).is_some_and(|paths| {
                paths.iter().any(|p| {
                    p.get("off").and_then(Value::as_bool) != Some(true)
                        && p.get("agent").and_then(Value::as_str).is_none_or(|a| a == harness)
                })
            })
        })
        .filter_map(|s| {
            Some(Skill {
                name: s.get("name")?.as_str()?.to_owned(),
                description: s.get("description").and_then(Value::as_str).map(str::to_owned),
            })
        })
        .collect()
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
        fast: row.fast,
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
    pub async fn read(host: &Host, target: &Target) -> Result<Opened, Failure> {
        let listed = call(host, request::<protocol::HarnessesListRequest>(json!({ "workspaceId": target.workspace_id }))?).await?;
        let agents = listed.harnesses.iter().filter_map(Agent::read).collect();
        let (rows, history) = match &target.thread_id {
            Some(thread) => {
                let rows = rows_of(host, &target.workspace_id, thread).await?;
                let (ws, thread) = (target.workspace_id.clone(), thread.clone());
                let history = host.run(move |client| async move { client.open(&ws, &thread).await.map(|t| t.history) }).await?;
                (rows, history)
            }
            None => (Vec::new(), Vec::new()),
        };
        Ok(Opened { agents, rows, history })
    }

    /// Ties `composer` to the host for `target`: shows what `opened` holds, follows the thread live, and answers every
    /// event the composer emits.
    pub fn new(
        host: Host,
        target: Target,
        opened: Opened,
        composer: Entity<Composer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (any, running, commands) = transcript_state(&opened.history);
        let prompts = prompts_of(&opened.history);
        let plan = plan_of(&opened.history);
        let drawer = drawer_of(&opened.rows, plan.as_ref(), running.is_some(), &opened.agents);
        let picks = ran(&opened.rows);
        let computer = opened.rows.last().and_then(|r| r.computer_name.clone());
        let no_agents = opened.agents.is_empty();
        composer.update(cx, |c, cx| {
            c.set_catalog(opened.agents, picks, commands, cx);
            c.set_turn(any, running.is_some(), cx);
            c.set_place(computer, None, cx);
            c.set_hold(no_agents.then_some(Hold::NoAgents), cx);
            c.set_recalls(prompts.clone(), cx);
            c.set_drawer(drawer, cx);
        });
        let key = target.thread_id.clone().unwrap_or_else(|| target.workspace_id.clone());
        composer.update(cx, |c, cx| c.set_thread_key(key, window, cx));
        let events = cx.subscribe_in(&composer, window, Self::on_event);
        let mut link = Link {
            host,
            target,
            composer: composer.downgrade(),
            rows: opened.rows,
            running_session: running,
            prompts,
            plan,
            _follow: None,
            _events: events,
        };
        link.follow(window, cx);
        link.read_skills(window, cx);
        link
    }

    fn read_skills(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        let Some(harness) = self.rows.last().map(|r| r.harness.clone()).or_else(|| composer.read(cx).resolved().agent.map(|a| a.harness))
        else {
            return;
        };
        let (host, ws) = (self.host.clone(), self.target.workspace_id.clone());
        let weak = composer.downgrade();
        cx.spawn_in(window, async move |_, cx| {
            let read = match request::<protocol::AgentsReadRequest>(json!({ "target": { "workspaceId": ws } })) {
                Ok(op) => call(&host, op).await.map(|r| serde_json::to_value(r.report).unwrap_or_default()),
                Err(e) => Err(e),
            };
            match read {
                Ok(report) => _ = weak.update(cx, |c, cx| c.set_skills(skills_of(&report, &harness), cx)),
                Err(failure) => eprintln!("desktop-gpui: agents.read: {}", failure.message),
            }
        })
        .detach();
    }

    /// Shows the drawer and the prompts as the rows and the transcript now say.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        let running = self.running_session.is_some();
        let (prompts, plan) = (self.prompts.clone(), self.plan.clone());
        let rows = &self.rows;
        composer.update(cx, |c, cx| {
            let drawer = drawer_of(rows, plan.as_ref(), running, &c.agents);
            c.set_drawer(drawer, cx);
            c.set_recalls(prompts, cx);
        });
    }

    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(thread) = self.target.thread_id.clone() else { return };
        let ws = self.target.workspace_id.clone();
        let (tx, mut rx) = mpsc::unbounded();
        // Follows again after a lost socket, a second, then up to five, apart, reading the transcript whole each time.
        let following = self.host.run(move |host| async move {
            let mut wait = Duration::from_secs(1);
            loop {
                let followed: Result<(), Failure> = async {
                    let mut live = host.open(&ws, &thread).await?;
                    wait = Duration::from_secs(1);
                    _ = tx.unbounded_send(Arrival::History(std::mem::take(&mut live.history)));
                    while let Some(next) = live.next().await {
                        let Ok(Live::Event(event)) = next else { continue };
                        // The rows say what the drawer shows: a question, a cap, the turn's state.
                        let ends = matches!(
                            kind(&event).0.as_str(),
                            "session.start"
                                | "session.end"
                                | "session.permission"
                                | "session.permission.closed"
                                | "session.capped"
                                | "session.uncapped"
                        );
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
                        return Ok::<(), Failure>(());
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
        let host_task = cx.spawn(async move |_, _| _ = following.await);
        self._follow = Some((host_task, receive));
    }

    fn arrive(&mut self, arrival: Arrival, _: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        match arrival {
            Arrival::History(events) => {
                let (any, running, _) = transcript_state(&events);
                self.running_session = running.clone();
                self.prompts = prompts_of(&events);
                self.plan = plan_of(&events);
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
                        self.plan = None;
                        composer.update(cx, |c, cx| c.set_turn(true, false, cx));
                    }
                    "session.plan" => self.plan = value.get("steps").cloned(),
                    _ => {}
                }
                if kind == "session.start" {
                    self.plan = None;
                }
                self.prompts.extend(prompts_of(std::slice::from_ref(&event)));
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
        self.refresh(cx);
    }

    fn started(&mut self, session: SessionView, window: &mut Window, cx: &mut Context<Self>) {
        if self.target.thread_id.is_some() {
            return;
        }
        self.target.thread_id = session.thread_id.clone().or_else(|| Some(session.id.clone()));
        crate::screens::terminal::carry_new_thread(&self.target.workspace_id, self.target.thread_id.as_deref().unwrap_or_default(), cx);
        self.rows = vec![session];
        if let (Some(composer), Some(thread)) = (self.composer.upgrade(), self.target.thread_id.clone()) {
            composer.update(cx, |c, cx| c.thread_started(thread, cx));
        }
        self.follow(window, cx);
    }

    fn on_event(&mut self, composer: &Entity<Composer>, event: &ComposerEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (host, target) = (self.host.clone(), self.target.clone());
        let weak = composer.downgrade();
        let session = session_id(&self.rows, self.running_session.as_deref());
        match event.clone() {
            ComposerEvent::Start(out) => {
                let params = start_params(&target, &out, !self.rows.is_empty());
                cx.spawn_in(window, async move |this, cx| {
                    let started = match request::<protocol::SessionsStartRequest>(params) {
                        Ok(op) => call(&host, op).await,
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
                    let answered = match request::<protocol::SessionsSteerRequest>(params) {
                        Ok(op) => call(&host, op)
                            .await
                            .map(|r| serde_json::to_value(r.outcome).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()),
                        Err(e) => Err(e),
                    };
                    let notice = steer_notice(&answered);
                    let taken = matches!(&answered, Ok(outcome) if outcome == "accepted");
                    _ = weak.update(cx, |c, cx| {
                        c.steered(taken, cx);
                        if let Some(notice) = notice {
                            c.raise(notice, cx);
                        }
                    });
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
                        Ok(op) => call(&host, op).await.map(|_| ()),
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
                        _ = call(&host, op).await;
                    }
                    if let Some(session) = session {
                        if let Ok(op) = request::<protocol::SessionsAccessRequest>(json!({ "sessionId": session, "permissionMode": mode }))
                        {
                            _ = call(&host, op).await;
                        }
                    }
                })
                .detach();
            }
            ComposerEvent::Notice(_) => {}
            ComposerEvent::Items => {
                let cwd = self.rows.last().and_then(|r| r.cwd.clone());
                let computer = self.rows.last().and_then(|r| r.computer_name.clone()).unwrap_or_else(|| "this computer".to_owned());
                cx.spawn_in(window, async move |_, cx| {
                    let (items, note) = match pull_requests(&host, &target.workspace_id, cwd).await {
                        Ok((items, note, no_cli)) => {
                            let note = no_cli
                                .map(|host| format!("No signed-in command line for {host} is on {computer}, so its pull requests and issues are not listed"))
                                .or(note);
                            (items, note)
                        }
                        Err(failure) => (Vec::new(), Some(failure.message)),
                    };
                    _ = weak.update(cx, |c, cx| c.set_host_items(items, note, cx));
                })
                .detach();
            }
            ComposerEvent::Files => {
                let cwd = self.rows.last().and_then(|r| r.cwd.clone());
                cx.spawn_in(window, async move |_, cx| {
                    let files = list_files(&host, &target.workspace_id, cwd).await.unwrap_or_else(|failure| {
                        eprintln!("desktop-gpui: fs.files: {}", failure.message);
                        Vec::new()
                    });
                    _ = weak.update(cx, |c, cx| c.set_folder_files(files, cx));
                })
                .detach();
            }
        }
    }
}

/// What a steer the turn did not take says (ChatComposer.tsx, `COMPOSER_WORDS.sendNowFailed`); a turn that ended
/// first is no failure, since the message then starts the next turn.
pub fn steer_notice(answered: &Result<String, Failure>) -> Option<String> {
    let said = match answered {
        Ok(outcome) if outcome == "not-found" => "wsp no longer knows this turn".to_owned(),
        Ok(outcome) if outcome == "unsupported" => "this agent takes no message mid-turn".to_owned(),
        Ok(_) => return None,
        Err(failure) => failure.message.clone(),
    };
    Some(format!("Could not send now: {said}"))
}

/// Every row of one thread, oldest first.
pub async fn rows_of(host: &Host, workspace_id: &str, thread: &str) -> Result<Vec<SessionView>, Failure> {
    let listed = call(host, request::<protocol::SessionsListRequest>(json!({ "workspaceId": workspace_id }))?).await?;
    Ok(thread_rows(listed.sessions, thread))
}

/// One thread's rows out of a workspace's, oldest first.
fn thread_rows(sessions: Vec<SessionView>, thread: &str) -> Vec<SessionView> {
    let mut rows: Vec<SessionView> = sessions.into_iter().filter(|r| r.thread_id.as_deref() == Some(thread)).collect();
    rows.sort_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.)));
    rows
}

/// One daemon op on the workspace's computer over a channel opened for it, and its reply.
async fn daemon(host: &Host, workspace_id: &str, frame: Value) -> Result<Value, Failure> {
    let op = frame.get("op").and_then(Value::as_str).unwrap_or_default().to_owned();
    let opened = call(host, request::<protocol::DaemonOpenRequest>(json!({ "workspaceId": workspace_id }))?).await?;
    let channel = serde_json::to_value(&opened).ok().and_then(|v| v.get("channel").and_then(Value::as_str).map(str::to_owned));
    let channel = channel.ok_or_else(|| Failure::new("daemon.open answered no channel"))?;
    let sent = call(host, request::<protocol::DaemonSendRequest>(json!({ "channel": channel, "frame": frame }))?).await;
    if let Ok(op) = request::<protocol::DaemonCloseRequest>(json!({ "channel": channel })) {
        _ = call(host, op).await;
    }
    let reply = sent?.reply;
    if reply.get("ok").and_then(Value::as_bool) == Some(false) {
        return Err(Failure::new(reply.get("error").and_then(Value::as_str).map_or(format!("{op} failed"), str::to_owned)));
    }
    Ok(reply)
}

/// The open pull requests and issues of the folder's repository (the daemon's `git.prList`), the note it gave, and
/// the git host it has no signed-in command line for.
pub async fn pull_requests(
    host: &Host,
    workspace_id: &str,
    cwd: Option<String>,
) -> Result<(Vec<HostItem>, Option<String>, Option<String>), Failure> {
    let reply = daemon(host, workspace_id, json!({ "op": "git.prList", "cwd": cwd.unwrap_or_default() })).await?;
    let items = reply.get("items").and_then(|i| serde_json::from_value(i.clone()).ok()).unwrap_or_default();
    let text = |key: &str| reply.get(key).and_then(Value::as_str).map(str::to_owned);
    Ok((items, text("note"), text("noCliFor")))
}

/// The files of the thread's folder, as the mention menu lists them: the daemon's `fs.files` over a channel.
pub async fn list_files(host: &Host, workspace_id: &str, cwd: Option<String>) -> Result<Vec<String>, Failure> {
    let reply = daemon(host, workspace_id, json!({ "op": "fs.files", "cwd": cwd.unwrap_or_default() })).await?;
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
    if out.fast {
        params["fast"] = Value::from(true);
    }
    if let Some((id, copy)) = &out.resume {
        params["resume"] = if *copy { json!({ "id": id, "copy": true }) } else { json!({ "id": id }) };
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
        let file = File { name: "a.txt".into(), media_type: "text/plain".into(), bytes: std::sync::Arc::new(b"hi".to_vec()) };
        let out = Outgoing {
            prompt: "go".into(),
            files: vec![file],
            picks,
            fast: true,
            resume: Some(("conv-1".into(), false)),
            from_queue: false,
        };
        let ran = start_params(&target, &out, true);
        assert_eq!(ran["thread"], "t");
        assert_eq!(ran["model"], "opus");
        assert_eq!(ran["contextWindow"], "1m");
        assert!(ran.get("harness").is_none() && ran.get("permissionMode").is_none());
        assert_eq!(ran["attachments"][0]["bytes"], "aGk=");
        assert_eq!((&ran["fast"], &ran["resume"]["id"]), (&Value::from(true), &Value::from("conv-1")));
        let fresh = start_params(&target, &out, false);
        assert_eq!(fresh["harness"], "claude");
        assert_eq!(fresh["permissionMode"], "plan");
        request::<protocol::SessionsStartRequest>(fresh).expect("the params read as the recorded request");
    }

    #[test]
    fn a_steer_the_turn_did_not_take_says_why() {
        let said = |r: Result<&str, &str>| steer_notice(&r.map(str::to_owned).map_err(Failure::new));
        assert_eq!(said(Ok("accepted")), None);
        assert_eq!(said(Ok("not-running")), None, "the message starts the next turn instead");
        assert_eq!(said(Ok("not-found")).as_deref(), Some("Could not send now: wsp no longer knows this turn"));
        assert_eq!(said(Ok("unsupported")).as_deref(), Some("Could not send now: this agent takes no message mid-turn"));
        assert_eq!(said(Err("the socket closed")).as_deref(), Some("Could not send now: the socket closed"));
    }
}
