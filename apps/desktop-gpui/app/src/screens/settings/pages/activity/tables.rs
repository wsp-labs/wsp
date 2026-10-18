// SPDX-License-Identifier: AGPL-3.0-only
//! Activity's four views over one computer's groups, after `activity/tables.tsx`: a group row folds open onto its
//! processes, each with its kind's glyph, its figures inked only past their marks, and its acts in its menu.

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, ClipboardItem, Corners, Hsla, MouseButton, Pixels, Point, SharedString, WeakEntity, Window,
};

use super::model::{
    cpu_tint, cpu_word, energy_tint, energy_word, flat, fmt_uptime, group_spent, groups_under, memory_tint, nest_groups, ports_word, spent,
    total_spent, words as w, ContainerRow, Group, GroupKind, Placed, Port, Proc, Sort, Spent, Subagent, Tint,
};
use super::pieces::{cells, COLUMN_GAP};
use super::wire::{Kind, Pid, PlacedBy};
use super::Scoped;
use crate::screens::app_shell::{navigate, Route};
use crate::screens::settings::words::{fmt_duration, fmt_tokens};
use crate::ui::catalog;
use crate::ui::context_menu::MenuItem;
use crate::ui::fmt_bytes;
use crate::ui::grid::{
    ACTIVITY_COLUMNS, ACTIVITY_COLUMNS_NARROW as NARROW, ACTIVITY_FLAT_COLUMNS as FLAT, ACTIVITY_PORT_COLUMNS as PORTS,
    ACTIVITY_PORT_COLUMNS_NARROW as PORTS_NARROW,
};
use crate::ui::{
    framed, glyph, spinner, text, ActivePalette, Button, Card, CardItem, Col, Glyph, GridRow, Line, MenuAct, Run, Status, StatusMark, Width,
};

const TREE: &[Col] = ACTIVITY_COLUMNS;

/// The page's state as a row reads it and acts on it.
#[derive(Clone)]
pub struct Hand {
    pub scoped: WeakEntity<Scoped>,
    pub ending: Rc<HashSet<Pid>>,
    pub stopping: Rc<HashSet<String>>,
    pub open: Rc<HashSet<String>>,
    pub sort: Sort,
    pub energy: bool,
    pub filtering: bool,
    /// Under 768 px (`narrow:`) a row keeps its name, memory and menu, and under 640 its tags go (`max-sm:hidden`).
    pub width: Width,
    /// Where a port of a row with no thread opens: the box's folder the page reads it over, none on this computer.
    pub workspace: Option<String>,
}

/// The menu's groups by a `MenuAct`'s number: the row's acts, where it goes, and its copies.
const GROUPS: [&str; 3] = ["act", "go", "copy"];

/// A row's acts as the right-click menu lists them: no glyphs, a held act carrying its refusal.
pub fn menu_items(acts: &[MenuAct]) -> Vec<MenuItem> {
    acts.iter()
        .map(|a| MenuItem {
            label: a.title.clone(),
            group: GROUPS[usize::from(a.group).min(GROUPS.len() - 1)],
            refusal: a.held.then(|| a.why.clone().unwrap_or_default()),
            keys: None,
            destructive: a.destructive,
        })
        .collect()
}

impl Hand {
    fn with(&self, cx: &mut App, f: impl FnOnce(&mut Scoped, &mut gpui_kit::Context<Scoped>)) {
        if let Some(s) = self.scoped.upgrade() {
            s.update(cx, f);
        }
    }

    fn run(&self, f: impl Fn(&mut Scoped, &mut gpui_kit::Context<Scoped>) + 'static) -> crate::ui::Run {
        let hand = self.clone();
        Rc::new(move |_: &mut Window, cx: &mut App| hand.with(cx, |s, cx| f(s, cx)))
    }

    fn tree(&self) -> &'static [Col] {
        if self.width.md {
            NARROW
        } else {
            TREE
        }
    }

    fn flat(&self) -> &'static [Col] {
        if self.width.md {
            NARROW
        } else {
            FLAT
        }
    }

    fn ports(&self) -> &'static [Col] {
        if self.width.md {
            PORTS_NARROW
        } else {
            PORTS
        }
    }

    /// A row's name under a group: `ps-[calc(2.75rem+var(--depth)*1.25rem)]`, `narrow:ps-[calc(0.75rem+var(--depth)*0.75rem)]`.
    fn lead_pad(&self, depth: usize) -> f32 {
        if self.width.md {
            12. + depth as f32 * 12.
        } else {
            44. + depth as f32 * 20.
        }
    }

    /// A head's name: `ps-[calc(var(--nest)*1.25rem)]`, `narrow:ps-[calc(var(--nest)*0.75rem)]`.
    fn nest_pad(&self, nest: usize) -> f32 {
        nest as f32 * if self.width.md { 12. } else { 20. }
    }

    /// A tag that `max-sm:hidden` takes under 640 px.
    fn tag<T>(&self, tag: Option<T>) -> Option<T> {
        tag.filter(|_| !self.width.sm)
    }

    /// `openContextMenu`: the row's acts at `at`, the native menu on the Mac and the page's own elsewhere; a pick runs
    /// once the menu has let go of the page.
    pub fn open_menu(&self, at: Point<Pixels>, acts: Rc<Vec<MenuAct>>, window: &mut Window, cx: &mut App) {
        let Some(scoped) = self.scoped.upgrade() else { return };
        let items = menu_items(&acts);
        scoped.update(cx, |s, cx| {
            s.menu.open(at, items, window, cx, move |_, pick, window, cx| {
                let Some(run) = pick.and_then(|ix| acts.get(ix)).filter(|a| !a.held).map(|a| a.run.clone()) else { return };
                window.defer(cx, move |window, cx| run(window, cx));
            })
        });
    }

    /// The workspace a browser tab at a port of this group's opens in, where one is known.
    fn workspace_of(&self, group: Option<&Group>) -> Option<String> {
        group.and_then(|g| g.thread.as_ref()).map(|t| t.workspace_id.clone()).or_else(|| self.workspace.clone())
    }
}

pub fn kind_glyph(kind: Kind) -> Glyph {
    match kind {
        Kind::Agent => Glyph::Bot,
        Kind::Tool => Glyph::Server,
        Kind::Dev => Glyph::EthernetPort,
        Kind::Test => Glyph::FlaskConical,
        Kind::Build => Glyph::Hammer,
        Kind::Slate => Glyph::PanelsTopLeft,
        Kind::Terminal => Glyph::SquareTerminal,
        Kind::Tab => Glyph::Earth,
        Kind::App => Glyph::AppWindow,
        Kind::Wsp => Glyph::Cog,
        Kind::Other => Glyph::CircleDashed,
    }
}

/// Warn and danger take the design's caution and error inks; a figure under its mark keeps the row's own.
pub fn tint_ink(tint: Option<Tint>, cx: &App) -> Hsla {
    let p = cx.palette();
    match tint {
        Some(Tint::Warn) => p.warning_foreground,
        Some(Tint::Danger) => p.error_foreground,
        None => p.foreground,
    }
}

fn copy(text: String) -> crate::ui::Run {
    Rc::new(move |_: &mut Window, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

/// The port a row's Open reaches: a Ports row's own, else its process's first; none where no workspace is known.
fn open_port(p: &Proc, group: Option<&Group>, port: Option<Port>, hand: &Hand) -> Option<(i64, Run)> {
    if p.heading {
        return None;
    }
    let port = port.or_else(|| p.ports.first().copied())?.port;
    hand.workspace_of(group)?;
    // The in-app browser lands later (D78): until then a port opens in the system browser.
    let url = format!("http://localhost:{port}/");
    Some((port, Rc::new(move |_: &mut Window, cx: &mut App| cx.open_url(&url))))
}

/// `procActions`: what a process row's menu offers. A working agent is stopped through its thread; a kept one between
/// turns is freed; only a process a thread or wsp started is ended; a heading's one act is End all on its runs. A row
/// with a thread opens it.
fn proc_acts(p: &Proc, group: Option<&Group>, port: Option<Port>, hand: &Hand) -> Vec<MenuAct> {
    let busy = hand.ending.contains(&p.pid);
    let thread = group.and_then(|g| g.thread.as_ref());
    let mut acts = Vec::new();
    let act = |glyph: Glyph, title: &str, destructive: bool, held: bool, run| MenuAct {
        group: 0,
        glyph: Some(glyph),
        title: title.to_owned().into(),
        destructive,
        held,
        why: held.then(|| w::ENDING.into()),
        keys: None,
        run,
    };
    if let Some((port, run)) = open_port(p, group, port, hand) {
        acts.push(act(Glyph::Earth, &w::open_port(port), false, false, run));
    }
    if !p.heading && p.kind == Kind::Agent && !p.needed && thread.is_some_and(|t| t.working) {
        let g = group.cloned().unwrap();
        acts.push(act(Glyph::CircleStop, w::STOP_TURN, false, busy, hand.run(move |s, cx| s.ask_stop(&g, cx))));
    }
    if !p.heading && p.endable {
        let q = p.clone();
        acts.push(act(Glyph::SquareX, w::END, true, busy, hand.run(move |s, cx| s.end(&q, cx))));
    }
    if !p.heading && p.kind == Kind::Agent && !p.needed && thread.is_some_and(|t| !t.working && t.cgroup.is_none()) {
        let q = p.clone();
        acts.push(act(Glyph::SquareX, w::END, true, busy, hand.run(move |s, cx| s.free(&q, cx))));
    }
    if p.heading && group.is_some() && p.children.iter().any(|c| c.endable) {
        let (q, g) = (p.clone(), group.cloned().unwrap());
        acts.push(act(Glyph::SquareX, w::END_ALL, true, false, hand.run(move |s, cx| s.ask_end_all(&q, &g, cx))));
    }
    acts.extend(thread.map(open_thread));
    if !p.heading && !p.command.is_empty() {
        acts.push(MenuAct { group: 2, ..act(Glyph::ClipboardCopy, w::COPY_COMMAND, false, false, copy(p.command.clone())) });
    }
    if !p.heading {
        acts.push(MenuAct { group: 2, ..act(Glyph::ClipboardCopy, w::COPY_PID, false, false, copy(p.pid.to_string())) });
    }
    acts
}

/// `groupActions`: what a thread's head offers, its right click's menu and the narrow head's button alike.
pub fn thread_head_acts(t: &super::model::Thread) -> Rc<Vec<MenuAct>> {
    Rc::new(vec![open_thread(t)])
}

/// Open thread, which a thread's rows and its head offer: the frame goes to the thread.
fn open_thread(t: &super::model::Thread) -> MenuAct {
    let route = Route::Thread { workspace_id: t.workspace_id.clone(), thread_id: t.id.clone() };
    MenuAct {
        group: 1,
        glyph: Some(Glyph::MessageSquare),
        title: w::OPEN_THREAD.into(),
        destructive: false,
        held: false,
        why: None,
        keys: None,
        run: Rc::new(move |_: &mut Window, cx: &mut App| navigate(route.clone(), cx)),
    }
}

/// `containerActions`: End on one placed under its thread, and its name and id to copy.
fn container_acts(c: &ContainerRow, hand: &Hand) -> Vec<MenuAct> {
    let mut acts = Vec::new();
    if c.owned {
        let row = c.clone();
        acts.push(MenuAct {
            group: 0,
            glyph: Some(Glyph::SquareX),
            title: w::END_CONTAINER.into(),
            destructive: true,
            held: hand.stopping.contains(&c.id),
            why: hand.stopping.contains(&c.id).then(|| w::ENDING.into()),
            keys: None,
            run: hand.run(move |s, cx| s.stop_container(&row, cx)),
        });
    }
    for (title, value) in [(w::COPY_NAME, c.name.clone()), (w::COPY_CONTAINER_ID, c.id.clone())] {
        acts.push(MenuAct {
            group: 2,
            glyph: Some(Glyph::ClipboardCopy),
            title: title.into(),
            destructive: false,
            held: false,
            why: None,
            keys: None,
            run: copy(value),
        });
    }
    acts
}

/// A row's menu cell: shown while the row is pointed at, and always under 768 px; the spinner while what it ends is
/// ending.
fn menu_cell(id: SharedString, acts: &Rc<Vec<MenuAct>>, busy: bool, hand: &Hand, cx: &App) -> AnyElement {
    if busy {
        return h_flex().w_full().justify_end().child(spinner(cx)).into_any_element();
    }
    if acts.is_empty() {
        return div().into_any_element();
    }
    let always = hand.width.md;
    let (hand, acts) = (hand.clone(), acts.clone());
    let button = Button::icon_xs(id, Glyph::Ellipsis)
        .side(px(20.))
        .corners(Corners::all(px(4.)))
        .on_click(move |event, window, cx| hand.open_menu(event.position(), acts.clone(), window, cx));
    h_flex()
        .w_full()
        .justify_end()
        .when(!always, |el| el.opacity(0.).group_hover("activity-row", |s| s.opacity(1.)))
        .child(div().mx(px(-4.)).child(button))
        .into_any_element()
}

/// `SUB_ROW`: a row under a group, `py-2.5` on the card's inset, its menu on a right press too.
fn sub_row(
    id: SharedString,
    columns: &'static [Col],
    parts: Vec<AnyElement>,
    tip: Option<SharedString>,
    acts: Option<Rc<Vec<MenuAct>>>,
    hand: &Hand,
    cx: &App,
) -> CardItem {
    let hover = cx.palette().accent.opacity(0.6);
    let hand = hand.clone();
    CardItem::placed(move |corners| {
        let row = corners
            .apply(h_flex())
            .id(id)
            .group("activity-row")
            .debug_selector(|| "activity-sub-row".into())
            .gap_x(COLUMN_GAP)
            .py(px(10.))
            .px(corners.inset)
            .hover(|s| s.bg(hover))
            .when_some(tip, |el, tip| el.tooltip(crate::ui::tip(tip)))
            .children(cells(columns, parts));
        match acts.filter(|a| !a.is_empty()) {
            Some(acts) => row
                .on_mouse_down(MouseButton::Right, move |event, window, cx| {
                    cx.stop_propagation();
                    hand.open_menu(event.position, acts.clone(), window, cx)
                })
                .into_any_element(),
            None => row.into_any_element(),
        }
    })
}

fn num_cell(value: String, ink: Hsla) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .truncate()
        .text_right()
        .text_size(text::VALUE.0)
        .line_height(px(20.))
        .text_color(ink)
        .child(value)
        .into_any_element()
}

/// A row's memory, CPU and energy; untinted for a sum across threads, which says nothing past one thread's marks.
fn figure_cells(mem: f64, cpu: f64, energy: Option<f64>, hand: &Hand, tinted: bool, cx: &App) -> [AnyElement; 3] {
    let ink = |t: Option<Tint>| tint_ink(if tinted { t } else { None }, cx);
    [
        num_cell(fmt_bytes(mem as u64), ink(memory_tint(mem))),
        num_cell(cpu_word(cpu), ink(cpu_tint(cpu))),
        num_cell(if hand.energy { energy_word(energy) } else { String::new() }, ink(energy_tint(energy))),
    ]
}

/// The figures a row stands with at this width: memory alone under 768 px.
fn figures(mem: f64, cpu: f64, energy: Option<f64>, hand: &Hand, tinted: bool, cx: &App) -> Vec<AnyElement> {
    let mut cells = Vec::from(figure_cells(mem, cpu, energy, hand, tinted, cx));
    if hand.width.md {
        cells.truncate(1);
    }
    cells
}

/// The empty cells a row leaves under CPU and energy, none under 768 px.
fn wide_blanks(hand: &Hand) -> Vec<AnyElement> {
    if hand.width.md {
        Vec::new()
    } else {
        vec![div().into_any_element(), div().into_any_element()]
    }
}

fn proc_glyph(p: &Proc, cx: &App) -> AnyElement {
    match &p.harness {
        Some(h) => catalog::mark(h, &catalog::name(h), px(16.), cx.palette().foreground, cx),
        None => Icon::new(kind_glyph(p.kind)).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element(),
    }
}

fn small(word: impl Into<SharedString>, shrink: bool, cx: &App) -> gpui_kit::Div {
    let el = div().text_size(text::XS.0).line_height(text::XS.1).text_color(cx.palette().muted_foreground).child(word.into());
    if shrink {
        el.min_w_0().truncate()
    } else {
        el.flex_none().whitespace_nowrap()
    }
}

fn row_name(name: impl Into<SharedString>, muted: bool, cx: &App) -> gpui_kit::Div {
    let p = cx.palette();
    div()
        .min_w_0()
        .truncate()
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .text_color(if muted { p.muted_foreground } else { p.foreground.opacity(0.8) })
        .child(name.into())
}

/// The name cell of a row under a group: the glyph, the name, then its tags and ports, `pad` from the left.
fn lead_cell(pad: f32, mark: AnyElement, name: gpui_kit::Div, tags: Vec<gpui_kit::Div>) -> AnyElement {
    h_flex()
        .w_full()
        .min_w_0()
        .gap_2()
        .pl(px(pad))
        .child(mark)
        .child(h_flex().min_w_0().items_baseline().gap_2().child(name).children(tags))
        .into_any_element()
}

fn ports_tag(ports: &[Port], cx: &App) -> Option<gpui_kit::Div> {
    (!ports.is_empty()).then(|| small(ports_word(ports), false, cx))
}

/// One process under a group: its glyph, its name, its kind and ports as tags, then its figures and its menu.
fn proc_row(p: &Proc, depth: usize, group: &Group, hand: &Hand, cx: &App) -> CardItem {
    let ending = hand.ending.contains(&p.pid);
    let tag = if ending {
        Some(w::ENDING.to_owned())
    } else {
        p.note.map(str::to_owned).or_else(|| (!p.kind.untagged() && !p.heading).then(|| p.kind.word().to_owned()))
    };
    let name = if p.heading { w::slate(&p.name) } else { p.name.clone() };
    let tags: Vec<gpui_kit::Div> = hand.tag(tag).map(|t| small(t, true, cx)).into_iter().chain(ports_tag(&p.ports, cx)).collect();
    let acts = Rc::new(proc_acts(p, Some(group), None, hand));
    let key = if p.heading { format!("{}-heading", p.kind.id()) } else { p.pid.to_string() };
    let mut parts = vec![lead_cell(hand.lead_pad(depth), proc_glyph(p, cx), row_name(name, false, cx), tags)];
    parts.extend(figures(p.mem, p.cpu, p.energy, hand, true, cx));
    parts.push(menu_cell(format!("activity-menu-{}-{key}", group.id).into(), &acts, ending, hand, cx));
    let tip = if p.heading {
        None
    } else if p.needed {
        Some(w::NEEDED.into())
    } else {
        Some(p.command.clone().into())
    };
    sub_row(format!("activity-proc-{}-{key}", group.id).into(), hand.tree(), parts, tip, Some(acts), hand, cx)
}

fn proc_rows(procs: &[Proc], depth: usize, group: &Group, hand: &Hand, cx: &App, out: &mut Vec<CardItem>) {
    for p in procs {
        out.push(proc_row(p, depth, group, hand, cx));
        proc_rows(&p.children, depth + 1, group, hand, cx, out);
    }
}

/// One of a thread's subagents: its title, then its state, how long it ran, what it spent and its model as tags.
fn subagent_row(s: &Subagent, nest: usize, group: &Group, model: Option<String>, hand: &Hand, cx: &App) -> CardItem {
    let mut facts = vec![w::subagent_state(s.state).to_owned(), fmt_duration(s.ms)];
    facts.extend(s.tokens.map(|t| w::tokens(&fmt_tokens(t))));
    facts.extend(model);
    let mut tags: Vec<gpui_kit::Div> = hand.tag(Some(small(w::SUBAGENT, false, cx))).into_iter().collect();
    tags.push(h_flex().min_w_0().items_baseline().gap_2().overflow_hidden().children(facts.into_iter().map(|f| small(f, false, cx))));
    let glyph = Icon::new(Glyph::Bot).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let mut parts = vec![lead_cell(hand.lead_pad(nest), glyph, row_name(s.title.clone(), false, cx), tags)];
    parts.push(div().into_any_element());
    parts.extend(wide_blanks(hand));
    parts.push(div().into_any_element());
    sub_row(format!("activity-subagent-{}-{}", group.id, s.id).into(), hand.tree(), parts, Some(w::SUBAGENT_NOTE.into()), None, hand, cx)
}

/// One container under its group: its image, how long it has run and under No thread its compose folder, then its
/// own figures and End, which stops it and keeps its data.
fn container_row(c: &ContainerRow, nest: usize, loose: bool, hand: &Hand, cx: &App) -> CardItem {
    let stopping = hand.stopping.contains(&c.id);
    let mut facts: Vec<String> = vec![c.image.clone()];
    facts.extend(c.up_ms.map(|ms| w::up(&fmt_uptime(ms))));
    if loose {
        facts.extend(c.folder.clone());
    }
    facts.retain(|f| !f.is_empty());
    let note = match (loose, c.placed_by) {
        (false, Some(PlacedBy::Compose)) => w::NOTE_COMPOSE,
        (false, _) => w::NOTE_LABEL,
        (true, Some(PlacedBy::Label)) => w::NOTE_GONE,
        (true, Some(PlacedBy::Compose)) => w::NOTE_GONE_COMPOSE,
        (true, None) => w::NOTE_LOOSE,
    };
    // Under 768 px the facts after the first go (`narrow:hidden`).
    if hand.width.md {
        facts.truncate(1);
    }
    let mut tags: Vec<gpui_kit::Div> =
        hand.tag(Some(small(if stopping { w::ENDING } else { w::CONTAINER }, false, cx))).into_iter().collect();
    tags.push(h_flex().min_w_0().items_baseline().gap_2().overflow_hidden().children(facts.into_iter().map(|f| small(f, true, cx))));
    let glyph = Icon::new(Glyph::Container).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let acts = Rc::new(container_acts(c, hand));
    let mut parts = vec![lead_cell(hand.lead_pad(nest), glyph, row_name(c.name.clone(), false, cx), tags)];
    parts.push(c.mem.map_or_else(|| div().into_any_element(), |m| num_cell(fmt_bytes(m as u64), tint_ink(memory_tint(m), cx))));
    if !hand.width.md {
        parts.push(c.cpu.map_or_else(|| div().into_any_element(), |u| num_cell(cpu_word(u), tint_ink(cpu_tint(u), cx))));
        parts.push(div().into_any_element());
    }
    parts.push(menu_cell(format!("activity-container-menu-{}", c.id).into(), &acts, stopping, hand, cx));
    sub_row(format!("activity-container-{}", c.id).into(), hand.tree(), parts, Some(note.into()), Some(acts), hand, cx)
}

/// The sampler's own row, last under wsp: what reading this page costs the daemon.
fn sampler_row(cpu: f64, held: f64, hand: &Hand, cx: &App) -> CardItem {
    let glyph = Icon::new(Glyph::Gauge).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let note = hand.tag(Some(small(w::SAMPLER_NOTE, true, cx))).into_iter().collect();
    let mut parts = vec![lead_cell(hand.lead_pad(0), glyph, row_name(w::SAMPLER, false, cx), note)];
    parts.extend(figures(held, cpu, None, hand, true, cx));
    parts.push(div().into_any_element());
    sub_row("activity-sampler".into(), hand.tree(), parts, Some(w::SAMPLER_NOTE.into()), None, hand, cx)
}

/// A group's head: its name and note, the figures it draws and the fold. A thread's carries its menu, which under 768
/// px a button beside its name opens, where the act cell stands down.
#[allow(clippy::too_many_arguments)]
fn group_head(
    id: &str,
    mark: AnyElement,
    name: String,
    status: Option<Status>,
    note: String,
    total: Spent,
    nest: usize,
    open: bool,
    menu: Option<Rc<Vec<MenuAct>>>,
    hand: &Hand,
    tinted: bool,
    cx: &App,
) -> CardItem {
    let p = cx.palette();
    let head_menu = menu.clone().filter(|acts| hand.width.md && !acts.is_empty()).map(|acts| {
        let hand = hand.clone();
        let button = Button::icon_xs(SharedString::from(format!("activity-head-menu-{id}")), Glyph::Ellipsis)
            .side(px(20.))
            .corners(Corners::all(px(4.)))
            .on_click(move |event, window, cx| hand.open_menu(event.position(), acts.clone(), window, cx));
        div()
            .debug_selector(|| "activity-head-menu".into())
            .flex_none()
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(button)
    });
    let title = h_flex()
        .min_w_0()
        .gap_2()
        .child(
            div()
                .debug_selector(|| "grid-name".into())
                .min_w_0()
                .truncate()
                .text_size(text::LIST_TITLE.0)
                .line_height(text::LIST_TITLE.1)
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(p.foreground)
                .child(name),
        )
        .children(status.map(StatusMark::new))
        .children(head_menu);
    let name_cell = h_flex().w_full().min_w_0().gap_3().pl(px(hand.nest_pad(nest))).child(framed(mark, cx)).child(
        v_flex()
            .min_w_0()
            .child(title)
            .child(div().truncate().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(note)),
    );
    let chevron = h_flex().w_full().justify_end().child(
        Icon::new(Glyph::ChevronRight)
            .with_size(px(14.))
            .text_color(p.muted_foreground)
            .when(open, |i| i.rotate(gpui_kit::radians(std::f32::consts::FRAC_PI_2))),
    );
    let toggle = {
        let (hand, id) = (hand.clone(), id.to_owned());
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| hand.with(cx, |s, cx| s.toggle(&id, cx))
    };
    let [mem, cpu, energy] = figure_cells(total.mem, total.cpu, total.energy, hand, tinted, cx);
    let mut row = GridRow::new(format!("activity-group-{id}"), TREE).narrow(NARROW).on_open(toggle).cell(name_cell);
    if let Some(acts) = menu {
        let hand = hand.clone();
        row = row.on_menu(move |event, window, cx| {
            cx.stop_propagation();
            hand.open_menu(event.position, acts.clone(), window, cx)
        });
    }
    row.cell(mem).wide(cpu).wide(energy).cell(chevron).into()
}

/// What a page tells the Threads view about each thread: its mark and how its subagents' models read.
pub struct ThreadLooks<'a> {
    pub status: &'a dyn Fn(&str) -> Option<Status>,
    pub model: &'a dyn Fn(&str, &str) -> Option<String>,
}

/// One group's head and, folded open, its rows.
#[allow(clippy::too_many_arguments)]
fn group_block(
    g: &Group,
    under: &[&Group],
    nest: usize,
    open: bool,
    looks: &ThreadLooks,
    box_: bool,
    cost: Option<(f64, f64)>,
    hand: &Hand,
    cx: &App,
    out: &mut Vec<CardItem>,
) {
    match g.kind {
        GroupKind::Wsp => {
            let note = if box_ { w::WSP_NOTE_BOX } else { w::WSP_NOTE };
            out.push(group_head(
                &g.id,
                glyph(Glyph::Activity, cx).into_any_element(),
                w::WSP.into(),
                None,
                note.into(),
                group_spent(g),
                0,
                open,
                None,
                hand,
                true,
                cx,
            ));
            if open {
                proc_rows(&g.procs, 0, g, hand, cx, out);
                if let (Some((cpu, held)), false) = (cost, hand.filtering) {
                    out.push(sampler_row(cpu, held, hand, cx));
                }
            }
        }
        GroupKind::Loose => {
            out.push(group_head(
                &g.id,
                glyph(Glyph::CircleDashed, cx).into_any_element(),
                w::LOOSE.into(),
                None,
                w::LOOSE_NOTE.into(),
                group_spent(g),
                0,
                open,
                None,
                hand,
                true,
                cx,
            ));
            if open {
                proc_rows(&g.procs, 0, g, hand, cx, out);
                out.extend(g.containers.iter().map(|c| container_row(c, 0, true, hand, cx)));
            }
        }
        GroupKind::Thread => {
            let t = g.thread.as_ref().unwrap();
            let total = total_spent(std::iter::once(g).chain(under.iter().copied()));
            let mark = catalog::mark(&t.harness, &catalog::name(&t.harness), px(16.), cx.palette().foreground, cx);
            let head = group_head(
                &g.id,
                mark,
                t.title.clone(),
                (looks.status)(&t.id),
                t.project.clone(),
                total,
                nest,
                open,
                Some(thread_head_acts(t)),
                hand,
                true,
                cx,
            );
            out.push(head);
            if open {
                proc_rows(&g.procs, nest, g, hand, cx, out);
                out.extend(
                    g.subagents
                        .iter()
                        .map(|s| subagent_row(s, nest, g, s.model.as_deref().and_then(|m| (looks.model)(&t.harness, m)), hand, cx)),
                );
                out.extend(g.containers.iter().map(|c| container_row(c, nest, false, hand, cx)));
            }
        }
    }
}

fn sort_cell(sort: Sort, word: &'static str, hand: &Hand, cx: &App) -> AnyElement {
    let p = cx.palette();
    let on = hand.sort == sort;
    let pick = hand.clone();
    h_flex()
        .id(word)
        .debug_selector(move || format!("activity-sort-{word}"))
        .w_full()
        .justify_end()
        .gap_1()
        .whitespace_nowrap()
        .cursor_pointer()
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .text_color(if on { p.foreground } else { p.muted_foreground })
        .hover(|s| s.text_color(p.foreground))
        .on_click(move |_, _, cx| pick.with(cx, |s, cx| s.set_sort(sort, cx)))
        .when(on, |el| el.child(Icon::new(Glyph::ArrowDown).with_size(px(12.))))
        .child(word)
        .into_any_element()
}

fn head_word(word: &'static str, right: bool, cx: &App) -> AnyElement {
    div()
        .w_full()
        .whitespace_nowrap()
        .text_color(cx.palette().muted_foreground)
        .when(right, |el| el.text_right())
        .child(word)
        .into_any_element()
}

/// The header row over a list: the section's name on the card's outer edge, the column words over their values.
fn head_row(columns: &'static [Col], name: String, rest: Vec<AnyElement>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let first = div().ml(px(-21.)).min_w_0().truncate().text_color(p.foreground.opacity(0.7)).child(name).into_any_element();
    h_flex()
        .debug_selector(|| "grid-head".into())
        .mb_4()
        .min_h(px(28.))
        .items_end()
        .gap_x(COLUMN_GAP)
        .border_x_1()
        .border_color(gpui_kit::transparent_black())
        .px(crate::ui::size::CARD_INSET)
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .children(cells(columns, std::iter::once(first).chain(rest).collect()))
        .into_any_element()
}

fn sorts(hand: &Hand, cx: &App) -> Vec<AnyElement> {
    if hand.width.md {
        return vec![sort_cell(Sort::Mem, w::MEMORY, hand, cx), div().into_any_element()];
    }
    vec![
        sort_cell(Sort::Mem, w::MEMORY, hand, cx),
        sort_cell(Sort::Cpu, w::CPU, hand, cx),
        if hand.energy { sort_cell(Sort::Energy, w::ENERGY, hand, cx) } else { div().into_any_element() },
        div().into_any_element(),
    ]
}

fn list(id: &'static str, head: AnyElement, mut rows: Vec<CardItem>, empty: bool, hand: &Hand) -> AnyElement {
    if empty {
        rows.push(Line::new(format!("{id}-empty"), if hand.filtering { w::NOTHING_MATCHES } else { w::NOTHING }).empty().into());
    }
    v_flex().id(id).child(head).child(Card::new(format!("{id}-list")).locked(true).items(rows)).into_any_element()
}

/// The Threads view: wsp's own parts, each thread heaviest first with the threads it started folded under it, then No
/// thread. A filter opens every group it left.
pub fn threads_table(
    name: String,
    groups: &[Group],
    looks: &ThreadLooks,
    box_: bool,
    cost: Option<(f64, f64)>,
    hand: &Hand,
    cx: &App,
) -> AnyElement {
    let (tops, kids) = nest_groups(groups);
    let mut rows = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn block(
        g: &Group,
        nest: usize,
        kids: &std::collections::HashMap<String, Vec<&Group>>,
        looks: &ThreadLooks,
        box_: bool,
        cost: Option<(f64, f64)>,
        hand: &Hand,
        cx: &App,
        rows: &mut Vec<CardItem>,
    ) {
        let under = groups_under(kids, &g.id);
        let open = hand.filtering || hand.open.contains(&g.id);
        group_block(g, &under, nest, open, looks, box_, cost, hand, cx, rows);
        if open {
            for k in kids.get(&g.id).into_iter().flatten() {
                block(k, nest + 1, kids, looks, box_, cost, hand, cx, rows);
            }
        }
    }
    for g in tops {
        block(g, 0, &kids, looks, box_, cost, hand, cx, &mut rows);
    }
    list("activity-threads", head_row(hand.tree(), name, sorts(hand, cx), cx), rows, groups.is_empty(), hand)
}

/// Where a flat row came from, in the note under its name.
fn where_of(group: Option<&Group>) -> String {
    match group {
        None => String::new(),
        Some(g) => match g.kind {
            GroupKind::Wsp => w::WSP.into(),
            GroupKind::Loose => w::LOOSE.into(),
            GroupKind::Thread => g.thread.as_ref().map(|t| t.title.clone()).unwrap_or_default(),
        },
    }
}

/// The Processes view: every process wsp owns flat, and with Every process the rest of the computer in the muted ink.
pub fn processes_table(name: String, rows: &[Placed], hand: &Hand, cx: &App) -> AnyElement {
    let p = cx.palette();
    let items = rows
        .iter()
        .map(|row| {
            let q = row.p;
            let outside = row.group.is_none();
            let ending = hand.ending.contains(&q.pid);
            let bare = outside || hand.width.sm || (q.kind.untagged() && q.note.is_none());
            let tag = (!bare).then(|| if ending { w::ENDING.to_owned() } else { q.note.map_or(q.kind.word().to_owned(), str::to_owned) });
            let line = h_flex()
                .min_w_0()
                .items_baseline()
                .gap_2()
                .child(row_name(q.name.clone(), outside, cx))
                .children(tag.map(|t| small(t, true, cx)))
                .children(ports_tag(&q.ports, cx));
            let lead = h_flex()
                .w_full()
                .min_w_0()
                .gap_2()
                .child(proc_glyph(q, cx))
                .child(
                    v_flex().min_w_0().child(line).when(!outside, |el| el.child(small(where_of(row.group), true, cx).line_height(px(16.)))),
                )
                .into_any_element();
            let acts = Rc::new(proc_acts(q, row.group, None, hand));
            let mut parts = vec![lead];
            if !hand.width.md {
                parts.push(
                    div()
                        .w_full()
                        .text_right()
                        .text_size(text::FACT.0)
                        .text_color(p.muted_foreground)
                        .child(q.pid.to_string())
                        .into_any_element(),
                );
            }
            parts.extend(figures(q.mem, q.cpu, q.energy, hand, true, cx));
            parts.push(menu_cell(format!("activity-flat-menu-{}", q.pid).into(), &acts, ending, hand, cx));
            sub_row(format!("activity-flat-{}", q.pid).into(), hand.flat(), parts, Some(q.command.clone().into()), Some(acts), hand, cx)
        })
        .collect();
    let mut head = if hand.width.md { Vec::new() } else { vec![head_word(w::PID, true, cx)] };
    head.extend(sorts(hand, cx));
    list("activity-processes", head_row(hand.flat(), name, head, cx), items, rows.is_empty(), hand)
}

/// The Kinds view: a group per kind, its processes flat under it, each naming the thread it belongs to.
pub fn kinds_table(name: String, kinds: &[(Kind, Vec<Placed>)], hand: &Hand, cx: &App) -> AnyElement {
    let mut rows = Vec::new();
    for (kind, placed) in kinds {
        let open = hand.filtering || hand.open.contains(kind.id());
        let procs: Vec<Proc> = placed.iter().map(|r| r.p.clone()).collect();
        let total = spent(&procs);
        rows.push(group_head(
            kind.id(),
            glyph(kind_glyph(*kind), cx).into_any_element(),
            kind.head().into(),
            None,
            w::process_count(placed.len()),
            total,
            0,
            open,
            None,
            hand,
            false,
            cx,
        ));
        if !open {
            continue;
        }
        for row in placed {
            let q = row.p;
            let ending = hand.ending.contains(&q.pid);
            let tags: Vec<gpui_kit::Div> = hand
                .tag(Some(small(if ending { w::ENDING.to_owned() } else { where_of(row.group) }, true, cx)))
                .into_iter()
                .chain(ports_tag(&q.ports, cx))
                .collect();
            let acts = Rc::new(proc_acts(q, row.group, None, hand));
            let mut parts = vec![lead_cell(hand.lead_pad(0), proc_glyph(q, cx), row_name(q.name.clone(), false, cx), tags)];
            parts.extend(figures(q.mem, q.cpu, q.energy, hand, true, cx));
            parts.push(menu_cell(format!("activity-kind-menu-{}", q.pid).into(), &acts, ending, hand, cx));
            rows.push(sub_row(
                format!("activity-kind-{}-{}", kind.id(), q.pid).into(),
                hand.tree(),
                parts,
                Some(q.command.clone().into()),
                Some(acts),
                hand,
                cx,
            ));
        }
    }
    list("activity-kinds", head_row(hand.tree(), name, sorts(hand, cx), cx), rows, kinds.is_empty(), hand)
}

/// The Ports view: every port a process wsp owns listens on, its thread, and who can reach it.
pub fn ports_table(rows: &[(Port, &Proc, &Group)], hand: &Hand, cx: &App) -> AnyElement {
    let p = cx.palette();
    let note = |word: String| {
        div().min_w_0().truncate().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(word)
    };
    let items = rows
        .iter()
        .map(|(port, q, group)| {
            let thread = match &group.thread {
                None => note(where_of(Some(group))).into_any_element(),
                Some(t) => h_flex()
                    .min_w_0()
                    .gap_2()
                    .child(catalog::mark(&t.harness, &catalog::name(&t.harness), px(14.), cx.palette().foreground, cx))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(text::SETTING_TITLE.0)
                            .text_color(p.foreground.opacity(0.8))
                            .child(t.title.clone()),
                    )
                    .into_any_element(),
            };
            let reach = match port.loopback {
                None => String::new(),
                Some(true) => w::LOOPBACK.into(),
                Some(false) => w::EVERY_ADDRESS.into(),
            };
            let acts = Rc::new(proc_acts(q, Some(group), Some(*port), hand));
            let ending = hand.ending.contains(&q.pid);
            let open = open_port(q, Some(group), Some(*port), hand).filter(|_| !ending).map(|(_, run)| {
                let open = Button::outline(format!("activity-port-open-{}-{}", q.pid, port.port), w::OPEN)
                    .xs()
                    .on_click(move |_, window, cx| run(window, cx));
                h_flex().opacity(0.).group_hover("activity-row", |s| s.opacity(1.)).child(open)
            });
            let mut parts = vec![
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(text::VALUE.0)
                    .line_height(px(20.))
                    .text_color(p.foreground)
                    .child(port.port.to_string())
                    .into_any_element(),
                h_flex().min_w_0().gap_2().child(proc_glyph(q, cx)).child(row_name(q.name.clone(), false, cx)).into_any_element(),
            ];
            if !hand.width.md {
                parts.extend([
                    thread,
                    note(reach).into_any_element(),
                    h_flex().w_full().justify_end().gap(px(6.)).children(open).into_any_element(),
                ]);
            }
            parts.push(menu_cell(format!("activity-port-menu-{}-{}", q.pid, port.port).into(), &acts, ending, hand, cx));
            sub_row(
                format!("activity-port-{}-{}", q.pid, port.port).into(),
                hand.ports(),
                parts,
                Some(q.command.clone().into()),
                Some(acts),
                hand,
                cx,
            )
        })
        .collect();
    let head = if hand.width.md {
        vec![head_word(w::PROCESS, false, cx), div().into_any_element()]
    } else {
        vec![
            head_word(w::PROCESS, false, cx),
            head_word(w::THREAD, false, cx),
            head_word(w::REACH, false, cx),
            div().into_any_element(),
            div().into_any_element(),
        ]
    };
    list("activity-ports", head_row(hand.ports(), w::PORT.into(), head, cx), items, rows.is_empty(), hand)
}

/// Every process a heading holds that End reaches, which End all signals.
pub fn ends_of(heading: &Proc) -> Vec<&Proc> {
    heading.children.iter().filter(|c| c.endable).collect()
}

pub fn held_by(heading: &Proc) -> f64 {
    let ends: Vec<Proc> = ends_of(heading).into_iter().cloned().collect();
    flat_mem(&ends)
}

fn flat_mem(procs: &[Proc]) -> f64 {
    procs.iter().flat_map(flat).map(|p| p.mem).sum()
}

#[cfg(test)]
mod tests {
    use super::super::model::Thread;
    use super::*;

    fn hand(workspace: Option<&str>, ending: &[Pid]) -> Hand {
        Hand {
            scoped: WeakEntity::new_invalid(),
            ending: Rc::new(ending.iter().copied().collect()),
            stopping: Rc::default(),
            open: Rc::default(),
            sort: Sort::Mem,
            energy: false,
            filtering: false,
            workspace: workspace.map(str::to_owned),
            width: Width { sm: false, md: false },
        }
    }

    fn dev(port: i64) -> Proc {
        Proc {
            pid: 41,
            name: "node".into(),
            kind: Kind::Dev,
            mem: 0.,
            cpu: 0.,
            energy: None,
            ports: vec![Port { port, loopback: Some(true) }],
            command: "npm run dev".into(),
            started_at: Some(1.),
            id: Some("tick-1".into()),
            harness: None,
            needed: false,
            endable: true,
            heading: false,
            note: None,
            children: vec![],
        }
    }

    fn group(kind: GroupKind) -> Group {
        let thread = (kind == GroupKind::Thread).then(|| Thread {
            id: "t1".into(),
            title: "Fix the login".into(),
            harness: "claude".into(),
            project: "bench".into(),
            workspace_id: "w1".into(),
            session_id: "s1".into(),
            working: false,
            pid: None,
            agent_pid: None,
            cgroup: None,
            parent: None,
            slate: None,
            subagents: vec![],
        });
        Group { id: "g".into(), kind, thread, procs: vec![], subagents: vec![], containers: vec![], memory: None }
    }

    fn listed(acts: &[MenuAct]) -> Vec<(String, &'static str, Option<String>)> {
        menu_items(acts).into_iter().map(|i| (i.label.to_string(), i.group, i.refusal.map(|r| r.to_string()))).collect()
    }

    fn row(label: &str, group: &'static str) -> (String, &'static str, Option<String>) {
        (label.to_owned(), group, None)
    }

    #[test]
    fn a_threads_process_offers_its_port_and_its_thread_in_the_webs_order() {
        let thread = group(GroupKind::Thread);
        assert_eq!(
            listed(&proc_acts(&dev(3000), Some(&thread), None, &hand(None, &[]))),
            [
                row("Open :3000 in a browser tab", "act"),
                row("End", "act"),
                row("Open thread", "go"),
                row("Copy command", "copy"),
                row("Copy PID", "copy"),
            ]
        );
        let on_port = proc_acts(&dev(3000), Some(&thread), Some(Port { port: 5173, loopback: None }), &hand(None, &[]));
        assert_eq!(on_port[0].title.as_ref(), "Open :5173 in a browser tab", "a Ports row opens its own port");
    }

    #[test]
    fn a_port_opens_only_where_a_workspace_is_known() {
        let loose = group(GroupKind::Loose);
        let here = listed(&proc_acts(&dev(3000), Some(&loose), None, &hand(None, &[])));
        assert_eq!(here, [row("End", "act"), row("Copy command", "copy"), row("Copy PID", "copy")]);
        let on_a_box = listed(&proc_acts(&dev(3000), Some(&loose), None, &hand(Some("w9"), &[])));
        assert_eq!(on_a_box[0], row("Open :3000 in a browser tab", "act"));
    }

    #[test]
    fn an_ending_process_holds_its_acts_with_the_refusal() {
        let thread = group(GroupKind::Thread);
        let acts = listed(&proc_acts(&dev(3000), Some(&thread), None, &hand(None, &[41])));
        assert_eq!(acts[1], ("End".to_owned(), "act", Some("Ending".to_owned())));
        assert_eq!(acts[0].2, None, "Open stays open while the process ends");
    }
}
