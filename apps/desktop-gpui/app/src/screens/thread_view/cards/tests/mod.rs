// SPDX-License-Identifier: AGPL-3.0-only

mod changed_files;
mod files;
mod over_composer;
mod plans;
mod prompts;
mod rows;
mod rule_lines;
mod spawn;
mod subagents;
mod threads;

use gpui_kit::component::Root;
use gpui_kit::{size, Bounds, Entity, Modifiers, TestAppContext, VisualTestContext};
use serde_json::json;
use wsp_host_client::protocol::SessionEvent;

use super::prompt::SENT;
use super::spawn::ROUTED;
use super::threads::RAN;
use super::*;
use crate::screens::sidebar::model::Kind;
use crate::screens::Route;
use crate::transcript::{ChildPart, Listed, Listing, ThreadAt};

fn base(kind: &str, pos: u64, extra: serde_json::Value) -> SessionEvent {
    let mut v = json!({"type": kind, "workspaceId": "w", "sessionId": "s", "threadId": "t", "turnId": "u1", "pos": pos});
    v.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}

fn window(events: Vec<SessionEvent>, cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    window_listed(events, None, cx)
}

/// The window with what the listing says already standing when the transcript is read, as the lead's page has an
/// address once the view follows its thread.
fn window_listed(
    events: Vec<SessionEvent>,
    listing: Option<Listing>,
    cx: &mut TestAppContext,
) -> (Entity<ThreadView>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let view = cx.new(ThreadView::new);
    let layered = cx.new(|_| Layered(view.clone()));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(layered, window, cx));
    cx.simulate_resize(size(px(900.), px(1000.)));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        if let Some(listing) = listing {
            v.set_listing(listing, cx);
        }
        v.load(events, cx);
        v.scroll_to_top(cx);
    });
    cx.run_until_parked();
    (view, cx)
}

/// The view under the window's dialogs layer, as the shell draws the layer last.
struct Layered(Entity<ThreadView>);

impl Render for Layered {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.0.clone()).children(crate::screens::dialogs::layer::draw(window, cx))
    }
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn"))
}

/// The frame of the first row of a kind, which every card stands in.
fn frame(view: &Entity<ThreadView>, cx: &mut VisualTestContext, kind: fn(&RowKind) -> bool) -> Bounds<Pixels> {
    let ix = view.read_with(cx, |v, _| v.rows().iter().position(|r| kind(&r.kind)).expect("a row of that kind"));
    let selector: &'static str = Box::leak(format!("row-{ix}").into_boxed_str());
    bounds(cx, selector)
}

fn sent() -> Vec<(String, String, String)> {
    SENT.with(|s| s.borrow().iter().map(|r| (r.session_id.clone(), r.ask_id.clone(), r.option_id.clone())).collect())
}

/// A permission to run `cargo test --all`: the thread's own, or where `subagent` names its call, a subagent's.
fn permission_of(subagent: Option<&str>) -> Vec<SessionEvent> {
    let options = json!([{"id": "y", "label": "Allow", "effect": "allow"}, {"id": "n", "label": "Deny", "effect": "deny"}]);
    let input = "{\"command\":\"cargo test --all\",\"timeout\":5}";
    let mut asked = json!({"askId": "k", "toolName": "Bash", "toolUseId": "a", "input": input, "options": options});
    if let Some(call) = subagent {
        asked["parentToolUseId"] = json!(call);
    }
    vec![
        base("session.start", 1, json!({"prompt": "go"})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Bash", "toolUseId": "a", "text": input})),
        base("session.permission", 3, asked),
    ]
}

fn permission() -> Vec<SessionEvent> {
    permission_of(None)
}

#[gpui_kit::test]
fn a_closed_permission_is_one_record_line(cx: &mut TestAppContext) {
    let mut events = permission();
    events.push(base("session.permission.closed", 4, json!({"askId": "k", "outcome": "allowed", "optionId": "y"})));
    let (view, cx) = window(events, cx);
    let row = frame(&view, cx, |k| matches!(k, RowKind::Permission { .. }));
    let record = bounds(cx, "record");
    // `px-0.5 py-0.5` round the 24 px glyph slot, inside `pt-1 pb-2 px-1`, then the row's `pb-4`.
    assert_eq!((record.left() - row.left(), record.top() - row.top(), record.size.height), (px(4.), px(4.), px(28.)));
    // The last row also keeps the list's end space and the composer's inset under its foot.
    assert_eq!(row.bottom() - record.bottom(), px(8. + 16.) + super::super::rows::END_SPACE + super::super::rows::COMPOSER_INSET);
    assert!(cx.debug_bounds("options").is_none(), "a closed prompt offers nothing");
}

fn plan(text: &str) -> Vec<SessionEvent> {
    vec![
        base("session.start", 1, json!({"prompt": "plan it"})),
        base("session.plan", 2, json!({"text": text})),
        base("session.delta", 3, json!({"kind": "text", "text": "Here it is."})),
        base("session.done", 4, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        base("session.end", 5, json!({"exitCode": 0, "sawResult": true})),
    ]
}

fn subagent() -> Vec<SessionEvent> {
    let task = "{\"description\":\"Survey the code\",\"prompt\":\"look\"}";
    vec![
        base("session.start", 1, json!({"prompt": "go", "at": 1_000.0})),
        base("session.delta", 2, json!({"kind": "tool_use", "toolName": "Task", "toolUseId": "t", "text": task, "at": 2_000.0})),
        base("session.delta", 3, json!({"kind": "text", "text": "Reading.", "parentToolUseId": "t"})),
        base(
            "session.delta",
            4,
            json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "c", "parentToolUseId": "t", "text": "{\"file_path\":\"src/b.rs\"}"}),
        ),
        base("session.delta", 5, json!({"kind": "tool_result", "toolUseId": "c", "parentToolUseId": "t", "text": "fn b() {}"})),
        base("session.delta", 6, json!({"kind": "tool_result", "toolUseId": "t", "text": "Found it.", "at": 67_000.0})),
    ]
}

fn changes() -> Vec<SessionEvent> {
    vec![
        base("session.start", 1, json!({"prompt": "fix it"})),
        base("session.delta", 2, json!({"kind": "text", "text": "Done here."})),
        base("session.done", 3, json!({"result": {"status": "completed", "durationMs": 2_000}})),
        base("session.end", 4, json!({"exitCode": 0, "sawResult": true})),
        base(
            "session.changes",
            5,
            json!({"from": "a", "to": "b", "moved": [], "files": [
                {"path": "src/ui/a.rs", "kind": "modified", "additions": 3, "deletions": 1},
                {"path": "src/ui/c.rs", "kind": "modified", "additions": 1200, "deletions": 0},
                {"path": "b.rs", "kind": "added", "additions": 9, "deletions": 0}
            ]}),
        ),
    ]
}

fn listed(view: &Entity<ThreadView>, listing: Listing, cx: &mut VisualTestContext) {
    view.update(cx, |v, cx| v.set_listing(Listing { thread: "t".into(), ..listing }, cx));
    cx.run_until_parked();
}

fn routed() -> Vec<Route> {
    ROUTED.with(|r| r.borrow().clone())
}

fn child(thread: &str, kind: Kind, part: ChildPart) -> Listed {
    let at = ThreadAt { workspace: "w2".into(), thread: thread.into() };
    Listed { harness: "codex".into(), ..Listed::thread(at, &format!("Thread {thread}"), kind, part) }
}

fn ran() -> Vec<String> {
    RAN.with(|r| r.borrow().clone())
}

fn routed_after_clear() -> Vec<Route> {
    let out = routed();
    ROUTED.with(|r| r.borrow_mut().clear());
    out
}

#[gpui_kit::test]
fn a_subagent_page_is_the_lead_cut_to_its_lines(cx: &mut TestAppContext) {
    let (view, cx) = window(subagent(), cx);
    let task = "{\"description\":\"Survey the code\",\"prompt\":\"look\"}";
    assert!(task.contains("look"));
    view.update(cx, |v, cx| v.set_page(Some("t".into()), cx));
    cx.run_until_parked();
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    let shown: Vec<&str> = rows.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(shown.first(), Some(&"look"), "{shown:?}");
    assert!(matches!(rows[0].kind, RowKind::Person { .. }));
    assert!(rows.iter().any(|r| matches!(r.kind, RowKind::Agent { .. }) && r.text == "Reading."), "{shown:?}");
    assert!(!rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. } | RowKind::Fold { .. } | RowKind::Subagent { .. })), "{shown:?}");
    let page = view.read_with(cx, |v, _| v.transcript().page_of("t")).expect("the page knows no subagent");
    assert_eq!((page.title.as_str(), page.state.as_str()), ("Survey the code", "done"));
    assert_eq!(super::super::subagent::ended_line(&page).as_deref(), Some("Worked for 1m 5s"));
    assert_eq!(super::super::subagent::parse("w/t/call"), Some((ThreadAt { workspace: "w".into(), thread: "t".into() }, "call".into())));
    view.update(cx, |v, cx| v.set_page(None, cx));
    assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Subagent { .. }))));
}

#[gpui_kit::test]
fn a_subagents_page_follows_its_fold_before_the_listing(cx: &mut TestAppContext) {
    let lead = Listing {
        thread: "t".into(),
        subagents: vec![transcript::ListedSubagent {
            call: "t".into(),
            title: "Survey the code".into(),
            state: "running".into(),
            model: None,
            asked: None,
            started: 0.,
            ended: None,
            failure: None,
        }],
        ..Listing::default()
    };
    let (view, cx) = window_listed(subagent(), Some(lead), cx);
    let page = view.read_with(cx, |v, _| v.transcript().page_of("t")).unwrap();
    assert_eq!((page.state.as_str(), super::super::subagent::ended_line(&page).as_deref()), ("done", Some("Worked for 1m 5s")));
}
