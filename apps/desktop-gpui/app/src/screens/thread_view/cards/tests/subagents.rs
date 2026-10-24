// SPDX-License-Identifier: AGPL-3.0-only

use super::*;
use crate::screens::thread_view::cards::subagent_fold::PAINTED;
use crate::screens::thread_view::find;

#[gpui_kit::test]
fn a_subagent_fold_opens_its_lines_on_the_rail(cx: &mut TestAppContext) {
    let lead = Listing { thread: "t".into(), workspace: Some("w".into()), ..Listing::default() };
    let (view, cx) = window_listed(subagent(), Some(lead), cx);
    let row = frame(&view, cx, |k| matches!(k, RowKind::Subagent { .. }));
    let trigger = bounds(cx, "subagent-trigger");
    let page = bounds(cx, "subagent-page");
    // `px-1`; the title's 20 px line; the `icon-micro` arrow 6 px after it; shut, the row's `pb-4`.
    assert_eq!((trigger.left() - row.left(), trigger.size.height, row.size.height), (px(4.), px(20.), px(36.)));
    assert_eq!((page.size, page.left() - trigger.right()), (size(px(20.), px(20.)), px(6.)));
    cx.simulate_click(trigger.center(), Modifiers::none());
    cx.run_until_parked();
    let body = bounds(cx, "subagent-body");
    // The rail hangs from the chevron's centre (`ml-1.5`) and the lines start under the title (`pl-[11px]`, `pt-1`).
    assert_eq!((body.left() - trigger.left(), body.top()), (px(6.), trigger.bottom()));
    // Prose, the call over its answer with no gap between the two, prose: `gap-1` only between the lines.
    let lines = px(4. + 22.75 + 4. + 16. + 16. + 4. + 22.75);
    assert!((body.size.height - lines).abs() < px(1.), "the fold's lines are {:?}, the web's {lines:?}", body.size.height);
}

#[gpui_kit::test]
fn open_its_page_moves_to_the_subagents_page(cx: &mut TestAppContext) {
    let (no_lead, cx) = window(subagent(), cx);
    assert!(cx.debug_bounds("subagent-page").is_none(), "Open its page stands before the lead's page has an address");
    drop(no_lead);
    let lead = Listing { thread: "t".into(), workspace: Some("w".into()), ..Listing::default() };
    let view = cx.new(ThreadView::new);
    cx.update(|window, cx| window.replace_root(cx, |window, cx| Root::new(view.clone(), window, cx)));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.set_listing(lead, cx);
        v.load(subagent(), cx);
    });
    cx.run_until_parked();
    ROUTED.with(|r| r.borrow_mut().clear());
    let at = bounds(cx, "subagent-page").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(routed(), [Route::Screen("subagent:w/t/t".into())]);
    assert!(
        view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Subagent { open: false, .. }))),
        "the press opened the fold too"
    );
}

#[gpui_kit::test]
fn find_paints_its_match_in_a_subagents_line(cx: &mut TestAppContext) {
    let mut events = subagent();
    events[2] = base("session.delta", 3, json!({"kind": "text", "text": "Reading the kiwi notes.", "parentToolUseId": "t"}));
    let (view, cx) = window(events, cx);
    view.update(cx, |v, cx| v.toggle("subagent:t", cx));
    cx.update(|window, cx| view.update(cx, |v, cx| find::open(v, window, cx)));
    cx.simulate_input("kiwi");
    PAINTED.with(|p| p.borrow_mut().clear());
    view.update(cx, |_, cx| cx.notify());
    cx.run_until_parked();
    let painted = PAINTED.with(|p| p.borrow().clone());
    assert!(
        painted.iter().any(|(key, part, spans)| key == "subagent:t"
            && *part == find::subagent_part(0, false)
            && spans.len() == 1
            && spans[0] == (12..16)),
        "the subagent's line was drawn without its match: {painted:?}"
    );
}

#[gpui_kit::test]
fn a_subagent_pages_calls_stand_grouped_as_the_threads_own_do(cx: &mut TestAppContext) {
    let mut events = subagent();
    events.insert(
        4,
        base(
            "session.delta",
            7,
            json!({"kind": "tool_use", "toolName": "Read", "toolUseId": "d", "parentToolUseId": "t", "text": "{\"file_path\":\"src/c.rs\"}"}),
        ),
    );
    events
        .insert(5, base("session.delta", 8, json!({"kind": "tool_result", "toolUseId": "d", "parentToolUseId": "t", "text": "fn c() {}"})));
    let (view, cx) = window(events, cx);
    view.update(cx, |v, cx| v.set_page(Some("t".into()), cx));
    cx.run_until_parked();
    let rows = view.read_with(cx, |v, _| v.rows().to_vec());
    let kinds: Vec<String> = rows.iter().map(|r| format!("{}: {}", r.key, r.text)).collect();
    let group =
        rows.iter().find(|r| matches!(r.kind, RowKind::Group { .. })).unwrap_or_else(|| panic!("no group of calls on the page: {kinds:?}"));
    assert_eq!(group.text, "Read 2 files", "{kinds:?}");
    assert!(!rows.iter().any(|r| matches!(r.kind, RowKind::Work { tone: transcript::Tone::Tool, .. })), "a call stands alone: {kinds:?}");
    assert!(rows.iter().any(|r| matches!(r.kind, RowKind::Agent { .. }) && r.text == "Reading."), "{kinds:?}");
    assert!(!rows.iter().any(|r| matches!(r.kind, RowKind::Working { .. } | RowKind::Fold { .. } | RowKind::Thinking)), "{kinds:?}");
    view.update(cx, |v, cx| {
        v.toggle(
            &match &group.kind {
                RowKind::Group { group, .. } => group.clone(),
                _ => unreachable!(),
            },
            cx,
        )
    });
    assert!(
        view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Calls { .. }))),
        "the group did not open on the page"
    );
}
