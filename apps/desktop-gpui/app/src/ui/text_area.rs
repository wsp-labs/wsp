// SPDX-License-Identifier: AGPL-3.0-only
//! The multi-line field, gpui-kit's textarea in today's looks: the field (`components/ui/textarea.tsx`, the commit box
//! and the review's summary), a line comment's field (`DiffCommentAnnotation.tsx`) and the bare field a framed box
//! holds (`ThreadActs.tsx`'s reply and comment box). It grows with its words from its floor to `MAX_ROWS` lines, then
//! scrolls; Cmd or Ctrl and Enter submits and Escape cancels when the caller hooks them. The field and the comment's
//! carry the browser's resize grip in their corner (preflight's `resize: vertical`), which drags their height.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::input::{Enter, Escape, Textarea, TextareaState};
use gpui_kit::component::{v_flex, ActiveTheme as _, Sizable as _};
use gpui_kit::{
    canvas, div, point, prelude::*, px, relative, App, Bounds, CursorStyle, DragMoveEvent, ElementId, Empty, Entity, FocusHandle,
    Focusable as _, Hsla, MouseButton, PathBuilder, Pixels, SharedString, Window,
};

use super::controls::Dim;
use super::size;
use super::theme::ActivePalette;

/// The lines a field grows to before it scrolls; the web's `field-sizing-content` has no ceiling, a pane does.
pub const MAX_ROWS: usize = 12;

/// gpui-kit's xsmall textarea pads its text 4 px a side; the look's own padding stands on the frame instead.
const KIT_PAD_X: Pixels = px(4.);

type Hook = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

/// Chromium's resizer: the scrollbar's 15 px square in the corner inside the edge.
const GRIP: Pixels = px(15.);

/// What a drag on a field's grip carries: the field's id.
#[derive(Clone)]
struct Grip(SharedString);

/// Which of today's textareas it draws as.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Look {
    /// `Textarea`'s own: the input edge and fill, the ring's edge on focus, `min-h-17.5`, 14 px words.
    Field,
    /// A line comment's: the border at half, the page at a fifth, `min-h-12`, 12 px words on 20 px lines.
    Comment,
    /// No edge and no fill, for a box that frames it with its acts: `min-h-[52px]`, 13.5 px words.
    Bare,
}

struct Spec {
    pad_x: Pixels,
    pad_y: Pixels,
    /// The textarea's `min-h`, inside the edge.
    floor: Pixels,
    text: (Pixels, Pixels),
    edged: bool,
}

impl Look {
    fn spec(self) -> Spec {
        match self {
            // `px-[calc(--spacing(3)-1px)] py-[calc(--spacing(1.5)-1px)]`, `sm:text-sm`.
            Look::Field => Spec { pad_x: px(11.), pad_y: px(5.), floor: px(70.), text: (px(14.), px(20.)), edged: true },
            Look::Comment => Spec { pad_x: px(10.), pad_y: px(6.), floor: px(48.), text: (px(12.), px(20.)), edged: true },
            // `leading-normal` at 13.5 px.
            Look::Bare => Spec { pad_x: px(10.), pad_y: px(8.), floor: px(52.), text: (px(13.5), px(20.25)), edged: false },
        }
    }

    /// `ThreadActs.tsx` sets `resize-none` on the bare field; the other two keep preflight's grip.
    fn resizes(self) -> bool {
        self != Look::Bare
    }
}

/// Cmd on a Mac, Ctrl elsewhere: the key the submit chord's hint names.
pub fn chord_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "⌘"
    } else {
        "Ctrl"
    }
}

/// A multi-line field over a `TextareaState` made with [`TextArea::state`].
#[derive(IntoElement)]
pub struct TextArea {
    id: SharedString,
    state: Entity<TextareaState>,
    look: Look,
    mono: bool,
    text: Option<(Pixels, Pixels)>,
    floor: Option<Pixels>,
    disabled: bool,
    invalid: bool,
    on_submit: Option<Hook>,
    on_escape: Option<Hook>,
}

impl TextArea {
    /// The state a field keeps: its words, caret and placeholder; it grows from one line to `MAX_ROWS`.
    pub fn state(placeholder: impl Into<SharedString>, window: &mut Window, cx: &mut App) -> Entity<TextareaState> {
        let placeholder = placeholder.into();
        cx.new(|cx| TextareaState::new(window, cx).auto_grow(1, MAX_ROWS).placeholder(placeholder))
    }

    pub fn new(id: impl Into<SharedString>, state: &Entity<TextareaState>) -> Self {
        TextArea {
            id: id.into(),
            state: state.clone(),
            look: Look::Field,
            mono: false,
            text: None,
            floor: None,
            disabled: false,
            invalid: false,
            on_submit: None,
            on_escape: None,
        }
    }

    pub fn look(mut self, look: Look) -> Self {
        self.look = look;
        self
    }

    /// The mono family, as the commit message's `font-mono`.
    pub fn mono(mut self) -> Self {
        self.mono = true;
        self
    }

    /// A size and line height other than the look's.
    pub fn text_size(mut self, size: Pixels, line: Pixels) -> Self {
        self.text = Some((size, line));
        self
    }

    /// A floor other than the look's, inside the edge.
    pub fn min_height(mut self, floor: Pixels) -> Self {
        self.floor = Some(floor);
        self
    }

    /// Kept at 64% and taking nothing, as `has-disabled:opacity-64`.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// `aria-invalid`: the destructive edge, stronger while it has focus.
    pub fn invalid(mut self, invalid: bool) -> Self {
        self.invalid = invalid;
        self
    }

    /// Cmd or Ctrl and Enter: called when the words are more than blanks; no newline goes in either way.
    pub fn on_submit(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_submit = Some(Rc::new(f));
        self
    }

    /// Escape while it has focus; without a hook Escape goes on to what holds the field, as a dialog.
    pub fn on_escape(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_escape = Some(Rc::new(f));
        self
    }
}

impl RenderOnce for TextArea {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let spec = self.look.spec();
        let (text_size, line) = self.text.unwrap_or(spec.text);
        let floor = self.floor.unwrap_or(spec.floor);
        let focus: FocusHandle = self.state.read(cx).focus_handle(cx);
        let focused = focus.is_focused(window);
        let (edge, fill) = match self.look {
            Look::Field => {
                let edge = match (self.invalid, focused) {
                    (true, true) => p.destructive.opacity(0.64),
                    (true, false) => p.destructive.opacity(0.36),
                    (false, true) => p.ring,
                    (false, false) => p.input,
                };
                (edge, p.input_fill)
            }
            Look::Comment => {
                let edge = match (self.invalid, focused) {
                    (true, true) => p.destructive.opacity(0.64),
                    (true, false) => p.destructive.opacity(0.36),
                    (false, true) => p.border.opacity(0.7),
                    (false, false) => p.border.opacity(0.5),
                };
                (edge, p.background.opacity(0.2))
            }
            Look::Bare => (gpui_kit::transparent_black(), gpui_kit::transparent_black()),
        };
        let (edge, fill, ink) = if self.disabled {
            let dim = Dim::new(p.background, fill);
            (dim.over(edge), if fill.a > 0. { dim.fill() } else { fill }, dim.over(p.foreground))
        } else {
            (edge, fill, p.foreground)
        };
        // Preflight's `::placeholder`: the text's own ink at half.
        self.state.update(cx, |s, cx| s.set_placeholder_color(Some(p.foreground.opacity(0.5)), cx));
        let edge_w = if spec.edged { px(1.) } else { px(0.) };
        let resizes = self.look.resizes();
        let dragged = window.use_keyed_state(ElementId::from((ElementId::from(self.id.clone()), "height")), cx, |_, _| None::<Pixels>);
        let grab = window.use_keyed_state(ElementId::from((ElementId::from(self.id.clone()), "grab")), cx, |_, _| px(0.));
        let height = *dragged.read(cx);
        let mono = self.mono.then(|| cx.theme().mono_font_family.clone());
        let field = Textarea::new(&self.state)
            .appearance(false)
            .xsmall()
            .disabled(self.disabled)
            .text_size(text_size)
            .line_height(line)
            .text_color(ink)
            .when_some(mono, |el, family| el.font_family(family))
            .when(height.is_some(), |el| el.h(relative(1.)));
        let state = self.state.clone();
        let grip = resizes.then(|| grip(self.id.clone(), grab.clone(), edge_w));
        let (area, least) = (self.id.clone(), floor + edge_w * 2.);
        v_flex()
            .id(self.id)
            .debug_selector(|| "text-area".into())
            .key_context("TextArea")
            .relative()
            .w_full()
            .min_h(least)
            .when_some(height, |el, h| el.h(h))
            .px(spec.pad_x - KIT_PAD_X)
            .py(spec.pad_y)
            .rounded(size::BUTTON_RADIUS)
            .when(spec.edged, |el| el.border_1().border_color(edge))
            .bg(fill)
            .text_size(text_size)
            .line_height(line)
            .when(!self.disabled, |el| el.cursor_text())
            // The floor under the last line is the field's too, as a textarea's whole box takes the caret.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if !focus.is_focused(window) {
                    focus.focus(window, cx);
                }
            })
            .when_some(self.on_submit.filter(|_| !self.disabled), |el, f| {
                el.capture_action(move |action: &Enter, window, cx| {
                    if !action.secondary {
                        return;
                    }
                    cx.stop_propagation();
                    if !state.read(cx).value().trim().is_empty() {
                        f(window, cx);
                    }
                })
            })
            .when_some(self.on_escape, |el, f| {
                el.capture_action(move |_: &Escape, window, cx| {
                    cx.stop_propagation();
                    f(window, cx);
                })
            })
            .when(resizes, |el| {
                el.on_drag_move(move |e: &DragMoveEvent<Grip>, window, cx| {
                    if e.drag(cx).0 != area {
                        return;
                    }
                    let h = (e.event.position.y - e.bounds.origin.y + *grab.read(cx)).max(least);
                    dragged.update(cx, |d, _| *d = Some(h));
                    window.refresh();
                })
            })
            .child(field)
            .children(grip)
            .when(self.disabled, |el| el.child(div().absolute().inset_0().occlude()))
    }
}

/// The resizer Chromium paints in a textarea's corner: two diagonal strokes in black at 60%, each with a white one at
/// 60% a pixel under it, the same in every theme. Dragging it sets the field's height; `grab` keeps the pointer's
/// distance from the field's foot.
fn grip(id: SharedString, grab: Entity<Pixels>, edge: Pixels) -> impl IntoElement {
    // A drag starts some pixels after the press, so the press is where the pointer's distance is read.
    let drawn = Rc::new(Cell::new(Bounds::default()));
    let at = drawn.clone();
    div()
        .id(ElementId::from((ElementId::from(id.clone()), "grip")))
        .debug_selector(|| "text-area-grip".into())
        .absolute()
        .right_0()
        .bottom_0()
        .size(GRIP)
        .cursor(CursorStyle::ResizeUpDown)
        .on_mouse_down(MouseButton::Left, move |e, _, cx| {
            cx.stop_propagation();
            grab.update(cx, |g, _| *g = at.get().bottom() + edge - e.position.y);
        })
        .on_drag(Grip(id), |_, _, _, cx| cx.new(|_| Empty))
        .child(canvas(move |bounds, _, _| drawn.set(bounds), |bounds, _, window, _| paint_grip(bounds, window)).size_full())
}

fn paint_grip(bounds: Bounds<Pixels>, window: &mut Window) {
    let corner = bounds.bottom_right();
    // Read off today's at scale 2: each stroke's centre line, from its top end to its foot, from the inner corner.
    let strokes = [((-0.65, -8.), (-7.65, -1.)), ((-0.65, -4.5), (-4.15, -1.))];
    let ink = |down: f32, color: Hsla, window: &mut Window| {
        for ((x0, y0), (x1, y1)) in strokes {
            let mut line = PathBuilder::stroke(px(0.71));
            line.move_to(point(corner.x + px(x0), corner.y + px(y0 + down)));
            line.line_to(point(corner.x + px(x1), corner.y + px(y1 + down)));
            if let Ok(path) = line.build() {
                window.paint_path(path, color);
            }
        }
    };
    ink(0., gpui_kit::black().opacity(0.6), window);
    ink(1., gpui_kit::white().opacity(0.6), window);
}
