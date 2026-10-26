// SPDX-License-Identifier: AGPL-3.0-only
//! What stands around the list: the header with the trigger and the wordmark, the four one-line rows over the list,
//! and the foot with the cloud card and the corner's three buttons (`SidebarChrome.tsx`, `WorkspaceSidebar.tsx`,
//! `CloudCard.tsx`, `SidebarCorner.tsx`).

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, svg, App, ClickEvent, FontWeight, Pixels, SharedString, Window};

use crate::ui::icons::LOCKUP;
use crate::ui::{ActivePalette, Button, Glyph};

/// `--workspace-topbar-height`.
pub const HEADER: Pixels = px(52.);
/// `ONE_LINE_ROW_CLASS`'s `h-9`.
pub const ONE_LINE: Pixels = px(36.);
/// `--sidebar-content-inset`: the list's and the fixed rows' sides.
pub const INSET: Pixels = px(8.);
/// `--control-radius`.
pub const ROW_RADIUS: Pixels = px(8.);

type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The header row: the trigger, then the wordmark 14 px tall and 2 px low, in the quiet ink.
pub fn header(on_trigger: Click, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    h_flex()
        .debug_selector(|| "sidebar-header".into())
        .flex_none()
        .h(HEADER)
        .pl(INSET)
        .gap(px(16.))
        .child(Button::icon("sidebar-trigger", Glyph::PanelLeftClose).on_click(on_trigger))
        .child(svg().path(LOCKUP).flex_none().w(px(51.)).h(px(14.)).mt(px(4.)).text_color(p.muted_foreground))
}

/// One of the fixed rows over the list: a 16 px glyph in the icon ink, its words in the rest ink, and a chevron for a
/// switcher, 36 px tall with 10 px between, 8 px in.
pub fn top_row(
    id: &'static str,
    glyph: Glyph,
    words: impl Into<SharedString>,
    chevron: bool,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    let (icon, rest, hover_ink) = (p.sidebar_icon_color, p.sidebar_row_rest, p.sidebar_foreground);
    h_flex()
        .id(id)
        .debug_selector(move || id.into())
        .flex_none()
        .h(ONE_LINE)
        .gap(px(10.))
        .px(px(8.))
        .rounded(ROW_RADIUS)
        .text_size(px(14.))
        .line_height(px(20.))
        .text_color(rest)
        .cursor_pointer()
        .hover(move |s| s.bg(p.sidebar_row_hover).text_color(hover_ink))
        .active(move |s| s.bg(p.sidebar_row_active))
        .child(Icon::new(glyph).with_size(px(16.)).text_color(icon))
        .child(div().flex_1().min_w_0().truncate().child(words.into()))
        .when(chevron, |el| el.child(Icon::new(Glyph::ChevronDown).with_size(px(16.)).text_color(icon)))
}

/// The cloud card in the foot: a title line with the cloud and the dismiss, the line with the rate, and the one act.
pub fn cloud_card(on_dismiss: Click, on_add: Click, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    v_flex()
        .debug_selector(|| "cloud-card".into())
        .flex_none()
        .gap(px(4.))
        .px(px(8.))
        .pt(px(8.))
        .pb(px(10.))
        .rounded(ROW_RADIUS)
        .relative()
        .bg(p.sidebar_row_hover)
        // `inset-ring`: drawn inside the box, so it takes no room from what the card holds.
        .child(div().absolute().inset_0().rounded(ROW_RADIUS).border_1().border_color(p.sidebar_row_edge))
        .child(
            h_flex()
                .h(px(20.))
                .gap(px(6.))
                .text_size(px(14.))
                .line_height(px(20.))
                .text_color(p.sidebar_foreground)
                .child(Icon::new(Glyph::Cloud).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
                .child(div().flex_1().min_w_0().truncate().child("Run threads on a cloud"))
                .child(
                    div()
                        .id("cloud-card-dismiss")
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(20.))
                        .mr(px(-4.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(p.accent))
                        .on_click(on_dismiss)
                        .child(Icon::new(Glyph::X).with_size(px(12.)).text_color(p.muted_foreground)),
                ),
        )
        .child(div().text_size(px(11.)).line_height(px(14.)).text_color(p.sidebar_muted_foreground).child("Boat from $0.018/h."))
        .child(h_flex().pt(px(6.)).child(Button::outline("cloud-card-add", "Add a cloud provider").xs().on_click(on_add)))
}

/// The corner: Settings, Usage and Activity as 28 px ghost buttons, 4 px apart, 2 px in.
pub fn corner(on_settings: Click, on_usage: Click, on_activity: Click) -> impl IntoElement {
    h_flex()
        .debug_selector(|| "sidebar-corner".into())
        .flex_none()
        .gap(px(4.))
        .px(px(2.))
        .child(Button::icon("corner-settings", Glyph::Settings).on_click(on_settings))
        .child(Button::icon("corner-usage", Glyph::Gauge).on_click(on_usage))
        .child(Button::icon("corner-activity", Glyph::Activity).on_click(on_activity))
}

/// A section's head (`SectionRow.tsx`): its word and count in 12 px, a hairline to the chevron, 28 px tall.
pub fn section_head(id: SharedString, word: &str, count: usize, folded: bool, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    let hover_ink = p.sidebar_foreground;
    h_flex()
        .id(id)
        .h(px(28.))
        .w_full()
        .gap(px(8.))
        .px(px(8.))
        .rounded(ROW_RADIUS)
        .text_size(px(12.))
        .line_height(px(16.))
        .font_weight(FontWeight::NORMAL)
        .text_color(p.sidebar_muted_foreground)
        .font_features(crate::ui::text::tabular())
        .cursor_pointer()
        .hover(move |s| s.text_color(hover_ink))
        .child(SharedString::from(format!("{word} ({count})")))
        // Chromium puts the centred 1 px rule on the lower of the two rows its half pixel falls between.
        .child(v_flex().h(px(2.)).min_w(px(16.)).flex_1().justify_end().child(div().h(px(1.)).bg(p.sidebar_border)))
        // The web turns the down chevron a quarter for a folded section, which is lucide's right chevron.
        .child(Icon::new(if folded { Glyph::ChevronRight } else { Glyph::ChevronDown }).with_size(px(14.)))
}
