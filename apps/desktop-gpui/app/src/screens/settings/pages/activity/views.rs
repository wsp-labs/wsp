// SPDX-License-Identifier: AGPL-3.0-only
//! The page's live pieces, each a view of its own drawn cached in the box its figure stands in: a row's figure, a
//! tile's figure over its note, a tile's line and a thread head's title with its clock. A frame, a reading or a list
//! the host pushes lays each piece's look again, and only a piece whose look moved draws again, in place; the page
//! itself draws again only where what it lays out moved.

use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Corners, Entity, FontWeight, SharedString, StyleRefinement, WeakEntity, Window,
};

use super::model::Tint;
use super::tables::{open_menu, tint_ink};
use super::Scoped;
use crate::ui::{text, ActivePalette, Button, Glyph, MenuAct, Sparkline, Status, StatusMark};

/// A tile line's ink: CPU sky, memory violet and power teal (`LINE_INKS`, `activity/page.tsx`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Ink {
    Cpu,
    Memory,
    Power,
}

/// What a piece shows.
#[derive(Clone)]
pub(crate) enum Look {
    /// A row's figure in its column, inked past its mark.
    Figure { text: String, tint: Option<Tint> },
    /// A tile's figure over its note.
    Tile { value: String, note: String, tint: Option<Tint> },
    /// A tile's line of the last minutes.
    Line { points: Vec<(f32, f32)>, ink: Ink },
    /// A thread head's name and its status mark, and under 768 px the button its menu opens from.
    Title { name: String, status: Option<Status>, menu: Option<(SharedString, Rc<Vec<MenuAct>>)> },
}

impl PartialEq for Look {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Look::Figure { text: a, tint: x }, Look::Figure { text: b, tint: y }) => a == b && x == y,
            (Look::Tile { value: a, note: n, tint: x }, Look::Tile { value: b, note: m, tint: y }) => a == b && n == m && x == y,
            (Look::Line { points: a, ink: x }, Look::Line { points: b, ink: y }) => a == b && x == y,
            // A menu's acts are the thread's own, which its key names: its button's id says all that moves.
            (Look::Title { name: a, status: x, menu: p }, Look::Title { name: b, status: y, menu: q }) => {
                a == b && x == y && p.as_ref().map(|m| &m.0) == q.as_ref().map(|m| &m.0)
            }
            _ => false,
        }
    }
}

/// The figure's height on its row, `VALUE`'s leading.
const FIGURE: f32 = 20.;
/// A tile's figure, 32 px of leading, its 6 px gap and its note's 16 px.
const TILE: f32 = 32. + 6. + 16.;
/// A tile's line.
pub(crate) const LINE: f32 = 40.;

impl Look {
    /// The box the piece is drawn in, the one its figure filled when the page laid it out itself.
    fn bounds(&self) -> StyleRefinement {
        let style = StyleRefinement::default().w_full();
        match self {
            Look::Figure { .. } => style.h(px(FIGURE)),
            Look::Tile { .. } => style.h(px(TILE)),
            Look::Line { .. } => style.h(px(LINE)),
            Look::Title { .. } => style.h(text::LIST_TITLE.1),
        }
    }

    /// The look as drawn: in its piece, or on the page where no piece stands for it.
    pub(crate) fn draw(&self, scoped: &WeakEntity<Scoped>, cx: &App) -> AnyElement {
        let p = cx.palette();
        match self {
            Look::Figure { text: value, tint } => num_cell(value.clone(), tint_ink(*tint, cx)),
            Look::Tile { value, note, tint } => v_flex()
                .w_full()
                .gap(px(6.))
                .child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(26.))
                        .line_height(px(32.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(tint_ink(*tint, cx))
                        .child(value.clone()),
                )
                .child(div().truncate().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(note.clone()))
                .into_any_element(),
            Look::Line { points, ink } => {
                let ink = match ink {
                    Ink::Cpu => p.hue_blue,
                    Ink::Memory => p.hue_violet,
                    Ink::Power => p.hue_teal,
                };
                Sparkline::new(points.clone(), ink).height(px(LINE)).into_any_element()
            }
            Look::Title { name, status, menu } => {
                let head_menu = menu.clone().map(|(id, acts)| {
                    let scoped = scoped.clone();
                    let button = Button::icon_xs(id, Glyph::Ellipsis)
                        .side(px(20.))
                        .corners(Corners::all(px(4.)))
                        .on_click(move |event, window, cx| open_menu(&scoped, event.position(), acts.clone(), window, cx));
                    div()
                        .debug_selector(|| "activity-head-menu".into())
                        .flex_none()
                        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(button)
                });
                h_flex()
                    .w_full()
                    .min_w_0()
                    .gap_2()
                    .child(
                        div()
                            .debug_selector(|| "grid-name".into())
                            .min_w_0()
                            .truncate()
                            .text_size(text::LIST_TITLE.0)
                            .line_height(text::LIST_TITLE.1)
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(p.foreground)
                            .child(name.clone()),
                    )
                    .children(status.clone().map(StatusMark::new))
                    .children(head_menu)
                    .into_any_element()
            }
        }
    }
}

/// A figure on its row, right of its column, in the row's tabular digits.
pub(crate) fn num_cell(value: String, ink: gpui_kit::Hsla) -> AnyElement {
    div()
        .w_full()
        .min_w_0()
        .truncate()
        .text_right()
        .text_size(text::VALUE.0)
        .line_height(px(FIGURE))
        .text_color(ink)
        .font_features(text::tabular())
        .child(value)
        .into_any_element()
}

/// One live piece.
pub struct Piece {
    look: Look,
    scoped: WeakEntity<Scoped>,
}

#[cfg(test)]
impl Piece {
    pub(crate) fn look(&self) -> &Look {
        &self.look
    }
}

impl Render for Piece {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count(match self.look {
            Look::Figure { .. } => "ActivityFigure",
            Look::Tile { .. } => "ActivityTile",
            Look::Line { .. } => "ActivityLine",
            Look::Title { .. } => "ActivityTitle",
        });
        self.look.draw(&self.scoped, cx)
    }
}

/// The pieces standing, by the key the page draws each under.
pub(crate) type Drawn = Rc<HashMap<SharedString, Entity<Piece>>>;

/// The page's pieces, made as their keys first come and let go once no look names them.
#[derive(Default)]
pub(crate) struct Pieces {
    held: Drawn,
}

impl Pieces {
    /// Lays each look on its piece. `loud` draws again, in place, each piece whose look moved; the page drawing them
    /// all again needs none of that.
    pub(crate) fn lay(&mut self, looks: Vec<(SharedString, Look)>, scoped: &WeakEntity<Scoped>, loud: bool, cx: &mut App) {
        let mut was = std::mem::take(&mut self.held);
        let was = Rc::make_mut(&mut was);
        let mut held = HashMap::with_capacity(looks.len());
        for (key, look) in looks {
            let piece = match was.remove(&key) {
                Some(piece) => {
                    piece.update(cx, |piece, cx| {
                        let moved = piece.look != look;
                        piece.look = look;
                        if moved && loud {
                            cx.notify_in_place();
                        }
                    });
                    piece
                }
                None => cx.new(|_| Piece { look, scoped: scoped.clone() }),
            };
            held.insert(key, piece);
        }
        self.held = Rc::new(held);
    }

    pub(crate) fn drawn(&self) -> Drawn {
        self.held.clone()
    }
}

/// `look` in its piece under `key`, or drawn plainly where none stands.
pub(crate) fn piece(drawn: &Drawn, key: &str, look: Look, scoped: &WeakEntity<Scoped>, cx: &App) -> AnyElement {
    match drawn.get(key) {
        Some(piece) => piece.clone().cached(look.bounds()).into_any_element(),
        None => look.draw(scoped, cx),
    }
}
