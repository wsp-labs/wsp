// SPDX-License-Identifier: AGPL-3.0-only
//! Activity's four views over one computer's groups, after `activity/tables.tsx`: a group row folds open onto its
//! processes, each with its kind's glyph, its figures inked only past their marks, and its acts in its menu.

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::component::menu::ContextMenuExt as _;
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClipboardItem, Hsla, SharedString, WeakEntity, Window};

use super::model::{
    cpu_tint, cpu_word, energy_tint, energy_word, flat, fmt_bytes, fmt_duration, fmt_tokens, fmt_uptime, group_spent, groups_under,
    memory_tint, nest_groups, ports_word, spent, total_spent, words as w, ContainerRow, Group, GroupKind, Placed, Port, Proc, Sort, Spent,
    Subagent, Tint,
};
use super::pieces::{cells, harness_mark, menu_button, MenuAct, COLUMN_GAP};
use super::wire::{Kind, Pid, PlacedBy};
use super::Scoped;
use crate::ui::grid::{ACTIVITY_COLUMNS, ACTIVITY_FLAT_COLUMNS as FLAT, ACTIVITY_PORT_COLUMNS as PORTS};
use crate::ui::{framed, glyph, spinner, text, ActivePalette, Card, CardItem, Col, Glyph, GridRow, Line, Status, StatusMark};

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
}

impl Hand {
    fn with(&self, cx: &mut App, f: impl FnOnce(&mut Scoped, &mut gpui_kit::Context<Scoped>)) {
        if let Some(s) = self.scoped.upgrade() {
            s.update(cx, f);
        }
    }

    fn run(&self, f: impl Fn(&mut Scoped, &mut gpui_kit::Context<Scoped>) + 'static) -> super::pieces::Run {
        let hand = self.clone();
        Rc::new(move |_: &mut Window, cx: &mut App| hand.with(cx, |s, cx| f(s, cx)))
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

fn copy(text: String) -> super::pieces::Run {
    Rc::new(move |_: &mut Window, cx: &mut App| cx.write_to_clipboard(ClipboardItem::new_string(text.clone())))
}

/// `procActions`: what a process row's menu offers. A working agent is stopped through its thread; a kept one between
/// turns is freed; only a process a thread or wsp started is ended; a heading's one act is End all on its runs.
fn proc_acts(p: &Proc, group: Option<&Group>, hand: &Hand) -> Vec<MenuAct> {
    let busy = hand.ending.contains(&p.pid);
    let thread = group.and_then(|g| g.thread.as_ref());
    let mut acts = Vec::new();
    let act = |glyph: Glyph, title: &str, destructive: bool, held: bool, run| MenuAct {
        group: 0,
        glyph,
        title: title.to_owned().into(),
        destructive,
        held,
        run,
    };
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
    if !p.heading && !p.command.is_empty() {
        acts.push(MenuAct { group: 2, ..act(Glyph::ClipboardCopy, w::COPY_COMMAND, false, false, copy(p.command.clone())) });
    }
    if !p.heading {
        acts.push(MenuAct { group: 2, ..act(Glyph::ClipboardCopy, w::COPY_PID, false, false, copy(p.pid.to_string())) });
    }
    acts
}

/// `containerActions`: End on one placed under its thread, and its name and id to copy.
fn container_acts(c: &ContainerRow, hand: &Hand) -> Vec<MenuAct> {
    let mut acts = Vec::new();
    if c.owned {
        let row = c.clone();
        acts.push(MenuAct {
            group: 0,
            glyph: Glyph::SquareX,
            title: w::END_CONTAINER.into(),
            destructive: true,
            held: hand.stopping.contains(&c.id),
            run: hand.run(move |s, cx| s.stop_container(&row, cx)),
        });
    }
    for (title, value) in [(w::COPY_NAME, c.name.clone()), (w::COPY_CONTAINER_ID, c.id.clone())] {
        acts.push(MenuAct {
            group: 2,
            glyph: Glyph::ClipboardCopy,
            title: title.into(),
            destructive: false,
            held: false,
            run: copy(value),
        });
    }
    acts
}

/// A row's menu cell: shown while the row is pointed at; the spinner while what it ends is ending.
fn menu_cell(id: SharedString, acts: &Rc<Vec<MenuAct>>, busy: bool, cx: &App) -> AnyElement {
    if busy {
        return h_flex().w_full().justify_end().child(spinner(cx)).into_any_element();
    }
    if acts.is_empty() {
        return div().into_any_element();
    }
    h_flex()
        .w_full()
        .justify_end()
        .opacity(0.)
        .group_hover("activity-row", |s| s.opacity(1.))
        .child(div().mx(px(-4.)).child(menu_button(id, acts.clone(), cx)))
        .into_any_element()
}

/// `SUB_ROW`: a row under a group, `py-2.5` on the card's inset, its menu on a right press too.
fn sub_row(
    id: SharedString,
    columns: &'static [Col],
    parts: Vec<AnyElement>,
    tip: Option<SharedString>,
    acts: Option<Rc<Vec<MenuAct>>>,
    cx: &App,
) -> CardItem {
    let hover = cx.palette().accent.opacity(0.6);
    let danger = cx.palette().error_foreground;
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
            Some(acts) => row.context_menu(move |menu, _, _| super::pieces::menu_of(menu, &acts, danger)).into_any_element(),
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
        .text_size(text::VALUE)
        .line_height(px(20.))
        .text_color(ink)
        .child(value)
        .into_any_element()
}

/// A row's figures; untinted for a sum across threads, which says nothing past one thread's marks.
fn figures(mem: f64, cpu: f64, energy: Option<f64>, hand: &Hand, tinted: bool, cx: &App) -> Vec<AnyElement> {
    let ink = |t: Option<Tint>| tint_ink(if tinted { t } else { None }, cx);
    vec![
        num_cell(fmt_bytes(mem), ink(memory_tint(mem))),
        num_cell(cpu_word(cpu), ink(cpu_tint(cpu))),
        num_cell(if hand.energy { energy_word(energy) } else { String::new() }, ink(energy_tint(energy))),
    ]
}

fn proc_glyph(p: &Proc, cx: &App) -> AnyElement {
    match &p.harness {
        Some(h) => harness_mark(h, &super::place::agent_name(h), px(16.), cx),
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
    let tags: Vec<gpui_kit::Div> = tag.map(|t| small(t, true, cx)).into_iter().chain(ports_tag(&p.ports, cx)).collect();
    let acts = Rc::new(proc_acts(p, Some(group), hand));
    let key = if p.heading { format!("{}-heading", p.kind.id()) } else { p.pid.to_string() };
    let mut parts = vec![lead_cell(44. + depth as f32 * 20., proc_glyph(p, cx), row_name(name, false, cx), tags)];
    parts.extend(figures(p.mem, p.cpu, p.energy, hand, true, cx));
    parts.push(menu_cell(format!("activity-menu-{}-{key}", group.id).into(), &acts, ending, cx));
    let tip = if p.heading {
        None
    } else if p.needed {
        Some(w::NEEDED.into())
    } else {
        Some(p.command.clone().into())
    };
    sub_row(format!("activity-proc-{}-{key}", group.id).into(), TREE, parts, tip, Some(acts), cx)
}

fn proc_rows(procs: &[Proc], depth: usize, group: &Group, hand: &Hand, cx: &App, out: &mut Vec<CardItem>) {
    for p in procs {
        out.push(proc_row(p, depth, group, hand, cx));
        proc_rows(&p.children, depth + 1, group, hand, cx, out);
    }
}

/// One of a thread's subagents: its title, then its state, how long it ran, what it spent and its model as tags.
fn subagent_row(s: &Subagent, nest: usize, group: &Group, model: Option<String>, cx: &App) -> CardItem {
    let mut facts = vec![w::subagent_state(s.state).to_owned(), fmt_duration(s.ms)];
    facts.extend(s.tokens.map(|t| w::tokens(&fmt_tokens(t))));
    facts.extend(model);
    let tags = vec![
        small(w::SUBAGENT, false, cx),
        h_flex().min_w_0().items_baseline().gap_2().overflow_hidden().children(facts.into_iter().map(|f| small(f, false, cx))),
    ];
    let glyph = Icon::new(Glyph::Bot).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let mut parts = vec![lead_cell(44. + nest as f32 * 20., glyph, row_name(s.title.clone(), false, cx), tags)];
    parts.extend([div().into_any_element(), div().into_any_element(), div().into_any_element(), div().into_any_element()]);
    sub_row(format!("activity-subagent-{}-{}", group.id, s.id).into(), TREE, parts, Some(w::SUBAGENT_NOTE.into()), None, cx)
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
    let tags = vec![
        small(if stopping { w::ENDING } else { w::CONTAINER }, false, cx),
        h_flex().min_w_0().items_baseline().gap_2().overflow_hidden().children(facts.into_iter().map(|f| small(f, true, cx))),
    ];
    let glyph = Icon::new(Glyph::Container).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let acts = Rc::new(container_acts(c, hand));
    let mut parts = vec![lead_cell(44. + nest as f32 * 20., glyph, row_name(c.name.clone(), false, cx), tags)];
    parts.push(c.mem.map_or_else(|| div().into_any_element(), |m| num_cell(fmt_bytes(m), tint_ink(memory_tint(m), cx))));
    parts.push(c.cpu.map_or_else(|| div().into_any_element(), |u| num_cell(cpu_word(u), tint_ink(cpu_tint(u), cx))));
    parts.push(div().into_any_element());
    parts.push(menu_cell(format!("activity-container-menu-{}", c.id).into(), &acts, stopping, cx));
    sub_row(format!("activity-container-{}", c.id).into(), TREE, parts, Some(note.into()), Some(acts), cx)
}

/// The sampler's own row, last under wsp: what reading this page costs the daemon.
fn sampler_row(cpu: f64, held: f64, hand: &Hand, cx: &App) -> CardItem {
    let glyph = Icon::new(Glyph::Gauge).with_size(px(16.)).flex_none().text_color(cx.palette().muted_foreground).into_any_element();
    let mut parts = vec![lead_cell(44., glyph, row_name(w::SAMPLER, false, cx), vec![small(w::SAMPLER_NOTE, true, cx)])];
    parts.extend(figures(held, cpu, None, hand, true, cx));
    parts.push(div().into_any_element());
    sub_row("activity-sampler".into(), TREE, parts, Some(w::SAMPLER_NOTE.into()), None, cx)
}

/// A group's head: its name and note, the figures it draws and the fold.
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
    hand: &Hand,
    tinted: bool,
    cx: &App,
) -> CardItem {
    let p = cx.palette();
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
        .children(status.map(StatusMark::new));
    let name_cell = h_flex().w_full().min_w_0().gap_3().pl(px(nest as f32 * 20.)).child(framed(mark, cx)).child(
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
    let mut row = GridRow::new(format!("activity-group-{id}"), TREE).on_open(toggle).cell(name_cell);
    for f in figures(total.mem, total.cpu, total.energy, hand, tinted, cx) {
        row = row.cell(f);
    }
    row.cell(chevron).into()
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
            let mark = harness_mark(&t.harness, &super::place::agent_name(&t.harness), px(16.), cx);
            out.push(group_head(&g.id, mark, t.title.clone(), (looks.status)(&t.id), t.project.clone(), total, nest, open, hand, true, cx));
            if open {
                proc_rows(&g.procs, nest, g, hand, cx, out);
                out.extend(
                    g.subagents.iter().map(|s| subagent_row(s, nest, g, s.model.as_deref().and_then(|m| (looks.model)(&t.harness, m)), cx)),
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
    list("activity-threads", head_row(TREE, name, sorts(hand, cx), cx), rows, groups.is_empty(), hand)
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
            let tag = (!outside && !(q.kind.untagged() && q.note.is_none())).then(|| {
                if ending {
                    w::ENDING.to_owned()
                } else {
                    q.note.map_or(q.kind.word().to_owned(), str::to_owned)
                }
            });
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
            let acts = Rc::new(proc_acts(q, row.group, hand));
            let mut parts = vec![
                lead,
                div()
                    .w_full()
                    .text_right()
                    .text_size(text::FACT)
                    .text_color(p.muted_foreground)
                    .child(q.pid.to_string())
                    .into_any_element(),
            ];
            parts.extend(figures(q.mem, q.cpu, q.energy, hand, true, cx));
            parts.push(menu_cell(format!("activity-flat-menu-{}", q.pid).into(), &acts, ending, cx));
            sub_row(format!("activity-flat-{}", q.pid).into(), FLAT, parts, Some(q.command.clone().into()), Some(acts), cx)
        })
        .collect();
    let mut head = vec![head_word(w::PID, true, cx)];
    head.extend(sorts(hand, cx));
    list("activity-processes", head_row(FLAT, name, head, cx), items, rows.is_empty(), hand)
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
            let tags: Vec<gpui_kit::Div> =
                std::iter::once(small(if ending { w::ENDING.to_owned() } else { where_of(row.group) }, true, cx))
                    .chain(ports_tag(&q.ports, cx))
                    .collect();
            let acts = Rc::new(proc_acts(q, row.group, hand));
            let mut parts = vec![lead_cell(44., proc_glyph(q, cx), row_name(q.name.clone(), false, cx), tags)];
            parts.extend(figures(q.mem, q.cpu, q.energy, hand, true, cx));
            parts.push(menu_cell(format!("activity-kind-menu-{}", q.pid).into(), &acts, ending, cx));
            rows.push(sub_row(
                format!("activity-kind-{}-{}", kind.id(), q.pid).into(),
                TREE,
                parts,
                Some(q.command.clone().into()),
                Some(acts),
                cx,
            ));
        }
    }
    list("activity-kinds", head_row(TREE, name, sorts(hand, cx), cx), rows, kinds.is_empty(), hand)
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
                    .child(harness_mark(&t.harness, &super::place::agent_name(&t.harness), px(14.), cx))
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
            let acts = Rc::new(proc_acts(q, Some(group), hand));
            let parts = vec![
                div()
                    .min_w_0()
                    .truncate()
                    .text_size(text::VALUE)
                    .line_height(px(20.))
                    .text_color(p.foreground)
                    .child(port.port.to_string())
                    .into_any_element(),
                h_flex().min_w_0().gap_2().child(proc_glyph(q, cx)).child(row_name(q.name.clone(), false, cx)).into_any_element(),
                thread,
                note(reach).into_any_element(),
                div().into_any_element(),
                menu_cell(format!("activity-port-menu-{}-{}", q.pid, port.port).into(), &acts, hand.ending.contains(&q.pid), cx),
            ];
            sub_row(format!("activity-port-{}-{}", q.pid, port.port).into(), PORTS, parts, Some(q.command.clone().into()), Some(acts), cx)
        })
        .collect();
    let head = vec![
        head_word(w::PROCESS, false, cx),
        head_word(w::THREAD, false, cx),
        head_word(w::REACH, false, cx),
        div().into_any_element(),
        div().into_any_element(),
    ];
    list("activity-ports", head_row(PORTS, w::PORT.into(), head, cx), items, rows.is_empty(), hand)
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
