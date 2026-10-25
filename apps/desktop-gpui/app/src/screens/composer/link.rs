// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's road to the host, through `wsp-host-client` alone: it reads the agents and their choices
//! (`harnesses.list`), the workspace's rows (`sessions.list`), the agent's skills (`agents.read`) and the person's
//! `midTurn` and `sendWith` (`preferences.get`), and answers the composer's events with `sessions.start`, `sessions.steer`,
//! `sessions.interrupt`, `sessions.answer`, `preferences.set` with `sessions.access`, and the daemon's `fs.files` and
//! `git.prList` for the menus. What every thread of a workspace shares is kept in the composer for the process, so a
//! thread switch asks the host nothing of it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use base64::Engine as _;
use futures::channel::mpsc;
use futures::{FutureExt as _, StreamExt};
use gpui_kit::{App, AsyncWindowContext, Context, Entity, Subscription, Task, WeakEntity, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{self, SessionEvent, SessionView, SessionsAnswerRequest};
use wsp_host_client::{Event, Failure, Live, Op};

use super::dock::{self, Answer};
use super::model::{Agent, Command, Drawer, File, Hold, HostItem, Limit, Picks, Plan, Skill, Slot};
use super::{Composer, ComposerEvent, Fork, Outgoing};
use crate::screens::thread_view::Carried;
use crate::screens::{navigate, Host, Route};

/// The thread the composer writes into: its workspace, and its id once it has one.
#[derive(Clone, Debug)]
pub struct Target {
    pub workspace_id: String,
    pub thread_id: Option<String>,
}

impl Target {
    /// The key the composer keeps the thread's draft and queue under: its id, or its workspace's before it has one.
    pub fn key(&self) -> String {
        self.thread_id.clone().unwrap_or_else(|| self.workspace_id.clone())
    }
}

/// What the host said about the thread when it was opened, and the workspace's rows where they were read for it.
pub struct Opened {
    pub agents: Vec<Agent>,
    pub rows: Vec<SessionView>,
    pub history: Vec<SessionEvent>,
    pub sessions: Option<Vec<SessionView>>,
}

/// What the host said that every thread of a workspace shares, kept in the composer for the process: the agents, each
/// agent's skills, and the workspace's rows as last listed.
#[derive(Default)]
pub struct Facts {
    pub(super) agents: HashMap<String, Vec<Agent>>,
    pub(super) skills: HashMap<(String, String), Vec<Skill>>,
    /// The skills asked for and not yet answered, with the keys of the threads waiting on them.
    pub(super) asking: HashMap<(String, String), Vec<String>>,
    pub(super) sessions: HashMap<String, Vec<SessionView>>,
}

/// What `Facts` already holds for one workspace; `read_facts` asks the host only for the rest.
#[derive(Clone, Default)]
pub struct Known {
    pub agents: Option<Vec<Agent>>,
    pub sessions: Option<Vec<SessionView>>,
}

impl Facts {
    pub fn known(&self, workspace: &str) -> Known {
        Known { agents: self.agents.get(workspace).cloned(), sessions: self.sessions.get(workspace).cloned() }
    }
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
    plan: Option<Plan>,
    /// The thread followed: the host task, stopped when this is dropped, and its arrivals read on the window's thread.
    _follow: Option<(Task<()>, Task<()>)>,
    /// The rows read again after an event; a newer read replaces it.
    _rereading: Option<Task<()>>,
    /// A send waiting on its agent: the watch for the host's `session.starting` about it.
    starting: Option<Task<()>>,
    /// The request id of the newest `session.start` the transcript showed.
    begun: Option<String>,
    /// A send the host started whose `session.start` the transcript has not shown yet: its turn runs meanwhile.
    awaiting: Option<String>,
    /// The watch for the starting line of a send that opens a thread, which goes on to that thread's own link.
    unheard: Option<StartWatch>,
    _events: Subscription,
}

/// The host's `session.starting` for one send, kept as it arrives, so a thread's link that stands only after the host
/// said it still says its line.
pub struct StartWatch {
    said: mpsc::UnboundedReceiver<(String, bool)>,
    watched: futures::future::BoxFuture<'static, Result<(), Failure>>,
}

/// One op on the host's runtime, its reply back on the window's thread.
pub(super) async fn call<O: Op + Send + 'static>(host: &Host, op: O) -> Result<O::Reply, Failure>
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
pub(crate) fn request_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    format!("gpui-{now}-{}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// What a transcript says to the composer: whether it holds a message, the session of a turn still running and of the
/// newest turn, the commands its agent announced last, the sent prompts for Up and Down, and the running turn's steps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Seen {
    any: bool,
    running: Option<String>,
    latest: Option<String>,
    /// The request id of the newest turn's start.
    begun: Option<String>,
    commands: Vec<Command>,
    prompts: Vec<(String, String)>,
    plan: Option<Plan>,
}

impl Seen {
    /// One pass over the events, each read as JSON once.
    pub fn of(events: &[SessionEvent]) -> Seen {
        let mut seen = Seen::default();
        for event in events {
            seen.take(&kind(event));
        }
        seen
    }

    /// `of` the events a view holds, with what its fold says of them (ChatComposer.tsx compacts on a running turn or
    /// any message): a page from the middle of a long thread holds no turn's start, though the turn it meets may run
    /// and the thread holds messages above it.
    pub fn of_view(events: &[SessionEvent], working: bool, messaged: bool) -> Seen {
        let mut seen = Seen::of(events);
        seen.any |= messaged;
        if working && seen.running.is_none() {
            seen.running = events.iter().rev().find_map(|e| kind(e).1.get("sessionId").and_then(Value::as_str).map(str::to_owned));
        }
        seen
    }

    /// ChatComposer.tsx's compact rule: a turn runs, or the thread holds a message.
    pub fn compact(&self) -> bool {
        self.any || self.running.is_some()
    }

    pub fn running(&self) -> Option<&str> {
        self.running.as_deref()
    }

    fn take(&mut self, (kind, value): &(String, Value)) {
        match kind.as_str() {
            "session.start" => {
                self.any = true;
                let session = value.get("sessionId").and_then(Value::as_str).unwrap_or_default().to_owned();
                self.running = Some(session.clone());
                self.latest = Some(session);
                self.begun = value.get("requestId").and_then(Value::as_str).map(str::to_owned);
                self.plan = None;
                if let Some(slash) = value.pointer("/harness/slashCommands").and_then(Value::as_array) {
                    self.commands = commands_of(slash);
                }
            }
            "session.end" => {
                self.running = None;
                self.plan = None;
            }
            "session.plan" => self.plan = plan_after(self.plan.take(), value),
            _ => {}
        }
        if let Some(prompt) = prompt_of(kind, value) {
            self.prompts.push(prompt);
        }
    }
}

fn commands_of(slash: &[Value]) -> Vec<Command> {
    slash
        .iter()
        .filter_map(Value::as_str)
        .map(|name| {
            let name = name.trim_start_matches('/');
            let source = name.split_once(':').map(|(source, _)| source.to_owned());
            Command { name: name.to_owned(), description: None, source }
        })
        .collect()
}

/// The events after which the thread's rows say something new for the drawer: a cap, a limit, the turn's state.
fn changes_rows(kind: &str) -> bool {
    matches!(
        kind,
        "session.start" | "session.end" | "session.permission" | "session.permission.closed" | "session.capped" | "session.uncapped"
    )
}

fn kind(event: &SessionEvent) -> (String, Value) {
    let value = serde_json::to_value(event).unwrap_or_default();
    (value.get("type").and_then(Value::as_str).unwrap_or_default().to_owned(), value)
}

/// A turn's or a steer's sent prompt by id.
fn prompt_of(kind: &str, value: &Value) -> Option<(String, String)> {
    if !matches!(kind, "session.start" | "session.steer") {
        return None;
    }
    let prompt = value.get("prompt")?.as_str()?.to_owned();
    let id = value.get("pos").map(Value::to_string).or_else(|| value.get("turnId").map(Value::to_string)).unwrap_or_default();
    Some((id, prompt))
}

/// Whether the rows read before are behind the transcript: it names a newer turn than they hold, or that turn's row
/// says it runs where the transcript says it ended, or the other way round.
fn behind(rows: &[SessionView], seen: &Seen) -> bool {
    let Some(latest) = seen.latest.as_deref() else { return false };
    let row = rows.iter().rev().find(|r| r.id == latest || r.claude_session_id.as_deref() == Some(latest));
    match row {
        None => true,
        Some(row) => {
            let runs = serde_json::to_value(row.status).ok().and_then(|v| v.as_str().map(|s| s == "running")).unwrap_or(false);
            runs != seen.running.is_some()
        }
    }
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

/// `sessions.mark` for the usage limit's acts: the thread goes on with its turn at the reset, or no longer.
pub fn mark_params(thread: &str, resume: bool) -> Value {
    json!({ "threadIds": [thread], "marks": { "resumeAtReset": resume } })
}

/// The step list after a `session.plan`: one that wrote no steps leaves the list as it stood.
fn plan_after(earlier: Option<Plan>, value: &Value) -> Option<Plan> {
    match value.get("steps").and_then(Value::as_array) {
        Some(steps) => Some(Plan::next(earlier.as_ref(), steps, value.get("at").and_then(Value::as_f64))),
        None => earlier,
    }
}

/// What waits around the composer, off the thread's newest row and the running turn's step list (ChatComposer.tsx's
/// `drawer`, `capRunningLine`, `capBarWords`, `capRaiseAct` and `LIMIT_WORDS`).
pub fn drawer_of(rows: &[SessionView], plan: Option<&Plan>, running: bool, agents: &[Agent]) -> Drawer {
    let Some(row) = rows.last() else { return Drawer::default() };
    let row_value = serde_json::to_value(row).unwrap_or_default();
    let agent = agents.iter().find(|a| a.harness == row.harness).map_or(row.harness.clone(), |a| a.label.clone());
    let failed = row_value.get("status").and_then(Value::as_str) == Some("failed");
    let limit = row_value.get("limit").filter(|_| failed).map(|limit| {
        let armed = row.resume_at.is_some();
        let resets_at = limit.get("resetsAt").and_then(Value::as_f64);
        let unknown = format!("{agent} stopped this turn and did not say when its plan resets.");
        let line = match resets_at {
            Some(at) => format!("{agent} resumes at {}", clock_word(at, std::time::SystemTime::now())),
            None => unknown.clone(),
        };
        let note = match (armed, resets_at) {
            (true, _) => format!("{agent} goes on with this turn when its plan resets."),
            (false, Some(_)) => format!("{agent} stopped this turn until its plan resets."),
            (false, None) => unknown,
        };
        let title = if armed { "Resumes at the reset" } else { "Usage limit reached" }.to_owned();
        Limit { title, line, note, resets: resets_at.is_some(), armed }
    });
    let slot = row_value.get("capped").map(|c| {
        let place = c.get("place").and_then(Value::as_str).unwrap_or_default();
        let (on, of) = (c.get("running").and_then(Value::as_i64).unwrap_or(0), c.get("atOnce").and_then(Value::as_i64).unwrap_or(0));
        let why = c.get("why").and_then(Value::as_str);
        let line = match why {
            Some("minute") => format!("{place}'s starts this minute are used: {on} of {of}"),
            Some("slot") => format!("{place} is running {on} of {}", plural(of, "machine")),
            _ => format!("{place} is running {on} of {}", plural(of, "thread")),
        };
        let raise = match why {
            None => Some("Raise threads at once".to_owned()),
            Some("slot") => Some("Raise machines at once".to_owned()),
            Some(_) => None,
        };
        Slot {
            line,
            title: format!("Waiting for a thread slot on {place}"),
            note: format!(
                "{on} of {} {} running. It starts on its own when one ends.",
                plural(of, "thread"),
                if on == 1 { "is" } else { "are" }
            ),
            raise,
            place_id: c.get("placeId").and_then(Value::as_str).unwrap_or_default().to_owned(),
        }
    });
    let tasks = plan.filter(|_| running).and_then(Plan::tasks);
    Drawer { limit, slot, tasks }
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
    /// What the composer needs for `target` less the transcript, for a Link the thread view feeds its own events to
    /// (`fed`): the agents and the workspace's rows, each read only where `known` does not hold it yet.
    pub async fn read_facts(host: &Host, target: &Target, known: Known) -> Result<Opened, Failure> {
        let agents = match known.agents {
            Some(agents) => agents,
            None => agents_of(host, &target.workspace_id).await?,
        };
        // Rows kept from before the thread was made hold none of it: read them again.
        let holds =
            |rows: &Vec<SessionView>| target.thread_id.as_deref().is_none_or(|t| rows.iter().any(|r| r.thread_id.as_deref() == Some(t)));
        let sessions = match known.sessions.filter(holds) {
            Some(sessions) => sessions,
            None => sessions_of(host, &target.workspace_id).await?,
        };
        let rows = target.thread_id.as_deref().map_or_else(Vec::new, |thread| thread_rows(sessions.clone(), thread));
        Ok(Opened { agents, rows, history: Vec::new(), sessions: Some(sessions) })
    }

    /// Reads what the composer needs for `target`, from the agents to the transcript.
    pub async fn read(host: &Host, target: &Target) -> Result<Opened, Failure> {
        let agents = agents_of(host, &target.workspace_id).await?;
        let (rows, history) = match &target.thread_id {
            Some(thread) => {
                let rows = rows_of(host, &target.workspace_id, thread).await?;
                let (ws, thread) = (target.workspace_id.clone(), thread.clone());
                let history = host.run(move |client| async move { client.open(&ws, &thread).await.map(|t| t.history) }).await?;
                (rows, history)
            }
            None => (Vec::new(), Vec::new()),
        };
        Ok(Opened { agents, rows, history, sessions: None })
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
        Self::build(host, target, opened, composer, true, window, cx)
    }

    fn build(
        host: Host,
        target: Target,
        opened: Opened,
        composer: Entity<Composer>,
        follows: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let seen = Seen::of(&opened.history);
        let drawer = drawer_of(&opened.rows, seen.plan.as_ref(), seen.running.is_some(), &opened.agents);
        let picks = ran(&opened.rows);
        let computer = crate::screens::terminal::workspaces_sidebar(cx).and_then(|s| s.read(cx).computer_name(&target.workspace_id));
        let no_agents = opened.agents.is_empty();
        let ws = target.workspace_id.clone();
        composer.update(cx, |c, cx| {
            c.set_thread_key(target.key(), window, cx);
            // A workspace that listed no agents may list them once one is set up, so none is not kept.
            if !opened.agents.is_empty() {
                c.facts.agents.insert(ws.clone(), opened.agents.clone());
            }
            if let Some(sessions) = opened.sessions {
                c.facts.sessions.insert(ws.clone(), sessions);
            }
            c.set_catalog(opened.agents, picks, seen.commands.clone(), cx);
            c.set_turn(seen.any, seen.running.is_some(), cx);
            c.set_place(computer, None, cx);
            c.set_workspace(ws.clone(), cx);
            c.set_hold(no_agents.then_some(Hold::NoAgents), cx);
            c.set_recalls(seen.prompts.clone(), cx);
            c.set_drawer(drawer, cx);
        });
        let events = cx.subscribe_in(&composer, window, Self::on_event);
        let mut link = Link {
            host,
            target,
            composer: composer.downgrade(),
            rows: opened.rows,
            running_session: seen.running,
            prompts: seen.prompts,
            plan: seen.plan,
            _follow: None,
            _rereading: None,
            starting: None,
            begun: None,
            awaiting: None,
            unheard: None,
            _events: events,
        };
        if follows {
            link.follow(window, cx);
        }
        link.read_skills(window, cx);
        link.watch_preferences(&composer, cx);
        link
    }

    /// `new` for a thread whose events the thread view already follows: it reads nothing of the transcript itself and
    /// takes the view's through `take_history` and `take_event`, so a thread is read and followed once.
    pub fn fed(
        host: Host,
        target: Target,
        opened: Opened,
        composer: Entity<Composer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(host, target, opened, composer, false, window, cx)
    }

    /// The key the composer keeps this link's thread under.
    pub fn key(&self) -> String {
        self.target.key()
    }

    /// The thread's title as its newest row has it.
    pub fn title(&self) -> Option<String> {
        let named = self.rows.iter().rev().find_map(|r| r.harness_title.as_deref());
        crate::screens::app_shell::title::thread_title(named, self.rows.first().and_then(|r| r.prompt.as_deref()))
    }

    /// What the thread view's transcript, read whole, says to the composer (`Seen::of`); rows the transcript has moved
    /// past since they were read are read again.
    pub fn take_history(&mut self, seen: Seen, _: &mut Window, cx: &mut Context<Self>) {
        let stale = behind(&self.rows, &seen);
        self.seen(seen, cx);
        self.refresh(cx);
        if stale && self.target.thread_id.is_some() {
            self.reread(cx);
        }
    }

    /// One live event the thread view took; one that changes the drawer reads the thread's rows again.
    pub fn take_event(&mut self, event: SessionEvent, window: &mut Window, cx: &mut Context<Self>) {
        // A streamed reply or call says nothing the composer shows; only these are read.
        use SessionEvent as E;
        let tells = matches!(
            event,
            E::SessionStart { .. }
                | E::SessionEnd { .. }
                | E::SessionPlan { .. }
                | E::SessionSteer { .. }
                | E::SessionPermission { .. }
                | E::SessionPermissionClosed { .. }
                | E::SessionCapped { .. }
                | E::SessionUncapped { .. }
        );
        if !tells {
            return;
        }
        let read = kind(&event);
        let ends = changes_rows(&read.0);
        self.arrive(Arrival::Event(Box::new(event)), Some(read), window, cx);
        if ends && self.target.thread_id.is_some() {
            self.reread(cx);
        }
    }

    /// Reads the workspace's rows again; a newer read drops one still out, so an older answer never lands last.
    fn reread(&mut self, cx: &mut Context<Self>) {
        let (host, ws) = (self.host.clone(), self.target.workspace_id.clone());
        self._rereading = Some(cx.spawn(async move |this, cx| {
            if let Ok(sessions) = sessions_of(&host, &ws).await {
                _ = this.update(cx, |this, cx| this.listed(sessions, cx));
            }
        }));
    }

    /// The workspace's rows as the host listed them now: this thread's for the drawer, all of them kept for the next.
    fn listed(&mut self, sessions: Vec<SessionView>, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        let ws = self.target.workspace_id.clone();
        let rows = self.target.thread_id.as_deref().map_or_else(Vec::new, |thread| thread_rows(sessions.clone(), thread));
        composer.update(cx, |c, _| _ = c.facts.sessions.insert(ws, sessions));
        if composer.read(cx).holds(&self.key()) {
            self.take_rows(rows, cx);
            self.refresh(cx);
        }
    }

    /// The thread's agent's skills: kept per workspace and agent, so `agents.read`, which runs the agents' own command
    /// lines on the host, is asked once a process for each.
    fn read_skills(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        let Some(harness) = self.rows.last().map(|r| r.harness.clone()).or_else(|| composer.read(cx).resolved().agent.map(|a| a.harness))
        else {
            return;
        };
        let ws = self.target.workspace_id.clone();
        let place = (ws.clone(), harness.clone());
        let known = composer.read(cx).facts.skills.get(&place).cloned();
        if let Some(skills) = known {
            composer.update(cx, |c, cx| c.set_skills(skills, cx));
            return;
        }
        let key = self.key();
        let asked = composer.update(cx, |c, _| {
            let waiting = c.facts.asking.entry(place.clone()).or_default();
            waiting.push(key);
            waiting.len() > 1
        });
        if asked {
            return;
        }
        let (host, weak) = (self.host.clone(), composer.downgrade());
        cx.spawn_in(window, async move |_, cx| {
            let read = match request::<protocol::AgentsReadRequest>(json!({ "target": { "workspaceId": ws } })) {
                Ok(op) => call(&host, op).await.map(|r| serde_json::to_value(r.report).unwrap_or_default()),
                Err(e) => Err(e),
            };
            match read {
                Ok(report) => {
                    _ = weak.update(cx, |c, cx| {
                        let skills = skills_of(&report, &harness);
                        let waiting = c.facts.asking.remove(&place).unwrap_or_default();
                        c.facts.skills.insert(place, skills.clone());
                        if waiting.iter().any(|k| c.holds(k)) {
                            c.set_skills(skills, cx);
                        }
                    })
                }
                Err(failure) => {
                    eprintln!("desktop-gpui: agents.read: {}", failure.message);
                    _ = weak.update(cx, |c, _| c.facts.asking.remove(&place));
                }
            }
        })
        .detach();
    }

    /// The person's choice of what a message sent mid-turn does and of the key that sends (`preferences.get`'s
    /// `midTurn` and `sendWith`), read once per composer and followed as the host pushes a change.
    fn watch_preferences(&mut self, composer: &Entity<Composer>, cx: &mut Context<Self>) {
        if composer.read(cx).watching.is_some() {
            return;
        }
        let (tx, mut rx) = mpsc::unbounded::<(bool, bool)>();
        // Read again and followed again a second after the socket closes, as the header's watch does.
        let watched = self.host.run(move |host| async move {
            loop {
                let followed: Result<(), Failure> = async {
                    let mut events = host.events().await?;
                    let read = host.call(request::<protocol::PreferencesGetRequest>(json!({}))?).await?;
                    tx.unbounded_send(followed(&read.preferences)).map_err(|_| Failure::new("gone"))?;
                    while let Some(event) = events.next().await {
                        if let Ok(Event::Host(event)) = event {
                            if let protocol::EventUnion::PreferencesChanged { preferences, .. } = *event {
                                tx.unbounded_send(followed(&preferences)).map_err(|_| Failure::new("gone"))?;
                            }
                        }
                    }
                    Ok(())
                }
                .await;
                if tx.is_closed() {
                    return Ok::<(), Failure>(());
                }
                if let Err(failure) = followed {
                    eprintln!("desktop-gpui: preferences: {}", failure.message);
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
        let weak = composer.downgrade();
        let task = cx.spawn(async move |_, cx| {
            let receive = async {
                while let Some((steer, mod_enter)) = rx.next().await {
                    let set = weak.update(cx, |c, cx| {
                        c.set_steer_preferred(steer, cx);
                        c.set_send_with_mod(mod_enter, cx);
                    });
                    if set.is_err() {
                        break;
                    }
                }
            };
            futures::join!(receive, async { _ = watched.await });
        });
        composer.update(cx, |c, _| c.watching = Some(task));
    }

    /// Listens for the host saying the send `request` waits on its agent (`startingSaid` in useChatThread.ts), until
    /// the turn starts or the send is refused.
    fn watch_start(&self, request: String) -> StartWatch {
        let (tx, said) = mpsc::unbounded::<(String, bool)>();
        let wanted = request;
        let watched = self.host.run(move |host| async move {
            let mut events = host.events().await?;
            while let Some(event) = events.next().await {
                let Ok(Event::Host(event)) = event else { continue };
                let said = serde_json::to_value(&*event).unwrap_or_default();
                if said.get("type").and_then(Value::as_str) == Some("session.starting")
                    && said.get("requestId").and_then(Value::as_str) == Some(wanted.as_str())
                {
                    let harness = said.get("harness").and_then(Value::as_str).unwrap_or_default().to_owned();
                    _ = tx.unbounded_send((harness, said.get("installs").and_then(Value::as_bool) == Some(true)));
                    break;
                }
            }
            Ok::<(), Failure>(())
        });
        StartWatch { said, watched: watched.boxed() }
    }

    /// Says the line `watch` hears under the send while the composer writes into the thread `key` names.
    fn hear_start(&mut self, watch: StartWatch, key: String, composer: &Entity<Composer>, cx: &mut Context<Self>) {
        let StartWatch { mut said, watched } = watch;
        let weak = composer.downgrade();
        let task = cx.spawn(async move |_, cx| {
            let heard = async {
                if let Some((harness, installs)) = said.next().await {
                    _ = weak.update(cx, |c, cx| {
                        let agent = c.agents.iter().find(|a| a.harness == harness).map_or(harness.clone(), |a| a.label.clone());
                        if c.holds(&key) {
                            cx.emit(ComposerEvent::Starting(Some(crate::transcript::agent_starting_line(&agent, installs))));
                        }
                    });
                }
            };
            futures::join!(heard, async { _ = watched.await });
        });
        self.starting = Some(task);
    }

    /// The send's turn started, or the send was refused: its starting line goes.
    fn started_agent(&mut self, cx: &mut Context<Self>) {
        self.unheard = None;
        if self.starting.take().is_none() {
            return;
        }
        let Some(composer) = self.composer.upgrade() else { return };
        let key = self.key();
        composer.update(cx, |c, cx| {
            if c.holds(&key) {
                cx.emit(ComposerEvent::Starting(None));
            }
        });
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
                        // The rows say what the drawer shows: a cap, a limit, the turn's state.
                        let ends = changes_rows(&kind(&event).0);
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
                if this.update_in(cx, |this, window, cx| this.arrive(arrival, None, window, cx)).is_err() {
                    break;
                }
            }
        });
        let host_task = cx.spawn(async move |_, _| _ = following.await);
        self._follow = Some((host_task, receive));
    }

    fn seen(&mut self, seen: Seen, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        let Seen { any, running, commands, prompts, plan, begun, .. } = seen;
        if begun.is_some() && begun == self.awaiting {
            self.awaiting = None;
            self.started_agent(cx);
        }
        self.begun = begun;
        // A transcript read before the host wrote the start of a turn it already answered still runs that turn.
        let runs = running.is_some() || self.awaiting.is_some();
        self.running_session = running.clone();
        self.prompts = prompts;
        self.plan = plan;
        composer.update(cx, |c, cx| {
            if c.hold() == Some(&Hold::HostDown) {
                c.set_hold(None, cx);
            }
            let (agents, ran) = (c.agents.clone(), c.ran.clone());
            c.set_catalog(agents, ran, commands, cx);
            c.set_turn(any || runs, runs, cx)
        });
    }

    fn take_rows(&mut self, rows: Vec<SessionView>, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        self.rows = rows;
        let picks = ran(&self.rows);
        composer.update(cx, |c, cx| {
            let (agents, commands) = (c.agents.clone(), c.commands.clone());
            c.set_catalog(agents, picks, commands, cx);
        });
    }

    /// An arrival; an event the caller already read as JSON comes with it.
    fn arrive(&mut self, arrival: Arrival, read: Option<(String, Value)>, _: &mut Window, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        match arrival {
            Arrival::History(events) => self.seen(Seen::of(&events), cx),
            Arrival::Event(event) => {
                let (kind, value) = read.unwrap_or_else(|| kind(&event));
                match kind.as_str() {
                    "session.start" => {
                        self.running_session = value.get("sessionId").and_then(Value::as_str).map(str::to_owned);
                        self.begun = value.get("requestId").and_then(Value::as_str).map(str::to_owned);
                        if self.begun.is_some() && self.begun == self.awaiting {
                            self.awaiting = None;
                        }
                        self.plan = None;
                        self.started_agent(cx);
                        let commands = value.pointer("/harness/slashCommands").and_then(Value::as_array).map(|s| commands_of(s));
                        composer.update(cx, |c, cx| {
                            if let Some(commands) = commands.filter(|c| !c.is_empty()) {
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
                    "session.plan" => self.plan = plan_after(self.plan.take(), &value),
                    _ => {}
                }
                self.prompts.extend(prompt_of(&kind, &value));
            }
            Arrival::Rows(rows) => self.take_rows(rows, cx),
            Arrival::Failed(failure) => {
                eprintln!("desktop-gpui: composer: {failure}");
                composer.update(cx, |c, cx| c.set_hold(Some(Hold::HostDown), cx));
            }
        }
        self.refresh(cx);
    }

    /// The thread the page's send made opens in place, as ChatView's fresh thread becomes that thread: the thread screen
    /// takes up the send's bubble, its starting line and its turn.
    fn started(
        &mut self,
        session: SessionView,
        request: String,
        (text, files): (String, Vec<File>),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let (workspace_id, thread_id) = (self.target.workspace_id.clone(), self.target.thread_id.clone().unwrap_or_default());
        let watch = self.unheard.take();
        crate::screens::thread_view::carry(Carried { thread: thread_id.clone(), text, files, request, watch }, cx);
        navigate(Route::Thread { workspace_id, thread_id }, cx);
    }

    /// A send the New thread page made on this thread before the screen showed it: its turn runs until the transcript
    /// shows it begun, and its bubble and starting line stand until then, as for a send made here.
    pub fn carry(&mut self, request: String, watch: Option<StartWatch>, cx: &mut Context<Self>) {
        let Some(composer) = self.composer.upgrade() else { return };
        let key = self.key();
        self.awaiting = Some(request);
        if let Some(watch) = watch {
            self.hear_start(watch, key, &composer, cx);
        }
    }

    /// Answers what the composer asks of the host. Every answer that lands checks that the composer still writes into
    /// the thread it was asked for, so a link's late answer never acts on the next thread's box.
    fn on_event(&mut self, composer: &Entity<Composer>, event: &ComposerEvent, window: &mut Window, cx: &mut Context<Self>) {
        let (host, target) = (self.host.clone(), self.target.clone());
        let weak = composer.downgrade();
        let key = composer.read(cx).thread_key().map_or_else(|| self.key(), str::to_owned);
        let session = session_id(&self.rows, self.running_session.as_deref());
        match event.clone() {
            ComposerEvent::Start(out) => {
                let params = match &out.fork {
                    Some(fork) => fork_params(fork, &out),
                    None => start_params(&target, &out, !self.rows.is_empty()),
                };
                let asked = params.get("requestId").and_then(Value::as_str).unwrap_or_default().to_owned();
                if out.fork.is_none() {
                    let watch = self.watch_start(asked.clone());
                    // A send that opens a thread says its line in that thread, which the host may answer first.
                    match target.thread_id {
                        Some(_) => self.hear_start(watch, key.clone(), composer, cx),
                        None => self.unheard = Some(watch),
                    }
                }
                cx.spawn_in(window, async move |this, cx| {
                    let started = match request::<protocol::SessionsStartRequest>(params) {
                        Ok(op) => call(&host, op).await,
                        Err(e) => Err(e),
                    };
                    match started {
                        // A fork opens a thread of its own, where the window goes once the host took it.
                        // A reply that lands after the person left the draft's thread moves nothing.
                        Ok(started) if out.fork.is_some() => {
                            let session = started.session;
                            let thread = session.thread_id.clone().unwrap_or_else(|| session.id.clone());
                            let here = session.workspace_id == target.workspace_id;
                            let route = Route::Thread { workspace_id: session.workspace_id.clone(), thread_id: thread.clone() };
                            _ = weak.update(cx, |c, cx| {
                                if c.holds(&key) {
                                    c.sent(cx);
                                    c.forked(cx);
                                    // A fork in the page's own workspace is the page's thread from here, so its drawer and
                                    // shells go with it, as ChatView's carry does; one onto a new branch opens elsewhere.
                                    if here {
                                        crate::screens::terminal::carry_new_thread(&target.workspace_id, &thread, cx);
                                    }
                                    navigate(route, cx);
                                } else {
                                    c.sent_on(&key, cx);
                                }
                            });
                        }
                        // A send that opened the thread names it; from here the link follows it as any other.
                        Ok(started) => {
                            let held = weak
                                .update(cx, |c, cx| {
                                    let held = c.holds(&key);
                                    c.sent_on(&key, cx);
                                    held
                                })
                                .unwrap_or(false);
                            if held {
                                let sent = (out.prompt.clone(), out.files.clone());
                                _ = this.update_in(cx, |link, window, cx| {
                                    // The turn the host started runs until the transcript says it ended; one whose start
                                    // the transcript already showed is the transcript's to say.
                                    if link.begun.as_deref() != Some(asked.as_str()) {
                                        link.awaiting = Some(asked.clone());
                                        _ = weak.update(cx, |c, cx| c.set_turn(true, true, cx));
                                    }
                                    link.started(started.session, asked, sent, window, cx)
                                });
                            }
                        }
                        Err(failure) => {
                            eprintln!("desktop-gpui: sessions.start: {}", failure.message);
                            _ = this.update(cx, |link, cx| link.started_agent(cx));
                            _ = weak.update_in(cx, |c, window, cx| c.refused_on(&key, out, window, cx));
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
                        let here = c.holds(&key);
                        c.steered_on(&key, taken, cx);
                        if let Some(notice) = notice.filter(|_| here) {
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
                        _ = weak.update(cx, |c, cx| {
                            if c.holds(&key) {
                                c.stopped(cx)
                            }
                        });
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
            ComposerEvent::Mark(resume) => {
                let Some(thread) = target.thread_id.clone() else { return };
                cx.spawn_in(window, async move |this, cx| {
                    let marked = match request::<protocol::SessionsMarkRequest>(mark_params(&thread, resume)) {
                        Ok(op) => call(&host, op).await.map(|_| ()),
                        Err(e) => Err(e),
                    };
                    match marked {
                        Ok(()) => _ = this.update(cx, |this, cx| this.reread(cx)),
                        Err(failure) => eprintln!("desktop-gpui: sessions.mark: {}", failure.message),
                    }
                })
                .detach();
            }
            ComposerEvent::Notice(_) | ComposerEvent::Answer(_) | ComposerEvent::Starting(_) => {}
            ComposerEvent::Aside(question) => match session {
                Some(session) => {
                    crate::screens::aside::ask(&host, &target.workspace_id, &session, question, window, cx);
                    composer.update(cx, |c, cx| c.aside_asked(window, cx));
                }
                None => crate::ui::notify(cx, crate::ui::Notice::note(crate::screens::aside::words::NO_SESSION)),
            },
            ComposerEvent::Items => {
                let cwd = self.rows.last().and_then(|r| r.cwd.clone());
                let computer = composer.read(cx).computer.clone().unwrap_or_else(|| "this computer".to_owned());
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
                    _ = weak.update(cx, |c, cx| {
                        if c.holds(&key) {
                            c.set_host_items(items, note, cx)
                        }
                    });
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
                    _ = weak.update(cx, |c, cx| {
                        if c.holds(&key) {
                            c.set_folder_files(files, cx)
                        }
                    });
                })
                .detach();
            }
        }
    }
}

/// A pick on the docked prompt to `sessions.answer`, its reason with it; whatever the host would not take is said under
/// the dock while the composer still writes into the thread `key` names (answerPrompt.ts).
pub fn answer(host: &Host, answer: Answer, composer: WeakEntity<Composer>, key: String, cx: &mut App) {
    let request =
        SessionsAnswerRequest { session_id: answer.session, ask_id: answer.ask.clone(), option_id: answer.option, reason: answer.reason };
    let host = host.clone();
    cx.spawn(async move |cx| {
        let refused = match call(&host, request).await {
            Ok(result) => {
                let outcome = serde_json::to_value(result.outcome).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
                dock::answer_words(&outcome)
                    .map(|said| crate::screens::thread_view::cards::Refusal { said: dock::sentence(said), fix: None })
            }
            Err(failure) => Some(dock::refusal_of(&failure)),
        };
        _ = composer.update(cx, |c, cx| {
            if c.holds(&key) {
                c.prompt_refused(&answer.ask, refused, cx);
            }
        });
    })
    .detach();
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
    Ok(thread_rows(sessions_of(host, workspace_id).await?, thread))
}

/// Every row of a workspace.
async fn sessions_of(host: &Host, workspace_id: &str) -> Result<Vec<SessionView>, Failure> {
    Ok(call(host, request::<protocol::SessionsListRequest>(json!({ "workspaceId": workspace_id }))?).await?.sessions)
}

/// The agents a workspace offers and their choices.
async fn agents_of(host: &Host, workspace_id: &str) -> Result<Vec<Agent>, Failure> {
    let listed = call(host, request::<protocol::HarnessesListRequest>(json!({ "workspaceId": workspace_id }))?).await?;
    Ok(listed.harnesses.iter().filter_map(Agent::read).collect())
}

/// Whether the preferences' `midTurn` steers and their `sendWith` sends on Cmd or Ctrl and Enter.
pub fn followed(preferences: &impl serde::Serialize) -> (bool, bool) {
    let read = serde_json::to_value(preferences).unwrap_or_default();
    (read["midTurn"] == "steer", read["sendWith"] == "mod-enter")
}

/// One thread's rows out of a workspace's, oldest first.
fn thread_rows(sessions: Vec<SessionView>, thread: &str) -> Vec<SessionView> {
    let mut rows: Vec<SessionView> = sessions.into_iter().filter(|r| r.thread_id.as_deref() == Some(thread)).collect();
    rows.sort_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.)));
    rows
}

/// One daemon op on the workspace's computer over a channel opened for it, and its reply.
pub(super) async fn daemon(host: &Host, workspace_id: &str, frame: Value) -> Result<Value, Failure> {
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

/// A fork's first send as the web app's `start()` writes it: no workspace and no thread, the source and its turn, the
/// new branch where the person picked one, the picks a thread that ran takes, the files and Fast.
pub fn fork_params(fork: &Fork, out: &Outgoing) -> Value {
    let target = Target { workspace_id: String::new(), thread_id: None };
    let mut params = start_params(&target, out, true);
    if let Some(params) = params.as_object_mut() {
        params.remove("workspaceId");
    }
    params["fork"] = match &fork.turn {
        Some(turn) => json!({ "threadId": fork.thread, "turnId": turn }),
        None => json!({ "threadId": fork.thread, "at": 0 }),
    };
    if let Some(branch) = &fork.branch {
        params["branch"] = Value::from(branch.as_str());
    }
    params
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

    #[test]
    fn follows_mid_turn_and_send_with() {
        let read = |v: Value| followed(&v);
        assert_eq!(read(json!({ "midTurn": "steer", "sendWith": "mod-enter" })), (true, true));
        assert_eq!(read(json!({ "midTurn": "queue", "sendWith": "enter" })), (false, false));
        assert_eq!(read(json!({})), (false, false), "Enter sends where the host names no key, as today's default");
    }

    fn row(more: Value) -> SessionView {
        let mut v = json!({ "id": "s1", "workspaceId": "w", "harness": "claude", "status": "failed" });
        v.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn the_drawer_words_a_limit_a_held_turn_and_the_steps_as_the_bars_say_them() {
        let agents = [Agent { harness: "claude".into(), label: "Claude Code".into(), ..Agent::default() }];
        let hit = drawer_of(&[row(json!({ "limit": { "resetsAt": 1_700_000_000_000.0 } }))], None, false, &agents);
        let limit = hit.limit.expect("a stopped turn's limit");
        assert_eq!((limit.title.as_str(), limit.resets, limit.armed), ("Usage limit reached", true, false));
        assert_eq!(limit.note, "Claude Code stopped this turn until its plan resets.");
        let armed = drawer_of(&[row(json!({ "limit": { "resetsAt": 1.0 }, "resumeAt": 1.0 }))], None, false, &agents).limit.unwrap();
        assert_eq!(
            (armed.title.as_str(), armed.note.as_str()),
            ("Resumes at the reset", "Claude Code goes on with this turn when its plan resets.")
        );
        let unknown = drawer_of(&[row(json!({ "limit": {} }))], None, false, &agents).limit.unwrap();
        assert!(!unknown.resets);
        assert_eq!(unknown.note, "Claude Code stopped this turn and did not say when its plan resets.");

        let capped = |why: Value| {
            let mut c = json!({ "placeId": "p1", "place": "build-box", "running": 1, "atOnce": 1 });
            if !why.is_null() {
                c["why"] = why;
            }
            drawer_of(&[row(json!({ "status": "running", "capped": c }))], None, false, &agents).slot.unwrap()
        };
        let slot = capped(Value::Null);
        assert_eq!(slot.title, "Waiting for a thread slot on build-box");
        assert_eq!(slot.note, "1 of 1 thread is running. It starts on its own when one ends.");
        assert_eq!((slot.raise.as_deref(), slot.place_id.as_str()), (Some("Raise threads at once"), "p1"));
        assert_eq!(capped(json!("slot")).raise.as_deref(), Some("Raise machines at once"));
        assert_eq!(capped(json!("minute")).raise, None);

        let steps = [json!({ "text": "a", "state": "done" }), json!({ "text": "b", "state": "working" })];
        let plan = Plan::next(None, &steps, Some(5.0));
        assert!(drawer_of(&[row(json!({}))], Some(&plan), false, &agents).tasks.is_none(), "only while the turn runs");
        assert_eq!(drawer_of(&[row(json!({}))], Some(&plan), true, &agents).tasks.unwrap().count(), "1/2");
    }

    #[test]
    fn resume_at_reset_marks_the_thread_by_sessions_mark() {
        let params = mark_params("t1", true);
        assert_eq!(params, json!({ "threadIds": ["t1"], "marks": { "resumeAtReset": true } }));
        assert!(request::<protocol::SessionsMarkRequest>(params).is_ok(), "the host's request takes it");
        assert!(request::<protocol::SessionsMarkRequest>(mark_params("t1", false)).is_ok());
    }

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
            fork: None,
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
