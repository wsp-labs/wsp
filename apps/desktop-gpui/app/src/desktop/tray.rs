// SPDX-License-Identifier: AGPL-3.0-only
//! The menu bar's icon and menu as data (`desktop:tray.ts`): how many threads work, whether one waits on the person,
//! and a row per live thread with what can be done from there, in the sidebar's words. The Mac draws it with
//! `tray-icon` and `muda`; muda has no sublabel, so a row's state rides in its label after a middle dot.

use std::collections::HashMap;

use serde_json::Value;

use super::notify::{outside_line, Choices, Moment, OutsideLine};
use crate::screens::notices::words::asking_line;
use crate::screens::sidebar::menu::workspace_computer_name;
use crate::screens::sidebar::model::{fold_threads, Row, Status, Thread};

/// `TRAY_WORDS`.
pub const NOTHING: &str = "No threads working";
pub const NOT_RUNNING: &str = "wsp is not running";
pub const START: &str = "Start wsp";
pub const OPEN_APP: &str = "Open wsp";
pub const OPEN: &str = "Open";
pub const ALLOW: &str = "Allow";
pub const DENY: &str = "Deny";
pub const STOP: &str = "Stop";
pub use crate::screens::app_shell::menu::QUIT;

pub fn working(working: usize, waiting: usize) -> String {
    let words = [(working, "working"), (waiting, "waiting on you")];
    words.iter().filter(|(n, _)| *n > 0).map(|(n, w)| format!("{n} {w}")).collect::<Vec<_>>().join(", ")
}

pub fn placed(state: &str, computer: &str) -> String {
    if computer.is_empty() {
        state.to_owned()
    } else {
        format!("{state} on {computer}")
    }
}

pub fn on(host: &str) -> String {
    format!("On {host}")
}

pub fn unreached(host: &str) -> String {
    format!("{host} is not answering")
}

/// A refusal in the host's words, its first letter raised.
pub fn refused(said: &str) -> String {
    crate::screens::sidebar::model::capitalised(said)
}

/// `OpenAsk`: a prompt seen open, by the turn that asks it: what sessions.answer names it by and its options' ids and
/// effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenAsk {
    pub ask_id: String,
    pub options: Vec<(String, String)>,
}

impl OpenAsk {
    /// The prompt a `session.permission` frame opens.
    pub fn of(frame: &Value) -> OpenAsk {
        let options = frame.get("options").and_then(Value::as_array).map_or_else(Vec::new, |options| {
            options
                .iter()
                .filter_map(|o| Some((o.get("id")?.as_str()?.to_owned(), o.get("effect")?.as_str().unwrap_or_default().to_owned())))
                .collect()
        });
        OpenAsk { ask_id: frame.get("askId").map(text).unwrap_or_default(), options }
    }
}

fn text(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_owned)
}

/// `TrayHost`: what the window and the menu call the host, whether it is somewhere else, and whether it answers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayHost {
    pub label: String,
    pub remote: bool,
    pub lost: bool,
}

/// `TrayInput`.
#[derive(Clone, Copy)]
pub struct TrayInput<'a> {
    pub sessions: &'a [Value],
    pub asks: &'a HashMap<String, OpenAsk>,
    /// A pick the host would not take, by the prompt it was made on.
    pub refused: &'a HashMap<String, String>,
    pub workspaces: &'a [Value],
    pub places: &'a [Value],
    pub host: &'a TrayHost,
}

/// `TrayAct`: what a menu row does. Open carries the workspace too, which the window's route names a thread by.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayAct {
    Open { workspace_id: String, thread_id: String },
    Answer { session_id: String, ask_id: String, option_id: String },
    Stop { session_id: String },
    Start,
    OpenApp,
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayRow {
    Separator,
    /// A row with no act is drawn disabled.
    Line {
        label: String,
        act: Option<TrayAct>,
    },
    Thread {
        thread_id: String,
        label: String,
        sublabel: String,
        actions: Vec<(String, TrayAct)>,
    },
}

impl TrayRow {
    /// The words the menu draws: a thread's title with its state after a middle dot.
    pub fn label(&self) -> String {
        match self {
            TrayRow::Separator => String::new(),
            TrayRow::Line { label, .. } => label.clone(),
            TrayRow::Thread { label, sublabel, .. } => format!("{label} \u{b7} {sublabel}"),
        }
    }
}

/// `TrayModel`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrayModel {
    /// Beside the icon: the threads working, and the host's name where it is somewhere else.
    pub title: String,
    /// A thread waits on the person, which swaps the mark for the question mark.
    pub needs_you: bool,
    /// The Dock's badge: the threads waiting on the person.
    pub badge: usize,
    pub rows: Vec<TrayRow>,
}

impl TrayModel {
    /// The icon's tooltip.
    pub fn tooltip(&self) -> String {
        if self.title.is_empty() {
            crate::TITLE.to_owned()
        } else {
            format!("{} {}", crate::TITLE, self.title)
        }
    }
}

/// `threadState`'s words for the three states a listed thread is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Waiting,
    Running,
    Failed,
}

impl State {
    fn word(self) -> &'static str {
        match self {
            State::Waiting => "Needs you",
            State::Running => "Working",
            State::Failed => "Failed",
        }
    }
}

/// The state a thread is listed in: one that asks or works, and one that failed and nobody has opened since.
fn listed(t: &Thread) -> Option<State> {
    if t.asks {
        return Some(State::Waiting);
    }
    match t.status {
        Status::Running => Some(State::Running),
        Status::Failed if t.needs_you => Some(State::Failed),
        _ => None,
    }
}

/// The prompt a thread's row stands on: a prompt carries the agent's own session id, which a Codex row's id is not.
fn open_ask_of<'a>(t: &Thread, asks: &'a HashMap<String, OpenAsk>) -> Option<&'a OpenAsk> {
    t.asking_text.as_ref()?;
    asks.get(t.claude_session_id.as_deref().unwrap_or(&t.session_id))
}

fn actions(t: &Thread, asks: &HashMap<String, OpenAsk>) -> Vec<(String, TrayAct)> {
    let open = (OPEN.to_owned(), TrayAct::Open { workspace_id: t.workspace_id.clone(), thread_id: t.key.clone() });
    if t.status == Status::Failed {
        return vec![open];
    }
    let mut acts = vec![open];
    if let Some(ask) = open_ask_of(t, asks) {
        for (label, effect) in [(ALLOW, "allow"), (DENY, "deny")] {
            if let Some((id, _)) = ask.options.iter().find(|(_, e)| e == effect) {
                let act = TrayAct::Answer { session_id: t.session_id.clone(), ask_id: ask.ask_id.clone(), option_id: id.clone() };
                acts.push((label.to_owned(), act));
            }
        }
    }
    acts.push((STOP.to_owned(), TrayAct::Stop { session_id: t.session_id.clone() }));
    acts
}

fn computer_of(workspaces: &[Value], places: &[Value], workspace_id: &str) -> String {
    workspaces
        .iter()
        .find(|w| w.get("id").and_then(Value::as_str) == Some(workspace_id))
        .map(|w| workspace_computer_name(places, w))
        .unwrap_or_default()
}

fn threads(sessions: &[Value]) -> Vec<Thread> {
    let rows: Vec<Row> = sessions.iter().filter_map(Row::of).collect();
    fold_threads(&rows)
}

/// `trayModel`.
pub fn tray_model(input: TrayInput) -> TrayModel {
    let host = input.host;
    let tail = [
        TrayRow::Separator,
        TrayRow::Line { label: OPEN_APP.to_owned(), act: Some(TrayAct::OpenApp) },
        TrayRow::Line { label: QUIT.to_owned(), act: Some(TrayAct::Quit) },
    ];
    if host.lost {
        let said = if host.remote {
            vec![TrayRow::Line { label: unreached(&host.label), act: None }]
        } else {
            vec![
                TrayRow::Line { label: NOT_RUNNING.to_owned(), act: None },
                TrayRow::Line { label: START.to_owned(), act: Some(TrayAct::Start) },
            ]
        };
        let title = if host.remote { host.label.clone() } else { String::new() };
        return TrayModel { title, needs_you: false, badge: 0, rows: said.into_iter().chain(tail).collect() };
    }
    let all = threads(input.sessions);
    let mut live: Vec<(&Thread, State)> = all.iter().filter_map(|t| listed(t).map(|s| (t, s))).collect();
    live.sort_by(|(a, sa), (b, sb)| {
        (*sb != State::Running).cmp(&(*sa != State::Running)).then(b.started_at.unwrap_or(0.0).total_cmp(&a.started_at.unwrap_or(0.0)))
    });
    let working_now = live.iter().filter(|(_, s)| *s == State::Running).count();
    let waiting = live.len() - working_now;
    let count = if working_now > 0 { working_now.to_string() } else { String::new() };
    let mut rows = Vec::new();
    if host.remote {
        rows.push(TrayRow::Line { label: on(&host.label), act: None });
    }
    let head = if live.is_empty() { NOTHING.to_owned() } else { working(working_now, waiting) };
    rows.push(TrayRow::Line { label: head, act: None });
    for (t, state) in &live {
        let refusal = open_ask_of(t, input.asks).and_then(|ask| input.refused.get(&ask.ask_id)).map(|said| refused(said));
        rows.push(TrayRow::Thread {
            thread_id: t.key.clone(),
            label: t.title.clone(),
            sublabel: refusal.unwrap_or_else(|| placed(state.word(), &computer_of(input.workspaces, input.places, &t.workspace_id))),
            actions: actions(t, input.asks),
        });
    }
    rows.extend(tail);
    let title =
        if host.remote { [host.label.clone(), count].into_iter().filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ") } else { count };
    TrayModel { title, needs_you: waiting > 0, badge: all.iter().filter(|t| t.needs_you).count(), rows }
}

/// Whether a thread was started by an agent, read off its first row as `foldThreads` reads it.
fn started_by_agent(sessions: &[Value], key: &str) -> bool {
    sessions
        .iter()
        .find(|s| s.get("threadId").or_else(|| s.get("id")).and_then(Value::as_str) == Some(key))
        .and_then(|s| s.get("startedBy"))
        .and_then(Value::as_str)
        == Some("agent")
}

/// `trayNotice`: what is said over the system while no window says it: a prompt, a thread a person or a line started
/// that finished or failed, and a plan alert. An agent's own thread reports to that agent; nothing else is said.
pub fn tray_notice(event: &Value, sessions: &[Value], workspaces: &[Value], places: &[Value], choices: &Choices) -> Option<OutsideLine> {
    let field = |k: &str| event.get(k).and_then(Value::as_str);
    match field("type")? {
        "usage.alert" => outside_line(&Moment::Plan { label: field("label").unwrap_or_default(), alert: event.get("alert")? }, choices),
        "session.permission" => {
            let line = asking_line(field("toolName").unwrap_or_default(), field("input").unwrap_or_default(), field("detail"));
            outside_line(&Moment::Asks { line: &line }, choices)
        }
        "session.done" => {
            let key = field("threadId").or(field("sessionId"))?;
            let thread = threads(sessions).into_iter().find(|t| t.key == key)?;
            if started_by_agent(sessions, &thread.key) {
                return None;
            }
            let place = computer_of(workspaces, places, field("workspaceId").unwrap_or_default());
            let result = event.get("result")?;
            match result.get("status").and_then(Value::as_str)? {
                "failed" => outside_line(
                    &Moment::Failed { thread: &thread.title, place: &place, error: result.get("error").and_then(Value::as_str) },
                    choices,
                ),
                "completed" => outside_line(&Moment::Finished { thread: &thread.title, place: &place }, choices),
                _ => None,
            }
        }
        _ => None,
    }
}

/// A menu row's id, never made twice in a run, so a pick from a menu drawn over finds no act rather than another row's.
#[cfg(any(target_os = "macos", test))]
fn menu_id() -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!("tray-{}", NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

#[cfg(target_os = "macos")]
pub use native::draw;

#[cfg(target_os = "macos")]
mod native {
    use std::collections::HashMap;
    use std::rc::Rc;

    use futures::channel::mpsc;
    use futures::StreamExt;
    use gpui_kit::{App, AsyncApp, Global};
    use muda::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
    use objc2::rc::Retained;
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSCellImagePosition, NSImage};
    use objc2_foundation::{NSData, NSSize};
    use tray_icon::{TrayIcon, TrayIconBuilder, TrayIconEvent};

    use super::{menu_id, TrayAct, TrayModel, TrayRow};

    /// Today's template marks (`apps/desktop/src/tray`), drawn at 18 points from their 36 px art.
    const MARK: &[u8] = include_bytes!("../../../../desktop/src/tray/trayTemplate@2x.png");
    const ASKING: &[u8] = include_bytes!("../../../../desktop/src/tray/trayAskTemplate@2x.png");
    const POINTS: f64 = 18.0;

    /// What a picked row's act is handed to.
    pub type OnAct = Rc<dyn Fn(TrayAct, &mut App)>;

    /// The status item, the act behind each of its rows by id, and what a pick is handed to.
    struct Tray {
        icon: TrayIcon,
        acts: HashMap<String, TrayAct>,
        asking: Option<bool>,
        on_act: OnAct,
    }

    impl Global for Tray {}

    fn image(png: &[u8]) -> Option<Retained<NSImage>> {
        let image = NSImage::initWithData(NSImage::alloc(), &NSData::with_bytes(png))?;
        image.setSize(NSSize::new(POINTS, POINTS));
        image.setTemplate(true);
        Some(image)
    }

    fn set_mark(icon: &TrayIcon, asking: bool) {
        let (Some(mtm), Some(item)) = (MainThreadMarker::new(), icon.ns_status_item()) else { return };
        let Some(button) = item.button(mtm) else { return };
        button.setImage(image(if asking { ASKING } else { MARK }).as_deref());
        button.setImagePosition(NSCellImagePosition::ImageLeft);
    }

    /// A label as muda takes it, which reads a lone `&` as a mnemonic.
    fn escaped(label: &str) -> String {
        label.replace('&', "&&")
    }

    /// The menu for `rows`, and the act behind each row's id.
    fn menu(rows: &[TrayRow]) -> (Menu, HashMap<String, TrayAct>) {
        let menu = Menu::new();
        let mut acts = HashMap::new();
        let item = |label: &str, act: Option<&TrayAct>, acts: &mut HashMap<String, TrayAct>| {
            let id = menu_id();
            if let Some(act) = act {
                acts.insert(id.clone(), act.clone());
            }
            MenuItem::with_id(id, escaped(label), act.is_some(), None)
        };
        for row in rows {
            let added = match row {
                TrayRow::Separator => menu.append(&PredefinedMenuItem::separator()),
                TrayRow::Line { label, act } => menu.append(&item(label, act.as_ref(), &mut acts)),
                TrayRow::Thread { actions, .. } => {
                    let items: Vec<MenuItem> = actions.iter().map(|(label, act)| item(label, Some(act), &mut acts)).collect();
                    let items: Vec<&dyn IsMenuItem> = items.iter().map(|i| i as &dyn IsMenuItem).collect();
                    Submenu::with_items(escaped(&row.label()), true, &items).and_then(|sub| menu.append(&sub))
                }
            };
            if let Err(e) = added {
                crate::desktop::log::error(&format!("menu bar: {e}"));
            }
        }
        (menu, acts)
    }

    /// Draws `model` in the menu bar, making the status item the first time; a pick goes through a channel to a
    /// foreground task and then to `on_act`, never into GPUI inside AppKit's menu loop.
    pub fn draw(model: &TrayModel, on_act: OnAct, cx: &mut App) {
        let (menu, acts) = menu(&model.rows);
        if !cx.has_global::<Tray>() {
            let icon = match TrayIconBuilder::new().with_id("wsp").build() {
                Ok(icon) => icon,
                Err(e) => {
                    crate::desktop::log::error(&format!("menu bar: {e}"));
                    return;
                }
            };
            let (tx, mut rx) = mpsc::unbounded::<String>();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| _ = tx.unbounded_send(event.id.0)));
            // Without a handler tray-icon queues every click on the icon in a channel nothing reads.
            TrayIconEvent::set_event_handler(Some(|_: TrayIconEvent| {}));
            cx.spawn(async move |cx: &mut AsyncApp| {
                while let Some(id) = rx.next().await {
                    cx.update(|cx| {
                        let picked = cx.try_global::<Tray>().and_then(|t| Some((t.acts.get(&id)?.clone(), t.on_act.clone())));
                        if let Some((act, on_act)) = picked {
                            on_act(act, cx);
                        }
                    });
                }
            })
            .detach();
            cx.set_global(Tray { icon, acts: HashMap::new(), asking: None, on_act: on_act.clone() });
        }
        let tray = cx.global_mut::<Tray>();
        if tray.asking != Some(model.needs_you) {
            set_mark(&tray.icon, model.needs_you);
            tray.asking = Some(model.needs_you);
        }
        tray.icon.set_title(Some(&model.title));
        if let Err(e) = tray.icon.set_tooltip(Some(model.tooltip())) {
            crate::desktop::log::error(&format!("menu bar: {e}"));
        }
        tray.icon.set_menu(Some(Box::new(menu)));
        tray.acts = acts;
        tray.on_act = on_act;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::notify::NotifyChoice;
    use serde_json::json;

    #[test]
    fn a_menu_drawn_again_reuses_no_row_id() {
        let first: Vec<String> = (0..3).map(|_| menu_id()).collect();
        let again: Vec<String> = (0..3).map(|_| menu_id()).collect();
        assert!(again.iter().all(|id| !first.contains(id)), "{first:?} {again:?}");
    }

    fn places() -> Vec<Value> {
        vec![
            json!({ "id": "here", "kind": "computer", "name": "zmac", "label": "zingzy's MacBook Pro", "default": true }),
            json!({ "id": "pl_box", "kind": "computer", "name": "spoo", "default": false }),
        ]
    }

    fn workspaces() -> Vec<Value> {
        vec![
            json!({ "id": "ws_mac", "kind": "local", "machineId": "local", "name": "wsp" }),
            json!({ "id": "ws_box", "kind": "cloud", "machineId": "m1", "place": "pl_box", "name": "wsp-box" }),
        ]
    }

    fn row(over: Value) -> Value {
        let id = over["id"].as_str().unwrap().to_owned();
        let mut row =
            json!({ "workspaceId": "ws_mac", "harness": "claude", "status": "running", "prompt": format!("task {id}"), "startedAt": 1 });
        row.as_object_mut().unwrap().extend(over.as_object().unwrap().clone());
        row
    }

    struct Case {
        sessions: Vec<Value>,
        asks: HashMap<String, OpenAsk>,
        refused: HashMap<String, String>,
        workspaces: Vec<Value>,
        places: Vec<Value>,
        host: TrayHost,
    }

    fn case(sessions: Vec<Value>) -> Case {
        Case {
            sessions,
            asks: HashMap::new(),
            refused: HashMap::new(),
            workspaces: workspaces(),
            places: places(),
            host: TrayHost { label: "zingzy's MacBook Pro".to_owned(), remote: false, lost: false },
        }
    }

    impl Case {
        fn model(&self) -> TrayModel {
            tray_model(TrayInput {
                sessions: &self.sessions,
                asks: &self.asks,
                refused: &self.refused,
                workspaces: &self.workspaces,
                places: &self.places,
                host: &self.host,
            })
        }
    }

    fn labels(rows: &[TrayRow]) -> Vec<String> {
        rows.iter().map(|r| if *r == TrayRow::Separator { "---".to_owned() } else { r.label() }).collect()
    }

    /// A thread row's id, title, state and acts.
    type Listed = (String, String, String, Vec<(String, TrayAct)>);

    fn thread_rows(rows: &[TrayRow]) -> Vec<Listed> {
        rows.iter()
            .filter_map(|r| match r {
                TrayRow::Thread { thread_id, label, sublabel, actions } => {
                    Some((thread_id.clone(), label.clone(), sublabel.clone(), actions.clone()))
                }
                _ => None,
            })
            .collect()
    }

    fn ask(ask_id: &str, options: &[(&str, &str)]) -> OpenAsk {
        OpenAsk { ask_id: ask_id.to_owned(), options: options.iter().map(|(i, e)| (i.to_string(), e.to_string())).collect() }
    }

    fn open(thread: &str) -> (String, TrayAct) {
        (OPEN.to_owned(), TrayAct::Open { workspace_id: "ws_mac".to_owned(), thread_id: thread.to_owned() })
    }

    /// `tray.test.ts:31`.
    #[test]
    fn counts_the_threads_working_and_says_nothing_when_none_is() {
        let idle = case(vec![]).model();
        assert_eq!(idle.title, "");
        assert!(!idle.needs_you);
        assert_eq!(labels(&idle.rows), [NOTHING, "---", OPEN_APP, QUIT]);
        assert_eq!(idle.tooltip(), "wsp");
        let busy = case(vec![
            row(json!({ "id": "s1", "threadId": "t1" })),
            row(json!({ "id": "s2", "threadId": "t2", "workspaceId": "ws_box" })),
            row(json!({ "id": "s3", "threadId": "t3", "status": "completed", "endedAt": 5, "readAt": 6 })),
        ])
        .model();
        assert_eq!(busy.title, "2");
        assert_eq!(busy.tooltip(), "wsp 2");
        assert_eq!(labels(&busy.rows)[0], working(2, 0));
    }

    /// `tray.test.ts:42`, and with 14 threads every one is listed.
    #[test]
    fn lists_a_waiting_thread_first_then_the_newest_with_no_cap() {
        let model = case(vec![
            row(json!({ "id": "s1", "threadId": "t_work", "startedAt": 9 })),
            row(json!({ "id": "s2", "threadId": "t_ask", "asking": "Bash: sleep 5", "startedAt": 1 })),
            row(json!({ "id": "s3", "threadId": "t_old", "status": "completed", "endedAt": 5, "readAt": 6 })),
        ])
        .model();
        assert!(model.needs_you);
        assert_eq!(model.title, "1");
        assert_eq!(thread_rows(&model.rows).iter().map(|r| r.0.as_str()).collect::<Vec<_>>(), ["t_ask", "t_work"]);
        assert_eq!(labels(&model.rows)[0], working(1, 1));
        let many: Vec<Value> =
            (0..14).map(|n| row(json!({ "id": format!("s{n}"), "threadId": format!("t{n}"), "startedAt": n }))).collect();
        let model = case(many).model();
        let listed: Vec<String> = thread_rows(&model.rows).into_iter().map(|r| r.0).collect();
        assert_eq!(listed, (0..14).rev().map(|n| format!("t{n}")).collect::<Vec<_>>());
        assert_eq!(model.title, "14");
    }

    /// `tray.test.ts:53`.
    #[test]
    fn names_each_row_by_its_thread_and_computer() {
        let model = case(vec![
            row(json!({ "id": "s1", "threadId": "t1", "prompt": "fix login", "startedAt": 2 })),
            row(json!({ "id": "s2", "threadId": "t2", "workspaceId": "ws_box", "prompt": "ship it", "startedAt": 1 })),
        ])
        .model();
        let rows = thread_rows(&model.rows);
        assert_eq!((rows[0].1.as_str(), rows[0].2.as_str()), ("fix login", "Working on zingzy's MacBook Pro"));
        assert_eq!((rows[1].1.as_str(), rows[1].2.as_str()), ("ship it", "Working on spoo"));
        assert_eq!(model.rows[1].label(), "fix login \u{b7} Working on zingzy's MacBook Pro");
    }

    /// A 200-character title is held whole.
    #[test]
    fn a_long_title_is_held_whole() {
        let title = "a".repeat(200);
        let model = case(vec![row(json!({ "id": "s1", "threadId": "t1", "harnessTitle": title }))]).model();
        assert_eq!(thread_rows(&model.rows)[0].1, title);
    }

    /// `tray.test.ts:59`.
    #[test]
    fn allow_and_deny_only_while_the_row_holds_an_open_prompt() {
        let mut c = case(vec![
            row(json!({ "id": "s1", "threadId": "t_work", "harness": "codex" })),
            row(json!({ "id": "s2", "threadId": "t_ask", "asking": "Bash: sleep 5" })),
        ]);
        c.asks.insert("s2".to_owned(), ask("a1", &[("o_yes", "allow"), ("o_mode", "mode"), ("o_no", "deny")]));
        let rows = thread_rows(&c.model().rows);
        let answer = |o: &str| TrayAct::Answer { session_id: "s2".to_owned(), ask_id: "a1".to_owned(), option_id: o.to_owned() };
        assert_eq!(
            rows[0].3,
            [
                open("t_ask"),
                (ALLOW.to_owned(), answer("o_yes")),
                (DENY.to_owned(), answer("o_no")),
                (STOP.to_owned(), TrayAct::Stop { session_id: "s2".to_owned() })
            ]
        );
        assert_eq!(rows[1].3.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), [OPEN, STOP]);
    }

    /// `tray.test.ts:71`.
    #[test]
    fn a_codex_rows_prompt_is_found_by_the_agents_session_id() {
        let mut c = case(vec![row(
            json!({ "id": "s_launch", "threadId": "t_codex", "harness": "codex", "claudeSessionId": "019a-codex-thread", "asking": "Bash: ls" }),
        )]);
        c.asks.insert("019a-codex-thread".to_owned(), ask("a1", &[("o_yes", "allow")]));
        let act = TrayAct::Answer { session_id: "s_launch".to_owned(), ask_id: "a1".to_owned(), option_id: "o_yes".to_owned() };
        assert!(thread_rows(&c.model().rows)[0].3.contains(&(ALLOW.to_owned(), act)));
    }

    /// `tray.test.ts:78`.
    #[test]
    fn a_refused_pick_is_the_rows_words_while_its_prompt_stays_open() {
        let mut c = case(vec![row(json!({ "id": "s2", "threadId": "t_ask", "asking": "Bash: ls" }))]);
        c.asks.insert("s2".to_owned(), ask("a1", &[("o_yes", "allow")]));
        c.refused.insert("a1".to_owned(), "this host holds no turn of that thread".to_owned());
        assert_eq!(thread_rows(&c.model().rows)[0].2, "This host holds no turn of that thread");
        c.asks.clear();
        assert_eq!(thread_rows(&c.model().rows)[0].2, "Needs you on zingzy's MacBook Pro");
    }

    /// `tray.test.ts:86`.
    #[test]
    fn a_prompt_never_seen_open_offers_open_and_stop_and_marks_the_icon() {
        let model = case(vec![row(json!({ "id": "s1", "threadId": "t_ask", "asking": "Bash: ls" }))]).model();
        assert!(model.needs_you);
        assert_eq!(thread_rows(&model.rows)[0].3.iter().map(|a| a.0.as_str()).collect::<Vec<_>>(), [OPEN, STOP]);
    }

    /// `tray.test.ts:92`.
    #[test]
    fn on_a_host_elsewhere_the_title_and_first_row_name_it() {
        let mut c = case(vec![row(json!({ "id": "s1", "threadId": "t1", "workspaceId": "ws_box" }))]);
        c.host = TrayHost { label: "spoo".to_owned(), remote: true, lost: false };
        let model = c.model();
        assert_eq!(model.title, "spoo 1");
        assert_eq!(labels(&model.rows)[0], on("spoo"));
        c.sessions.clear();
        assert_eq!(c.model().title, "spoo");
    }

    /// `tray.test.ts:99`: lost here is a Start; lost elsewhere is one line; plain mark and badge 0 either way.
    #[test]
    fn a_host_that_stopped_answering_says_so() {
        let mut c = case(vec![row(json!({ "id": "s1", "threadId": "t1", "asking": "Bash: ls" }))]);
        c.host.lost = true;
        let here = c.model();
        assert_eq!(here.title, "");
        assert_eq!((here.needs_you, here.badge), (false, 0));
        assert_eq!(labels(&here.rows), [NOT_RUNNING, START, "---", OPEN_APP, QUIT]);
        assert_eq!(here.rows[1], TrayRow::Line { label: START.to_owned(), act: Some(TrayAct::Start) });
        assert_eq!(here.rows[0], TrayRow::Line { label: NOT_RUNNING.to_owned(), act: None }, "the line takes no click");
        c.host = TrayHost { label: "spoo".to_owned(), remote: true, lost: true };
        let away = c.model();
        assert_eq!(away.title, "spoo");
        assert_eq!(labels(&away.rows), [unreached("spoo"), "---".to_owned(), OPEN_APP.to_owned(), QUIT.to_owned()]);
    }

    /// `tray.test.ts:109`.
    #[test]
    fn carries_the_count_the_dock_reads() {
        let model = case(vec![
            row(json!({ "id": "s1", "threadId": "t_ask", "asking": "Bash: ls" })),
            row(json!({ "id": "s2", "threadId": "t_done", "status": "completed", "endedAt": 5 })),
            row(json!({ "id": "s3", "threadId": "t_work" })),
        ])
        .model();
        assert_eq!(model.badge, 2);
    }

    /// `tray.test.ts:114`.
    #[test]
    fn a_failure_nobody_opened_stands_as_a_row_that_opens_it() {
        let failed = row(json!({ "id": "s1", "threadId": "t_failed", "status": "failed", "endedAt": 5 }));
        let model = case(vec![failed.clone()]).model();
        assert!(model.needs_you);
        let rows = thread_rows(&model.rows);
        assert_eq!((rows[0].0.as_str(), rows[0].2.as_str()), ("t_failed", "Failed on zingzy's MacBook Pro"));
        assert_eq!(rows[0].3, [open("t_failed")]);
        let mut seen = failed;
        seen["readAt"] = json!(6);
        let model = case(vec![seen]).model();
        assert!(!model.needs_you);
        assert!(thread_rows(&model.rows).is_empty());
    }

    /// `tray.test.ts:127`.
    #[test]
    fn never_names_the_parts_a_person_does_not_see() {
        let every = [
            NOTHING.to_owned(),
            working(3, 2),
            NOT_RUNNING.to_owned(),
            START.to_owned(),
            unreached("spoo"),
            on("spoo"),
            OPEN_APP.to_owned(),
        ];
        for word in every.iter().map(String::as_str).chain([OPEN, ALLOW, DENY, STOP]) {
            let lower = word.to_lowercase();
            assert!(!["host", "daemon", "service", "session"].iter().any(|w| lower.contains(w)), "{word}");
        }
    }

    fn loud() -> Choices {
        Choices { notify_needs: NotifyChoice::NotifySound, notify_done: NotifyChoice::NotifySound, plan_alerts: true }
    }

    fn noticed(event: Value, choices: &Choices) -> Option<OutsideLine> {
        let sessions = [
            row(json!({ "id": "s1", "threadId": "t1", "prompt": "fix login" })),
            row(json!({ "id": "s2", "threadId": "t_agent", "prompt": "sub task", "startedBy": "agent" })),
        ];
        tray_notice(&event, &sessions, &workspaces(), &places(), choices)
    }

    fn line(title: &str, body: &str, show: bool, sound: bool) -> Option<OutsideLine> {
        Some(OutsideLine { title: title.to_owned(), body: body.to_owned(), show, sound })
    }

    fn done(session: &str, thread: &str, result: Value) -> Value {
        json!({ "type": "session.done", "workspaceId": "ws_mac", "sessionId": session, "threadId": thread, "result": result })
    }

    /// `tray.test.ts:139`.
    #[test]
    fn a_finished_turn_says_the_thread_finished_and_where() {
        let finished = done("s1", "t1", json!({ "status": "completed" }));
        assert_eq!(noticed(finished.clone(), &loud()), line("fix login finished", "zingzy's MacBook Pro", true, true));
        let quiet = Choices { notify_done: NotifyChoice::Notify, ..loud() };
        assert_eq!(noticed(finished.clone(), &quiet).map(|l| (l.show, l.sound)), Some((true, false)));
        let sound = Choices { notify_done: NotifyChoice::Sound, ..loud() };
        assert_eq!(noticed(finished.clone(), &sound).map(|l| (l.show, l.sound)), Some((false, true)));
        assert_eq!(noticed(finished, &Choices::default()).map(|l| (l.show, l.sound)), Some((true, false)));
    }

    /// `tray.test.ts:147`, `:166`.
    #[test]
    fn an_agents_thread_an_end_and_a_turn_somebody_stopped_say_nothing() {
        assert_eq!(noticed(done("s2", "t_agent", json!({ "status": "completed" })), &loud()), None);
        assert_eq!(noticed(done("s2", "t_agent", json!({ "status": "failed", "error": "x" })), &loud()), None);
        assert_eq!(noticed(done("s1", "t1", json!({ "status": "interrupted" })), &loud()), None);
        let end = json!({ "type": "session.end", "workspaceId": "ws_mac", "sessionId": "s1", "threadId": "t1", "exitCode": 1, "sawResult": false });
        assert_eq!(noticed(end, &loud()), None);
    }

    /// `tray.test.ts:152`.
    #[test]
    fn a_failed_turn_says_the_thread_stopped_with_its_error_in_one_line() {
        let failed = done("s1", "t1", json!({ "status": "failed", "error": "API Error: 529\n overloaded" }));
        assert_eq!(noticed(failed.clone(), &loud()), line("fix login stopped", "API Error: 529 overloaded", true, true));
        assert_eq!(noticed(failed, &Choices { notify_done: NotifyChoice::Off, ..loud() }), None);
    }

    /// `tray.test.ts:174`.
    #[test]
    fn a_plan_alert_is_said_with_no_sound_while_its_switch_is_on() {
        let low = json!({ "type": "usage.alert", "key": "claude:acct_1", "agent": "claude", "label": "Max plan", "alert": { "kind": "low", "window": "week", "step": 90 } });
        assert_eq!(noticed(low.clone(), &Choices::default()), line("Max plan has used 90% of its week", "", true, false));
        let mut blocked = low.clone();
        blocked["alert"] = json!({ "kind": "blocked" });
        assert_eq!(noticed(blocked, &Choices::default()).unwrap().title, "Max plan reached its plan limit");
        assert_eq!(noticed(low, &Choices { plan_alerts: false, ..Choices::default() }), None);
    }

    /// `tray.test.ts:181`.
    #[test]
    fn a_prompt_says_what_the_thread_asks() {
        let asked = json!({ "type": "session.permission", "workspaceId": "ws_mac", "sessionId": "s1", "threadId": "t1", "askId": "a1", "toolName": "Bash", "input": "{\"command\":\"sleep 5\"}", "options": [] });
        let said = noticed(asked.clone(), &Choices::default()).unwrap();
        assert_eq!(said.title, "wsp needs you");
        assert!(said.body.contains("sleep 5"), "{}", said.body);
        assert!(said.show && said.sound);
        assert_eq!(noticed(asked, &Choices { notify_needs: NotifyChoice::Off, ..Choices::default() }), None);
    }

    #[test]
    fn a_workspace_no_row_places_goes_by_its_provider_else_its_kind() {
        let places = places();
        assert_eq!(workspace_computer_name(&places, &json!({ "kind": "cloud", "provider": "box", "machineId": "m" })), "Boat");
        assert_eq!(workspace_computer_name(&places, &json!({ "kind": "cloud", "machineId": "m" })), "a provider");
        assert_eq!(workspace_computer_name(&places, &json!({ "kind": "place", "place": "gone", "machineId": "m9" })), "m9");
        assert_eq!(workspace_computer_name(&[], &json!({ "kind": "local", "machineId": "local" })), "");
    }
}
