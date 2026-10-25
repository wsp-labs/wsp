// SPDX-License-Identifier: AGPL-3.0-only
//! The one row a folder is named in (`files/FolderBreadcrumbs.tsx` in `files/CrumbScroller.tsx`): the root the folder
//! sits in first, then one crumb per folder below it, the last being the folder shown. Every other crumb goes to its
//! folder, each names its whole path on hover, and over two roots a chevron beside the first picks between them. The
//! row scrolls sideways with no bar and fades the edge it cuts.

use std::rc::Rc;

use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, linear_color_stop, linear_gradient, prelude::*, px, Anchor, App, ElementId, FontWeight, Pixels, ScrollHandle, SharedString, Window,
};

use super::paths::crumbs;
use crate::ui::{menu_on, motion, tip, ActivePalette as _, Glyph, MenuAct, Run};

/// `max-w-40`: a crumb's widest.
const CRUMB_MAX: Pixels = px(160.);
/// `text-[11px]`: the crumb's own size, under the file tab's `text-xs` on the row.
const CRUMB_TEXT: Pixels = px(11.);
/// The fade at a cut edge of the row.
const FADE: Pixels = px(24.);

pub type Pick = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The row over `roots` naming `folder`, 11 px mono over the tree and over a file alike.
#[derive(IntoElement)]
pub struct CrumbRow {
    id: SharedString,
    roots: Vec<String>,
    folder: String,
    on_pick: Pick,
}

impl CrumbRow {
    pub fn new(id: impl Into<SharedString>, roots: Vec<String>, folder: impl Into<String>, on_pick: Pick) -> Self {
        CrumbRow { id: id.into(), roots, folder: folder.into(), on_pick }
    }
}

impl RenderOnce for CrumbRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let scroll = window.use_keyed_state(ElementId::from((ElementId::from(self.id.clone()), "scroll")), cx, |_, _| ScrollHandle::new());
        let scroll = scroll.read(cx).clone();
        let list = crumbs(&self.roots, &self.folder);
        let two_roots = self.roots.len() > 1;
        let mut row = h_flex().id(self.id.clone()).debug_selector(|| "folder-crumbs".into()).h_full().items_center().flex_none();
        for (ix, crumb) in list.iter().enumerate() {
            let current = crumb.path == self.folder;
            if ix > 0 {
                row = row.child(
                    Icon::new(Glyph::ChevronRight).with_size(px(14.)).flex_none().mx_1().text_color(p.muted_foreground.opacity(0.6)),
                );
            }
            let crumb_id = SharedString::from(format!("{}-crumb-{ix}", self.id));
            let hovered = motion::hovered(crumb_id.clone(), window, cx);
            let over = *hovered.read(cx) && (!current || ix == 0);
            let (fill, ink) = match (over, current) {
                (true, _) => (p.accent, p.foreground),
                (false, true) => (p.accent.opacity(0.), p.foreground),
                (false, false) => (p.accent.opacity(0.), p.muted_foreground),
            };
            let fill = motion::ease_ink(crumb_id.clone(), "fill", fill, motion::COLORS, window, cx);
            let ink = motion::ease_ink(crumb_id.clone(), "ink", ink, motion::COLORS, window, cx);
            let path = crumb.path.clone();
            let mut el = div()
                .id(crumb_id.clone())
                .debug_selector({
                    let path = path.clone();
                    move || format!("crumb-{path}")
                })
                .flex_none()
                .max_w(CRUMB_MAX)
                .truncate()
                .rounded(px(6.))
                .px(px(2.))
                .font_family(mono.clone())
                .text_size(CRUMB_TEXT)
                .line_height(CRUMB_TEXT + px(5.))
                .text_color(ink)
                .bg(fill)
                .when(current, |el| el.font_weight(FontWeight::MEDIUM))
                .tooltip(tip(path.clone()))
                .on_hover(motion::track_hover(hovered))
                .child(SharedString::from(crumb.name.clone()));
            // The folder shown is text; the first crumb always goes to its root.
            if !current || ix == 0 {
                let pick = self.on_pick.clone();
                el = el.cursor_pointer().on_click(move |_, window, cx| pick(&path, window, cx));
            }
            row = row.child(el);
            if ix == 0 && two_roots {
                row = row.child(self.root_menu(&crumb.path, &p));
            }
        }
        let offset = scroll.offset().x;
        let max = scroll.max_offset().x;
        let fade =
            |angle: f32| linear_gradient(angle, linear_color_stop(p.background, 0.), linear_color_stop(p.background.opacity(0.), 1.));
        div()
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(
                // A flex box, so the row keeps its whole width (`w-max`) and the scroll knows how far it runs.
                div()
                    .id(SharedString::from(format!("{}-scroller", self.id)))
                    .flex()
                    .size_full()
                    .overflow_x_scroll()
                    .track_scroll(&scroll)
                    .child(row),
            )
            .when(offset < px(0.), |el| {
                el.child(div().debug_selector(|| "crumbs-fade-start".into()).absolute().left_0().top_0().bottom_0().w(FADE).bg(fade(90.)))
            })
            .when(max > px(0.) && -offset < max, |el| {
                el.child(div().debug_selector(|| "crumbs-fade-end".into()).absolute().right_0().top_0().bottom_0().w(FADE).bg(fade(270.)))
            })
    }
}

impl CrumbRow {
    /// The chevron beside the first crumb, the only control that switches root: the roots, the shown one ticked.
    fn root_menu(&self, current: &str, p: &crate::ui::Palette) -> impl IntoElement {
        let acts = self
            .roots
            .iter()
            .map(|root| {
                let (pick, root_at) = (self.on_pick.clone(), root.clone());
                let run: Run = Rc::new(move |window, cx| pick(&root_at, window, cx));
                MenuAct {
                    group: 0,
                    glyph: (root == current).then_some(Glyph::Check),
                    title: root.clone().into(),
                    destructive: false,
                    held: false,
                    why: None,
                    keys: None,
                    run,
                }
            })
            .collect();
        let trigger = div()
            .id(SharedString::from(format!("{}-roots-button", self.id)))
            .debug_selector(|| "pick-root".into())
            .flex_none()
            .p_1()
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(p.muted_foreground)
            .hover(|s| s.bg(p.accent))
            .tooltip(tip(super::words::PICK_ROOT))
            .child(Icon::new(Glyph::ChevronDown).with_size(px(14.)));
        menu_on(format!("{}-roots", self.id), trigger, Anchor::TopLeft, Rc::new(acts))
    }
}
