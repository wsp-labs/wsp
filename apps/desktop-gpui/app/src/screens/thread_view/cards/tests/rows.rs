// SPDX-License-Identifier: AGPL-3.0-only

use super::*;
use crate::screens::thread_view::markdown;

#[gpui_kit::test]
fn an_unreached_stop_says_why_in_12_px_under_its_word(cx: &mut TestAppContext) {
    let said = "The stop did not reach box: its computer did not answer";
    let (_view, cx) = window(
        vec![
            base("session.start", 1, json!({"prompt": "fix it"})),
            base("session.done", 2, json!({"result": {"status": "interrupted", "error": said, "unreached": true}})),
            base("session.end", 3, json!({"exitCode": 0, "sawResult": true})),
        ],
        cx,
    );
    // The word's 16.5 px line, 2 px, the reason's 16 px line, then the pair's own 8 px foot.
    assert_eq!(bounds(cx, "settled-why").size.height, px(16.5 + 2. + 16. + 8.));
}

#[gpui_kit::test]
fn an_open_calls_body_takes_the_code_size_the_person_picked(cx: &mut TestAppContext) {
    let (view, cx) = window(
        vec![
            base("session.start", 1, json!({"prompt": "go", "at": 1_000.0})),
            base(
                "session.delta",
                2,
                json!({"kind": "tool_use", "toolName": "mcp__docs__lookup", "toolUseId": "g", "text": "{\"topic\":\"lists\"}"}),
            ),
            base("session.delta", 3, json!({"kind": "tool_result", "toolUseId": "g", "text": "one line of the answer"})),
            base("session.delta", 4, json!({"kind": "text", "text": "On it.", "at": 3_000.0})),
        ],
        cx,
    );
    let group = view.read_with(cx, |v, _| {
        v.rows().iter().find_map(|r| match &r.kind {
            RowKind::Group { group, .. } => Some(group.clone()),
            _ => None,
        })
    });
    view.update(cx, |v, cx| v.toggle(&group.expect("no group of calls"), cx));
    cx.run_until_parked();
    let call = view.read_with(cx, |v, _| {
        v.rows().iter().find_map(|r| match &r.kind {
            RowKind::Calls { calls, .. } => calls.first().map(|c| c.key.clone()),
            _ => None,
        })
    });
    view.update(cx, |v, cx| v.toggle(&call.expect("no call in the open group"), cx));
    cx.run_until_parked();
    let height = |cx: &mut VisualTestContext| f32::from(bounds(cx, "call-detail").size.height);
    assert!((height(cx) - 17.875).abs() < 0.5, "11 px at leading-relaxed where no code size is picked: {}", height(cx));
    cx.update(|_, cx| markdown::set_sizes(&json!({"codeSize": 16}), cx));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!((height(cx) - 26.).abs() < 0.5, "the body did not take the picked code size: {} px high", height(cx));
}
