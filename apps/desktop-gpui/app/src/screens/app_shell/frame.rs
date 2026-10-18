// SPDX-License-Identifier: AGPL-3.0-only
//! The window's frame as the web app draws it: the 52 px rows that start the sidebar and the main column, the
//! sidebar's toggle and wordmark, the crumbs, the panel toggles, and the widths each column may take.

use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, svg, AnyElement, App, ClickEvent, FontWeight, Hsla, Pixels, SharedString, Window};

use crate::ui::{ActivePalette, Button, Glyph};

/// `--workspace-topbar-height`: every row that starts a column.
pub const TOPBAR: Pixels = px(52.);
/// `--header-frame-inset` in a window drawn without the Mac's lights, `--sidebar-content-inset`.
pub const FRAME_INSET: Pixels = px(8.);
/// The Mac's: the third light's centre at 69 px, then `--header-gap` less half a control.
pub const FRAME_INSET_MAC: Pixels = px(85.);
/// A header row's left padding where it is not the frame, `sm:pl-5`.
pub const HEADER_INSET: Pixels = px(20.);
/// `--header-gap` less half of `--workspace-titlebar-control-size`.
pub const HEADER_GAP: Pixels = px(16.);

/// `sidebarWidth.ts`.
pub const SIDEBAR_DEFAULT: Pixels = px(256.);
pub const SIDEBAR_MIN: Pixels = px(220.);
pub const SIDEBAR_MAX: Pixels = px(480.);
/// `rightPanelLayout.ts`: what the centre column keeps beside the inline panel.
pub const CENTER_MIN: Pixels = px(360.);
/// At this window width and under the web app draws the panel as a sheet over the centre column.
pub const SHEET_MAX_VIEWPORT: Pixels = px(980.);
/// The rail a column is dragged by: 16 px over the sidebar's edge, 8 px over the panel's.
pub const SIDEBAR_RAIL: Pixels = px(16.);
pub const PANEL_RAIL: Pixels = px(8.);

/// `sidebarMaxWidthBeside`: with the panel inline the sidebar gives way so the panel and the centre keep their minimums.
pub fn sidebar_max(viewport: Pixels, panel_inline: bool) -> Pixels {
    if panel_inline {
        SIDEBAR_MAX.min(viewport - super::panel::MIN_WIDTH - CENTER_MIN)
    } else {
        SIDEBAR_MAX
    }
}

/// The kit's clamp: the minimum wins over a maximum below it.
pub fn clamp_sidebar(width: Pixels, viewport: Pixels, panel_inline: bool) -> Pixels {
    width.min(sidebar_max(viewport, panel_inline)).max(SIDEBAR_MIN)
}

/// `getPreviewPanelMaxWidth`: 70% of the window, and never so wide the centre column drops under its minimum.
pub fn panel_max(viewport: Pixels, row: Pixels) -> Pixels {
    let fraction = (viewport * 0.7).floor();
    super::panel::MIN_WIDTH.max(fraction.min(row.floor() - CENTER_MIN))
}

pub fn clamp_panel(width: Pixels, viewport: Pixels, row: Pixels) -> Pixels {
    width.min(panel_max(viewport, row)).max(super::panel::MIN_WIDTH)
}

/// The frame row's left inset: the Mac keeps the traffic lights' room.
pub fn frame_inset() -> Pixels {
    if cfg!(target_os = "macos") {
        FRAME_INSET_MAC
    } else {
        FRAME_INSET
    }
}

/// `SidebarTrigger`: the 28 px ghost button, its glyph saying which way the sidebar goes.
pub fn sidebar_trigger(id: &'static str, open: bool, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Button {
    Button::icon(id, if open { Glyph::PanelLeftClose } else { Glyph::PanelLeft }).on_click(f)
}

/// The muted ink inside the sidebar, `--sidebar-quiet`: every theme sheet sets it to the muted foreground on a light
/// theme and to the sidebar's own muted foreground on a dark one.
pub fn sidebar_quiet(cx: &App) -> Hsla {
    let p = cx.palette();
    if cx.theme().is_dark() {
        p.sidebar_muted_foreground
    } else {
        p.muted_foreground
    }
}

/// `Lockup`: the tilde beside the wordmark, 14 px tall, set 2 px low so its optical centre sits on the row's.
pub fn lockup(ink: Hsla) -> impl IntoElement {
    svg().path(super::assets::LOCKUP).flex_none().w(px(51.)).h(px(14.)).mt(px(4.)).text_color(ink)
}

/// A row that starts a column: 52 px, its children 16 px apart.
pub fn row(id: &'static str) -> gpui_kit::Stateful<gpui_kit::Div> {
    h_flex().id(id).debug_selector(move || id.into()).flex_none().h(TOPBAR).min_h(TOPBAR).min_w_0().items_center().gap(HEADER_GAP)
}

/// The crumbs of the header: the project, a slash, the thread; or the line for no thread. The line takes what it holds
/// and gives way to the row; beside its project the thread takes at most 70% of the line, as `max-w-[70%]` does in
/// the web app, so a long title stops at 70% of the room the row leaves it.
pub fn crumbs(project: Option<&str>, title: Option<&str>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let line =
        h_flex().id("crumbs").debug_selector(|| "crumbs".into()).min_w_0().items_center().gap_2().text_size(px(14.)).line_height(px(20.));
    let Some(title) = title else {
        return line.child(div().truncate().text_color(p.muted_foreground).child("No thread selected")).into_any_element();
    };
    let slash = div().flex_none().text_color(p.muted_foreground.opacity(0.5)).child("/");
    let project = project.filter(|p| !p.is_empty()).map(|name| {
        h_flex()
            .min_w_0()
            .items_center()
            .gap_2()
            .text_color(p.muted_foreground)
            .child(Icon::new(IconName::Folder).with_size(px(16.)).flex_none())
            .child(div().truncate().child(SharedString::from(name.to_owned())))
    });
    let own = project.is_some();
    line.children(project)
        .when(own, |el| el.child(slash))
        .child(
            div()
                .debug_selector(|| "crumbs-title".into())
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.foreground)
                .when(own, |el| el.flex_none().max_w(gpui_kit::relative(0.7)))
                .child(SharedString::from(title.to_owned())),
        )
        .into_any_element()
}

/// `PanelLayoutControls`: the terminal drawer's toggle and the right panel's, 28 px ghost toggles 4 px apart.
pub fn layout_controls(
    drawer_open: bool,
    panel_open: bool,
    on_drawer: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_panel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> impl IntoElement {
    h_flex()
        .flex_none()
        .h_full()
        .items_center()
        .gap_1()
        .child(
            toggle("toggle-drawer", IconName::PanelBottom, drawer_open, on_drawer, cx)
                .tooltip(crate::ui::tip(crate::screens::terminal::toolbar::toggle_label(cfg!(target_os = "macos")))),
        )
        .child(toggle("toggle-panel", IconName::PanelRight, panel_open, on_panel, cx))
}

/// `Toggle` ghost at `sm`: no edge, the foreground glyph at 80%, the accent fill once pressed or under the pointer.
fn toggle(
    id: &'static str,
    glyph: IconName,
    pressed: bool,
    f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    div()
        .id(id)
        .flex()
        .flex_none()
        .size(px(28.))
        .items_center()
        .justify_center()
        .rounded(px(8.))
        .cursor_pointer()
        .when(pressed, |el| el.bg(p.accent))
        .hover(|s| s.bg(p.accent))
        .on_click(f)
        .child(Icon::new(glyph).with_size(px(16.)).text_color(p.foreground.opacity(0.8)))
}

/// The web app's `surface-grain`, its noise tile at 3.5%, taken at its mean: on every ground the tile lands as this grey
/// at 2.06% (measured off Chromium's Paper and Graphite surfaces, 252 to 250.8 and 10 to 13.8). The tile's own texture,
/// under one level either way, is left out: drawn as an image it held a polychrome atlas page, 12 MB at rest.
pub fn grain() -> impl IntoElement {
    div().absolute().top_0().left_0().size_full().bg(gpui_kit::hsla(0., 0., 194. / 255., 0.0206))
}
