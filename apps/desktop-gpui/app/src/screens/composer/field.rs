// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's field: its editor in a view of its own, which the composer draws cached at the height it was last
//! drawn at, so the caret's blink draws the field alone and never the composer around it.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::input::{Textarea, TextareaState};
use gpui_kit::component::Sizable as _;
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, Bounds, Context, Entity, MouseButton, Pixels, StyleRefinement, Subscription, WeakEntity,
    Window,
};

use super::model::Layout;
use super::{parts, Composer, LINE, TEXT};
use crate::ui::ActivePalette;

pub struct Field {
    editor: Entity<TextareaState>,
    composer: WeakEntity<Composer>,
    layout: Layout,
    shut: bool,
    /// The editor's width as the one-line layout lays it out, which the placeholder is fitted to.
    width: Rc<Cell<Pixels>>,
    drawn: Rc<Cell<Pixels>>,
    laid: Rc<Cell<Option<Pixels>>>,
    /// Set by any change of the editor or of what the field draws: its next frame draws it whole, not cached at the
    /// height it had.
    moved: Rc<Cell<bool>>,
    _edits: Subscription,
}

impl Field {
    pub fn new(editor: Entity<TextareaState>, composer: WeakEntity<Composer>, width: Rc<Cell<Pixels>>, cx: &mut Context<Self>) -> Self {
        let moved = Rc::new(Cell::new(true));
        let edits = {
            let moved = moved.clone();
            cx.observe(&editor, move |_, _, _| moved.set(true))
        };
        Field {
            editor,
            composer,
            layout: Layout::Full,
            shut: false,
            width,
            drawn: Rc::new(Cell::new(px(0.))),
            laid: Rc::new(Cell::new(None)),
            moved,
            _edits: edits,
        }
    }
}

/// The field as the composer draws it with `layout`: cached at its height as drawn while nothing it draws has changed,
/// else whole, measuring that height again.
pub fn drawn(field: &Entity<Field>, layout: Layout, shut: bool, cx: &mut Context<Composer>) -> AnyElement {
    let height = field.update(cx, |f, _| {
        let changed = f.layout != layout || f.shut != shut;
        (f.layout, f.shut) = (layout, shut);
        let height = Some(f.drawn.get()).filter(|h| *h > px(0.) && !changed && !f.moved.replace(false));
        f.laid.set(height);
        height
    });
    match height {
        Some(height) => field.clone().cached(StyleRefinement::default().flex_1().min_w_0().h(height)).into_any_element(),
        None => field.clone().into_any_element(),
    }
}

impl Render for Field {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("ComposerField");
        let p = cx.palette().clone();
        let paste = self.composer.clone();
        let (width, drawn, laid, moved, me) =
            (self.width.clone(), self.drawn.clone(), self.laid.clone(), self.moved.clone(), cx.entity().downgrade());
        // Laid out cached at a height it no longer has, the field asks to be drawn whole in the next frame.
        let measure = canvas(
            move |bounds: Bounds<Pixels>, _, cx| {
                drawn.set(bounds.size.height);
                if laid.get().is_some_and(|at| at != bounds.size.height) {
                    moved.set(true);
                    let me = me.clone();
                    cx.defer(move |cx| _ = me.update(cx, |_, cx| cx.notify()));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        div()
            .id("composer-editor")
            .debug_selector(|| "composer-editor".into())
            .relative()
            .flex_1()
            .min_w_0()
            .w_full()
            .text_size(TEXT)
            .line_height(LINE)
            .text_color(p.foreground)
            .when(self.layout == Layout::Full, |el| el.min_h(px(74.)).pb_1())
            // A press anywhere in the box puts the caret in the field, as the web's editor fills its whole box.
            .when(!self.shut, |el| {
                el.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        this.editor.update(cx, |e, cx| e.focus(window, cx));
                        window.prevent_default();
                    }),
                )
            })
            .child(
                // gpui-kit pads the field 4 px a side and wraps its text 10 px short of its right edge for the caret;
                // the web's text runs edge to edge of its column. The field is w_full, so the margins go on a wrapper.
                div().ml(px(-4.)).mr(px(-14.)).child(
                    Textarea::new(&self.editor)
                        .appearance(false)
                        .xsmall()
                        .disabled(self.shut)
                        .text_size(TEXT)
                        .line_height(LINE)
                        .token(|ctx, _, cx| parts::chip(ctx, cx))
                        .on_paste(move |item, _, cx| {
                            let entries = item.entries().to_vec();
                            paste.update(cx, |this, cx| this.on_paste(entries, cx)).unwrap_or(false)
                        }),
                ),
            )
            .when(self.layout == Layout::Line, |el| {
                el.child(canvas(move |bounds: Bounds<Pixels>, _, _| width.set(bounds.size.width), |_, _, _, _| {}).absolute().size_full())
            })
            .child(measure)
    }
}
