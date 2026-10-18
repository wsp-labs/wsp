// SPDX-License-Identifier: AGPL-3.0-only
//! The head pinned at the top of the code view (pierre's `stickyHeaders`) and the 200 ms fade a file's body comes in
//! with, drawn over gpui-kit's `Diff` from where its head rows were painted this frame.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::base::animation::cubic_bezier;
use gpui_kit::{
    fill, point, px, size, AnyElement, App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Style, Window,
};

use super::code_view::{head_row, Shared, HEAD_HEIGHT};

/// `transition: opacity 200ms ease-out` on `[data-diff]`, from `@starting-style`'s 0.
const FADE: Duration = Duration::from_millis(200);

/// How opaque the background drawn over a body is `elapsed` into its fade: the body's own opacity, eased out, upside
/// down.
pub(super) fn cover(elapsed: Duration) -> f32 {
    let t = (elapsed.as_secs_f32() / FADE.as_secs_f32()).min(1.);
    1. - cubic_bezier(0., 0., 0.58, 1.)(t)
}

/// Where head rows were painted this frame, and what the pinned head showed last frame.
#[derive(Default)]
pub(crate) struct Frame {
    seen: Vec<(usize, Bounds<Pixels>)>,
    pub top: Option<usize>,
    fades: Vec<(usize, Option<Instant>)>,
}

impl Frame {
    pub fn saw(&mut self, ix: usize, bounds: Bounds<Pixels>) {
        self.seen.push((ix, bounds));
    }

    /// New files: `moved` gives each old file's index among them, or `None` for one gone, so the pinned head and the
    /// fades under way stay with their files.
    pub fn carry(&mut self, moved: impl Fn(usize) -> Option<usize>) {
        self.seen.clear();
        self.top = self.top.and_then(&moved);
        self.fades = self.fades.iter().filter_map(|(ix, started)| moved(*ix).map(|ix| (ix, *started))).collect();
    }

    /// A body opened: it fades in from the first frame that draws it.
    pub fn fade(&mut self, ix: usize) {
        self.fades.retain(|(f, _)| *f != ix);
        self.fades.push((ix, None));
    }

    /// A scroll the view made lands in file `ix`, whose head may not be on screen.
    pub fn jumped(&mut self, ix: usize) {
        self.top = Some(ix);
    }

    /// The file whose rows are at the top edge `top`, from the heads painted this frame; with none painted (one file
    /// taller than the view) the file of the list's top row, `listed`.
    fn file_at(&self, top: Pixels, listed: Option<usize>) -> Option<usize> {
        if self.seen.is_empty() {
            return listed.or(self.top);
        }
        match self.seen.iter().rposition(|(_, b)| b.top() <= top) {
            Some(i) => Some(self.seen[i].0),
            None => self.seen[0].0.checked_sub(1),
        }
    }
}

pub(super) struct StickyHead {
    shared: Rc<Shared>,
    now: Instant,
    still: bool,
}

impl StickyHead {
    pub fn new(shared: Rc<Shared>, now: Instant, still: bool) -> Self {
        StickyHead { shared, now, still }
    }
}

pub(super) struct Drawn {
    head: Option<AnyElement>,
    covers: Vec<(Bounds<Pixels>, f32)>,
}

impl IntoElement for StickyHead {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for StickyHead {
    type RequestLayoutState = ();
    type PrepaintState = Drawn;

    fn id(&self) -> Option<ElementId> {
        Some("wsp-diff-sticky-layer".into())
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
        let mut style = Style { position: gpui_kit::Position::Absolute, ..Style::default() };
        style.inset.top = px(0.).into();
        style.inset.left = px(0.).into();
        style.inset.right = px(0.).into();
        style.inset.bottom = px(0.).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> Drawn {
        let listed = self.shared.state.read(cx).top_file();
        let mut frame = self.shared.frame.borrow_mut();
        let mut seen = std::mem::take(&mut frame.seen);
        seen.sort_by(|a, b| a.1.top().partial_cmp(&b.1.top()).unwrap_or(std::cmp::Ordering::Equal));
        seen.dedup_by_key(|(ix, _)| *ix);
        frame.seen = seen;
        let top = bounds.top();
        let file = frame.file_at(top, listed).filter(|ix| *ix < self.shared.heads.len());
        frame.top = file;
        let seen = std::mem::take(&mut frame.seen);
        let below = |ix: usize| seen.iter().find(|(f, b)| *f > ix && b.top() > top).map(|(_, b)| b.top());

        let mut covers = Vec::new();
        if !self.still {
            let now = self.now;
            frame.fades.retain_mut(|(ix, started)| {
                let head = seen.iter().find(|(f, _)| f == ix).map(|(_, b)| b.bottom());
                let from = head.or((file == Some(*ix)).then_some(top));
                let Some(from) = from else { return started.is_none() };
                let start = *started.get_or_insert(now);
                let elapsed = now.duration_since(start);
                let to = below(*ix).unwrap_or(bounds.bottom());
                if to > from {
                    covers.push((Bounds::from_corners(point(bounds.left(), from.max(top)), point(bounds.right(), to)), cover(elapsed)));
                }
                elapsed < FADE
            });
            // A fade waiting for its file to scroll into view needs no frame: the scroll draws one.
            if frame.fades.iter().any(|(_, started)| started.is_some()) {
                window.request_animation_frame();
            }
        } else {
            frame.fades.clear();
        }
        let pinned = file.filter(|ix| seen.iter().find(|(f, _)| f == ix).is_none_or(|(_, b)| b.top() < top));
        drop(frame);

        let head = pinned.map(|ix| {
            let push = below(ix).map_or(px(0.), |next| (next - (top + px(HEAD_HEIGHT))).min(px(0.)));
            let mut head = head_row(&self.shared, ix, true, window, cx);
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                head.layout_as_root(size(AvailableSpace::Definite(bounds.size.width), AvailableSpace::MinContent), window, cx);
                head.prepaint_at(point(bounds.left(), top + push), window, cx);
            });
            head
        });
        Drawn { head, covers }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        drawn: &mut Drawn,
        window: &mut Window,
        cx: &mut App,
    ) {
        let background = self.shared.colors.background;
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            for (cover, alpha) in &drawn.covers {
                window.paint_quad(fill(*cover, background.opacity(*alpha)));
            }
            if let Some(head) = drawn.head.as_mut() {
                head.paint(window, cx);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_fades_in_over_200_ms_easing_out() {
        assert_eq!(cover(Duration::ZERO), 1.);
        assert_eq!(cover(FADE), 0.);
        assert_eq!(cover(Duration::from_millis(400)), 0.);
        let half = cover(Duration::from_millis(100));
        assert!(half < 0.5 && half > 0.1, "ease-out is past halfway at half time: {half}");
    }
}
