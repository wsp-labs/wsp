// SPDX-License-Identifier: AGPL-3.0-only
//! Each piece type's view, after `apps/web/src/slate/pieces/`: a draw function per type and the flags the layout
//! reads off a type without drawing it (`PieceView` in `SlateView.tsx`). A piece draws from a [`PieceCx`]: its props
//! as the host resolved them, its rows, the slate it stands in, and the screen to raise its events on.

pub mod barswitch;
pub mod charts;
pub mod controls;
pub mod layout;
pub mod lists;
pub mod look;
pub mod media;
pub mod numbers;
pub mod output;
pub mod riders;
pub mod text;
pub mod zoom;

use gpui_kit::{div, prelude::*, px, AnyElement, App, SharedString, WeakEntity, Window};
use serde_json::{Map, Value};

use super::model::{Fallback, Piece, Row};
use super::{Slate, SlateScreen};
use crate::ui::ActivePalette;

/// The flags of a type, as the web's piece views set them.
#[derive(Clone, Copy, Default)]
pub struct Flags {
    /// Never a row of a card: it stands bare between cards.
    pub bare: bool,
    /// Heads what stands beside it, so a row holding it is a head row.
    pub heads: bool,
    /// A control: a row of nothing else is the toolbar.
    pub control: bool,
    /// Says its own refreshing.
    pub says_refreshing: bool,
    /// Takes the slate's one accent when it names no tone.
    pub accent: bool,
    /// Lines its columns up with the tables beside it.
    pub aligns: bool,
    /// The edge of what lines up.
    pub bounds: bool,
}

pub fn flags(kind: &str) -> Flags {
    let bare = Flags { bare: true, ..Flags::default() };
    match kind {
        "heading" => Flags { heads: true, ..bare },
        "text" | "markdown" | "diagram" | "image" | "images" | "bars" | "stats" => bare,
        "timeline" | "donut" | "treemap" | "chart" => Flags { accent: true, ..bare },
        "button" => Flags { control: true, ..bare },
        "input" | "select" | "toggle" => Flags { control: true, ..Flags::default() },
        "section" => Flags { bounds: true, says_refreshing: true, ..bare },
        "output" => Flags { says_refreshing: true, ..Flags::default() },
        "table" => Flags { aligns: true, ..bare },
        _ => Flags::default(),
    }
}

/// The types this build draws, `SLATE_PIECES` in `protocol:slate/catalog.ts`.
pub const KNOWN: [&str; 34] = [
    "column",
    "row",
    "grid",
    "section",
    "heading",
    "text",
    "markdown",
    "number",
    "meter",
    "ring",
    "chart",
    "donut",
    "timeline",
    "treemap",
    "diagram",
    "image",
    "images",
    "sparkline",
    "bars",
    "status",
    "chip",
    "facts",
    "table",
    "checklist",
    "output",
    "button",
    "input",
    "select",
    "choices",
    "toggle",
    "empty",
    "decision",
    "stats",
    "progress",
];

/// What one piece draws from.
pub struct PieceCx<'a> {
    pub slate: &'a Slate,
    pub piece: &'a Piece,
    /// Every prop resolved, but the ones the piece reads per row; a prop whose formula has not answered yet is absent.
    pub props: Map<String, Value>,
    /// Its rows, for a piece with items: None until the host answers them.
    pub rows: Option<Vec<Row>>,
    pub screen: WeakEntity<SlateScreen>,
}

impl<'a> PieceCx<'a> {
    pub fn id(&self) -> &'a str {
        &self.piece.id
    }

    pub fn prop(&self, name: &str) -> Option<&Value> {
        self.props.get(name).filter(|v| !v.is_null())
    }

    pub fn str(&self, name: &str) -> Option<String> {
        look::str(self.prop(name))
    }

    pub fn num(&self, name: &str) -> Option<f64> {
        self.prop(name).and_then(Value::as_f64).filter(|n| n.is_finite())
    }

    /// Whether this piece's press is out and not answered yet.
    pub fn busy(&self) -> bool {
        self.slate.busy.contains(&self.piece.id)
    }

    /// What this piece's last press said, or why it was refused, for its two seconds.
    pub fn outcome(&self) -> Option<&'a super::Outcome> {
        self.slate.outcomes.get(&self.piece.id)
    }

    pub fn bool(&self, name: &str) -> bool {
        self.prop(name).and_then(Value::as_bool) == Some(true)
    }

    /// The element id of a part of this piece, unique across the slate.
    pub fn el(&self, part: &str) -> SharedString {
        SharedString::from(format!("slate-{}-{part}", self.piece.id))
    }

    /// Draws one child piece by id, as its own host would.
    pub fn child(&self, id: &str, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        draw(self.slate, id, &self.screen, window, cx)
    }

    pub fn children(&self, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
        self.piece.children.iter().filter_map(|c| self.child(c, window, cx)).collect()
    }
}

/// One piece by id: nothing where it is hidden or gone, its fallback where its type is unknown, else its view in its
/// box. The box carries the piece's id and type for the tests, as `[data-slate-piece][data-slate-type]` does.
pub fn draw(slate: &Slate, id: &str, screen: &WeakEntity<SlateScreen>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let piece = slate.doc()?.piece(id)?;
    if !slate.shown(id) {
        return None;
    }
    if !KNOWN.contains(&piece.kind.as_str()) {
        return fallback(slate, piece, screen, window, cx);
    }
    let pcx = PieceCx { slate, piece, props: slate.props(piece), rows: slate.rows(piece), screen: screen.clone() };
    let body = view(&pcx, window, cx)?;
    let (id, kind) = (piece.id.clone(), piece.kind.clone());
    let fade = SharedString::from(format!("slate-{id}-in"));
    Some(crate::ui::motion::fade_in(fade, div().min_w_0().debug_selector(move || format!("slate-piece-{id}-{kind}")).child(body), cx))
}

fn view(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "column" | "row" | "grid" | "section" => layout::draw(p, window, cx),
        "heading" | "text" | "markdown" | "status" | "chip" | "facts" | "empty" => text::draw(p, window, cx),
        "number" | "meter" | "ring" | "sparkline" | "bars" | "stats" => numbers::draw(p, window, cx),
        "chart" | "donut" | "timeline" | "treemap" => charts::draw(p, window, cx),
        "diagram" | "image" | "images" => media::draw(p, window, cx),
        "table" | "checklist" | "progress" => lists::draw(p, window, cx),
        "button" | "input" | "select" | "choices" | "decision" | "toggle" => controls::draw(p, window, cx),
        "output" => output::draw(p, window, cx),
        _ => None,
    }
}

/// What a type this build does not know draws: its fallback piece, nothing, its sentence, or the one quiet line.
fn fallback(slate: &Slate, piece: &Piece, screen: &WeakEntity<SlateScreen>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match &piece.fallback {
        Fallback::Drop => None,
        Fallback::Piece(f) if slate.doc()?.piece(f).is_some_and(|t| KNOWN.contains(&t.kind.as_str())) => draw(slate, f, screen, window, cx),
        Fallback::Text(t) => Some(quiet(t.clone(), cx)),
        _ => Some(quiet("This part needs a newer wsp", cx)),
    }
}

/// `Quiet`: a 12 px muted line, a piece's placeholder and every "could not" it says.
pub fn quiet(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div().text_size(px(12.)).line_height(px(16.)).text_color(cx.palette().muted_foreground).child(text.into()).into_any_element()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{size, TestAppContext, VisualTestContext, WeakEntity};
    use serde_json::json;

    use super::*;
    use crate::screens::slate::pieces::layout::tests::slate;

    struct Drawn(Slate);

    impl Render for Drawn {
        fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full().children(draw(&self.0, "root", &WeakEntity::new_invalid(), window, cx))
        }
    }

    fn drawn(cx: &mut TestAppContext, still: bool) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
            cx.set_global(crate::ui::Still(still));
        });
        let s = slate(json!({
            "root": {"type": "column", "children": ["t"]},
            "t": {"type": "text", "props": {"value": "Fades in"}},
        }));
        let (_, cx) = cx.add_window_view(move |_, _| Drawn(s));
        cx.simulate_resize(size(px(540.), px(400.)));
        cx
    }

    /// The frames one drawn frame asks for, after what earlier frames asked for is spent.
    fn asked(cx: &mut VisualTestContext) -> usize {
        cx.update(|window, cx| {
            window.simulate_next_frame(cx);
            window.render_frame(cx);
            window.simulate_next_frame(cx)
        })
    }

    #[gpui_kit::test]
    fn a_piece_fades_in_once_then_asks_for_no_frame(cx: &mut TestAppContext) {
        let cx = drawn(cx, false);
        assert!(asked(cx) > 0, "the piece fades in as it first draws");
        std::thread::sleep(Duration::from_millis(200));
        asked(cx);
        assert_eq!(asked(cx), 0, "past its 150 ms the fade is done and asks for no frame");
    }

    #[gpui_kit::test]
    fn under_reduced_motion_a_piece_stands_at_once(cx: &mut TestAppContext) {
        let cx = drawn(cx, true);
        assert_eq!(asked(cx), 0, "with motion reduced the piece asks for no frame");
    }
}
