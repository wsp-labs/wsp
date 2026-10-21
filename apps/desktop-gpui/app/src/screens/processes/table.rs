// SPDX-License-Identifier: AGPL-3.0-only
//! The computer table (`ComputerProcesses` in `ProcessesSurface.tsx`): a filter and the count of the rows on screen, a
//! head that sorts by cpu or memory, the rows in mono 22 px apiece nested by parent with the harness pieces named, and
//! the lit row's inspect fields over its one act and what that act said.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Entity, FocusHandle, Pixels, SharedString, StyleRefinement, WeakEntity, Window,
};
use serde_json::Value;

use super::tree::{self, words as w, AgentOf, Send, Sort};
use super::{measure, Form, Processes};
use crate::screens::computer::daemon_down::DaemonDown;
use crate::screens::settings::pages::activity::wire::{Pid, ProcEntry};
use crate::screens::terminal::link::{Link, Target};
use crate::screens::terminal::state::Absent;
use crate::ui::{scroll_area, text, ActivePalette, Button};

/// Every row is this tall, so the table reads as a grid and a selection moves nothing.
pub const ROW: Pixels = px(22.);
/// `text-[11px]` over the whole pane.
const TYPE: (Pixels, Pixels) = (px(11.), px(16.5));
/// `grid-cols-[3.5rem_3.25rem_3.75rem_minmax(0,1fr)]`.
const PID_W: Pixels = px(56.);
const CPU_W: Pixels = px(52.);
const MEM_W: Pixels = px(60.);

/// One row as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct RowShown {
    pub pid: Pid,
    pub depth: usize,
    pub label: Option<String>,
    pub command: String,
    pub cpu: String,
    pub mem: String,
}

/// The lit row's one act.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    /// Kill, its word and whether the next press confirms; none while the daemon works or TERM waits.
    Kill(Option<(String, bool)>),
    /// A running turn's agent: its thread's stop, asked first.
    Stop(AgentOf),
    /// A kept agent between turns on this computer.
    End,
    /// A turn's agent whose thread cannot be named.
    Unnamed,
}

/// The lit row as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct LitShown {
    pub pid: Pid,
    pub fields: Vec<(&'static str, String)>,
    pub act: Act,
    pub said: Option<String>,
}

/// Everything the table draws.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub count: String,
    pub stale: bool,
    pub absent: Option<Absent>,
    pub unavailable: Option<String>,
    pub sort: Sort,
    pub rows: Vec<RowShown>,
    pub lit: Option<LitShown>,
}

impl Shown {
    /// Whether `other` lays the pane out as this does: everything but what its rows say, which their slots draw.
    fn lays_as(&self, other: &Shown) -> bool {
        let lit_at = |s: &Shown| s.lit.as_ref().and_then(|l| s.rows.iter().position(|r| r.pid == l.pid));
        (&self.count, self.stale, &self.absent, &self.unavailable, self.sort, self.rows.len(), &self.lit, lit_at(self))
            == (&other.count, other.stale, &other.absent, &other.unavailable, other.sort, other.rows.len(), &other.lit, lit_at(other))
    }
}

/// Each row's focus, kept by its process wherever the sort puts it.
pub(super) type Focuses = Rc<RefCell<HashMap<Pid, FocusHandle>>>;

/// The table's rows by their places: a frame that moves only what rows say draws again, in place, the slots it moved.
#[derive(Default)]
pub(super) struct Slots {
    rows: Vec<Entity<RowSlot>>,
    focuses: Focuses,
    drawn: Option<Shown>,
}

impl Slots {
    /// Lays the rows on their slots as the pane draws them all, making or letting go of slots for the rows' count.
    fn lay(&mut self, shown: &Shown, pane: WeakEntity<Processes>, cx: &mut App) {
        let lit = shown.lit.as_ref().map(|l| l.pid);
        self.rows.truncate(shown.rows.len());
        for (ix, row) in shown.rows.iter().enumerate() {
            let at = (row.clone(), lit == Some(row.pid));
            match self.rows.get(ix) {
                Some(slot) => slot.update(cx, |slot, _| slot.row = at),
                None => {
                    let focuses = self.focuses.clone();
                    let pane = pane.clone();
                    self.rows.push(cx.new(|_| RowSlot { row: at, pane, focuses }));
                }
            }
        }
        self.focuses.borrow_mut().retain(|pid, _| shown.rows.iter().any(|r| r.pid == *pid));
        self.drawn = Some(shown.clone());
    }
}

impl Processes {
    /// A frame came: rows that only say something new draw again in their slots; anything else lays the pane out again.
    pub(super) fn relay_rows(&mut self, cx: &mut Context<Self>) {
        let Some(shown) = self.table(cx) else { return };
        if !self.slots.drawn.as_ref().is_some_and(|drawn| drawn.lays_as(&shown)) {
            return cx.notify();
        }
        let lit = shown.lit.as_ref().map(|l| l.pid);
        for (slot, row) in self.slots.rows.iter().zip(&shown.rows) {
            let at = (row.clone(), lit == Some(row.pid));
            slot.update(cx, |slot, cx| {
                if slot.row != at {
                    slot.row = at;
                    cx.notify_in_place();
                }
            });
        }
        self.slots.drawn = Some(shown);
    }
}

/// `terminalLabels`: each shell the computer's terminal holds by its pty, as its tab names it.
fn titles(pane: &Processes, cx: &App) -> HashMap<String, String> {
    let target = pane.opened.workspace_id.clone().map_or(Target::Here, Target::Workspace);
    let Some(link) = Link::find(&target, cx) else { return HashMap::new() };
    link.read(cx)
        .ptys()
        .iter()
        .map(|pty| {
            let s = pty.session.read(cx);
            let title = if s.exited().is_some() { format!("{} (exited)", s.title()) } else { s.title().to_string() };
            (s.pty().to_string(), title)
        })
        .collect()
}

fn command_of(p: &ProcEntry) -> String {
    if p.cmdline.is_empty() {
        format!("[{}]", p.comm)
    } else {
        p.cmdline.clone()
    }
}

/// `clockLabel`: the start in the person's zone, hours and minutes.
fn clock(ms: f64) -> String {
    use chrono::TimeZone as _;
    chrono::Local.timestamp_millis_opt(ms as i64).single().map_or_else(String::new, |t| crate::words::clock_label(&t))
}

fn list(v: Option<&Value>) -> String {
    match v.and_then(Value::as_array) {
        None => String::new(),
        Some(a) if a.is_empty() => w::NONE.into(),
        Some(a) => a.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(" "),
    }
}

impl Processes {
    /// What the computer table draws now, read off its computer's state and what the person set.
    pub fn table(&self, cx: &App) -> Option<Shown> {
        let Some(Form::Computer(scoped)) = &self.form else { return None };
        let s = scoped.read(cx);
        let (absent, stale) = self.away(cx);
        let snapshot = s.snapshot.as_ref();
        let procs = snapshot.map_or(&[][..], |snap| snap.procs.as_slice());
        let rows = tree::table(procs, self.session.sort, &self.session.filter);
        let count = match snapshot {
            None => w::PENDING.to_owned(),
            Some(snap) => w::count(rows.len(), snap.total),
        };
        let unavailable = s.unavailable.clone().filter(|_| stale.is_none() && snapshot.is_none());
        let count = stale.map(str::to_owned).unwrap_or_else(|| if unavailable.is_some() { w::UNAVAILABLE.to_owned() } else { count });
        let here = self.opened.workspace_id.is_none();
        let titles = titles(self, cx);
        let none = HashMap::new();
        let app = if here { &s.app } else { &none };
        let daemon = snapshot.map_or(-1, |snap| snap.daemon);
        let shown: Vec<RowShown> = rows
            .iter()
            .map(|r| RowShown {
                pid: r.proc.pid,
                depth: r.depth,
                label: tree::label(r.proc, daemon, &titles, app),
                command: command_of(r.proc),
                cpu: format!("{:.1}", r.proc.cpu),
                mem: tree::compact_bytes(r.proc.rss),
            })
            .collect();
        let lit = self.lit.pid.and_then(|pid| procs.iter().find(|p| p.pid == pid)).map(|p| self.lit_shown(p, procs, here, cx));
        Some(Shown { count, stale: stale.is_some(), absent, unavailable, sort: self.session.sort, rows: shown, lit })
    }

    fn lit_shown(&self, p: &ProcEntry, procs: &[ProcEntry], here: bool, cx: &App) -> LitShown {
        let detail = self.lit.inspect.as_ref().and_then(|a| a.as_ref().ok());
        let mut fields = vec![
            ("user", p.user.clone()),
            ("state", p.state.clone()),
            ("started", clock(p.started_at)),
            ("threads", detail.and_then(|d| d.get("threads")).and_then(Value::as_i64).map(|n| n.to_string()).unwrap_or_default()),
            ("cwd", detail.map(|d| d.get("cwd").and_then(Value::as_str).unwrap_or(w::UNREADABLE).to_owned()).unwrap_or_default()),
            ("ports", list(detail.and_then(|d| d.get("ports")))),
            ("children", list(detail.and_then(|d| d.get("children")))),
            ("command", command_of(p)),
        ];
        if let Some(Err(said)) = &self.lit.inspect {
            fields.push(("inspect", said.clone()));
        }
        let (threads, _) = self.threads(cx);
        let agent = tree::agent_rows(procs, &threads).remove(&p.pid).filter(|a| a.session.is_some());
        let run_agent = tree::run_agent_pids(procs).contains(&p.pid);
        let act = match agent {
            Some(agent) => Act::Stop(agent),
            None if run_agent && here => Act::End,
            None if run_agent => Act::Unnamed,
            None => Act::Kill(self.lit.kill.offered(Processes::now(cx)).map(|offer| {
                let word = if offer.send == Send::Force { w::KILL_FORCE } else { w::KILL };
                (if offer.confirm { w::confirm(word) } else { word.to_owned() }, offer.confirm)
            })),
        };
        LitShown { pid: p.pid, fields, act, said: self.lit.said.clone() }
    }
}

fn sort_head(word: &'static str, sort: Sort, on: bool, cx: &mut Context<Processes>) -> AnyElement {
    let p = cx.palette();
    div()
        .id(word)
        .debug_selector(move || format!("procs-sort-{word}"))
        .w(if sort == Sort::Cpu { CPU_W } else { MEM_W })
        .flex_none()
        .text_right()
        .cursor_pointer()
        .text_color(if on { p.foreground } else { p.muted_foreground })
        .hover(|s| s.text_color(p.foreground))
        .on_click(cx.listener(move |this, _, _, cx| this.set_sort(sort, cx)))
        .child(word)
        .into_any_element()
}

/// One place in the table, drawing whichever row stands there now.
pub struct RowSlot {
    row: (RowShown, bool),
    pane: WeakEntity<Processes>,
    focuses: Focuses,
}

impl Render for RowSlot {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("ProcRow");
        let (r, lit) = self.row.clone();
        // Each row is a tab stop, as the web's `tabIndex={0}`, kept for as long as the row is drawn.
        let focus = self.focuses.borrow_mut().entry(r.pid).or_insert_with(|| cx.focus_handle().tab_stop(true)).clone();
        row(r, lit, focus, self.pane.clone(), window, cx)
    }
}

fn row(r: RowShown, lit: bool, focus: FocusHandle, pane: WeakEntity<Processes>, window: &mut Window, cx: &App) -> AnyElement {
    let p = cx.palette().clone();
    let pid = r.pid;
    let ringed = !lit && focus.is_focused(window) && window.last_input_was_keyboard();
    let keyed = pane.clone();
    h_flex()
        .id(SharedString::from(format!("proc-{pid}")))
        .debug_selector(move || format!("proc-row-{pid}"))
        .track_focus(&focus)
        .h(ROW)
        .flex_none()
        .px_2()
        .cursor_default()
        .when(lit, |el| el.bg(p.accent).text_color(p.foreground))
        .when(ringed, |el| el.bg(p.accent.opacity(0.4)))
        .when(!lit, |el| el.hover(|s| s.bg(p.accent.opacity(0.4))))
        .on_click(move |_, _, cx| _ = pane.update(cx, |this, cx| this.toggle(pid, cx)))
        .on_key_down(move |e: &gpui_kit::KeyDownEvent, _, cx| {
            if matches!(e.keystroke.key.as_str(), "enter" | "space") {
                cx.stop_propagation();
                _ = keyed.update(cx, |this, cx| this.toggle(pid, cx));
            }
        })
        .child(div().w(PID_W).flex_none().text_right().when(!lit, |el| el.text_color(p.muted_foreground)).child(r.pid.to_string()))
        .child(div().w(CPU_W).flex_none().text_right().child(r.cpu))
        .child(div().w(MEM_W).flex_none().text_right().child(r.mem))
        .child(
            h_flex()
                .min_w_0()
                .flex_1()
                .pl(px(8. + r.depth as f32 * 12.))
                .children(
                    r.label.map(|l| {
                        div().debug_selector(|| "proc-label".into()).flex_none().mr(px(6.)).text_color(p.muted_foreground).child(l)
                    }),
                )
                // `truncate` on a box that may shrink, so a long command ends in an ellipsis at the row's edge.
                .child(div().debug_selector(move || format!("proc-command-{pid}")).min_w_0().truncate().child(r.command)),
        )
        .into_any_element()
}

/// The pane's one act at the table's height: quiet until it is the confirming press.
fn act_button(id: &'static str, word: String, confirm: bool, held: bool) -> Button {
    let button = if confirm { Button::destructive(id, word) } else { Button::danger(id, word) };
    button.xs().height(px(20.)).flat().held(held)
}

fn details(lit: LitShown, stale: bool, cx: &mut Context<Processes>) -> AnyElement {
    let p = cx.palette().clone();
    let pid = lit.pid;
    let busy = matches!(lit.said.as_deref(), Some(w::ENDING | w::STOPPING));
    let act: Option<AnyElement> = match lit.act {
        Act::Kill(offer) => offer.map(|(word, confirm)| {
            act_button("procs-kill", word, confirm, stale).on_click(cx.listener(|this, _, _, cx| this.press_kill(cx))).into_any_element()
        }),
        Act::End => Some(
            act_button("procs-end", w::END.into(), false, stale || busy)
                .on_click(cx.listener(|this, _, _, cx| this.end_kept(cx)))
                .into_any_element(),
        ),
        Act::Stop(agent) => Some(
            act_button("procs-stop", w::STOP_TURN.into(), false, stale || busy)
                .on_click(cx.listener(move |this, _, window, cx| this.ask_stop(agent.clone(), window, cx)))
                .into_any_element(),
        ),
        Act::Unnamed => Some(
            div()
                .debug_selector(|| "procs-agent-unnamed".into())
                .truncate()
                .text_color(p.muted_foreground)
                .child(w::AGENT_UNNAMED)
                .into_any_element(),
        ),
    };
    let said =
        lit.said.map(|said| div().debug_selector(|| "procs-said".into()).min_w_0().truncate().text_color(p.muted_foreground).child(said));
    let fields = lit.fields.into_iter().map(|(k, v)| {
        h_flex()
            .gap_2()
            .child(div().w(px(72.)).flex_none().text_color(p.muted_foreground).child(k))
            .child(div().debug_selector(move || format!("procs-field-{k}")).min_w_0().flex_1().truncate().text_color(p.foreground).child(v))
    });
    v_flex()
        .debug_selector(move || format!("proc-details-{pid}"))
        .border_y_1()
        .border_color(p.border.opacity(0.5))
        .bg(p.accent.opacity(0.3))
        .px_2()
        .py(px(6.))
        .child(v_flex().gap(px(2.)).children(fields))
        .child(h_flex().mt(px(6.)).h(px(20.)).gap_2().children(act).children(said))
        .into_any_element()
}

fn absent_lines(pane: &Processes, absent: Absent, cx: &App) -> AnyElement {
    let p = cx.palette();
    let id = pane.opened.workspace_id.clone().unwrap_or_default();
    let start = absent.start.clone().map(SharedString::from);
    v_flex()
        .debug_selector(|| "procs-absent".into())
        .items_start()
        .gap_2()
        .px_2()
        .py_2()
        .font_family(cx.theme().font_family.clone())
        .text_size(text::FACT.0)
        .line_height(text::FACT.1)
        .text_color(p.muted_foreground)
        .child(absent.said.clone())
        .children(absent.will.clone())
        .child(DaemonDown::new(pane.host.clone(), id, absent.said, start).button_only().state(pane.restart.clone()))
        .into_any_element()
}

pub fn view(pane: &mut Processes, window: &mut Window, cx: &mut Context<Processes>) -> impl IntoElement {
    let Some(shown) = pane.table(cx) else { return div().into_any_element() };
    pane.keep_clock(cx);
    pane.slots.lay(&shown, cx.weak_entity(), cx);
    let p = cx.palette().clone();
    let mono = cx.theme().mono_font_family.clone();
    let lit = shown.lit.as_ref().map(|l| l.pid);
    let mut body: Vec<AnyElement> = Vec::new();
    match (shown.absent.clone(), shown.unavailable.clone()) {
        (Some(absent), _) => body.push(absent_lines(pane, absent, cx)),
        (None, Some(said)) => body.push(
            div()
                .debug_selector(|| "procs-unavailable".into())
                .px_2()
                .truncate()
                .font_family(cx.theme().font_family.clone())
                .text_size(text::FACT.0)
                .line_height(ROW)
                .text_color(p.muted_foreground)
                .child(said)
                .into_any_element(),
        ),
        (None, None) => {}
    }
    let mut details_of = shown.lit.clone();
    for (slot, r) in pane.slots.rows.iter().zip(&shown.rows) {
        let pid = r.pid;
        body.push(slot.clone().cached(StyleRefinement::default().w_full().h(ROW).flex_none()).into_any_element());
        if lit == Some(pid) {
            if let Some(l) = details_of.take() {
                body.push(details(l, shown.stale, cx));
            }
        }
    }
    let table = v_flex().debug_selector(|| "procs-table".into()).when(shown.stale, |el| el.text_color(p.muted_foreground)).children(body);
    let filter = Input::new(&pane.filter).appearance(false).px_0().h(px(24.)).text_size(TYPE.0);
    v_flex()
        .debug_selector(|| "procs".into())
        .relative()
        .size_full()
        .min_h_0()
        .font_family(mono)
        .text_size(TYPE.0)
        .line_height(TYPE.1)
        .child(
            h_flex()
                .h(px(32.))
                .flex_none()
                .gap_2()
                .px_2()
                .border_b_1()
                .border_color(p.border.opacity(0.5))
                .child(div().debug_selector(|| "procs-filter".into()).min_w_0().flex_1().child(filter))
                .child(div().debug_selector(|| "procs-count".into()).flex_none().text_color(p.muted_foreground).child(shown.count.clone())),
        )
        .child(
            h_flex()
                .debug_selector(|| "procs-head".into())
                .h(px(24.))
                .flex_none()
                .px_2()
                .border_b_1()
                .border_color(p.border.opacity(0.5))
                .text_size(text::XS.0)
                .text_color(p.muted_foreground)
                .child(div().w(PID_W).flex_none().text_right().child("pid"))
                .child(sort_head("cpu", Sort::Cpu, shown.sort == Sort::Cpu, cx))
                .child(sort_head("mem", Sort::Mem, shown.sort == Sort::Mem, cx))
                .child(div().pl_2().child("command")),
        )
        .child(scroll_area("procs-scroll", &pane.scroll, table, window, cx).flex_1().min_h_0())
        .child(measure(pane.width.clone()))
        .into_any_element()
}
