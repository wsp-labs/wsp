// SPDX-License-Identifier: AGPL-3.0-only
//! A markdown file rendered (`components/files/FileMarkdownPreview.tsx`) on gpui-kit's markdown view: an image stays as
//! its alt text in a chip, since a relative image path has no road into the workspace and an absolute one would leave
//! it, and only an http or https link opens; any other is plain primary text.

use gpui_kit::component::text::{markdown_ast::Node, MarkdownNode, MarkdownParseContext, MarkdownPlugin, TextView, TextViewState};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{div, prelude::*, px, App, Entity, SharedString, Window};

use crate::ui::ActivePalette as _;

/// Whether a link may open: http and https only.
pub fn opens(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http:") || lower.starts_with("https:")
}

/// An image's chip: its alt text, else its source, else "image".
pub fn image_word(alt: &str, src: &str) -> String {
    if !alt.is_empty() {
        alt.to_owned()
    } else if !src.is_empty() {
        src.to_owned()
    } else {
        "image".to_owned()
    }
}

/// The text of a link's children, for a link that does not open.
fn plain(nodes: &[Node]) -> String {
    nodes.iter().map(|n| n.to_string()).collect()
}

struct ImageChip;

impl MarkdownPlugin for ImageChip {
    fn name(&self) -> &str {
        "file-image"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Image(image) = node else { return None };
        Some(MarkdownNode::new("file-image", ()).text(image_word(&image.alt, &image.url)))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = cx.palette();
        div()
            .debug_selector(|| "markdown-image".into())
            .rounded(px(6.))
            .border_1()
            .border_color(p.border.opacity(0.7))
            .bg(p.muted)
            .px(px(6.))
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(11.))
            .line_height(px(16.))
            .text_color(p.muted_foreground)
            .child(SharedString::from(node.as_text().to_owned()))
    }
}

struct DeadLink;

impl MarkdownPlugin for DeadLink {
    fn name(&self) -> &str {
        "file-dead-link"
    }

    fn parse(&self, node: &Node, _: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let Node::Link(link) = node else { return None };
        (!opens(&link.url)).then(|| MarkdownNode::new("file-dead-link", ()).text(plain(&link.children)))
    }

    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .debug_selector(|| "markdown-dead-link".into())
            .text_color(cx.palette().primary)
            .child(SharedString::from(node.as_text().to_owned()))
    }
}

/// The rendered file, `max-w-4xl` centered, `px-6 py-5`, 14 px at a 24 px line.
pub fn preview(state: &Entity<TextViewState>, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    div().w_full().flex().justify_center().child(
        div()
            .debug_selector(|| "markdown-preview".into())
            .w_full()
            .max_w(px(896.))
            .px_6()
            .py_5()
            .text_size(px(14.))
            .line_height(px(24.))
            .text_color(p.foreground)
            .child(TextView::new(state).selectable(true).plugin(ImageChip).plugin(DeadLink).on_link_click(|url, _, _, cx| {
                if opens(url) {
                    cx.open_url(url);
                }
            })),
    )
}
