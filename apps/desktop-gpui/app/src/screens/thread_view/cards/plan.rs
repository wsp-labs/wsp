// SPDX-License-Identifier: AGPL-3.0-only
//! `ProposedPlanCard`: the plan, its title and menu, its fold, and its download as markdown.

use std::path::PathBuf;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, linear_color_stop, linear_gradient, point, prelude::*, px, Anchor, AnyElement, App, BoxShadow, ClipboardItem,
    Context, FontWeight, Pixels, SharedString, Window,
};

use super::super::markdown;
use super::{mix_over, SM_TIGHT};
use crate::thread_view::ThreadView;
use crate::transcript::{self, Row, RowKind};
use crate::ui::{Button, Glyph, Palette};

/// A plan folds behind Expand plan at `max-h-104`, its last `h-24` faded into the card.
const PLAN_FOLDED: Pixels = px(416.);
const PLAN_FADE: Pixels = px(96.);
/// The text a plan row's parsed document is built from: the collapsed preview while shut, the plan while open.
pub fn plan_text(row: &Row) -> String {
    match row.kind {
        RowKind::Plan { open, .. } => transcript::plan_shown(&row.text, open),
        _ => row.text.clone(),
    }
}

/// The outline keycap at `icon-xs`, 24 px square: the plan's menu button.
fn outline_icon(id: SharedString, g: Glyph, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let bevel =
        vec![BoxShadow { color: p.keycap_top, offset: point(px(0.), px(1.)), blur_radius: px(0.), spread_radius: px(0.), inset: true }];
    h_flex()
        .id(id)
        .debug_selector(|| "menu-button".into())
        .flex_none()
        .justify_center()
        .size(px(24.))
        .rounded(px(8.))
        .border_1()
        .border_color(p.input)
        .bg(p.input_fill)
        .cursor_pointer()
        .hover(|s| s.bg(p.input_fill_hover))
        .child(div().absolute().inset_0().rounded(px(7.)).shadow(bevel))
        .child(Icon::new(g).with_size(px(16.)).text_color(p.muted_foreground))
}

/// `buildProposedPlanMarkdownFilename`: the title's words joined by hyphens, `plan.md` without one.
pub fn plan_filename(title: Option<&str>) -> String {
    let mut name = String::new();
    for c in title.unwrap_or("plan").to_lowercase().chars().filter(|c| !"`'\".,!?()[]{}".contains(*c)) {
        if c.is_ascii_alphanumeric() {
            name.push(c);
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let name = name.trim_matches('-');
    format!("{}.md", if name.is_empty() { "plan" } else { name })
}

/// Download as markdown: the window's own save dialog, opened on the Downloads folder under the plan's name as a
/// browser's download prompt is, then the plan written where the person says.
fn download(markdown: &str, cx: &mut App) {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let folder = Some(home.join("Downloads")).filter(|d| d.is_dir()).unwrap_or(home);
    let asked = cx.prompt_for_new_path(&folder, Some(&plan_filename(plan_heading(markdown).as_deref())));
    let text = format!("{}\n", markdown.trim_end());
    cx.background_spawn(async move {
        let Ok(Ok(Some(path))) = asked.await else { return };
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("desktop-gpui: downloading the plan to {}: {e}", path.display());
        }
    })
    .detach();
}

/// `proposedPlanTitle`: the first heading's words, as the file is named, which the row's title falls back from.
fn plan_heading(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let t = line.trim_start();
        let lead = line.len() - t.len();
        let hashes = t.chars().take_while(|c| *c == '#').count();
        let words = t[hashes..].trim();
        (lead <= 3 && (1..=6).contains(&hashes) && t[hashes..].starts_with(char::is_whitespace) && !words.is_empty())
            .then(|| words.to_owned())
    })
}

/// `ProposedPlanCard`: the card, its title and menu, the plan, and past 900 characters or 20 lines its fold.
#[allow(clippy::too_many_arguments)]
pub(super) fn plan(
    view: &mut ThreadView,
    row: &Row,
    title: &str,
    open: bool,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let folds = transcript::plan_folds(&row.text);
    let body = view.body(row, cx);
    let cwd = view.transcript().cwd().map(str::to_owned);
    let menu = window.use_keyed_state(SharedString::from(format!("plan-menu-{}", row.key)), cx, |_, _| false);
    let menu_open = *menu.read(cx);
    let toggle = format!("plan:{}", row.key);
    let opener = menu.clone();
    let trigger = outline_icon(SharedString::from(format!("plan-actions-{}", row.key)), Glyph::Ellipsis, p).on_click(move |_, _, cx| {
        opener.update(cx, |open, cx| {
            *open = !*open;
            cx.notify();
        })
    });
    let popup = menu_open.then(|| {
        let item = |id: &'static str, label: &'static str, act: Box<dyn Fn(&mut App)>| {
            let menu = menu.clone();
            div()
                .id(id)
                .debug_selector(move || id.into())
                .flex()
                .items_center()
                .min_h(px(28.))
                .px(px(8.))
                .py(px(4.))
                .rounded(px(8.))
                .text_size(SM_TIGHT.0)
                .line_height(SM_TIGHT.1)
                .text_color(p.foreground)
                .cursor_pointer()
                .hover(|s| s.bg(p.accent))
                .child(label)
                .on_click(move |_, _, cx| {
                    act(cx);
                    menu.update(cx, |open, cx| {
                        *open = false;
                        cx.notify();
                    });
                })
        };
        let (copied, downloaded) = (format!("{}\n", row.text.trim_end()), row.text.clone());
        let closer = menu.clone();
        div().absolute().right_0().top(px(28.)).child(
            deferred(
                anchored().anchor(Anchor::TopRight).snap_to_window().child(
                    v_flex()
                        .id(SharedString::from(format!("plan-menu-{}", row.key)))
                        .debug_selector(|| "plan-menu".into())
                        .occlude()
                        .min_w(px(128.))
                        .p(px(4.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(p.foreground.opacity(0.1))
                        .bg(p.popover)
                        .shadow(p.popover_shadow.clone())
                        .child(item(
                            "plan-copy",
                            "Copy to clipboard",
                            Box::new(move |cx| cx.write_to_clipboard(ClipboardItem::new_string(copied.clone()))),
                        ))
                        .child(item("plan-download", "Download as markdown", Box::new(move |cx| download(&downloaded, cx))))
                        .on_mouse_down_out(move |_, _, cx| {
                            closer.update(cx, |open, cx| {
                                *open = false;
                                cx.notify();
                            })
                        }),
                ),
            )
            .with_priority(1),
        )
    });
    let folded = folds && !open;
    let (mid, fade) = (p.card.opacity(0.8), p.card.opacity(0.95));
    let shown = div()
        .debug_selector(|| "plan-body".into())
        .relative()
        .when(folded, |el| el.max_h(PLAN_FOLDED).overflow_hidden())
        .child(markdown::reply(&body, false, None, cwd.as_deref(), Some(mix_over(p.card, 0.7, p.background)), cx))
        .when(folded, |el| {
            el.child(
                v_flex()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(PLAN_FADE)
                    .child(div().h(PLAN_FADE / 2.).bg(linear_gradient(
                        180.,
                        linear_color_stop(p.card.opacity(0.), 0.),
                        linear_color_stop(mid, 1.),
                    )))
                    .child(div().h(PLAN_FADE / 2.).bg(linear_gradient(180., linear_color_stop(mid, 0.), linear_color_stop(fade, 1.)))),
            )
        });
    let expand = folds.then(|| {
        let toggle = toggle.clone();
        let label = if open { "Collapse plan" } else { "Expand plan" };
        h_flex().mt(px(16.)).justify_center().child(
            Button::outline(SharedString::from(format!("plan-fold-{}", row.key)), label)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&toggle, cx))),
        )
    });
    div()
        .min_w_0()
        .px(px(4.))
        .py(px(2.))
        .child(
            v_flex()
                .debug_selector(|| "plan".into())
                .rounded(px(14.))
                .border_1()
                .border_color(p.border.opacity(0.8))
                .bg(p.card.opacity(0.7))
                .p(px(20.))
                .child(
                    h_flex()
                        .flex_wrap()
                        .justify_between()
                        .gap(px(12.))
                        .child(
                            div()
                                .debug_selector(|| "plan-title".into())
                                .min_w_0()
                                .truncate()
                                .text_size(SM_TIGHT.0)
                                .line_height(SM_TIGHT.1)
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .child(title.to_owned()),
                        )
                        .child(div().relative().child(trigger).children(popup)),
                )
                .child(div().mt(px(16.)).child(shown).children(expand)),
        )
        .into_any_element()
}
