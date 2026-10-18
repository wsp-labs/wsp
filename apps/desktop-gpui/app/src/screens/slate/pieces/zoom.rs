// SPDX-License-Identifier: AGPL-3.0-only
//! A drawing in a frame a person can zoom and pan, after `apps/web/src/slate/pieces/zoom.tsx`: it opens fitted to the
//! whole frame but never under its floor, centred where it is smaller than the frame. Ctrl or cmd with the wheel zooms
//! around the pointer, a drag pans once the drawing outgrows the frame, and a double press fits it again. A plain wheel
//! is left to what is under it.

use gpui_kit::{canvas, div, prelude::*, px, AnyElement, App, Bounds, Entity, MouseButton, Pixels, Point, SharedString, Window};

/// The most and least a drawing scales to.
const MOST: f32 = 4.;
const LEAST: f32 = 0.2;

/// Where the drawing stands in its frame: its scale and its top left corner.
#[derive(Default)]
pub struct Zoom {
    k: f32,
    x: f32,
    y: f32,
    /// The person zoomed or panned, so a resize keeps their view until they fit it again.
    moved: bool,
    drag: Option<Point<Pixels>>,
    natural: (f32, f32),
    frame: Bounds<Pixels>,
    floor: f32,
}

impl Zoom {
    fn room(&self) -> (f32, f32) {
        (f32::from(self.frame.size.width), f32::from(self.frame.size.height))
    }

    fn fitted(&self) -> f32 {
        let ((w, h), (nw, nh)) = (self.room(), self.natural);
        if w <= 0. || nw <= 0. || nh <= 0. {
            return 1.;
        }
        self.floor.max(1f32.min(w / nw).min(if h > 0. { h / nh } else { f32::INFINITY }))
    }

    /// Centred where it is smaller than the frame, its edges never inside the frame's where larger.
    fn place(&mut self) {
        let along = |at: f32, size: f32, room: f32| if size <= room { (room - size) / 2. } else { at.clamp(room - size, 0.) };
        let (w, h) = self.room();
        self.x = along(self.x, self.natural.0 * self.k, w);
        self.y = along(self.y, self.natural.1 * self.k, h);
    }

    pub fn fit(&mut self) {
        self.moved = false;
        self.k = self.fitted();
        self.place();
    }

    /// Zooms by `by` around a point in the frame.
    fn zoom_at(&mut self, at: (f32, f32), by: f32) {
        self.moved = true;
        let k = (self.k * by).clamp(LEAST, MOST);
        self.x = at.0 - (at.0 - self.x) * k / self.k;
        self.y = at.1 - (at.1 - self.y) * k / self.k;
        self.k = k;
        self.place();
    }

    /// Zooms by `by` around the frame's middle, as the dialog's zoom buttons do.
    pub fn zoom(&mut self, by: f32) {
        let (w, h) = self.room();
        self.zoom_at((w / 2., h / 2.), by);
    }

    fn outgrows(&self) -> bool {
        let (w, h) = self.room();
        self.natural.0 * self.k > w + 1. || self.natural.1 * self.k > h + 1.
    }

    /// Takes the frame's box as it was laid out; a new box refits unless the person moved the view.
    fn measured(&mut self, frame: Bounds<Pixels>) -> bool {
        if frame == self.frame {
            return false;
        }
        self.frame = frame;
        if self.moved {
            self.place();
        } else {
            self.fit();
        }
        true
    }
}

/// The zoom state of a frame, kept by `key` while it is drawn.
pub fn state(key: SharedString, natural: (f32, f32), floor: f32, window: &mut Window, cx: &mut App) -> Entity<Zoom> {
    let zoom = window.use_keyed_state(key, cx, |_, _| Zoom { k: 1., ..Zoom::default() });
    zoom.update(cx, |z, _| {
        if z.natural != natural || z.floor != floor {
            z.natural = natural;
            z.floor = floor;
            z.fit();
        }
    });
    zoom
}

/// The frame drawing `content` at the drawing's natural size `natural`, scaled and placed as `zoom` says.
pub fn frame(id: SharedString, zoom: &Entity<Zoom>, content: AnyElement, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let z = zoom.read(cx);
    let (k, x, y, (nw, nh)) = (z.k, z.x, z.y, z.natural);
    let grab = z.outgrows();
    let (probe, wheel, down, moving, up) = (zoom.clone(), zoom.clone(), zoom.clone(), zoom.clone(), zoom.clone());
    div()
        .id(id)
        .debug_selector(|| "slate-zoom".into())
        .relative()
        .min_w_0()
        .overflow_hidden()
        .when(grab, |el| el.cursor_grab())
        .child(
            canvas(
                move |bounds, _, cx| {
                    probe.update(cx, |z, cx| {
                        if z.measured(bounds) {
                            cx.notify();
                        }
                    })
                },
                |_, _, _, _| {},
            )
            .absolute()
            .size_full(),
        )
        .child(div().absolute().left(px(x)).top(px(y)).w(px(nw * k)).h(px(nh * k)).child(content))
        .on_scroll_wheel(move |e, window, cx| {
            if !e.modifiers.control && !e.modifiers.platform {
                return;
            }
            cx.stop_propagation();
            let dy = f32::from(e.delta.pixel_delta(window.line_height()).y);
            wheel.update(cx, |z, cx| {
                let at = (f32::from(e.position.x - z.frame.origin.x), f32::from(e.position.y - z.frame.origin.y));
                z.zoom_at(at, (dy * 0.01).exp());
                cx.notify();
            });
        })
        .on_mouse_down(MouseButton::Left, move |e, _, cx| {
            down.update(cx, |z, cx| {
                if e.click_count == 2 {
                    z.fit();
                    z.drag = None;
                    cx.notify();
                } else if z.outgrows() {
                    z.drag = Some(e.position);
                }
            })
        })
        .on_mouse_move(move |e, _, cx| {
            moving.update(cx, |z, cx| {
                let Some(from) = z.drag else { return };
                if e.pressed_button != Some(MouseButton::Left) {
                    z.drag = None;
                    return;
                }
                z.drag = Some(e.position);
                z.moved = true;
                z.x += f32::from(e.position.x - from.x);
                z.y += f32::from(e.position.y - from.y);
                z.place();
                cx.notify();
            })
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| up.update(cx, |z, _| z.drag = None))
}
