// SPDX-License-Identifier: AGPL-3.0-only
use gpui::{
    AnyElement, App, Bounds, Context, Corners, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Render, Styled, Window, div,
    px, rgb, rgba,
};
use gpui_component::sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem};

pub struct Spike;

impl Render for Spike {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .bg(rgb(0x0e0f12))
            .text_color(rgb(0xe6e6e6))
            .text_size(px(13.))
            .child(
                Sidebar::new("threads")
                    .w(px(260.))
                    .child(
                        SidebarGroup::new("Threads").child(SidebarMenu::new().children(
                            (1..=30).map(|i| {
                                SidebarMenuItem::new(format!(
                                    "Thread {i:02}  fix the sidebar row {i}"
                                ))
                                .active(i == 3)
                            }),
                        )),
                    ),
            )
            .child(content())
    }
}

fn content() -> impl IntoElement {
    let palette = [0xef4444, 0xf59e0b, 0x22c55e, 0x06b6d4, 0x3b82f6, 0xa855f7];
    div()
        .relative()
        .flex_1()
        .h_full()
        .overflow_hidden()
        .child(div().absolute().inset_0().flex().children(
            (0..24).map(move |i| div().flex_1().h_full().bg(rgb(palette[i % palette.len()]))),
        ))
        .child(
            div()
                .absolute()
                .inset_0()
                .p_6()
                .flex()
                .flex_col()
                .gap_2()
                .text_size(px(22.))
                .text_color(rgb(0x000000))
                .children((0..14).map(|i| format!("Content line {i} sits behind the glass panel"))),
        )
        .child(
            div()
                .absolute()
                .left(px(120.))
                .top(px(140.))
                .w(px(520.))
                .h(px(300.))
                .child(glass_panel()),
        )
}

fn glass_panel() -> impl IntoElement {
    Glass {
        radius: px(24.),
        blur: px(24.),
        child: div()
            .size_full()
            .rounded(px(24.))
            .bg(rgba(0xffffff26))
            .border_1()
            .border_color(rgba(0xffffff66))
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(56.))
                    .flex_none()
                    .bg(rgb(0xff6b35))
                    .px_5()
                    .flex()
                    .items_center()
                    .text_color(rgb(0x000000))
                    .child("Header strip: its corners should follow the panel's radius"),
            )
            .child(
                div()
                    .p_5()
                    .text_size(px(16.))
                    .text_color(rgb(0xffffff))
                    .child("Backdrop blur 24px over the content area"),
            )
            .into_any_element(),
    }
}

/// Blurs what is painted beneath its child and clips the child to rounded corners.
struct Glass {
    radius: Pixels,
    blur: Pixels,
    child: AnyElement,
}

impl IntoElement for Glass {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for Glass {
    type RequestLayoutState = ();
    type PrepaintState = ();

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
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        window.paint_layer(bounds, |window| {
            window.paint_backdrop_blur(bounds, Corners::all(self.radius), self.blur);
            window.with_rounded_clip(bounds, self.radius, |window| self.child.paint(window, cx));
        });
    }
}
