// SPDX-License-Identifier: AGPL-3.0-only
//! A review workspace's draft in its Pull request pane (`pull-request/ReviewDraftSection.tsx`) and the Review with an
//! agent dialog (`ReviewDialog.tsx`). The draft is the host's: every verdict, tick and summary is saved there as it
//! changes (`workspaces.reviewDraft`), the host's `workspace.review` push draws it again, and nothing reaches GitHub
//! until Post (`workspaces.reviewPost`), which the host sends through this computer's gh.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, FontWeight, MouseButton, SharedString, Subscription, Task, Window};
use wsp_host_client::protocol::{
    ReviewDraft, ReviewDraftVariant0Verdict, WorkspacesReviewDraftRequest, WorkspacesReviewDraftRequestOnItem,
    WorkspacesReviewDraftRequestVerdict, WorkspacesReviewPostRequest, WorkspacesReviewRequest,
};
use wsp_host_client::{protocol::EventUnion, Event};

use super::parts::restricted;
use crate::screens::composer::model::ago;
use crate::screens::dialogs::layer::{self, Closes};
use crate::screens::pr_pane::comments::{send_to_thread, SEND_TO_THREAD};
use crate::screens::pr_pane::files::markdown::{on_link, open_web};
use crate::screens::{navigate, Host, Route};
use crate::ui::controls::checkbox;
use crate::ui::{self, size, ActivePalette as _, Button, Dialog, Refusal, Segmented, TextArea};

/// The words, as `START_WORDS`, `PR_WORDS` and the two files say them.
pub mod words {
    pub const HEAD: &str = "Draft review";
    pub const STILL_WORKING: &str = "The reviewer is still working";
    pub const MOVED_ON: &str = "the pull request moved on since the review";
    pub const TO_SUMMARY: &str = "outside the diff, goes into the summary";
    pub const POST: &str = "Post review";
    pub const SUMMARY: &str = "Summary";
    pub const REVIEW_WITH_AGENT: &str = "Review with an agent";
    pub const REVIEW_NOTE: &str =
        "Reads the pull request in a worktree on its branch and changes nothing. Its review waits here until you post it.";
    pub const CANCEL: &str = "Cancel";
    pub const START: &str = "Start";

    /// What a post says, as `wsp review --post` prints it (`START_WORDS.posted`).
    pub fn posted(workspace: &str, number: u64, comments: u64, folded: u64) -> String {
        format!(
            "{workspace}: review posted on #{number} ({comments} comment{}, {folded} in the summary)",
            if comments == 1 { "" } else { "s" }
        )
    }
}

/// The reviewers the dialog offers, Codex first by the owner's rule, with the names the catalog gives them.
pub const REVIEWERS: [(&str, &str); 2] = [("codex", "Codex"), ("claude", "Claude")];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Comment,
    Approve,
    RequestChanges,
}

impl Verdict {
    pub const ALL: [Verdict; 3] = [Verdict::Comment, Verdict::Approve, Verdict::RequestChanges];

    pub fn word(self) -> &'static str {
        match self {
            Verdict::Comment => "Comment",
            Verdict::Approve => "Approve",
            Verdict::RequestChanges => "Request changes",
        }
    }

    fn wire(self) -> WorkspacesReviewDraftRequestVerdict {
        match self {
            Verdict::Comment => WorkspacesReviewDraftRequestVerdict::Comment,
            Verdict::Approve => WorkspacesReviewDraftRequestVerdict::Approve,
            Verdict::RequestChanges => WorkspacesReviewDraftRequestVerdict::RequestChanges,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DraftComment {
    pub id: String,
    pub path: String,
    pub line: u64,
    pub body: String,
    pub on: bool,
    pub in_summary: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Posted {
    pub url: String,
    pub at_ms: f64,
    pub folded: usize,
}

/// A draft the reviewer's reply read into.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub verdict: Verdict,
    pub summary: String,
    pub comments: Vec<DraftComment>,
    pub head_oid: String,
    pub thread_id: String,
    pub posted: Option<Posted>,
}

/// What the host holds for the workspace: nothing yet (the reviewer is still working), the sentence a reply that did
/// not read left, or the draft.
#[derive(Clone, Debug, PartialEq)]
pub enum Held {
    Working,
    Note(String),
    Read(Draft),
}

impl Held {
    pub fn of(review: Option<ReviewDraft>) -> Held {
        match review {
            None => Held::Working,
            Some(ReviewDraft::Variant1 { note, .. }) => Held::Note(note),
            Some(ReviewDraft::Variant0 { verdict, summary, comments, head_oid, thread_id, posted, .. }) => Held::Read(Draft {
                verdict: match verdict {
                    // A verdict this release does not know posts as the one that approves and blocks nothing.
                    ReviewDraftVariant0Verdict::Comment | ReviewDraftVariant0Verdict::Unknown => Verdict::Comment,
                    ReviewDraftVariant0Verdict::Approve => Verdict::Approve,
                    ReviewDraftVariant0Verdict::RequestChanges => Verdict::RequestChanges,
                },
                summary,
                comments: comments
                    .into_iter()
                    .map(|c| DraftComment {
                        id: c.id,
                        path: c.path,
                        line: c.line.get(),
                        body: c.body,
                        on: c.on,
                        in_summary: c.in_summary == Some(true),
                    })
                    .collect(),
                head_oid,
                thread_id,
                posted: posted.map(|p| Posted { url: p.url, at_ms: p.at, folded: p.folded.len() }),
            }),
        }
    }
}

/// "Posted 3h ago", by the composer's own reading of an elapsed time.
fn posted_ago(at_ms: f64) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.);
    ago(Duration::from_millis((now - at_ms).max(0.) as u64))
}

/// A comment's words as the draft row reads them: the reviewer wrote them, so an image stands as its alt words and is
/// never fetched.
pub fn shown_body(body: &str) -> String {
    restricted(body)
}

/// What one save changes.
#[derive(Default)]
struct Edits {
    summary: Option<String>,
    verdict: Option<Verdict>,
    on: Option<(String, bool)>,
}

/// The draft section of a review workspace's pane: the verdict, the summary, a row per comment and Post. It holds
/// the one workspace's draft as the host last sent it, its summary field, and the one listener on the host's pushes.
pub struct ReviewDraftView {
    host: Host,
    workspace_id: String,
    /// The workspace's name, which the post's toast leads with.
    name: String,
    held: Held,
    /// The pull request's head as the pane reads it; a draft written against another says so.
    head_oid: Option<String>,
    summary: Entity<TextareaState>,
    /// The summary the field was last given from the host, so a push does not overwrite what the person typed.
    given: Option<String>,
    posting: bool,
    refusal: Option<String>,
    said: Option<String>,
    _summary: Subscription,
    _load: Task<()>,
    _post: Task<()>,
    _events: Task<()>,
}

impl ReviewDraftView {
    pub fn new(host: Host, workspace_id: impl Into<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let workspace_id = workspace_id.into();
        let summary = TextArea::state("", window, cx);
        let blur = cx.subscribe_in(&summary, window, |this: &mut Self, _, e: &InputEvent, window, cx| match e {
            InputEvent::Blur => this.save_summary(window, cx),
            InputEvent::Change => cx.notify(),
            _ => {}
        });
        let mut view = ReviewDraftView {
            host,
            workspace_id,
            name: String::new(),
            held: Held::Working,
            head_oid: None,
            summary,
            given: None,
            posting: false,
            refusal: None,
            said: None,
            _summary: blur,
            _load: Task::ready(()),
            _post: Task::ready(()),
            _events: Task::ready(()),
        };
        view.follow(window, cx);
        view.load(window, cx);
        view
    }

    pub fn held(&self) -> &Held {
        &self.held
    }

    pub fn summary_field(&self) -> &Entity<TextareaState> {
        &self.summary
    }

    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    pub fn is_posting(&self) -> bool {
        self.posting
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// What the last post's toast said.
    pub fn said(&self) -> Option<&str> {
        self.said.as_deref()
    }

    /// The pull request's head oid as the pane last read it.
    pub fn set_head_oid(&mut self, head_oid: Option<String>, cx: &mut Context<Self>) {
        if self.head_oid != head_oid {
            self.head_oid = head_oid;
            cx.notify();
        }
    }

    /// Reads the draft again from the host.
    pub fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let request = WorkspacesReviewDraftRequest { workspace_id: self.workspace_id.clone(), on: None, summary: None, verdict: None };
        let asking = self.host.run(move |client| async move { client.call(request).await });
        self._load = cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            _ = this.update_in(cx, |this, window, cx| match answer {
                Ok(reply) => this.take(Held::of(reply.review), window, cx),
                Err(failure) => {
                    this.refusal = Some(failure.message);
                    cx.notify();
                }
            });
        });
    }

    fn take(&mut self, held: Held, window: &mut Window, cx: &mut Context<Self>) {
        if let Held::Read(draft) = &held {
            let typed = self.summary.read(cx).value().to_string();
            let untouched = self.given.as_deref().is_none_or(|given| given == typed);
            if untouched && self.given.as_deref() != Some(draft.summary.as_str()) {
                let summary = draft.summary.clone();
                self.summary.update(cx, |s, cx| s.set_value(summary, window, cx));
            }
            self.given = Some(draft.summary.clone());
        }
        self.held = held;
        cx.notify();
    }

    /// Hears the host's `workspace.review` for this workspace and reads the draft again.
    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (tx, mut rx) = mpsc::unbounded::<()>();
        let id = self.workspace_id.clone();
        let listening = self.host.run(move |client| async move {
            let mut events = client.events().await?;
            while let Some(event) = events.next().await {
                if let Ok(Event::Host(event)) = event {
                    if matches!(*event, EventUnion::WorkspaceReview { ref workspace_id, .. } if *workspace_id == id)
                        && tx.unbounded_send(()).is_err()
                    {
                        break;
                    }
                }
            }
            Ok(())
        });
        self._events = cx.spawn_in(window, async move |this, cx| {
            let _listening = listening;
            while rx.next().await.is_some() {
                if this.update_in(cx, |this, window, cx| this.load(window, cx)).is_err() {
                    break;
                }
            }
        });
    }

    fn edit(&mut self, edits: Edits, window: &mut Window, cx: &mut Context<Self>) {
        let request = WorkspacesReviewDraftRequest {
            workspace_id: self.workspace_id.clone(),
            summary: edits.summary,
            verdict: edits.verdict.map(Verdict::wire),
            on: edits.on.map(|(id, on)| vec![WorkspacesReviewDraftRequestOnItem { id, on }]),
        };
        let asking = self.host.run(move |client| async move { client.call(request).await });
        // Each save runs until the host answers, so a tick right after a summary's blur does not drop the summary.
        cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            _ = this.update_in(cx, |this, window, cx| match answer {
                Ok(reply) => {
                    this.refusal = None;
                    if reply.review.is_some() {
                        this.take(Held::of(reply.review), window, cx);
                    }
                    cx.notify();
                }
                Err(failure) => {
                    this.refusal = Some(failure.message);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn save_summary(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Held::Read(draft) = &self.held else { return };
        let typed = self.summary.read(cx).value().to_string();
        if typed != draft.summary {
            self.edit(Edits { summary: Some(typed), ..Edits::default() }, window, cx);
        }
    }

    /// A press elsewhere in the draft blurs the summary first, as the web's field does, so what was typed saves too.
    pub fn pick_verdict(&mut self, verdict: Verdict, window: &mut Window, cx: &mut Context<Self>) {
        self.save_summary(window, cx);
        self.edit(Edits { verdict: Some(verdict), ..Edits::default() }, window, cx);
    }

    pub fn tick(&mut self, id: &str, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.save_summary(window, cx);
        self.edit(Edits { on: Some((id.to_owned(), on)), ..Edits::default() }, window, cx);
    }

    /// Whether Post can be pressed: a draft that is not posted, with a ticked comment or a summary, and no post
    /// running.
    pub fn can_post(&self, cx: &App) -> bool {
        let Held::Read(draft) = &self.held else { return false };
        let ticked = draft.comments.iter().any(|c| c.on);
        draft.posted.is_none() && !self.posting && (ticked || !self.summary.read(cx).value().trim().is_empty())
    }

    /// Saves a summary typed since the last save, then posts the draft through the host; the draft is read again
    /// after, which carries "Posted".
    pub fn post(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_post(cx) {
            return;
        }
        let Held::Read(draft) = &self.held else { return };
        let typed = self.summary.read(cx).value().to_string();
        let summary = (typed != draft.summary).then_some(typed);
        let id = self.workspace_id.clone();
        self.posting = true;
        self.refusal = None;
        let posting = self.host.run(move |client| async move {
            if let Some(summary) = summary {
                client
                    .call(WorkspacesReviewDraftRequest { workspace_id: id.clone(), summary: Some(summary), verdict: None, on: None })
                    .await?;
            }
            client.call(WorkspacesReviewPostRequest { workspace_id: id, thread_id: None }).await
        });
        self._post = cx.spawn_in(window, async move |this, cx| {
            let answer = posting.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.posting = false;
                match answer {
                    Ok(done) => {
                        let said = words::posted(&this.name, done.number, done.comments, done.folded);
                        ui::notify(cx, ui::Notice::done(said.clone()).place(this.name.clone()));
                        this.said = Some(said);
                    }
                    Err(failure) => this.refusal = Some(failure.message),
                }
                this.load(window, cx);
                cx.notify();
            });
        });
        cx.notify();
    }

    fn row(&self, i: usize, c: &DraftComment, thread_id: &str, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let place = format!("{}:{}", c.path, c.line);
        let quote = format!("About {place} in your review: {}", c.body);
        let key = thread_id.to_owned();
        let (id, on) = (c.id.clone(), c.on);
        h_flex()
            .id(SharedString::from(format!("draft-comment-{}", c.id)))
            .debug_selector(|| "draft-comment".into())
            .w_full()
            .min_w_0()
            .items_start()
            .gap_3()
            .rounded(size::BUTTON_RADIUS)
            .px_2()
            .py_2()
            .hover(|s| s.bg(p.accent))
            .child(
                div()
                    .id(SharedString::from(format!("draft-tick-{i}")))
                    .debug_selector(move || format!("draft-tick-{i}"))
                    .mt(px(2.))
                    .flex_none()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| this.tick(&id, !on, window, cx)))
                    .child(checkbox(SharedString::from(format!("draft-box-{i}")), c.on, false, window, cx)),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .font_family(cx.theme().mono_font_family.clone())
                            .text_size(px(13.))
                            .text_color(p.foreground)
                            .child(place),
                    )
                    .child(div().debug_selector(move || format!("draft-body-{i}")).text_size(px(13.)).line_height(px(20.)).child(
                        TextView::markdown(SharedString::from(format!("draft-body-{}", c.id)), shown_body(&c.body)).on_link_click(on_link),
                    ))
                    .when(c.in_summary, |el| {
                        el.child(div().text_size(px(11.)).line_height(px(14.)).text_color(p.muted_foreground).child(words::TO_SUMMARY))
                    })
                    .child(h_flex().mt_1().child(div().debug_selector(move || format!("draft-send-{i}")).child(
                        Button::ghost(SharedString::from(format!("draft-send-{i}")), SEND_TO_THREAD).xs().on_click(move |_, _, cx| {
                            if let Err(e) = send_to_thread(&key, &quote, cx) {
                                eprintln!("desktop-gpui: send to thread: {e}");
                            }
                        }),
                    ))),
            )
            .into_any_element()
    }
}

impl Render for ReviewDraftView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let quiet = |key: &'static str, words: SharedString| {
            div().debug_selector(move || key.into()).px_2().py_2().text_size(px(13.)).text_color(p.muted_foreground).child(words)
        };
        let mut body: Vec<AnyElement> = Vec::new();
        match self.held.clone() {
            Held::Working => body.push(quiet("draft-still-working", words::STILL_WORKING.into()).into_any_element()),
            Held::Note(note) => body.push(quiet("draft-note", note.into()).into_any_element()),
            Held::Read(draft) => {
                if self.head_oid.as_deref().is_some_and(|head| head != draft.head_oid) {
                    body.push(quiet("draft-moved-on", words::MOVED_ON.into()).into_any_element());
                }
                let picked = Verdict::ALL.iter().position(|v| *v == draft.verdict);
                let me = cx.entity();
                let verdicts = Verdict::ALL.iter().fold(Segmented::new("review-verdict", picked), |s, v| s.segment(v.word())).on_pick(
                    move |ix, window, cx| {
                        me.update(cx, |this, cx| this.pick_verdict(Verdict::ALL[ix], window, cx));
                    },
                );
                body.push(
                    v_flex()
                        .gap_3()
                        .px_2()
                        .py_2()
                        .child(div().flex().child(verdicts))
                        .child(TextArea::new("review-summary", &self.summary))
                        .into_any_element(),
                );
                for (i, c) in draft.comments.iter().enumerate() {
                    body.push(self.row(i, c, &draft.thread_id, window, cx));
                }
                let foot = h_flex().debug_selector(|| "draft-foot".into()).gap_3().px_2().pt_3();
                let foot = match &draft.posted {
                    Some(posted) => {
                        let url = posted.url.clone();
                        foot.child(
                            div()
                                .id("draft-posted")
                                .debug_selector(|| "draft-posted".into())
                                .cursor_pointer()
                                .text_size(px(13.))
                                .text_color(p.foreground)
                                .hover(|s| s.underline())
                                .on_click(move |_, _, cx| open_web(&url, cx))
                                .child(format!("Posted {}", posted_ago(posted.at_ms))),
                        )
                        .when(posted.folded > 0, |el| {
                            el.child(
                                div()
                                    .debug_selector(|| "draft-folded".into())
                                    .text_size(px(11.))
                                    .line_height(px(14.))
                                    .text_color(p.muted_foreground)
                                    .child(format!("{} went into the summary", posted.folded)),
                            )
                        })
                    }
                    None => foot.child(
                        Button::primary("draft-post", words::POST)
                            .held(!self.can_post(cx))
                            .busy(self.posting)
                            .on_click(cx.listener(|this, _, window, cx| this.post(window, cx))),
                    ),
                };
                body.push(foot.into_any_element());
            }
        }
        v_flex()
            .debug_selector(|| "review-draft".into())
            .mt(px(32.))
            .gap(px(2.))
            .child(
                div()
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .px_2()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .font_weight(FontWeight::NORMAL)
                    .text_color(p.muted_foreground)
                    .child(words::HEAD),
            )
            .children(body)
            .children(self.refusal.clone().map(|said| {
                div().debug_selector(|| "draft-refusal".into()).px_2().pt_2().text_size(px(13.)).text_color(p.error_foreground).child(said)
            }))
    }
}

/// The Review with an agent dialog's name on the dialogs' layer.
pub const REVIEW_DIALOG: &str = "review-with-agent";

/// The Review with an agent dialog's body: the reviewer, the note or the host's refusal, Cancel and Start. Start asks
/// the host for the reviewer's workspace and thread (`workspaces.review`) and opens that thread.
pub struct ReviewDialog {
    host: Host,
    workspace_id: String,
    agent: usize,
    busy: bool,
    refusal: Option<String>,
    _start: Task<()>,
}

impl ReviewDialog {
    pub fn new(host: Host, workspace_id: impl Into<String>) -> Self {
        ReviewDialog { host, workspace_id: workspace_id.into(), agent: 0, busy: false, refusal: None, _start: Task::ready(()) }
    }

    /// Stands the dialog on the dialogs' layer, which keeps the pane from presses and closes it on Escape or a press
    /// outside, a start on its way or not.
    pub fn open(host: &Host, workspace_id: &str, window: &mut Window, cx: &mut App) -> Entity<ReviewDialog> {
        let view = cx.new(|_| ReviewDialog::new(host.clone(), workspace_id));
        layer::open(REVIEW_DIALOG, view.clone().into(), Closes::Outside, None, window, cx);
        view
    }

    pub fn agent(&self) -> &'static str {
        REVIEWERS[self.agent].0
    }

    /// Whether a start is on its way.
    pub fn is_starting(&self) -> bool {
        self.busy
    }

    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    pub fn pick(&mut self, agent: usize, cx: &mut Context<Self>) {
        self.agent = agent.min(REVIEWERS.len() - 1);
        cx.notify();
    }

    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        self.busy = true;
        self.refusal = None;
        let request = WorkspacesReviewRequest {
            workspace_id: Some(self.workspace_id.clone()),
            agent: Some(self.agent().to_owned()),
            model: None,
            effort: None,
            url: None,
        };
        let asking = self.host.run(move |client| async move { client.call(request).await });
        self._start = cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            _ = this.update_in(cx, |this, window, cx| {
                this.busy = false;
                match answer {
                    Ok(made) => {
                        layer::close(REVIEW_DIALOG, window, cx);
                        navigate(Route::Thread { workspace_id: made.workspace.id, thread_id: made.thread_id }, cx);
                    }
                    Err(failure) => this.refusal = Some(failure.message),
                }
                cx.notify();
            });
        });
        cx.notify();
    }
}

impl Render for ReviewDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity();
        let reviewer = REVIEWERS.iter().fold(Segmented::new("reviewer", Some(self.agent)), |s, (_, name)| s.segment(*name)).on_pick(
            move |ix, _, cx| {
                me.update(cx, |this, cx| this.pick(ix, cx));
            },
        );
        let refusal = match &self.refusal {
            Some(said) => Refusal::new().said(said.clone()),
            None => Refusal::new().quiet(words::REVIEW_NOTE),
        };
        div().debug_selector(|| "review-dialog".into()).child(
            Dialog::new(words::REVIEW_WITH_AGENT)
                .layered()
                .on_outside(layer::dismiss)
                .child(v_flex().gap_2().child(div().flex().child(reviewer)).child(refusal))
                .act(
                    Button::outline("review-cancel", words::CANCEL)
                        .neutral_ring()
                        .on_click(|_, window, cx| layer::close(REVIEW_DIALOG, window, cx)),
                )
                .act(
                    Button::primary("review-start", words::START)
                        .held(self.busy)
                        .on_click(cx.listener(|this, _, window, cx| this.start(window, cx))),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drafts_image_is_never_fetched() {
        let shown = shown_body("Look ![pixel](https://tracker.example/p.png) here <img src=\"https://tracker.example/q.png\">");
        assert!(!shown.contains("https://"), "{shown}");
        assert!(shown.contains("pixel"), "the image stands as its alt words: {shown}");
    }
}
