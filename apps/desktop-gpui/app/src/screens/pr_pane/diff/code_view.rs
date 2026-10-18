// SPDX-License-Identifier: AGPL-3.0-only
//! [`CodeView`]: gpui-kit's `Diff` over many files, with today's file heads, folds, notes and the owner's bodies.
//!
//! gpui-kit draws a file head of its own with a chevron and statistics in its own style; the view hides those and
//! draws today's head as the first file annotation of each file instead, which gpui-kit places in the file's head row,
//! so it folds, scrolls and virtualizes with the file. A file whose body the owner draws is folded in gpui-kit and its
//! body is a second file annotation.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::component::diff::{
    Diff, DiffAnnotation, DiffChangeIndicator, DiffFile, DiffHoverHighlight, DiffHunkSeparator, DiffMode, DiffState,
};
use gpui_kit::component::{ActiveTheme as _, Theme};
use gpui_kit::{canvas, div, prelude::*, px, AnyElement, App, Context, ElementId, Entity, SharedString, WeakEntity, Window};

use super::look::{self, DiffColors};
use super::model::{FilePatch, Hunk, PLAIN_LINE};
use super::sticky::{Frame, StickyHead};
use super::{BodyRenderer, FileHead, FileStatus, HeadRenderer, Layout, LineNote, LinePosition, LineRange, NoteRenderer, Side};

/// gpui-kit draws each annotation in a frame of 12 px padding and a 1 px rule above and below; a head or a note pulls
/// itself out over that frame so it is drawn edge to edge, as pierre's slots are.
const FRAME_INSET: f32 = 12.;
const FRAME_RULE: f32 = 1.;
/// `itemMetrics.diffHeaderHeight`.
pub(super) const HEAD_HEIGHT: f32 = 32.;
/// `itemMetrics.hunkSeparatorHeight`.
const GAP_HEIGHT: f32 = 24.;
/// `itemMetrics.paddingBottom`: the 8 px pierre paints under an open file's last line, drawn here above the next head.
const FILE_FOOT: f32 = 8.;
/// gpui-kit's `DiffHunkSeparator::Simple` row, `h_1`.
const HUNK_ROW: f32 = 4.;

type FoldHandler = Rc<dyn Fn(&SharedString, bool, &mut Window, &mut App)>;
type RangeHandler = Rc<dyn Fn(&LineRange, &mut Window, &mut App)>;
type ClickHandler = Rc<dyn Fn(&LinePosition, &mut Window, &mut App)>;

/// What one render hands the rows it builds lazily: the heads, the hooks, and gpui-kit's theme as the rest of the app
/// draws it, which the owner's head, note and body contents are drawn in.
pub(super) struct Shared {
    pub heads: Rc<Vec<FileHead>>,
    gaps: Vec<usize>,
    tucked: Vec<bool>,
    first_gaps: Vec<usize>,
    split: bool,
    head: Option<HeadRenderer>,
    note: Option<NoteRenderer>,
    bodies: Rc<HashMap<SharedString, BodyRenderer>>,
    notes: Rc<Vec<LineNote>>,
    app_theme: Theme,
    view: WeakEntity<CodeView>,
    pub frame: Rc<RefCell<Frame>>,
    pub colors: DiffColors,
}

/// The shared code view. See the module docs of [`super`] for what each method is for.
pub struct CodeView {
    state: Entity<DiffState>,
    heads: Rc<Vec<FileHead>>,
    /// Each drawn file's path and patch, shared with the owner's model, to tell a refresh that changed nothing.
    patches: Vec<(SharedString, Option<SharedString>)>,
    hunks: Rc<Vec<Vec<Hunk>>>,
    layout: Layout,
    wrap: bool,
    folded: HashSet<SharedString>,
    bodies: Rc<HashMap<SharedString, BodyRenderer>>,
    notes: Rc<Vec<LineNote>>,
    selected: Option<LinePosition>,
    head: Option<HeadRenderer>,
    note: Option<NoteRenderer>,
    on_fold: Option<FoldHandler>,
    on_add_comment: Option<RangeHandler>,
    on_line_click: Option<ClickHandler>,
    frame: Rc<RefCell<Frame>>,
}

impl CodeView {
    /// An empty view, stacked and wrapped as today's Changes pane opens.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| {
            DiffState::new(Vec::<DiffFile>::new(), cx)
                .with_context_lines(None)
                .with_syntax_max_line_length(PLAIN_LINE)
                .with_inline_max_line_length(PLAIN_LINE)
                .with_inline_unit(None)
        });
        cx.observe(&state, |_, _, cx| cx.notify()).detach();
        CodeView {
            state,
            heads: Rc::default(),
            patches: Vec::new(),
            hunks: Rc::default(),
            layout: Layout::Stacked,
            wrap: true,
            folded: HashSet::new(),
            bodies: Rc::default(),
            notes: Rc::default(),
            selected: None,
            head: None,
            note: None,
            on_fold: None,
            on_add_comment: None,
            on_line_click: None,
            frame: Rc::default(),
        }
    }

    /// Replaces the files. Files past the cut (no patch) and files whose patch did not parse are left out, as today's
    /// code view leaves them to the tree. Folds and the pinned head of paths still present stay; only a path the view
    /// did not show before fades in, so a refresh does not.
    pub fn set_files(&mut self, files: &[FilePatch], cx: &mut Context<Self>) {
        let before = self.heads.clone();
        let drawn: Vec<&FilePatch> = files.iter().filter(|f| f.parsed.is_some()).collect();
        let patches: Vec<(SharedString, Option<SharedString>)> = drawn.iter().map(|f| (f.path.clone(), f.patch.clone())).collect();
        // gpui-kit's set_files scrolls back to the top, so a refresh that changed nothing leaves its rows alone.
        if patches == self.patches {
            return;
        }
        self.patches = patches;
        self.folded.retain(|path| drawn.iter().any(|f| &f.path == path));
        let diffs: Vec<DiffFile> = drawn.iter().filter_map(|f| f.parsed.clone()).collect();
        self.hunks = Rc::new(drawn.iter().map(|f| f.hunks.clone()).collect());
        self.heads = Rc::new(
            drawn
                .iter()
                .map(|f| FileHead {
                    path: f.path.clone(),
                    old_path: f.old_path.clone(),
                    status: f.status,
                    additions: f.additions,
                    deletions: f.deletions,
                    folded: self.folded.contains(&f.path),
                })
                .collect(),
        );
        let shut: Vec<SharedString> = self.shut_paths();
        self.state.update(cx, |state, cx| {
            state.set_files(diffs, cx);
            for path in &shut {
                state.set_file_collapsed(path, true, cx);
            }
        });
        let now: HashMap<&SharedString, usize> = self.heads.iter().enumerate().map(|(ix, h)| (&h.path, ix)).collect();
        let mut frame = self.frame.borrow_mut();
        frame.carry(|old| before.get(old).and_then(|h| now.get(&h.path).copied()));
        let shown: HashSet<&SharedString> = before.iter().map(|h| &h.path).collect();
        for (ix, head) in self.heads.iter().enumerate() {
            if !shown.contains(&head.path) {
                frame.fade(ix);
            }
        }
        let kept = frame.top.and_then(|ix| self.heads.get(ix)).map(|h| h.path.clone());
        drop(frame);
        if let Some(path) = kept {
            self.state.update(cx, |state, cx| state.scroll_to_file(&path, cx));
        }
        self.apply_selected(cx);
        cx.notify();
    }

    /// The files drawn now, in order, with their folds.
    pub fn heads(&self) -> &[FileHead] {
        &self.heads
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    pub fn set_layout(&mut self, layout: Layout, cx: &mut Context<Self>) {
        if self.layout == layout {
            return;
        }
        self.layout = layout;
        let mode = if layout == Layout::Split { DiffMode::Split } else { DiffMode::Unified };
        self.state.update(cx, |state, cx| state.set_mode(mode, cx));
        cx.notify();
    }

    pub fn wrap(&self) -> bool {
        self.wrap
    }

    /// Wrapped lines break at the column (today's `overflow: "wrap"`); unwrapped ones scroll sideways.
    pub fn set_wrap(&mut self, wrap: bool, cx: &mut Context<Self>) {
        self.wrap = wrap;
        cx.notify();
    }

    pub fn is_folded(&self, path: &str) -> bool {
        self.folded.contains(path)
    }

    /// Folds a file to its head, or opens it; an opened body fades in. Reports nothing to [`Self::on_fold`].
    pub fn set_folded(&mut self, path: &str, folded: bool, cx: &mut Context<Self>) {
        let Some(ix) = self.heads.iter().position(|h| h.path.as_ref() == path) else { return };
        let path = self.heads[ix].path.clone();
        let changed = if folded { self.folded.insert(path.clone()) } else { self.folded.remove(&path) };
        if !changed {
            return;
        }
        if !folded {
            self.frame.borrow_mut().fade(ix);
        }
        self.sync_heads();
        let shut = folded || self.bodies.contains_key(&path) || self.heads[ix].status == FileStatus::Binary;
        self.state.update(cx, |state, cx| state.set_file_collapsed(&path, shut, cx));
        cx.notify();
    }

    /// Makes `paths` the folded set: the owner's whole fold state at once (fold all, viewed files).
    pub fn set_folded_paths(&mut self, paths: impl IntoIterator<Item = SharedString>, cx: &mut Context<Self>) {
        let want: HashSet<SharedString> = paths.into_iter().collect();
        let heads = self.heads.clone();
        for head in heads.iter() {
            self.set_folded(&head.path, want.contains(&head.path), cx);
        }
    }

    /// Draws the owner's element in place of a file's lines, under its head, or gives the lines back with `None`.
    pub fn set_body(&mut self, path: &str, body: Option<BodyRenderer>, cx: &mut Context<Self>) {
        let mut bodies = (*self.bodies).clone();
        let path: SharedString = SharedString::from(path.to_owned());
        match body {
            Some(body) => bodies.insert(path.clone(), body),
            None => bodies.remove(&path),
        };
        self.bodies = Rc::new(bodies);
        let binary = self.heads.iter().any(|h| h.path == path && h.status == FileStatus::Binary);
        let shut = self.folded.contains(&path) || self.bodies.contains_key(&path) || binary;
        self.state.update(cx, |state, cx| state.set_file_collapsed(&path, shut, cx));
        cx.notify();
    }

    /// Draws each file's head row contents: chevron, glyph, name, counts and controls. The row frames them and folds
    /// the file on a press; a control inside that must not fold calls `cx.stop_propagation()` on its press.
    pub fn on_head(&mut self, render: impl Fn(&FileHead, &mut Window, &mut App) -> AnyElement + 'static) {
        self.head = Some(Rc::new(render));
    }

    /// Hears a press on a head row, with the path and whether the press folded it.
    pub fn on_fold(&mut self, handler: impl Fn(&SharedString, bool, &mut Window, &mut App) + 'static) {
        self.on_fold = Some(Rc::new(handler));
    }

    /// Replaces the notes drawn under lines. Each is drawn through [`Self::on_note`] while its line is on screen.
    pub fn set_notes(&mut self, notes: Vec<LineNote>, cx: &mut Context<Self>) {
        self.notes = Rc::new(notes);
        cx.notify();
    }

    pub fn on_note(&mut self, render: impl Fn(&LineNote, &mut Window, &mut App) -> AnyElement + 'static) {
        self.note = Some(Rc::new(render));
    }

    /// Shows the gutter's + beside a hovered line and at the end of a line-number drag, and hears it with the lines.
    pub fn on_add_comment(&mut self, handler: impl Fn(&LineRange, &mut Window, &mut App) + 'static) {
        self.on_add_comment = Some(Rc::new(handler));
    }

    /// Hears a press on a line's code that did not drag a text selection.
    pub fn on_line_click(&mut self, handler: impl Fn(&LinePosition, &mut Window, &mut App) + 'static) {
        self.on_line_click = Some(Rc::new(handler));
    }

    /// Scrolls to a file's head, or to a line of it, opening the file if it was folded.
    pub fn scroll_to(&mut self, path: &str, line: Option<(Side, usize)>, cx: &mut Context<Self>) {
        let Some(ix) = self.heads.iter().position(|h| h.path.as_ref() == path) else { return };
        if self.folded.contains(path) {
            self.set_folded(path, false, cx);
        }
        let path = self.heads[ix].path.clone();
        self.frame.borrow_mut().jumped(ix);
        self.state.update(cx, |state, cx| match line {
            Some((side, line)) => state.scroll_to_line(LinePosition::new(path, side, line), cx),
            None => state.scroll_to_file(&path, cx),
        });
        cx.notify();
    }

    /// Lights one line, with the selected tint, or none.
    pub fn set_selected(&mut self, line: Option<LinePosition>, cx: &mut Context<Self>) {
        self.selected = line;
        self.apply_selected(cx);
    }

    /// The gpui-kit state the view draws, for a test or a bench to read.
    pub fn state(&self) -> &Entity<DiffState> {
        &self.state
    }

    fn apply_selected(&mut self, cx: &mut Context<Self>) {
        let range = self.selected.as_ref().map(|p| LineRange::new(p.path().clone(), p.side(), p.line(), p.line()));
        self.state.update(cx, |state, cx| state.set_selected_lines(range, cx));
    }

    fn shut_paths(&self) -> Vec<SharedString> {
        self.heads
            .iter()
            .filter(|h| h.folded || h.status == FileStatus::Binary || self.bodies.contains_key(&h.path))
            .map(|h| h.path.clone())
            .collect()
    }

    fn sync_heads(&mut self) {
        self.heads = Rc::new(self.heads.iter().map(|h| FileHead { folded: self.folded.contains(&h.path), ..h.clone() }).collect());
    }

    fn press_head(&mut self, ix: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(head) = self.heads.get(ix) else { return };
        let path = head.path.clone();
        let folded = !head.folded;
        self.set_folded(&path, folded, cx);
        if let Some(handler) = self.on_fold.clone() {
            handler(&path, folded, window, cx);
        }
    }

    /// gpui-kit's annotations: each file's head, then its first "unmodified lines" bar and the owner's body, the bars
    /// between hunks under each hunk's last line, and the owner's notes; with the line counts the bars say, by index.
    fn annotations(&self) -> Annotated {
        let mut out = Vec::with_capacity(self.heads.len() * 3 + self.notes.len());
        let mut gaps = Vec::new();
        let mut tucked = vec![false; self.heads.len()];
        let mut first_gaps = vec![0; self.heads.len()];
        let split = self.layout == Layout::Split;
        for (ix, head) in self.heads.iter().enumerate() {
            out.push(DiffAnnotation::file(ElementId::NamedInteger(HEAD.into(), ix as u64), head.path.clone()));
            if head.folded {
                continue;
            }
            if self.bodies.contains_key(&head.path) {
                out.push(DiffAnnotation::file(ElementId::NamedInteger(BODY.into(), ix as u64), head.path.clone()));
                continue;
            }
            let hunks = self.hunks.get(ix).map(Vec::as_slice).unwrap_or_default();
            tucked[ix] = !hunks.is_empty();
            for (k, hunk) in hunks.iter().enumerate() {
                let previous = k.checked_sub(1).map(|p| &hunks[p]);
                let hidden = hunk.hidden_before(previous);
                let Some(p) = previous else {
                    first_gaps[ix] = hidden;
                    continue;
                };
                if hidden == 0 {
                    continue;
                }
                let (side, line) = p.last_row(split);
                let position = LinePosition::new(head.path.clone(), side, line);
                out.push(DiffAnnotation::line(ElementId::NamedInteger(GAP.into(), gaps.len() as u64), position));
                gaps.push(hidden);
            }
        }
        for (ix, note) in self.notes.iter().enumerate() {
            out.push(DiffAnnotation::line(
                ElementId::NamedInteger(NOTE.into(), ix as u64),
                LinePosition::new(note.path.clone(), note.side, note.line),
            ));
        }
        Annotated { annotations: out, gaps, tucked, first_gaps }
    }
}

struct Annotated {
    annotations: Vec<DiffAnnotation>,
    gaps: Vec<usize>,
    tucked: Vec<bool>,
    first_gaps: Vec<usize>,
}

const HEAD: &str = "wsp-diff-head";
const BODY: &str = "wsp-diff-body";
const NOTE: &str = "wsp-diff-note";
const GAP: &str = "wsp-diff-gap";

/// The owner's element, laid out and drawn in gpui-kit's theme as the rest of the app has it rather than the code
/// surface's. It is built while the code surface's is active: swapping the global for the build too made every frame
/// draw twice (31 fps against 62 in `pr_diff_scroll`), so a builder reads colors from `cx.palette()`, as wsp's pieces do.
fn owner_drawn(shared: &Shared, cx: &mut App, build: impl FnOnce(&mut App) -> AnyElement) -> AnyElement {
    let element = build(cx);
    look::with_theme(shared.app_theme.clone(), element).into_any_element()
}

/// Pulls a slot's contents out over gpui-kit's annotation frame.
fn edge_to_edge(child: impl IntoElement) -> gpui_kit::Div {
    div().mx(px(-FRAME_INSET)).my(px(-(FRAME_INSET + FRAME_RULE))).child(child)
}

/// gpui-kit opens every hunk with a 4 px row of its own (the plainest separator it has); the slot drawn just above one
/// gives those 4 px back, so a head, a bar and the first line stand where today's do. The row paints only the
/// background over the slot's last 4 px, which hold no ink.
fn tuck(slot: gpui_kit::Div) -> gpui_kit::Div {
    slot.mb(px(-(FRAME_INSET + FRAME_RULE + HUNK_ROW)))
}

/// One file's head row: 32 px, today's sides, the head's background, a 3 px inset edge on hover, and a press that
/// folds. `sticky` is the copy drawn pinned at the top, which records nothing and blocks the row under it.
pub(super) fn head_row(shared: &Rc<Shared>, ix: usize, sticky: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let Some(head) = shared.heads.get(ix) else { return div().into_any_element() };
    let content = shared.head.as_ref().map(|render| owner_drawn(shared, cx, |cx| render(head, window, cx)));
    let view = shared.view.clone();
    let colors = shared.colors;
    let frame = shared.frame.clone();
    div()
        .id(ElementId::NamedInteger(if sticky { "wsp-diff-sticky" } else { "wsp-diff-row" }.into(), ix as u64))
        .group("wsp-diff-head")
        .debug_selector(move || format!("diff-{}-{ix}", if sticky { "sticky" } else { "head" }))
        .relative()
        .flex()
        .items_center()
        .h(px(HEAD_HEIGHT))
        .pl(px(8.))
        .pr(px(12.))
        .bg(colors.background)
        .text_size(px(12.))
        .cursor_pointer()
        .when(sticky, |this| this.w_full().occlude())
        .on_click(move |_, window, cx| {
            _ = view.update(cx, |view, cx| view.press_head(ix, window, cx));
        })
        .child(div().flex_1().min_w_0().h_full().flex().items_center().children(content))
        .child(div().absolute().left_0().top_0().bottom_0().w(px(3.)).group_hover("wsp-diff-head", move |this| this.bg(colors.edge)))
        .when(!sticky, |this| {
            this.child(canvas(move |bounds, _, _| frame.borrow_mut().saw(ix, bounds), |_, _, _, _| {}).absolute().size_full())
        })
        .into_any_element()
}

fn render_annotation(shared: &Rc<Shared>, annotation: &DiffAnnotation, window: &mut Window, cx: &mut App) -> AnyElement {
    let ElementId::NamedInteger(kind, ix) = annotation.id() else { return div().into_any_element() };
    let ix = *ix as usize;
    match kind.as_ref() {
        HEAD => {
            let first = shared.first_gaps.get(ix).copied().unwrap_or(0);
            let row = head_row(shared, ix, false, window, cx);
            let mut slot = edge_to_edge(div().child(row).when(first > 0, |this| this.child(gap_row(first, shared.split, &shared.colors))));
            if ix > 0 && shared.heads.get(ix - 1).is_some_and(|h| !h.folded && h.status != FileStatus::Binary) {
                slot = slot.mt(px(FILE_FOOT - FRAME_INSET - FRAME_RULE));
            }
            if shared.tucked.get(ix).copied().unwrap_or(false) { tuck(slot) } else { slot }.into_any_element()
        }
        BODY => {
            let (Some(head), Some(body)) = (shared.heads.get(ix), shared.heads.get(ix).and_then(|h| shared.bodies.get(&h.path))) else {
                return div().into_any_element();
            };
            edge_to_edge(owner_drawn(shared, cx, |cx| body(head, window, cx))).into_any_element()
        }
        GAP => {
            let Some(count) = shared.gaps.get(ix) else { return div().into_any_element() };
            // gpui-kit holds an annotation to 26 px, so with its 4 px row this bar spans 30 px; it is centred in them.
            div()
                .mx(px(-FRAME_INSET))
                .mt(px(-(FRAME_INSET + FRAME_RULE) + 3.))
                .mb(px(-(GAP_HEIGHT - (FRAME_INSET + FRAME_RULE) + 3.)))
                .child(gap_row(*count, shared.split, &shared.colors))
                .into_any_element()
        }
        NOTE => {
            let (Some(note), Some(render)) = (shared.notes.get(ix), shared.note.as_ref()) else { return div().into_any_element() };
            edge_to_edge(owner_drawn(shared, cx, |cx| render(note, window, cx))).into_any_element()
        }
        _ => div().into_any_element(),
    }
}

/// `N unmodified lines`, as pierre counts them (`createSeparator`), in English's plural.
pub fn unmodified(count: usize) -> String {
    format!("{count} unmodified line{}", if count == 1 { "" } else { "s" })
}

/// Today's `line-info` separator: 24 px of the code background, the count in 11 px sans at 52% ink, between two rules in
/// a stacked view, at the start of the row in a split one.
fn gap_row(count: usize, split: bool, colors: &DiffColors) -> AnyElement {
    let rule = || div().flex_1().h(px(1.)).bg(colors.gap_rule);
    div()
        .debug_selector(move || format!("diff-gap-{count}"))
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(GAP_HEIGHT))
        .pl(px(if split { 16. } else { 8. }))
        .pr(px(12.))
        .bg(colors.background)
        .text_size(px(11.))
        .text_color(colors.gap_ink)
        .when(!split, |this| this.child(rule()))
        .child(unmodified(count))
        .when(!split, |this| this.child(rule()))
        .into_any_element()
}

impl Render for CodeView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = DiffColors::of(cx);
        let Annotated { annotations, gaps, tucked, first_gaps } = self.annotations();
        let shared = Rc::new(Shared {
            heads: self.heads.clone(),
            gaps,
            tucked,
            first_gaps,
            split: self.layout == Layout::Split,
            head: self.head.clone(),
            note: self.note.clone(),
            bodies: self.bodies.clone(),
            notes: self.notes.clone(),
            app_theme: cx.theme().clone(),
            view: cx.entity().downgrade(),
            frame: self.frame.clone(),
            colors,
        });
        let rows = shared.clone();
        let mut diff = Diff::new(&self.state)
            .header_visible(false)
            .hunk_separator(DiffHunkSeparator::Simple)
            .change_indicator(DiffChangeIndicator::Bars)
            .hover_highlight(DiffHoverHighlight::None)
            .soft_wrap(self.wrap)
            .annotations(annotations)
            .render_annotation(move |annotation, window, cx| render_annotation(&rows, annotation, window, cx))
            .border_0();
        if let Some(handler) = self.on_add_comment.clone() {
            diff = diff.on_add_annotation(move |range, window, cx| handler(range, window, cx));
        }
        if let Some(handler) = self.on_line_click.clone() {
            diff = diff.on_line_click(move |position, _, window, cx| handler(position, window, cx));
        }
        let still = cx.reduce_motion() || cx.try_global::<crate::ui::Still>().is_some_and(|s| s.0);
        div()
            .id("code-view")
            .relative()
            .size_full()
            .bg(colors.background)
            .child(look::with_theme(look::code_theme(cx), diff))
            .child(StickyHead::new(shared, Instant::now(), still))
    }
}
