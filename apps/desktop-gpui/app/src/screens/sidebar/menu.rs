// SPDX-License-Identifier: AGPL-3.0-only
//! A tile's context menu as the web app builds it (`actions/threadActions.ts`, `actions/workspaceActions.ts`,
//! `sidebar/WorkspaceSidebar.tsx` tileItem) and draws it in a page (`actions/ContextMenuHost.tsx`): the thread's own
//! items, then its workspace's, a rule wherever the group changes, a refused item held with its reason.

use gpui_kit::{ClipboardItem, Context, Keystroke, Pixels, Point, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::thread::History;
use wsp_host_client::protocol::{
    SessionsHistoryRequest, SessionsRewindRequest, WorkspacesDeleteRequest, WorkspacesNapRequest, WorkspacesRebuildRequest,
    WorkspacesRestartDaemonRequest, WorkspacesStopWakeRequest, WorkspacesWakeRequest,
};

use super::model::{self, Shape, Tile};
use super::{dialogs, host, Asked, Processes, Sidebar};
use crate::ui::context_menu::MenuItem;

/// What a menu item does.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Stop,
    Settle,
    Restore,
    Rename,
    UndoRewind,
    CopyMarkdown,
    Pin(bool),
    Snooze,
    Move(model::Move),
    CopyLink,
    ContinueInTerminal,
    StopAndContinue,
    OpenThread(String),
    /// The child Settle or Restore of a tile under a lead.
    SettleChild,
    RestoreChild,
    SettleFinished(Vec<String>),
    KeepThis(Vec<String>),
    DeleteCopies(Vec<String>),
    Delete,
    Forget,
    Phase,
    Rebuild,
    Reconnect,
    NewThreadHere,
    OpenTerminal,
    OpenBrowser,
    BringBack,
    ExportProject,
    RenameTask,
    Fork,
    CopyComputerId(String),
    DeleteTask,
    ForgetTask,
    /// A failed creation's one act: its row goes, and the runtime's hold on it.
    DismissCreation,
}

/// What a menu was opened on.
#[derive(Clone, Debug, PartialEq)]
pub enum On {
    Thread(String),
    Workspace(String),
    Creation(String),
}

#[derive(Clone, Debug)]
pub struct Item {
    pub act: Act,
    pub label: SharedString,
    pub group: &'static str,
    pub refusal: Option<String>,
    pub keys: Option<Keystroke>,
    pub destructive: bool,
}

fn item(act: Act, label: impl Into<SharedString>, group: &'static str, refusal: Option<String>) -> Item {
    Item { act, label: label.into(), group, refusal, keys: None, destructive: false }
}

/// A chord as GPUI reads one, `secondary` being ⌘ on the Mac and Ctrl elsewhere, as `keybindings.ts` binds it.
fn chord(keys: &str) -> Option<Keystroke> {
    Keystroke::parse(keys).ok()
}

/// `shellLine`: a word bare where it is safe, else in single quotes.
fn shell_word(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)) {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// The line that goes on with a thread in a terminal (`views/harness.ts` terminalResumeLine), for an agent whose
/// catalog on the thread's workspace names a `terminalResume`.
pub fn terminal_line(s: &Sidebar, t: &model::Thread) -> Option<String> {
    let command = s.around.catalogs.get(&t.workspace_id)?.iter().find(|c| c.harness == t.harness)?.terminal_resume.as_deref()?;
    let (folder, id) = (t.cwd.as_deref()?, t.claude_session_id.as_deref()?);
    Some(format!("cd {} && {command} {}", shell_word(folder), shell_word(id)))
}

/// `threadRenameRefusal`: an agent whose catalog says a name is not kept, else a machine that is gone.
pub(crate) fn rename_refusal(s: &Sidebar, t: &model::Thread) -> Option<String> {
    let catalog = s.around.catalogs.get(&t.workspace_id).and_then(|c| c.iter().find(|c| c.harness == t.harness));
    if catalog.is_some_and(|c| !c.keeps_rename) {
        return Some(format!("Rename in {} is not kept", crate::ui::catalog::name(&t.harness)));
    }
    let w = s.around.workspaces.get(&t.workspace_id)?;
    (workspace_state(w, s.around.statuses.get(&t.workspace_id)) == "gone").then(|| gone_refusal("rename"))
}

/// A workspace's one state word (`workspace-state.ts` workspaceState), off its record and its live status.
pub(crate) fn workspace_state(w: &Value, status: Option<&Value>) -> &'static str {
    let machine = status.and_then(|s| s.get("machineState")?.as_str());
    let reach = status.and_then(|s| s.pointer("/reach/state")?.as_str());
    let phase = status.and_then(|s| s.get("phase")?.as_str()).or_else(|| w.get("phase")?.as_str()).unwrap_or("running");
    if phase == "released" {
        return "released";
    }
    if machine == Some("gone") {
        return "gone";
    }
    match phase {
        "pausing" => "pausing",
        "napping" => "paused",
        "waking" => "waking",
        "gone" => "gone",
        "queued" => "waiting",
        _ if machine == Some("paused") => "paused",
        _ if machine == Some("starting") => "waking",
        _ if matches!(reach, Some("unreachable" | "no-daemon" | "zombie")) => "unreachable",
        _ => "running",
    }
}

/// `actionRefusal` in `workspace-state.ts`, for the states a workspace in this list can be in.
fn action_refusal(state: &str, action: &str) -> Option<String> {
    match state {
        "running" => None,
        "pausing" | "paused" => Some(format!("Workspace is {state}; wake it to {action}")),
        "waking" => Some(format!("Workspace is waking; {action}s open when it is running")),
        "unreachable" => Some(format!("Workspace is unreachable; {action}s open when the machine answers")),
        "waiting" => Some(format!("Workspace is waiting for a machine; {action}s open when it is running")),
        "released" => Some(format!("This workspace's machine was deleted once its sub-thread was done, so there is none to {action} on")),
        _ => Some(gone_refusal(action)),
    }
}

/// `phaseRefusal`: why the pause or the wake cannot run in this state.
fn phase_refusal(state: &str, why: Option<&str>) -> Option<String> {
    match state {
        "pausing" => Some("Task is pausing; it can be woken once it is paused".to_owned()),
        "gone" => Some(gone_refusal("wake")),
        "waiting" if why == Some("minute") => {
            Some("Task is waiting for its cloud's next start; it starts on its own when one is free".to_owned())
        }
        "waiting" => Some("Task is waiting for a free computer on its cloud; it starts on its own when one frees".to_owned()),
        "released" => Some("This sub-thread's computer was deleted once it was done".to_owned()),
        _ => None,
    }
}

/// `threadForgetRefusal`: only a thread no turn ever ran on can be forgotten.
fn forget_refusal(t: &model::Thread) -> Option<String> {
    let id = t.thread_id.as_deref().filter(|_| t.ran)?;
    Some(format!(
        "thread {} has a turn that ran; only a thread no turn ever ran on can be forgotten",
        id.chars().take(8).collect::<String>()
    ))
}

const NO_WORKSPACE_FORK: &str =
    "Running a copy of a task is not in the runtime yet; make a second task of the same project from the plus on its row";

pub(crate) fn gone_refusal(action: &str) -> String {
    format!(
        "This workspace's machine is gone with its disk, so work that was not pushed is lost; rebuild it to {action}, which brings back \
         its home folder from the last saved nap"
    )
}

/// Why a workspace's computer is not there (`settings/places.ts` absenceOf): this computer's daemon stopped, or
/// another computer not answering; with the word of the button that starts the daemon again where there is one.
fn absence(s: &Sidebar, w: &Value, status: Option<&Value>) -> Option<(String, bool)> {
    if w.get("kind").and_then(Value::as_str) == Some("local") {
        let silent = matches!(status.and_then(|s| s.pointer("/reach/state")?.as_str()), Some("unreachable" | "no-daemon"));
        let here = s.around.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some("here"));
        let name = here
            .map(super::looks::place_name)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| w.get("name").and_then(Value::as_str).unwrap_or_default().to_owned());
        return silent.then(|| (format!("{name}'s terminals and files stopped"), true));
    }
    let id = s.place_of(w.get("id")?.as_str()?)?;
    let place = s.around.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id.as_str()))?;
    (place.get("present").and_then(Value::as_bool) == Some(false))
        .then(|| (format!("{} is not answering; it connects on its own when it is on", super::looks::place_name(place)), false))
}

/// A workspace's own items (`workspaceActions.ts`): each its kind and state can take, a refused one with its reason.
pub(crate) fn workspace_items(s: &Sidebar, workspace_id: &str) -> Vec<Item> {
    let Some(w) = s.around.workspaces.get(workspace_id) else { return Vec::new() };
    let status = s.around.statuses.get(workspace_id);
    let read = |key: &str| status.and_then(|s| s.get(key)?.as_str()).or_else(|| w.get(key)?.as_str());
    let kind = w.get("kind").and_then(Value::as_str).unwrap_or("cloud");
    let state = workspace_state(w, status);
    let machine = read("machineId").unwrap_or_default().to_owned();
    let delete = Item { destructive: true, ..item(Act::DeleteTask, "Delete task", "remove", None) };
    // A create that failed before any machine stood has its delete alone (`startFailed`).
    if read("phase") == Some("gone") && machine.is_empty() {
        return vec![delete];
    }
    let zombie = status.and_then(|s| s.pointer("/reach/state")?.as_str()) == Some("zombie");
    let dead = state == "gone" || zombie || read("wakeRefused").is_some();
    let driven = kind == "cloud";
    let absent = absence(s, w, status);
    let name = w.get("name").and_then(Value::as_str).unwrap_or_default();
    let mut out = Vec::new();
    if driven {
        let label = match state {
            "waking" => "Stop waking".to_owned(),
            "running" | "unreachable" => format!("Pause {name}"),
            _ => format!("Wake {name}"),
        };
        let why = status.and_then(|s| s.pointer("/queue/why")).or_else(|| w.pointer("/queue/why")).and_then(Value::as_str);
        out.push(item(Act::Phase, label, "state", phase_refusal(state, why)));
        if dead {
            out.push(item(Act::Rebuild, "Rebuild task", "state", None));
        }
    }
    if absent.as_ref().is_some_and(|(_, start)| *start) {
        out.push(item(Act::Reconnect, "Reconnect", "state", None));
    }
    out.push(item(Act::NewThreadHere, "New thread here", "open", dead.then(|| "New threads wait for the rebuild".to_owned())));
    out.push(Item {
        keys: chord("secondary-j"),
        ..item(Act::OpenTerminal, "Open terminal", "open", (state == "gone").then(|| gone_refusal("open a terminal")))
    });
    let sentence = absent.as_ref().map(|(said, _)| said.clone());
    out.push(Item {
        keys: chord("secondary-shift-j"),
        ..item(Act::OpenBrowser, "Open browser", "open", sentence.clone().or_else(|| action_refusal(state, "preview")))
    });
    if kind != "place" {
        let away = action_refusal(state, if matches!(state, "waking" | "unreachable" | "waiting") { "git action" } else { "push" });
        out.push(item(Act::BringBack, "Bring back", "project", sentence.or(away)));
        out.push(item(Act::ExportProject, "Export project", "project", dead.then(|| "Projects wait for the rebuild".to_owned())));
    }
    out.push(item(Act::RenameTask, "Rename task", "edit", None));
    if driven {
        out.push(item(Act::Fork, "Run a copy", "edit", Some(NO_WORKSPACE_FORK.to_owned())));
    }
    out.push(item(Act::CopyComputerId(machine), "Copy computer id", "copy", None));
    out.push(if state == "gone" { Item { destructive: true, ..item(Act::ForgetTask, "Forget task", "remove", None) } } else { delete });
    out
}

/// Every item a tile's menu holds, in the web's order.
pub fn items_for(s: &Sidebar, tile: &Tile, key: &str) -> Vec<Item> {
    let t = &s.threads[tile.thread];
    let settled = tile.shape == Shape::Settled;
    let is_root = tile.moves || settled && t.parent.is_none() || tile.inbox && tile.started_by.is_empty();
    let working = s.tree_works(key);
    let running = t.status == model::Status::Running;
    let in_folder = s.in_folder(t);
    let place = if tile.moves { s.root_place(key) } else { None };
    let mut out = Vec::new();

    let stop_refusal = (!running && !(is_root && working)).then(|| "Thread is not running".to_owned());
    out.push(item(Act::Stop, "Stop thread", "state", stop_refusal));
    if is_root && !settled {
        out.push(Item {
            keys: chord("secondary-shift-e"),
            ..item(Act::Settle, "Settle thread", "state", working.then(|| "A thread in it is still working".to_owned()))
        });
    }
    if is_root && settled {
        out.push(item(Act::Restore, "Restore thread", "state", None));
    }
    out.push(item(Act::Rename, "Rename thread", "edit", rename_refusal(s, t)));
    if t.rewound_at.is_some() && t.thread_id.is_some() {
        out.push(item(Act::UndoRewind, "Undo rewind", "edit", None));
    }
    let no_id = t.thread_id.is_none().then(|| "This thread has no id yet".to_owned());
    out.push(item(Act::CopyMarkdown, "Copy as Markdown", "edit", no_id.clone()));
    if is_root && !settled {
        let pinned = t.pinned_at.is_some();
        out.push(item(Act::Pin(!pinned), if pinned { "Unpin thread" } else { "Pin thread" }, "place", None));
        out.push(item(Act::Snooze, "Snooze thread", "place", None));
    }
    if let Some(place) = place {
        let first = (place.index == 0).then(|| "Already first".to_owned());
        let last = (place.index + 1 >= place.count).then(|| "Already last".to_owned());
        out.push(Item { keys: chord("alt-shift-up"), ..item(Act::Move(model::Move::Up), "Move up", "place", first.clone()) });
        out.push(Item { keys: chord("alt-shift-down"), ..item(Act::Move(model::Move::Down), "Move down", "place", last) });
        out.push(item(Act::Move(model::Move::Top), "Move to top", "place", first));
    }
    out.push(item(Act::CopyLink, "Copy thread link", "copy", no_id.clone()));
    if terminal_line(s, t).is_some() {
        let busy = running
            .then(|| "wsp is still running this thread. Stop it first, or wsp and the terminal both write to one conversation".to_owned());
        out.push(item(Act::ContinueInTerminal, "Continue in terminal", "copy", busy));
        if running {
            out.push(item(Act::StopAndContinue, "Stop and continue in terminal", "copy", None));
        }
    }
    let others = s.attempt_others(key);
    if !others.is_empty() {
        out.push(Item { destructive: true, ..item(Act::KeepThis(others), "Keep this one", "remove", None) });
    }
    if is_root && settled {
        let copies = s.tree_workspaces(key);
        out.push(Item { destructive: true, ..item(Act::DeleteCopies(copies), "Delete copies", "remove", None) });
    }
    if in_folder {
        let refusal = no_id.clone().or_else(|| running.then(|| "this thread is working; stop its turn first".to_owned()));
        out.push(Item { destructive: true, ..item(Act::Delete, "Delete thread", "remove", refusal) });
    } else {
        out.push(Item { destructive: true, ..item(Act::Forget, "Forget thread", "remove", no_id.or_else(|| forget_refusal(t))) });
    }
    if let Some(replaced) = &t.replaces {
        out.push(item(Act::OpenThread(replaced.clone()), "Open the thread it replaced", "open", None));
    }
    if let Some(restart) = &t.replaced_by {
        out.push(item(Act::OpenThread(restart.clone()), "Open its restart", "open", None));
    }
    if !is_root {
        if tile.shape == Shape::Finished || !settled && !running {
            out.push(item(Act::SettleChild, "Settle thread", "state", None));
        }
        if t.settled_at.is_some() {
            out.push(item(Act::RestoreChild, "Restore thread", "state", None));
        }
    }
    let finished = s.view().finished_under(key);
    if !finished.is_empty() {
        out.push(item(Act::SettleFinished(finished.clone()), format!("Settle {} finished", finished.len()), "state", None));
    }
    // An inbox tile is drawn with no lead above it, so it carries its copy's items as a root does (`tileItem`).
    let above = t.parent.as_deref().and_then(|p| s.thread(p)).map(|p| p.workspace_id.clone()).filter(|_| !tile.inbox);
    if above.as_deref() != Some(t.workspace_id.as_str()) {
        out.extend(workspace_items(s, &t.workspace_id).into_iter().filter(|i| !(in_folder && matches!(i.group, "edit" | "remove"))));
    }
    out
}

impl Sidebar {
    /// Opens a tile's menu where the pointer pressed, read off the tile as it is drawn now.
    pub(crate) fn open_menu(&mut self, key: &str, inbox: bool, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let tile = self.lines.iter().find_map(|l| match &l.item {
            model::Item::Tile(t) if t.inbox == inbox && self.threads[t.thread].key == key => Some(t.clone()),
            _ => None,
        });
        let Some(tile) = tile else { return };
        let items = items_for(self, &tile, key);
        self.show_menu(On::Thread(key.to_owned()), items, at, window, cx);
    }

    /// A tile with no thread behind it: a workspace's own items, or a failed creation's Delete.
    pub(crate) fn open_other_menu(&mut self, on: On, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        let items = match &on {
            On::Workspace(id) => workspace_items(self, id),
            On::Creation(_) => vec![Item { destructive: true, ..item(Act::DismissCreation, "Delete", "remove", None) }],
            On::Thread(_) => return,
        };
        self.show_menu(on, items, at, window, cx);
    }

    fn show_menu(&mut self, on: On, items: Vec<Item>, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.card = None;
        let rows = items
            .iter()
            .map(|it| MenuItem {
                label: it.label.clone(),
                group: it.group,
                refusal: it.refusal.clone().map(SharedString::from),
                keys: it.keys.clone(),
                destructive: it.destructive,
            })
            .collect();
        self.menu.open(at, rows, window, cx, move |this, pick, window, cx| {
            let Some(act) = pick.and_then(|ix| items.get(ix)).map(|it| it.act.clone()) else { return };
            match &on {
                On::Thread(key) => this.run(key, act, window, cx),
                On::Workspace(id) => this.run_workspace(id, None, act, window, cx),
                On::Creation(key) => this.dismiss_creation(key, cx),
            }
        });
    }

    /// Runs one item's act on the thread it was opened on.
    pub(crate) fn run(&mut self, key: &str, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let Some(t) = self.thread(key).cloned() else { return };
        match act {
            Act::Stop => self.stop(key, cx),
            Act::Settle => self.settle(key, cx),
            Act::Restore => self.restore(key, cx),
            Act::SettleChild => self.settle_ids(vec![key.to_owned()], Processes::Ask, cx),
            Act::RestoreChild => self.restore_ids(vec![key.to_owned()], cx),
            Act::SettleFinished(ids) => self.settle_ids(ids, Processes::Ask, cx),
            Act::Rename => self.open_rename(key, window, cx),
            Act::UndoRewind => {
                let Some(thread) = t.thread_id.clone() else { return };
                self.confirm(
                    window,
                    cx,
                    "Undo the rewind?",
                    "The turns the rewind cut come back.",
                    "Undo rewind",
                    false,
                    move |this, cx| this.write_op::<SessionsRewindRequest>(json!({ "threadId": thread, "undo": true }), cx),
                );
            }
            Act::CopyMarkdown => self.copy_markdown(&t, cx),
            Act::Pin(to) => self.mark_threads(vec![key.to_owned()], json!({ "pinned": to }), cx),
            Act::Snooze => dialogs::snooze(self, key, window, cx),
            Act::Move(to) => self.move_root(key, to, cx),
            Act::CopyLink => {
                let page = self.link.as_ref().and_then(|l| l.page.clone()).unwrap_or_else(|| "http://127.0.0.1:4400/".to_owned());
                let thread = t.thread_id.clone().unwrap_or_default();
                let link = format!("{page}#w/{}/t/{}", encode(&t.workspace_id), encode(&thread));
                cx.write_to_clipboard(ClipboardItem::new_string(link));
            }
            Act::ContinueInTerminal | Act::StopAndContinue => {
                if let Some(line) = terminal_line(self, &t) {
                    cx.write_to_clipboard(ClipboardItem::new_string(line));
                    self.refuse("Copied. Paste it in a terminal to go on with this thread there".to_owned(), cx);
                }
                if act == Act::StopAndContinue {
                    self.stop(key, cx);
                }
            }
            Act::OpenThread(other) => self.select(&other, cx),
            Act::KeepThis(workspaces) => dialogs::end_workspaces(self, workspaces, dialogs::Road::Others, cx),
            Act::DeleteCopies(workspaces) => dialogs::end_workspaces(self, workspaces, dialogs::Road::Copies, cx),
            Act::Delete => dialogs::delete(self, key, window, cx),
            Act::Forget => self.delete(key, cx),
            other => self.run_workspace(&t.workspace_id, Some(key), other, window, cx),
        }
    }

    /// A workspace's act, from its own tile or from a thread's tile on it, whose rename box takes a task's rename.
    fn run_workspace(&mut self, id: &str, thread: Option<&str>, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let id = id.to_owned();
        let phase = self.around.workspaces.get(&id).and_then(|w| w.get("phase")?.as_str()).unwrap_or("running").to_owned();
        match act {
            Act::Phase => match phase.as_str() {
                "waking" => self.write_op::<WorkspacesStopWakeRequest>(json!({ "workspaceId": id }), cx),
                "running" => self.write_op::<WorkspacesNapRequest>(json!({ "workspaceId": id }), cx),
                _ => self.write_op::<WorkspacesWakeRequest>(json!({ "workspaceId": id }), cx),
            },
            Act::Rebuild => self.write_op::<WorkspacesRebuildRequest>(json!({ "workspaceId": id }), cx),
            Act::Reconnect => self.write_op::<WorkspacesRestartDaemonRequest>(json!({ "workspaceId": id }), cx),
            Act::NewThreadHere => cx.emit(Asked::NewThreadIn(id)),
            Act::OpenTerminal => cx.emit(Asked::OpenTerminal(id)),
            Act::OpenBrowser => cx.emit(Asked::OpenBrowser(id)),
            Act::BringBack => self.bring_back(&id, cx),
            Act::ExportProject => cx.emit(Asked::ExportProject(id)),
            Act::RenameTask => match thread {
                Some(key) => self.open_rename(key, window, cx),
                None => self.open_rename(&format!("w:{id}"), window, cx),
            },
            Act::CopyComputerId(machine) => cx.write_to_clipboard(ClipboardItem::new_string(machine)),
            Act::DeleteTask => dialogs::end_workspaces(self, vec![id], dialogs::Road::Delete, cx),
            Act::ForgetTask => dialogs::end_workspaces(self, vec![id], dialogs::Road::Forget, cx),
            _ => {}
        }
    }

    /// `dismissCreation`: the row goes, and a failed create's record with it; a host that restarted since holds none.
    fn dismiss_creation(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(at) = self.around.creations.iter().position(|c| c.key == key) else { return };
        let creation = self.around.creations.remove(at);
        if let (Some(w), Some(link), true) = (creation.workspace_id, self.link.clone(), creation.failed) {
            let task = link.runtime.spawn(host::write::<WorkspacesDeleteRequest>(link.host.clone(), json!({ "workspaceId": w })));
            cx.spawn(async move |this, cx| {
                if let Ok(Err(f)) = task.await {
                    if f.kind.as_deref() != Some("not-found") {
                        _ = this.update(cx, |this, cx| this.refuse(f.message, cx));
                    }
                }
            })
            .detach();
        }
        self.rebuild(cx);
    }

    /// Reads the whole transcript and puts it on the clipboard as Markdown.
    fn copy_markdown(&mut self, t: &model::Thread, cx: &mut Context<Self>) {
        let (Some(link), Some(thread)) = (self.link.clone(), t.thread_id.clone()) else { return };
        let params = json!({ "workspaceId": t.workspace_id, "threadId": thread, "whole": true });
        let task = link.runtime.spawn(host::ask::<SessionsHistoryRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let events = match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                Ok(History::Whole { events }) => events.iter().filter_map(|e| serde_json::to_value(e).ok()).collect::<Vec<Value>>(),
                Ok(History::Page(_)) => Vec::new(),
                Err(failure) => {
                    _ = this.update(cx, |this, cx| this.refuse(failure.message, cx));
                    return;
                }
            };
            let offset = super::utc_offset_ms();
            let markdown = super::markdown::thread_markdown(&events, &thread, offset);
            _ = this.update(cx, |_, cx| cx.write_to_clipboard(ClipboardItem::new_string(markdown)));
        })
        .detach();
    }
}

/// `encodeURIComponent`, for the ids a link carries.
fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_a_turn_ran_on_is_not_forgotten() {
        let rows = |r: Value| model::fold_threads(&[model::Row::of(&r).unwrap()]);
        let worked =
            rows(json!({ "id": "s1", "workspaceId": "w", "threadId": "0123456789ab", "status": "completed", "claudeSessionId": "c" }));
        assert_eq!(
            forget_refusal(&worked[0]).as_deref(),
            Some("thread 01234567 has a turn that ran; only a thread no turn ever ran on can be forgotten")
        );
        let refused = rows(
            json!({ "id": "s2", "workspaceId": "w", "threadId": "t2", "status": "failed", "claudeSessionId": "c", "refusal": { "kind": "auth" } }),
        );
        assert_eq!(forget_refusal(&refused[0]), None);
        let never = rows(json!({ "id": "s3", "workspaceId": "w", "threadId": "t3", "status": "failed" }));
        assert_eq!(forget_refusal(&never[0]), None);
    }

    #[test]
    fn a_word_goes_bare_where_it_is_safe_and_quoted_where_not() {
        assert_eq!(shell_word("/root/wsp"), "/root/wsp");
        assert_eq!(shell_word("/tmp/my repo"), "'/tmp/my repo'");
        assert_eq!(shell_word("it's"), "'it'\\''s'");
        assert_eq!(encode("ws_1/a b"), "ws_1%2Fa%20b");
    }

    #[gpui_kit::test]
    fn a_rename_the_agent_does_not_keep_opens_no_box(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::AppContext as _;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let sidebar = cx.new(Sidebar::new);
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(sidebar.clone(), window, cx));
        let row =
            json!({ "id": "r1", "workspaceId": "w1", "harness": "opencode", "threadId": "t1", "status": "completed", "prompt": "a title" });
        sidebar.update(cx, |s, cx| {
            s.apply(host::Change::Row { id: "r1".to_owned(), row: Some(row) }, cx);
            let catalog = super::super::Catalog { harness: "opencode".to_owned(), keeps_rename: false, ..Default::default() };
            s.around.catalogs.insert("w1".to_owned(), vec![catalog]);
        });
        cx.update(|window, cx| sidebar.update(cx, |s, cx| s.open_rename("t1", window, cx)));
        assert!(sidebar.read_with(cx, |s, _| s.renaming.is_none()), "a double-click opens no box where the menu refuses the rename");
    }

    #[test]
    fn a_workspace_state_reads_its_phase_then_its_machine_and_reach() {
        let running = json!({ "phase": "running" });
        assert_eq!(workspace_state(&running, None), "running");
        assert_eq!(workspace_state(&running, Some(&json!({ "reach": { "state": "no-daemon" } }))), "unreachable");
        assert_eq!(workspace_state(&json!({ "phase": "napping" }), None), "paused");
        assert_eq!(
            action_refusal("unreachable", "preview").as_deref(),
            Some("Workspace is unreachable; previews open when the machine answers")
        );
    }
}
