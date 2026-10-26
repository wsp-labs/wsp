// SPDX-License-Identifier: AGPL-3.0-only
//! `SubagentFoldRow`: a subagent's fold on the rail, and the address of its page.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, SharedString, StyledText};

use super::super::find;
use super::super::rows::SM;
use super::spawn::go;
use super::{FOOT, META, SM_TIGHT, XS};
use crate::screens::Route;
use crate::thread_view::ThreadView;
use crate::transcript::{self, ThreadAt};
use crate::ui::{tip, Glyph, Palette};

/// `SubagentFoldRow`: the chevron, the title, its state and elapsed, the page arrow, and open, its lines on the rail.
#[allow(clippy::too_many_arguments)]
pub(super) fn subagent(
    view: &ThreadView,
    id: &str,
    title: &str,
    state: Option<&str>,
    open: bool,
    lines: &[transcript::SubLine],
    rail_follows: bool,
    page: Option<String>,
    p: &Palette,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let mono = cx.theme().mono_font_family.clone();
    let toggle = id.to_owned();
    // `Spaced`: the state word and the elapsed apart by a space and `ms-2`, never a dot.
    let parts: Vec<String> = match state {
        Some(state) => match state.split_once(' ') {
            Some((word, rest)) if word.chars().all(char::is_alphabetic) => vec![word.to_owned(), rest.to_owned()],
            _ => vec![state.to_owned()],
        },
        None => Vec::new(),
    };
    let slot = h_flex()
        .flex_none()
        .gap(px(8.))
        .font_family(mono.clone())
        .text_size(META.0)
        .line_height(META.1)
        .text_color(p.muted_foreground)
        .children(parts);
    let glyph = if open { Glyph::ChevronDown } else { Glyph::ChevronRight };
    let trigger = h_flex()
        .id(SharedString::from(format!("subagent-{id}")))
        .debug_selector(|| "subagent-trigger".into())
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .cursor_pointer()
        .child(Icon::new(glyph).with_size(px(12.)).text_color(p.muted_foreground))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(SM_TIGHT.0)
                .line_height(SM_TIGHT.1)
                .text_color(p.foreground.opacity(0.8))
                .child(title.to_owned()),
        )
        .child(slot)
        .on_click(cx.listener(move |this, _, _, cx| this.toggle(&toggle, cx)));
    // `icon-micro` ghost: Open its page, its word on the tooltip; drawn only where the lead's page has an address.
    let page = page.map(|route| {
        h_flex()
            .id(SharedString::from(format!("subagent-open-{id}")))
            .debug_selector(|| "subagent-page".into())
            .flex_none()
            .justify_center()
            .size(px(20.))
            .rounded(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(p.accent))
            .tooltip(tip(OPEN_PAGE))
            .child(Icon::new(Glyph::ArrowUpRight).with_size(px(12.)).text_color(p.muted_foreground))
            .on_click(move |_, _, cx| go(Route::Screen(route.clone().into()), cx))
    });
    let body = open.then(|| {
        v_flex()
            .debug_selector(|| "subagent-body".into())
            .min_w_0()
            .gap(px(4.))
            .ml(px(6.))
            .border_l_1()
            .border_color(p.border.opacity(0.6))
            .pt(px(4.))
            .pl(px(11.))
            .children(lines.iter().enumerate().map(|(i, line)| {
                // Plain text, so find's matches are painted here off the marks it hands each part.
                let marked = |text: &str, detail: bool| marked(view, id, &find::subagent_part(i, detail), text, cx);
                let code = |text: StyledText| {
                    div().font_family(mono.clone()).text_size(XS.0).line_height(XS.1).text_color(p.muted_foreground).child(text)
                };
                let label = (!line.text.is_empty()).then(|| {
                    if line.mono {
                        code(marked(&line.text, false))
                    } else {
                        div().text_size(SM.0).line_height(SM.1).text_color(p.foreground.opacity(0.8)).child(marked(&line.text, false))
                    }
                });
                v_flex().min_w_0().children(label).children(line.detail.as_deref().map(|d| code(marked(d, true))))
            }))
    });
    v_flex()
        .min_w_0()
        .px(px(4.))
        .pb(if rail_follows { px(0.) } else { FOOT })
        .child(h_flex().min_w_0().gap(px(6.)).child(trigger).children(page))
        .children(body)
        .into_any_element()
}

/// A row, its part and the spans find marked on it.
#[cfg(test)]
type Painted = (String, String, Vec<std::ops::Range<usize>>);

#[cfg(test)]
thread_local! {
    /// Each part of a row a card painted find's marks on in this test thread: its row, its part and the marked spans.
    pub static PAINTED: std::cell::RefCell<Vec<Painted>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A part of a row drawn as plain text, with find's matches in it marked while the bar is open.
fn marked(view: &ThreadView, key: &str, part: &str, text: &str, cx: &App) -> StyledText {
    let marks = find::marks(view, key, part, text, cx);
    #[cfg(test)]
    if !marks.is_empty() {
        PAINTED.with(|p| p.borrow_mut().push((key.to_owned(), part.to_owned(), marks.iter().map(|(r, _)| r.clone()).collect())));
    }
    StyledText::new(text.to_owned()).with_highlights(marks)
}

/// The lead thread the view shows, where its workspace is known: what a subagent's page is opened under.
pub(super) fn lead_of(view: &ThreadView) -> Option<ThreadAt> {
    let workspace =
        view.followed_workspace().map(str::to_owned).or_else(|| view.transcript().listed().and_then(|l| l.workspace.clone()))?;
    Some(ThreadAt { workspace, thread: view.thread()?.to_owned() })
}

/// The address of a subagent's page under `lead`, by the call that launched it (`subagent:<workspace>/<thread>/<call>`).
pub fn subagent_route(lead: &ThreadAt, call: &str) -> String {
    format!("{SUBAGENT}:{}/{}/{call}", lead.workspace, lead.thread)
}

pub(super) fn subagent_page(view: &ThreadView, id: &str) -> Option<String> {
    Some(subagent_route(&lead_of(view)?, id.strip_prefix("subagent:")?))
}

const OPEN_PAGE: &str = "Open its page";
/// The name the subagent's page registers under.
pub const SUBAGENT: &str = "subagent";
