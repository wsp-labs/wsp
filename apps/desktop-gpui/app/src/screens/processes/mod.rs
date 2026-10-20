// SPDX-License-Identifier: AGPL-3.0-only
//! The Processes pane of the right panel, after `components/procs/ProcessesSurface.tsx` and `ThreadProcesses.tsx`. A
//! thread on this computer or on a box the person joined draws Activity's Threads table over that thread and the
//! threads under it, with Open in Activity at its foot; anything else draws the computer's every process in one mono
//! table, a tree by parent, its lit row opening onto the daemon's inspect fields and its one act. Both read one computer
//! through Activity's `Scoped`, which this pane builds when it opens and drops with itself, so a pane nobody draws
//! watches nothing. What a person set here (the sort, the filter, the lit row) is kept by the pane's tab, so the pane
//! built again after a thread switch finds it, and goes when the tab closes.

pub mod table;
pub mod thread;
pub mod tree;

use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{prelude::*, App, Context, Entity, Global, Pixels, ScrollHandle, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{ActivityEndRequest, PlaceView, SessionInterruptResultOutcome, SessionsInterruptRequest};

use crate::screens::computer::daemon_down::Restart;
use crate::screens::computer::model::stale_word;
use crate::screens::dialogs::layer;
use crate::screens::palette::overlays;
use crate::screens::settings::model::req;
use crate::screens::settings::pages::activity::model::{Group, GroupKind, Thread};
use crate::screens::settings::pages::activity::wire::Pid;
use crate::screens::settings::pages::activity::{kill_of, reached_alone, status_of, threads_on, Computer, Latest, Lists, Scoped};
use crate::screens::settings::words::HERE;
use crate::screens::terminal::state::Absent;
use crate::screens::{Host, Opened, Pane, Place, Screen};
use crate::sidebar::Sidebar;
use crate::ui::ComputerIcon;
use tree::{words as w, AgentOf, Kill, Send, Sort};

/// How often an armed or waiting kill reads the clock (`kill.ts`'s 250 ms interval).
const TICK: Duration = Duration::from_millis(250);
/// The key the stop's ask stands under on the window's dialogs layer.
const STOP_ASK: &str = "processes-stop";

/// What a person set in the pane, found again by its tab.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub sort: Sort,
    pub filter: String,
    pub lit: Option<Pid>,
}

#[derive(Default)]
struct Sessions(HashMap<SharedString, Session>);

impl Global for Sessions {}

fn tab_of(opened: &Opened) -> SharedString {
    opened.tab.clone().unwrap_or_else(|| SharedString::new_static(Processes::NAME))
}

/// What the pane reads: the computer table, or a thread's own.
pub enum Form {
    Computer(Entity<Scoped>),
    Thread(Entity<Scoped>),
}

/// The lit row's inspect answer, its act's state and what that act said.
#[derive(Default)]
struct Lit {
    pid: Option<Pid>,
    /// Whether its inspect was asked for, which waits for a frame that lists it.
    asked: bool,
    inspect: Option<Result<Value, String>>,
    kill: Kill,
    said: Option<String>,
}

pub struct Processes {
    host: Host,
    opened: Opened,
    session: Session,
    sidebar: Option<Entity<Sidebar>>,
    lists: Entity<Lists>,
    form: Option<Form>,
    filter: Entity<InputState>,
    scroll: ScrollHandle,
    lit: Lit,
    /// Ticks while the lit row's kill waits on the clock.
    clock: Option<Task<()>>,
    restart: Entity<Restart>,
    /// The pane's width as last drawn, which the thread table's columns read as the web's container query does.
    width: Rc<std::cell::Cell<Pixels>>,
    _observed: Vec<Subscription>,
}

/// The thread pane's state as drawn.
#[derive(Clone, Debug, PartialEq)]
pub enum ThreadState {
    Absent(Absent),
    Refused(String),
    Pending,
    Nothing,
    Groups(Vec<Group>),
}

impl Processes {
    pub fn form(&self) -> Option<&Form> {
        self.form.as_ref()
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn scoped(&self) -> Option<&Entity<Scoped>> {
        match self.form.as_ref()? {
            Form::Computer(s) | Form::Thread(s) => Some(s),
        }
    }

    /// Whether the computer's link is open.
    pub fn live(&self, cx: &App) -> bool {
        self.scoped().is_some_and(|s| s.read(cx).channel.is_some())
    }

    /// The pids the last frame listed, none before one came.
    pub fn listed(&self, cx: &App) -> Option<Vec<Pid>> {
        self.scoped()?.read(cx).snapshot.as_ref().map(|snap| snap.procs.iter().map(|p| p.pid).collect())
    }

    fn save(&self, cx: &mut App) {
        cx.default_global::<Sessions>().0.insert(tab_of(&self.opened), self.session.clone());
    }

    fn find_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let waiting = overlays::find_sidebar(self, window, cx, |this, sidebar, window, cx| {
            this._observed.push(cx.observe_in(&sidebar, window, |this, _, window, cx| {
                this.settle_form(window, cx);
                cx.notify();
            }));
            this.sidebar = Some(sidebar);
            this.settle_form(window, cx);
        });
        self._observed.extend(waiting);
    }

    fn workspace<'a>(&self, s: &'a Sidebar) -> Option<(&'a Value, Option<&'a Value>)> {
        let id = self.opened.workspace_id.as_deref()?;
        Some((s.around().workspaces.get(id)?, s.around().statuses.get(id)))
    }

    /// This computer's row of the places list, or a stand-in until the sidebar has read it.
    fn here_place(&self, cx: &App) -> PlaceView {
        let row = self.sidebar.as_ref().and_then(|s| s.read(cx).around().places.iter().find(|p| p["id"] == HERE).cloned());
        row.and_then(|r| serde_json::from_value(r).ok())
            .unwrap_or_else(|| serde_json::from_value(json!({ "id": HERE, "kind": "computer", "name": "", "default": true })).unwrap())
    }

    /// The places as Activity reads them.
    fn places(&self, cx: &App) -> Vec<(PlaceView, ComputerIcon)> {
        let Some(s) = self.sidebar.as_ref() else { return Vec::new() };
        s.read(cx)
            .around()
            .places
            .iter()
            .filter_map(|p| serde_json::from_value::<PlaceView>(p.clone()).ok())
            .map(|p| {
                let icon = ComputerIcon::of(&p, None);
                (p, icon)
            })
            .collect()
    }

    /// Builds what the pane reads once it knows what the workspace is: none for this computer's own panel, the record's
    /// kind from the sidebar for a thread's.
    fn settle_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.form.is_some() {
            return;
        }
        let (computer, thread) = match self.opened.workspace_id.clone() {
            None => (Computer::new(self.here_place(cx), ComputerIcon::of(&self.here_place(cx), None), None), false),
            Some(id) => {
                let Some(sidebar) = self.sidebar.clone() else { return };
                let s = sidebar.read(cx);
                let Some((ws, _)) = self.workspace(s) else { return };
                match ws["kind"].as_str() {
                    Some("local") => (Computer::new(self.here_place(cx), ComputerIcon::of(&self.here_place(cx), None), None), true),
                    kind => {
                        let row = s.place_of(&id).and_then(|at| s.around().places.iter().find(|p| p["id"] == at.as_str()).cloned());
                        let place = row.and_then(|r| serde_json::from_value::<PlaceView>(r).ok()).unwrap_or_else(|| {
                            serde_json::from_value(json!({ "id": id, "kind": "computer", "name": ws["name"], "default": false })).unwrap()
                        });
                        let icon = ComputerIcon::of(&place, None);
                        (Computer::new(place, icon, Some(id.clone())), kind == Some("place"))
                    }
                }
            }
        };
        let host = self.host.clone();
        let scoped = cx.new(|cx| Scoped::pane(computer, &host, !thread, window, cx));
        if let (true, Some(t)) = (thread, self.opened.thread_id.clone()) {
            scoped.update(cx, |s, _| _ = s.open.insert(t));
        }
        self._observed.push(cx.observe(&scoped, |this, _, cx| {
            this.follow_frame(cx);
            cx.notify();
        }));
        self.form = Some(if thread { Form::Thread(scoped) } else { Form::Computer(scoped) });
        self.lay_threads(cx);
    }

    /// The threads on the pane's computer, with the pids their turns recorded, and each one's newest turn.
    fn threads(&self, cx: &App) -> (Vec<Thread>, Latest) {
        let (Some(scoped), Some(listing)) = (self.scoped(), self.lists.read(cx).listing.as_ref()) else { return Default::default() };
        let computer = &scoped.read(cx).computer;
        let (mut threads, latests) =
            threads_on(listing, &self.places(cx), computer, computer.here() || matches!(self.form, Some(Form::Computer(_))));
        // A cloud's table lists its one machine, which no other workspace's pids name.
        if let (Some(id), Some(Form::Computer(_))) = (&self.opened.workspace_id, &self.form) {
            threads.retain(|t| &t.workspace_id == id);
        }
        (threads, latests)
    }

    /// The thread pane's threads laid on its computer's state, so its groups name them.
    fn lay_threads(&mut self, cx: &mut Context<Self>) {
        let Some(Form::Thread(scoped)) = &self.form else { return };
        let scoped = scoped.clone();
        let (threads, _) = self.threads(cx);
        scoped.update(cx, |s, cx| {
            s.set_threads(threads);
            s.ask_slates(cx);
        });
    }

    /// A frame came: the lit row goes with its process.
    fn follow_frame(&mut self, cx: &mut Context<Self>) {
        let Some(Form::Computer(scoped)) = &self.form else { return };
        let Some(lit) = self.session.lit else { return };
        let gone = scoped.read(cx).snapshot.as_ref().is_some_and(|snap| !snap.procs.iter().any(|p| p.pid == lit));
        if gone {
            self.light(None, cx);
        } else {
            self.inspect(cx);
        }
    }

    pub fn set_sort(&mut self, sort: Sort, cx: &mut Context<Self>) {
        self.session.sort = sort;
        self.save(cx);
        cx.notify();
    }

    pub fn set_filter(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.filter.update(cx, |f, cx| f.set_value(text.to_owned(), window, cx));
        self.session.filter = text.to_owned();
        self.save(cx);
        cx.notify();
    }

    /// Lights a row, or none; a press on the lit row puts it out. A newly lit row reads its inspect fields once.
    pub fn light(&mut self, pid: Option<Pid>, cx: &mut Context<Self>) {
        self.session.lit = pid;
        self.save(cx);
        self.lit = Lit { pid, ..Lit::default() };
        self.clock = None;
        cx.notify();
        self.inspect(cx);
    }

    /// Asks the daemon for the lit row's inspect fields once a frame lists it, as the web's `Details` asks once the row
    /// is drawn: a row lit again from the tab's session waits for the new link's first frame.
    fn inspect(&mut self, cx: &mut Context<Self>) {
        let (Some(pid), Some(Form::Computer(scoped))) = (self.lit.pid, &self.form) else { return };
        let listed = scoped.read(cx).snapshot.as_ref().is_some_and(|snap| snap.procs.iter().any(|p| p.pid == pid));
        if self.lit.asked || !listed {
            return;
        }
        self.lit.asked = true;
        let this = cx.weak_entity();
        scoped.update(cx, |s, cx| {
            // A refusal with no channel comes back at once, while the pane is still the one asking.
            s.daemon(json!({ "op": "proc.inspect", "pid": pid }), cx, move |_, answer, cx| {
                cx.defer(move |cx| {
                    _ = this.update(cx, |this, cx| {
                        if this.lit.pid == Some(pid) {
                            this.lit.inspect = Some(answer);
                            cx.notify();
                        }
                    });
                })
            })
        });
    }

    pub fn toggle(&mut self, pid: Pid, cx: &mut Context<Self>) {
        let next = (self.session.lit != Some(pid)).then_some(pid);
        self.light(next, cx);
    }

    fn now(cx: &App) -> std::time::Instant {
        cx.background_executor().now()
    }

    fn keep_clock(&mut self, cx: &mut Context<Self>) {
        if !self.lit.kill.ticks() {
            self.clock = None;
        } else if self.clock.is_none() {
            self.clock = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(TICK).await;
                let going = this.update(cx, |this, cx| {
                    let now = Processes::now(cx);
                    this.lit.kill = this.lit.kill.settle(now);
                    cx.notify();
                    this.lit.kill.ticks()
                });
                if !matches!(going, Ok(true)) {
                    _ = this.update(cx, |this, _| this.clock = None);
                    return;
                }
            }));
        }
    }

    /// The computer's name as an old daemon's update line says it.
    fn computer_name(&self, cx: &App) -> String {
        let Some(s) = self.sidebar.as_ref().map(|s| s.read(cx)) else { return String::new() };
        match &self.opened.workspace_id {
            None => s.here_name(),
            Some(id) => s
                .own_computer_name(id)
                .unwrap_or_else(|| self.workspace(s).and_then(|(ws, _)| ws["name"].as_str()).unwrap_or(id).to_owned()),
        }
    }

    /// `Kill`'s press: the first arms, the second within two seconds asks the daemon to end the process and what it
    /// started, checked against the row's identity and start; a daemon too old for that gets KILL once TERM did nothing.
    pub fn press_kill(&mut self, cx: &mut Context<Self>) {
        let (Some(pid), Some(Form::Computer(scoped))) = (self.lit.pid, &self.form) else { return };
        let scoped = scoped.clone();
        let Some(p) = scoped.read(cx).snapshot.as_ref().and_then(|s| s.procs.iter().find(|p| p.pid == pid)).cloned() else { return };
        let (next, send) = self.lit.kill.press(Processes::now(cx));
        self.lit.kill = next;
        self.keep_clock(cx);
        cx.notify();
        let Some(send) = send else { return };
        let (this, name) = (cx.weak_entity(), self.computer_name(cx));
        let tree = send == Send::End;
        if tree {
            self.lit.said = Some(w::ENDING.into());
        }
        let frame = kill_of(p.pid, Some(p.started_at), p.id.as_deref(), if tree { "TERM" } else { "KILL" }, tree);
        scoped.update(cx, |s, cx| {
            s.daemon(frame, cx, move |_, answer, cx| {
                cx.defer(move |cx| {
                    _ = this.update(cx, |this, cx| {
                        if this.lit.pid != Some(pid) {
                            return;
                        }
                        match answer {
                            Err(said) => {
                                this.lit.kill = Kill::Idle;
                                this.lit.said = Some(said);
                            }
                            Ok(_) if !tree => this.lit.said = Some(w::KILL_SENT.into()),
                            // A daemon from before kill modes answers with none of the reply's fields, having sent TERM alone.
                            Ok(reply) if reached_alone(&reply) => {
                                this.lit.kill = Kill::Termed { at: Processes::now(cx) };
                                this.lit.said = Some(w::too_old(&name));
                                this.keep_clock(cx);
                            }
                            Ok(reply) => this.lit.said = Some(tree::killed_word(&reply).into()),
                        }
                        cx.notify();
                    })
                });
            })
        });
    }

    /// A kept agent between turns: one press frees it through its thread, as `wsp activity end` does.
    pub fn end_kept(&mut self, cx: &mut Context<Self>) {
        let (Some(pid), Some(Form::Computer(scoped))) = (self.lit.pid, &self.form) else { return };
        let s = scoped.read(cx);
        let Some(started) = s.snapshot.as_ref().and_then(|snap| snap.procs.iter().find(|p| p.pid == pid)).map(|p| p.started_at) else {
            return;
        };
        let acts = s.voice.acts.clone();
        self.lit.said = Some(w::ENDING.into());
        cx.notify();
        let this = cx.weak_entity();
        acts.call(req::<ActivityEndRequest>(json!({ "pid": pid, "startedAt": started })), cx, move |answer, cx| {
            _ = this.update(cx, |this, cx| {
                if this.lit.pid == Some(pid) {
                    this.lit.said = Some(match answer {
                        Ok(ended) if ended.freed.is_some() => w::KEPT_FREED.into(),
                        Ok(_) => w::ENDED.into(),
                        Err(said) => said,
                    });
                    cx.notify();
                }
            });
        });
    }

    /// Asks before a running turn's agent is stopped through its thread; a signal would read as a failed turn.
    pub fn ask_stop(&mut self, agent: AgentOf, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = agent.session else { return };
        let pane = cx.weak_entity();
        let ask = cx.new(|_| StopAsk { title: agent.title, session, pane });
        layer::open(STOP_ASK, ask.into(), layer::Closes::Escape, None, window, cx);
    }

    pub fn stop_turn(&mut self, session: String, cx: &mut Context<Self>) {
        let (Some(pid), Some(scoped)) = (self.lit.pid, self.scoped()) else { return };
        let acts = scoped.read(cx).voice.acts.clone();
        self.lit.said = Some(w::STOPPING.into());
        cx.notify();
        let this = cx.weak_entity();
        acts.call(req::<SessionsInterruptRequest>(json!({ "sessionId": session })), cx, move |answer, cx| {
            _ = this.update(cx, |this, cx| {
                if this.lit.pid != Some(pid) {
                    return;
                }
                this.lit.said = Some(match answer {
                    Ok(r) => r.error.or(r.left).unwrap_or_else(|| {
                        if matches!(r.outcome, SessionInterruptResultOutcome::NotFound) { w::NO_TURN } else { w::STOPPED }.into()
                    }),
                    Err(said) => said,
                });
                cx.notify();
            });
        });
    }

    /// Why the pane's computer is not there, and the word its table's count says in place of a figure.
    fn away(&self, cx: &App) -> (Option<Absent>, Option<&'static str>) {
        let (Some(id), Some(sidebar)) = (&self.opened.workspace_id, &self.sidebar) else { return (None, None) };
        let s = sidebar.read(cx);
        let absent = s.absence(id);
        let live = self.scoped().is_some_and(|sc| sc.read(cx).channel.is_some());
        let stale = absent.as_ref().map(|a| a.away).or_else(|| {
            let (ws, status) = self.workspace(s)?;
            let phase = status.and_then(|st| st["phase"].as_str()).or_else(|| ws["phase"].as_str()).unwrap_or_default();
            stale_word(phase, live).map(|w| w.word())
        });
        (absent, stale)
    }

    /// The thread and every thread under it, its children first.
    fn tree_of(&self, threads: &[Thread]) -> Vec<String> {
        let Some(root) = self.opened.thread_id.clone() else { return Vec::new() };
        let mut ids = vec![root];
        let mut at = 0;
        while at < ids.len() {
            for t in threads {
                if t.parent.as_deref() == Some(ids[at].as_str()) && !ids.contains(&t.id) {
                    ids.push(t.id.clone());
                }
            }
            at += 1;
        }
        ids
    }

    /// What the thread pane draws now.
    pub fn thread_state(&self, cx: &App) -> Option<ThreadState> {
        let Some(Form::Thread(scoped)) = &self.form else { return None };
        let s = scoped.read(cx);
        if let (Some(absent), _) = self.away(cx) {
            return Some(ThreadState::Absent(absent));
        }
        if s.snapshot.is_none() {
            if let Some(said) = &s.unavailable {
                return Some(ThreadState::Refused(said.clone()));
            }
        }
        let Some(scope) = &s.scope else { return Some(ThreadState::Pending) };
        let (threads, _) = self.threads(cx);
        let tree = self.tree_of(&threads);
        let mine: Vec<Group> = scope.groups.iter().filter(|g| g.kind == GroupKind::Thread && tree.contains(&g.id)).cloned().collect();
        let own = self.opened.thread_id.as_deref();
        let mut groups: Vec<Group> = mine
            .iter()
            .filter(|g| Some(g.id.as_str()) == own)
            .cloned()
            .map(|mut g| {
                crate::screens::settings::pages::activity::model::sort_procs(&mut g.procs, s.sort);
                g
            })
            .collect();
        groups.extend(crate::screens::settings::pages::activity::model::sort_groups(
            mine.into_iter().filter(|g| Some(g.id.as_str()) != own).collect(),
            s.sort,
        ));
        Some(if groups.is_empty() { ThreadState::Nothing } else { ThreadState::Groups(groups) })
    }

    /// Each thread's head mark, off its newest turn.
    fn looks(&self, cx: &App) -> HashMap<String, crate::ui::Status> {
        let now = chrono::Utc::now().timestamp_millis() as f64;
        self.threads(cx).1.iter().map(|(id, latest)| (id.clone(), status_of(latest, now))).collect()
    }

    /// Open in Activity: Settings on Activity, scoped to this pane's computer.
    pub fn open_activity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(place) = self.scoped().map(|s| s.read(cx).computer.place_id().to_owned()) {
            let pick = crate::screens::settings::pages::activity::kept(window, cx);
            pick.update(cx, |p, cx| {
                p.pick_place(place);
                cx.notify();
            });
        }
        crate::screens::navigate(crate::screens::Route::Screen("settings:activity".into()), cx);
    }
}

impl Screen for Processes {
    const NAME: &'static str = "processes";
    const PLACE: Place = Place::Panel(Pane::Processes);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let session = cx.default_global::<Sessions>().0.get(&tab_of(&opened)).cloned().unwrap_or_default();
        let acts =
            crate::screens::settings::Acts::new(gpui_kit::WeakEntity::new_invalid(), Some(host.client().clone()), host.runtime().clone());
        let lists = cx.new(|cx| Lists::new(&acts, cx));
        let held = session.filter.clone();
        let filter = cx.new(|cx| {
            let state = InputState::new(window, cx).placeholder(w::FILTER);
            state.default_value(held)
        });
        let observed = vec![
            cx.observe(&lists, |this, _, cx| {
                this.lay_threads(cx);
                cx.notify();
            }),
            cx.subscribe(&filter, |this: &mut Processes, field, event: &InputEvent, cx| {
                if let InputEvent::Change = event {
                    this.session.filter = field.read(cx).value().to_string();
                    this.save(cx);
                    cx.notify();
                }
            }),
        ];
        let lit = session.lit;
        let mut pane = Processes {
            host,
            opened,
            session,
            sidebar: None,
            lists,
            form: None,
            filter,
            scroll: ScrollHandle::new(),
            lit: Lit::default(),
            clock: None,
            restart: cx.new(|_| Restart::default()),
            width: Rc::new(std::cell::Cell::new(crate::screens::app_shell::panel::DEFAULT_WIDTH)),
            _observed: observed,
        };
        pane.find_sidebar(window, cx);
        pane.settle_form(window, cx);
        if lit.is_some() {
            pane.light(lit, cx);
        }
        pane
    }

    /// The tab closed for good: what the person set in it goes.
    fn closed(_host: &Host, opened: Opened, cx: &mut App) {
        if cx.has_global::<Sessions>() {
            cx.global_mut::<Sessions>().0.remove(&tab_of(&opened));
        }
    }
}

impl Render for Processes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.form {
            Some(Form::Thread(_)) => thread::view(self, window, cx).into_any_element(),
            Some(Form::Computer(_)) => table::view(self, window, cx).into_any_element(),
            None => gpui_kit::div().debug_selector(|| "procs-waiting".into()).size_full().into_any_element(),
        }
    }
}

/// `Stop <title>?`: the running turn's agent is stopped through its thread, its turn read as interrupted.
struct StopAsk {
    title: String,
    session: String,
    pane: gpui_kit::WeakEntity<Processes>,
}

impl Render for StopAsk {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (pane, session) = (self.pane.clone(), self.session.clone());
        gpui_kit::div().debug_selector(|| "procs-stop-ask".into()).child(
            crate::ui::AlertDialog::new(w::stop_title(&self.title))
                .layered()
                .sentence(w::STOP_SENTENCE)
                .act(
                    crate::ui::Button::outline("procs-stop-cancel", w::CANCEL)
                        .neutral_ring()
                        .on_click(|_, window, cx| layer::close(STOP_ASK, window, cx)),
                )
                .act(crate::ui::Button::destructive("procs-stop-confirm", w::STOP).on_click(move |_, window, cx| {
                    layer::close(STOP_ASK, window, cx);
                    let session = session.clone();
                    _ = pane.update(cx, |p, cx| p.stop_turn(session, cx));
                })),
        )
    }
}

/// The thread table's widths as the web's `narrow` reads them, a window under 768 px or a pane under 640; a tag's
/// `max-sm:hidden` reads the window alone.
fn width_of(pane: &Processes, window: &Window) -> crate::ui::Width {
    let page = crate::ui::narrow::width(window);
    crate::ui::Width { sm: page.sm, md: page.md || crate::ui::Width::at(pane.width.get()).sm }
}

/// Keeps `cell` at the width the element it sits in was laid out at, drawing again when a breakpoint moved.
fn measure(cell: Rc<std::cell::Cell<Pixels>>) -> impl IntoElement {
    gpui_kit::canvas(
        move |bounds, window, _| {
            let was = crate::ui::Width::at(cell.get());
            cell.set(bounds.size.width);
            if crate::ui::Width::at(bounds.size.width) != was {
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
}
