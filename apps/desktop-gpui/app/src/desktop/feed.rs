// SPDX-License-Identifier: AGPL-3.0-only
//! The shell's own socket to the host (`desktop:host-feed.ts`): the rows, the prompts open on them and the switches the
//! shell acts on, kept current off the host's pushes. The rows are read again 150 ms after a turn starts, asks or
//! ends, or a workspace or computer comes or goes; a host that does not answer, or goes, reads as lost and is dialled
//! again every 3 s. From it the shell draws the menu bar and the Dock's badge, holds the computer awake, says what
//! happened while no window has the person's eyes, and answers the Quit question.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::{FutureExt, StreamExt};
use gpui_kit::{App, AsyncApp, Global};
use serde_json::{json, Map, Value};
use tokio::runtime::Runtime;
use wsp_host_client::protocol::{SessionsAnswerRequest, SessionsInterruptRequest};
use wsp_host_client::{start, Env, Failure, HostClient};

use super::notify::{self, Choices, NotifyChoice};
use super::tray::{self, OpenAsk, TrayAct, TrayHost, TrayInput, TrayModel};
use super::{awake, quit};
use crate::screens::sidebar::host::{ask as call, kind_of};
use crate::screens::sidebar::looks::place_name;
use crate::screens::sidebar::model::Row;
use crate::screens::sidebar::shared;
use crate::screens::Route;
use crate::shell::Shell;

/// How long a burst of pushes is let settle before the rows are read again.
pub const SETTLE: Duration = Duration::from_millis(150);
/// How long after a failed dial or a lost socket the next dial waits.
pub const RETRY: Duration = Duration::from_secs(3);

/// `OnQuit`: General's standing answer to the Quit question.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OnQuit {
    #[default]
    Ask,
    Keep,
    Stop,
}

/// The five switches the shell acts on (`preferences.get`, `preferences.changed`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub keep_awake: bool,
    pub choices: Choices,
    pub on_quit: OnQuit,
}

impl Default for Prefs {
    /// `DEFAULT_PREFERENCES`.
    fn default() -> Self {
        Prefs { keep_awake: true, choices: Choices::default(), on_quit: OnQuit::Ask }
    }
}

impl Prefs {
    /// The switches a preferences record holds, each one it lacks at its default.
    pub fn of(preferences: &Value) -> Prefs {
        let base = Prefs::default();
        let word = |k: &str| preferences.get(k).and_then(Value::as_str);
        let flag = |k: &str| preferences.get(k).and_then(Value::as_bool);
        let choice = |k: &str, or: NotifyChoice| word(k).and_then(NotifyChoice::parse).unwrap_or(or);
        Prefs {
            keep_awake: flag("keepAwake").unwrap_or(base.keep_awake),
            choices: Choices {
                notify_needs: choice("notifyNeeds", base.choices.notify_needs),
                notify_done: choice("notifyDone", base.choices.notify_done),
                plan_alerts: flag("planAlerts").unwrap_or(base.choices.plan_alerts),
            },
            on_quit: match word("onQuit") {
                Some("keep") => OnQuit::Keep,
                Some("stop") => OnQuit::Stop,
                _ => OnQuit::Ask,
            },
        }
    }
}

/// `FeedState`: what the feed holds of the host the window is on. The lists are shared, so the state the shell keeps
/// and the one the feed's task folds pushes into are one copy until a read replaces them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FeedState {
    /// The rows the sidebar and Activity hold too ([`shared`]).
    pub sessions: Arc<[Arc<Row>]>,
    pub workspaces: Arc<[Value]>,
    pub places: Arc<[Value]>,
    /// The prompts seen open, by the turn that asks each.
    pub asks: HashMap<String, OpenAsk>,
    /// A pick the host would not take, in its words, by the prompt it was made on, kept until a prompt closes.
    pub refused: HashMap<String, String>,
    pub prefs: Prefs,
    pub lost: bool,
    /// The host serves from another computer.
    pub remote: bool,
    /// The alias of the host on the account the feed dials, where it dials one.
    pub alias: Option<String>,
}

impl FeedState {
    /// What the menu calls the host: the alias of one on the account, else the name of the computer it runs on.
    pub fn label(&self) -> String {
        if let Some(alias) = &self.alias {
            return alias.clone();
        }
        self.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some("here")).map(place_name).unwrap_or_default()
    }

    pub fn tray(&self) -> TrayModel {
        let host = TrayHost { label: self.label(), remote: self.remote, lost: self.lost };
        tray::tray_model(TrayInput {
            sessions: &self.sessions,
            asks: &self.asks,
            refused: &self.refused,
            workspaces: &self.workspaces,
            places: &self.places,
            host: &host,
        })
    }

    /// Whether this computer is kept awake: a thread works on its own workspace, Keep awake is on, and the host is
    /// here and answering.
    pub fn awake(&self) -> bool {
        !self.lost && !self.remote && awake::awake_wanted(&self.sessions, &self.workspaces, self.prefs.keep_awake)
    }
}

/// `REREAD`: the pushes after which the rows read differently.
pub const REREAD: [&str; 12] = [
    "session.start",
    "session.done",
    "session.end",
    "session.permission",
    "session.permission.closed",
    "thread.marked",
    "workspace.created",
    "workspace.deleted",
    "workspace.renamed",
    "workspace.gone",
    "place.joined",
    "place.removed",
];

/// The pushes the shell may say over the system (`trayNotice`).
const SAID: [&str; 3] = ["session.permission", "session.done", "usage.alert"];

/// What one push did to the feed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Heard {
    pub changed: bool,
    pub reread: bool,
}

/// `heard`: a push folded into the feed: the switches, a prompt opened or closed.
pub fn heard(state: &mut FeedState, kind: &str, frame: &Value) -> Heard {
    let session = frame.get("sessionId").and_then(Value::as_str);
    let mut changed = false;
    if kind == "preferences.changed" {
        if let Some(preferences) = frame.get("preferences") {
            state.prefs = Prefs::of(preferences);
            changed = true;
        }
    }
    if let (Some(session), "session.permission") = (session, kind) {
        state.asks.insert(session.to_owned(), OpenAsk::of(frame));
        changed = true;
    }
    if let Some(session) = session {
        let closed = kind == "session.permission.closed"
            && state.asks.get(session).is_some_and(|a| Some(a.ask_id.as_str()) == frame.get("askId").and_then(Value::as_str));
        if closed || (kind == "session.end" && state.asks.contains_key(session)) {
            state.asks.remove(session);
            let asks = &state.asks;
            state.refused.retain(|ask_id, _| asks.values().any(|a| a.ask_id == *ask_id));
            changed = true;
        }
    }
    Heard { changed, reread: REREAD.contains(&kind) }
}

/// `ANSWER_WORDS`: what a pick the host would not take came to; none for one it took.
pub fn answer_words(outcome: &str) -> Option<String> {
    let said = match outcome {
        "answered" => return None,
        "gone" => "the prompt closed before the answer reached it",
        "unsupported" => "this thread's agent raises no prompt this host can answer",
        "not-found" => "this host holds no turn of that thread",
        "no-option" => "the prompt carries no option by that id",
        other => other,
    };
    Some(said.to_owned())
}

/// A pick's answer kept by its prompt: a refusal in the host's words, and nothing once a pick is taken.
pub fn answered(state: &mut FeedState, ask_id: &str, said: Option<String>) {
    match said {
        Some(said) => _ = state.refused.insert(ask_id.to_owned(), said),
        None => _ = state.refused.remove(ask_id),
    }
}

/// What the feed hands the shell.
#[derive(Debug)]
pub enum Fed {
    State(Box<FeedState>),
    /// A push the shell may say over the system, as it was sent.
    Event(Value),
}

/// What the shell asks of the feed.
#[derive(Debug)]
pub enum Ask {
    Answer {
        session_id: String,
        ask_id: String,
        option_id: String,
    },
    Interrupt(String),
    /// Dials now rather than at the next retry, for a Start that just brought the host back.
    Redial,
}

/// The feed's task, stopped when this goes.
pub struct Feed {
    asks: mpsc::UnboundedSender<Ask>,
    task: tokio::task::AbortHandle,
}

impl Feed {
    pub fn ask(&self, ask: Ask) {
        _ = self.asks.unbounded_send(ask);
    }
}

impl Drop for Feed {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Follows the host on the account saved under `alias`, or the host serving `state` for none, on `runtime`, handing
/// each change to `tx`, until the feed or the receiver goes.
pub fn follow(
    alias: Option<String>,
    state: PathBuf,
    env: Env,
    runtime: &Runtime,
    settle: Duration,
    retry: Duration,
    tx: mpsc::UnboundedSender<Fed>,
) -> Feed {
    let (asks, mut asked) = mpsc::unbounded();
    let task = runtime
        .spawn(async move {
            let mut fed = FeedState::default();
            loop {
                match HostClient::connect_on(&state, &env, alias.as_deref(), None).await {
                    Ok(client) => {
                        if let Err(failure) = listen(Arc::new(client), &mut fed, settle, &tx, &mut asked).await {
                            super::log::error(&format!("menu bar: {}", failure.message));
                        }
                        if tx.is_closed() {
                            return;
                        }
                        fed.lost = true;
                        fed.asks.clear();
                        fed.refused.clear();
                        _ = tx.unbounded_send(Fed::State(Box::new(fed.clone())));
                    }
                    Err(failure) => {
                        super::log::error(&format!("menu bar: {}", failure.message));
                        if !fed.lost {
                            fed.lost = true;
                            _ = tx.unbounded_send(Fed::State(Box::new(fed.clone())));
                        }
                    }
                }
                let mut wait = pin!(tokio::time::sleep(retry).fuse());
                loop {
                    futures::select! {
                        _ = wait => break,
                        ask = asked.next() => match ask {
                            Some(Ask::Redial) => break,
                            Some(_) => {}
                            None => return,
                        },
                    }
                }
            }
        })
        .abort_handle();
    Feed { asks, task }
}

/// The rows, the workspaces and the computers, read again as the host sent them, as `host-feed.ts` reads them, so a
/// row of a newer host fails nothing; a host that lists no computers still has its rows read.
async fn reread(client: &Arc<HostClient>, fed: &mut FeedState) -> Result<(), Failure> {
    let (sessions, workspaces, places) = futures::join!(
        listed(client, "sessions.list", "sessions"),
        listed(client, "workspaces.list", "workspaces"),
        listed(client, "places.list", "places"),
    );
    fed.sessions = shared::rows_of(sessions?).into();
    fed.workspaces = workspaces?.into();
    fed.places = match places {
        Ok(places) => places.into(),
        Err(failure) => {
            super::log::error(&format!("menu bar: {}", failure.message));
            Arc::default()
        }
    };
    Ok(())
}

/// One list op's rows as the host sent them, a row with no id skipped and logged.
pub(super) async fn listed(client: &HostClient, op: &str, key: &str) -> Result<Vec<Value>, Failure> {
    let Some(Value::Array(rows)) = client.call_raw(op, Map::new()).await?.remove(key) else {
        return Err(Failure::new(format!("{op} answered no {key}")));
    };
    let read = |(at, row): &(usize, Value)| {
        let read = row.get("id").is_some_and(Value::is_string);
        if !read {
            super::log::error(&format!("menu bar: {op} answered row {at} with no id, skipped"));
        }
        read
    };
    Ok(rows.into_iter().enumerate().filter(read).map(|(_, row)| row).collect())
}

/// The state handed to the shell.
fn send(tx: &mpsc::UnboundedSender<Fed>, fed: &FeedState) -> Result<(), Failure> {
    tx.unbounded_send(Fed::State(Box::new(fed.clone()))).map_err(|_| Failure::new("the shell let go of the feed"))
}

/// The frames already waiting when the subscription answers, folded into the feed and said to nobody: a finish from an
/// hour ago is not news.
fn replayed<S: futures::Stream<Item = (String, Value)> + Unpin>(events: &mut S, fed: &mut FeedState) {
    while let Some(Some((kind, frame))) = events.next().now_or_never() {
        heard(fed, &kind, &frame);
    }
}

/// A live push folded into the feed: the state handed on where it changed, then the push itself where the shell may
/// say it.
fn fold(fed: &mut FeedState, kind: &str, frame: Value, tx: &mpsc::UnboundedSender<Fed>) -> Result<Heard, Failure> {
    let heard = heard(fed, kind, &frame);
    if heard.changed {
        send(tx, fed)?;
    }
    if SAID.contains(&kind) {
        _ = tx.unbounded_send(Fed::Event(frame));
    }
    Ok(heard)
}

/// One socket's life: the switches, the pushes from the moment it listens, the rows, then every push and every ask
/// until the socket goes.
async fn listen(
    client: Arc<HostClient>,
    fed: &mut FeedState,
    settle: Duration,
    tx: &mpsc::UnboundedSender<Fed>,
    asked: &mut mpsc::UnboundedReceiver<Ask>,
) -> Result<(), Failure> {
    let preferences = client.call_raw("preferences.get", Map::new()).await?.remove("preferences").unwrap_or_default();
    let events = client.events().await?;
    // A push of a newer protocol than the record is skipped: the next read brings what it said.
    let events = futures::stream::unfold(events, |mut events| async move { Some((events.next().await?, events)) })
        .filter_map(|event| async move { event.ok().as_ref().and_then(kind_of) });
    let mut events = Box::pin(events).fuse();
    fed.prefs = Prefs::of(&preferences);
    fed.lost = false;
    fed.remote = !client.here();
    fed.alias = client.alias();
    replayed(&mut events, fed);
    reread(&client, fed).await?;
    send(tx, fed)?;
    let mut settled = Box::pin(futures::future::Fuse::terminated());
    loop {
        futures::select! {
            event = events.next() => {
                let Some((kind, frame)) = event else { return Ok(()) };
                if fold(fed, &kind, frame, tx)?.reread {
                    settled.set(tokio::time::sleep(settle).fuse());
                }
            }
            _ = settled => match reread(&client, fed).await {
                Ok(()) => send(tx, fed)?,
                Err(failure) => super::log::error(&format!("menu bar: {}", failure.message)),
            },
            ask = asked.next() => match ask {
                None => return Ok(()),
                Some(Ask::Answer { session_id, ask_id, option_id }) => {
                    let params = json!({ "sessionId": session_id, "askId": ask_id, "optionId": option_id });
                    let said = match call::<SessionsAnswerRequest>(client.clone(), params).await {
                        Ok(reply) => answer_words(&reply.outcome.to_string()),
                        Err(failure) => Some(failure.message),
                    };
                    answered(fed, &ask_id, said);
                    send(tx, fed)?;
                }
                Some(Ask::Interrupt(session_id)) => {
                    if let Err(failure) = call::<SessionsInterruptRequest>(client.clone(), json!({ "sessionId": session_id })).await {
                        super::log::error(&format!("menu bar: {}", failure.message));
                    }
                }
                Some(Ask::Redial) => {}
            },
        }
    }
}

/// The shell around the window: where the feed dials, the feed once the app's window stands, and the last it said.
struct Desk {
    state: PathBuf,
    env: Env,
    runtime: Arc<Runtime>,
    feed: Option<Feed>,
    fed: Option<FeedState>,
    /// Counts the feeds begun, so what a feed left behind once the window moved is not taken for the next one's.
    begun: u64,
    draw: Draw,
}

impl Global for Desk {}

/// What the shell draws outside its windows from each state the feed hands over.
pub type Draw = Rc<dyn Fn(&TrayModel, &mut App)>;

/// A Mac's Dock badge and menu bar item; nothing elsewhere.
pub fn native(model: &TrayModel, cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        super::mac::set_badge(model.badge);
        tray::draw(model, Rc::new(act), cx);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (model, cx);
}

/// The shell's native pieces: notifications and their clicks, then the feed once the app's window first stands, never
/// for onboarding, the starting window or a failed start, each of its states drawn by `draw`. `reopen` opens the app's
/// window where none stands.
pub fn start(state: PathBuf, env: Env, runtime: Arc<Runtime>, reopen: Rc<dyn Fn(&mut App)>, draw: Draw, cx: &mut App) {
    notify::init(reopen, cx);
    cx.set_global(Desk { state, env, runtime, feed: None, fed: None, begun: 0, draw });
    cx.observe_new::<Shell>(|_, _, cx| cx.defer(begin)).detach();
    let store = crate::prefs::store(cx);
    cx.observe(&store, |_, cx| {
        let Some(mut fed) = cx.global::<Desk>().fed.clone() else { return };
        if switched(&mut fed, cx) {
            awake::hold(fed.awake(), cx);
            cx.global_mut::<Desk>().fed = Some(fed);
        }
    })
    .detach();
}

/// The switches as the app's store shows them, a write still out over the host's record, where it holds one; whether
/// they moved.
pub(crate) fn switched(fed: &mut FeedState, cx: &App) -> bool {
    let Some(shown) = crate::prefs::record(cx).map(Prefs::of) else { return false };
    let moved = fed.prefs != shown;
    fed.prefs = shown;
    moved
}

fn begin(cx: &mut App) {
    if cx.global::<Desk>().feed.is_some() {
        return;
    }
    let alias = super::hosts::picked(cx);
    let desk = cx.global_mut::<Desk>();
    let (tx, rx) = mpsc::unbounded();
    desk.begun += 1;
    let begun = desk.begun;
    desk.feed = Some(follow(alias, desk.state.clone(), desk.env.clone(), &desk.runtime, SETTLE, RETRY, tx));
    take(rx, begun, cx);
}

/// What the feed numbered `begun` hands over, taken while it is the latest feed begun.
fn take(mut rx: mpsc::UnboundedReceiver<Fed>, begun: u64, cx: &mut App) {
    let current = move |cx: &App| cx.global::<Desk>().begun == begun;
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Some(fed) = rx.next().await {
            cx.update(|cx| {
                if current(cx) {
                    took(fed, cx)
                }
            });
        }
        // The feed's task ended, a panic in it included, so nothing says a thread works here any more.
        cx.update(|cx| {
            if current(cx) {
                awake::hold(false, cx)
            }
        });
    })
    .detach();
}

/// The feed dialled again at the host the window is on now, once the window moved; nothing before the feed began.
pub fn follow_window(cx: &mut App) {
    if cx.try_global::<Desk>().is_none_or(|d| d.feed.is_none()) {
        return;
    }
    let desk = cx.global_mut::<Desk>();
    (desk.feed, desk.fed) = (None, None);
    begin(cx);
}

fn took(fed: Fed, cx: &mut App) {
    match fed {
        Fed::State(mut state) => {
            switched(&mut state, cx);
            awake::hold(state.awake(), cx);
            let draw = cx.global::<Desk>().draw.clone();
            draw(&state.tray(), cx);
            cx.global_mut::<Desk>().fed = Some(*state);
        }
        Fed::Event(event) => {
            let Some(fed) = cx.global::<Desk>().fed.as_ref() else { return };
            let Some(line) = tray::tray_notice(&event, &fed.sessions, &fed.workspaces, &fed.places, &fed.prefs.choices) else { return };
            let field = |k: &str| event.get(k).and_then(Value::as_str).map(str::to_owned);
            let open = match (field("workspaceId"), field("threadId").or(field("sessionId"))) {
                (Some(workspace_id), Some(thread_id)) if !fed.remote => Some(Rc::new(move |cx: &mut App| {
                    crate::screens::app_shell::navigate(
                        Route::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() },
                        cx,
                    )
                }) as notify::Opener),
                _ => None,
            };
            notify::say(&line, open, cx);
        }
    }
}

/// What a menu bar row does when it is picked.
pub fn act(what: TrayAct, cx: &mut App) {
    let ask = |ask: Ask, cx: &mut App| {
        if let Some(feed) = cx.try_global::<Desk>().and_then(|d| d.feed.as_ref()) {
            feed.ask(ask);
        }
    };
    match what {
        TrayAct::Open { workspace_id, thread_id } => {
            let remote = cx.try_global::<Desk>().and_then(|d| d.fed.as_ref()).is_some_and(|f| f.remote);
            notify::raise(cx);
            // A thread on a host somewhere else is read in that host's own window.
            if !remote {
                crate::screens::app_shell::navigate(Route::Thread { workspace_id, thread_id }, cx);
            }
        }
        TrayAct::Answer { session_id, ask_id, option_id } => ask(Ask::Answer { session_id, ask_id, option_id }, cx),
        TrayAct::Stop { session_id } => ask(Ask::Interrupt(session_id), cx),
        TrayAct::Start => start_host(cx),
        TrayAct::OpenApp => notify::raise(cx),
        TrayAct::Quit => quit::ask(cx),
    }
}

/// Menu bar Start wsp (`desktop:main.ts:758`): the launch's host start without its starting window, then the feed
/// dials again; a failure goes to the log as "menu bar: <why>".
fn start_host(cx: &mut App) {
    let Some(desk) = cx.try_global::<Desk>() else { return };
    let mut road = start::Start::here(&desk.state, &desk.env);
    road.wsp = crate::service::owned();
    let started = desk
        .runtime
        .spawn(async move { start::ready(&road, &start::System::new(&road.env), |_| async { Ok::<_, Failure>(()) }).await.map(|_| ()) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        match started.await {
            Ok(Ok(())) => {}
            Ok(Err(failed)) => super::log::error(&format!("menu bar: {}", failed.failure.message)),
            Err(e) => super::log::error(&format!("menu bar: {e}")),
        }
        cx.update(|cx| {
            if let Some(feed) = cx.try_global::<Desk>().and_then(|d| d.feed.as_ref()) {
                feed.ask(Ask::Redial);
            }
        });
    })
    .detach();
}

/// What the Quit question reads of the feed: none before the app's window first stood.
pub struct QuitHost {
    pub remote: bool,
    pub on_quit: OnQuit,
    pub state: PathBuf,
    pub env: Env,
    pub runtime: Arc<Runtime>,
}

pub fn quit_host(cx: &App) -> Option<QuitHost> {
    let desk = cx.try_global::<Desk>()?;
    desk.feed.as_ref()?;
    let fed = desk.fed.clone().unwrap_or_default();
    Some(QuitHost {
        remote: fed.remote,
        on_quit: fed.prefs.on_quit,
        state: desk.state.clone(),
        env: desk.env.clone(),
        runtime: desk.runtime.clone(),
    })
}

/// The last state the feed handed over, which a test reads.
pub fn fed(cx: &App) -> Option<FeedState> {
    cx.try_global::<Desk>()?.fed.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Catches: a state, or the awake hold's release, that a feed the window moved away from left behind, taken as the
    /// next feed's.
    #[gpui_kit::test]
    fn what_a_feed_left_behind_is_not_taken(cx: &mut gpui_kit::TestAppContext) {
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        cx.update(|cx| {
            cx.set_global(Desk {
                state: PathBuf::new(),
                env: Env::default(),
                runtime,
                feed: None,
                fed: None,
                begun: 2,
                draw: Rc::new(|_, _| {}),
            })
        });
        let ((old, left), (new, now)) = (mpsc::unbounded(), mpsc::unbounded());
        cx.update(|cx| {
            take(left, 1, cx);
            take(now, 2, cx);
        });
        let studio = FeedState { remote: true, alias: Some("studio".to_owned()), ..FeedState::default() };
        new.unbounded_send(Fed::State(Box::new(studio.clone()))).unwrap();
        cx.run_until_parked();
        cx.update(|cx| awake::hold(true, cx));
        cx.run_until_parked();
        old.unbounded_send(Fed::State(Box::default())).unwrap();
        drop(old);
        cx.run_until_parked();
        assert_eq!(cx.update(|cx| fed(cx)), Some(studio), "the state is the latest feed's");
        assert!(cx.update(|cx| awake::held(cx)), "the old feed's end lets go of nothing");
    }

    fn permission() -> Value {
        json!({ "type": "session.permission", "workspaceId": "ws_mac", "sessionId": "s1", "threadId": "t1", "askId": "a1", "toolName": "Bash", "input": "{}", "options": [{ "id": "o_yes", "label": "Yes", "effect": "allow" }] })
    }

    /// `host-feed.test.ts:56`: a prompt is kept by the turn that asks it until it closes, and the switches follow.
    #[test]
    fn a_prompt_is_kept_until_it_closes_and_the_switches_follow_the_pushes() {
        let mut fed = FeedState::default();
        assert_eq!(heard(&mut fed, "session.permission", &permission()), Heard { changed: true, reread: true });
        assert_eq!(fed.asks["s1"], OpenAsk { ask_id: "a1".to_owned(), options: vec![("o_yes".to_owned(), "allow".to_owned())] });
        let other = json!({ "type": "session.permission.closed", "sessionId": "s1", "askId": "a0" });
        assert!(!heard(&mut fed, "session.permission.closed", &other).changed, "a close of another prompt keeps this one");
        let closed = json!({ "type": "session.permission.closed", "sessionId": "s1", "askId": "a1", "outcome": "allowed" });
        assert!(heard(&mut fed, "session.permission.closed", &closed).changed);
        assert!(fed.asks.is_empty());
        heard(&mut fed, "session.permission", &permission());
        heard(&mut fed, "session.end", &json!({ "type": "session.end", "sessionId": "s1" }));
        assert!(fed.asks.is_empty(), "a turn that ended takes its prompt with it");
        let prefs = json!({ "keepAwake": true, "notifyNeeds": "sound", "notifyDone": "notify", "planAlerts": false, "onQuit": "keep" });
        heard(&mut fed, "preferences.changed", &json!({ "type": "preferences.changed", "preferences": prefs }));
        assert_eq!(
            fed.prefs,
            Prefs {
                keep_awake: true,
                choices: Choices { notify_needs: NotifyChoice::Sound, notify_done: NotifyChoice::Notify, plan_alerts: false },
                on_quit: OnQuit::Keep
            }
        );
    }

    /// `host-feed.test.ts:77`.
    #[test]
    fn a_refused_pick_is_kept_in_the_hosts_words_until_a_pick_is_taken() {
        let mut fed = FeedState::default();
        answered(&mut fed, "a1", answer_words("not-found"));
        assert_eq!(fed.refused["a1"], "this host holds no turn of that thread");
        answered(&mut fed, "a1", Some("workspace api is paused. Wake it first.".to_owned()));
        assert_eq!(fed.refused["a1"], "workspace api is paused. Wake it first.");
        answered(&mut fed, "a1", answer_words("answered"));
        assert!(fed.refused.is_empty());
    }

    /// A refusal goes once a prompt closes, so a long run does not keep one per refused pick.
    #[test]
    fn a_refusal_goes_with_its_prompt() {
        let mut fed = FeedState::default();
        heard(&mut fed, "session.permission", &permission());
        answered(&mut fed, "a1", answer_words("no-option"));
        answered(&mut fed, "a0", answer_words("gone"));
        assert_eq!(fed.refused.len(), 2);
        heard(&mut fed, "session.permission.closed", &json!({ "type": "session.permission.closed", "sessionId": "s1", "askId": "a1" }));
        assert!(fed.refused.is_empty(), "{:?}", fed.refused);
    }

    /// `host-feed.ts:150`: frames waiting when the subscription answers fold into the feed and are said to nobody; a
    /// push after that is said.
    #[test]
    fn a_finish_replayed_before_the_feed_listens_is_not_said_and_a_later_one_is() {
        let done = |id: &str| json!({ "type": "session.done", "sessionId": id, "result": { "status": "completed" } });
        let (frames, mut events) = mpsc::unbounded::<(String, Value)>();
        let (tx, mut rx) = mpsc::unbounded();
        frames.unbounded_send(("session.permission".to_owned(), permission())).unwrap();
        frames.unbounded_send(("session.done".to_owned(), done("s_old"))).unwrap();
        let mut fed = FeedState::default();
        replayed(&mut events, &mut fed);
        frames.unbounded_send(("session.done".to_owned(), done("s_new"))).unwrap();
        while let Some(Some((kind, frame))) = events.next().now_or_never() {
            fold(&mut fed, &kind, frame, &tx).unwrap();
        }
        let said: Vec<Value> = std::iter::from_fn(|| rx.try_recv().ok())
            .filter_map(|fed| match fed {
                Fed::Event(event) => Some(event),
                Fed::State(_) => None,
            })
            .collect();
        assert_eq!(said, [done("s_new")]);
        assert!(fed.asks.contains_key("s1"), "a replayed prompt is still held");
    }

    /// `host-feed.test.ts:96`: a turn starting reads the rows again; its own words and a machine's samples do not.
    #[test]
    fn the_rows_are_read_again_after_a_turn_starts_and_not_on_its_words() {
        let mut fed = FeedState::default();
        for kind in ["session.delta", "workspace.sys", "thread.head"] {
            assert_eq!(heard(&mut fed, kind, &json!({ "type": kind, "sessionId": "s1" })), Heard::default(), "{kind}");
        }
        for kind in REREAD {
            assert!(heard(&mut fed, kind, &json!({ "type": kind })).reread, "{kind}");
        }
    }

    /// `host-feed.test.ts:123`: a socket gone drops the prompts, so rows lose Allow and Deny until the next read.
    #[test]
    fn a_lost_host_reads_as_lost_with_no_open_prompts() {
        let row = json!({ "id": "s1", "workspaceId": "ws_mac", "harness": "claude", "status": "running", "threadId": "t1", "asking": "Bash: ls" });
        let mut fed = FeedState { sessions: shared::rows_of(vec![row]).into(), ..FeedState::default() };
        heard(&mut fed, "session.permission", &permission());
        let allows = |fed: &FeedState| {
            fed.tray()
                .rows
                .iter()
                .any(|r| matches!(r, tray::TrayRow::Thread { actions, .. } if actions.iter().any(|(l, _)| l == tray::ALLOW)))
        };
        assert!(allows(&fed));
        fed.asks.clear();
        assert!(!allows(&fed));
        fed.lost = true;
        let model = fed.tray();
        assert_eq!((model.title.as_str(), model.badge, model.needs_you), ("", 0, false));
    }

    #[test]
    fn a_record_with_no_switches_reads_as_the_defaults() {
        assert_eq!(Prefs::of(&json!({})), Prefs::default());
        assert_eq!(Prefs::of(&json!({ "notifyNeeds": "loud", "onQuit": "later" })), Prefs::default());
    }

    #[test]
    fn awake_only_here_and_answering() {
        let row = json!({ "id": "s1", "workspaceId": "ws_mac", "harness": "claude", "status": "running", "threadId": "t1" });
        let mut fed = FeedState {
            sessions: shared::rows_of(vec![row]).into(),
            workspaces: [json!({ "id": "ws_mac", "kind": "local" })].into(),
            ..FeedState::default()
        };
        assert!(fed.awake());
        fed.lost = true;
        assert!(!fed.awake(), "not when lost");
        fed.lost = false;
        fed.remote = true;
        assert!(!fed.awake(), "not on a host elsewhere");
    }
}
