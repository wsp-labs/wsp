// SPDX-License-Identifier: AGPL-3.0-only
//! The shimmer over a live line and Thinking (`live-activity-focus`), drawn over the cached rows so it builds none.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    canvas, div, prelude::*, px, size, AnyElement, App, AvailableSpace, Bounds, ContentMask, Context, Element, ElementId, Entity,
    GlobalElementId, Hsla, InspectorElementId, LayoutId, ListState, Pixels, SharedString, Style, Window,
};

use super::rows::activity;
use crate::thread_view::ThreadView;
use crate::transcript::{Glyph as RowGlyph, Row};
use crate::ui::ActivePalette;

/// `--live-activity-focus-width` and one pass of `live-activity-focus`.
pub const BAND: Pixels = px(72.);
pub const PERIOD: Duration = Duration::from_millis(2_200);
/// The band's mask, `(x, alpha)` across its 72 px.
const MASK: [(f32, f32); 7] = [(0., 0.), (10.8, 0.12), (25.2, 0.55), (36., 1.), (46.8, 0.55), (61.2, 0.12), (72., 0.)];
/// The mask is drawn as strips this wide, each at the alpha its middle reads.
const STRIP: f32 = 4.;

/// What the line shows.
#[derive(Clone, PartialEq)]
pub struct Spec {
    pub glyph: RowGlyph,
    pub glyph_ink: Hsla,
    pub verb: Option<String>,
    pub text: SharedString,
    pub mono: bool,
}

/// Where a piece drawn in a row stood when the rows last drew, which the overlay over the cached rows draws at.
pub type Spot = Rc<Cell<Option<Bounds<Pixels>>>>;

pub struct Shimmer {
    spec: Spec,
    start: Instant,
    spot: Spot,
    /// How many times it has drawn, for the tests.
    pub frames: usize,
}

/// The mask's alpha `x` px into the band.
pub fn alpha(x: f32) -> f32 {
    if !(0. ..=72.).contains(&x) {
        return 0.;
    }
    MASK.windows(2)
        .find(|w| x <= w[1].0)
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)
        })
        .unwrap_or(0.)
}

/// The band's left edge `elapsed` into a pass, from one band before the line to one band past its end.
pub fn band_left(width: Pixels, elapsed: Duration) -> Pixels {
    let t = (elapsed.as_secs_f32() % PERIOD.as_secs_f32()) / PERIOD.as_secs_f32();
    -BAND + (width + BAND * 2.) * t
}

impl Shimmer {
    pub fn new(spec: Spec) -> Self {
        Shimmer { spec, start: Instant::now(), spot: Spot::default(), frames: 0 }
    }
}

impl Render for Shimmer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.frames += 1;
        let p = cx.palette().clone();
        let s = &self.spec;
        let size = self.spot.get().map(|b| b.size).unwrap_or_default();
        let width = size.width;
        let still = cx.reduce_motion();
        let strips = if still || width <= px(0.) {
            Vec::new()
        } else {
            let left = band_left(width, self.start.elapsed());
            (0..(72. / STRIP) as usize)
                .filter_map(|i| {
                    let x = left + px(i as f32 * STRIP);
                    let a = alpha(i as f32 * STRIP + STRIP / 2.);
                    (x + px(STRIP) > px(0.) && x < width && a > 0.).then(|| {
                        div().absolute().top_0().bottom_0().left(x).w(px(STRIP)).overflow_hidden().opacity(a).child(
                            div().absolute().top_0().bottom_0().left(-x).w(width).child(activity(
                                s.glyph,
                                p.foreground,
                                s.verb.as_deref(),
                                s.text.clone(),
                                s.mono,
                                true,
                                &p,
                                cx,
                            )),
                        )
                    })
                })
                .collect()
        };
        div()
            .debug_selector(|| "shimmer-band".into())
            .relative()
            .w(width)
            .h(size.height)
            .overflow_hidden()
            .rounded(px(8.))
            .children(strips)
            .child(
                canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        if !still && bounds.intersects(&window.content_mask().bounds) {
                            window.request_animation_frame();
                        }
                    },
                )
                .absolute()
                .inset_0(),
            )
    }
}

/// The overlay is built as the thread view draws, before the rows, so a piece a row adds draws from the next frame.
fn redraw(cx: &mut Context<ThreadView>) {
    let view = cx.entity();
    cx.defer(move |cx| view.update(cx, |_, cx| cx.notify()));
}

/// A canvas filling its parent that keeps where the parent stands in `spot`.
pub fn mark(spot: Spot) -> impl IntoElement {
    canvas(move |bounds, _, _| spot.set(Some(bounds)), |_, _, _, _| {}).absolute().inset_0()
}

/// The row's live line as it stands still, `w-fit`, with the mark the band's overlay stands on.
pub fn line(view: &mut ThreadView, row: &Row, spec: Spec, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    let base = activity(spec.glyph, spec.glyph_ink, spec.verb.as_deref(), spec.text.clone(), spec.mono, false, &p, cx);
    let held = view.shimmers().get(&row.key).cloned();
    let entity = match held {
        Some(entity) => {
            if entity.read(cx).spec != spec {
                entity.update(cx, |s, _| s.spec = spec);
            }
            entity
        }
        None => {
            let entity = cx.new(|_| Shimmer::new(spec));
            view.shimmers().insert(row.key.clone(), entity.clone());
            redraw(cx);
            entity
        }
    };
    let spot = entity.read(cx).spot.clone();
    div()
        .debug_selector(|| "shimmer".into())
        .relative()
        .min_w_0()
        .max_w_full()
        .overflow_hidden()
        .rounded(px(8.))
        .child(base)
        .child(mark(spot))
        .into_any_element()
}

/// The shimmer's band of the line whose mark it reads.
pub fn band(entity: &Entity<Shimmer>, cx: &App) -> (Spot, AnyElement) {
    (entity.read(cx).spot.clone(), entity.clone().into_any_element())
}

/// How a row's clock reads: its text size and line height, and the height of the line it stands in.
#[derive(Clone, Copy)]
pub struct ClockLook {
    pub size: Pixels,
    pub line_height: Option<Pixels>,
    pub height: Pixels,
}

/// The empty slot where a row's clock stands; the clock ticks over the cached rows so a second builds none.
pub fn clock_slot(view: &mut ThreadView, row: &Row, look: ClockLook, cx: &mut Context<ThreadView>) -> AnyElement {
    if !view.clock_spots().contains_key(&row.key) {
        redraw(cx);
    }
    let frame = view.frame();
    let held = view.clock_spots().entry(row.key.clone()).or_insert_with(|| (Spot::default(), look, frame));
    (held.1, held.2) = (look, frame);
    let spot = held.0.clone();
    div().debug_selector(|| "clock-slot".into()).flex_none().relative().w(px(0.)).h(look.height).child(mark(spot)).into_any_element()
}

/// Draws `child` at `spot` while row `row` is on screen; it reads the spot after the rows prepaint, so a scroll keeps up.
pub struct Follow {
    spot: Spot,
    list: ListState,
    row: usize,
    child: Option<AnyElement>,
}

impl Follow {
    pub fn new(spot: Spot, list: ListState, row: usize, child: AnyElement) -> Self {
        Follow { spot, list, row, child: Some(child) }
    }
}

impl IntoElement for Follow {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Follow {
    type RequestLayoutState = ();
    type PrepaintState = Option<(AnyElement, ContentMask<Pixels>)>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let shown = self.list.item_is_above_viewport(self.row) == Some(false) && self.list.item_is_below_viewport(self.row) == Some(false);
        let at = self.spot.get().filter(|_| shown)?;
        let mut child = self.child.take()?;
        let mask = ContentMask { bounds: self.list.viewport_bounds() };
        window.with_content_mask(Some(mask), |window| {
            child.layout_as_root(size(AvailableSpace::MaxContent, AvailableSpace::MaxContent), window, cx);
            child.prepaint_at(at.origin, window, cx);
        });
        Some((child, mask))
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some((child, mask)) = prepaint {
            window.with_content_mask(Some(*mask), |window| child.paint(window, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_crosses_from_one_band_before_the_line_to_one_band_past_it_in_one_pass() {
        assert_eq!(band_left(px(300.), Duration::ZERO), px(-72.));
        assert_eq!(band_left(px(300.), Duration::from_millis(1_100)), px(-72.) + px(222.));
        assert!((band_left(px(300.), Duration::from_millis(2_199)) - px(372.)).abs() < px(1.), "the band ends one band past the line");
        assert_eq!(band_left(px(300.), PERIOD), px(-72.));
    }

    #[test]
    fn the_mask_is_clear_at_its_edges_and_whole_in_its_middle() {
        assert_eq!(alpha(0.), 0.);
        assert_eq!(alpha(36.), 1.);
        assert!((alpha(10.8) - 0.12).abs() < 1e-6);
        assert_eq!(alpha(72.), 0.);
        assert_eq!(alpha(80.), 0.);
    }
}
