// SPDX-License-Identifier: AGPL-3.0-only
//! `TreeRows`: the Threads section under the transcript, its head, its counts and its levels.

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Context, FontWeight, Hsla, Pixels, SharedString, Window};

use super::tree_row::{tree_row, Sending};
use super::{SM_TIGHT, XS};
use crate::thread_view::ThreadView;
use crate::transcript::{self, ChildPart, Listed};
use crate::ui::{tip, Button, Glyph, Palette};

/// How many rows an open fold of the Threads section draws at a time (`PAGE` in TreeRows.tsx).
const TREE_PAGE: usize = 20;
/// `TRANSCRIPT_LIST_CLASS`'s `ml-3.5`: a child list's line under the centre of the 13 px mark 8 px in.
const TREE_LIST: Pixels = px(14.);
/// `ELBOW`'s `rounded-bl-sm`.
const TREE_ELBOW: Pixels = px(6.);
#[cfg(test)]
thread_local! {
    /// Every act the Threads section and the strip handed the sidebar's runner or the thread in this test thread.
    pub static RAN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Runs an act on the window's sidebar, whose runner says a refusal over its list.
pub(super) fn on_sidebar(
    said: String,
    cx: &mut App,
    act: impl FnOnce(&mut crate::sidebar::Sidebar, &mut Context<crate::sidebar::Sidebar>),
) {
    #[cfg(test)]
    RAN.with(|r| r.borrow_mut().push(said.clone()));
    let _ = said;
    if let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) {
        sidebar.update(cx, act);
    }
}

/// `stopSubagent`: interrupts one subagent's task in its lead's session.
pub(super) fn stop_task(session: &str, task: &str, cx: &mut App) {
    let params = serde_json::json!({ "sessionId": session, "task": task });
    on_sidebar(format!("stop {session} {task}"), cx, |s, cx| s.write_op::<wsp_host_client::protocol::SessionsInterruptRequest>(params, cx));
}

pub(super) fn stop_child(session: &str, cx: &mut App) {
    let params = serde_json::json!({ "sessionId": session });
    on_sidebar(format!("stop {session}"), cx, |s, cx| s.write_op::<wsp_host_client::protocol::SessionsInterruptRequest>(params, cx));
}

pub(super) fn settle_children(ids: Vec<String>, cx: &mut App) {
    on_sidebar(format!("settle {}", ids.join(" ")), cx, |s, cx| s.settle_ids(ids, crate::sidebar::Processes::Ask, cx));
}

pub(super) fn restore_child(id: String, cx: &mut App) {
    on_sidebar(format!("restore {id}"), cx, |s, cx| s.restore_ids(vec![id], cx));
}

/// Shuts or opens the section under `lead`: at once on the view, and in the preferences through the sidebar, whose
/// next read agrees.
fn shut_threads(view: &gpui_kit::WeakEntity<ThreadView>, lead: &str, shut: bool, cx: &mut App) {
    let lead = lead.to_owned();
    _ = view.update(cx, |v, cx| {
        if let Some(listing) = v.listing().filter(|l| l.thread == lead).cloned() {
            v.set_listing(transcript::Listing { shut, ..listing }, cx);
        }
    });
    on_sidebar(format!("shut {lead} {shut}"), cx, |s, cx| s.shut_threads(&lead, shut, cx));
}

/// One count of the shut head as the status slot draws its kind: the glyph, then the figure, in the kind's ink and
/// weight, its words on the hover (`Glyphed` in TreeCounts.tsx).
fn tree_count(
    id: &'static str,
    glyph: Option<Glyph>,
    n: usize,
    words: String,
    ink: Hsla,
    toned: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    h_flex()
        .id(id)
        .debug_selector(move || format!("tree-count-{id}"))
        .flex_none()
        .gap(px(4.))
        .text_color(ink)
        .when(toned, |el| el.font_weight(FontWeight::MEDIUM))
        .tooltip(tip(words))
        .children(glyph.map(|g| Icon::new(g).with_size(px(12.)).text_color(ink)))
        .child(n.to_string())
}

/// `TreeCounts`: each live status's figure, working's crab after its number, and with nothing live how many finished.
fn tree_counts(counts: transcript::TreeCounts, p: &Palette) -> AnyElement {
    let many = |n: usize, one: &str| format!("{n} {one}{}", if n == 1 { "" } else { "s" });
    let live = counts.needs_you + counts.failed + counts.working + counts.waiting > 0;
    let threads = counts.working - counts.working_subagents;
    let working = [
        (threads > 0).then(|| many(threads, "thread")),
        (counts.working_subagents > 0).then(|| many(counts.working_subagents, "subagent")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" and ");
    h_flex()
        .debug_selector(|| "tree-counts".into())
        .flex_none()
        .gap(px(12.))
        .when(counts.needs_you > 0, |el| {
            el.child(tree_count(
                "needs-you",
                Some(Glyph::MessageCircleQuestionMark),
                counts.needs_you,
                format!("{} needs you", counts.needs_you),
                p.status_input,
                true,
            ))
        })
        .when(counts.failed > 0, |el| {
            el.child(tree_count(
                "failed",
                Some(Glyph::CircleAlert),
                counts.failed,
                format!("{} failed", counts.failed),
                p.status_failed,
                true,
            ))
        })
        .when(counts.working > 0, |el| {
            el.child(
                tree_count("working", None, counts.working, format!("{working} working"), p.status_working, true)
                    .child(crate::ui::Crab::new(p.status_working).key("tree-count-crab")),
            )
        })
        .when(counts.waiting > 0, |el| {
            el.child(tree_count(
                "waiting",
                Some(Glyph::Hourglass),
                counts.waiting,
                format!("{} waiting", counts.waiting),
                p.muted_foreground,
                false,
            ))
        })
        .when(!live && counts.finished > 0, |el| {
            el.child(
                div()
                    .debug_selector(|| "tree-count-finished".into())
                    .text_color(p.muted_foreground)
                    .child(format!("{} finished", counts.finished)),
            )
        })
        .into_any_element()
}

/// The section's head (`Head` in TreeRows.tsx): the tree glyph and the word as the toggle, the tree's counts while shut,
/// Settle N finished while open, and the chevron.
fn tree_head(listing: &transcript::Listing, open: bool, p: &Palette, cx: &mut Context<ThreadView>) -> AnyElement {
    let me = cx.weak_entity();
    let lead = listing.thread.clone();
    let toggle = {
        let (me, lead) = (me.clone(), lead.clone());
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| shut_threads(&me, &lead, open, cx)
    };
    let (finished, threads) = listing.finished();
    let settle = (open && !finished.is_empty()).then(|| {
        div().debug_selector(|| "threads-settle".into()).flex_none().child(
            Button::outline("threads-settle", format!("Settle {threads} finished"))
                .xs()
                .glyph(Glyph::Archive)
                .on_click(move |_, _, cx| settle_children(finished.clone(), cx)),
        )
    });
    let fg = p.foreground;
    h_flex()
        .debug_selector(|| "threads-head".into())
        .h(px(32.))
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .text_size(XS.0)
        .line_height(XS.1)
        .text_color(p.muted_foreground)
        .child(
            h_flex()
                .id("threads-toggle")
                .debug_selector(|| "threads-toggle".into())
                .flex_1()
                .min_w_0()
                .h_full()
                .gap(px(10.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(move |s| s.text_color(fg))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(13.))
                        .child(Icon::new(Glyph::ListTree).with_size(px(12.))),
                )
                .child(div().flex_1().min_w_0().truncate().child("Threads"))
                .when(!open, |el| el.child(tree_counts(listing.counts(), p)))
                .on_click(toggle.clone()),
        )
        .children(settle)
        .child(
            div()
                .id("threads-chevron")
                .debug_selector(|| "threads-chevron".into())
                .flex()
                .flex_none()
                .items_center()
                .cursor_pointer()
                .hover(move |s| s.text_color(fg))
                .child(Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }).with_size(px(14.)))
                .on_click(toggle),
        )
        .into_any_element()
}

/// A fold among the children in a row's box (`FoldRow`): the part's glyph, the word, then the count and the chevron in
/// the status slot's grammar; "N more" draws neither.
fn tree_fold(
    id: SharedString,
    glyph: Glyph,
    word: String,
    count: Option<(usize, bool)>,
    p: &Palette,
    flip: impl Fn(&mut App) + 'static,
) -> AnyElement {
    let selector = id.clone();
    let (accent, fg) = (p.accent, p.foreground);
    h_flex()
        .id(id)
        .debug_selector(move || selector.to_string())
        .h(px(36.))
        .w_full()
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .text_color(p.muted_foreground)
        .cursor_pointer()
        .hover(move |s| s.bg(accent).text_color(fg))
        .child(div().flex().flex_none().items_center().justify_center().size(px(13.)).child(Icon::new(glyph).with_size(px(12.))))
        .child(div().flex_1().min_w_0().truncate().child(word))
        .children(count.map(|(n, open)| {
            h_flex()
                .flex_none()
                .min_w(px(16.))
                .justify_end()
                .gap(px(4.))
                .text_size(XS.0)
                .line_height(XS.1)
                .text_color(p.muted_foreground)
                .child(n.to_string())
                .child(Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight }).with_size(px(14.)))
        }))
        .on_click(move |_, _, cx| flip(cx))
        .into_any_element()
}

/// A drawn entry of a level, and whether it is a row with a second line, where a rail's elbow meets it lower.
struct TreeEntry {
    el: AnyElement,
    two: bool,
}

/// A list of entries on the rail (`TRANSCRIPT_LIST_CLASS` and `TRANSCRIPT_ITEM_CLASS`): 14 px in, each item 8 px past
/// its line, the elbow from its top into its row's middle, the line carried on to the next item's top.
fn tree_rail(entries: Vec<TreeEntry>, p: &Palette) -> AnyElement {
    let n = entries.len();
    let rail = p.sidebar_rail;
    v_flex()
        .debug_selector(|| "tree-list".into())
        .ml(TREE_LIST)
        .min_w_0()
        .children(entries.into_iter().enumerate().map(|(k, e)| {
            let mark = if e.two { 24. } else { 18. };
            div()
                .relative()
                .min_w_0()
                .pl(px(8.))
                .when(k + 1 < n, |el| el.child(div().absolute().left_0().top(px(mark - 5.)).bottom_0().w(px(1.)).bg(rail)))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(8.))
                        .h(px(mark + 1.))
                        .border_l_1()
                        .border_b_1()
                        .border_color(rail)
                        .rounded_bl(TREE_ELBOW),
                )
                .child(e.el)
        }))
        .into_any_element()
}

/// The Threads section under the transcript (`TreeRows`): its head, then open, the tree's levels; shut, the most
/// pressing live child at any depth fading out under it, a peek that opens the section on a press.
pub(super) fn threads(
    key: &str,
    open: bool,
    listing: Option<&transcript::Listing>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let Some(listing) = listing else { return div().into_any_element() };
    let head = tree_head(listing, open, p, cx);
    let body: Vec<AnyElement> = if open {
        tree_level(&listing.children, 1, true, &format!("{key}-{}", listing.thread), p, window, cx).into_iter().map(|e| e.el).collect()
    } else {
        let me = cx.weak_entity();
        let lead = listing.thread.clone();
        listing
            .most_pressing()
            .map(|node| {
                let row = tree_row(node, true, p, window, cx);
                div()
                    .id("threads-peek")
                    .debug_selector(|| "threads-peek".into())
                    .cursor_pointer()
                    .on_click(move |_, _, cx| shut_threads(&me, &lead, false, cx))
                    // `mask-b-from-0%`: whole at the row's top, gone at its bottom.
                    .mask_fade_bottom(relative(1.), px(0.))
                    .child(row)
                    .into_any_element()
            })
            .into_iter()
            .collect()
    };
    v_flex().debug_selector(|| "threads".into()).mt(px(8.)).min_w_0().child(head).children(body).into_any_element()
}

/// One level of the tree (`Level`): its live children, then its Finished fold; the top level adds Settled. From the
/// second level on a child's own children stand at its x, in the same list.
fn tree_level(
    nodes: &[Listed],
    depth: usize,
    top: bool,
    scope: &str,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> Vec<TreeEntry> {
    let mut out = Vec::new();
    for node in nodes.iter().filter(|n| n.part == ChildPart::Live) {
        out.extend(tree_item(node, depth, p, window, cx));
    }
    let folds = [(ChildPart::Finished, "Finished", Glyph::CircleCheck, true), (ChildPart::Settled, "Settled", Glyph::Archive, top)];
    for (part, word, glyph, stands) in folds {
        let held: Vec<&Listed> = nodes.iter().filter(|n| n.part == part).collect();
        if !stands || held.is_empty() {
            continue;
        }
        let id = format!("{scope}-{word}");
        let shown = window.use_keyed_state(SharedString::from(format!("tree-open-{id}")), cx, |_, _| false);
        let page = window.use_keyed_state(SharedString::from(format!("tree-page-{id}")), cx, |_, _| TREE_PAGE);
        let is_open = *shown.read(cx);
        let flip = move |cx: &mut App| {
            shown.update(cx, |o, cx| {
                *o = !*o;
                cx.notify();
            })
        };
        out.push(TreeEntry {
            el: tree_fold(SharedString::from(format!("threads-fold-{word}")), glyph, word.to_owned(), Some((held.len(), is_open)), p, flip),
            two: false,
        });
        if !is_open {
            continue;
        }
        let shown_n = *page.read(cx);
        for node in held.iter().take(shown_n) {
            out.extend(tree_item(node, depth, p, window, cx));
        }
        if held.len() > shown_n {
            let rest = held.len() - shown_n;
            let more = move |cx: &mut App| {
                page.update(cx, |n, cx| {
                    *n += TREE_PAGE;
                    cx.notify();
                })
            };
            out.push(TreeEntry {
                el: tree_fold(
                    SharedString::from(format!("threads-more-{word}")),
                    Glyph::ChevronsDown,
                    format!("{rest} more"),
                    None,
                    p,
                    more,
                ),
                two: false,
            });
        }
    }
    out
}

/// One child's row with its own children under it (`Item`): at the top level they hang on a rail under it, deeper
/// they follow it in the list it stands in.
fn tree_item(node: &Listed, depth: usize, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> Vec<TreeEntry> {
    let row = tree_row(node, false, p, window, cx);
    // The row's message field stands in its second line's place, which the elbow follows.
    let sending = node.subagent.is_none()
        && window
            .use_keyed_state(SharedString::from(format!("tree-row-{}-sending", node.at.thread)), cx, |_, _| None::<Sending>)
            .read(cx)
            .is_some();
    let two = sending || node.note.is_some() || node.computer.is_some();
    let scope = format!("{}-{}", node.at.thread, node.subagent.as_deref().unwrap_or(""));
    let kids =
        if node.part == ChildPart::Settled { Vec::new() } else { tree_level(&node.children, depth + 1, false, &scope, p, window, cx) };
    if depth == 1 {
        let list = (!kids.is_empty()).then(|| tree_rail(kids, p));
        return vec![TreeEntry { el: v_flex().min_w_0().child(row).children(list).into_any_element(), two }];
    }
    let mut out = vec![TreeEntry { el: row, two }];
    out.extend(kids);
    out
}
