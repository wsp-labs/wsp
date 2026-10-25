// SPDX-License-Identifier: AGPL-3.0-only
//! What stands where, after `apps/web/src/slate/pieces/runs.tsx`, `column.tsx`, `row.tsx`, `grid.tsx` and
//! `section.tsx`: a section, and the page around the sections, lay their children out as soft cards whose rows are the
//! children, a hairline between rows; pieces that are not rows stand bare between the cards. Also the rules the other
//! pieces read off a piece's place: where it stands, whether it fills its row, and whether it is the slate's one loud
//! piece of its kind.

use std::collections::HashSet;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{canvas, div, prelude::*, px, relative, AnyElement, App, Bounds, Pixels, SharedString, Transformation, Window};
use serde_json::{json, Value};

use super::look::{self, Tone};
use super::riders::{is_stat_cell, is_strip, ridden_by, strip_cells};
use super::{flags, PieceCx};
use crate::screens::slate::model::{self, own_path, Loud};
use crate::screens::slate::{truthy, Raise, Slate};
use crate::ui::{motion, tip, ActivePalette, Card, CardItem, Glyph};

/// The slate's card inset (`--settings-inset:16px` in `SlateView.tsx`).
pub const INSET: Pixels = px(16.);
/// A card row's floor, `min-h-11`.
pub const ROW_FLOOR: Pixels = px(44.);
/// The gap between a group's cards, `gap-3`.
pub const GAP: Pixels = px(12.);

// ---- reading a piece without drawing it ----

fn piece<'a>(slate: &'a Slate, id: &str) -> Option<&'a model::Piece> {
    slate.doc()?.piece(id)
}

fn kind<'a>(slate: &'a Slate, id: &str) -> Option<&'a str> {
    piece(slate, id).map(|p| p.kind.as_str())
}

fn children<'a>(slate: &'a Slate, id: &str) -> &'a [String] {
    piece(slate, id).map_or(&[], |p| p.children.as_slice())
}

fn parent<'a>(slate: &'a Slate, id: &str) -> Option<&'a str> {
    slate.doc()?.parent_id(id)
}

/// One prop of a piece as the host answered it: a literal as written, a live one once resolved.
pub fn read(slate: &Slate, id: &str, name: &str) -> Option<Value> {
    let v = piece(slate, id)?.prop(name)?;
    if model::is_live(v) { slate.resolved.get(&model::expr_of(v)).cloned() } else { Some(v.clone()) }.filter(|v| !v.is_null())
}

/// `twoWayPath`: the own path a two-way prop writes, its binding where that is a bare `$name` path.
pub fn two_way(v: Option<&Value>) -> Option<String> {
    let path = v?.as_object().filter(|o| o.len() == 1)?.get("bind")?.as_str()?.trim();
    own_path(path).map(|_| path.to_owned())
}

/// `gapOf`: tight 4, normal 12, loose 16.
pub fn gap_of(v: Option<&Value>) -> Pixels {
    match v.and_then(Value::as_str) {
        Some("tight") => px(4.),
        Some("loose") => px(16.),
        _ => GAP,
    }
}

/// `isNoteText`: a text written muted, quiet or small with no tone of its own, drawn as a note.
pub fn is_note_text(slate: &Slate, id: &str) -> bool {
    if kind(slate, id) != Some("text") {
        return false;
    }
    let (tone, emphasis, size) = (read(slate, id, "tone"), read(slate, id, "emphasis"), read(slate, id, "size"));
    let s = |v: &Option<Value>| v.as_ref().and_then(Value::as_str).map(str::to_owned);
    let (tone, emphasis, size) = (s(&tone), s(&emphasis), s(&size));
    if size.as_deref() == Some("large")
        || emphasis.as_deref() == Some("strong")
        || tone.as_deref().is_some_and(|t| t != "muted" && t != "default")
    {
        return false;
    }
    tone.as_deref() == Some("muted") || emphasis.as_deref() == Some("quiet") || size.as_deref() == Some("small")
}

/// Whether a group lays its children out in cards: a section, the root column, and a column standing in such a group.
pub fn is_group(slate: &Slate, id: &str) -> bool {
    match kind(slate, id) {
        Some("section") => true,
        Some("column") => !is_strip(slate, id) && parent(slate, id).is_none_or(|p| is_group(slate, p)),
        _ => false,
    }
}

/// A row of controls and no words: the slate's toolbar, bare above the cards.
pub fn is_toolbar(slate: &Slate, id: &str) -> bool {
    let all = children(slate, id);
    kind(slate, id) == Some("row") && !all.is_empty() && all.iter().all(|c| kind(slate, c).is_some_and(|k| flags(k).control))
}

/// Whether a piece in a group is a row of a card.
pub fn is_card_row(slate: &Slate, id: &str) -> bool {
    match kind(slate, id) {
        None => false,
        // A strip is a card of its own, never a row of the card the pieces after it gather into.
        _ if is_strip(slate, id) => false,
        Some("column") => !is_group(slate, id),
        Some("row") => !is_toolbar(slate, id) && !children(slate, id).iter().any(|c| kind(slate, c).is_some_and(|k| flags(k).heads)),
        // A state word alone on the page stands bare; after a card's rows it joins them, as `gather` says.
        Some(
            "section" | "heading" | "text" | "markdown" | "chart" | "donut" | "timeline" | "treemap" | "diagram" | "image" | "images"
            | "bars" | "button" | "table" | "stats" | "status",
        ) => false,
        Some(_) => true,
    }
}

/// As a row of a card, whether the piece draws its own rows edge to edge rather than taking the row's inset.
pub fn fills(slate: &Slate, id: &str) -> bool {
    match kind(slate, id) {
        Some("output" | "checklist" | "choices" | "number") => true,
        Some("facts") => read(slate, id, "layout").as_ref().and_then(Value::as_str) == Some("grid"),
        Some("grid" | "column") => is_strip(slate, id),
        Some("row") => is_stat_cell(slate, id) || is_strip(slate, id),
        _ => false,
    }
}

/// Where a piece stands: a row of a card, inside such a row, or bare on the page between cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Row,
    Inside,
    Page,
}

pub fn place_of(slate: &Slate, id: &str) -> Place {
    match parent(slate, id) {
        None => Place::Page,
        Some(p) if is_group(slate, p) => {
            if is_card_row(slate, id) {
                Place::Row
            } else {
                Place::Page
            }
        }
        Some(_) => Place::Inside,
    }
}

/// A grid or row holding bar lists and nothing else.
pub fn is_bar_holder(slate: &Slate, id: &str) -> bool {
    let all = children(slate, id);
    matches!(kind(slate, id), Some("grid" | "row")) && !all.is_empty() && all.iter().all(|c| kind(slate, c) == Some("bars"))
}

/// Bar lists switched in one card: two or more written one after another, or any held in a grid or row of nothing else.
pub fn in_bar_switch(slate: &Slate, id: &str) -> bool {
    if kind(slate, id) != Some("bars") {
        return false;
    }
    let Some(parent) = parent(slate, id) else { return false };
    if is_bar_holder(slate, parent) {
        return true;
    }
    let siblings = children(slate, parent);
    let Some(at) = siblings.iter().position(|s| s == id) else { return false };
    [at.checked_sub(1), Some(at + 1)].into_iter().flatten().filter_map(|i| siblings.get(i)).any(|o| kind(slate, o) == Some("bars"))
}

/// One run of a group's children: a piece bare on the page, a card of rows, or bar lists under one switch.
#[derive(Clone, Debug, PartialEq)]
pub enum Run {
    Bare(String),
    Card(Vec<String>),
    Lists(Vec<String>),
}

/// `Runs`: a group's shown children, the rows among them gathered into cards and the rest bare between.
pub fn gather(slate: &Slate, ids: &[String]) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for id in ids {
        let holder = is_bar_holder(slate, id);
        if holder || in_bar_switch(slate, id) {
            let lists = if holder { children(slate, id).to_vec() } else { vec![id.clone()] };
            match runs.last_mut() {
                Some(Run::Lists(last)) => last.extend(lists),
                _ => runs.push(Run::Lists(lists)),
            }
            continue;
        }
        let joins = is_card_row(slate, id) || ridden_by(slate, id).is_some() || is_note_text(slate, id) || kind(slate, id) == Some("status");
        match runs.last_mut() {
            Some(Run::Card(card)) if joins => card.push(id.clone()),
            _ if !is_card_row(slate, id) => runs.push(Run::Bare(id.clone())),
            _ => runs.push(Run::Card(vec![id.clone()])),
        }
    }
    runs
}

// ---- tones ----

/// `toneOf`: the tone a value names, accent kept for the slate's one loud piece and the rest drawn default.
pub fn tone_of(v: Option<&Value>, slate: &Slate, id: &str) -> Tone {
    match Tone::of(v) {
        Tone::Accent if !slate.is_loud(Loud::Accent, id) => Tone::Default,
        tone => tone,
    }
}

// ---- what lines up and what refreshes ----

/// `tablesBeside`: the pieces of this one's type drawn within the same bound (the root where there is none), itself
/// included, so stacked tables can share their column widths.
pub fn tables_beside(slate: &Slate, id: &str) -> Vec<String> {
    let Some(own) = kind(slate, id) else { return vec![id.to_owned()] };
    let bounds = |at: &str| kind(slate, at).is_some_and(|k| flags(k).bounds);
    let mut group = parent(slate, id);
    while let Some(g) = group {
        if bounds(g) || parent(slate, g).is_none() {
            break;
        }
        group = parent(slate, g);
    }
    let Some(group) = group else { return vec![id.to_owned()] };
    let mut out = Vec::new();
    let mut stack = vec![group];
    while let Some(at) = stack.pop() {
        if kind(slate, at) == Some(own) {
            out.push(at.to_owned());
        }
        if at == group || !bounds(at) {
            stack.extend(children(slate, at).iter().rev().map(String::as_str));
        }
    }
    out
}

/// Whether a formula text reads `$name`, a word scan as the clock check is.
fn reads_name(expr: &str, name: &str) -> bool {
    let word = format!("${name}");
    expr.match_indices(&word).any(|(at, _)| !expr[at + word.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_'))
}

/// The formula texts a piece reads, with the derived values they name read through.
fn reads_of(slate: &Slate, p: &model::Piece) -> Vec<String> {
    let mut out: Vec<String> = p.when.iter().map(|w| format!("({w})")).collect();
    out.extend(p.props.values().filter(|v| model::is_live(v)).map(model::expr_of));
    let Some(doc) = slate.doc() else { return out };
    let mut seen = HashSet::new();
    let mut at = 0;
    while at < out.len() {
        for (name, expr) in &doc.derived {
            if reads_name(&out[at], name) && seen.insert(name.clone()) {
                if let Some(e) = expr.as_str() {
                    out.push(e.to_owned());
                }
            }
        }
        at += 1;
    }
    out
}

/// Of these runs, the ones a piece under this one reads, short of a nested section and of a run's own output.
fn read_under(slate: &Slate, id: &str, runs: &[String]) -> Vec<String> {
    let mut found = HashSet::new();
    let mut seen = HashSet::new();
    let mut stack = vec![(id, true)];
    while let Some((at, top)) = stack.pop() {
        if !seen.insert(at) {
            continue;
        }
        let Some(p) = piece(slate, at) else { continue };
        if !top && flags(&p.kind).says_refreshing {
            continue;
        }
        for expr in reads_of(slate, p) {
            found.extend(runs.iter().filter(|r| reads_name(&expr, r)).cloned());
        }
        stack.extend(p.children.iter().map(|c| (c.as_str(), false)));
    }
    runs.iter().filter(|r| found.contains(*r)).cloned().collect()
}

/// `refreshingUnder`: whether a piece under this one reads a refreshing run it does not say itself.
pub fn refreshing_under(slate: &Slate, id: &str) -> bool {
    let Some(doc) = slate.doc() else { return false };
    let runs: Vec<String> = doc.runs.keys().filter(|r| slate.record.refreshing(r)).cloned().collect();
    !runs.is_empty() && !read_under(slate, id, &runs).is_empty()
}

/// `timedUnder`: the runs on a timer that a piece under this one reads.
pub fn timed_under(slate: &Slate, id: &str) -> Vec<String> {
    let Some(doc) = slate.doc() else { return Vec::new() };
    let runs: Vec<String> = doc.runs.iter().filter(|(_, d)| d.get("every").is_some_and(|e| !e.is_null())).map(|(r, _)| r.clone()).collect();
    if runs.is_empty() {
        return runs;
    }
    read_under(slate, id, &runs)
}

/// `seconds` in `consent.tsx`: seconds under two minutes or off whole minutes, then minutes, then hours.
fn seconds(s: f64) -> String {
    if s < 120. || s % 60. != 0. {
        format!("{} s", look::num_text(s))
    } else if s < 7200. || s % 3600. != 0. {
        format!("{} min", look::num_text(s / 60.))
    } else {
        format!("{} h", look::num_text(s / 3600.))
    }
}

/// `cadenceOf` for a run on a timer: how often it starts and whether it starts while the slate is off screen.
pub fn timer_cadence(slate: &Slate, run: &str) -> Option<String> {
    let decl = slate.doc()?.runs.get(run)?;
    let every = format!("Runs every {}", seconds(decl.get("every")?.as_f64()?));
    Some(if decl.get("always").and_then(Value::as_bool) == Some(true) {
        format!("{every}, also while this slate is not on screen")
    } else {
        format!("{every}, only while this slate is on screen")
    })
}

// ---- drawing ----

/// The settings head in the locked look: 14 px regular in the foreground at 70%, 28 px tall.
pub fn head(text: impl Into<SharedString>, cx: &App) -> gpui_kit::Div {
    crate::ui::card::locked_head(text, cx).line_height(px(20.))
}

/// The width an element of this piece had last frame, and the probe that measures it; the probe goes in a relative
/// box. A new width draws the window again, as a container query answers at once.
pub fn measured(key: SharedString, window: &mut Window, cx: &mut App) -> (Option<Pixels>, AnyElement) {
    let state = window.use_keyed_state(key, cx, |_, _| None::<Pixels>);
    let width = *state.read(cx);
    let probe = canvas(
        move |bounds: Bounds<Pixels>, window, cx| {
            let w = bounds.size.width;
            if *state.read(cx) != Some(w) {
                state.update(cx, |s, _| *s = Some(w));
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full()
    .into_any_element();
    (width, probe)
}

/// A soft card of rows at the slate's inset: each row 44 px at the least, 16 px in and 12 px above and below, unless
/// it draws its own rows edge to edge.
pub fn card(key: SharedString, rows: Vec<(bool, AnyElement)>) -> Option<AnyElement> {
    if rows.is_empty() {
        return None;
    }
    let items = rows.into_iter().map(|(fill, el)| {
        CardItem::placed(move |c| {
            if fill {
                c.apply(div().min_w_0()).child(el).into_any_element()
            } else {
                c.apply(v_flex().min_w_0().min_h(ROW_FLOOR).justify_center().px(c.inset).py(px(12.))).child(el).into_any_element()
            }
        })
    });
    Some(Card::new(key).locked(true).inset(INSET).items(items).into_any_element())
}

/// One piece standing alone in a card surface of its own, as a strip or a table on the page draws it.
pub fn surface(key: SharedString, el: impl IntoElement + 'static) -> AnyElement {
    Card::new(key)
        .locked(true)
        .inset(INSET)
        .item(CardItem::placed(move |c| c.apply(div().min_w_0()).child(el).into_any_element()))
        .into_any_element()
}

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "column" => column(p, window, cx),
        "row" => row(p, window, cx),
        "grid" => grid(p, window, cx),
        "section" => section(p, window, cx),
        _ => None,
    }
}

/// A group's children drawn as runs, each with the kind of its first piece for the spacing rules.
fn runs(p: &PieceCx, window: &mut Window, cx: &mut App) -> Vec<(Option<String>, AnyElement)> {
    let shown: Vec<String> = p.piece.children.iter().filter(|c| p.slate.shown(c)).cloned().collect();
    let mut out = Vec::new();
    for run in gather(p.slate, &shown) {
        match run {
            Run::Bare(id) => {
                if let Some(el) = p.child(&id, window, cx) {
                    out.push((kind(p.slate, &id).map(str::to_owned), el));
                }
            }
            Run::Card(ids) => {
                let key = p.el(&format!("card-{}", ids[0]));
                let rows = ids.iter().filter_map(|id| p.child(id, window, cx).map(|el| (fills(p.slate, id), el))).collect();
                if let Some(el) = card(key, rows) {
                    out.push((None, el));
                }
            }
            Run::Lists(ids) => out.push((Some("bars".into()), super::barswitch::bar_switch(p, &ids, window, cx))),
        }
    }
    out
}

fn column(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if ridden_by(p.slate, p.id()).is_some() {
        return None;
    }
    if is_strip(p.slate, p.id()) {
        return Some(strip(p, window, cx));
    }
    if is_group(p.slate, p.id()) {
        let mut prev_section = false;
        let mut first = true;
        let items: Vec<AnyElement> = runs(p, window, cx)
            .into_iter()
            .map(|(k, el)| {
                let section = k.as_deref() == Some("section");
                // Sections stand 32 px from what is beside them: the column's 12 px gap and 20 more.
                let apart = (section && !first) || prev_section;
                (prev_section, first) = (section, false);
                div().min_w_0().when(apart, |d| d.mt(px(20.))).child(el).into_any_element()
            })
            .collect();
        return Some(v_flex().min_w_0().gap(GAP).children(items).into_any_element());
    }
    let el = v_flex().min_w_0().gap(gap_of(p.prop("gap")));
    let el = match p.str("align").as_deref() {
        Some("start") => el.items_start(),
        Some("center") => el.items_center(),
        Some("end") => el.items_end(),
        _ => el,
    };
    Some(el.children(p.children(window, cx)).into_any_element())
}

/// With no align set, whether a child sits at the row's end: the first button after other pieces, a toggle after a
/// select, a status after other pieces, a list of facts after other pieces.
fn at_end(prev: Option<&str>, this: &str) -> bool {
    match (prev, this) {
        (Some(p), "button" | "status" | "facts") => p != this,
        (Some("select"), "toggle") => true,
        _ => false,
    }
}

fn row(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if is_bar_holder(p.slate, p.id()) {
        return Some(super::barswitch::bar_switch(p, &p.piece.children, window, cx));
    }
    if is_strip(p.slate, p.id()) {
        return Some(strip(p, window, cx));
    }
    let slate = p.slate;
    let aligned = p.prop("align").is_some();
    let nowrap = p.prop("wrap") == Some(&Value::Bool(false));
    let mut prev: Option<&str> = None;
    let mut items = Vec::new();
    for c in &p.piece.children {
        let Some(el) = p.child(c, window, cx) else { continue };
        let k = kind(slate, c).unwrap_or_default();
        let end = !aligned && at_end(prev, k);
        prev = Some(k);
        items.push(div().min_w_0().when(end, |d| d.ml_auto()).when(nowrap, |d| d.flex_shrink(1.)).child(el).into_any_element());
    }
    let beside_number = p.piece.children.iter().any(|c| kind(slate, c) == Some("number"));
    let el = h_flex().min_w_0().gap(gap_of(p.prop("gap")));
    // Beside a number, the rest stands on the figure's line, not halfway between its label and its figure.
    let el = if beside_number { el.items_end() } else { el.items_center() };
    let el = match p.str("align").as_deref() {
        Some("center") => el.justify_center(),
        Some("end") => el.justify_end(),
        Some("between") => el.justify_between(),
        _ => el.justify_start(),
    };
    let el = if nowrap { el.flex_nowrap() } else { el.flex_wrap() };
    Some(el.children(items).into_any_element())
}

fn grid(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    if is_bar_holder(p.slate, p.id()) {
        return Some(super::barswitch::bar_switch(p, &p.piece.children, window, cx));
    }
    if is_strip(p.slate, p.id()) {
        return Some(strip(p, window, cx));
    }
    let asked = p.num("columns").map(|n| n as u16).filter(|n| (2..=4).contains(n)).unwrap_or(2);
    let (width, probe) = measured(p.el("width"), window, cx);
    // Columns from 360 px of the grid's own width; under it, one column.
    let cols = if width.is_none_or(|w| w >= px(360.)) { asked } else { 1 };
    let align = p.str("align");
    let items: Vec<AnyElement> = p
        .children(window, cx)
        .into_iter()
        .map(|el| {
            let cell = h_flex().min_w_0();
            match align.as_deref() {
                Some("center") => cell.justify_center().text_center(),
                Some("end") => cell.justify_end().text_right(),
                _ => cell,
            }
            .child(div().min_w_0().flex_1().child(el))
            .into_any_element()
        })
        .collect();
    Some(
        div()
            .relative()
            .min_w_0()
            .child(div().grid().grid_cols(cols).gap(px(16.)).min_w_0().children(items))
            .child(probe)
            .into_any_element(),
    )
}

// ---- the stat strip ----

/// One character of the 26 px mono figure, and a 12 px unit with its 4 px gap per character.
const FIGURE_CH: f64 = 15.6;
const UNIT_CH: f64 = 7.2;
/// One character of the 13 px sans label, which keeps to one line so the figures under it share theirs.
const LABEL_CH: f64 = 6.4;
/// The card's inner width a strip can count on: the 400 px panel's, and the least a widened panel's (34rem).
const NARROW: f64 = 366.;
const WIDE: f64 = 542.;
const WIDE_FROM: Pixels = px(544.);

/// `across`: cells per row, the most whose figures fit their cells (at most `cap`), then the count near it that
/// leaves the last row fullest, the larger on a tie.
pub fn across(count: usize, widest: f64, room: f64, cap: usize) -> usize {
    let fits = |c: usize| c as f64 * (widest + if c >= 3 { 24. } else { 32. }) <= room;
    let mut most = count.min(cap).max(1);
    while most > 1 && !fits(most) {
        most -= 1;
    }
    let short = |c: usize| (c - count % c) % c;
    let mut best = most;
    for c in (most.div_ceil(2).max(1)..=most).rev() {
        if short(c) < short(best) {
            best = c;
        }
    }
    best
}

/// The widest a strip cell needs: its figure and unit, or its one-line label.
fn cell_width(slate: &Slate, id: &str) -> f64 {
    let figure = look::figure(read(slate, id, "value").as_ref(), read(slate, id, "format").as_ref()).map_or(0, |f| f.chars().count());
    let unit = look::str(read(slate, id, "unit").as_ref());
    let label = look::str(read(slate, id, "label").as_ref()).map_or(0, |l| l.chars().count());
    let figure = figure as f64 * FIGURE_CH + unit.map_or(0., |u| u.chars().count() as f64 * UNIT_CH + 4.);
    figure.max(label as f64 * LABEL_CH)
}

/// The stat strip: one card split into equal cells with hairlines between them, a number per cell.
fn strip(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let slate = p.slate;
    let cells: Vec<String> = strip_cells(slate, p.id()).unwrap_or_default().into_iter().filter(|c| slate.shown(c)).collect();
    let widest = cells.iter().map(|c| cell_width(slate, c)).fold(0., f64::max);
    let (width, probe) = measured(p.el("strip"), window, cx);
    let per =
        if width.is_some_and(|w| w >= WIDE_FROM) { across(cells.len(), widest, WIDE, 6) } else { across(cells.len(), widest, NARROW, 3) };
    let inset = if per >= 3 { px(12.) } else { INSET };
    let rule = cx.palette().border.opacity(0.5);
    let drawn: Vec<AnyElement> = cells.iter().filter_map(|c| p.child(c, window, cx)).collect();
    let mut rows = Vec::new();
    let mut drawn = drawn.into_iter().peekable();
    let mut first_row = true;
    while drawn.peek().is_some() {
        let line: Vec<AnyElement> = drawn
            .by_ref()
            .take(per)
            .enumerate()
            .map(|(at, el)| {
                div()
                    .w(relative(1. / per as f32))
                    .min_w_0()
                    .pt(px(16.))
                    .pb(px(14.))
                    .px(inset)
                    .border_color(rule)
                    .when(at > 0, |d| d.border_l_1())
                    .child(el)
                    .into_any_element()
            })
            .collect();
        rows.push(h_flex().items_stretch().min_w_0().border_color(rule).when(!first_row, |d| d.border_t_1()).children(line));
        first_row = false;
    }
    let body = div().relative().min_w_0().debug_selector(|| "slate-strip".into()).child(v_flex().min_w_0().children(rows)).child(probe);
    if place_of(slate, p.id()) == Place::Page {
        surface(p.el("strip-card"), body)
    } else {
        body.into_any_element()
    }
}

// ---- the section ----

fn section(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let slate = p.slate;
    let id = p.id().to_owned();
    let title = p.str("title");
    let note = p.str("note");
    let raw_open = p.piece.prop("open");
    let path = two_way(raw_open);
    let folds = p.bool("collapsible") || raw_open.is_some();
    let asked = p.prop("open").is_none_or(|v| truthy(Some(v)));
    // A fold that names no state path is the window's own, kept by piece id for the window's life.
    let open = if path.is_some() { asked } else { slate.folds.get(&id).copied().unwrap_or(asked) };
    let refreshing = refreshing_under(slate, &id);
    let timed = timed_under(slate, &id);
    let palette = cx.palette().clone();

    let head_row = h_flex().min_h(px(28.)).min_w_0().w_full().gap(GAP).text_size(px(14.)).line_height(px(20.));
    let title_el = div().min_w_0().when(note.is_some(), |d| d.max_w(relative(0.65)).flex_shrink_0()).children(title.clone());
    let note_el = match &note {
        None => div().flex_1(),
        Some(n) => div()
            .debug_selector(|| "slate-section-note".into())
            .min_w_0()
            .flex_1()
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(palette.muted_foreground)
            .when(!folds, |d| d.text_right())
            .child(n.clone()),
    };
    let refresh = (!timed.is_empty() || refreshing).then(|| {
        let words = timed.iter().filter_map(|r| timer_cadence(slate, r)).collect::<Vec<_>>().join("\n");
        let glyph = Icon::new(Glyph::RefreshCw).with_size(px(14.)).text_color(palette.muted_foreground);
        let glyph = if refreshing { crate::ui::status::turning(glyph, window, cx) } else { glyph };
        div()
            .id(p.el("refresh"))
            .debug_selector(|| "slate-refresh".into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(24.))
            .my(px(-2.))
            .rounded(px(10.))
            .child(glyph)
            .when(!words.is_empty(), |d| d.tooltip(tip(words)))
    });
    let body = |window: &mut Window, cx: &mut App| v_flex().min_w_0().gap(GAP).children(section_body(p, window, cx));

    if !folds {
        let body = body(window, cx);
        let head = head_row.text_color(palette.foreground.opacity(0.7)).child(title_el).child(note_el).children(refresh);
        return Some(v_flex().min_w_0().gap(px(10.)).debug_selector(|| "slate-section".into()).child(head).child(body).into_any_element());
    }

    let over = motion::hovered(p.el("head"), window, cx);
    let ink = motion::ease_ink(
        p.el("head"),
        "ink",
        if *over.read(cx) { palette.foreground } else { palette.foreground.opacity(0.7) },
        motion::COLORS,
        window,
        cx,
    );
    let turn = motion::ease_px(p.el("chevron"), "turn", px(if open { 0.25 } else { 0. }), motion::COLORS, window, cx);
    let chevron = Icon::new(Glyph::ChevronRight)
        .with_size(px(14.))
        .flex_none()
        .text_color(palette.muted_foreground)
        .transform(Transformation::rotate(gpui_kit::percentage(f32::from(turn))));
    let screen = p.screen.clone();
    let has_change = p.piece.on.contains_key("change");
    let head = head_row
        .id(p.el("head"))
        .cursor_pointer()
        .text_color(ink)
        .on_hover(motion::track_hover(over))
        .on_click(move |_, _, cx| {
            let next = !open;
            match &path {
                Some(path) => {
                    let path = path.clone();
                    _ = screen.update(cx, |s, cx| s.write(path, json!(next), cx));
                }
                None => {
                    let id = id.clone();
                    _ = screen.update(cx, |s, cx| s.fold(id, next, cx));
                }
            }
            if has_change {
                let id = id.clone();
                _ = screen.update(cx, |s, cx| s.raise(id, "change", Raise::default(), cx));
            }
        })
        .child(title_el)
        .child(note_el)
        .children(refresh)
        .child(chevron);
    let panel = |window: &mut Window, cx: &mut App| {
        div().debug_selector(|| "slate-section-panel".into()).pt(px(10.)).child(body(window, cx)).into_any_element()
    };
    let panel = motion::fold(p.el("fold"), open, panel, window, cx);
    Some(v_flex().min_w_0().debug_selector(|| "slate-section".into()).child(head).children(panel).into_any_element())
}

/// A section's runs: a chart's foot line 10 px under it, a table after other pieces 16 px clear.
fn section_body(p: &PieceCx, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    let mut prev: Option<String> = None;
    let mut first = true;
    runs(p, window, cx)
        .into_iter()
        .map(|(k, el)| {
            let foot = prev.as_deref() == Some("chart") && k.as_deref() == Some("text");
            let table = !first && k.as_deref() == Some("table");
            (prev, first) = (k, false);
            div().min_w_0().when(foot, |d| d.mt(px(-2.))).when(table, |d| d.mt(px(4.))).child(el).into_any_element()
        })
        .collect()
}

#[cfg(test)]
pub mod tests {
    use serde_json::{json, Value};

    use super::*;
    use crate::screens::slate::model::{Doc, Record};

    /// A slate of these pieces under a root column, every prop literal.
    pub fn slate(pieces: Value) -> Slate {
        let doc = json!({"schema": 2, "root": "root", "pieces": pieces});
        Slate::new(Record { doc: Doc::read(&doc), ..Record::default() })
    }

    fn ids(s: &[&str]) -> Vec<String> {
        s.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn a_group_gathers_rows_into_cards_and_leaves_the_rest_bare() {
        let s = slate(json!({
            "root": {"type": "column", "children": ["h", "n", "m", "t", "note", "tbl", "b1", "b2", "sec", "tb"]},
            "h": {"type": "heading", "props": {"value": "Head"}},
            "n": {"type": "number", "props": {"value": 1}},
            "m": {"type": "meter", "props": {"value": 2}},
            "t": {"type": "text", "props": {"value": "prose"}},
            "note": {"type": "text", "props": {"value": "meta", "tone": "muted"}},
            "tbl": {"type": "table"},
            "b1": {"type": "bars"},
            "b2": {"type": "bars"},
            "sec": {"type": "section", "children": ["x"]},
            "x": {"type": "facts"},
            "tb": {"type": "row", "children": ["sel", "btn"]},
            "sel": {"type": "select"},
            "btn": {"type": "button"},
        }));
        let all = ids(&["h", "n", "m", "t", "note", "tbl", "b1", "b2", "sec", "tb"]);
        assert_eq!(
            gather(&s, &all),
            vec![
                Run::Bare("h".into()),
                Run::Card(ids(&["n", "m"])),
                Run::Bare("t".into()),
                Run::Bare("note".into()),
                Run::Bare("tbl".into()),
                Run::Lists(ids(&["b1", "b2"])),
                Run::Bare("sec".into()),
                Run::Bare("tb".into()),
            ]
        );
        assert_eq!(
            gather(&s, &ids(&["n", "note", "m"])),
            vec![Run::Card(ids(&["n", "note", "m"]))],
            "a meta line among rows joins their card"
        );
        assert_eq!(place_of(&s, "n"), Place::Row);
        assert_eq!(place_of(&s, "x"), Place::Row);
        assert_eq!(place_of(&s, "sel"), Place::Inside);
        assert_eq!(place_of(&s, "tbl"), Place::Page);
        assert!(is_toolbar(&s, "tb"));
    }

    #[test]
    fn a_status_after_a_number_rides_its_card() {
        let s = slate(json!({
            "root": {"type": "column", "children": ["n", "st", "tx", "g"]},
            "n": {"type": "number", "props": {"value": 3}},
            "st": {"type": "status", "props": {"value": "slow"}},
            "tx": {"type": "text", "props": {"value": "since", "size": "small"}},
            "g": {"type": "grid", "children": ["a", "s2", "t2", "b"]},
            "a": {"type": "number"}, "s2": {"type": "status"}, "t2": {"type": "text"}, "b": {"type": "number"},
        }));
        // The strip after them is a card of its own, never a row of theirs.
        assert_eq!(gather(&s, &ids(&["n", "st", "tx", "g"])), vec![Run::Card(ids(&["n", "st", "tx"])), Run::Bare("g".into())]);
        assert_eq!(super::super::riders::riders_of(&s, "n").all, ids(&["st", "tx"]));
        assert_eq!(strip_cells(&s, "g"), Some(ids(&["a", "b"])));
        assert_eq!(super::super::riders::riders_of(&s, "a").all, ids(&["s2", "t2"]));
        assert!(fills(&s, "g"));
    }

    #[test]
    fn a_strip_counts_its_cells_across() {
        assert_eq!(across(3, 60., NARROW, 3), 3);
        assert_eq!(across(4, 60., NARROW, 3), 2, "four in a narrow panel leave no row short at two");
        assert_eq!(across(5, 60., NARROW, 3), 3);
        assert_eq!(across(3, 200., NARROW, 3), 1, "a wide figure stands alone");
        assert_eq!(across(6, 60., WIDE, 6), 6);
        assert_eq!(across(7, 60., WIDE, 6), 4);
        assert_eq!(across(1, 60., NARROW, 3), 1);
    }

    #[test]
    fn an_accent_past_the_first_draws_default() {
        let s = slate(json!({
            "root": {"type": "column", "children": ["c1", "c2", "b", "t"]},
            "c1": {"type": "chart"},
            "c2": {"type": "chart"},
            "b": {"type": "button", "props": {"variant": "primary"}},
            "t": {"type": "text", "props": {"tone": "accent"}},
        }));
        assert_eq!(tone_of(Some(&json!("accent")), &s, "t"), Tone::Default);
    }

    #[test]
    fn a_section_says_its_timed_runs() {
        let mut s = slate(json!({
            "root": {"type": "column", "children": ["sec"]},
            "sec": {"type": "section", "children": ["n"]},
            "n": {"type": "number", "props": {"value": {"bind": "$disk.result"}}},
        }));
        if let Some(doc) = s.record.doc.as_mut() {
            doc.runs.insert("disk".into(), json!({"kind": "cmd", "cmd": "df", "every": 30}));
            doc.runs.insert("diskette".into(), json!({"kind": "cmd", "cmd": "df", "every": 600}));
        }
        assert_eq!(timed_under(&s, "sec"), ids(&["disk"]));
        assert_eq!(timer_cadence(&s, "disk").as_deref(), Some("Runs every 30 s, only while this slate is on screen"));
        assert_eq!(timer_cadence(&s, "diskette").as_deref(), Some("Runs every 10 min, only while this slate is on screen"));
    }

    struct Drawn(Slate);

    impl Render for Drawn {
        fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full().children(super::super::draw(&self.0, "root", &gpui_kit::WeakEntity::new_invalid(), window, cx))
        }
    }

    #[gpui_kit::test]
    fn a_fold_opens_and_shuts_over_200_ms_between_its_head_and_its_height(cx: &mut gpui_kit::TestAppContext) {
        use gpui_kit::test::TestWindowExt as _;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let s = slate(json!({
            "root": {"type": "column", "children": ["sec"]},
            "sec": {"type": "section", "props": {"title": "Fold", "collapsible": true, "open": false}, "children": ["a", "b", "c"]},
            "a": {"type": "text", "props": {"value": "One"}},
            "b": {"type": "text", "props": {"value": "Two"}},
            "c": {"type": "text", "props": {"value": "Three"}},
        }));
        let (view, cx) = cx.add_window_view(move |_, _| Drawn(s));
        cx.simulate_resize(gpui_kit::size(px(540.), px(600.)));
        let height = |cx: &mut gpui_kit::VisualTestContext, ms: u64| {
            cx.executor().advance_clock(std::time::Duration::from_millis(ms));
            for _ in 0..3 {
                cx.update(|window, cx| window.render_frame(cx));
                cx.run_until_parked();
            }
            f32::from(cx.debug_bounds("slate-section").expect("the section is drawn").size.height)
        };
        let shut = height(cx, 0);
        let fold = |open: bool, cx: &mut gpui_kit::VisualTestContext| {
            view.update(cx, |d, cx| {
                d.0.folds.insert("sec".into(), open);
                cx.notify();
            })
        };
        fold(true, cx);
        let opening = height(cx, 100);
        let open = height(cx, 300);
        assert!(shut < opening && opening < open, "half way it stands between its head and its height: {shut} {opening} {open}");
        fold(false, cx);
        let shutting = height(cx, 100);
        assert!(shut < shutting && shutting < open, "it shuts as it opened: {shut} {shutting} {open}");
        assert_eq!(height(cx, 300), shut);
    }
}
