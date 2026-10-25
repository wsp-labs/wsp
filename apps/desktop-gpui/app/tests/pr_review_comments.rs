// SPDX-License-Identifier: AGPL-3.0-only
//! Line comments in a window, as the code view draws them under their lines: the gutter's + opens the field, Cmd or
//! Ctrl and Enter keeps the comment, Escape and Cancel close the field, a kept comment's delete takes it away, and the
//! kept ones quote into one block for the thread. A GitHub thread folds once resolved and opens on a press, and its
//! reply field sends through the pane's hook and says the host's refusal.

use std::cell::RefCell;
use std::rc::Rc;

use desktop_gpui::screens::pr_pane::comments::{CommentParts, LineComments, ReviewThread, ThreadBox, ThreadComment, HINT};
use desktop_gpui::screens::pr_pane::diff::{LineRange, Side};
use desktop_gpui::ui::{self, Side as Theme};
use gpui_kit::component::Root;
use gpui_kit::{
    div, prelude::*, px, size, AnyElement, Context, Entity, Focusable as _, Modifiers, Task, TestAppContext, VisualTestContext, Window,
};

const PATCH: &str = "diff --git a/src/pane.rs b/src/pane.rs\n--- a/src/pane.rs\n+++ b/src/pane.rs\n@@ -1 +1,3 @@\n-fn main() {}\n+fn main() {\n+    println!(\"pane\");\n+}\n";

struct Lines {
    comments: Entity<LineComments>,
}

impl Render for Lines {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notes = self.comments.read(cx).notes();
        let drawn: Vec<AnyElement> = notes.iter().filter_map(|n| LineComments::render_note(&self.comments, n, window, cx)).collect();
        div().w(px(560.)).children(drawn)
    }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Theme::Light, cx);
    });
}

fn lines(cx: &mut TestAppContext) -> (Entity<LineComments>, &mut VisualTestContext) {
    init(cx);
    let comments = cx.new(|_| LineComments::new());
    let held = comments.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| {
            cx.observe(&held, |_, _, cx| cx.notify()).detach();
            Lines { comments: held.clone() }
        });
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(600.)));
    cx.run_until_parked();
    (comments, cx)
}

fn begin(comments: &Entity<LineComments>, start: usize, end: usize, cx: &mut VisualTestContext) -> bool {
    let opened =
        comments.update_in(cx, |c, window, cx| c.begin(&LineRange::new("src/pane.rs", Side::Modified, start, end), PATCH, window, cx));
    cx.run_until_parked();
    opened
}

#[gpui_kit::test]
fn the_chord_keeps_a_comment_and_escape_drops_the_field(cx: &mut TestAppContext) {
    let (comments, cx) = lines(cx);
    assert!(begin(&comments, 1, 2, cx), "the gutter's + opens a field on lines the patch holds");
    assert!(cx.debug_bounds("line-comment-field").is_some(), "the field stands under the line");
    let placeholder = comments.read_with(cx, |c, cx| c.draft_field().unwrap().read(cx).presentation().placeholder().to_string());
    assert_eq!(placeholder, "Add a comment…");
    assert_eq!(HINT, "⌘/Ctrl Enter to send");
    assert!(!begin(&comments, 3, 3, cx), "a second field waits on the first");

    cx.simulate_input("Name the pane.");
    cx.simulate_keystrokes("secondary-enter");
    cx.run_until_parked();
    let kept = comments.read_with(cx, |c, _| c.kept().to_vec());
    assert_eq!(kept.len(), 1, "Cmd or Ctrl and Enter keeps the comment");
    assert_eq!(kept[0].text, "Name the pane.");
    assert_eq!(kept[0].range_label, "+1 to +2");
    assert_eq!((kept[0].side, kept[0].line), (Side::Modified, 2));
    assert!(comments.read_with(cx, |c, _| !c.has_draft()), "and closes the field");
    assert!(cx.debug_bounds("kept-comment").is_some(), "the kept comment stands on its line");

    assert!(begin(&comments, 3, 3, cx));
    cx.simulate_input("never mind");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(comments.read_with(cx, |c, _| !c.has_draft()), "Escape closes the field");
    assert_eq!(comments.read_with(cx, |c, _| c.kept().len()), 1, "and keeps nothing");

    let quote = comments.update(cx, |c, cx| c.take_quote(cx)).expect("a quote");
    assert_eq!(quote, "src/pane.rs +1 to +2\n```diff\n@@ -0,0 +1,2 @@\n+fn main() {\n+    println!(\"pane\");\n```\nName the pane.");
    assert!(comments.read_with(cx, |c, _| c.kept().is_empty()), "the pane keeps none once they go to the thread");
}

#[gpui_kit::test]
fn cancel_and_delete(cx: &mut TestAppContext) {
    let (comments, cx) = lines(cx);
    begin(&comments, 2, 2, cx);
    let cancel = cx.debug_bounds("line-comment-cancel").expect("Cancel");
    cx.simulate_click(cancel.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(comments.read_with(cx, |c, _| !c.has_draft()), "Cancel closes the field");

    begin(&comments, 2, 2, cx);
    cx.simulate_input("keep me");
    cx.simulate_keystrokes("secondary-enter");
    cx.run_until_parked();
    let delete = cx.debug_bounds("kept-comment-delete").expect("the delete");
    cx.simulate_click(delete.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(comments.read_with(cx, |c, _| c.kept().is_empty()), "the delete takes the comment away");
}

struct Holder {
    thread: Entity<ThreadBox>,
}

impl Render for Holder {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(560.)).child(self.thread.clone())
    }
}

fn a_thread(resolved: bool) -> ReviewThread {
    ReviewThread {
        key: "t1".into(),
        path: "src/pane.rs".into(),
        line: Some(2),
        resolved,
        hunk: Some("@@ -1 +1,3 @@\n-fn main() {}\n+fn main() {\n+    println!(\"pane\");\n+}".into()),
        comments: vec![
            ThreadComment { id: 1, author: "octo".into(), at: "3h ago".into() },
            ThreadComment { id: 2, author: "ada".into(), at: "1h ago".into() },
        ],
    }
}

fn thread(cx: &mut TestAppContext, resolved: bool) -> (Entity<ThreadBox>, Rc<RefCell<Vec<String>>>, &mut VisualTestContext) {
    init(cx);
    let sent = Rc::new(RefCell::new(Vec::new()));
    let said = sent.clone();
    let mut held = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let parts = Rc::new(|c: &ThreadComment, _: &mut Window, _: &mut gpui_kit::App| CommentParts {
            face: div().size(px(20.)).into_any_element(),
            body: div().child(format!("body {}", c.id)).into_any_element(),
            tools: None,
            chips: None,
            note: None,
        });
        let thread = cx.new(|_| {
            let mut t = ThreadBox::new(a_thread(resolved), parts);
            t.on_resolve(|_, _, _| Task::ready(Ok(())));
            t.on_reply(move |body, _, _| {
                said.borrow_mut().push(body.clone());
                Task::ready(if body.contains("refuse") { Err("GitHub said no".into()) } else { Ok(()) })
            });
            t
        });
        held = Some(thread.clone());
        let view = cx.new(|_| Holder { thread });
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(800.)));
    cx.run_until_parked();
    (held.unwrap(), sent, cx)
}

#[gpui_kit::test]
fn a_resolved_thread_folds_and_opens_on_a_press(cx: &mut TestAppContext) {
    let (thread, _, cx) = thread(cx, true);
    assert!(!thread.read_with(cx, |t, _| t.is_open()), "resolved, it folds to its path row");
    assert!(cx.debug_bounds("said-comment").is_none(), "with no comments drawn");
    let row = cx.debug_bounds("thread-path").expect("the path row");
    cx.simulate_click(row.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(thread.read_with(cx, |t, _| t.is_open()), "a press opens it");
    assert!(cx.debug_bounds("said-comment").is_some());
    assert!(cx.debug_bounds("thread-code").is_some(), "the hunk's tail stands over the comments");
}

#[gpui_kit::test]
fn resolving_folds_the_thread(cx: &mut TestAppContext) {
    let (thread, _, cx) = thread(cx, false);
    assert!(thread.read_with(cx, |t, _| t.is_open()));
    thread.update_in(cx, |t, window, cx| t.toggle_resolved(window, cx));
    cx.run_until_parked();
    assert!(thread.read_with(cx, |t, _| t.thread().resolved && !t.is_open()), "Resolve folds it");
}

#[gpui_kit::test]
fn a_reply_sends_through_the_hook_and_says_a_refusal(cx: &mut TestAppContext) {
    let (thread, sent, cx) = thread(cx, false);
    thread.update_in(cx, |t, window, cx| t.start_reply(window, cx));
    cx.run_until_parked();
    let field = thread.read_with(cx, |t, _| t.reply_field().cloned()).expect("Reply opens the field");
    let placeholder = field.read_with(cx, |f, cx| f.field().read(cx).presentation().placeholder().to_string());
    assert_eq!(placeholder, "Reply to octo");
    cx.simulate_input("please refuse");
    cx.simulate_keystrokes("secondary-enter");
    cx.run_until_parked();
    assert_eq!(*sent.borrow(), ["please refuse"]);
    assert_eq!(field.read_with(cx, |f, _| f.refusal().map(str::to_owned)), Some("GitHub said no".into()), "the refusal stands under it");
    assert!(thread.read_with(cx, |t, _| t.reply_field().is_some()), "and the words stay");
    let focus = field.read_with(cx, |f, cx| f.field().read(cx).focus_handle(cx));
    cx.update(|window, cx| focus.focus(window, cx));
    cx.simulate_keystrokes("secondary-a");
    cx.simulate_input("ship it");
    cx.simulate_keystrokes("secondary-enter");
    cx.run_until_parked();
    assert_eq!(sent.borrow().last().map(String::as_str), Some("ship it"));
    assert!(thread.read_with(cx, |t, _| t.reply_field().is_none()), "a sent reply closes its field");

    thread.update_in(cx, |t, window, cx| t.start_reply(window, cx));
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(thread.read_with(cx, |t, _| t.reply_field().is_none()), "Escape cancels the reply");
}
