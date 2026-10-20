// SPDX-License-Identifier: AGPL-3.0-only
//! A file's head in the code view: its row of controls, its title and its counts.

use std::time::Duration;

use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, SharedString, WeakEntity, Window};

use super::model::{self, base_name};
use super::{words, ChangesScreen};
use crate::screens::pr_pane::diff::{self, FileHead, FileStatus};
use crate::ui::controls::checkbox;
use crate::ui::{tagged, tip, ActivePalette as _, Button, Glyph};

/// A file head's counts as pierre writes them: deletions first in red, then additions in green, each left out at
/// zero unless both are.
pub fn head_counts(additions: usize, deletions: usize, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    h_flex()
        .debug_selector(|| "head-counts".into())
        .flex_none()
        .gap(px(4.))
        .mr(px(4.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(11.))
        .children(head_count_words(additions, deletions).into_iter().map(|word| {
            let ink = if word.starts_with('-') { p.error_foreground } else { p.success };
            let tag = format!("head-count:{word}");
            div().debug_selector(move || tag.clone()).text_color(ink).child(word)
        }))
}

/// A diff head's file title as today's `[data-title]` draws it: under the pointer its ink takes 16% of the primary and
/// its underline comes in, both over 120 ms in CSS's `ease`. GPUI draws an underline at its own offset, not 2 px.
pub fn file_title(path: &str, el: gpui_kit::Div, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let id = SharedString::from(format!("file-title:{path}"));
    let hovered = crate::ui::motion::hovered(id.clone(), window, cx);
    let over = *hovered.read(cx);
    let target = if over { p.foreground.blend(p.primary.opacity(0.16)) } else { p.foreground };
    let ease = gpui_kit::base::Easing::cubic_bezier(0.25, 0.1, 0.25, 1.).expect("CSS's ease is a valid curve");
    let timing = gpui_kit::base::Transition::new(Duration::from_millis(120)).easing(ease);
    let ink = crate::ui::motion::ease_ink_by(id.clone(), "ink", target, timing.clone(), window, cx);
    let line = crate::ui::motion::ease_ink_by(id.clone(), "underline", if over { target } else { target.opacity(0.) }, timing, window, cx);
    let tag = id.clone();
    el.id(id)
        .debug_selector(move || format!("{tag}:{}", hex(ink)))
        .cursor_pointer()
        .text_color(ink)
        .text_decoration_1()
        .text_decoration_color(line)
        .on_hover(crate::ui::motion::track_hover(hovered))
}

/// `#rrggbb`, for a selector a test reads a drawn ink by.
pub fn hex(ink: gpui_kit::Hsla) -> String {
    let c = ink.to_rgb();
    let byte = |x: f32| (x * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

/// The words a file head's counts say, in their order.
pub fn head_count_words(additions: usize, deletions: usize) -> Vec<String> {
    let mut words = Vec::new();
    if deletions > 0 || additions == 0 {
        words.push(format!("-{deletions}"));
    }
    if additions > 0 || deletions == 0 {
        words.push(format!("+{additions}"));
    }
    words
}

impl ChangesScreen {
    /// One file's head row inside the code view's 32 px frame: its chevron and viewed tick, its path and counts, then
    /// its edit or save and stop, and its discard.
    pub(super) fn head_row(screen: &Entity<ChangesScreen>, head: &FileHead, window: &mut Window, cx: &mut App) -> AnyElement {
        let this = screen.read(cx);
        let p = cx.palette().clone();
        let path = head.path.to_string();
        let name = base_name(&path).to_owned();
        let file = this.file(&path);
        let blob = file.and_then(|f| f.blob.clone());
        let viewed = file.is_some_and(|f| model::is_viewed(f, &this.marks));
        let turn = this.is_turn();
        let scope = this.scope();
        let editing = this.editing.as_ref().filter(|e| e.path == path).map(|e| e.saving);
        let can_edit = this.editing.is_none() && this.repo.is_some() && file.is_some_and(|f| model::editable(f, scope, turn));
        let can_discard = model::discardable(scope, turn) && !this.elsewhere();
        let weak = screen.downgrade();
        let act =
            |weak: &WeakEntity<ChangesScreen>, f: fn(&mut ChangesScreen, &str, &mut Window, &mut Context<ChangesScreen>), path: &str| {
                let (weak, path) = (weak.clone(), path.to_owned());
                move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut App| {
                    cx.stop_propagation();
                    _ = weak.update(cx, |this, cx| f(this, &path, window, cx));
                }
            };
        let mut controls: Vec<(String, AnyElement)> = Vec::new();
        match editing {
            Some(saving) => {
                controls.push((
                    format!("save:{path}"),
                    Button::icon_xs(SharedString::from(format!("save:{path}")), Glyph::Check)
                        .side(px(20.))
                        .held(saving)
                        .tooltip(words::save(&name))
                        .on_click(act(&weak, |this, _, window, cx| this.save_edit(window, cx), &path))
                        .into_any_element(),
                ));
                controls.push((
                    format!("stop:{path}"),
                    Button::icon_xs(SharedString::from(format!("stop:{path}")), Glyph::X)
                        .side(px(20.))
                        .held(saving)
                        .tooltip(words::stop_editing(&name))
                        .on_click(act(&weak, |this, _, _, cx| this.stop_edit(cx), &path))
                        .into_any_element(),
                ));
            }
            None if can_edit => controls.push((
                format!("edit:{path}"),
                Button::icon_xs(SharedString::from(format!("edit:{path}")), Glyph::Pencil)
                    .side(px(20.))
                    .tooltip(words::edit(&name))
                    .on_click(act(&weak, |this, path, window, cx| this.start_edit(path, window, cx), &path))
                    .into_any_element(),
            )),
            None => {}
        }
        if can_discard {
            controls.push((
                format!("discard:{path}"),
                Button::icon_xs(SharedString::from(format!("discard:{path}")), Glyph::Trash)
                    .side(px(20.))
                    .tooltip(words::discard_control(&name))
                    .on_click(act(&weak, |this, path, window, cx| this.ask_discard(path, window, cx), &path))
                    .into_any_element(),
            ));
        }
        let controls: Vec<AnyElement> = controls.into_iter().map(|(tag, button)| tagged(tag, button)).collect();
        let renamed_from = head.old_path.clone().filter(|_| head.status == FileStatus::Renamed);
        // Pierre's head marks: its own glyphs in its addition, deletion and modified bases, which the code view's
        // colors read off the same highlight status (diff/look.rs).
        let status = &cx.theme().highlight_theme.style.status;
        let (mark, ink) = match head.status {
            FileStatus::Added => (crate::ui::icons::DIFF_ADDED, status.success(cx)),
            FileStatus::Deleted => (crate::ui::icons::DIFF_DELETED, status.error(cx)),
            FileStatus::Renamed => (crate::ui::icons::DIFF_MOVED, status.info(cx)),
            FileStatus::Modified | FileStatus::Binary => (crate::ui::icons::DIFF_MODIFIED, status.info(cx)),
        };
        let mark = div()
            .debug_selector({
                let tag = format!("head-mark:{path}");
                move || tag.clone()
            })
            .flex_none()
            .ml(px(4.))
            .child(Icon::empty().path(mark).with_size(px(16.)).text_color(ink));
        let sans = cx.theme().font_family.clone();
        // Pierre's `[data-header-content]`: names cut from the start (`direction: rtl`), 8 px apart, the old one at 70%.
        let name_el = h_flex()
            .flex_1()
            .min_w_0()
            .ml(px(4.))
            .gap(px(8.))
            .overflow_hidden()
            .whitespace_nowrap()
            .font_family(sans)
            .text_size(px(13.))
            .children(
                renamed_from
                    .map(|old| div().min_w_0().overflow_hidden().text_ellipsis_start().text_color(p.foreground.opacity(0.7)).child(old)),
            )
            .when(head.status == FileStatus::Renamed && head.old_path.is_some(), |el| {
                el.child(Icon::empty().path(crate::ui::icons::DIFF_RENAMED_TO).with_size(px(16.)).flex_none().text_color(p.foreground))
            })
            .child(file_title(&path, div().min_w_0().overflow_hidden().text_ellipsis_start().child(path.clone()), window, cx));
        let chevron = div()
            .id(SharedString::from(format!("fold:{path}")))
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(20.))
            .ml(px(-2.))
            .rounded(px(4.))
            .hover(|s| s.bg(p.foreground.opacity(0.1)))
            .cursor_pointer()
            .tooltip(tip(if head.folded { words::EXPAND_FILE } else { words::COLLAPSE_FILE }))
            .on_click(act(&weak, |this, path, _, cx| this.toggle_file(path, cx), &path))
            .child(diff::file_chevron(head, cx));
        let tick = blob.map(|_| {
            div()
                .id(SharedString::from(format!("viewed:{path}")))
                .debug_selector({
                    let tag = format!("viewed-tick:{path}");
                    move || tag.clone()
                })
                .ml(px(4.))
                .cursor_pointer()
                .tooltip(tip(words::viewed_tick(&name)))
                .on_click(act(&weak, |this, path, _, cx| this.toggle_viewed(path, cx), &path))
                .child(checkbox(SharedString::from(format!("viewed-box:{path}")), viewed, false, window, cx))
        });
        h_flex()
            .w_full()
            .h_full()
            .gap(px(4.))
            .child(chevron)
            .children(tick)
            .child(mark)
            .child(name_el)
            .child(head_counts(head.additions, head.deletions, cx))
            .child(h_flex().flex_none().gap(px(4.)).children(controls))
            .into_any_element()
    }
}
