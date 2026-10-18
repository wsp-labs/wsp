// SPDX-License-Identifier: AGPL-3.0-only
//! The review draft and the Review with an agent dialog against a throwaway host. The draft is the host's own, read
//! off a reviewer's reply: the host's gh is the stub in `pr_pane/gh/` (with this part's answers in
//! `pr_pane/gh/pr_review.cjs`) and its claude answers a review prompt with the fenced json block a reviewer ends with,
//! so Post goes through the stub and nothing reaches GitHub.

#[path = "../../host-client/tests/common/mod.rs"]
mod common;
// The Pull request pane's host helper, shared; this file uses part of it.
#[allow(dead_code)]
#[path = "pr_pane/pr_page_host.rs"]
mod pr_page_host;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use common::Throwaway;
use desktop_gpui::screens::pr_pane::pull_request::review::{Held, ReviewDialog, ReviewDraftView, Verdict};
use desktop_gpui::screens::Host;
use desktop_gpui::ui::{self, Button, Side};
use gpui_kit::component::Root;
use gpui_kit::component::WindowExt as _;
use gpui_kit::{div, prelude::*, px, size, Context, Entity, Modifiers, TestAppContext, VisualTestContext, Window};
use pr_page_host::PrHost;
use serde_json::json;
use wsp_host_client::protocol;

const WAIT: Duration = Duration::from_secs(30);

/// A claude that answers every prompt with a review of the stub's pull request in the block a reviewer ends with.
const REVIEWER: &str = r#"#!/bin/sh
case " $* " in
  *" --version "*) echo "2.1.295 (Claude Code)"; exit 0 ;;
  *" --help "*) echo '--permission-mode <mode>  (choices: "default", "dontAsk", "bypassPermissions")'; exit 0 ;;
  *" --bare "*) read -r _; echo '{"type":"control_response","response":{"subtype":"success","request_id":"init","response":{"models":[{"value":"default","resolvedModel":"claude-haiku-5-5"}]}}}'; exit 0 ;;
  *" --input-format stream-json "*) ;;
  *) exit 1 ;;
esac
session=""; prev=""
for a in "$@"; do [ "$prev" = "--session-id" ] && session=$a; prev=$a; done
review='Looks fine.\n\n```json\n{\"verdict\":\"request_changes\",\"summary\":\"Two notes.\",\"comments\":[{\"path\":\"src/pane.rs\",\"line\":7,\"side\":\"RIGHT\",\"body\":\"Name the field.\"},{\"path\":\"src/pane.rs\",\"line\":1,\"side\":\"RIGHT\",\"body\":\"Keep main small.\"},{\"path\":\"src/pane.rs\",\"line\":2,\"side\":\"RIGHT\",\"body\":\"Say why.\"}]}\n```'
while IFS= read -r _; do
  printf '{"type":"system","subtype":"init","session_id":"%s","model":"claude-haiku-5-5","cwd":"/","tools":[]}\n' "$session"
  printf '{"type":"assistant","session_id":"%s","message":{"role":"assistant","content":[{"type":"text","text":"%s"}]}}\n' "$session" "$review"
  printf '{"type":"result","subtype":"success","is_error":false,"result":"%s","session_id":"%s"}\n' "$review" "$session"
done
"#;

fn init(cx: &mut TestAppContext) {
    cx.executor().allow_parking();
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

fn until(cx: &mut VisualTestContext, what: &str, done: impl Fn(&mut VisualTestContext) -> bool) {
    let limit = Instant::now() + WAIT;
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < limit, "never got to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The page under the dialog: one button, counting the presses that reach it.
struct Page {
    pressed: Rc<Cell<u32>>,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pressed = self.pressed.clone();
        div().size_full().child(
            div()
                .debug_selector(|| "under".into())
                .absolute()
                .top(px(560.))
                .left(px(40.))
                .child(Button::outline("under", "Under").on_click(move |_, _, _| pressed.set(pressed.get() + 1))),
        )
    }
}

fn throwaway_host() -> (Throwaway, Host) {
    let server = Throwaway::start();
    let runtime = Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap());
    let client = Arc::new(runtime.block_on(server.client()));
    (server, Host::new(client, runtime))
}

fn dialog<'a>(cx: &'a mut TestAppContext, host: &Host) -> (Entity<ReviewDialog>, Rc<Cell<u32>>, &'a mut VisualTestContext) {
    init(cx);
    let pressed = Rc::new(Cell::new(0));
    let page = pressed.clone();
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|_| Page { pressed: page });
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1000.), px(800.)));
    cx.run_until_parked();
    let opened = cx.update(|window, cx| ReviewDialog::open(host, "no-such-workspace", window, cx));
    cx.run_until_parked();
    (opened, pressed, cx)
}

fn open(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| window.has_active_dialog(cx))
}

#[gpui_kit::test]
fn the_dialog_occludes_keeps_a_press_inside_and_closes_on_escape(cx: &mut TestAppContext) {
    let (_server, host) = throwaway_host();
    let (dialog, pressed, cx) = dialog(cx, &host);
    assert!(open(cx), "Review with an agent opens over the pane");
    assert_eq!(dialog.read_with(cx, |d, _| d.agent()), "codex", "Codex first, by the owner's rule");
    let title = cx.debug_bounds("dialog-title").expect("the dialog's title is drawn");
    cx.simulate_click(title.center(), Modifiers::none());
    assert!(open(cx), "a press inside keeps it open");
    let under = cx.debug_bounds("under").expect("the page's button");
    cx.simulate_click(under.center(), Modifiers::none());
    assert_eq!(pressed.get(), 0, "a press over the page does not reach it");
    if !open(cx) {
        cx.update(|window, cx| ReviewDialog::open(&host, "no-such-workspace", window, cx));
        cx.run_until_parked();
    }
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(!open(cx), "Escape closes it");
}

#[gpui_kit::test]
fn start_says_the_hosts_refusal_and_stays_open(cx: &mut TestAppContext) {
    let (_server, host) = throwaway_host();
    let (dialog, _, cx) = dialog(cx, &host);
    dialog.update_in(cx, |d, window, cx| {
        d.pick(1, cx);
        d.start(window, cx);
    });
    until(cx, "the host's refusal", |cx| dialog.read_with(cx, |d, _| d.refusal().is_some()));
    assert_eq!(dialog.read_with(cx, |d, _| d.agent()), "claude");
    assert!(open(cx), "a refused start keeps the dialog open");
}

struct Pane {
    draft: Entity<ReviewDraftView>,
    /// The workspace the review was asked from, which holds no draft.
    other: Entity<ReviewDraftView>,
}

impl Render for Pane {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(560.)).child(self.draft.clone()).child(self.other.clone())
    }
}

/// A review workspace whose reviewer's reply left a draft: request changes, a comment on a line of the diff and one
/// outside it.
fn reviewed() -> (PrHost, String) {
    let host = PrHost::start("open");
    let home = host.server.dir.path();
    assert!(!host.pull_request().is_null(), "the workspace reads its pull request");
    // A review on this computer runs in a worktree on the pull request's branch.
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(&host.folder)
            .env_clear()
            .envs(host.server.env())
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    };
    git(&["switch", "-q", "main"]);
    git(&["worktree", "add", "-q", &home.join("work-tree").to_string_lossy(), "work"]);
    std::fs::write(host.server.dir.path().join("bin/claude"), REVIEWER).unwrap();
    let client = host.client.clone();
    let ask: protocol::WorkspacesReviewRequest =
        serde_json::from_value(json!({ "workspaceId": host.workspace_id, "agent": "claude" })).unwrap();
    let made =
        host.runtime.block_on(async move { client.call(ask).await }).unwrap_or_else(|f| panic!("{}\n{}", f.message, host.server.log()));
    let review = made.workspace.id;
    let limit = Instant::now() + WAIT;
    loop {
        let client = host.client.clone();
        let ask: protocol::WorkspacesReviewDraftRequest = serde_json::from_value(json!({ "workspaceId": review })).unwrap();
        let read = host.runtime.block_on(async move { client.call(ask).await }).map(|d| Held::of(d.review));
        if matches!(read, Ok(Held::Read(_))) {
            break;
        }
        if Instant::now() >= limit {
            let client = host.client.clone();
            let list: protocol::SessionsListRequest = serde_json::from_value(json!({})).unwrap();
            let sessions = host.runtime.block_on(async move { client.call(list).await }).map(|l| {
                serde_json::to_value(&l.sessions)
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| {
                        format!(
                            "{} ws={} status={} last={} prompt={:.80}",
                            s["id"],
                            s["workspaceId"],
                            s["status"],
                            s["lastLine"],
                            s["prompt"].to_string()
                        )
                    })
                    .collect::<Vec<_>>()
            });
            panic!("the reviewer's reply never read as a draft: {read:?}\n{}\n{sessions:?}", host.server.log());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    (host, review)
}

#[gpui_kit::test]
fn the_draft_reads_ticks_posts_and_redraws_on_the_push(cx: &mut TestAppContext) {
    let (host, review) = reviewed();
    init(cx);
    let screen_host = host.host();
    let from = host.workspace_id.clone();
    let mut held = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let draft = cx.new(|cx| ReviewDraftView::new(screen_host.clone(), review.clone(), window, cx));
        let other = cx.new(|cx| ReviewDraftView::new(screen_host.clone(), from.clone(), window, cx));
        held = Some((draft.clone(), other.clone()));
        let pane = cx.new(|_| Pane { draft, other });
        Root::new(pane, window, cx)
    });
    let (draft, other) = held.unwrap();
    cx.simulate_resize(size(px(800.), px(1200.)));
    let read = |cx: &mut VisualTestContext| match draft.read_with(cx, |d, _| d.held().clone()) {
        Held::Read(d) => Some(d),
        _ => None,
    };
    until(cx, "the host's draft", |cx| read(cx).is_some());
    let first = read(cx).unwrap();
    assert_eq!(first.verdict, Verdict::RequestChanges);
    assert_eq!(first.comments.len(), 3);
    until(cx, "the other workspace's answer", |cx| {
        other.read_with(cx, |d, _| matches!(d.held(), Held::Working)) && cx.debug_bounds("draft-still-working").is_some()
    });

    assert!(cx.debug_bounds("draft-moved-on").is_none(), "a draft on the pull request's head says nothing of it");
    draft.update(cx, |d, cx| d.set_head_oid(Some("0000000000000000000000000000000000000000".into()), cx));
    cx.run_until_parked();
    assert!(cx.debug_bounds("draft-moved-on").is_some(), "a head that moved since says so");
    draft.update(cx, |d, cx| d.set_head_oid(Some(first.head_oid.clone()), cx));
    cx.run_until_parked();
    assert!(first.comments.iter().all(|c| c.on), "every comment starts ticked");
    assert_eq!(
        first.comments.iter().map(|c| c.in_summary).collect::<Vec<_>>(),
        [false, true, true],
        "a line outside the diff goes into the summary"
    );
    let summary = draft.read_with(cx, |d, cx| d.summary_field().read(cx).value().to_string());
    assert_eq!(summary, "Two notes.", "the summary field holds the draft's");
    assert!(cx.debug_bounds("draft-comment").is_some(), "a row per comment");

    // Another window unticks the second comment: the host's push draws it here.
    let client = host.client.clone();
    let id = first.comments[1].id.clone();
    let untick: protocol::WorkspacesReviewDraftRequest =
        serde_json::from_value(json!({ "workspaceId": review, "on": [{ "id": id, "on": false }] })).unwrap();
    host.runtime.block_on(async move { client.call(untick).await }).unwrap();
    until(cx, "the push's redraw", |cx| read(cx).is_some_and(|d| !d.comments[1].on));

    // A summary typed, then a tick at once: the blur's save and the tick's are both in flight, and both land.
    let field = cx.debug_bounds("text-area").expect("the summary field");
    cx.simulate_click(field.center(), Modifiers::none());
    cx.simulate_keystrokes("end");
    cx.simulate_input(" More.");
    let tick = cx.debug_bounds("draft-tick-0").expect("the first row's tick");
    cx.simulate_click(tick.center(), Modifiers::none());
    until(cx, "the untick saved", |cx| read(cx).is_some_and(|d| !d.comments[0].on));
    let saved = |host: &PrHost| {
        let client = host.client.clone();
        let ask: protocol::WorkspacesReviewDraftRequest = serde_json::from_value(json!({ "workspaceId": review })).unwrap();
        match host.runtime.block_on(async move { client.call(ask).await }).map(|d| Held::of(d.review)) {
            Ok(Held::Read(d)) => d.summary,
            other => panic!("{other:?}"),
        }
    };
    until(cx, "the summary saved beside the tick", |_| saved(&host) == "Two notes. More.");
    draft.update_in(cx, |d, window, cx| d.tick(&first.comments[0].id, true, window, cx));
    until(cx, "the tick saved", |cx| read(cx).is_some_and(|d| d.comments[0].on));

    draft.update_in(cx, |d, window, cx| d.pick_verdict(Verdict::Comment, window, cx));
    until(cx, "the verdict saved", |cx| read(cx).is_some_and(|d| d.verdict == Verdict::Comment));

    draft.update_in(cx, |d, window, cx| d.post(window, cx));
    until(cx, "Posted", |cx| read(cx).is_some_and(|d| d.posted.is_some()));
    assert_eq!(read(cx).unwrap().posted.unwrap().folded, 1, "the ticked comment outside the diff went into the summary");
    assert!(cx.debug_bounds("draft-folded").is_some(), "and the foot says so");
    assert!(cx.debug_bounds("draft-posted").is_some(), "the foot says Posted in place of Post");
    let posts = host.calls_with(&["--method", "POST"]);
    let posted = posts
        .iter()
        .find(|c| c["argv"].as_array().is_some_and(|a| a.iter().any(|w| w.as_str().is_some_and(|w| w.ends_with("/reviews")))))
        .expect("the review went to gh");
    let body: serde_json::Value = serde_json::from_str(posted["stdin"].as_str().unwrap()).unwrap();
    let paths: Vec<_> = body["comments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c["path"].as_str().unwrap().to_owned(), c["line"].as_u64().unwrap()))
        .collect();
    assert_eq!(paths, [("src/pane.rs".to_owned(), 7)], "only the ticked rows go: {body:#}");
    assert_eq!(body["event"], "COMMENT", "the verdict picked here");
    let said = body["body"].as_str().unwrap();
    assert!(said.contains("Two notes. More.") && said.contains("Say why.") && !said.contains("Keep main small."), "{said}");
}
