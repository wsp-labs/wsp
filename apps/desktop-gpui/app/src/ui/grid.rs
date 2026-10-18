// SPDX-License-Identifier: AGPL-3.0-only
//! The settings list grammar, after `grid.tsx`: a header row whose first cell names the section, then rows on the
//! same columns in one soft card, so the columns line up down the page. A name with its glyph and note, numbers in
//! right-aligned tabular figures, a state as a word or the act itself.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClickEvent, FontWeight, MouseButton, Pixels, SharedString, Window};

use super::card::{opens, row_frame, surface, CardItem, Corners};
use super::dialog::tip;
use super::icons::Glyph;
use super::motion;
use super::theme::ActivePalette;
use super::{size, text};

/// A grid's column gap, `gap-x-4`.
const COLUMN_GAP: Pixels = px(16.);

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// One column of a template: `minmax(0,1fr)` or a fixed width.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Col {
    Fill,
    Px(f32),
}

use Col::{Fill, Px};

/// `LIST_COLUMNS`: the Computers page's lists, so the state column is one line across them.
pub const LIST_COLUMNS: &[Col] = &[Fill, Px(56.), Px(72.), Px(64.), Px(72.), Px(14.)];
/// `PAGE_COLUMNS`: a computer's or a cloud's page, the name, a version and the act at the right.
pub const PAGE_COLUMNS: &[Col] = &[Fill, Px(64.), Px(280.)];
/// `ACTIVITY_COLUMNS`: the name, memory, CPU, energy, and the fold or the row's menu.
pub const ACTIVITY_COLUMNS: &[Col] = &[Fill, Px(76.), Px(60.), Px(64.), Px(20.)];
/// `ACTIVITY_FLAT_COLUMNS`: the name, the pid, memory, CPU, energy and the row's menu.
pub const ACTIVITY_FLAT_COLUMNS: &[Col] = &[Fill, Px(56.), Px(76.), Px(60.), Px(64.), Px(20.)];
/// `ACTIVITY_PORT_COLUMNS`: the port, the process, its thread, who can connect, Open, and the row's menu.
pub const ACTIVITY_PORT_COLUMNS: &[Col] = &[Px(72.), Fill, Fill, Px(120.), Px(64.), Px(20.)];

/// The cell boxes of a template, `span` columns per cell; a cell that spans a fixed column and the next takes both
/// widths and the gap between them.
fn cells(columns: &[Col], mut items: Vec<(AnyElement, usize)>) -> Vec<AnyElement> {
    // A row with fewer cells than columns still stands on the whole template, so its columns line up.
    let taken: usize = items.iter().map(|(_, span)| span).sum();
    items.extend((taken..columns.len()).map(|_| (div().into_any_element(), 1)));
    let mut at = 0;
    items
        .into_iter()
        .map(|(el, span)| {
            let cols = &columns[at.min(columns.len())..(at + span).min(columns.len())];
            at += span;
            let cell = h_flex().min_w_0();
            let cell = if cols.contains(&Fill) || cols.is_empty() {
                cell.flex_1()
            } else {
                let w: f32 = cols.iter().map(|c| if let Px(w) = c { *w } else { 0. }).sum();
                cell.flex_none().w(px(w) + COLUMN_GAP * (cols.len() - 1) as f32)
            };
            cell.child(el).into_any_element()
        })
        .collect()
}

/// `Grid`: a list, its header row over the card of rows.
#[derive(IntoElement)]
pub struct Grid {
    id: SharedString,
    head: Option<GridHead>,
    rows: Vec<CardItem>,
}

impl Grid {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Grid { id: id.into(), head: None, rows: Vec::new() }
    }

    pub fn head(mut self, head: GridHead) -> Self {
        self.head = Some(head);
        self
    }

    pub fn row(mut self, row: impl Into<CardItem>) -> Self {
        self.rows.push(row.into());
        self
    }

    pub fn rows(mut self, rows: impl IntoIterator<Item = impl Into<CardItem>>) -> Self {
        self.rows.extend(rows.into_iter().map(Into::into));
        self
    }
}

impl RenderOnce for Grid {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        v_flex().id(self.id).debug_selector(|| "grid".into()).children(self.head).child(surface(
            self.rows,
            size::CARD_INSET,
            size::CARD_RADIUS,
            cx,
        ))
    }
}

/// One word of a header row.
pub struct HeadCell {
    word: SharedString,
    num: bool,
    span: usize,
}

impl HeadCell {
    pub fn new(word: impl Into<SharedString>) -> Self {
        HeadCell { word: word.into(), num: false, span: 1 }
    }

    /// Over a column of numbers: set to the right.
    pub fn num(mut self) -> Self {
        self.num = true;
        self
    }

    /// Over two columns, for a list whose one figure takes the room of another list's two.
    pub fn span2(mut self) -> Self {
        self.span = 2;
        self
    }
}

/// `GridHead`: the header row on the list's columns, or on its own. The first word names the section on the card's
/// outer edge like every settings head; the column words sit over their values.
#[derive(IntoElement)]
pub struct GridHead {
    columns: Option<&'static [Col]>,
    cells: Vec<HeadCell>,
}

impl GridHead {
    /// `columns` is the rows' template, or `None` for words side by side.
    pub fn new(columns: Option<&'static [Col]>, cells: Vec<HeadCell>) -> Self {
        GridHead { columns, cells }
    }
}

impl RenderOnce for GridHead {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let words: Vec<(AnyElement, usize)> = self
            .cells
            .into_iter()
            .enumerate()
            .map(|(ix, cell)| {
                let el = div()
                    .whitespace_nowrap()
                    .text_size(text::SETTING_TITLE.0)
                    .line_height(text::SETTING_TITLE.1)
                    .font_weight(FontWeight::NORMAL)
                    .when(ix == 0, |el| el.ml(-(size::CARD_INSET + px(1.))).text_color(p.foreground.opacity(0.7)))
                    .when(ix > 0, |el| el.text_color(p.muted_foreground))
                    .when(cell.num, |el| el.w_full().text_right())
                    .child(cell.word);
                (el.into_any_element(), cell.span)
            })
            .collect();
        let row = h_flex()
            .debug_selector(|| "grid-head".into())
            .mb_4()
            .min_h(size::SECTION_HEAD_FLOOR)
            .items_end()
            .gap_x_4()
            .border_x_1()
            .border_color(gpui_kit::transparent_black())
            .px(size::CARD_INSET);
        match self.columns {
            Some(columns) => row.children(cells(columns, words)),
            None => row.children(words.into_iter().map(|(el, _)| el)),
        }
    }
}

/// `GridRow`: one row on the template, under the pointer in the accent at 60%. A row that opens a page is the press
/// itself, and a control inside it keeps its own press.
#[derive(IntoElement)]
pub struct GridRow {
    id: SharedString,
    columns: &'static [Col],
    tight: bool,
    open: Option<Click>,
    hover: Option<SharedString>,
    cells: Vec<(AnyElement, usize)>,
    line: Option<Vec<(AnyElement, usize)>>,
    corners: Corners,
}

impl GridRow {
    pub fn new(id: impl Into<SharedString>, columns: &'static [Col]) -> Self {
        GridRow {
            id: id.into(),
            columns,
            tight: false,
            open: None,
            hover: None,
            cells: Vec::new(),
            line: None,
            corners: Corners::default(),
        }
    }

    /// At a line's 48 px floor rather than a row's 60.
    pub fn tight(mut self) -> Self {
        self.tight = true;
        self
    }

    pub fn on_open(mut self, open: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.open = Some(Box::new(open));
        self
    }

    /// The row's whole sentence on its hover.
    pub fn hover(mut self, hover: impl Into<SharedString>) -> Self {
        self.hover = Some(hover.into());
        self
    }

    /// The next cell, in the next column.
    pub fn cell(mut self, el: impl IntoElement) -> Self {
        self.cells.push((el.into_any_element(), 1));
        self
    }

    /// The next cell over the next two columns.
    pub fn cell2(mut self, el: impl IntoElement) -> Self {
        self.cells.push((el.into_any_element(), 2));
        self
    }

    /// A second line under the first on the same columns, each cell over its span (`col-span-full` on a subgrid,
    /// `mt-3 mb-1`): the Computers list's live band.
    pub fn line(mut self, cells: Vec<(AnyElement, usize)>) -> Self {
        self.line = Some(cells);
        self
    }
}

impl From<GridRow> for CardItem {
    fn from(row: GridRow) -> Self {
        CardItem::placed(move |corners| GridRow { corners, ..row }.into_any_element())
    }
}

impl RenderOnce for GridRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let floor = if self.tight { size::LINE_FLOOR } else { size::ROW_FLOOR };
        let body = h_flex()
            .gap_x_4()
            .children(cells(self.columns, self.cells.into_iter().map(|(el, span)| (keep_press(el, self.open.is_some()), span)).collect()));
        let line = self.line.map(|line| h_flex().mt_3().mb_1().gap_x_4().children(cells(self.columns, line)));
        let frame = row_frame(&self.id, floor, self.corners).debug_selector(|| "grid-row".into());
        let frame = match self.open {
            Some(open) => opens(frame, &self.id, window, cx).on_click(move |e, window, cx| open(e, window, cx)),
            None => {
                let over = motion::hovered(self.id.clone(), window, cx);
                let ground = if *over.read(cx) { p.accent.opacity(0.6) } else { p.accent.opacity(0.) };
                frame
                    .bg(motion::ease_ink(self.id.clone(), "ground", ground, motion::COLORS, window, cx))
                    .on_hover(motion::track_hover(over))
            }
        };
        frame.when_some(self.hover, |el, hover| el.tooltip(tip(hover))).child(body).children(line)
    }
}

/// A cell of a row that opens fills its column; a press on it opens the row, and the controls a cell holds keep their
/// own presses (`StateCell::act`).
fn keep_press(el: AnyElement, opens: bool) -> AnyElement {
    if !opens {
        return el;
    }
    h_flex().w_full().min_w_0().child(el).into_any_element()
}

/// `GridName`: the mark in its frame, the name, an optional tag beside it, and under it a note or a version fact.
#[derive(IntoElement)]
pub struct GridName {
    glyph: AnyElement,
    name: SharedString,
    tag: Option<SharedString>,
    note: Option<SharedString>,
    fact: Option<Vec<SharedString>>,
}

impl GridName {
    /// `glyph` is the framed mark: `glyph_frame(Glyph::Server, cx)`.
    pub fn new(glyph: impl IntoElement, name: impl Into<SharedString>) -> Self {
        GridName { glyph: glyph.into_any_element(), name: name.into(), tag: None, note: None, fact: None }
    }

    pub fn tag(mut self, tag: impl Into<SharedString>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    pub fn note(mut self, note: impl Into<SharedString>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn fact(mut self, parts: Vec<SharedString>) -> Self {
        self.fact = Some(parts);
        self
    }
}

impl RenderOnce for GridName {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        // GPUI's flex does not align text baselines, so the tag takes the name's 24 px line and sits down by what its
        // smaller ascent leaves: about a third of the 3 px between the two sizes.
        let name = h_flex()
            .min_w_0()
            .items_start()
            .gap_2()
            .child(
                div()
                    .debug_selector(|| "grid-name".into())
                    .min_w_0()
                    .text_size(text::LIST_TITLE.0)
                    .line_height(text::LIST_TITLE.1)
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(self.name),
            )
            .children(self.tag.map(|tag| {
                div()
                    .debug_selector(|| "grid-tag".into())
                    .flex_none()
                    .mt(px(1.))
                    .text_size(text::XS.0)
                    .line_height(text::LIST_TITLE.1)
                    .text_color(p.muted_foreground)
                    .child(tag)
            }));
        h_flex().min_w_0().gap_3().child(self.glyph).child(
            v_flex()
                .min_w_0()
                .child(name)
                .children(
                    self.note
                        .map(|note| div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(note)),
                )
                .children(self.fact.map(VersionFact::new)),
        )
    }
}

/// `VersionFact`: which wsp and daemon a computer runs, in the mono at the fact's size, broken between its parts and
/// never inside one.
#[derive(IntoElement)]
pub struct VersionFact {
    parts: Vec<SharedString>,
}

impl VersionFact {
    pub fn new(parts: Vec<SharedString>) -> Self {
        VersionFact { parts }
    }
}

impl RenderOnce for VersionFact {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        let last = self.parts.len().saturating_sub(1);
        h_flex()
            .debug_selector(|| "version-fact".into())
            .min_w_0()
            .flex_wrap()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(text::FACT)
            .line_height(px(20.))
            .text_color(p.muted_foreground)
            .children(self.parts.into_iter().enumerate().map(|(ix, part)| {
                let part: SharedString = if ix < last { format!("{part}, ").into() } else { part };
                div().flex_none().whitespace_nowrap().child(part)
            }))
    }
}

/// `Num`: a number or a version, set right in the `VALUE` class and cut at its cell's edge.
pub fn num(value: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .debug_selector(|| "num".into())
        .w_full()
        .min_w_0()
        .truncate()
        .text_right()
        .text_size(text::VALUE)
        .text_color(cx.palette().foreground)
        .child(value.into())
}

/// `StateCell`: the state as a word in the muted sans, or the act itself, set right; the whole sentence rides the
/// hover.
#[derive(IntoElement)]
pub struct StateCell {
    id: SharedString,
    content: Option<AnyElement>,
    why: Option<SharedString>,
}

impl StateCell {
    pub fn word(id: impl Into<SharedString>, word: impl Into<SharedString>) -> Self {
        StateCell { id: id.into(), content: Some(word.into().into_any_element()), why: None }
    }

    /// The act in the state's place: a button.
    pub fn act(id: impl Into<SharedString>, act: impl IntoElement) -> Self {
        // A press on the act is the act's, not the row's it stands in (`insideControl`).
        let act = div().on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()).child(act);
        StateCell { id: id.into(), content: Some(act.into_any_element()), why: None }
    }

    pub fn why(mut self, why: impl Into<SharedString>) -> Self {
        self.why = Some(why.into());
        self
    }
}

impl RenderOnce for StateCell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        h_flex()
            .id(self.id)
            .debug_selector(|| "state-cell".into())
            .w_full()
            .min_w_0()
            .justify_end()
            .whitespace_nowrap()
            .text_size(text::FACT)
            .line_height(px(20.))
            .text_color(cx.palette().muted_foreground)
            .when_some(self.why, |el, why| el.tooltip(tip(why)))
            .children(self.content)
    }
}

/// `Chevron`: a row that opens a page ends in it.
pub fn chevron(cx: &App) -> impl IntoElement {
    Icon::new(Glyph::ChevronRight).with_size(px(14.)).flex_none().text_color(cx.palette().muted_foreground)
}

/// `Slash`: the slash between crumbs, at half the muted ink.
pub fn slash(cx: &App) -> impl IntoElement {
    div().flex_none().text_color(cx.palette().muted_foreground.opacity(0.5)).child("/")
}
