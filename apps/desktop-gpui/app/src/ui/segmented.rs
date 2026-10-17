// SPDX-License-Identifier: AGPL-3.0-only
//! `SegmentedControl` (`apps/web/src/components/ui/segmented-control.tsx`): one pick among a few, as segments in an
//! edged track with no fill of its own; the picked segment takes the accent fill and the foreground.

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{prelude::*, px, App, Pixels, SharedString, Window};

use super::icons::Glyph;
use super::text;
use super::theme::ActivePalette;

type Pick = Box<dyn Fn(usize, &mut Window, &mut App) + 'static>;

/// The track's `rounded-lg` and the segment's `rounded-md`, off `--radius`.
const TRACK_RADIUS: Pixels = px(10.);
const SEGMENT_RADIUS: Pixels = px(8.);

#[derive(IntoElement)]
pub struct Segmented {
    id: SharedString,
    segments: Vec<(SharedString, Option<Glyph>)>,
    picked: usize,
    height: Pixels,
    on_pick: Option<Pick>,
}

impl Segmented {
    /// The control at its default `h-8`.
    pub fn new(id: impl Into<SharedString>, picked: usize) -> Self {
        Segmented { id: id.into(), segments: Vec::new(), picked, height: px(32.), on_pick: None }
    }

    pub fn segment(mut self, label: impl Into<SharedString>) -> Self {
        self.segments.push((label.into(), None));
        self
    }

    /// A segment led by its glyph, 8 px before the label.
    pub fn glyph_segment(mut self, glyph: Glyph, label: impl Into<SharedString>) -> Self {
        self.segments.push((label.into(), Some(glyph)));
        self
    }

    /// The track's whole height, its edge and padding included.
    pub fn height(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }

    pub fn on_pick(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Box::new(f));
        self
    }
}

impl RenderOnce for Segmented {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let on_pick = self.on_pick.map(std::rc::Rc::new);
        let (font, line) = (px(14.), text::SETTING_TITLE.1);
        h_flex()
            .id(self.id.clone())
            .debug_selector(|| "segmented".into())
            .flex_none()
            .items_stretch()
            .h(self.height)
            .p(px(2.))
            .gap(px(1.))
            .rounded(TRACK_RADIUS)
            .border_1()
            .border_color(p.border)
            .children(self.segments.into_iter().enumerate().map(|(ix, (label, glyph))| {
                let picked = ix == self.picked;
                let ink = if picked { p.foreground } else { p.muted_foreground };
                h_flex()
                    .id((self.id.clone(), ix))
                    .debug_selector(|| "segment".into())
                    .justify_center()
                    .px(px(14.))
                    .gap(px(8.))
                    .rounded(SEGMENT_RADIUS)
                    .text_size(font)
                    .line_height(line)
                    .text_color(ink)
                    .when(picked, |el| el.bg(p.accent))
                    .when(!picked, |el| el.cursor_pointer().hover(|s| s.text_color(p.foreground)))
                    .children(glyph.map(|g| Icon::new(g).with_size(px(16.))))
                    .child(label)
                    .when_some(on_pick.clone().filter(|_| !picked), |el, f| el.on_click(move |_, window, cx| f(ix, window, cx)))
            }))
    }
}
