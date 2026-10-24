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
use super::views::{piece, Drawn, Look};
use super::wire::{Kind, Pid, PlacedBy};
use super::Scoped;
use crate::screens::app_shell::{navigate, Route};
use crate::screens::settings::words::{fmt_duration, fmt_tokens};
use crate::ui::card::surface_item;
use crate::ui::catalog;
use crate::ui::context_menu::MenuItem;
use crate::ui::fmt_bytes;
use crate::ui::grid::{
    ACTIVITY_COLUMNS, ACTIVITY_COLUMNS_NARROW as NARROW, ACTIVITY_FLAT_COLUMNS as FLAT, ACTIVITY_PORT_COLUMNS as PORTS,
    ACTIVITY_PORT_COLUMNS_NARROW as PORTS_NARROW,
};
use crate::ui::{framed, glyph, spinner, text, ActivePalette, Button, CardItem, Col, Glyph, GridRow, Line, MenuAct, Run, Status, Width};

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
    /// The live pieces the rows draw their figures and their titles in; a look with none is drawn plainly.
    pub pieces: Drawn,
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

    /// The row's acts at `at`, as [`open_menu`] opens them.
    pub fn open_menu(&self, at: Point<Pixels>, acts: Rc<Vec<MenuAct>>, window: &mut Window, cx: &mut App) {
        open_menu(&self.scoped, at, acts, window, cx);
    }

    /// `look` in the piece standing under `key`.
    fn piece(&self, key: &str, look: Look, cx: &App) -> AnyElement {
        piece(&self.pieces, key, look, &self.scoped, cx)
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

/// `openContextMenu`: the row's acts at `at`, the native menu on the Mac and the page's own elsewhere; a pick runs once
/// the menu has let go of the page.
pub(crate) fn open_menu(scoped: &WeakEntity<Scoped>, at: Point<Pixels>, acts: Rc<Vec<MenuAct>>, window: &mut Window, cx: &mut App) {
    let Some(scoped) = scoped.upgrade() else { return };
    let items = menu_items(&acts);
    scoped.update(cx, |s, cx| {
        s.menu.open(at, items, window, cx, move |_, pick, window, cx| {
            let Some(run) = pick.and_then(|ix| acts.get(ix)).filter(|a| !a.held).map(|a| a.run.clone()) else { return };
            window.defer(cx, move |window, cx| run(window, cx));
        })
    });
}

/// The key a process row's figures stand under: its pid, or for a slate's heading its group's heading of its kind.
pub(crate) fn proc_key(group: &str, p: &Proc) -> String {
    if p.heading {
        format!("p:{group}:{}-heading", p.kind.id())
    } else {
        format!("p:{}", p.pid)
    }
}

/// A row's memory, CPU and energy as their figures read; untinted for a sum across threads, which says nothing past one
/// thread's marks.
pub(crate) fn figure_looks(mem: f64, cpu: f64, energy: Option<f64>, energy_on: bool, tinted: bool) -> [Look; 3] {
    let tint = |t: Option<Tint>| if tinted { t } else { None };
    [
        Look::Figure { text: fmt_bytes(mem as u64), tint: tint(memory_tint(mem)) },
        Look::Figure { text: cpu_word(cpu), tint: tint(cpu_tint(cpu)) },
        Look::Figure { text: if energy_on { energy_word(energy) } else { String::new() }, tint: tint(energy_tint(energy)) },
    ]
}

/// A container's memory and CPU, each blank where its engine read none.
pub(crate) fn container_looks(c: &ContainerRow) -> [Look; 2] {
    [
        Look::Figure { text: c.mem.map(|m| fmt_bytes(m as u64)).unwrap_or_default(), tint: c.mem.and_then(memory_tint) },
        Look::Figure { text: c.cpu.map(cpu_word).unwrap_or_default(), tint: c.cpu.and_then(cpu_tint) },
    ]
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
    // The in-app browser lands later: until then a port opens in the system browser.
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
        crate::frames::count("ActivityRow");
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

/// A row's memory, CPU and energy, each in its piece under `key`.
fn figure_cells(key: &str, mem: f64, cpu: f64, energy: Option<f64>, hand: &Hand, tinted: bool, cx: &App) -> [AnyElement; 3] {
    let [m, c, e] = figure_looks(mem, cpu, energy, hand.energy, tinted);
    [hand.piece(&format!("{key}:0"), m, cx), hand.piece(&format!("{key}:1"), c, cx), hand.piece(&format!("{key}:2"), e, cx)]
}

/// The figures a row stands with at this width: memory alone under 768 px.
fn figures(key: &str, mem: f64, cpu: f64, energy: Option<f64>, hand: &Hand, tinted: bool, cx: &App) -> Vec<AnyElement> {
    let mut cells = Vec::from(figure_cells(key, mem, cpu, energy, hand, tinted, cx));
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
    (!ports.is_empty()).then(|| small(ports_word(ports), false, cx).font_features(text::tabular()))
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
    let tags: Vec<gpui_kit::Div> = hand
        .tag(tag)
        .map(|t| small(t, true, cx).debug_selector(|| "activity-tag".into()))
        .into_iter()
        .chain(ports_tag(&p.ports, cx))
        .collect();
    let acts = Rc::new(proc_acts(p, Some(group), None, hand));
    let key = if p.heading { format!("{}-heading", p.kind.id()) } else { p.pid.to_string() };
    let mut parts = vec![lead_cell(hand.lead_pad(depth), proc_glyph(p, cx), row_name(name, false, cx), tags)];
    parts.extend(figures(&proc_key(&group.id, p), p.mem, p.cpu, p.energy, hand, true, cx));
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

#[allow(dead_code)]
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
    tags.push(
        h_flex()
            .min_w_0()
            .items_baseline()
            .gap_2()
            .overflow_hidden()
            .font_features(text::tabular())
            .children(facts.into_iter().map(|f| small(f, false, cx))),
    );
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
    tags.push(
        h_flex()
            .min_w_0()
            .items_baseline()
            .gap_2()
            .overflow_hidden()
            .font_features(text::tabular())
            .children(facts.into_iter().map(|f| small(f, true, cx))),
    );
    let glyph = Icon::new(Glyph::Container).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let acts = Rc::new(container_acts(c, hand));
    let mut parts = vec![lead_cell(hand.lead_pad(nest), glyph, row_name(c.name.clone(), false, cx), tags)];
    let [mem, cpu] = container_looks(c);
    parts.push(hand.piece(&format!("c:{}:0", c.id), mem, cx));
    if !hand.width.md {
        parts.push(hand.piece(&format!("c:{}:1", c.id), cpu, cx));
        parts.push(div().into_any_element());
    }
    parts.push(menu_cell(format!("activity-container-menu-{}", c.id).into(), &acts, stopping, hand, cx));
    sub_row(format!("activity-container-{}", c.id).into(), hand.tree(), parts, Some(note.into()), Some(acts), hand, cx)
}

/// The key the sampler's figures stand under.
pub(crate) const SAMPLER: &str = "sampler";

/// The sampler's own row, last under wsp: what reading this page costs the daemon.
fn sampler_row(cpu: f64, held: f64, hand: &Hand, cx: &App) -> CardItem {
    let glyph = Icon::new(Glyph::Gauge).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let note = hand.tag(Some(small(w::SAMPLER_NOTE, true, cx))).into_iter().collect();
    let mut parts = vec![lead_cell(hand.lead_pad(0), glyph, row_name(w::SAMPLER, false, cx), note)];
    parts.extend(figures(SAMPLER, held, cpu, None, hand, true, cx));
    parts.push(div().into_any_element());
    sub_row("activity-sampler".into(), hand.tree(), parts, Some(w::SAMPLER_NOTE.into()), None, hand, cx)
}

/// A group's head: its name and note, the figures it draws and the fold. A thread's carries its menu, which under 768
/// px a button beside its name opens, where the act cell stands down.
#[allow(clippy::too_many_arguments)]
fn group_head(
    id: &str,
    key: &str,
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
    let title = hand.piece(&format!("t:{id}"), title_look(id, name, status, menu.as_ref(), hand.width), cx);
    // Filling the row's rest leaves every pixel where the content's own width put it, and gives the title a box.
    let name_cell = h_flex().w_full().min_w_0().gap_3().pl(px(hand.nest_pad(nest))).child(framed(mark, cx)).child(
        v_flex()
            .flex_1()
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
    let [mem, cpu, energy] = figure_cells(key, total.mem, total.cpu, total.energy, hand, tinted, cx);
    // The page's width or the pane's, as `Hand` reads it, never the window's alone.
    let mut row = GridRow::new(format!("activity-group-{id}"), hand.tree()).on_open(toggle).cell(name_cell);
    if let Some(acts) = menu {
        let hand = hand.clone();
        row = row.on_menu(move |event, window, cx| {
            cx.stop_propagation();
            hand.open_menu(event.position, acts.clone(), window, cx)
        });
    }
    row = row.cell(mem);
    if !hand.width.md {
        row = row.cell(cpu).cell(energy);
    }
    row.cell(chevron).into()
}

/// A head's title: its name, its status mark, and under 768 px the button its menu opens from.
pub(crate) fn title_look(id: &str, name: String, status: Option<Status>, menu: Option<&Rc<Vec<MenuAct>>>, width: Width) -> Look {
    let menu =
        menu.filter(|acts| width.md && !acts.is_empty()).map(|acts| (SharedString::from(format!("activity-head-menu-{id}")), acts.clone()));
    Look::Title { name, status, menu }
}

/// A thread's status mark, by its id.
pub type StatusOf = Rc<dyn Fn(&str) -> Option<Status>>;
/// How a subagent's model reads, by its harness and the model's id.
pub type ModelOf = Rc<dyn Fn(&str, &str) -> Option<String>>;

/// What a page tells the Threads view about each thread: its mark and how its subagents' models read.
#[derive(Clone)]
pub struct ThreadLooks {
    pub status: StatusOf,
    pub model: ModelOf,
}

/// A group's head, as its kind draws it: wsp's own parts, No thread, or a thread with the threads `under` it counted.
#[allow(clippy::too_many_arguments)]
fn group_head_of(g: &Group, under: &[&Group], nest: usize, open: bool, looks: &ThreadLooks, box_: bool, hand: &Hand, cx: &App) -> CardItem {
    match g.kind {
        GroupKind::Wsp => {
            let note = if box_ { w::WSP_NOTE_BOX } else { w::WSP_NOTE };
            let mark = glyph(Glyph::Activity, cx).into_any_element();
            group_head(&g.id, &format!("g:{}", g.id), mark, w::WSP.into(), None, note.into(), group_spent(g), 0, open, None, hand, true, cx)
        }
        GroupKind::Loose => {
            let mark = glyph(Glyph::CircleDashed, cx).into_any_element();
            let note = w::LOOSE_NOTE.into();
            group_head(&g.id, &format!("g:{}", g.id), mark, w::LOOSE.into(), None, note, group_spent(g), 0, open, None, hand, true, cx)
        }
        GroupKind::Thread => {
            let t = g.thread.as_ref().unwrap();
            let total = total_spent(std::iter::once(g).chain(under.iter().copied()));
            let mark = catalog::mark(&t.harness, &catalog::name(&t.harness), px(16.), cx.palette().foreground, cx);
            group_head(
                &g.id,
                &format!("g:{}", g.id),
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
            )
        }
    }
}

/// One row of the Threads view, by the group it stands in, its place among the groups.
enum ThreadRow {
    Head { at: usize, under: Vec<usize>, nest: usize, open: bool },
    Proc { at: usize, p: Proc, depth: usize },
    Sampler { cpu: f64, held: f64 },
    Container { at: usize, c: usize, nest: usize },
    Subagent { at: usize, s: usize, nest: usize },
}

impl ThreadRow {
    fn make(&self, groups: &[Group], looks: &ThreadLooks, box_: bool, hand: &Hand, cx: &App) -> CardItem {
        match self {
            ThreadRow::Head { at, under, nest, open } => {
                let under: Vec<&Group> = under.iter().map(|u| &groups[*u]).collect();
                group_head_of(&groups[*at], &under, *nest, *open, looks, box_, hand, cx)
            }
            ThreadRow::Proc { at, p, depth } => proc_row(p, *depth, &groups[*at], hand, cx),
            ThreadRow::Sampler { cpu, held } => sampler_row(*cpu, *held, hand, cx),
            ThreadRow::Container { at, c, nest } => {
                let g = &groups[*at];
                container_row(&g.containers[*c], *nest, g.kind == GroupKind::Loose, hand, cx)
            }
            ThreadRow::Subagent { at, s, nest } => {
                let g = &groups[*at];
                let s = &g.subagents[*s];
                let model = g.thread.as_ref().and_then(|t| s.model.as_deref().and_then(|m| (looks.model)(&t.harness, m)));
                subagent_row(s, *nest, g, model, hand, cx)
            }
        }
    }
}

/// A group's head and, folded open, its rows and the threads it started, each nested a step further: as `at` each
/// group's place among the groups.
#[allow(clippy::too_many_arguments)]
fn thread_rows(
    g: &Group,
    nest: usize,
    kids: &std::collections::HashMap<String, Vec<&Group>>,
    at: &dyn Fn(&Group) -> usize,
    cost: Option<(f64, f64)>,
    hand: &Hand,
    out: &mut Vec<ThreadRow>,
) {
    fn procs(list: &[Proc], at: usize, depth: usize, out: &mut Vec<ThreadRow>) {
        for p in list {
            out.push(ThreadRow::Proc { at, p: p.clone(), depth });
            procs(&p.children, at, depth + 1, out);
        }
    }
    let open = hand.filtering || hand.open.contains(&g.id);
    let (here, under) = (at(g), groups_under(kids, &g.id).into_iter().map(at).collect());
    out.push(ThreadRow::Head { at: here, under, nest, open });
    if !open {
        return;
    }
    let containers =
        |nest: usize, out: &mut Vec<ThreadRow>| out.extend((0..g.containers.len()).map(|c| ThreadRow::Container { at: here, c, nest }));
    match g.kind {
        GroupKind::Wsp => {
            procs(&g.procs, here, 0, out);
            if let (Some((cpu, held)), false) = (cost, hand.filtering) {
                out.push(ThreadRow::Sampler { cpu, held });
            }
        }
        GroupKind::Loose => {
            procs(&g.procs, here, 0, out);
            containers(0, out);
        }
        GroupKind::Thread => {
            procs(&g.procs, here, nest, out);
            out.extend((0..g.subagents.len()).map(|s| ThreadRow::Subagent { at: here, s, nest }));
            containers(nest, out);
        }
    }
    for k in kids.get(&g.id).into_iter().flatten() {
        thread_rows(k, nest + 1, kids, at, cost, hand, out);
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

/// A row of a table, made again each time the page's list draws it.
pub(crate) type MakeRow = Rc<dyn Fn(&App) -> CardItem>;

/// A table as the page's list draws it: its head, then its rows, each made only where it shows, and whether each is
/// ruled off the one above, as a lead's rows are not.
pub(crate) struct Table {
    pub id: &'static str,
    pub head: Rc<dyn Fn(&App) -> AnyElement>,
    pub rows: Vec<(MakeRow, bool)>,
}

impl Table {
    /// The table drawn whole, on one card.
    pub fn whole(self, cx: &App) -> AnyElement {
        let count = self.rows.len();
        let rows = self.rows.iter().enumerate().map(|(ix, (row, ruled))| surface_item(row(cx), ix, count, *ruled, cx));
        v_flex().id(self.id).child((self.head)(cx)).child(v_flex().children(rows.collect::<Vec<_>>())).into_any_element()
    }
}

/// A process of a flat view and its group's place among the groups.
pub(crate) type Held = (Proc, Option<usize>);

/// Placed rows held apart from the groups they point into: each process, and its group's place among `groups`.
pub(crate) fn held(groups: &[Group], rows: Vec<Placed>) -> Vec<Held> {
    rows.into_iter().map(|r| (r.p.clone(), r.group.and_then(|g| groups.iter().position(|x| std::ptr::eq(x, g))))).collect()
}

/// A table whose rows are each ruled off the one above.
fn table(id: &'static str, head: impl Fn(&App) -> AnyElement + 'static, rows: Vec<MakeRow>, empty: bool, hand: &Hand) -> Table {
    let mut table = Table { id, head: Rc::new(head), rows: rows.into_iter().enumerate().map(|(ix, row)| (row, ix > 0)).collect() };
    if empty {
        let word = if hand.filtering { w::NOTHING_MATCHES } else { w::NOTHING };
        table.rows.push((Rc::new(move |_| Line::new(format!("{id}-empty"), word).empty().into()), false));
    }
    table
}

/// The Threads view: wsp's own parts, each thread heaviest first with the threads it started folded under it, then No
/// thread. A filter opens every group it left.
pub fn threads_table(name: String, groups: Rc<Vec<Group>>, looks: ThreadLooks, box_: bool, cost: Option<(f64, f64)>, hand: &Hand) -> Table {
    let (tops, kids) = nest_groups(&groups);
    let at = |g: &Group| groups.iter().position(|x| std::ptr::eq(x, g)).expect("a group among the groups");
    let mut rows = Vec::new();
    // A lead with all it folds open is ruled off the lead above, with no rule inside it.
    for (lead, g) in tops.into_iter().enumerate() {
        let mut block = Vec::new();
        thread_rows(g, 0, &kids, &at, cost, hand, &mut block);
        for (ix, row) in block.into_iter().enumerate() {
            let (groups, looks, hand) = (groups.clone(), looks.clone(), hand.clone());
            let make: MakeRow = Rc::new(move |cx: &App| row.make(&groups, &looks, box_, &hand, cx));
            rows.push((make, lead > 0 && ix == 0));
        }
    }
    let at = hand.clone();
    let head = move |cx: &App| head_row(at.tree(), name.clone(), sorts(&at, cx), cx);
    let mut table = table("activity-threads", head, Vec::new(), groups.is_empty(), hand);
    rows.append(&mut table.rows);
    table.rows = rows;
    table
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
pub fn processes_table(name: String, groups: Rc<Vec<Group>>, rows: Vec<Held>, hand: &Hand) -> Table {
    let empty = rows.is_empty();
    let items = rows
        .into_iter()
        .map(|(q, at)| {
            let (groups, hand) = (groups.clone(), hand.clone());
            Rc::new(move |cx: &App| {
                let (q, group, hand) = (&q, at.map(|at| &groups[at]), &hand);
                let p = cx.palette();
                let outside = group.is_none();
                let ending = hand.ending.contains(&q.pid);
                let bare = outside || hand.width.sm || (q.kind.untagged() && q.note.is_none());
                let tag =
                    (!bare).then(|| if ending { w::ENDING.to_owned() } else { q.note.map_or(q.kind.word().to_owned(), str::to_owned) });
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
                        v_flex().min_w_0().child(line).when(!outside, |el| el.child(small(where_of(group), true, cx).line_height(px(16.)))),
                    )
                    .into_any_element();
                let acts = Rc::new(proc_acts(q, group, None, hand));
                let mut parts = vec![lead];
                if !hand.width.md {
                    parts.push(
                        div()
                            .w_full()
                            .text_right()
                            .text_size(text::FACT.0)
                            .text_color(p.muted_foreground)
                            .font_features(text::tabular())
                            .child(q.pid.to_string())
                            .into_any_element(),
                    );
                }
                parts.extend(figures(&proc_key("", q), q.mem, q.cpu, q.energy, hand, true, cx));
                parts.push(menu_cell(format!("activity-flat-menu-{}", q.pid).into(), &acts, ending, hand, cx));
                sub_row(format!("activity-flat-{}", q.pid).into(), hand.flat(), parts, Some(q.command.clone().into()), Some(acts), hand, cx)
            }) as MakeRow
        })
        .collect();
    let at = hand.clone();
    let head = move |cx: &App| {
        let mut head = if at.width.md { Vec::new() } else { vec![head_word(w::PID, true, cx)] };
        head.extend(sorts(&at, cx));
        head_row(at.flat(), name.clone(), head, cx)
    };
    table("activity-processes", head, items, empty, hand)
}

/// The Kinds view: a group per kind, its processes flat under it, each naming the thread it belongs to.
pub fn kinds_table(name: String, groups: Rc<Vec<Group>>, kinds: Vec<(Kind, Vec<Held>)>, hand: &Hand) -> Table {
    let empty = kinds.is_empty();
    let mut rows: Vec<MakeRow> = Vec::new();
    for (kind, placed) in kinds {
        let open = hand.filtering || hand.open.contains(kind.id());
        let procs: Vec<Proc> = placed.iter().map(|(q, _)| q.clone()).collect();
        let (total, count, at) = (spent(&procs), placed.len(), hand.clone());
        rows.push(Rc::new(move |cx: &App| {
            group_head(
                kind.id(),
                &format!("k:{}", kind.id()),
                glyph(kind_glyph(kind), cx).into_any_element(),
                kind.head().into(),
                None,
                w::process_count(count),
                total,
                0,
                open,
                None,
                &at,
                false,
                cx,
            )
        }));
        if !open {
            continue;
        }
        for (q, at) in placed {
            let (groups, hand) = (groups.clone(), hand.clone());
            rows.push(Rc::new(move |cx: &App| {
                let (q, group, hand) = (&q, at.map(|at| &groups[at]), &hand);
                let ending = hand.ending.contains(&q.pid);
                let tags: Vec<gpui_kit::Div> = hand
                    .tag(Some(small(if ending { w::ENDING.to_owned() } else { where_of(group) }, true, cx)))
                    .into_iter()
                    .chain(ports_tag(&q.ports, cx))
                    .collect();
                let acts = Rc::new(proc_acts(q, group, None, hand));
                let mut parts = vec![lead_cell(hand.lead_pad(0), proc_glyph(q, cx), row_name(q.name.clone(), false, cx), tags)];
                parts.extend(figures(&proc_key("", q), q.mem, q.cpu, q.energy, hand, true, cx));
                parts.push(menu_cell(format!("activity-kind-menu-{}", q.pid).into(), &acts, ending, hand, cx));
                sub_row(
                    format!("activity-kind-{}-{}", kind.id(), q.pid).into(),
                    hand.tree(),
                    parts,
                    Some(q.command.clone().into()),
                    Some(acts),
                    hand,
                    cx,
                )
            }));
        }
    }
    let at = hand.clone();
    table("activity-kinds", move |cx| head_row(at.tree(), name.clone(), sorts(&at, cx), cx), rows, empty, hand)
}

/// The Ports view: every port a process wsp owns listens on, its thread, and who can reach it.
pub fn ports_table(groups: Rc<Vec<Group>>, rows: Vec<(Port, Proc, usize)>, hand: &Hand) -> Table {
    let empty = rows.is_empty();
    let items = rows
        .into_iter()
        .map(|(port, q, at)| {
            let (groups, hand) = (groups.clone(), hand.clone());
            Rc::new(move |cx: &App| {
                let (q, group, hand) = (&q, &groups[at], &hand);
                let p = cx.palette();
                let note = |word: String| {
                    div().min_w_0().truncate().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(word)
                };
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
                let acts = Rc::new(proc_acts(q, Some(group), Some(port), hand));
                let ending = hand.ending.contains(&q.pid);
                let open = open_port(q, Some(group), Some(port), hand).filter(|_| !ending).map(|(_, run)| {
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
                        .font_features(text::tabular())
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
            }) as MakeRow
        })
        .collect();
    let at = hand.clone();
    let head = move |cx: &App| {
        let head = if at.width.md {
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
        head_row(at.ports(), w::PORT.into(), head, cx)
    };
    table("activity-ports", head, items, empty, hand)
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
            pieces: Rc::default(),
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
