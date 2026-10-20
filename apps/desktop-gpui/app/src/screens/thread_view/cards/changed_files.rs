// SPDX-License-Identifier: AGPL-3.0-only
//! `ChangedFilesCard`: a turn's changed files under a head that sticks under the list's top.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    canvas, div, point, prelude::*, px, AnyElement, App, Bounds, Context, Display, Element, ElementId, FlexDirection, FontWeight,
    GlobalElementId, InspectorElementId, LayoutId, Pixels, SharedString, Style, Window,
};

use super::{ghost_xs, mix_over, XS};
use crate::screens::pr_pane::changes::model::{tree_rows, TreeFile};
use crate::screens::pr_pane::changes::tree::{stat_inline, ChangedFilesTree};
use crate::thread_view::ThreadView;
use crate::transcript::ChangedFile;
use crate::ui::{tip, Glyph, Palette};

/// A press on the changed files' card or one of its files, by the file's path, none for the whole turn.
type PressPath = Rc<dyn Fn(Option<&str>, &mut App)>;

/// Which folders a card's trees stand open in: every folder at the card's toggle, less the ones pressed since, by
/// tree (the others' or the turn's own); and the card's width as last laid out.
#[derive(Default)]
struct Folders {
    all: bool,
    pressed: HashMap<(bool, String), bool>,
    width: Option<Pixels>,
}

/// `@[24rem]/changed-files`: from this wide Open diff says its word.
const OPEN_DIFF_WORD: Pixels = px(384.);
/// `top-2`: where the head sticks under the list's top.
const HEAD_STICK: Pixels = px(8.);

/// `ChangedFilesCard`: the head with the count, the stat and its acts, sticking under the list's top while the card is
/// in view; then the panes' tree with every folder shut, and the folder's others under their own words.
#[allow(clippy::too_many_arguments)]
pub(super) fn changed_files(
    key: &str,
    label: &str,
    files: &[ChangedFile],
    others: &[ChangedFile],
    turn: Option<&str>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let dark = cx.theme().is_dark();
    let folders = window.use_keyed_state(SharedString::from(format!("folders-{key}")), cx, |_, _| Folders::default());
    let (all, width) = {
        let f = folders.read(cx);
        (f.all, f.width.unwrap_or(super::super::rows::COLUMN))
    };
    let (added, removed) = files.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
    let nested = files.iter().chain(others).any(|f| f.path.contains(['/', '\\']));
    let (view, turn) = (cx.weak_entity(), turn.map(str::to_owned));
    let press: PressPath = Rc::new(move |path, cx| {
        let (Some(view), Some(turn)) = (view.upgrade(), turn.as_deref()) else { return };
        let opens = super::super::rows::turn_diff(view.read(cx), turn, path, None, cx);
        opens.run(cx);
    });
    let toggle_all = nested.then(|| {
        let words = &crate::screens::pr_pane::pull_request::words::PR_WORDS;
        let (g, word) = if all { (Glyph::ChevronsDownUp, words.collapse_folders) } else { (Glyph::ChevronsUpDown, words.expand_folders) };
        let folders = folders.clone();
        ghost_xs(SharedString::from(format!("folders-all-{key}")), g, None, true, p)
            .debug_selector(|| "expand-all".into())
            .tooltip(tip(word))
            .on_click(move |_, _, cx| {
                folders.update(cx, |f, cx| {
                    f.all = !f.all;
                    f.pressed.clear();
                    cx.notify();
                })
            })
    });
    let first = files.first().or(others.first()).map(|f| f.path.clone());
    let opening = press.clone();
    let open_diff = ghost_xs(SharedString::from(format!("open-diff-{key}")), Glyph::FileDiff, None, true, p)
        .debug_selector(|| "open-diff".into())
        .w_auto()
        .px(px(8.))
        .when(width >= OPEN_DIFF_WORD, |el| el.child(div().debug_selector(|| "open-diff-word".into()).child("Open diff")))
        .tooltip(tip("Open the full diff"))
        .on_click(move |_, _, cx| opening(first.as_deref(), cx));
    let head_fill = if dark { mix_over(p.input, 0.2, p.background) } else { p.muted };
    let head = h_flex()
        .id(SharedString::from(format!("changed-head-{key}")))
        .debug_selector(|| "changed-head".into())
        .occlude()
        .justify_between()
        .gap(px(8.))
        .rounded_t(px(10.))
        .bg(head_fill)
        .px(px(12.))
        .py(px(8.))
        .child(
            h_flex()
                .min_w_0()
                .flex_wrap()
                .gap_x(px(12.))
                .text_size(XS.0)
                .line_height(XS.1)
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.foreground)
                .child(label.to_owned())
                .when(added > 0 || removed > 0, |el| el.child(stat_inline(added as usize, removed as usize, XS.0, cx))),
        )
        .child(h_flex().flex_none().gap(px(4.)).children(toggle_all).child(open_diff));
    let tree_of = |list: &[ChangedFile], theirs: bool, cx: &mut Context<ThreadView>| {
        let files: Vec<TreeFile> =
            list.iter().map(|f| TreeFile { path: f.path.clone(), additions: f.added as usize, deletions: f.removed as usize }).collect();
        let state = folders.read(cx);
        let shut: HashSet<String> = tree_rows(&files, &HashSet::new())
            .into_iter()
            .filter(|r| r.folder && !state.pressed.get(&(theirs, r.path.clone())).copied().unwrap_or(state.all))
            .map(|r| r.path)
            .collect();
        let rows = Rc::new(tree_rows(&files, &shut));
        let (pressing, opening) = (folders.clone(), press.clone());
        let id = format!("{key}-{}", if theirs { "others" } else { "own" });
        let tree = ChangedFilesTree::new(id, rows, Rc::new(shut), &files, width - px(16.))
            .uncapped()
            .on_folder(move |path, _, cx| {
                pressing.update(cx, |f, cx| {
                    let now = f.pressed.get(&(theirs, path.to_owned())).copied().unwrap_or(f.all);
                    f.pressed.insert((theirs, path.to_owned()), !now);
                    cx.notify();
                })
            })
            .on_file(move |path, _, cx| opening(Some(path), cx));
        div().py(px(8.)).px(px(4.)).child(tree)
    };
    let own = (!files.is_empty()).then(|| tree_of(files, false, cx));
    let also = (!others.is_empty()).then(|| {
        v_flex()
            .pt(px(16.))
            .child(
                div()
                    .debug_selector(|| "changed-also".into())
                    .px(px(12.))
                    .text_size(XS.0)
                    .line_height(XS.1)
                    .text_color(p.muted_foreground)
                    .child("Also changed in this folder"),
            )
            .child(tree_of(others, true, cx))
    });
    // A width read in this frame draws in the next, which nothing else may ask for.
    let measure = canvas(
        move |bounds, window, cx| {
            let moved = folders.update(cx, |f, cx| {
                let moved = f.width != Some(bounds.size.width);
                if moved {
                    f.width = Some(bounds.size.width);
                    cx.notify();
                }
                moved
            });
            if moved {
                window.request_animation_frame();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    v_flex()
        .debug_selector(|| "changed-files".into())
        .relative()
        .rounded(px(10.))
        .bg(if dark { p.input.opacity(0.2) } else { p.muted })
        .child(measure)
        .child(StickyHead { head: head.into_any_element(), body: v_flex().children(own).children(also).into_any_element() })
        .into_any_element()
}

/// `sticky top-2 z-10`: a card's head held 8 px under the list's top while the rest of its card is in view under it,
/// drawn over the rows it passes.
struct StickyHead {
    head: AnyElement,
    body: AnyElement,
}

impl IntoElement for StickyHead {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Element for StickyHead {
    type RequestLayoutState = LayoutId;
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
    ) -> (LayoutId, LayoutId) {
        let head = self.head.request_layout(window, cx);
        let body = self.body.request_layout(window, cx);
        let style = Style { display: Display::Flex, flex_direction: FlexDirection::Column, ..Style::default() };
        (window.request_layout(style, [head, body], cx), head)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        head: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.body.prepaint(window, cx);
        let at = window.layout_bounds(*head);
        let shift = (window.content_mask().bounds.top() + HEAD_STICK - at.top()).min(bounds.bottom() - at.bottom()).max(px(0.));
        window.with_element_offset(point(px(0.), shift), |window| self.head.prepaint(window, cx));
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.body.paint(window, cx);
        self.head.paint(window, cx);
    }
}
