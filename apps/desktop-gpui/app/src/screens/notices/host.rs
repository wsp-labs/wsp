// SPDX-License-Identifier: AGPL-3.0-only
//! The host's events that become notices, one rule per event type (`notices/hostNotices.ts`). A notice is for what
//! happened away from where the person is looking: an event whose home is on screen (that thread open, that
//! computer's page, that workspace's thread) says nothing here. A wait (a build's need, a thread's prompt, a setup's
//! sign-in) is keyed and stands until the event that closes it. What goes out on the system's own notifications, and
//! a workspace waking, which says only that, are not toasts.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use futures::channel::mpsc;
use futures::StreamExt;
use gpui_kit::{Context, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{InitGetRequest, PreferencesGetRequest};
use wsp_host_client::{Failure, HostClient};

use super::words::{self, OPEN};
use super::{route, Action, Kind, Notice, Notices};
use crate::screens::app_shell::split;
use crate::screens::dialogs::{add_cloud, add_computer};
use crate::screens::sidebar::host::{ask, kind_of};
use crate::screens::sidebar::looks::place_name;
use crate::screens::{navigate, Host, Route};

/// A turn whose session.end never arrived (a socket gap) is not held past this many later turns, nor a job's end
/// or a setup's past this many others.
const HELD: usize = 64;
const NEED_KEY: &str = "needs-you";

/// One push the rules read, with the preferences read just after it where its rule reads them.
#[derive(Debug)]
pub struct Heard {
    pub kind: String,
    pub frame: Value,
    pub preferences: Option<Value>,
}

/// What the rules keep between events.
#[derive(Default)]
pub struct Rules {
    /// The build job as last read, its need included.
    job: Option<Value>,
    /// The need the keyed notice stands for.
    need: Option<String>,
    /// Whether the build's page was in front of the person at the last change.
    shown: bool,
    jobs_ended: VecDeque<String>,
    /// Each running turn's result, by turn id, from its session.done until its session.end.
    results: VecDeque<(String, Value)>,
    /// The sign-in waits said, by computer and row, with the computer and the run they were said for.
    waits: HashMap<String, (String, String)>,
    /// The setups that ended ready, by computer, with their run, until the list shows how.
    ready: HashMap<String, String>,
    pub(super) places: Vec<Value>,
}

impl Rules {
    pub fn job_needs_you(&self) -> bool {
        self.job.as_ref().and_then(|j| j.get("needsYou")).is_some_and(|n| !n.is_null())
    }

    fn need_what(&self) -> Option<String> {
        self.job.as_ref()?.pointer("/needsYou/what")?.as_str().map(str::to_owned)
    }

    fn job_place(&self) -> Option<String> {
        self.job.as_ref()?.pointer("/place/id")?.as_str().map(str::to_owned)
    }
}

fn s<'a>(v: &'a Value, name: &str) -> Option<&'a str> {
    v.get(name).and_then(Value::as_str)
}

fn owned(v: &Value, name: &str) -> Option<String> {
    s(v, name).map(str::to_owned)
}

fn push_held<T>(list: &mut VecDeque<T>, item: T) {
    list.push_back(item);
    if list.len() > HELD {
        list.pop_front();
    }
}

/// Whether a rule reads the preferences after its event, and whether the event is one a rule reads at all.
fn reads(kind: &str) -> Option<bool> {
    Some(match kind {
        "usage.alert" => true,
        "place.stage"
        | "place.setup"
        | "init.job"
        | "job.needs-you"
        | "session.end"
        | "session.permission"
        | "session.notify"
        | "workspace.gone"
        | "session.done"
        | "session.permission.closed"
        | "host.notice" => false,
        _ => return None,
    })
}

/// Reads the job as it stands, then sends each push a rule reads with what it reads, until the receiver is gone. A
/// socket that closes is dialled again a second later.
async fn hear(client: Arc<HostClient>, tx: mpsc::UnboundedSender<Heard>) {
    loop {
        let ended: Result<(), Failure> = async {
            let mut events = client.events().await?;
            let job = ask::<InitGetRequest>(client.clone(), json!({})).await.ok().and_then(|r| serde_json::to_value(r.setup.job).ok());
            let read = Heard { kind: "init.read".into(), frame: json!({ "job": job }), preferences: None };
            tx.unbounded_send(read).map_err(|_| Failure::new("gone"))?;
            while let Some(event) = events.next().await {
                let Ok(event) = event else { continue };
                let Some((kind, frame)) = kind_of(&event) else { continue };
                let Some(preferences) = reads(&kind) else { continue };
                let heard = Heard {
                    preferences: match preferences {
                        true => ask::<PreferencesGetRequest>(client.clone(), json!({}))
                            .await
                            .ok()
                            .and_then(|r| serde_json::to_value(&r.preferences).ok()),
                        false => None,
                    },
                    kind,
                    frame,
                };
                tx.unbounded_send(heard).map_err(|_| Failure::new("gone"))?;
            }
            Err(Failure::new("the host closed the socket"))
        }
        .await;
        if ended.is_ok() || tx.is_closed() {
            return;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

/// Follows the host's pushes for as long as the returned tasks are held.
pub fn follow(host: Host, window: &mut Window, cx: &mut Context<Notices>) -> Vec<Task<()>> {
    let (tx, mut rx) = mpsc::unbounded::<Heard>();
    let reading = host.run(move |client| async move {
        hear(client, tx).await;
        Ok::<(), Failure>(())
    });
    let keep = cx.spawn(async move |_, _| {
        _ = reading.await;
    });
    let apply = cx.spawn_in(window, async move |this, cx| {
        while let Some(heard) = rx.next().await {
            if this.update_in(cx, |n, window, cx| n.hear(heard, window, cx)).is_err() {
                break;
            }
        }
    });
    vec![keep, apply]
}

impl Notices {
    /// Takes one push as [`follow`] hands each over: the road the host's frames take, which tests drive with frames no
    /// host here can send, a computer's setup ending among them.
    pub fn hear(&mut self, heard: Heard, window: &mut Window, cx: &mut Context<Notices>) {
        apply(self, heard, window, cx);
    }
}

fn computer_route(place: Option<&str>) -> Route {
    Route::Screen(match place {
        Some(id) => format!("settings:computers:{id}").into(),
        None => "settings:computers".into(),
    })
}

/// A computer's notice opens Settings on that computer.
fn open_computer(place: Option<String>) -> Action {
    Action::new(OPEN, move |_, cx| navigate(computer_route(place.as_deref()), cx))
}

/// A setup's notice opens Add a computer on its running steps.
fn open_setup(place: &str) -> Action {
    add_computer::open_setup(place.to_owned())
}

fn open_usage() -> Action {
    Action::new(OPEN, |_, cx| navigate(Route::Screen("settings:usage".into()), cx))
}

fn open_thread(workspace_id: String, thread_id: String) -> Action {
    Action::new(OPEN, move |_, cx| navigate(Route::Thread { workspace_id: workspace_id.clone(), thread_id: thread_id.clone() }, cx))
}

/// The thread a session event is about, by the key the shell opens it by.
fn thread_key(e: &Value) -> Option<String> {
    owned(e, "threadId").or_else(|| owned(e, "sessionId"))
}

/// `threadOnScreen`: that thread open, Settings not over it.
fn thread_on_screen(e: &Value, window: &Window, cx: &gpui_kit::App) -> bool {
    match route(window, cx) {
        Some(Route::Thread { workspace_id, thread_id }) => {
            s(e, "workspaceId") == Some(workspace_id.as_str()) && thread_key(e) == Some(thread_id)
        }
        _ => false,
    }
}

fn workspace_on_screen(workspace: &str, window: &Window, cx: &gpui_kit::App) -> bool {
    matches!(route(window, cx), Some(Route::Thread { workspace_id, .. }) if workspace_id == workspace)
}

/// Settings' argument while it is open.
fn settings_at(window: &Window, cx: &gpui_kit::App) -> Option<String> {
    match route(window, cx) {
        Some(Route::Screen(name)) => match split(&name) {
            ("settings", arg) => Some(arg.unwrap_or_default().to_owned()),
            _ => None,
        },
        _ => None,
    }
}

/// The computer's own page or the Computers list is in front.
fn computer_on_screen(place: Option<&str>, window: &Window, cx: &gpui_kit::App) -> bool {
    place.is_some_and(|p| add_computer::stands_on(p, cx))
        || settings_at(window, cx).is_some_and(|at| at == "computers" || place.is_some_and(|p| at == format!("computers:{p}")))
}

/// The page of the computer a build runs on, which draws its rows.
fn build_on_screen(place: Option<&str>, window: &Window, cx: &gpui_kit::App) -> bool {
    place.is_some_and(|p| settings_at(window, cx).is_some_and(|at| at == format!("computers:{p}")))
}

fn place_of<'a>(places: &'a [Value], id: Option<&str>) -> Option<&'a Value> {
    places.iter().find(|p| id.is_some() && s(p, "id") == id)
}

/// `copyName`: a copy beside a folder by its own name, the workspace that is a computer itself by that computer's.
pub(crate) fn copy_name(w: &Value, places: &[Value]) -> Option<String> {
    if s(w, "kind") != Some("local") || w.get("worktree").is_some_and(|t| !t.is_null()) {
        return owned(w, "name");
    }
    let at = s(w, "place").and_then(|p| place_of(places, Some(p))).or_else(|| place_of(places, Some("here"))).or(places.first());
    Some(at.map(place_name).unwrap_or_default())
}

fn ask_key(e: &Value) -> String {
    let workspace = s(e, "workspaceId").unwrap_or_default();
    format!("ask:{workspace}:{}:{}", s(e, "threadId").unwrap_or(workspace), s(e, "askId").unwrap_or_default())
}

fn wait_key(place: &str, row: &str) -> String {
    format!("sign-in:{place}/{row}")
}

fn add(n: &mut Notices, notice: Notice, cx: &mut Context<Notices>) {
    n.add(notice, cx);
}

fn say_need(n: &mut Notices, what: String, at: Option<String>, cx: &mut Context<Notices>) {
    let notice = Notice::waiting(words::init_needs_you(&what)).key(NEED_KEY).action(open_computer(at));
    n.rules.need = Some(what);
    add(n, notice, cx);
}

fn end_wait(n: &mut Notices, key: &str, cx: &mut Context<Notices>) {
    n.rules.waits.remove(key);
    n.end(key, cx);
}

/// The build's page came into view or left it: a need it carried is said once it is gone, and ended once it is back.
pub fn settle_shown(n: &mut Notices, window: &mut Window, cx: &mut Context<Notices>) {
    let shown = build_on_screen(n.rules.job_place().as_deref(), window, cx);
    if shown == n.rules.shown {
        return;
    }
    n.rules.shown = shown;
    if shown {
        if n.rules.need.take().is_some() {
            n.end(NEED_KEY, cx);
        }
    } else if let (Some(what), None) = (n.rules.need_what(), &n.rules.need) {
        say_need(n, what, n.rules.job_place(), cx);
    }
}

/// The job as read now: a need it no longer carries is over.
fn take_job(n: &mut Notices, job: Option<Value>, window: &mut Window, cx: &mut Context<Notices>) {
    n.rules.job = job.filter(|j| !j.is_null());
    if n.rules.need.is_some() && n.rules.need_what() != n.rules.need {
        n.rules.need = None;
        n.end(NEED_KEY, cx);
    }
    settle_shown(n, window, cx);
    n.mark(window, cx);
}

/// The computers as the sidebar now lists them: a wait or a ready setup said once the list shows how it stands, as the
/// web's store subscription settles them.
pub(super) fn take_places(n: &mut Notices, places: Vec<Value>, cx: &mut Context<Notices>) {
    n.rules.places = places;
    settle_waits(n, cx);
    settle_ready(n, cx);
}

/// A wait said before that left its computer's record, the person signed in or the run moved on, goes.
fn settle_waits(n: &mut Notices, cx: &mut Context<Notices>) {
    let gone: Vec<String> = n
        .rules
        .waits
        .iter()
        .filter(|(key, (place, run))| {
            let Some(setup) = place_of(&n.rules.places, Some(place)).and_then(|p| p.get("setup")) else { return true };
            if s(setup, "addId") != Some(run.as_str()) {
                return false;
            }
            let rows = setup.get("waiting").and_then(Value::as_array).cloned().unwrap_or_default();
            !rows.iter().any(|w| wait_key(place, s(w, "row").unwrap_or_default()) == **key && s(w, "state") == Some("waiting"))
        })
        .map(|(key, _)| key.clone())
        .collect();
    for key in gone {
        end_wait(n, &key, cx);
    }
}

/// A setup that ended ready is said once the list shows its run done with nothing waiting (`sayEnded`).
fn settle_ready(n: &mut Notices, cx: &mut Context<Notices>) {
    let ready: Vec<(String, String)> = n.rules.ready.iter().map(|(p, r)| (p.clone(), r.clone())).collect();
    for (place_id, run) in ready {
        let Some(place) = place_of(&n.rules.places, Some(&place_id)).cloned() else {
            n.rules.ready.remove(&place_id);
            continue;
        };
        let setup = place.get("setup").cloned().unwrap_or_default();
        if s(&setup, "addId") == Some(run.as_str()) && s(&setup, "state") == Some("running") {
            continue;
        }
        n.rules.ready.remove(&place_id);
        let done = s(&setup, "addId") == Some(run.as_str())
            && s(&setup, "state") == Some("done")
            && setup.get("waiting").and_then(Value::as_array).is_none_or(Vec::is_empty);
        // `!(flow.open && flow.placeId === placeId)`: only Add a computer in front of it says it instead.
        if !done || add_computer::stands_on(&place_id, cx) {
            continue;
        }
        add_computer::say_ended(&place, cx);
    }
}

fn say_sealed(n: &mut Notices, job: &Value, cx: &mut Context<Notices>) {
    let notice = add_cloud::sealed(job, &n.rules.places);
    add(n, notice, cx);
}

/// One rule per event type; a type with no rule says nothing.
fn apply(n: &mut Notices, heard: Heard, window: &mut Window, cx: &mut Context<Notices>) {
    let Heard { kind, frame: e, preferences } = heard;
    // Read at the event, as the web's rules read its store; the sidebar follows the host's lists.
    let workspaces = match n.sidebar.as_ref().and_then(|s| s.upgrade()) {
        Some(sidebar) => {
            let around = sidebar.read(cx).around();
            n.rules.places = around.places.clone();
            around.workspaces.values().cloned().collect()
        }
        None => Vec::new(),
    };
    let places = n.rules.places.clone();
    let named = |id: &str| workspaces.iter().find(|w| s(w, "id") == Some(id)).and_then(|w| copy_name(w, &places));
    match kind.as_str() {
        "init.read" => take_job(n, e.get("job").cloned(), window, cx),
        "place.stage" => {
            let state = s(&e, "state").unwrap_or_default();
            let at = s(&e, "placeId");
            if state == "running" || (state == "done" && s(&e, "step") != Some("join")) || computer_on_screen(at, window, cx) {
                return;
            }
            let name = place_of(&places, at).map(place_name);
            let action = open_computer(at.map(str::to_owned));
            if state == "failed" {
                let said = s(&e, "note").or(s(&e, "step")).unwrap_or_default();
                let notice = Notice::error(words::not_added(said)).action(action);
                add(n, if let Some(name) = name { notice.place(name) } else { notice }, cx);
            } else if let Some(name) = name {
                add(n, Notice::done(words::joined(&name)).place(name).action(action), cx);
            }
        }
        "place.setup" => {
            let Some(at) = owned(&e, "placeId") else { return };
            let run = owned(&e, "addId").unwrap_or_default();
            let wait = e.get("wait").filter(|w| !w.is_null());
            let end = s(&e, "end");
            if let Some(w) = wait.filter(|w| s(w, "state") == Some("expired")) {
                end_wait(n, &wait_key(&at, s(w, "row").unwrap_or_default()), cx);
            }
            if end.is_some() {
                let keys: Vec<String> = n.rules.waits.iter().filter(|(_, (p, _))| *p == at).map(|(k, _)| k.clone()).collect();
                for key in keys {
                    end_wait(n, &key, cx);
                }
            }
            if end.is_none() && wait.and_then(|w| s(w, "state")) != Some("waiting") {
                return;
            }
            let Some(place) = place_of(&places, Some(&at)).cloned() else { return };
            let name = place_name(&place);
            let shows = computer_on_screen(Some(&at), window, cx);
            let said = s(&e, "said").unwrap_or_default();
            match (end, wait) {
                (None, Some(w)) => {
                    let key = wait_key(&at, s(w, "row").unwrap_or_default());
                    if !shows {
                        let text = words::setup_needs_you(&name, &words::wait_line(w));
                        add(n, Notice::waiting(text).key(key.clone()).place(name).action(open_setup(&at)), cx);
                    }
                    n.rules.waits.entry(key).or_insert((at, run));
                }
                (Some("failed"), _) if !shows => {
                    add(n, Notice::error(words::not_set_up(&name, said)).place(name).action(open_setup(&at)), cx)
                }
                (Some("needs-you"), _) => {
                    let setup = place.get("setup").filter(|r| s(r, "addId") == Some(run.as_str()));
                    let waiting = setup.and_then(|r| r.get("waiting")).and_then(Value::as_array).is_some_and(|w| !w.is_empty());
                    if setup.is_none() || waiting {
                        if !shows {
                            add(n, Notice::note(words::setup_needs_you(&name, said)).place(name).action(open_setup(&at)), cx);
                        }
                    } else {
                        n.rules.ready.insert(at, run);
                    }
                }
                (Some("failed"), _) => {}
                _ => {
                    n.rules.ready.insert(at, run);
                }
            }
        }
        "session.done" => {
            if let Some(turn) = owned(&e, "turnId") {
                push_held(&mut n.rules.results, (turn, e.get("result").cloned().unwrap_or_default()));
            }
        }
        "session.end" => {
            let workspace = s(&e, "workspaceId").unwrap_or_default();
            let prefix = format!("ask:{workspace}:{}:", s(&e, "threadId").unwrap_or(workspace));
            n.end_prefixed(&prefix, cx);
            let turn = s(&e, "turnId");
            let result = turn
                .and_then(|t| n.rules.results.iter().position(|(id, _)| id == t))
                .and_then(|at| n.rules.results.remove(at))
                .map(|(_, r)| r);
            let exit = e.get("exitCode").and_then(Value::as_i64);
            let interrupted = result.as_ref().is_some_and(|r| s(r, "status") == Some("interrupted"));
            let reason = e.get("reason").is_some_and(|r| !r.is_null());
            let saw = e.get("sawResult").and_then(Value::as_bool).unwrap_or(false);
            if exit == Some(0) || saw || reason || interrupted || thread_on_screen(&e, window, cx) {
                return;
            }
            let said = result.as_ref().and_then(|r| owned(r, "error")).or_else(|| words::exit_line(exit));
            let title = title_of(n, s(&e, "threadId"), cx);
            let mut notice = Notice::error(words::thread_stopped(&title, said.as_deref()));
            if let Some(name) = named(workspace) {
                notice = notice.place(name);
            }
            if let Some(thread) = thread_key(&e) {
                notice = notice.action(open_thread(workspace.to_owned(), thread));
            }
            add(n, notice, cx);
        }
        "session.permission" => {
            if thread_on_screen(&e, window, cx) {
                return;
            }
            let workspace = s(&e, "workspaceId").unwrap_or_default();
            let text = words::asking_line(s(&e, "toolName").unwrap_or_default(), s(&e, "input").unwrap_or_default(), s(&e, "detail"));
            let mut notice = Notice::waiting(text).key(ask_key(&e));
            if let Some(name) = named(workspace) {
                notice = notice.place(name);
            }
            if let Some(thread) = thread_key(&e) {
                notice = notice.action(open_thread(workspace.to_owned(), thread));
            }
            add(n, notice, cx);
        }
        "session.permission.closed" => n.end(&ask_key(&e), cx),
        "session.notify" => {
            if s(&e, "notify") != Some(words::NOTIFY_ME) || thread_on_screen(&e, window, cx) {
                return;
            }
            let workspace = s(&e, "workspaceId").unwrap_or_default();
            let mut notice = Notice::note(s(&e, "text").unwrap_or_default().to_owned());
            if let Some(name) = named(workspace) {
                notice = notice.place(name);
            }
            if let Some(thread) = thread_key(&e) {
                notice = notice.action(open_thread(workspace.to_owned(), thread));
            }
            add(n, notice, cx);
        }
        "job.needs-you" => {
            let need = e.get("needsYou").cloned().unwrap_or_default();
            let mut job = n.rules.job.clone().unwrap_or_else(|| json!({}));
            job["needsYou"] = need.clone();
            n.rules.job = Some(job);
            n.mark(window, cx);
            let at = n.rules.job_place();
            if build_on_screen(at.as_deref(), window, cx) {
                return;
            }
            say_need(n, s(&need, "what").unwrap_or_default().to_owned(), at, cx);
        }
        "init.job" => {
            let job = e.get("job").cloned().unwrap_or_default();
            take_job(n, Some(job.clone()), window, cx);
            let (Some(id), Some(phase)) = (owned(&job, "id"), s(&job, "phase")) else { return };
            if (phase != "done" && phase != "failed") || n.rules.jobs_ended.contains(&id) {
                return;
            }
            push_held(&mut n.rules.jobs_ended, id);
            let at = job.pointer("/place/id").and_then(Value::as_str).map(str::to_owned);
            if build_on_screen(at.as_deref(), window, cx) || at.as_deref().is_some_and(|p| add_cloud::build_on_screen(p, cx)) {
                return;
            }
            if phase == "failed" {
                add(n, Notice::error(words::image_not_built(s(&job, "error"))).action(open_computer(at)), cx);
            } else {
                say_sealed(n, &job, cx);
            }
        }
        "workspace.gone" => {
            let workspace = s(&e, "workspaceId").unwrap_or_default();
            let Some(name) = named(workspace) else { return };
            if workspace_on_screen(workspace, window, cx) {
                return;
            }
            let mut notice = Notice::error(words::gone(&name, s(&e, "reason").unwrap_or_default())).place(name);
            let thread = n
                .sidebar
                .as_ref()
                .and_then(|s| s.upgrade())
                .and_then(|side| side.read(cx).threads().iter().find(|t| t.workspace_id == workspace).map(|t| t.key.clone()));
            if let Some(thread) = thread {
                notice = notice.action(open_thread(workspace.to_owned(), thread));
            }
            add(n, notice, cx);
        }
        "usage.alert" => {
            if preferences.as_ref().and_then(|p| p.get("planAlerts")).and_then(Value::as_bool) == Some(false) {
                return;
            }
            let alert = e.get("alert").cloned().unwrap_or_default();
            let kind = if s(&alert, "kind") == Some("back") { Kind::Done } else { Kind::Note };
            add(n, Notice::new(kind, words::plan_alert(s(&e, "label").unwrap_or_default(), &alert)).action(open_usage()), cx);
        }
        "host.notice" => add(n, Notice::note(s(&e, "message").unwrap_or_default().to_owned()), cx),
        _ => {}
    }
    settle_waits(n, cx);
    settle_ready(n, cx);
}

/// `threadTitle`: the thread's title as the sidebar folds it.
fn title_of(n: &Notices, thread: Option<&str>, cx: &gpui_kit::App) -> String {
    thread
        .and_then(|t| n.sidebar.as_ref()?.upgrade()?.read(cx).title_of(t))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| words::A_THREAD.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_local_workspace_goes_by_its_computer_and_a_copy_by_its_own_name() {
        let places = vec![json!({ "id": "here", "name": "boat", "label": "Boat", "kind": "place" })];
        let workspaces = [
            json!({ "id": "a", "kind": "local", "name": "repo" }),
            json!({ "id": "b", "kind": "local", "name": "repo-fix", "worktree": { "branch": "fix" } }),
        ];
        assert_eq!(copy_name(&workspaces[0], &places).as_deref(), Some("Boat"));
        assert_eq!(copy_name(&workspaces[1], &places).as_deref(), Some("repo-fix"));
    }

    #[test]
    fn an_ask_is_keyed_by_its_thread() {
        let e = json!({ "workspaceId": "w", "threadId": "t", "askId": "a" });
        assert_eq!(ask_key(&e), "ask:w:t:a");
        assert_eq!(ask_key(&json!({ "workspaceId": "w", "askId": "a" })), "ask:w:w:a");
    }
}
