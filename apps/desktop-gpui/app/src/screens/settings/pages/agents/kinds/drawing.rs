// SPDX-License-Identifier: AGPL-3.0-only
//! Drawing a tab: its list of groups and rows, an item's own page, its facts, picks and lists, and a skill's preview.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, App, Div, Entity, ScrollHandle, SharedString, Window};

use super::add::{add_query, add_server, add_skill};
use super::plugins::{plugin_detail, plugin_row_view};
use super::servers::{icon_host, server_detail, server_row_view, servers_shown};
use super::skills::{remote_detail, skill_detail, skill_row_view};
use super::{fold_servers, Choice, Detail, Fact, Group, Item, RowView, Under, K, PREVIEW_BYTES};
use crate::screens::settings::page::{Ctx, Level};
use crate::screens::settings::pages::agents::model::{read_when, refused_lines, w, Status, Tone};
use crate::screens::settings::pages::agents::parts::{
    agent_marks, lead, lead_mark, on_head, plain_head, refusal, status_word, step_mark, Lead,
};
use crate::screens::settings::pages::agents::store::{Doc, Store, Tab};
use crate::screens::settings::pages::agents::{flow, headed, keyed, not_read, with_under, Rows};
use crate::screens::settings::Acts;
use crate::ui::{
    act_button, catalog, scroll_view, size, text, tip, Act, ActivePalette, Button, Card, CardItem, CopyRow, Glyph, HeadRow, Line, Row,
    RowSkeleton, Run, Select, Skeleton, Switch, WordClass,
};

fn row_view(item: &Item, k: &K, cx: &App) -> RowView {
    match item {
        Item::Server(e) => server_row_view(e, k, cx),
        Item::Skill(s) => skill_row_view(s),
        Item::Plugin(p) => plugin_row_view(p, &k.rows.name),
    }
}

fn detail(item: &Item, k: &K, cx: &App) -> Detail {
    match item {
        Item::Server(e) => server_detail(e, k, cx),
        Item::Skill(s) => skill_detail(s, k, cx),
        Item::Plugin(p) => plugin_detail(p, k, cx),
    }
}

/// The one step a row stands in its state's place (`stepOf`).
fn step_of(row: &RowView) -> Option<Act> {
    row.quick.clone().filter(|q| q.id != "turn-on" && (q.run.is_some() || q.busy)).map(Act::word_only)
}

/// The step of a failed state, drawn as its glyph beside the state (`failedStep`).
fn failed_step(row: &RowView, status: Option<&Status>, step: Option<&Act>) -> Option<Act> {
    (status.is_some_and(|s| s.tone == Tone::Bad) && step.is_some() && row.quick.as_ref().is_some_and(|q| q.glyph.is_some()))
        .then(|| row.quick.clone())
        .flatten()
}

/// The switch an item's acts turn it on and off by, where the host can (`turnOf`).
fn turn_of(acts: &[Act]) -> Option<(bool, Act)> {
    let act = acts.iter().find(|a| a.id == "turn-on" || a.id == "turn-off")?;
    act.hover.is_none().then(|| (act.id == "turn-off", act.clone()))
}

fn turn_switch(key: &str, turn: (bool, Act)) -> AnyElement {
    let (on, act) = turn;
    let switch = Switch::new(SharedString::from(format!("{key}-on")), on).held(act.run.is_none());
    match act.run {
        Some(run) => switch.on_change(move |_, window, cx| run(window, cx)).into_any_element(),
        None => switch.into_any_element(),
    }
}

fn kind_status(key: &str, status: &Status, cx: &App) -> AnyElement {
    status_word(SharedString::from(format!("{key}-status")), &status.said(), status.tone, status.hover.as_deref(), cx)
}

/// What stands at a row's or a head's right: the failed state's step and the state, the step, the switch, or the state.
fn right(key: &str, row: &RowView, d: &Detail, with_turn: bool, acts: &Acts, cx: &App) -> Option<AnyElement> {
    let step = step_of(row);
    let status = row.status.clone().or_else(|| d.facts.iter().find(|f| f.id == "status").and_then(|f| f.status.clone()));
    let turn = if with_turn && row.status.is_none() { turn_of(&d.acts) } else { None };
    let failed = failed_step(row, status.as_ref(), step.as_ref());
    if let Some(failed) = failed {
        return Some(
            h_flex()
                .gap_3()
                .child(step_mark(format!("{key}-step"), &failed))
                .child(kind_status(key, status.as_ref()?, cx))
                .into_any_element(),
        );
    }
    if let Some(step) = step {
        return Some(act_button(format!("{key}-{}", step.id), &step, acts));
    }
    if let Some(turn) = turn {
        return Some(turn_switch(key, turn));
    }
    status.map(|s| kind_status(key, &s, cx))
}

fn kind_row(item: &Item, k: &K, window: &mut Window, cx: &mut App) -> CardItem {
    let row = row_view(item, k, cx);
    let d = detail(item, k, cx);
    let acts = k.s(cx).acts().clone();
    let control = right(&row.key, &row, &d, true, &acts, cx);
    let mut line = Row::new(SharedString::from(row.key.clone()), row.title.clone()).lead(lead(&row.lead, &row.title, cx)).mono().clip();
    if let Some(marks) = agent_marks(format!("{}-marks", row.key), &row.marks, cx) {
        line = line.mark(marks);
    }
    if let Some(sub) = &row.subtext {
        line = line.note(sub.clone());
    }
    if let Some(control) = control {
        line = line.control(control);
    }
    let level = Level { key: format!("item:{}", row.key).into(), name: row.title.clone().into(), up: None };
    let open = acts.clone();
    line = line.on_open(move |_, _, cx| open.open_level(Some(level.clone()), cx));
    let under = match (&d.flow, &d.refused) {
        (Some((row_id, flow)), _) => Some(flow::view(k.store, &k.rows.place, row_id, flow, false, window, cx)),
        (None, Some(said)) => Some(
            div()
                .text_size(text::FACT.0)
                .line_height(px(18.))
                .text_color(cx.palette().error_foreground)
                .child(said.clone())
                .into_any_element(),
        ),
        _ => None,
    };
    with_under(line, under)
}

/// A group's head: its name, and a project's folder after it.
fn group_head(group: &Group, first: Option<String>, cx: &App) -> AnyElement {
    super::super::parts::group_head(first.or_else(|| group.label.clone()).unwrap_or_default(), group.path.clone(), cx)
}

/// The field a tab's search or an add's is typed into, at the row controls' height.
pub(super) fn search_field(input: &Entity<gpui_kit::component::input::InputState>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let el = Styled::h(gpui_kit::component::input::Input::new(input), px(30.))
        .prefix(Icon::new(Glyph::Search).with_size(px(16.)).text_color(p.muted_foreground))
        .rounded(px(7.))
        .px(px(10.))
        .py_0()
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT.0)
        .w_full();
    div().flex_1().min_w_0().max_w(px(320.)).child(el).into_any_element()
}

/// One tab: its list, or the page of one of its items or of its add, as the crumbs' level names it.
pub fn tab(ctx: &Ctx, store: &Entity<Store>, rows: &Rows, tab: Tab, window: &mut Window, cx: &mut App) -> AnyElement {
    let (report, reading, error, read_at) = {
        let s = store.read(cx);
        let read = s.reads.get(&rows.place);
        (
            read.and_then(|r| r.report.clone()),
            read.is_some_and(|r| r.reading),
            read.and_then(|r| r.error.clone()),
            read.and_then(|r| r.read_at),
        )
    };
    let icons = ctx.data.preferences().is_none_or(|p| p.server_icons);
    let k = K { store, rows, report: report.as_ref(), icons };
    if tab == Tab::Servers {
        if let Some(report) = &report {
            servers_shown(store, rows, report, cx);
            for e in fold_servers(&report.servers) {
                if let Some(host) = icon_host(&e) {
                    store.update(cx, |s, cx| s.ask_icon(host, icons, cx));
                }
            }
        }
    }
    let items = report.as_ref().map(|r| tab.items(r)).unwrap_or_default();
    let level = ctx.level.map(|l| l.key.to_string());
    let held = rows.held.is_some();
    let adds = !held && tab.add().is_some();
    if let Some(level) = &level {
        if let Some(key) = level.strip_prefix("item:") {
            match items.iter().find(|i| i.key() == key) {
                Some(item) => return item_page(Some(&row_view(item, &k, cx)), &detail(item, &k, cx), &k, window, cx),
                None if report.is_some() => ctx.acts.open_level(None, cx),
                None => {}
            }
        } else if let Some(rest) = level.strip_prefix("found:") {
            let (key, query) = rest.split_once('\n').unwrap_or((rest, ""));
            let hit = store.read(cx).search(query.trim()).and_then(|s| s.hits.clone()).and_then(|h| h.into_iter().find(|h| h.id == key));
            match hit {
                Some(hit) if tab == Tab::Skills => return item_page(None, &remote_detail(&hit, &k, cx), &k, window, cx),
                _ if report.is_some() => ctx.acts.open_level(None, cx),
                _ => {}
            }
        } else if let (Some(query), true) = (add_query(level), adds) {
            match tab {
                Tab::Skills => return add_skill(ctx, &k, query, window, cx),
                Tab::Servers => return add_server(&k, window, cx),
                _ => {}
            }
        } else if report.is_some() {
            ctx.acts.open_level(None, cx);
        }
    }
    kind_list(ctx, &k, tab, items, report.is_some(), reading, error, read_at, window, cx)
}

#[allow(clippy::too_many_arguments)]
fn kind_list(
    ctx: &Ctx,
    k: &K,
    tab: Tab,
    items: Vec<Item>,
    has_report: bool,
    reading: bool,
    error: Option<String>,
    read_at: Option<std::time::SystemTime>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let name = k.rows.name.clone();
    let mut toolbar = h_flex().debug_selector(|| "kind-toolbar".into()).justify_between().gap_3();
    let mut query = String::new();
    if let Some(search) = tab.search() {
        let input = keyed::typed(format!("kind-search-{}-{}", k.rows.place, tab.id()), search, "", false, |_, _, _| {}, window, cx);
        query = keyed::words(&input, cx);
        toolbar = toolbar.child(search_field(&input, cx));
    }
    if let Some(add) = tab.add() {
        let held = k.rows.held.is_some() || !has_report;
        let mut button = Button::add(format!("kind-add-{}", tab.id()), add).height(px(30.)).held(held);
        if !held {
            let acts = ctx.acts.clone();
            let name = if tab == Tab::Skills { w::ADD_SKILL } else { w::ADD_SERVER };
            button = button.on_click(move |_, _, cx| acts.open_level(Some(Level { key: "add".into(), name: name.into(), up: None }), cx));
        }
        let el = match (&k.rows.held, has_report && held) {
            (Some(why), true) => div().id("kind-add-hover").tooltip(tip(why.clone())).child(button).into_any_element(),
            _ => button.into_any_element(),
        };
        toolbar = toolbar.child(el);
    }
    let matching: Vec<Item> = items.into_iter().filter(|i| i.matches(&query)).collect();
    let groups: Vec<Group> = tab.groups(matching).into_iter().filter(|g| !g.items.is_empty()).collect();
    let when = read_at.map(|at| w::checked_now(&read_when(at.elapsed().unwrap_or_default().as_millis() as u64)));
    let refresh: Run = {
        let (store, place) = (k.store.clone(), k.rows.place.clone());
        Rc::new(move |_, cx| {
            store.update(cx, |s, cx| {
                s.forget_icons();
                s.refresh(&place, cx)
            })
        })
    };
    let head = |text: AnyElement, cx: &App| {
        on_head(&format!("kind-{}", tab.id()), text, when.clone(), reading, Some(refresh.clone()), w::READ_AGENTS_AGAIN, cx)
    };
    let mut cards: Vec<AnyElement> = Vec::new();
    if !has_report && reading {
        cards.push(Card::new("kind-reading").head(w::on(&name)).body(RowSkeleton::new("agents-reading")).into_any_element());
    } else if groups.is_empty() {
        let mut card = Card::new("kind-none");
        if has_report {
            let q = query.trim();
            card = card.item(Line::new("kind-none", if q.is_empty() { tab.empty(&name) } else { w::nothing_matches(q) }).empty());
        }
        cards.push(headed(head(w::on(&name).into_any_element(), cx), card));
    } else {
        for (at, group) in groups.iter().enumerate() {
            let items: Vec<CardItem> = group.items.iter().map(|i| kind_row(i, k, window, cx)).collect();
            let card = Card::new(SharedString::from(format!("kind-{}", group.id))).items(items);
            let title = if at == 0 {
                let words = match &group.label {
                    None => w::on(&name),
                    Some(label) => w::group_on(label, &name),
                };
                head(group_head(group, Some(words), cx), cx)
            } else {
                plain_head(group_head(group, None, cx), cx)
            };
            cards.push(headed(title, card));
        }
    }
    let mut lines = refused_lines(k.refused());
    lines.extend(k.rows.misses.iter().cloned());
    if !has_report {
        if let Some(error) = error {
            lines.push((error, None));
        }
    }
    cards.extend(not_read(lines));
    v_flex().gap_3().child(toolbar).child(v_flex().gap(px(30.)).children(cards)).into_any_element()
}

/// Acts the item's page draws in a place of their own, or whose work the page already shows.
const PAGE_OWN: [&str; 4] = ["turn-on", "turn-off", "remove", "reconnect"];

/// One item's own page: its head with its state or its step and its switch, how it stands, its picks, what it lists,
/// its document, and Remove at the foot.
fn item_page(row: Option<&RowView>, d: &Detail, k: &K, window: &mut Window, cx: &mut App) -> AnyElement {
    if let Some(load) = d.doc.as_ref().and_then(|doc| doc.load.clone()) {
        load(cx);
    }
    let acts = k.s(cx).acts().clone();
    let step = row.and_then(step_of);
    let status = row.and_then(|r| r.status.clone()).or_else(|| d.facts.iter().find(|f| f.id == "status").and_then(|f| f.status.clone()));
    let failed = row.and_then(|r| failed_step(r, status.as_ref(), step.as_ref()));
    let turn = turn_of(&d.acts);
    let remove = d.acts.iter().find(|a| a.id == "remove").cloned();
    let page_acts: Vec<Act> = d
        .acts
        .iter()
        .filter(|a| !PAGE_OWN.contains(&a.id) && Some(a.id) != step.as_ref().map(|s| s.id) && (a.run.is_some() || a.busy))
        .cloned()
        .map(Act::word_only)
        .collect();
    let mut slot = h_flex().gap_3();
    for act in &page_acts {
        slot = slot.child(act_button(format!("kind-head-{}", act.id), act, &acts));
    }
    if let Some(failed) = failed {
        slot = slot.child(step_mark("kind-head-step", &failed));
        if let Some(status) = &status {
            slot = slot.child(kind_status("kind-head", status, cx));
        }
    } else if let Some(step) = &step {
        slot = slot.child(act_button(format!("kind-head-{}", step.id), step, &acts));
    } else if let (Some(status), true) = (&status, page_acts.is_empty()) {
        slot = slot.child(kind_status("kind-head", status, cx));
    }
    if let Some(turn) = turn {
        slot = slot.child(turn_switch("kind-head", turn));
    }
    let mut head = HeadRow::new("kind-head", lead_mark(&d.lead, &d.title, cx), d.title.clone()).slot(slot);
    if let Some(marks) = agent_marks("kind-head-marks", &d.marks, cx) {
        head = head.mark(marks);
    }
    if let Some(sub) = row.and_then(|r| r.subtext.clone()) {
        head = head.line(div().id("kind-head-line").truncate().tooltip(tip(sub.clone())).child(sub));
    }
    let p = cx.palette().clone();
    let mut section = v_flex().gap_3().child(Card::new("kind-head").item(head));
    if let Some(about) = &d.about {
        section =
            section.child(div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(about.clone()));
    }
    if let Some((row_id, f)) = &d.flow {
        section = section.child(flow::view(k.store, &k.rows.place, row_id, f, true, window, cx));
    } else if let Some(said) = &d.refused {
        section = section.child(refusal(Some(said)));
    }
    let mut out = vec![section.into_any_element()];
    let facts: Vec<&Fact> = d.facts.iter().filter(|f| f.id != "status" && (f.id != "tools" || d.under.is_none())).collect();
    let ones: Vec<&Choice> = d.choices.iter().filter(|c| !c.many).collect();
    if !facts.is_empty() || !ones.is_empty() {
        let mut card = Card::new("kind-facts").head(w::DETAILS);
        let mut label_for = String::new();
        for f in &facts {
            if !f.label.is_empty() {
                label_for = f.label.clone();
            }
            card = card.item(fact_line(f, &label_for, &acts, cx));
        }
        for c in ones {
            card = card.item(one_of(c, window, cx));
        }
        out.push(card.into_any_element());
    }
    for c in d.choices.iter().filter(|c| c.many) {
        out.push(choice_card(c, cx));
    }
    if let Some(under) = &d.under {
        out.push(under_card(under, cx));
    }
    if let Some(doc) = &d.doc {
        out.push(Card::new("kind-doc").head(w::SKILL_FILE).body(skill_preview(&doc.key, &doc.doc, window, cx)).into_any_element());
    }
    if let Some(remove) = remove {
        let row = Row::new("kind-remove", w::remove_name(&remove.label, &d.title))
            .note(remove.confirm.as_ref().map(|(_, b)| b.clone()).unwrap_or_default())
            .control(act_button("kind-remove-act", &remove.clone().word_only(), &acts));
        out.push(Card::new("kind-remove").item(row).into_any_element());
    }
    v_flex().gap(px(30.)).children(out).into_any_element()
}

/// An add's submit, which Enter in any of its fields runs as its button does.
pub(super) type Submit = Rc<dyn Fn(&mut App)>;

/// How long a fact's copy shows its check after it took (`COPIED_MS`).
const COPIED: std::time::Duration = std::time::Duration::from_millis(1_400);

/// `CopyGlyph`: a fact's copy, its check standing a moment after it took.
#[derive(IntoElement)]
struct FactCopy {
    id: SharedString,
    value: String,
    label: String,
}

impl RenderOnce for FactCopy {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // How many copies were pressed, and whether the last still shows its check; a later press keeps it longer.
        let copied = window.use_keyed_state(SharedString::from(format!("{}-copied", self.id)), cx, |_, _| (0u32, false));
        let shown = copied.read(cx).1;
        let value = self.value;
        Button::icon_xs(self.id, if shown { Glyph::Check } else { Glyph::Copy }).tooltip(self.label).on_click(move |_, window, cx| {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(value.clone()));
            let press = copied.update(cx, |c, cx| {
                *c = (c.0 + 1, true);
                cx.notify();
                c.0
            });
            let copied = copied.clone();
            window
                .spawn(cx, async move |cx| {
                    cx.background_executor().timer(COPIED).await;
                    copied.update(cx, |c, cx| {
                        if c.0 == press {
                            c.1 = false;
                            cx.notify();
                        }
                    });
                })
                .detach();
        })
    }
}

/// One fact as a line: its label, and at its right the agent it is for, its value, its note and its act.
pub(super) fn fact_line(f: &Fact, label_for: &str, acts: &Acts, cx: &App) -> CardItem {
    let p = cx.palette().clone();
    let id = SharedString::from(format!("fact-{}", f.id));
    if f.prose {
        return Row::new(id, f.label.clone()).note(f.value.clone().unwrap_or_default()).into();
    }
    if f.line {
        let (label, value) = (f.label.clone(), f.value.clone().unwrap_or_default());
        let ground = p.card;
        return CardItem::placed(move |corners| {
            corners
                .apply(v_flex())
                .px(corners.inset)
                .py_3()
                .min_h(size::LINE_FLOOR)
                .justify_center()
                .gap_2()
                .child(div().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1).child(label))
                .child(CopyRow::new(id.clone(), value).on(ground))
                .into_any_element()
        });
    }
    let mut right = h_flex().group("fact").flex_wrap().justify_end().gap_x_3().gap_y_1().min_w_0();
    if let (true, Some(value)) = (f.copy, &f.value) {
        let value = value.clone();
        right = right.child(div().opacity(0.).group_hover("fact", |s| s.opacity(1.)).child(FactCopy {
            id: SharedString::from(format!("fact-{}-copy", f.id)),
            value,
            label: format!("{} {}", w::COPY, label_for.to_lowercase()),
        }));
    }
    if let Some(agent) = &f.agent {
        right = right.child(catalog::mark(agent, &catalog::name(agent), px(14.), cx.palette().foreground, cx));
    }
    if let Some(status) = &f.status {
        right = right.child(kind_status(&format!("fact-{}", f.id), status, cx));
    }
    if let Some(value) = &f.value {
        right = right.child(match &f.href {
            Some(href) => {
                let href = href.clone();
                h_flex()
                    .id(SharedString::from(format!("fact-{}-link", f.id)))
                    .min_w_0()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(text::VALUE.0)
                    .text_color(p.foreground)
                    .font_features(text::tabular())
                    .hover(|s| s.underline())
                    .tooltip(tip(href.clone()))
                    .on_click(move |_, _, cx| cx.open_url(&href))
                    .child(div().truncate().child(value.clone()))
                    .child(Icon::new(Glyph::ExternalLink).with_size(px(12.)).text_color(p.muted_foreground))
                    .into_any_element()
            }
            None => crate::ui::word(value.clone(), if f.muted { WordClass::Fact } else { WordClass::Value }, cx)
                .id(SharedString::from(format!("fact-{}-value", f.id)))
                .debug_selector({
                    let id = f.id.clone();
                    move || format!("fact-{id}-value")
                })
                .text_right()
                .tooltip(tip(value.clone()))
                .into_any_element(),
        });
    }
    if let Some(note) = &f.note {
        right = right.child(crate::ui::word(note.clone(), WordClass::Fact, cx));
    }
    if let Some(status) = &f.note_status {
        right = right.child(kind_status(&format!("fact-{}-note", f.id), status, cx));
    }
    if let Some(act) = &f.act {
        right = right.child(act_button(format!("fact-{}-act", f.id), &act.clone().word_only(), acts));
    }
    Line::new(id, f.label.clone()).right(right).into()
}

/// One pick of a few as a line with its select.
fn one_of(c: &Choice, window: &mut Window, cx: &mut App) -> CardItem {
    let items = c.options.iter().map(|o| (o.value.clone(), SharedString::from(o.label.clone()), None)).collect();
    let select = keyed::pick(format!("choice-{}", c.id), items, c.value.first().cloned(), window, cx);
    let set = c.set.clone();
    let select = Select::new(&select).on_change(move |v: &String, _, cx| set(vec![v.clone()], cx));
    let right = v_flex().items_end().gap_1().child(select).children(c.lost.clone().map(|lost| crate::ui::word(lost, WordClass::Fact, cx)));
    Line::new(SharedString::from(format!("choice-{}", c.id)), c.label).right(right).into()
}

/// Several picks as rows, each with its switch, a held one standing on with its reason under it.
fn choice_card(c: &Choice, cx: &App) -> AnyElement {
    let rows = c.options.iter().map(|o| {
        let on = c.value.contains(&o.value);
        let (set, value, all) = (c.set.clone(), o.value.clone(), c.value.clone());
        let switch = Switch::new(SharedString::from(format!("choice-{}-{}-on", c.id, o.value)), on).held(o.held.is_some()).on_change(
            move |next, _, cx| {
                let picked: Vec<String> = if *next {
                    all.iter().cloned().chain([value.clone()]).collect()
                } else {
                    all.iter().filter(|v| **v != value).cloned().collect()
                };
                set(picked, cx)
            },
        );
        let mut row = Row::new(SharedString::from(format!("choice-{}-{}", c.id, o.value)), o.label.clone())
            .note(o.held.clone().unwrap_or_default())
            .control(switch);
        if let Some(agent) = &o.agent {
            row = row.lead(lead(&Lead::Agent(agent.clone()), &o.label, cx));
        }
        row
    });
    Card::new(SharedString::from(format!("kind-choice-{}", c.id))).head(c.label).items(rows).into_any_element()
}

/// What a level under a detail lists (a server's tools), with when they were read and the refresh.
fn under_card(u: &Under, cx: &App) -> AnyElement {
    let rows = u.rows.as_ref();
    let when = u.read_at.as_deref().and_then(parse_time).map(|at| w::checked_now(&read_when(at)));
    let head = on_head("kind-under", u.title.clone().into_any_element(), when, u.reading, u.refresh.clone(), "Read again", cx);
    let mut card = Card::new("kind-under");
    match rows {
        Some(rows) if !rows.is_empty() => {
            for r in rows {
                let mut row =
                    Row::new(SharedString::from(format!("under-{}", r.key)), r.title.clone()).note(r.subtext.clone().unwrap_or_default());
                if let Some(count) = &r.count {
                    row = row.clip().fact(count.clone());
                }
                card = card.item(row);
            }
        }
        _ if u.reading => card = card.body(RowSkeleton::new("under-reading")),
        _ => {
            let said = u.refused.clone().or_else(|| u.empty.clone()).unwrap_or_else(|| w::NOT_LISTED.into());
            card = card.item(Line::new("under-none", said).empty());
        }
    }
    headed(head, card)
}

/// How long ago an RFC 3339 time was, in ms; none where it does not read.
fn parse_time(at: &str) -> Option<u64> {
    let at = chrono::DateTime::parse_from_rfc3339(at).ok()?;
    let ms = chrono::Utc::now().signed_duration_since(at).num_milliseconds();
    Some(ms.max(0) as u64)
}

/// `skillBody`: the SKILL.md with its frontmatter taken off.
pub(super) fn skill_body(text: &str) -> &str {
    let rest = text.strip_prefix("---\n").or_else(|| text.strip_prefix("---\r\n"));
    let Some(rest) = rest else { return text };
    match rest.find("\n---") {
        Some(end) => {
            let after = &rest[end + 4..];
            let line_end = after.find('\n').map_or(after.len(), |i| i + 1);
            if after[..line_end].trim().is_empty() {
                &after[line_end..]
            } else {
                text
            }
        }
        None => text,
    }
}

/// `restrictedHref`: a link in a SKILL.md nobody here wrote opens only as a web page or a mail address.
pub(super) fn opens(url: &str) -> bool {
    let url = url.to_ascii_lowercase();
    url.starts_with("https:") || url.starts_with("http:") || url.starts_with("mailto:")
}

/// A skill's SKILL.md drawn whole under its facts, its frontmatter off; its bars while it is read.
pub(super) fn skill_preview(key: &str, doc: &Doc, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let scroll = window.use_keyed_state(SharedString::from(format!("skill-preview-{key}")), cx, |_, _| ScrollHandle::new());
    let scroll = scroll.read(cx).clone();
    let boxed = |body: Div, window: &mut Window, cx: &mut App| {
        let body = body.debug_selector(|| "skill-preview".into()).max_h(px(480.)).min_h(px(168.)).rounded(px(10.)).border_1();
        let body = body.border_color(p.border).bg(p.card).px_4().py_3();
        scroll_view("skill-preview-body", &scroll, body, window, cx).into_any_element()
    };
    let mut col = v_flex().gap(px(6.));
    if doc.text.is_some() && doc.size > PREVIEW_BYTES {
        col = col.child(crate::ui::word(
            format!("shows the first {} KB of {} KB", PREVIEW_BYTES / 1024, doc.size.div_ceil(1024)),
            WordClass::Fact,
            cx,
        ));
    }
    let body = match (&doc.text, &doc.error) {
        (Some(text), _) => {
            let body = skill_body(text).to_owned();
            let state = window.use_keyed_state(SharedString::from(format!("skill-md-{key}-{}", doc.size)), cx, move |_, cx| {
                gpui_kit::component::text::TextViewState::markdown(&body, cx)
            });
            let view = gpui_kit::component::text::TextView::new(&state).on_link_click(|url, _, _, cx| {
                if opens(url) {
                    cx.open_url(url);
                }
            });
            boxed(div().text_size(text::VALUE.0).child(view), window, cx)
        }
        (None, Some(error)) => refusal(Some(error)),
        _ => boxed(
            div().child(
                v_flex()
                    .gap_2()
                    .child(Skeleton::new(px(640.), px(14.)))
                    .child(Skeleton::new(px(640.), px(14.)))
                    .child(Skeleton::new(px(420.), px(14.))),
            ),
            window,
            cx,
        ),
    };
    col.child(body).into_any_element()
}
