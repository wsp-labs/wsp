// SPDX-License-Identifier: AGPL-3.0-only
//! The code view the Changes pane, the pull request's Files tab and the file tab share, over gpui-kit's `Diff`, drawn
//! as today's app draws @pierre/diffs (`components/diffs/StyledDiffCodeView.tsx`, `lib/diffRendering.ts`).
//!
//! # The public pieces
//!
//! - [`model`]: [`read_reply`] reads a `git.diff` (or `git.turn`, `git.range`) reply map into a [`DiffModel`]: one
//!   [`FilePatch`] per file the daemon listed (path, old path, [`FileStatus`], counts, the patch text or `None` past
//!   the daemon's 2 MiB cut), the whole reply's cut flag, and a [`RawPatch`] when the text did not parse. Pure.
//! - [`CodeView`]: an entity built once per pane. Give it files with [`CodeView::set_files`]; set
//!   [`Layout`] (stacked or split) and wrapping on the live view; fold files ([`CodeView::set_folded`], or the owner's
//!   whole set with [`CodeView::set_folded_paths`]); draw each file's head row contents through
//!   [`CodeView::on_head`] (the view draws the sticky 32 px frame, its hover edge and the press that folds, and
//!   reports that press through [`CodeView::on_fold`]); put the owner's own element in place of a file's body with
//!   [`CodeView::set_body`]; attach [`LineNote`]s under lines and draw them through [`CodeView::on_note`]; hear the
//!   gutter's + (a press, or a drag over line numbers then +) through [`CodeView::on_add_comment`] and a press on code
//!   through [`CodeView::on_line_click`]; move with [`CodeView::scroll_to`]; light one line with
//!   [`CodeView::set_selected`].
//! - [`FileView`]: an entity for the file tab: one whole file's text with syntax colors and line numbers, no file
//!   head, scrolling sideways, with one lit line ([`FileView::set_lit`]).
//! - [`file_chevron`] (a file head's chevron, tinted by kind) and [`fold_chevron`] (a fold's chevron that turns
//!   90 degrees in 150 ms), [`raw_patch`] (a patch that did not parse, drawn raw), and the words [`CUT_NOTE`] and
//!   [`ALL_CUT`].
//!
//! Syntax colors come from each theme file's `highlight` block, generated from the pierre themes by
//! `scripts/pierre-themes.mjs`, and are prepared off the UI thread by gpui-kit's `DiffState`.

mod code_view;
mod file_view;
mod look;
pub mod model;
mod sticky;

use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use gpui_kit::component::{ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, uniform_list, AnyElement, App, ElementId, SharedString, Window};

pub use code_view::{unmodified, CodeView};
pub use file_view::FileView;
pub use gpui_kit::component::diff::{DiffLinePosition as LinePosition, DiffLineRange as LineRange, DiffSide as Side};
pub use model::{read_reply, DiffModel, FilePatch, FileStatus, RawPatch, PLAIN_LINE};

use crate::ui::{motion, ActivePalette as _, Glyph};

/// The note under the head when the daemon cut the reply (`DiffSurface.tsx`, `data-diff-truncated`).
pub const CUT_NOTE: &str = "These changes were cut at the daemon's 2 MB budget. Files listed without a patch changed too.";
/// The pane line when every listed file came with no patch.
pub const ALL_CUT: &str = "Every changed file was over the patch budget; nothing to render.";

/// How a diff lays out its two sides: today's "Stacked view" and "Split view".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    Stacked,
    Split,
}

/// What a head row draws for one file: the [`FilePatch`] facts and whether it is folded now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileHead {
    pub path: SharedString,
    pub old_path: Option<SharedString>,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    pub folded: bool,
}

/// Something the owner draws under one source line: a comment field, a kept comment, a pull request thread. `id` is
/// the owner's and stays the same while the note stands; `line` is one-based on `side`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineNote {
    pub id: SharedString,
    pub path: SharedString,
    pub side: Side,
    pub line: usize,
}

/// Draws a file head row's contents. The row around it is 32 px tall with 8 px and 12 px sides, as today's.
pub type HeadRenderer = Rc<dyn Fn(&FileHead, &mut Window, &mut App) -> AnyElement>;
/// Draws one [`LineNote`], full width under its line (or its side's column in a split view).
pub type NoteRenderer = Rc<dyn Fn(&LineNote, &mut Window, &mut App) -> AnyElement>;
/// Draws a file's body in place of its lines (an editor, while a file is edited).
pub type BodyRenderer = Rc<dyn Fn(&FileHead, &mut Window, &mut App) -> AnyElement>;

/// A file head's chevron: `size-4`, tinted by kind as `getDiffCollapseIconClassName` tints it, pointing right while
/// the file is folded and down while it is open.
pub fn file_chevron(head: &FileHead, cx: &App) -> impl IntoElement {
    let colors = look::DiffColors::of(cx);
    let tint = match head.status {
        FileStatus::Added => colors.added,
        FileStatus::Deleted => colors.deleted,
        FileStatus::Modified | FileStatus::Renamed | FileStatus::Binary => colors.modified,
    };
    Icon::new(if head.folded { Glyph::ChevronRight } else { Glyph::ChevronDown }).with_size(px(16.)).flex_none().text_color(tint)
}

/// A fold's chevron (the changed files fold, `DiffSurface.tsx`): `size-3.5`, muted, turned 90 degrees while open over
/// 150 ms in the default curve; held still under reduced motion.
pub fn fold_chevron(id: impl Into<ElementId>, open: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let degrees = motion::ease_px(id, "fold-chevron", px(if open { 90. } else { 0. }), motion::COLORS, window, cx);
    Icon::new(Glyph::ChevronRight)
        .with_size(px(14.))
        .flex_none()
        .text_color(cx.palette().muted_foreground)
        .rotate(gpui_kit::radians(f32::from(degrees) / 90. * FRAC_PI_2))
}

/// A patch that did not parse, as today's pane draws it: the reason in 11 px muted ink over the text in a bordered
/// 11 px mono block. The block draws only the lines in view, so a reply cut at 2 MiB scrolls; lines do not wrap.
pub fn raw_patch(raw: &RawPatch, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let mono = cx.theme().mono_font_family.clone();
    let lines = raw.lines.clone();
    let ink = p.muted_foreground;
    let list = uniform_list("raw-patch", lines.len(), move |range, _, _| {
        range
            .map(|ix| {
                div().debug_selector(move || format!("raw-line-{ix}")).h(px(11. * 1.625)).whitespace_nowrap().child(lines[ix].clone())
            })
            .collect()
    });
    div().size_full().flex().flex_col().p(px(8.)).child(div().mb(px(8.)).text_size(px(11.)).text_color(ink).child(raw.reason)).child(
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border.opacity(0.7))
            .bg(p.background.opacity(0.7))
            .p(px(12.))
            .font_family(mono)
            .text_size(px(11.))
            .text_color(ink)
            .child(list.flex_1().min_h_0()),
    )
}
