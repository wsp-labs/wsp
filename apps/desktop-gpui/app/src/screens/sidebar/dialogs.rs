// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar's dialogs as the web app draws them: Snooze (`sidebar/SnoozeDialog.tsx`, `snooze.ts`), the delete
//! and the settle that ask about processes a thread still runs (`components/DeleteThreadDialog.tsx`,
//! `components/procs/ProcessesLeft.tsx`), the end of one workspace or several (`components/ForgetWorkspaceDialog.tsx`)
//! and a plain confirm. Each stands over a blurred scrim across the window.

use std::rc::Rc;

use chrono::{Datelike, Duration as Days, Local, NaiveTime, TimeZone, Weekday};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, Entity, FocusHandle, FontWeight, KeyDownEvent, SharedString,
    Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    SessionsDeleteRequest, SessionsMarkRequest, SessionsSettleRequest, WorkspacesCheckoutRequest, WorkspacesDeleteRequest,
    WorkspacesForgetRequest,
};

use super::{host, Processes, Sidebar};
use crate::ui::context_menu::ContextMenu;
use crate::ui::{Button, Palette};

/// `max-w-110`.
const WIDTH: f32 = 440.;

type OnOk = Rc<dyn Fn(&mut Sidebar, &mut Context<Sidebar>)>;

/// What a processes question finishes once it is answered.
#[derive(Clone, Debug, PartialEq)]
pub enum Ends {
    Delete(String),
    Settle(Vec<String>),
}

/// Which road out a workspaces dialog is for: one workspace's delete or forget, the copies Keep this one deletes, or
/// a settled tree's copies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Road {
    Delete,
    Forget,
    Others,
    Copies,
}

pub struct Dialog {
    pub open: OpenDialog,
    pub focus: FocusHandle,
    focused: bool,
    /// Whether a dialog of the dialogs' layer stood when this one first drew, so this one stands over it.
    over_dialog: bool,
    /// Unseen while a delete reads its copies with Ask before deleting off, so a clean delete never flashes one.
    pub shown: bool,
}

pub enum OpenDialog {
    Snooze {
        key: String,
        presets: Vec<(&'static str, i64, String)>,
        until: Entity<InputState>,
    },
    /// A delete or a settle; `left` is the host's sentence once it said processes still run, `busy` the answer on its
    /// way, `said` the host's refusal.
    Processes {
        ends: Ends,
        title: SharedString,
        line: Option<SharedString>,
        left: Option<String>,
        busy: Option<Processes>,
        said: Option<String>,
        checked: bool,
    },
    /// `holding` is each copy's line on what it holds that is not pushed, none while they are read.
    Workspaces {
        road: Road,
        ids: Vec<String>,
        title: SharedString,
        line: SharedString,
        holding: Option<Vec<String>>,
        busy: bool,
        said: Option<String>,
    },
    Confirm {
        title: SharedString,
        line: SharedString,
        ok: SharedString,
        destructive: bool,
        on_ok: OnOk,
    },
}

/// The presets `snooze.ts` offers from `now`, each with its label, its time in ms and its time as the dialog writes it.
pub fn presets(now: chrono::DateTime<Local>) -> Vec<(&'static str, i64, String)> {
    let at = |t: chrono::DateTime<Local>| t.timestamp_millis();
    let day = |d: chrono::NaiveDate, h: u32| {
        Local.from_local_datetime(&d.and_time(NaiveTime::from_hms_opt(h, 0, 0).unwrap())).earliest().unwrap()
    };
    let clock = |t: chrono::DateTime<Local>| t.format("%-I:%M %p").to_string();
    let mut out = vec![
        ("In 1 hour", at(now + Days::hours(1)), clock(now + Days::hours(1))),
        ("In 3 hours", at(now + Days::hours(3)), clock(now + Days::hours(3))),
    ];
    let evening = day(now.date_naive(), 18);
    if evening - now > Days::hours(1) {
        out.push(("This evening", at(evening), clock(evening)));
    }
    let tomorrow = day(now.date_naive() + Days::days(1), 9);
    out.push(("Tomorrow", at(tomorrow), clock(tomorrow)));
    if now.weekday() != Weekday::Sun {
        let ahead = 7 - i64::from(now.weekday().num_days_from_monday());
        let monday = day(now.date_naive() + Days::days(ahead), 9);
        out.push(("Next week", at(monday), format!("{} {}", monday.format("%a"), clock(monday))));
    }
    out
}

/// Opens Snooze on a thread; a preset snoozes it at once.
pub fn snooze(s: &mut Sidebar, key: &str, window: &mut Window, cx: &mut Context<Sidebar>) {
    let now = Local::now();
    let presets = presets(now);
    let first = Local.timestamp_millis_opt(presets[0].1).single().unwrap_or(now);
    let until = cx.new(|cx| InputState::new(window, cx).default_value(first.format("%Y-%m-%d %H:%M").to_string()));
    s.show_dialog(OpenDialog::Snooze { key: key.to_owned(), presets, until }, true, cx);
}

/// Asks before a delete; the host's check says first whether processes still run.
pub fn delete(s: &mut Sidebar, key: &str, _: &mut Window, cx: &mut Context<Sidebar>) {
    let Some(t) = s.thread(key) else { return };
    let title = SharedString::from(format!("Delete {}?", t.title));
    // The worktree wsp made goes with the last thread in it, as the host deletes.
    let w = s.around.workspaces.get(&t.workspace_id);
    let last = s.threads.iter().filter(|o| o.workspace_id == t.workspace_id).all(|o| o.key == t.key);
    let worktree = w
        .and_then(|w| w.get("worktree"))
        .filter(|tree| last && tree.get("made") == Some(&json!(true)) && tree.get("gone") != Some(&json!(true)))
        .and_then(|tree| tree.get("path")?.as_str());
    let line = SharedString::from(thread_delete_line(worktree));
    let ends = Ends::Delete(key.to_owned());
    let open = OpenDialog::Processes { ends: ends.clone(), title, line: Some(line), left: None, busy: None, said: None, checked: false };
    s.show_dialog(open, true, cx);
    let Some(link) = s.link.clone() else { return };
    let params = json!({ "threadId": key, "processes": "ask", "check": true });
    let task = link.runtime.spawn(host::write::<SessionsDeleteRequest>(link.host.clone(), params));
    cx.spawn(async move |this, cx| {
        let checked = task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string())));
        _ = this.update(cx, |this, cx| this.checked(&ends, checked, cx));
    })
    .detach();
}

/// `threadDeleteQuestion`'s second line.
fn thread_delete_line(worktree: Option<&str>) -> String {
    match worktree {
        None => "Its turns and checkpoints leave this computer; the folder it worked in is left as it is.".to_owned(),
        Some(path) => format!(
            "Its turns and checkpoints leave this computer, and the worktree at {path} with it where no other thread is in it; the branch \
             and the project folder are left as they are."
        ),
    }
}

/// `plural(n, "thread")`.
fn fmt_threads(n: usize) -> String {
    format!("{n} thread{}", if n == 1 { "" } else { "s" })
}

/// What a delete does to a copy's computer (`onDeleteOf`), in its kind's words, or the computer it stands on.
pub(super) fn on_delete(s: &Sidebar, w: &Value) -> String {
    let worktree = w.get("worktree").filter(|t| t.get("made") == Some(&json!(true))).and_then(|t| t.get("path")?.as_str());
    if let Some(path) = worktree {
        return format!("worktree at {path} is removed and the project folder is left as it is");
    }
    let joined = w
        .get("place")
        .and_then(Value::as_str)
        .and_then(|id| s.around.places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id)));
    if let Some(place) = joined.filter(|p| p.get("kind").and_then(Value::as_str) != Some("provider")) {
        return format!("copy on {} is deleted", super::looks::place_name(place));
    }
    match w.get("kind").and_then(Value::as_str).unwrap_or("cloud") {
        "cloud" => "computer is deleted in the cloud".to_owned(),
        _ => "computer is left as it is".to_owned(),
    }
}

/// `unpushedLine`: what a copy holds that its remote lacks, or that it could not be read; nothing for a clean copy.
pub fn unpushed_line(name: &str, checkout: Option<&Value>) -> Option<String> {
    let Some(c) = checkout else { return Some(format!("{name}: could not read what is not pushed")) };
    let count = |n: u64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let num = |k: &str| c.get(k).and_then(Value::as_u64).unwrap_or(0);
    let yes = |k: &str| c.get(k).and_then(Value::as_bool) == Some(true);
    let parts: Vec<String> = [
        if yes("countsUnknown") {
            Some("commits it could not count".to_owned())
        } else {
            (num("ahead") > 0).then(|| format!("{} not pushed", count(num("ahead"), "commit", "commits")))
        },
        if yes("editsUnread") {
            Some("edits it could not read".to_owned())
        } else {
            (num("changed") > 0).then(|| count(num("changed"), "uncommitted file", "uncommitted files"))
        },
        (num("stashes") > 0).then(|| count(num("stashes"), "stash", "stashes")),
    ]
    .into_iter()
    .flatten()
    .collect();
    let (last, rest) = parts.split_last()?;
    let listed = if rest.is_empty() { last.clone() } else { format!("{} and {last}", rest.join(", ")) };
    Some(format!("{name} holds {listed}"))
}

/// Opens the end of one workspace or several: a delete reads each copy's checkout first and names any that holds work.
pub fn end_workspaces(s: &mut Sidebar, ids: Vec<String>, road: Road, cx: &mut Context<Sidebar>) {
    let records: Vec<Value> = ids.iter().filter_map(|id| s.around.workspaces.get(id).cloned()).collect();
    let Some(first) = records.first() else { return };
    let name = |w: &Value| w.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
    let threads = fmt_threads(s.threads.iter().filter(|t| ids.contains(&t.workspace_id)).count());
    let word = if road == Road::Forget { "Forget" } else { "Delete" };
    let title = match road {
        _ if records.len() == 1 => format!("{word} {}?", name(first)),
        Road::Copies => format!("{word} {} copies?", records.len()),
        _ => format!("{word} the other {} copies?", records.len()),
    };
    let asked: Vec<String> = records.iter().map(|w| on_delete(s, w)).collect();
    let line = if records.len() > 1 {
        let records_go = format!("their records and {threads} leave this computer.");
        if asked.iter().all(|a| *a == asked[0]) {
            format!("Each one's {}; {records_go}", asked[0])
        } else {
            let each: Vec<String> = records.iter().zip(&asked).map(|(w, a)| format!("{}: its {a}.", name(w))).collect();
            format!("{} T{}", each.join(" "), &records_go[1..])
        }
    } else if road == Road::Forget {
        format!("Its record and {threads} leave this computer; the computer it ran on is already gone.")
    } else {
        format!("Its {}; its record and {threads} leave this computer.", asked[0])
    };
    let asks = s.around.preferences.as_ref().and_then(|p| p.get("askDelete")?.as_bool()).unwrap_or(true);
    let forget = road == Road::Forget;
    let open = OpenDialog::Workspaces {
        road,
        ids: ids.clone(),
        title: title.into(),
        line: line.into(),
        holding: forget.then(Vec::new),
        busy: false,
        said: None,
    };
    s.show_dialog(open, asks, cx);
    if forget {
        if !asks {
            s.end_now(cx);
        }
        return;
    }
    let Some(link) = s.link.clone() else { return };
    let reads: Vec<_> = records
        .iter()
        .map(|w| {
            let id = w.get("id").and_then(Value::as_str).unwrap_or_default().to_owned();
            (name(w), link.runtime.spawn(host::ask::<WorkspacesCheckoutRequest>(link.host.clone(), json!({ "workspaceId": id }))))
        })
        .collect();
    cx.spawn(async move |this, cx| {
        let mut holding = Vec::new();
        for (name, read) in reads {
            let reply = read.await.ok().and_then(Result::ok).and_then(|r| serde_json::to_value(r).ok());
            let checkout = reply.as_ref().and_then(|r| r.get("checkout")).filter(|c| !c.is_null());
            holding.extend(unpushed_line(&name, checkout));
        }
        _ = this.update(cx, |this, cx| this.read_holding(&ids, holding, asks, cx));
    })
    .detach();
}

/// A refusal that says processes still run, kept as the host's sentence; any other answer reads as none.
trait ProcessesLeft {
    fn filter_processes(self) -> Result<(), String>;
}

impl ProcessesLeft for Result<(), wsp_host_client::Failure> {
    fn filter_processes(self) -> Result<(), String> {
        match self {
            Err(f) if f.kind.as_deref() == Some("processes-left") => Err(f.message),
            _ => Ok(()),
        }
    }
}

impl Sidebar {
    /// The settle the host left for processes still running: the question, then the settle again with the answer.
    pub(crate) fn ask_settle(&mut self, ids: Vec<String>, said: String, cx: &mut Context<Self>) {
        let title = match ids.as_slice() {
            [one] => format!("Stop what {} still runs?", self.thread(one).map_or("it", |t| t.title.as_str())),
            many => format!("Stop what {} threads still run?", many.len()),
        };
        let open = OpenDialog::Processes {
            ends: Ends::Settle(ids),
            title: title.into(),
            line: None,
            left: Some(said),
            busy: None,
            said: None,
            checked: true,
        };
        self.show_dialog(open, true, cx);
    }

    pub(crate) fn show_dialog(&mut self, open: OpenDialog, shown: bool, cx: &mut Context<Self>) {
        self.menu = ContextMenu::default();
        self.dialog = Some(Dialog { open, focus: cx.focus_handle(), focused: false, over_dialog: false, shown });
        cx.notify();
    }

    /// Ends each workspace on its own, so one the host refuses leaves the rest going and its refusal stands.
    fn end_now(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        let OpenDialog::Workspaces { road, ids, busy, said, .. } = &mut dialog.open else { return };
        *busy = true;
        *said = None;
        let (road, ids) = (*road, ids.clone());
        cx.notify();
        let Some(link) = self.link.clone() else { return };
        let tasks: Vec<_> = ids
            .iter()
            .map(|id| {
                let params = json!({ "workspaceId": id });
                match road {
                    Road::Forget => link.runtime.spawn(host::write::<WorkspacesForgetRequest>(link.host.clone(), params)),
                    _ => link.runtime.spawn(host::write::<WorkspacesDeleteRequest>(link.host.clone(), params)),
                }
            })
            .collect();
        cx.spawn(async move |this, cx| {
            let mut refused = None;
            for task in tasks {
                if let Err(f) = task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                    refused.get_or_insert(f.message);
                }
            }
            _ = this.update(cx, |this, cx| this.ended(&ids, refused, cx));
        })
        .detach();
    }

    /// Opens a plain confirm whose OK runs `on_ok`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn confirm(
        &mut self,
        _: &mut Window,
        cx: &mut Context<Self>,
        title: &str,
        line: &str,
        ok: &str,
        destructive: bool,
        on_ok: impl Fn(&mut Sidebar, &mut Context<Sidebar>) + 'static,
    ) {
        let open = OpenDialog::Confirm {
            title: title.to_owned().into(),
            line: line.to_owned().into(),
            ok: ok.to_owned().into(),
            destructive,
            on_ok: Rc::new(on_ok),
        };
        self.show_dialog(open, true, cx);
    }

    /// Delete task on a workspace, as its menu runs it.
    pub fn end_workspace(&mut self, id: &str, cx: &mut Context<Self>) {
        end_workspaces(self, vec![id.to_owned()], Road::Delete, cx);
    }

    /// The open dialog's words as shown: its title, its line, each line on what a copy holds and the host's refusal;
    /// none while unseen.
    pub fn dialog_words(&self) -> Option<Vec<String>> {
        let dialog = self.dialog.as_ref().filter(|d| d.shown)?;
        Some(match &dialog.open {
            OpenDialog::Workspaces { title, line, holding, said, .. } => {
                [title.to_string(), line.to_string()].into_iter().chain(holding.iter().flatten().cloned()).chain(said.clone()).collect()
            }
            OpenDialog::Processes { title, line, .. } => {
                [Some(title.to_string()), line.as_ref().map(|l| l.to_string())].into_iter().flatten().collect()
            }
            OpenDialog::Confirm { title, line, .. } => vec![title.to_string(), line.to_string()],
            OpenDialog::Snooze { .. } => vec!["Snooze thread".to_owned()],
        })
    }

    /// The end of a workspaces dialog, as its destructive button sends it once the copies are read; false before.
    pub fn end_asked(&mut self, cx: &mut Context<Self>) -> bool {
        let read = matches!(self.dialog.as_ref().map(|d| &d.open), Some(OpenDialog::Workspaces { holding: Some(_), .. }));
        if read {
            self.end_now(cx);
        }
        read
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.take().is_some() {
            window.focus(&self.focus, cx);
        }
        cx.notify();
    }

    /// Snoozes a thread until `at` ms, as a pick in Snooze does.
    pub fn snooze_until(&mut self, key: String, at: i64, cx: &mut Context<Self>) {
        self.dialog = None;
        self.hand_keys_back(cx);
        self.write_op::<SessionsMarkRequest>(json!({ "threadIds": [key], "marks": { "snoozedUntil": at } }), cx);
    }

    /// Answers the processes question, or sends the delete with none asked yet.
    fn finish(&mut self, processes: Processes, cx: &mut Context<Self>) {
        let Some(OpenDialog::Processes { ends, busy, said, .. }) = self.dialog.as_mut().map(|d| &mut d.open) else { return };
        *busy = Some(processes);
        *said = None;
        let ends = ends.clone();
        cx.notify();
        let Some(link) = self.link.clone() else { return };
        let task = match &ends {
            Ends::Delete(key) => link
                .runtime
                .spawn(host::write::<SessionsDeleteRequest>(link.host.clone(), json!({ "threadId": key, "processes": processes.word() }))),
            Ends::Settle(ids) => link
                .runtime
                .spawn(host::write::<SessionsSettleRequest>(link.host.clone(), json!({ "threadIds": ids, "processes": processes.word() }))),
        };
        cx.spawn(async move |this, cx| {
            let done = task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string())));
            _ = this.update(cx, |this, cx| this.finished(&ends, done, cx));
        })
        .detach();
    }

    /// The delete's check, on its own dialog alone: whether processes still run.
    pub(super) fn checked(&mut self, ends: &Ends, checked: Result<(), wsp_host_client::Failure>, cx: &mut Context<Self>) {
        let Some(OpenDialog::Processes { ends: on, left, checked: done, .. }) = self.dialog.as_mut().map(|d| &mut d.open) else { return };
        if on != ends {
            return;
        }
        *done = true;
        if let Err(f) = checked.filter_processes() {
            *left = Some(f);
        }
        cx.notify();
    }

    /// The host's answer to a delete or a settle, on its own dialog alone: it closes, or says why not.
    pub(super) fn finished(&mut self, ends: &Ends, done: Result<(), wsp_host_client::Failure>, cx: &mut Context<Self>) {
        let Some(OpenDialog::Processes { ends: on, left, busy, said, .. }) = self.dialog.as_mut().map(|d| &mut d.open) else { return };
        if on != ends {
            return;
        }
        *busy = None;
        match done {
            Ok(()) => {
                self.dialog = None;
                self.hand_keys_back(cx);
            }
            Err(f) if f.kind.as_deref() == Some("processes-left") => *left = Some(f.message),
            Err(f) => *said = Some(f.message),
        }
        cx.notify();
    }

    /// What each copy holds that is not pushed, on its own dialog alone; a clean read ends them where nobody is asked.
    pub(super) fn read_holding(&mut self, ids: &[String], holding: Vec<String>, asks: bool, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        let OpenDialog::Workspaces { ids: on, holding: read, .. } = &mut dialog.open else { return };
        if on != ids {
            return;
        }
        let clean = holding.is_empty();
        *read = Some(holding);
        dialog.shown |= !clean;
        if !asks && clean {
            self.end_now(cx);
        }
        cx.notify();
    }

    /// The host's answers to ending each workspace, on their own dialog alone.
    pub(super) fn ended(&mut self, ids: &[String], refused: Option<String>, cx: &mut Context<Self>) {
        let Some(dialog) = self.dialog.as_mut() else { return };
        let OpenDialog::Workspaces { ids: on, busy, said, .. } = &mut dialog.open else { return };
        if on != ids {
            return;
        }
        *busy = false;
        match refused {
            None => {
                self.dialog = None;
                self.hand_keys_back(cx);
            }
            Some(message) => {
                *said = Some(message);
                dialog.shown = true;
            }
        }
        cx.notify();
    }

    /// The open dialog over a scrim the size of the window, in the page's palette.
    pub(crate) fn dialog_overlay(&mut self, p: &Palette, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.dialog.as_mut().filter(|d| d.shown)?;
        // The dialog takes the keys as it opens, so Escape closes it; a field inside it takes them on its own after.
        if !std::mem::replace(&mut dialog.focused, true) {
            dialog.over_dialog = crate::screens::dialogs::layer::top(cx).is_some();
            window.focus(&dialog.focus, cx);
        }
        let focus = dialog.focus.clone();
        let priority = if dialog.over_dialog { 5 } else { 3 };
        let dialog = &self.dialog.as_ref()?.open;
        let (body, top) = match dialog {
            OpenDialog::Snooze { key, presets, until } => (self.snooze_body(key, presets, until, p, cx), true),
            OpenDialog::Processes { ends, title, line, left, busy, said, checked } => {
                (self.processes_body(ends, title, line.clone(), left.as_deref(), *busy, said.as_deref(), *checked, p, cx), false)
            }
            OpenDialog::Workspaces { road, title, line, holding, busy, said, .. } => {
                (self.workspaces_body(*road, title, line, holding.as_deref(), *busy, said.as_deref(), p, cx), false)
            }
            OpenDialog::Confirm { title, line, ok, destructive, on_ok } => {
                let on_ok = on_ok.clone();
                let act =
                    if *destructive { Button::destructive("dialog-ok", ok.clone()) } else { Button::primary("dialog-ok", ok.clone()) };
                let (me, focus_back) = (cx.weak_entity(), self.focus.clone());
                let act = act.on_click(move |_, window, cx| {
                    let on_ok = on_ok.clone();
                    _ = me.update(cx, |this, cx| {
                        this.dialog = None;
                        on_ok(this, cx);
                        cx.notify();
                    });
                    window.focus(&focus_back, cx);
                });
                (popup(title.clone(), Some(line.clone()), None, vec![self.cancel(cx).into_any_element(), act.into_any_element()], p), false)
            }
        };
        let size = window.viewport_size();
        let scrim = p.background.opacity(if gpui_kit::component::ActiveTheme::theme(&**cx).is_dark() { 0.64 } else { 0.6 });
        // An alert asks for its answer: only Snooze closes on the scrim (`AlertDialog` against `Dialog`).
        let scrim_closes = matches!(dialog, OpenDialog::Snooze { .. });
        let layer = div()
            .id("sidebar-dialog")
            .track_focus(&focus)
            .on_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" {
                    this.close_dialog(window, cx);
                }
            }))
            .occlude()
            .w(size.width)
            .h(size.height)
            .p(px(16.))
            .bg(scrim)
            .backdrop_blur(px(4.))
            .flex()
            .flex_col()
            .items_center()
            .when(!top, |el| el.justify_center())
            // `sm:mt-36` on Snooze's popup, under the viewport's 16 px.
            .when(top, |el| el.pt(px(16. + 144.)))
            .when(scrim_closes, |el| {
                el.on_mouse_down(gpui_kit::MouseButton::Left, cx.listener(|this, _, window, cx| this.close_dialog(window, cx)))
            })
            .child(div().on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(body));
        // The newer stands on top, as the web stacks its dialogs: over the dialogs' layer (4) only if one stood first.
        Some(deferred(anchored().position(point(px(0.), px(0.))).child(layer)).with_priority(priority).into_any_element())
    }

    fn cancel(&self, cx: &mut Context<Self>) -> Button {
        let me = cx.weak_entity();
        Button::outline("dialog-cancel", "Cancel")
            .neutral_ring()
            .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.close_dialog(window, cx)))
    }

    fn snooze_body(
        &self,
        key: &str,
        presets: &[(&'static str, i64, String)],
        until: &Entity<InputState>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = presets.iter().map(|(label, at, shown)| {
            let (me, key, at) = (cx.weak_entity(), key.to_owned(), *at);
            h_flex()
                .id(SharedString::from(format!("snooze-{label}")))
                .h(px(44.))
                .mx(px(-8.))
                .px(px(8.))
                .justify_between()
                .rounded(px(8.))
                .cursor_pointer()
                .hover(|s| s.bg(p.accent))
                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.snooze_until(key.clone(), at, cx)))
                .child(div().text_size(px(14.)).line_height(px(20.)).child(*label))
                .child(
                    div().text_size(px(13.)).text_color(p.muted_foreground).font_features(crate::ui::text::tabular()).child(shown.clone()),
                )
                .into_any_element()
        });
        let custom = h_flex()
            .h(px(44.))
            .justify_between()
            .child(div().text_size(px(14.)).child("Until"))
            .child(div().w(px(224.)).child(Input::new(until).small()));
        let (me, key, until) = (cx.weak_entity(), key.to_owned(), until.clone());
        let snooze = Button::primary("snooze-ok", "Snooze").on_click(move |_, _, cx| {
            let typed = until.read(cx).value().to_string();
            let at = chrono::NaiveDateTime::parse_from_str(typed.trim(), "%Y-%m-%d %H:%M")
                .ok()
                .and_then(|t| Local.from_local_datetime(&t).earliest())
                .filter(|t| *t > Local::now());
            if let Some(at) = at {
                _ = me.update(cx, |this, cx| this.snooze_until(key.clone(), at.timestamp_millis(), cx));
            }
        });
        let body = v_flex().px(px(20.)).pt(px(4.)).children(rows).child(custom).into_any_element();
        popup("Snooze thread".into(), None, Some(body), vec![self.cancel(cx).into_any_element(), snooze.into_any_element()], p)
    }

    #[allow(clippy::too_many_arguments)]
    fn processes_body(
        &self,
        ends: &Ends,
        title: &SharedString,
        line: Option<SharedString>,
        left: Option<&str>,
        busy: Option<Processes>,
        said: Option<&str>,
        checked: bool,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let me = cx.weak_entity();
        let answer = move |id: &'static str, word: &'static str, processes: Processes, destructive: bool| {
            let me = me.clone();
            let b = if destructive { Button::destructive(id, word) } else { Button::outline(id, word).neutral_ring() };
            b.held(busy.is_some()).on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.finish(processes, cx))).into_any_element()
        };
        let deleting = matches!(ends, Ends::Delete(_));
        let acts = match (left.is_some(), deleting) {
            (false, _) => vec![
                self.cancel(cx).into_any_element(),
                answer("delete-thread", if busy.is_some() { "Deleting…" } else { "Delete" }, Processes::Ask, true),
            ],
            (true, true) => vec![
                self.cancel(cx).into_any_element(),
                answer("delete-keep", "Delete, keep running", Processes::Keep, false),
                answer(
                    "delete-stop",
                    if busy == Some(Processes::Stop) { "Stopping…" } else { "Delete and stop them" },
                    Processes::Stop,
                    true,
                ),
            ],
            (true, false) => vec![
                self.cancel(cx).into_any_element(),
                answer("settle-keep", "Settle, keep running", Processes::Keep, false),
                answer(
                    "settle-stop",
                    if busy == Some(Processes::Stop) { "Stopping…" } else { "Settle and stop them" },
                    Processes::Stop,
                    true,
                ),
            ],
        };
        // The status line (`RefusalSlot`): the host's refusal, else its sentence on what still runs.
        let slot = match (said, left) {
            (Some(said), _) => Some((said.to_owned(), p.error_foreground)),
            (None, Some(left)) => Some((left.to_owned(), p.muted_foreground)),
            _ => None,
        };
        let body = div()
            .px(px(20.))
            .pt(px(8.))
            .child(
                div()
                    .min_h(px(36.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .children(slot.map(|(text, ink)| div().text_color(ink).child(text)))
                    .when(!checked, |el| el.opacity(0.)),
            )
            .into_any_element();
        popup(title.clone(), line, Some(body), acts, p)
    }

    #[allow(clippy::too_many_arguments)]
    fn workspaces_body(
        &self,
        road: Road,
        title: &SharedString,
        line: &SharedString,
        holding: Option<&[String]>,
        busy: bool,
        said: Option<&str>,
        p: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (word, going) = if road == Road::Forget { ("Forget", "Forgetting…") } else { ("Delete", "Deleting…") };
        let me = cx.weak_entity();
        let end = Button::destructive("end-workspace", if busy { going } else { word })
            .held(busy || holding.is_none())
            .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.end_now(cx)));
        let lines = holding.unwrap_or_default().iter().map(|l| div().child(l.clone()));
        // The list closes the head (`mt-2` under its `gap-1`, then its `pb-1`); the slot is `pt-2` over `min-h-9`.
        let body = v_flex()
            .px(px(20.))
            .when(holding.is_some_and(|h| !h.is_empty()), |el| {
                el.child(
                    v_flex()
                        .pt(px(8.))
                        .pb(px(4.))
                        .gap(px(4.))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.foreground)
                        .children(lines),
                )
            })
            .child(
                div().pt(px(8.)).child(
                    div()
                        .min_h(px(36.))
                        .text_size(px(13.))
                        .line_height(px(18.))
                        .text_color(p.error_foreground)
                        .children(said.map(str::to_owned)),
                ),
            )
            .into_any_element();
        popup(title.clone(), Some(line.clone()), Some(body), vec![self.cancel(cx).into_any_element(), end.into_any_element()], p)
    }
}

/// The popup: `rounded-xl`, the popover's edge, fill and dialog shadow; the head at `px-5 pt-4 pb-1`, the foot at
/// `px-5 pt-3 pb-4` with its acts at the right.
fn popup(title: SharedString, line: Option<SharedString>, body: Option<AnyElement>, acts: Vec<AnyElement>, p: &Palette) -> AnyElement {
    v_flex()
        .debug_selector(|| "confirm-popup".into())
        .w(px(WIDTH))
        .rounded(px(14.))
        .border_1()
        .border_color(p.popover_edge)
        .bg(p.popover)
        .shadow(p.dialog_shadow.clone())
        .text_color(p.foreground)
        .child(
            v_flex()
                .gap(px(4.))
                .px(px(20.))
                .pt(px(16.))
                .pb(px(4.))
                .child(div().text_size(px(15.)).line_height(px(20.)).font_weight(FontWeight::MEDIUM).child(title))
                .children(line.map(|l| div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(l))),
        )
        .children(body)
        .child(h_flex().justify_end().gap(px(8.)).px(px(20.)).pt(px(12.)).pb(px(16.)).children(acts))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sidebar in a window, its dialogs drawn over it.
    fn drawn(cx: &mut gpui_kit::TestAppContext) -> (Entity<Sidebar>, &mut gpui_kit::VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let sidebar = cx.new(Sidebar::new);
        let root = sidebar.clone();
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(root, window, cx));
        cx.simulate_resize(gpui_kit::size(px(1280.), px(800.)));
        (sidebar, cx)
    }

    /// W2-DLG-126: one workspace by name, Keep this one's others and Delete copies by count, on either road.
    #[gpui_kit::test]
    fn the_workspace_dialogs_name_what_they_end(cx: &mut gpui_kit::TestAppContext) {
        let (sidebar, cx) = drawn(cx);
        let title = |ids: &[&str], road: Road, cx: &mut gpui_kit::VisualTestContext| {
            sidebar.update(cx, |s, cx| {
                for id in ids {
                    s.around.workspaces.insert(id.to_string(), json!({ "id": id, "name": id, "kind": "local" }));
                }
                end_workspaces(s, ids.iter().map(|i| i.to_string()).collect(), road, cx);
                let words = s.dialog_words().unwrap_or_default();
                s.dialog = None;
                words.first().cloned().unwrap_or_default()
            })
        };
        assert_eq!(title(&["fix"], Road::Delete, cx), "Delete fix?");
        assert_eq!(title(&["fix"], Road::Forget, cx), "Forget fix?");
        assert_eq!(title(&["a", "b"], Road::Copies, cx), "Delete 2 copies?");
        assert_eq!(title(&["a", "b", "c"], Road::Others, cx), "Delete the other 3 copies?");
        assert_eq!(title(&["a", "b"], Road::Forget, cx), "Forget the other 2 copies?");
    }

    /// W2-DLG-012 on the sidebar's own frame: 440 px, centred in the window.
    #[gpui_kit::test]
    fn the_delete_stands_in_the_confirm_frame(cx: &mut gpui_kit::TestAppContext) {
        let (sidebar, cx) = drawn(cx);
        sidebar.update(cx, |s, cx| {
            let open = OpenDialog::Processes {
                ends: Ends::Delete("t".into()),
                title: "Delete t?".into(),
                line: Some(thread_delete_line(None).into()),
                left: None,
                busy: None,
                said: None,
                checked: true,
            };
            s.show_dialog(open, true, cx);
        });
        cx.run_until_parked();
        let popup = cx.debug_bounds("confirm-popup").expect("the delete is drawn");
        assert_eq!(popup.size.width, px(WIDTH));
        assert_eq!(popup.center().x, px(640.));
    }

    #[test]
    fn a_copy_names_what_it_holds_that_is_not_pushed() {
        assert_eq!(unpushed_line("repo", Some(&json!({ "ahead": 0, "changed": 0 }))), None);
        assert_eq!(unpushed_line("repo", None).as_deref(), Some("repo: could not read what is not pushed"));
        let held = json!({ "ahead": 2, "changed": 1, "stashes": 3 });
        assert_eq!(
            unpushed_line("repo", Some(&held)).as_deref(),
            Some("repo holds 2 commits not pushed, 1 uncommitted file and 3 stashes")
        );
        let unread = json!({ "ahead": 0, "countsUnknown": true, "changed": 4 });
        assert_eq!(unpushed_line("repo", Some(&unread)).as_deref(), Some("repo holds commits it could not count and 4 uncommitted files"));
    }

    #[test]
    fn the_delete_names_the_worktree_that_goes_with_the_last_thread() {
        assert!(thread_delete_line(None).ends_with("the folder it worked in is left as it is."));
        assert!(thread_delete_line(Some("/w/x")).contains("and the worktree at /w/x with it where no other thread is in it"));
    }

    #[test]
    fn the_presets_follow_snooze_ts() {
        let wednesday_noon = Local.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
        let words: Vec<&str> = presets(wednesday_noon).iter().map(|p| p.0).collect();
        assert_eq!(words, ["In 1 hour", "In 3 hours", "This evening", "Tomorrow", "Next week"]);
        let late = Local.with_ymd_and_hms(2026, 10, 14, 17, 30, 0).unwrap();
        assert!(!presets(late).iter().any(|p| p.0 == "This evening"), "an evening under an hour away is not offered");
        let sunday = Local.with_ymd_and_hms(2026, 10, 18, 12, 0, 0).unwrap();
        assert!(!presets(sunday).iter().any(|p| p.0 == "Next week"), "on Sunday next week is tomorrow");
        let monday = Local.with_ymd_and_hms(2026, 10, 12, 12, 0, 0).unwrap();
        let next = presets(monday).into_iter().find(|p| p.0 == "Next week").unwrap();
        assert_eq!(next.2, "Mon 9:00 AM");
        assert_eq!(Local.timestamp_millis_opt(next.1).unwrap().date_naive(), monday.date_naive() + Days::days(7));
    }
}
