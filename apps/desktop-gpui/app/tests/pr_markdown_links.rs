// SPDX-License-Identifier: AGPL-3.0-only
//! Markdown someone else wrote. A body in the Pull request pane: a press on its link reaches the system's opener only
//! for a web page, as the desktop app's window allows (`apps/desktop/src/main.ts`). A file in the Files pane: its raw
//! HTML stays as words, so an `<img>` there is never decoded.

use desktop_gpui::screens::pr_pane::files::markdown as file_markdown;
use desktop_gpui::screens::pr_pane::pull_request::{conversation, parts};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::text::TextViewState;
use gpui_kit::{div, point, prelude::*, px, Context, Entity, Modifiers, MouseButton, TestAppContext, Window};

struct Body(Entity<TextViewState>);

impl Render for Body {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(400.)).child(conversation::markdown("body", &self.0, cx))
    }
}

/// What the opener was handed after a press of `button` on the body's first word.
fn press_with(button: MouseButton, text: &str, cx: &mut TestAppContext) -> Option<String> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let shown = parts::restricted(text);
    let (_, cx) = cx.add_window_view(|_, cx| Body(cx.new(|cx| TextViewState::markdown(&shown, cx))));
    cx.run_until_parked();
    let at = point(px(10.), px(10.));
    cx.simulate_mouse_down(at, button, Modifiers::default());
    cx.simulate_mouse_up(at, button, Modifiers::default());
    cx.opened_url()
}

fn press(text: &str, cx: &mut TestAppContext) -> Option<String> {
    press_with(MouseButton::Left, text, cx)
}

#[gpui_kit::test]
fn a_web_page_opens(cx: &mut TestAppContext) {
    assert_eq!(press("[open](https://example.com/a)", cx).as_deref(), Some("https://example.com/a"));
}

#[gpui_kit::test]
fn a_file_address_does_not_open(cx: &mut TestAppContext) {
    assert_eq!(press("[open](file:///etc/hosts)", cx), None);
}

#[gpui_kit::test]
fn an_app_scheme_does_not_open(cx: &mut TestAppContext) {
    assert_eq!(press("[open](vscode://file/etc/hosts)", cx), None);
}

#[gpui_kit::test]
fn a_right_press_on_a_web_page_does_not_open(cx: &mut TestAppContext) {
    assert_eq!(press_with(MouseButton::Right, "[open](https://example.com/a)", cx), None);
}

#[gpui_kit::test]
fn a_middle_press_on_a_web_page_opens(cx: &mut TestAppContext) {
    assert_eq!(press_with(MouseButton::Middle, "[open](https://example.com/a)", cx).as_deref(), Some("https://example.com/a"));
}

struct File(Entity<TextViewState>);

impl Render for File {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().w(px(600.)).child(file_markdown::preview(&self.0, cx))
    }
}

#[gpui_kit::test]
fn raw_html_in_a_file_stays_as_its_words(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let text = "see <img src=\"data:image/png;base64,AA\"> here\n\n<img src=\"logo.png\">\n";
    let (_, cx) = cx.add_window_view(|_, cx| File(cx.new(|cx| TextViewState::markdown(text, cx))));
    cx.run_until_parked();
    assert!(cx.debug_bounds("markdown-raw-html").is_some(), "an image tag is drawn as its words, not decoded");
}
