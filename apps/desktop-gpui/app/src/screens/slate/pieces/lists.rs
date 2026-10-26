// SPDX-License-Identifier: AGPL-3.0-only
//! Rows from a list, after `apps/web/src/slate/pieces/table.tsx` and `checklist.tsx`. A table: a header as its card's
//! first row, whose first cell names the list, the rows in the card on one set of column widths shared with the tables beside it,
//! the first text column the row's name taking the room, figures 12 px mono at the end, words 13 px muted; with more
//! than two text columns a row folds to its title over a note of the rest; one action every row takes makes the row
//! open, ending in a chevron. A checklist: a checkbox, the title muted once done, its note. Progress, after
//! `progress.tsx`: the count of rows done over a track, then a row per row not done.

use std::time::Instant;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Entity, Hsla, Pixels, SharedString, Window};
use serde_json::{json, Value};

use super::layout::{measured, place_of, tables_beside, tone_of, two_way, Place, INSET, ROW_FLOOR};
use super::look::{self, is_figure, is_sentence, Tone};
use super::PieceCx;
use crate::screens::slate::model::Row;
use crate::screens::slate::{truthy, Outcome, Raise, Slate};
use crate::ui::{motion, text, tip, ActivePalette, Button, Card, CardItem, Glyph};

/// Past this many rows a table says how many more there are rather than draw them all on every push.
pub const ROWS_DRAWN: usize = 200;
/// The columns a table draws, and the actions a row takes.
pub const COLUMNS_MAX: usize = 8;
pub const ACTIONS_MAX: usize = 3;
/// The widest a column stays on one line, in characters of the mono face; anything longer wraps.
const TIGHT_CH: usize = 26;
const NOTHING: &str = "Nothing here";
/// Past this many rows not done, progress says how many more there are.
const OPEN_SHOWN: usize = 8;
/// A progress track's fill easing to its new share, `duration-700`.
const PROGRESS_FILL: std::time::Duration = std::time::Duration::from_millis(700);

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "table" => table(p, window, cx),
        "checklist" => checklist(p, window, cx),
        "progress" => progress(p, window, cx),
        _ => None,
    }
}

// ---- the table's rules ----

/// How many of a table's rows draw: what its rows prop asks for, never past `ROWS_DRAWN`.
pub fn drawn_rows(asked: Option<f64>, length: usize) -> usize {
    asked.map_or(length, |n| n.max(0.).floor() as usize).min(ROWS_DRAWN).min(length)
}

/// The columns whose own `when` holds, eight at most, by their place in the written list.
pub fn shown_columns(slate: &Slate, raw: Option<&Value>) -> Vec<usize> {
    let Some(Value::Array(all)) = raw else { return Vec::new() };
    all.iter()
        .enumerate()
        .filter(|(_, c)| c.is_object())
        .filter(|(_, c)| c.get("when").and_then(Value::as_str).is_none_or(|w| truthy(slate.resolved.get(&format!("({w})")))))
        .map(|(at, _)| at)
        .take(COLUMNS_MAX)
        .collect()
}

/// Each column's tight width in characters, or None for a column that wraps: a mono column with no sentence in it,
/// or one whose every cell is a figure, as long as its widest cell fits on one line.
pub fn tight_widths(columns: &[Column], cells: &[Vec<Option<Value>>]) -> Vec<Option<usize>> {
    columns
        .iter()
        .enumerate()
        .map(|(at, column)| {
            let col: Vec<Option<&Value>> = cells.iter().map(|row| row.get(at).and_then(Option::as_ref)).collect();
            let mono = column.mono && !col.iter().any(|c| is_sentence(*c));
            if !mono && (col.is_empty() || !col.iter().all(|c| is_figure(*c))) {
                return None;
            }
            let widest = col
                .iter()
                .map(|c| look::str(*c).map_or(0, |s| s.chars().count()))
                .chain([column.title.as_ref().map_or(0, |t| t.chars().count())])
                .max()
                .unwrap_or(0);
            (widest <= TIGHT_CH).then_some(widest)
        })
        .collect()
}

/// The actions a row takes, three at most.
pub fn row_actions(raw: Option<&Value>) -> Vec<&Value> {
    raw.and_then(Value::as_array).map(|a| a.iter().filter(|x| x.is_object()).take(ACTIONS_MAX).collect()).unwrap_or_default()
}

/// A table folds its rows to a title over a note once more than two of its columns are text.
pub fn folds(widths: &[Option<usize>], rows: usize) -> bool {
    widths.iter().filter(|w| w.is_none()).count() > 2 && rows > 0
}

/// One column as the table draws it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Column {
    pub title: Option<String>,
    pub mono: bool,
    pub align: Option<String>,
}

/// One drawn row: its cells and their tones, and the actions it takes, None where an action's `when` fails.
pub struct TableRow {
    pub key: String,
    pub item: Value,
    pub index: usize,
    pub cells: Vec<Option<Value>>,
    pub tones: Vec<Option<Tone>>,
    pub actions: Vec<Option<String>>,
}

/// A table's whole content, from a piece or a run's result.
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<TableRow>,
    /// Every row the list holds, drawn or not.
    pub total: usize,
    pub empty: Option<String>,
    pub place: Place,
    /// The tight widths, shared with the tables beside it.
    pub widths: Vec<Option<usize>>,
    /// One action every row takes: the row opens.
    pub opens: bool,
    pub actions: usize,
    /// A section's head names the list, so a folded table's own header is left out.
    pub in_section: bool,
    /// What each action's steps do, said on its hover.
    pub hovers: Vec<Option<String>>,
    /// Each action's own steps, which its press runs in place of the piece's handler.
    pub steps: Vec<Option<Value>>,
}

/// `rowKeys`: each row's key off the key it was given, its index where it has none, the index added where two rows
/// give the same.
pub fn row_keys(given: &[Option<Value>]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    given
        .iter()
        .enumerate()
        .map(|(index, v)| {
            let base = match v {
                None | Some(Value::Null) => format!("#{index}"),
                Some(Value::String(s)) => s.clone(),
                Some(other) => other.to_string(),
            };
            let key = if seen.contains(&base) { format!("{base}#{index}") } else { base };
            seen.insert(key.clone());
            key
        })
        .collect()
}

/// A table piece's columns, drawn rows and actions, as the host answered them.
fn table_of(p: &PieceCx, all: &[Row]) -> (Vec<usize>, Vec<Column>, Vec<TableRow>) {
    let slate = p.slate;
    let raw = p.piece.prop("columns");
    let shown = shown_columns(slate, raw);
    let raw_cols: Vec<&Value> = raw.and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let first = all.first();
    let columns = shown
        .iter()
        .map(|&at| {
            let c = raw_cols[at];
            Column {
                title: column_title(first, c, at),
                mono: c.get("mono").and_then(Value::as_bool) == Some(true),
                align: c.get("align").and_then(Value::as_str).map(str::to_owned),
            }
        })
        .collect();
    let raw_actions = row_actions(p.piece.prop("rowActions"));
    let drawn = drawn_rows(p.num("rows"), all.len());
    let keys = row_keys(&all[..drawn].iter().map(|r| r.props.get("key").cloned()).collect::<Vec<_>>());
    let rows = all[..drawn]
        .iter()
        .zip(keys)
        .map(|(row, key)| {
            let col = |at: usize, field: &str| {
                row.props.get("columns").and_then(|c| c.get(at)).and_then(|c| c.get(field)).filter(|v| !v.is_null()).cloned()
            };
            let act = |at: usize, field: &str| row.props.get("rowActions").and_then(|c| c.get(at)).and_then(|c| c.get(field)).cloned();
            TableRow {
                key,
                item: row.item.clone(),
                index: row.index,
                cells: shown.iter().map(|&at| col(at, "value")).collect(),
                tones: shown.iter().map(|&at| col(at, "tone").map(|t| tone_of(Some(&t), slate, p.id()))).collect(),
                actions: raw_actions
                    .iter()
                    .enumerate()
                    .map(|(at, a)| {
                        let held = a.get("when").is_some() && !truthy(act(at, "when").as_ref());
                        (!held).then(|| look::str(act(at, "label").as_ref().or(a.get("label"))).unwrap_or_default())
                    })
                    .collect(),
            }
        })
        .collect();
    (shown, columns, rows)
}

/// A column's title: the host's answer on the first row where it is a formula, else the words written.
fn column_title(first: Option<&Row>, column: &Value, at: usize) -> Option<String> {
    let title = first
        .and_then(|r| r.props.get("columns")?.get(at)?.get("title").cloned())
        .or_else(|| column.get("title").filter(|t| t.is_string()).cloned());
    look::str(title.as_ref())
}

/// A table piece's own tight widths, for the tables beside it.
fn piece_widths(slate: &Slate, id: &str) -> Option<Vec<Option<usize>>> {
    let piece = slate.doc()?.piece(id)?;
    let all = slate.rows(piece)?;
    let shown = shown_columns(slate, piece.prop("columns"));
    let raw: Vec<&Value> = piece.prop("columns").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let asked = slate.props(piece).get("rows").and_then(Value::as_f64);
    let drawn = &all[..drawn_rows(asked, all.len())];
    let columns: Vec<Column> = shown
        .iter()
        .map(|&at| Column {
            title: column_title(all.first(), raw[at], at),
            mono: raw[at].get("mono").and_then(Value::as_bool) == Some(true),
            align: None,
        })
        .collect();
    let cells: Vec<Vec<Option<Value>>> = drawn
        .iter()
        .map(|r| shown.iter().map(|&at| r.props.get("columns").and_then(|c| c.get(at)).and_then(|c| c.get("value")).cloned()).collect())
        .collect();
    Some(tight_widths(&columns, &cells))
}

/// `sharedWidths`: each tight column the widest any table beside it with as many columns needs.
fn shared_widths(slate: &Slate, id: &str, own: Vec<Option<usize>>) -> Vec<Option<usize>> {
    let fellows: Vec<Vec<Option<usize>>> = tables_beside(slate, id)
        .iter()
        .filter(|o| *o != id)
        .filter_map(|o| piece_widths(slate, o))
        .filter(|w| w.len() == own.len())
        .collect();
    own.iter().enumerate().map(|(at, w)| w.map(|w| fellows.iter().filter_map(|f| f[at]).fold(w, usize::max))).collect()
}

fn table(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let all = p.rows.clone()?;
    let (_, columns, rows) = table_of(p, &all);
    let cells: Vec<Vec<Option<Value>>> = rows.iter().map(|r| r.cells.clone()).collect();
    let widths = shared_widths(p.slate, p.id(), tight_widths(&columns, &cells));
    let raw_actions = row_actions(p.piece.prop("rowActions"));
    let in_section = p.slate.doc().and_then(|d| d.parent(p.id())).is_some_and(|s| s.kind == "section");
    let spec = Table {
        columns,
        rows,
        total: all.len(),
        empty: p.str("empty"),
        place: place_of(p.slate, p.id()),
        widths,
        opens: raw_actions.len() == 1 && raw_actions[0].get("when").is_none(),
        actions: raw_actions.len(),
        in_section,
        hovers: raw_actions
            .iter()
            .map(|a| {
                let steps = match a.get("on").and_then(|o| o.get("press")) {
                    Some(Value::Array(list)) => list.iter().collect(),
                    Some(one) => vec![one],
                    None => Vec::new(),
                };
                super::controls::press_title(&steps, p.slate)
            })
            .collect(),
        steps: raw_actions.iter().map(|a| a.get("on").and_then(|o| o.get("press")).cloned()).collect(),
    };
    Some(table_view(p, "table", spec, window, cx))
}

/// The ch of the table's 12 px mono face.
fn mono_ch(window: &Window, cx: &App) -> Pixels {
    text::width("0", 12., cx.theme().mono_font_family.clone(), window)
}

/// A table drawn from its content; `part` keeps two tables of one piece apart, as a run's result and its rows.
pub fn table_view(p: &PieceCx, part: &str, t: Table, window: &mut Window, cx: &mut App) -> AnyElement {
    let palette = cx.palette().clone();
    let shown = t.rows.len();
    let text_cols: Vec<usize> = t.widths.iter().enumerate().filter(|(_, w)| w.is_none()).map(|(at, _)| at).collect();
    let figures: Vec<bool> = (0..t.columns.len()).map(|at| shown > 0 && t.rows.iter().all(|r| is_figure(r.cells[at].as_ref()))).collect();
    let sentences = text_cols.iter().copied().find(|&at| t.rows.iter().filter(|r| is_sentence(r.cells[at].as_ref())).count() * 2 > shown);
    // The row's name: a mono first column of names (a path, an id) in its own face, so no column after it draws larger;
    // else the column most of whose cells are sentences, else the first text column. A cell led by a digit is a figure,
    // a date or a time, never a name.
    let mono_first = t.columns.first().is_some_and(|c| c.mono)
        && figures.first() == Some(&false)
        && !t.rows.iter().any(|r| {
            is_sentence(r.cells[0].as_ref()) || look::str(r.cells[0].as_ref()).is_some_and(|s| s.starts_with(|c: char| c.is_ascii_digit()))
        });
    let title = if mono_first { 0 } else { sentences.or(text_cols.first().copied()).unwrap_or(0) };
    let title_mono = t.columns.get(title).is_some_and(|c| c.mono) && !t.rows.iter().any(|r| is_sentence(r.cells[title].as_ref()));
    let end = |at: usize| at != title && (t.columns[at].align.as_deref() == Some("end") || (figures[at] && t.columns[at].align.is_none()));
    let edges = t.place != Place::Inside;
    let rule = palette.border.opacity(0.5);
    let mono_font = cx.theme().mono_font_family.clone();
    let sans = cx.theme().font_family.clone();
    // The actions' column: the chevron's 14 px, or the widest row's xs buttons (8 px each side and a 1 px edge).
    let act_w: Option<Pixels> = if t.opens {
        Some(px(14.))
    } else if t.actions > 0 {
        let row_w = |r: &TableRow| {
            let shown: Vec<Pixels> = r.actions.iter().flatten().map(|l| text::width(l, 12., sans.clone(), window) + px(18.)).collect();
            shown.iter().fold(px(0.), |a, w| a + *w) + px(4.) * shown.len().saturating_sub(1) as f32
        };
        Some(t.rows.iter().map(row_w).fold(px(0.), Pixels::max).ceil())
    } else {
        None
    };

    let mut below = Vec::new();
    if t.total == 0 {
        let line = h_flex()
            .min_h(ROW_FLOOR)
            .py(px(12.))
            .when(edges, |d| d.px(INSET))
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(palette.muted_foreground)
            .child(t.empty.clone().unwrap_or_else(|| NOTHING.into()));
        below.push(if edges { super::layout::surface(p.el(&format!("{part}-empty")), line) } else { line.into_any_element() });
    }
    if t.total > shown {
        below.push(super::quiet(format!("and {} more", t.total - shown), cx));
    }

    let pressed = window.use_keyed_state(p.el(&format!("{part}-pressed")), cx, |_, _| None::<(usize, usize)>);
    let last = window.use_keyed_state(p.el(&format!("{part}-last")), cx, |_, _| None::<(usize, usize, Instant)>);
    let busy = p.slate.busy.contains(p.id());
    let outcome = p.slate.outcomes.get(p.id()).cloned();

    let action_cell = |row: &TableRow, cx: &mut App| -> Option<AnyElement> {
        if t.opens {
            return Some(
                h_flex()
                    .self_center()
                    .flex_none()
                    .child(Icon::new(Glyph::ChevronRight).with_size(px(14.)).text_color(palette.muted_foreground))
                    .into_any_element(),
            );
        }
        if t.actions == 0 {
            return None;
        }
        let mut acts = Vec::new();
        for (at, label) in row.actions.iter().enumerate() {
            let Some(label) = label else { continue };
            let raise = Raise { row: Some((row.item.clone(), row.index)), row_action: Some(at), steps: t.steps.get(at).cloned().flatten() };
            let (screen, id, pressed, last) = (p.screen.clone(), p.id().to_owned(), pressed.clone(), last.clone());
            let index = row.index;
            let said = (*pressed.read(cx) == Some((index, at))).then(|| outcome.clone()).flatten();
            acts.push(
                v_flex()
                    .items_end()
                    .gap(px(4.))
                    .child(
                        Button::outline(p.el(&format!("{part}-act-{}-{at}", row.key)), label.clone())
                            .xs()
                            .held(busy)
                            .when_some(t.hovers.get(at).cloned().flatten(), |b, h| b.tooltip(h))
                            .on_click(move |_, _, cx| {
                                if !first_press(&last, index, at, cx) {
                                    return;
                                }
                                pressed.update(cx, |s, _| *s = Some((index, at)));
                                let (id, raise) = (id.clone(), raise.clone());
                                _ = screen.update(cx, |s, cx| s.raise(id, "press", raise, cx));
                            }),
                    )
                    .children(said.map(|o| outcome_line(&o, cx)))
                    .into_any_element(),
            );
        }
        Some(h_flex().self_center().flex_none().when_some(act_w, |d, w| d.w(w)).justify_end().gap(px(4.)).children(acts).into_any_element())
    };

    // Each row's frame: a row that opens takes the accent at 60% under the pointer over 150 ms and presses as a whole.
    let frame = |row: &TableRow, body: gpui_kit::Div, window: &mut Window, cx: &mut App| -> gpui_kit::Stateful<gpui_kit::Div> {
        let el_id = p.el(&format!("{part}-row-{}", row.key));
        let base = body.id(el_id.clone()).min_h(ROW_FLOOR).py(px(12.)).min_w_0();
        if !t.opens {
            return base;
        }
        let over = motion::hovered(el_id.clone(), window, cx);
        let accent = palette.accent;
        let ground = motion::ease_ink(
            el_id,
            "ground",
            if *over.read(cx) { accent.opacity(0.6) } else { accent.opacity(0.) },
            motion::COLORS,
            window,
            cx,
        );
        let raise = Raise { row: Some((row.item.clone(), row.index)), row_action: Some(0), steps: t.steps.first().cloned().flatten() };
        let (screen, id, pressed, last, index) = (p.screen.clone(), p.id().to_owned(), pressed.clone(), last.clone(), row.index);
        let hover = t.hovers.first().cloned().flatten();
        base.cursor_pointer().bg(ground).on_hover(motion::track_hover(over)).when_some(hover, |d, h| d.tooltip(tip(h))).on_click(
            move |_, _, cx| {
                if busy || !first_press(&last, index, 0, cx) {
                    return;
                }
                pressed.update(cx, |s, _| *s = Some((index, 0)));
                let (id, raise) = (id.clone(), raise.clone());
                _ = screen.update(cx, |s, cx| s.raise(id, "press", raise, cx));
            },
        )
    };
    let open_said = |row: &TableRow, cx: &App| -> Option<AnyElement> {
        (t.opens && *pressed.read(cx) == Some((row.index, 0)))
            .then(|| outcome.clone())
            .flatten()
            .map(|o| div().pb(px(8.)).px(INSET).child(outcome_line(&o, cx)).into_any_element())
    };

    let (seen_width, probe) = measured(p.el(&format!("{part}-width")), window, cx);
    let table_width = seen_width.unwrap_or(px(508.));
    let ch = mono_ch(window, cx);
    let longest = |at: usize| {
        t.rows
            .iter()
            .map(|r| look::str(r.cells[at].as_ref()).map_or(0, |s| s.chars().count()))
            .chain([t.columns[at].title.as_ref().map_or(0, |s| s.chars().count())])
            .max()
            .unwrap_or(0)
    };
    let fill = (0..t.columns.len()).filter(|&at| !figures[at]).max_by_key(|&at| (longest(at), std::cmp::Reverse(at))).unwrap_or(title);
    let fill_word = t
        .rows
        .iter()
        .filter_map(|r| look::str(r.cells[fill].as_ref()))
        .flat_map(|s| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>())
        .map(|w| text::width(&w, if fill == title && !title_mono { 14. } else { 13. }, sans.clone(), window))
        .fold(px(0.), Pixels::max)
        .ceil();
    // The tight columns at their widths and the filling column's longest word, 16 px apart, inside the insets: where the
    // table is narrower than that it folds, rather than break a word or spill out of its card.
    let needed =
        (0..t.columns.len()).filter_map(|at| t.widths[at].filter(|_| at != fill).map(|n| ch * n as f32)).fold(px(0.), |a, w| a + w)
            + fill_word
            + px(16.) * t.columns.len().saturating_sub(1) as f32
            + act_w.map_or(px(0.), |w| w + px(16.))
            + if edges { INSET * 2. } else { px(0.) };
    let cramped = seen_width.is_some_and(|w| needed > w);
    if folds(&t.widths, shown) || (cramped && shown > 0) {
        let note_order: Vec<usize> = text_cols.iter().copied().chain((0..t.columns.len()).filter(|at| !text_cols.contains(at))).collect();
        let mut items: Vec<(AnyElement, Option<AnyElement>)> = Vec::new();
        for row in &t.rows {
            let name = look::str(row.cells[title].as_ref()).unwrap_or_default();
            let notes = note_order.iter().filter(|&&at| at != title).filter_map(|&at| {
                let v = row.cells[at].as_ref();
                let s = look::str(v).filter(|s| !s.is_empty())?;
                let mono = (t.columns[at].mono && !is_sentence(v)) || is_figure(v);
                let ink = row.tones[at].map_or(palette.muted_foreground, |tone| tone.ink(cx));
                Some(
                    div()
                        .min_w_0()
                        .text_size(px(12.))
                        .line_height(px(16.))
                        .text_color(ink)
                        .when(mono, |d| d.font_family(mono_font.clone()))
                        .child(s),
                )
            });
            let words = v_flex()
                .min_w_0()
                .flex_1()
                .gap(px(4.))
                .child(
                    div()
                        .min_w_0()
                        .map(|d| if title_mono { d.font_family(mono_font.clone()).text_size(px(12.)) } else { d.text_size(px(14.)) })
                        .line_height(px(20.))
                        .text_color(row.tones[title].map_or(palette.foreground, |tone| tone.ink(cx)))
                        .child(name),
                )
                .child(h_flex().min_w_0().flex_wrap().items_baseline().gap_x(px(12.)).gap_y(px(4.)).children(notes));
            let body = h_flex().items_start().gap(px(16.)).when(edges, |d| d.px(INSET)).child(words).children(action_cell(row, cx));
            items.push((frame(row, body, window, cx).into_any_element(), open_said(row, cx)));
        }
        // The header names the list inside its card, its first row, as a grid's does.
        if let Some(h) = t.columns[title].title.clone().filter(|_| !t.in_section) {
            let row = head_row(cx)
                .debug_selector(move || format!("slate-table-name-{title}"))
                .when(edges, |d| d.px(INSET))
                .child(h)
                .into_any_element();
            items.insert(0, (row, None));
        }
        return v_flex()
            .debug_selector(|| "slate-table-folded".into())
            .relative()
            .min_w_0()
            .gap(px(10.))
            .child(rows_card(p.el(&format!("{part}-card")), items, edges, rule))
            .child(probe)
            .children(below)
            .into_any_element();
    }

    // A grid: every row on one set of widths, the header's cells on the rows' own.
    let tight = |at: usize| t.widths[at].is_some() && at != fill;
    // A column's width: the fill takes the room left, a tight one its widest cell, any other its words up to 40%.
    let widths: Vec<Option<Pixels>> = (0..t.columns.len())
        .map(|at| {
            if shown == 0 || at == fill {
                return None;
            }
            let head_w =
                t.columns[at].title.as_deref().map_or(px(0.), |h| text::width(h, if at == 0 { 14. } else { 13. }, sans.clone(), window));
            if let Some(n) = t.widths[at] {
                return Some((ch * n as f32).max(head_w).ceil());
            }
            let widest = t
                .rows
                .iter()
                .filter_map(|r| look::str(r.cells[at].as_ref()))
                .map(|s| {
                    let mono = t.columns[at].mono && !is_sentence(Some(&Value::String(s.clone())));
                    if mono {
                        text::width(&s, 12., mono_font.clone(), window)
                    } else {
                        text::width(&s, if at == title { 14. } else { 13. }, sans.clone(), window)
                    }
                })
                .fold(head_w, Pixels::max);
            Some(widest.min(table_width * 0.4).ceil())
        })
        .collect();
    // The filling column never narrows past its longest word, so a word wraps whole rather than break mid-word.
    let fill_word = fill_word.min(table_width * 0.5);
    let cell_box = |at: usize| {
        let d = div().min_w_0();
        let d = match widths[at] {
            Some(w) => d.w(w).flex_none(),
            None => d.flex_1().flex_basis(relative(0.)).min_w(fill_word),
        };
        let d = if end(at) { d.text_right() } else { d };
        if tight(at) {
            d.whitespace_nowrap()
        } else {
            d
        }
    };
    // The header names the columns inside the card, its first row, so it reads as the card's own and never floats over it.
    let header = (!t.columns.iter().all(|c| c.title.as_deref().unwrap_or_default().is_empty())).then(|| {
        head_row(cx)
            .debug_selector(|| "slate-table-head".into())
            .gap(px(16.))
            .when(edges, |d| d.px(INSET))
            .children((0..t.columns.len()).map(|at| {
                let words = t.columns[at].title.clone().unwrap_or_default();
                let named = format!("slate-{}-head-{at}", p.id());
                cell_box(at).debug_selector(move || named.clone()).child(words)
            }))
            .when_some(act_w, |d, w| d.child(div().flex_none().w(w)))
    });
    let mut items: Vec<(AnyElement, Option<AnyElement>)> = Vec::new();
    // With no rows the header stands alone over nothing; with rows it is the card's first row.
    let header = match header {
        Some(h) if shown > 0 => {
            items.push((h.into_any_element(), None));
            None
        }
        other => other,
    };
    for row in &t.rows {
        let cells = (0..t.columns.len()).map(|at| {
            let v = row.cells[at].as_ref();
            let mono = (t.columns[at].mono && !is_sentence(v)) || is_figure(v);
            let ink: Hsla =
                row.tones[at].map_or(if mono || at == title { palette.foreground } else { palette.muted_foreground }, |tone| tone.ink(cx));
            let cell = cell_box(at).text_color(ink);
            let cell = if mono {
                cell.font_family(mono_font.clone()).text_size(px(12.)).line_height(px(20.))
            } else if at == title {
                cell.text_size(px(14.)).line_height(px(20.))
            } else {
                cell.text_size(px(13.)).line_height(px(20.))
            };
            cell.children(look::str(v))
        });
        let body = h_flex().items_start().gap(px(16.)).when(edges, |d| d.px(INSET)).children(cells).children(action_cell(row, cx));
        items.push((frame(row, body, window, cx).into_any_element(), open_said(row, cx)));
    }
    v_flex()
        .debug_selector(|| "slate-table".into())
        .relative()
        .min_w_0()
        .children(header)
        .when(shown > 0, |d| d.child(rows_card(p.el(&format!("{part}-card")), items, edges, rule)))
        .when(!below.is_empty(), |d| d.child(v_flex().children(below)))
        .child(probe)
        .into_any_element()
}

/// A table's header row: 12 px muted, 36 px at the least, as a row of its card.
fn head_row(cx: &App) -> gpui_kit::Div {
    h_flex().min_h(px(36.)).py(px(8.)).items_center().text_size(px(12.)).line_height(px(16.)).text_color(cx.palette().muted_foreground)
}

/// The rows of a table in their card, or bare with a hairline between them inside another card's row.
fn rows_card(key: SharedString, items: Vec<(AnyElement, Option<AnyElement>)>, edges: bool, rule: Hsla) -> AnyElement {
    if !edges {
        return v_flex()
            .min_w_0()
            .children(
                items.into_iter().enumerate().map(|(at, (row, said))| {
                    v_flex().min_w_0().when(at > 0, |d| d.border_t_1().border_color(rule)).child(row).children(said)
                }),
            )
            .into_any_element();
    }
    let items = items
        .into_iter()
        .map(|(row, said)| CardItem::placed(move |c| c.apply(v_flex().min_w_0()).child(row).children(said).into_any_element()));
    Card::new(key).locked(true).inset(INSET).items(items).into_any_element()
}

/// `Outcome`: the one line under a control after a press, the refusal in the refusal ink.
fn outcome_line(o: &Outcome, cx: &App) -> AnyElement {
    let (words, ink) = match o {
        Outcome::Said(s) => (s.clone(), cx.palette().muted_foreground),
        Outcome::Refused(r) => (r.clone(), cx.palette().error_foreground),
    };
    div().text_size(px(12.)).line_height(px(16.)).text_color(ink).child(words).into_any_element()
}

/// Notes a press on a row's action, false for a second one on it within 500 ms, which folds into the first (`usePress`).
fn first_press(last: &Entity<Option<(usize, usize, Instant)>>, index: usize, at: usize, cx: &mut App) -> bool {
    let now = Instant::now();
    if last.read(cx).is_some_and(|(i, a, when)| (i, a) == (index, at) && now.duration_since(when) < super::controls::COALESCE) {
        return false;
    }
    last.update(cx, |l, _| *l = Some((index, at, now)));
    true
}

// ---- the checklist ----

/// The field a row's done reads, where it binds `item.<field>`.
pub fn row_field(v: Option<&Value>) -> Option<String> {
    let bind = v?.as_object().filter(|o| o.len() == 1)?.get("bind")?.as_str()?.trim();
    let field = bind.strip_prefix("item.")?;
    let ok = field.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && field.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    ok.then(|| field.to_owned())
}

/// The shared checkbox 2 px down to its title's first line, a tab stop wearing the ring while it can be ticked.
fn checkbox(id: SharedString, checked: bool, editable: bool, window: &mut Window, cx: &mut App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let ring_ink = cx.palette().ring;
    crate::ui::controls::checkbox(id.clone(), checked, !editable, window, cx)
        .id(id)
        .mt(px(2.))
        .when(editable, |el| el.cursor_pointer().tab_index(0).focus_visible(move |s| s.shadow(vec![crate::ui::focus::ring(ring_ink)])))
}

fn checklist(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let rows = p.rows.clone()?;
    let palette = cx.palette().clone();
    let list = two_way(p.piece.prop("items"));
    let field = row_field(p.piece.prop("done"));
    let editable = p.bool("editable") && list.is_some() && field.is_some();
    let place = place_of(p.slate, p.id());
    let inset = place != Place::Inside;
    if rows.is_empty() {
        let line = h_flex()
            .min_h(ROW_FLOOR)
            .py(px(12.))
            .when(inset, |d| d.px(INSET))
            .text_size(px(13.))
            .line_height(px(20.))
            .text_color(palette.muted_foreground)
            .child(p.str("empty").unwrap_or_else(|| NOTHING.into()));
        return Some(if place == Place::Page { super::layout::surface(p.el("empty"), line) } else { line.into_any_element() });
    }
    let keys = row_keys(&rows.iter().map(|r| r.props.get("key").cloned()).collect::<Vec<_>>());
    let rule = palette.border.opacity(0.5);
    let has_change = p.piece.on.contains_key("change");
    let items = rows.iter().zip(keys).enumerate().map(|(at, (row, key))| {
        let done = truthy(row.props.get("done"));
        let note = look::str(row.props.get("note"));
        // A row that is a plain word is its own title; a title written once would name every row alike.
        let title = match &row.item {
            Value::String(_) | Value::Number(_) => look::str(Some(&row.item)),
            _ => look::str(row.props.get("title")),
        }
        .unwrap_or_default();
        let mut tick = checkbox(p.el(&format!("check-{key}")), done, editable, window, cx);
        if let (true, Some(list), Some(field)) = (editable, list.clone(), field.clone()) {
            let (screen, id, item, index) = (p.screen.clone(), p.id().to_owned(), row.item.clone(), row.index);
            tick = tick.on_click(move |_, _, cx| {
                let path = format!("{list}[{index}].{field}");
                let (id, item) = (id.clone(), item.clone());
                _ = screen.update(cx, |s, cx| {
                    // A second press before the host answers the first flips what this window drew, not the stale row.
                    let now = s.slate().and_then(|slate| slate.value(&path)).map_or(done, |v| truthy(Some(v)));
                    s.write_then(
                        path,
                        json!(!now),
                        move |s, cx| {
                            if has_change {
                                s.raise(id, "change", Raise { row: Some((item, index)), ..Raise::default() }, cx);
                            }
                        },
                        cx,
                    );
                });
            });
        }
        h_flex()
            .min_h(ROW_FLOOR)
            .min_w_0()
            .items_start()
            .gap(px(12.))
            .py(px(12.))
            .when(inset, |d| d.px(INSET))
            .when(at > 0, |d| d.border_t_1().border_color(rule))
            .child(tick)
            .child(
                v_flex()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(px(14.))
                            .line_height(px(20.))
                            .text_color(if done { palette.muted_foreground } else { palette.foreground })
                            .child(title),
                    )
                    .children(note.map(|n| div().text_size(px(13.)).line_height(px(20.)).text_color(palette.muted_foreground).child(n))),
            )
    });
    let body = v_flex().min_w_0().debug_selector(|| "slate-checklist".into()).children(items);
    Some(if place == Place::Page { super::layout::surface(p.el("list"), body) } else { body.into_any_element() })
}

/// What progress says over its track: none yet, all of them, or how many of how many are done.
pub fn progress_count(done: usize, total: usize) -> String {
    match () {
        _ if total == 0 => "None yet".into(),
        _ if done == total => format!("All {total}"),
        _ => format!("{done} of {total}"),
    }
}

/// A count of rows done over a track that fills to its share, green once every row is done, then a row per row not
/// done, its empty circle, its name and its note. A card of its own on the page, never a card inside another's row.
fn progress(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let rows = p.rows.clone()?;
    let palette = cx.palette().clone();
    let place = place_of(p.slate, p.id());
    let inset = place != Place::Inside;
    let open: Vec<&Row> = rows.iter().filter(|r| !truthy(r.props.get("done"))).collect();
    let done = rows.len() - open.len();
    let share = if rows.is_empty() { 0. } else { done as f32 / rows.len() as f32 };
    let all = !rows.is_empty() && open.is_empty();
    // The share eases as a length would, read back as the fraction of the track it fills.
    let eased = f32::from(motion::ease_px_by(p.el("fill"), "share", px(share), motion::snappy(PROGRESS_FILL), window, cx));
    let fill = if all { palette.success } else { palette.foreground.opacity(0.55) };
    let head = v_flex()
        .min_w_0()
        .gap(px(10.))
        .py(px(14.))
        .when(inset, |d| d.px(INSET))
        .child(
            h_flex()
                .min_w_0()
                .flex_wrap()
                .items_baseline()
                .gap_x(px(6.))
                .child(
                    div()
                        .debug_selector(|| "slate-progress-count".into())
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(14.))
                        .line_height(px(24.))
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(palette.foreground)
                        .child(progress_count(done, rows.len())),
                )
                .children(p.str("label").filter(|_| !rows.is_empty()).map(|l| {
                    div()
                        .debug_selector(|| "slate-progress-label".into())
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(palette.muted_foreground)
                        .child(l)
                })),
        )
        .child(div().h(px(4.)).w_full().rounded_full().overflow_hidden().bg(palette.foreground.opacity(0.1)).child(
            div().debug_selector(|| "slate-progress-fill".into()).h_full().w(relative(eased.clamp(0., 1.))).rounded_full().bg(fill),
        ));
    let keys = row_keys(&open.iter().map(|r| r.props.get("key").cloned()).collect::<Vec<_>>());
    let exceptions = open.iter().zip(keys).take(OPEN_SHOWN).map(|(row, key)| {
        let name = look::str(row.props.get("name")).unwrap_or_default();
        let note = look::str(row.props.get("note")).filter(|n| !n.is_empty());
        h_flex()
            .debug_selector({
                let named = p.el(&format!("open-{key}")).to_string();
                move || named
            })
            .min_h(ROW_FLOOR)
            .min_w_0()
            .items_start()
            .gap(px(12.))
            .py(px(12.))
            .when(inset, |d| d.px(INSET))
            .child(Icon::new(Glyph::Circle).with_size(px(16.)).flex_none().mt(px(2.)).text_color(palette.muted_foreground.opacity(0.7)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(div().text_size(px(14.)).line_height(px(20.)).text_color(palette.foreground).child(name))
                    .children(note.map(|n| div().text_size(px(13.)).line_height(px(20.)).text_color(palette.muted_foreground).child(n))),
            )
            .into_any_element()
    });
    let more = (open.len() > OPEN_SHOWN).then(|| {
        div()
            .debug_selector(|| "slate-progress-more".into())
            .py(px(10.))
            .when(inset, |d| d.px(INSET))
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(palette.muted_foreground)
            .child(format!("and {} more not done", open.len() - OPEN_SHOWN))
            .into_any_element()
    });
    let rule = palette.border.opacity(0.5);
    let parts: Vec<AnyElement> = std::iter::once(head.into_any_element()).chain(exceptions).chain(more).collect();
    let body = v_flex().min_w_0().debug_selector(move || format!("slate-progress-{done}/{}", rows.len())).children(
        parts.into_iter().enumerate().map(|(at, el)| div().min_w_0().when(at > 0, |d| d.border_t_1().border_color(rule)).child(el)),
    );
    Some(if place == Place::Page { super::layout::surface(p.el("progress"), body) } else { body.into_any_element() })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::screens::slate::pieces::layout::tests::slate;

    fn col(mono: bool) -> Column {
        Column { title: Some("T".into()), mono, align: None }
    }

    #[test]
    fn a_table_draws_two_hundred_rows_at_most() {
        assert_eq!(drawn_rows(None, 250), 200);
        assert_eq!(drawn_rows(Some(5.), 250), 5);
        assert_eq!(drawn_rows(Some(500.), 3), 3);
        assert_eq!(drawn_rows(Some(-1.), 3), 0);
    }

    #[test]
    fn a_table_draws_eight_columns_whose_when_holds() {
        let columns: Vec<Value> = (0..10).map(|n| json!({"title": format!("c{n}"), "value": n})).collect();
        let mut with_when = columns.clone();
        with_when[1] = json!({"title": "hidden", "value": 1, "when": "$off"});
        let s = slate(json!({"root": {"type": "column"}}));
        assert_eq!(shown_columns(&s, Some(&Value::Array(columns))), (0..8).collect::<Vec<_>>());
        assert_eq!(shown_columns(&s, Some(&Value::Array(with_when))), vec![0, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn three_text_columns_fold_and_figures_stay_tight() {
        let columns = vec![col(false), col(false), col(false), col(false)];
        let cells =
            vec![vec![Some(json!("The cart rounds")), Some(json!("packages/cart")), Some(json!("Rounding once")), Some(json!("12s"))]];
        let widths = tight_widths(&columns, &cells);
        assert_eq!(widths, vec![None, None, None, Some(3)]);
        assert!(folds(&widths, 1));
        assert!(!folds(&widths, 0), "a table with no rows stands as a grid");
        assert!(!folds(&widths[1..], 1));
        let mono = tight_widths(&[col(true)], &[vec![Some(json!("x".repeat(27)))]]);
        assert_eq!(mono, vec![None], "a mono column past 26 characters wraps");
    }

    #[test]
    fn row_keys_never_repeat() {
        assert_eq!(row_keys(&[Some(json!("a")), Some(json!("a")), None]), vec!["a", "a#1", "#2"]);
        assert_eq!(row_field(Some(&json!({"bind": "item.done"}))).as_deref(), Some("done"));
        assert_eq!(row_field(Some(&json!({"bind": "item.a.b"}))), None);
        let four = json!([{"label": "a"}, {"label": "b"}, {"label": "c"}, {"label": "d"}]);
        assert_eq!(row_actions(Some(&four)).len(), 3, "a row takes three actions at most");
    }
}
