// SPDX-License-Identifier: AGPL-3.0-only
//! The bar above the transcript (`WorkspacePageHeader`, `ThreadBreadcrumb`): 52 px, no fill, no border, and the
//! breadcrumb of the thread's project and title.

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, App, FontWeight, Pixels};

use crate::ui::{ActivePalette, Glyph};

/// `--workspace-topbar-height`.
pub const HEIGHT: Pixels = px(52.);

pub fn header(project: &str, title: &str, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let crumb = (!project.is_empty()).then(|| {
        h_flex()
            .min_w_0()
            .gap(px(8.))
            .text_color(p.muted_foreground)
            .child(Icon::new(Glyph::Folder).with_size(px(16.)).flex_none())
            .child(div().min_w_0().truncate().child(project.to_owned()))
    });
    let slash = crumb.is_some().then(|| div().flex_none().text_color(p.muted_foreground.opacity(0.5)).child("/"));
    h_flex().debug_selector(|| "header".into()).flex_none().h(HEIGHT).pl(px(20.)).pr(px(20.)).child(
        h_flex().min_w_0().gap(px(8.)).text_size(px(14.)).line_height(px(20.)).children(crumb).children(slash).child(
            div()
                .flex_none()
                .max_w(gpui_kit::relative(0.7))
                .truncate()
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.foreground)
                .child(title.to_owned()),
        ),
    )
}
