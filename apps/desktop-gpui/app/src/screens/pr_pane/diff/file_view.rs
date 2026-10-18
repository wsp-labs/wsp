// SPDX-License-Identifier: AGPL-3.0-only
//! [`FileView`]: one whole file for the file tab (`files/FilePreviewSurface.tsx`), on the same gpui-kit `Diff` the code
//! view draws, as a file of unchanged lines: one line-number column, syntax colors, no head, sideways scrolling, and a
//! lit line.

use gpui_kit::component::diff::{Diff, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffState};
use gpui_kit::{div, prelude::*, Context, Entity, SharedString, Window};

use super::look::{self, DiffColors};
use super::model::PLAIN_LINE;
use super::{LinePosition, LineRange, Side};

pub struct FileView {
    state: Entity<DiffState>,
    path: SharedString,
    lit: Option<usize>,
}

impl FileView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| {
            DiffState::new(Vec::<DiffFile>::new(), cx)
                .with_context_lines(None)
                .with_syntax_max_line_length(PLAIN_LINE)
                .with_inline_max_line_length(PLAIN_LINE)
        });
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        FileView { state, path: SharedString::default(), lit: None }
    }

    /// Shows `text` as the file at `path`, whose name picks the syntax. Holds one copy of the text, which the file tab
    /// reads no more than 2 MB of.
    pub fn set_file(&mut self, path: impl Into<SharedString>, text: &str, cx: &mut Context<Self>) {
        self.path = path.into();
        let file = DiffFile::unchanged(self.path.clone(), &ended(text));
        self.state.update(cx, |state, cx| state.set_files([file], cx));
        self.apply_lit(cx);
    }

    /// Lights one one-based line and scrolls it into view (opened at a line, W2-PRP-039), or lights none.
    pub fn set_lit(&mut self, line: Option<usize>, cx: &mut Context<Self>) {
        self.lit = line;
        self.apply_lit(cx);
        if let Some(line) = line {
            let position = LinePosition::new(self.path.clone(), Side::Modified, line);
            self.state.update(cx, |state, cx| state.scroll_to_line(position, cx));
        }
    }

    pub fn lit(&self) -> Option<usize> {
        self.lit
    }

    /// The gpui-kit state the view draws, for a test to read.
    pub fn state(&self) -> &Entity<DiffState> {
        &self.state
    }

    fn apply_lit(&mut self, cx: &mut Context<Self>) {
        let range = self.lit.map(|line| LineRange::new(self.path.clone(), Side::Modified, line, line));
        self.state.update(cx, |state, cx| state.set_selected_lines(range, cx));
    }
}

/// `text` with a line end after its last line: gpui-kit draws "No newline at end of file" under a last line without one,
/// which today's file tab does not.
fn ended(text: &str) -> std::borrow::Cow<'_, str> {
    if text.is_empty() || text.ends_with('\n') {
        text.into()
    } else {
        format!("{text}\n").into()
    }
}

impl Render for FileView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = DiffColors::of(cx);
        let diff = Diff::new(&self.state)
            .header_visible(false)
            .change_indicator(DiffChangeIndicator::None)
            .hover_highlight(DiffHoverHighlight::None)
            .soft_wrap(false)
            .border_0();
        div().id("file-view").size_full().bg(colors.background).child(look::with_theme(look::code_theme(cx), diff))
    }
}
