// SPDX-License-Identifier: AGPL-3.0-only
//! Comments on a diff's lines, as today's app draws them (`components/diffs/DiffCommentAnnotation.tsx`,
//! `AnnotatableCodeView.tsx`, `reviewCommentContext.ts`, `pull-request/ThreadActs.tsx` and the thread box of
//! `pull-request/Conversation.tsx`):
//!
//! - [`LineComments`]: one pane tab's comments written here, the open field and the kept ones, handed to the code
//!   view as [`LineNote`]s and drawn by [`LineComments::render_note`]; [`LineComments::take_quote`] gives the block
//!   the pane's Send to thread puts in the composer.
//! - [`CommentField`]: the field a person writes a reply or a pull request comment in: Send, an optional Cancel,
//!   Cmd or Ctrl Enter, the host's refusal under it.
//! - [`ThreadBox`]: a GitHub review thread: its path row, the code it sits on, its comments, Resolve and Reply, folded
//!   to its path row once resolved.
//! - [`send_to_thread`]: words appended to a thread's composer draft.

use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, AnyElement, App, Context, Entity, EventEmitter, Focusable as _, FontWeight, Hsla, SharedString, Subscription,
    Task, Window,
};

use super::diff::{LineNote, LineRange, Side};
use crate::local_store::LocalStore;
use crate::screens::composer::store as drafts;
use crate::ui::{chord_key, ActivePalette as _, Button, Glyph, KeyCaps, TextArea, TextAreaLook};

/// `DiffCommentAnnotation`'s placeholder.
pub const PLACEHOLDER: &str = "Add a comment…";
/// The chord's hint under a line comment's field, the same words on every platform as today's.
pub const HINT: &str = "⌘/Ctrl Enter to send";
pub const SEND_TO_THREAD: &str = "Send to thread";
/// The comments one pane tab keeps; past it the oldest goes, as a pass's comments go to the thread long before.
pub const MAX_KEPT: usize = 200;
/// The lines of a hunk a thread shows above its comments (`hunkTail`).
const TAIL: usize = 3;
/// Under this window width a kept comment's delete stands without a hover (`max-sm:opacity-100`).
const NARROW: f32 = 640.;

/// What a diff line is: kept, added or deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Context,
    Add,
    Delete,
}

impl Change {
    fn mark(self) -> &'static str {
        match self {
            Change::Context => " ",
            Change::Add => "+",
            Change::Delete => "-",
        }
    }
}

/// One line of a unified patch with its numbers on each side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffRow {
    pub change: Change,
    pub old: Option<usize>,
    pub new: Option<usize>,
    pub content: String,
}

/// Every hunk line of a one-file unified patch, in order; the file's own head lines are skipped.
pub fn patch_rows(patch: &str) -> Vec<DiffRow> {
    let mut rows = Vec::new();
    let mut at: Option<(usize, usize)> = None;
    for raw in patch.lines() {
        if let Some(rest) = raw.strip_prefix("@@ -") {
            at = hunk_start(rest);
            continue;
        }
        if raw.starts_with("diff --git ") {
            at = None;
            continue;
        }
        let Some((old, new)) = at.as_mut() else { continue };
        if raw.starts_with('\\') {
            continue;
        }
        let (change, content) = match raw.chars().next() {
            Some('+') => (Change::Add, &raw[1..]),
            Some('-') => (Change::Delete, &raw[1..]),
            Some(' ') => (Change::Context, &raw[1..]),
            None => (Change::Context, ""),
            Some(_) => continue,
        };
        let (o, n) = match change {
            Change::Add => (None, Some(*new)),
            Change::Delete => (Some(*old), None),
            Change::Context => (Some(*old), Some(*new)),
        };
        if o.is_some() {
            *old += 1;
        }
        if n.is_some() {
            *new += 1;
        }
        rows.push(DiffRow { change, old: o, new: n, content: content.to_owned() });
    }
    rows
}

/// `12,3 +14,4 @@`: where a hunk starts on each side.
fn hunk_start(rest: &str) -> Option<(usize, usize)> {
    let (old, rest) = rest.split_once(" +")?;
    let new = rest.split(" @@").next()?;
    let first = |s: &str| s.split(',').next().and_then(|n| n.parse().ok());
    Some((first(old)?, first(new)?))
}

/// The last lines of a thread's hunk, the commented one last.
pub fn hunk_tail(hunk: &str) -> Vec<DiffRow> {
    let rows = patch_rows(hunk);
    rows[rows.len().saturating_sub(TAIL)..].to_vec()
}

fn row_of(rows: &[DiffRow], side: Side, line: usize) -> Option<usize> {
    rows.iter().position(|r| match side {
        Side::Original => r.old == Some(line) && r.change != Change::Add,
        Side::Modified => r.new == Some(line) && r.change != Change::Delete,
    })
}

/// `formatDiffReviewRangeLabel`: `+12`, `-4 to -7`, `12 to 15`.
pub fn range_label(rows: &[DiffRow]) -> String {
    let (Some(first), Some(last)) = (rows.first(), rows.last()) else { return "line".into() };
    let (Some(a), Some(b)) = (first.new.or(first.old), last.new.or(last.old)) else {
        return if rows.len() == 1 { "line".into() } else { format!("{} lines", rows.len()) };
    };
    let mark = if first.change != Change::Context && rows.iter().all(|r| r.change == first.change) { first.change.mark() } else { "" };
    if a == b {
        format!("{mark}{a}")
    } else {
        format!("{mark}{a} to {mark}{b}")
    }
}

/// The hunk a comment quotes: its head and its lines with their marks.
fn hunk_of(rows: &[DiffRow]) -> String {
    let side = |f: fn(&DiffRow) -> Option<usize>| {
        let numbered: Vec<usize> = rows.iter().filter_map(f).collect();
        (numbered.first().copied().unwrap_or(0), numbered.len())
    };
    let (old, old_n) = side(|r| r.old);
    let (new, new_n) = side(|r| r.new);
    let mut out = format!("@@ -{old},{old_n} +{new},{new_n} @@");
    for r in rows {
        out.push('\n');
        out.push_str(r.change.mark());
        out.push_str(&r.content);
    }
    out
}

/// A comment written here and kept on its line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    pub id: u64,
    pub path: SharedString,
    pub side: Side,
    pub line: usize,
    pub range_label: String,
    pub diff: String,
    pub text: String,
}

impl Kept {
    /// `reviewCommentQuote`: the file and lines, the hunk in a diff fence, the words.
    pub fn quote(&self) -> String {
        format!("{} {}\n```diff\n{}\n```\n{}", self.path, self.range_label, self.diff, self.text)
    }
}

struct Draft {
    id: u64,
    path: SharedString,
    side: Side,
    line: usize,
    range_label: String,
    diff: String,
    field: Entity<TextareaState>,
    _changes: Subscription,
}

pub enum LineCommentsEvent {
    /// A comment was kept or deleted, or the field opened or closed: the owner sets the code view's notes again.
    Changed,
}

/// One pane tab's line comments: at most one open field, which the gutter's + opens, and up to [`MAX_KEPT`] kept.
#[derive(Default)]
pub struct LineComments {
    kept: Vec<Kept>,
    draft: Option<Draft>,
    seq: u64,
}

impl EventEmitter<LineCommentsEvent> for LineComments {}

impl LineComments {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn kept(&self) -> &[Kept] {
        &self.kept
    }

    /// Whether a field is open; while it is, the gutter takes no new comment, as today's.
    pub fn has_draft(&self) -> bool {
        self.draft.is_some()
    }

    /// The open field's state, for a test or a caller that fills it.
    pub fn draft_field(&self) -> Option<&Entity<TextareaState>> {
        self.draft.as_ref().map(|d| &d.field)
    }

    /// Opens the field under the range's last line, from the file's patch; a range the patch does not hold opens
    /// nothing. Focuses the field.
    pub fn begin(&mut self, range: &LineRange, patch: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.draft.is_some() {
            return false;
        }
        let rows = patch_rows(patch);
        let (Some(a), Some(b)) = (row_of(&rows, range.side(), range.start()), row_of(&rows, range.end_side(), range.end())) else {
            return false;
        };
        let picked = &rows[a.min(b)..=a.max(b)];
        let field = TextArea::state(PLACEHOLDER, window, cx);
        let changes = cx.subscribe(&field, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        field.read(cx).focus_handle(cx).focus(window, cx);
        self.seq += 1;
        self.draft = Some(Draft {
            id: self.seq,
            path: range.path().clone(),
            side: range.end_side(),
            line: range.end(),
            range_label: range_label(picked),
            diff: hunk_of(picked),
            field,
            _changes: changes,
        });
        cx.emit(LineCommentsEvent::Changed);
        cx.notify();
        true
    }

    pub fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.draft.take().is_some() {
            cx.emit(LineCommentsEvent::Changed);
            cx.notify();
        }
    }

    /// Keeps the field's words on its line; blank words keep nothing.
    pub fn submit(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(draft) = self.draft.as_ref() else { return false };
        let text = draft.field.read(cx).value().trim().to_owned();
        if text.is_empty() {
            return false;
        }
        let Some(d) = self.draft.take() else { return false };
        self.kept.push(Kept { id: d.id, path: d.path, side: d.side, line: d.line, range_label: d.range_label, diff: d.diff, text });
        if self.kept.len() > MAX_KEPT {
            self.kept.remove(0);
        }
        cx.emit(LineCommentsEvent::Changed);
        cx.notify();
        true
    }

    pub fn remove(&mut self, id: u64, cx: &mut Context<Self>) {
        let before = self.kept.len();
        self.kept.retain(|k| k.id != id);
        if self.kept.len() != before {
            cx.emit(LineCommentsEvent::Changed);
            cx.notify();
        }
    }

    /// Every kept comment, oldest first, as one block for the composer (`reviewCommentsQuote`); the tab keeps none
    /// after, as today's pane hands them over. None when there are none.
    pub fn take_quote(&mut self, cx: &mut Context<Self>) -> Option<String> {
        if self.kept.is_empty() {
            return None;
        }
        let quote = self.kept.iter().map(Kept::quote).collect::<Vec<_>>().join("\n\n");
        self.kept.clear();
        cx.emit(LineCommentsEvent::Changed);
        cx.notify();
        Some(quote)
    }

    /// The notes the code view stands under lines: each kept comment, then the open field.
    pub fn notes(&self) -> Vec<LineNote> {
        let kept =
            self.kept.iter().map(|k| LineNote { id: format!("kept-{}", k.id).into(), path: k.path.clone(), side: k.side, line: k.line });
        let draft =
            self.draft.iter().map(|d| LineNote { id: format!("draft-{}", d.id).into(), path: d.path.clone(), side: d.side, line: d.line });
        kept.chain(draft).collect()
    }

    /// Draws one of [`Self::notes`]; none for a note that is not this tab's.
    pub fn render_note(this: &Entity<Self>, note: &LineNote, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let me = this.read(cx);
        if let Some(id) = note.id.strip_prefix("draft-").and_then(|n| n.parse::<u64>().ok()) {
            let draft = me.draft.as_ref().filter(|d| d.id == id)?;
            return Some(comment_field_note(this, draft.id, &draft.field, &draft.range_label, cx));
        }
        let id = note.id.strip_prefix("kept-").and_then(|n| n.parse::<u64>().ok())?;
        let kept = me.kept.iter().find(|k| k.id == id)?.clone();
        let this = this.clone();
        Some(kept_comment(kept.id, kept.text.into(), Some(Rc::new(move |_, cx| this.update(cx, |c, cx| c.remove(id, cx)))), window, cx))
    }
}

type Act = Rc<dyn Fn(&mut Window, &mut App)>;

/// The open field under a line: the field, the hint, Cancel and Comment.
fn comment_field_note(this: &Entity<LineComments>, id: u64, field: &Entity<TextareaState>, range: &str, cx: &App) -> AnyElement {
    let p = cx.palette();
    let blank = field.read(cx).value().trim().is_empty();
    let (submit, escape, cancel, comment) = (this.clone(), this.clone(), this.clone(), this.clone());
    div()
        .id(SharedString::from(format!("line-comment-field-{id}")))
        .debug_selector(|| "line-comment-field".into())
        .px(px(12.))
        .py(px(8.))
        .text_color(p.foreground)
        .child(
            TextArea::new(SharedString::from(format!("line-comment-{range}")), field)
                .look(TextAreaLook::Comment)
                .on_submit(move |_, cx| _ = submit.update(cx, |c, cx| c.submit(cx)))
                .on_escape(move |_, cx| escape.update(cx, |c, cx| c.cancel(cx))),
        )
        .child(
            h_flex()
                .mt(px(6.))
                .gap_1()
                .child(div().mr_auto().flex_1().min_w_0().text_size(px(11.)).text_color(p.muted_foreground).child(HINT))
                .child(div().debug_selector(|| "line-comment-cancel".into()).child(
                    Button::ghost("line-comment-cancel", "Cancel").xs().on_click(move |_, _, cx| cancel.update(cx, |c, cx| c.cancel(cx))),
                ))
                .child(
                    Button::primary("line-comment-submit", "Comment")
                        .xs()
                        .held(blank)
                        .on_click(move |_, _, cx| _ = comment.update(cx, |c, cx| c.submit(cx))),
                ),
        )
        .into_any_element()
}

/// A comment kept on its line: the bubble glyph, the words, and its delete on hover (always under 640 px).
pub fn kept_comment(id: u64, text: SharedString, on_delete: Option<Act>, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let group: SharedString = format!("kept-comment-{id}").into();
    let narrow = f32::from(window.viewport_size().width) < NARROW;
    let row = h_flex()
        .id(group.clone())
        .group(group.clone())
        .debug_selector(|| "kept-comment".into())
        .items_start()
        .gap(px(10.))
        .border_l_1()
        .border_color(p.border)
        .bg(p.muted.opacity(0.3))
        .px(px(12.))
        .py(px(10.))
        .child(Icon::new(Glyph::MessageCircle).with_size(px(14.)).mt(px(3.)).flex_none().text_color(p.primary.opacity(0.7)))
        .child(div().flex_1().min_w_0().text_size(px(13.)).line_height(px(20.)).text_color(p.foreground).child(text))
        .children(on_delete.map(|f| {
            div()
                .flex_none()
                .my(px(-4.))
                .mr(px(-4.))
                .when(!narrow, |el| el.opacity(0.).group_hover(group.clone(), |s| s.opacity(1.)))
                .debug_selector(|| "kept-comment-delete".into())
                .child(
                    Button::icon_xs(SharedString::from(format!("{group}-delete")), Glyph::Trash)
                        .tooltip("Delete comment")
                        .on_click(move |_, window, cx| f(window, cx)),
                )
        }));
    div().border_t_1().border_b_1().border_color(p.border.opacity(0.3)).child(row).into_any_element()
}

/// What a reply or comment send gives back: done, or the host's refusal in its words.
pub type Sent = Task<Result<(), String>>;
pub type SendBody = Rc<dyn Fn(String, &mut Window, &mut App) -> Sent>;

pub enum CommentFieldEvent {
    Sent,
    Cancelled,
}

/// The one field a person writes in from the pane (`ThreadActs.tsx`'s `CommentField`): the box with its words, the
/// chord's keycap, Cancel where it can close and Send; the words kept and the refusal said under it when refused.
pub struct CommentField {
    field: Entity<TextareaState>,
    cancellable: bool,
    clear_on_sent: bool,
    sending: bool,
    refusal: Option<String>,
    on_send: Option<SendBody>,
    _changes: Subscription,
    _send: Option<Task<()>>,
}

impl EventEmitter<CommentFieldEvent> for CommentField {}

impl CommentField {
    /// `label` is the placeholder: "Reply to octo", "Leave a comment".
    pub fn new(label: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let field = TextArea::state(label, window, cx);
        let changes = cx.subscribe(&field, |_, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                cx.notify();
            }
        });
        CommentField {
            field,
            cancellable: false,
            clear_on_sent: false,
            sending: false,
            refusal: None,
            on_send: None,
            _changes: changes,
            _send: None,
        }
    }

    /// A reply's field closes with Cancel; the foot's comment box has none.
    pub fn cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }

    /// The foot's box empties once its comment is posted; a reply's field closes instead.
    pub fn clear_on_sent(mut self) -> Self {
        self.clear_on_sent = true;
        self
    }

    pub fn on_send(&mut self, send: impl Fn(String, &mut Window, &mut App) -> Sent + 'static) {
        self.on_send = Some(Rc::new(send));
    }

    /// The field's state: a quote reply fills it, a test types in it.
    pub fn field(&self) -> &Entity<TextareaState> {
        &self.field
    }

    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let body = self.field.read(cx).value().trim().to_owned();
        let Some(send) = self.on_send.clone().filter(|_| !body.is_empty() && !self.sending) else { return };
        self.sending = true;
        self.refusal = None;
        let task = send(body, window, cx);
        self._send = Some(cx.spawn_in(window, async move |this, cx| {
            let said = task.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.sending = false;
                match said {
                    Ok(()) => {
                        if this.clear_on_sent {
                            this.field.update(cx, |s, cx| s.set_value("", window, cx));
                        }
                        cx.emit(CommentFieldEvent::Sent);
                    }
                    Err(said) => this.refusal = Some(said),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
        if self.cancellable {
            cx.emit(CommentFieldEvent::Cancelled);
        }
    }
}

impl Render for CommentField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let blank = self.field.read(cx).value().trim().is_empty();
        let (submit, escape, cancel, send) = (cx.entity(), cx.entity(), cx.entity(), cx.entity());
        let cancellable = self.cancellable;
        let area = TextArea::new("comment-field-text", &self.field)
            .look(TextAreaLook::Bare)
            .on_submit(move |window, cx| submit.update(cx, |f, cx| f.send(window, cx)));
        let area = if cancellable { area.on_escape(move |_, cx| escape.update(cx, |f, cx| f.cancel(cx))) } else { area };
        v_flex()
            .debug_selector(|| "comment-field".into())
            .w_full()
            .gap(px(6.))
            .child(
                v_flex().overflow_hidden().rounded(px(10.)).border_1().border_color(p.border).bg(p.background).child(area).child(
                    h_flex()
                        .gap_2()
                        .px_2()
                        .pb_2()
                        .child(div().mr_auto().child(KeyCaps::new(&[&[chord_key(), "Enter"]])))
                        .when(cancellable, |el| {
                            el.child(
                                Button::ghost("comment-field-cancel", "Cancel")
                                    .xs()
                                    .on_click(move |_, _, cx| cancel.update(cx, |f, cx| f.cancel(cx))),
                            )
                        })
                        .child(
                            Button::primary("comment-field-send", "Send")
                                .xs()
                                .held(blank || self.sending)
                                .on_click(move |_, window, cx| send.update(cx, |f, cx| f.send(window, cx))),
                        ),
                ),
            )
            .children(self.refusal.clone().map(|said| {
                div().debug_selector(|| "comment-field-refusal".into()).text_size(px(13.)).text_color(p.error_foreground).child(said)
            }))
    }
}

/// A GitHub review thread as the pane reads it: where it stands, whether it is resolved, its hunk and comments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewThread {
    pub key: SharedString,
    pub path: SharedString,
    pub line: Option<usize>,
    pub resolved: bool,
    pub hunk: Option<String>,
    pub comments: Vec<ThreadComment>,
}

/// One comment of a thread: its id, who wrote it as the pane names them, and when, as "3h ago".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadComment {
    pub id: u64,
    pub author: SharedString,
    pub at: SharedString,
}

/// What the pull request pane draws into a comment: the face, the body, the acts on the head's hover after the
/// time, the chips under the body and a note before the time ("Sent 3 h ago").
pub struct CommentParts {
    pub face: AnyElement,
    pub body: AnyElement,
    pub tools: Option<AnyElement>,
    pub chips: Option<AnyElement>,
    pub note: Option<SharedString>,
}

pub type PartsOf = Rc<dyn Fn(&ThreadComment, &mut Window, &mut App) -> CommentParts>;
pub type ResolveThread = Rc<dyn Fn(bool, &mut Window, &mut App) -> Sent>;

/// A comment someone wrote on the host (`DiffCommentAnnotation`'s `said`): the face, the name, a note, when, the acts
/// and the words.
pub fn said_comment(comment: &ThreadComment, parts: CommentParts, cx: &App) -> AnyElement {
    let p = cx.palette();
    let mono = cx.theme().mono_font_family.clone();
    let group: SharedString = format!("said-{}", comment.id).into();
    let has_note = parts.note.is_some();
    div()
        .id(group.clone())
        .group(group.clone())
        .debug_selector(|| "said-comment".into())
        .border_t_1()
        .border_color(p.border.opacity(0.5))
        .child(
            h_flex()
                .items_start()
                .gap(px(10.))
                .px(px(12.))
                .pt(px(10.))
                .pb(px(12.))
                .text_color(p.foreground)
                .child(div().flex_none().w(px(20.)).child(parts.face))
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            h_flex()
                                .min_h(px(20.))
                                .min_w_0()
                                .gap_2()
                                .text_size(px(13.))
                                .line_height(px(20.))
                                .child(div().min_w_0().truncate().font_weight(FontWeight::MEDIUM).child(comment.author.clone()))
                                .children(
                                    parts
                                        .note
                                        .map(|n| div().ml_auto().flex_none().text_size(px(12.)).text_color(p.muted_foreground).child(n)),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .map(|el| if has_note { el.ml(px(10.)) } else { el.ml_auto() })
                                        .font_family(mono)
                                        .text_size(px(12.))
                                        .text_color(p.muted_foreground)
                                        .child(comment.at.clone()),
                                )
                                .children(
                                    parts
                                        .tools
                                        .map(|t| div().flex_none().opacity(0.).group_hover(group.clone(), |s| s.opacity(1.)).child(t)),
                                ),
                        )
                        .child(parts.body)
                        .children(parts.chips),
                ),
        )
        .into_any_element()
}

/// `--code-background`: the card at 90% over the page.
fn code_ground(cx: &App) -> Hsla {
    let p = cx.palette();
    p.background.blend(p.card.opacity(0.9))
}

/// The code a thread sits on: its hunk's last lines, the commented one lit.
fn thread_code(hunk: &str, cx: &App) -> Option<AnyElement> {
    let rows = hunk_tail(hunk);
    if rows.is_empty() {
        return None;
    }
    let p = cx.palette().clone();
    let last = rows.len() - 1;
    let number = |n: Option<usize>| {
        div().flex_none().w(px(34.)).pr_2().text_right().text_color(p.muted_foreground).child(n.map(|n| n.to_string()).unwrap_or_default())
    };
    Some(
        v_flex()
            .debug_selector(|| "thread-code".into())
            .border_t_1()
            .border_color(p.border.opacity(0.5))
            .bg(code_ground(cx))
            .py_1()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .line_height(px(18.))
            .children(rows.into_iter().enumerate().map(|(i, r)| {
                let (fill, ink) = match r.change {
                    Change::Add => (Some(p.success.opacity(0.11)), p.success),
                    Change::Delete => (Some(p.error.opacity(0.11)), p.error_foreground),
                    Change::Context => (None, p.foreground),
                };
                h_flex()
                    .items_start()
                    .when_some(fill, |el, f| el.bg(f))
                    .when(i == last, |el| el.child(div().absolute().inset_0().bg(p.primary.opacity(0.09))))
                    .relative()
                    .child(number(r.old))
                    .child(number(r.new))
                    .child(div().flex_1().min_w_0().pr(px(10.)).text_color(ink).child(format!("{} {}", r.change.mark(), r.content)))
            }))
            .into_any_element(),
    )
}

/// A GitHub review thread under its line or in the activity: open while unresolved, folded to its path row once
/// resolved, where a press opens it; Resolve or Unresolve and Reply where the signed-in gh can act.
pub struct ThreadBox {
    thread: ReviewThread,
    parts: PartsOf,
    resolve: Option<ResolveThread>,
    reply: Option<SendBody>,
    open: bool,
    busy: bool,
    replying: Option<Entity<CommentField>>,
    _resolving: Option<Task<()>>,
    _reply: Option<Subscription>,
}

impl ThreadBox {
    pub fn new(thread: ReviewThread, parts: PartsOf) -> Self {
        let open = !thread.resolved;
        ThreadBox { thread, parts, resolve: None, reply: None, open, busy: false, replying: None, _resolving: None, _reply: None }
    }

    /// The thread as the host reads it now; the fold is kept unless it turned resolved.
    pub fn set_thread(&mut self, thread: ReviewThread, cx: &mut Context<Self>) {
        if thread.resolved != self.thread.resolved {
            self.open = !thread.resolved;
        }
        self.thread = thread;
        cx.notify();
    }

    pub fn thread(&self) -> &ReviewThread {
        &self.thread
    }

    pub fn is_open(&self) -> bool {
        self.open || !self.thread.resolved
    }

    pub fn on_resolve(&mut self, f: impl Fn(bool, &mut Window, &mut App) -> Sent + 'static) {
        self.resolve = Some(Rc::new(f));
    }

    pub fn on_reply(&mut self, f: impl Fn(String, &mut Window, &mut App) -> Sent + 'static) {
        self.reply = Some(Rc::new(f));
    }

    pub fn toggle_resolved(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(resolve) = self.resolve.clone().filter(|_| !self.busy) else { return };
        self.busy = true;
        let was = self.thread.resolved;
        let task = resolve(!was, window, cx);
        self._resolving = Some(cx.spawn(async move |this, cx| {
            let done = task.await;
            _ = this.update(cx, |this, cx| {
                this.busy = false;
                if done.is_ok() {
                    this.thread.resolved = !was;
                    this.open = was;
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn start_reply(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(reply) = self.reply.clone() else { return };
        let to = self.thread.comments.first().map(|c| c.author.clone()).unwrap_or_default();
        let field = cx.new(|cx| {
            let mut f = CommentField::new(format!("Reply to {to}"), window, cx).cancellable();
            f.on_send(move |body, window, cx| reply(body, window, cx));
            f
        });
        field.read(cx).field().read(cx).focus_handle(cx).focus(window, cx);
        self._reply = Some(cx.subscribe(&field, |this, _, _: &CommentFieldEvent, cx| {
            this.replying = None;
            this._reply = None;
            cx.notify();
        }));
        self.replying = Some(field);
        cx.notify();
    }

    /// The open reply field, for a test.
    pub fn reply_field(&self) -> Option<&Entity<CommentField>> {
        self.replying.as_ref()
    }

    fn path_row(&self, folded: bool, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let t = &self.thread;
        let row = h_flex()
            .id("thread-path")
            .debug_selector(|| "thread-path".into())
            .w_full()
            .min_w_0()
            .gap_2()
            .h(px(if folded { 34. } else { 30. }))
            .px(px(10.))
            .bg(code_ground(cx))
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(px(12.))
            .child(Icon::new(Glyph::File).with_size(px(13.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
            .child(div().min_w_0().truncate().child(t.path.clone()))
            .children(t.line.map(|l| div().flex_none().text_color(p.muted_foreground).child(format!(":{l}"))))
            .when(t.resolved, |el| {
                el.child(
                    h_flex()
                        .ml_auto()
                        .flex_none()
                        .gap(px(6.))
                        .font_family(cx.theme().font_family.clone())
                        .text_color(p.muted_foreground)
                        .child(Icon::new(Glyph::CircleCheck).with_size(px(13.)).text_color(p.status_done))
                        .child("Resolved"),
                )
            })
            .when(folded, |el| {
                let n = t.comments.len();
                el.child(
                    div()
                        .ml(px(10.))
                        .flex_none()
                        .font_family(cx.theme().font_family.clone())
                        .text_color(p.muted_foreground)
                        .child(if n == 1 { "1 comment".to_owned() } else { format!("{n} comments") }),
                )
            })
            .when(t.resolved, |el| {
                let turn = if folded { 0. } else { std::f32::consts::FRAC_PI_2 };
                el.child(
                    Icon::new(Glyph::ChevronRight)
                        .with_size(px(13.))
                        .ml_2()
                        .flex_none()
                        .text_color(p.muted_foreground.opacity(0.7))
                        .rotate(gpui_kit::radians(turn)),
                )
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.open = !this.open;
                    cx.notify();
                }))
            });
        row.into_any_element()
    }
}

impl Render for ThreadBox {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let shown = self.is_open();
        let mut body = Vec::new();
        if shown {
            if let Some(code) = self.thread.hunk.as_deref().and_then(|h| thread_code(h, cx)) {
                body.push(code);
            }
            let parts = self.parts.clone();
            for c in &self.thread.comments {
                let made = parts(c, window, cx);
                body.push(said_comment(c, made, cx));
            }
            if self.resolve.is_some() || self.reply.is_some() {
                let resolve_word = if self.thread.resolved { "Unresolve" } else { "Resolve" };
                body.push(
                    h_flex()
                        .debug_selector(|| "thread-foot".into())
                        .justify_end()
                        .gap(px(2.))
                        .border_t_1()
                        .border_color(p.border.opacity(0.5))
                        .px_2()
                        .py(px(6.))
                        .when(self.resolve.is_some(), |el| {
                            el.child(
                                Button::ghost("thread-resolve", resolve_word)
                                    .xs()
                                    .held(self.busy)
                                    .on_click(cx.listener(|this, _, window, cx| this.toggle_resolved(window, cx))),
                            )
                        })
                        .when(self.reply.is_some(), |el| {
                            el.child(
                                Button::ghost("thread-reply", "Reply")
                                    .xs()
                                    .on_click(cx.listener(|this, _, window, cx| this.start_reply(window, cx))),
                            )
                        })
                        .into_any_element(),
                );
            }
            if let Some(field) = self.replying.clone() {
                body.push(div().mx(px(12.)).mb(px(12.)).child(field).into_any_element());
            }
        }
        v_flex()
            .debug_selector(|| "review-thread".into())
            .w_full()
            .mt(px(10.))
            .overflow_hidden()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border)
            .bg(p.card)
            .child(self.path_row(!shown, cx))
            .children(body)
    }
}

/// `sendToThread`: the words go under whatever the thread's composer draft holds, for the person to edit and send;
/// nothing goes to the agent. `thread_key` is the composer's: the thread's id, else the workspace's for a new one. A
/// composer open on that key now reads it the next time it opens.
pub fn send_to_thread(thread_key: &str, quote: &str, cx: &mut App) -> std::io::Result<()> {
    if !cx.has_global::<LocalStore>() {
        return Err(std::io::Error::other("no local store"));
    }
    cx.update_global(|store: &mut LocalStore, _| {
        let draft = drafts::draft(store, thread_key).map(|(text, _)| text).unwrap_or_default();
        let prompt = if draft.is_empty() { format!("{quote}\n\n") } else { format!("{draft}\n\n{quote}\n\n") };
        drafts::set_draft(store, thread_key, &prompt, prompt.chars().count())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH: &str = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -10,4 +10,5 @@ fn main\n keep\n-gone\n+came\n+also\n tail\n";

    #[test]
    fn rows_carry_their_numbers() {
        let rows = patch_rows(PATCH);
        assert_eq!(rows.len(), 5);
        assert_eq!((rows[0].old, rows[0].new), (Some(10), Some(10)));
        assert_eq!((rows[1].old, rows[1].new, rows[1].change), (Some(11), None, Change::Delete));
        assert_eq!((rows[2].old, rows[2].new, rows[2].change), (None, Some(11), Change::Add));
        assert_eq!((rows[4].old, rows[4].new), (Some(12), Some(13)));
    }

    #[test]
    fn labels_read_as_today() {
        let rows = patch_rows(PATCH);
        assert_eq!(range_label(&rows[2..4]), "+11 to +12");
        assert_eq!(range_label(&rows[1..2]), "-11");
        assert_eq!(range_label(&rows[0..1]), "10");
        assert_eq!(range_label(&rows[0..3]), "10 to 11");
        assert_eq!(hunk_of(&rows[1..3]), "@@ -11,1 +11,1 @@\n-gone\n+came");
    }

    #[test]
    fn a_tail_is_the_last_three() {
        let tail = hunk_tail("@@ -1,4 +1,4 @@\n a\n b\n-c\n+d\n");
        assert_eq!(tail.iter().map(|r| r.content.as_str()).collect::<Vec<_>>(), ["b", "c", "d"]);
    }
}
