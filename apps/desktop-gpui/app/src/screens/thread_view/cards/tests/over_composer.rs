// SPDX-License-Identifier: AGPL-3.0-only

use gpui_kit::App;

use super::*;

type Build = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

struct Piece(Build);

impl Render for Piece {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child((self.0)(window, cx))
    }
}

fn piece(cx: &mut TestAppContext, draw: impl Fn(&mut Window, &mut App) -> AnyElement + 'static) -> &mut VisualTestContext {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let (_, cx) = cx.add_window_view(|_, _| Piece(Box::new(draw)));
    cx.simulate_resize(size(px(900.), px(700.)));
    cx.run_until_parked();
    cx
}

#[gpui_kit::test]
fn the_strips_copy_exports_the_work_and_push_again_asks_the_thread(cx: &mut TestAppContext) {
    RAN.with(|r| r.borrow_mut().clear());
    let row = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": 1.0,
        "sub": {"state": "running", "on": "box", "branch": "wsp/fix", "held": {"files": 2, "commits": 1}}});
    let rows: Vec<crate::screens::sidebar::model::Row> = [row].into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
    let strip_of = Listing::listed(&rows, "t", 10.0, None, &json!({}), "Mac mini").strip.expect("no strip for work not pushed");
    assert_eq!(strip_of.acts, [transcript::StripAct::CopyHome("Mac mini".into()), transcript::StripAct::PushAgain("wsp/fix".into())]);
    let view = cx.new(ThreadView::new);
    view.update(cx, |v, cx| v.set_listing(Listing { thread: "t".into(), workspace: Some("w".into()), ..Listing::default() }, cx));
    let cx = piece(cx, move |_, cx| strip(&view, &strip_of, cx));
    let acts = cx.debug_bounds("sub-strip-not-pushed").expect("the strip");
    let _ = acts;
    let (copy, push) = (bounds(cx, "sub-strip-act-0"), bounds(cx, "sub-strip-act-1"));
    assert!(push.left() > copy.right(), "the two acts stand side by side under the words");
    cx.simulate_click(copy.center(), Modifiers::none());
    cx.simulate_click(push.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        ran(),
        ["export w", "send Push wsp/fix again, and say what the remote answers."],
        "Copy to did not export the work, or Push again asked nothing"
    );
}

#[gpui_kit::test]
fn the_setup_card_stands_centred_with_every_step_done(cx: &mut TestAppContext) {
    let setup = Setup { name: "spoo-landing".into(), steps: vec!["Making the copy".into(), "Ready".into()] };
    let cx = piece(cx, move |_, _| setup_card(&setup, px(120.)).into_any_element());
    let room = bounds(cx, "setting-up-room");
    assert_eq!(room.bottom(), px(700. - 120.), "the room ends at the composer");
    assert!(cx.debug_bounds("step-row").is_some() || cx.debug_bounds("card").is_some());
}

#[gpui_kit::test]
fn a_fork_on_its_way_says_loading_in_the_middle(cx: &mut TestAppContext) {
    let cx = piece(cx, |_, cx| loading(px(100.), cx).into_any_element());
    let shown = bounds(cx, "transcript-loading");
    assert_eq!((shown.top(), shown.size.height), (px(0.), px(700.)));
}

#[gpui_kit::test]
fn a_sub_thread_waiting_for_a_machine_says_so_over_its_composer(cx: &mut TestAppContext) {
    let row = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": 1.0,
        "capped": {"placeId": "p", "place": "box", "running": 3, "atOnce": 3}, "sub": {"state": "waiting", "on": "box", "spawn": {"startedAt": 1.0, "log": []}}});
    let strip_of = Listing::of(&[row], "t", 10.0).strip.expect("no strip for a capped sub-thread");
    assert_eq!(strip_of.title, "Waiting for a machine on Boat");
    assert_eq!(strip_of.note, "3 of 3 machines are running. It starts on its own when one frees.");
    assert_eq!(strip_of.acts, [transcript::StripAct::Raise(Some("p".into()))]);
    let view = cx.new(ThreadView::new);
    let cx = piece(cx, move |_, cx| strip(&view, &strip_of, cx));
    assert!(cx.debug_bounds("sub-strip-waiting-machines").is_some());
    assert_eq!(bounds(cx, "sub-strip-title").size.height, px(24.));
    ROUTED.with(|r| r.borrow_mut().clear());
    let raise = bounds(cx, "sub-strip-act-0");
    cx.simulate_click(raise.center(), Modifiers::none());
    cx.run_until_parked();
    let sent = routed();
    let arg = match sent.as_slice() {
        [Route::Screen(name)] => name.strip_prefix("settings:").unwrap_or_default().to_owned(),
        other => panic!("Raise did not open the computer's settings: {other:?}"),
    };
    let page = crate::screens::settings::route(Some(&arg)).map(|(at, _)| at);
    assert_eq!(page, Ok(Some(crate::screens::settings::model::At::Computer("p".into()))), "Raise names no Settings page");
}

#[gpui_kit::test]
fn a_sub_thread_whose_cloud_left_or_whose_machine_goes_says_so_over_its_composer(cx: &mut TestAppContext) {
    RAN.with(|r| r.borrow_mut().clear());
    let rows = |sub: serde_json::Value, status: &str, extra: serde_json::Value| {
        let mut row =
            json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": status, "threadId": "t", "startedAt": 1.0, "sub": sub});
        row.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let rows: Vec<crate::screens::sidebar::model::Row> =
            [row].into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
        Listing::listed(&rows, "t", 0.0, None, &json!({}), "Mac mini").strip
    };
    let deleting = rows(json!({"state": "running", "on": "box", "branch": "wsp/fix", "deleteAt": 90.0 * 60_000.}), "completed", json!({}));
    let deleting = deleting.expect("no strip for a machine about to go");
    assert_eq!((deleting.key, deleting.title.as_str()), ("deleting", "Its machine is deleted in 1 h 30 min"));
    let kept = rows(
        json!({"state": "running", "on": "box", "branch": "wsp/fix", "held": {"files": 1, "commits": 0}}),
        "completed",
        json!({"diskKeptUntil": 1_760_000_000_000.0}),
    );
    assert!(
        kept.expect("no strip for work not pushed").note.contains(", but Boat may delete its disk from "),
        "the strip leaves out when the disk may go"
    );
    let gone =
        rows(json!({"state": "running", "on": "box", "cloudGone": true}), "completed", json!({})).expect("no strip for a cloud that left");
    assert_eq!(
        (gone.title.as_str(), &gone.acts[..]),
        ("Boat was removed from Mac mini", &[transcript::StripAct::AddAgain("Boat".into())][..])
    );
    let view = cx.new(ThreadView::new);
    let cx = piece(cx, move |_, cx| strip(&view, &gone, cx));
    ROUTED.with(|r| r.borrow_mut().clear());
    let add = bounds(cx, "sub-strip-act-0");
    cx.simulate_click(add.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(
        (routed(), ran()),
        (vec![Route::Screen("settings:computers".into())], vec!["add a cloud".to_owned()]),
        "Add again did not open Add a computer"
    );
}

#[gpui_kit::test]
fn a_sub_thread_killed_for_memory_asks_its_lead_to_start_again_at_the_next_size(cx: &mut TestAppContext) {
    use crate::screens::sidebar::host::{Change, Snapshot};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let sidebar = cx.new(crate::sidebar::Sidebar::new);
    let lead = json!({"id": "s0", "workspaceId": "w0", "harness": "codex", "status": "running", "threadId": "lead", "startedAt": 1.0});
    let sub = json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": "failed", "threadId": "t", "startedAt": 2.0,
        "parentThreadId": "lead", "harnessTitle": "Fix docs", "failure": "the agent on box was killed for memory: 3.9 of 4 GB",
        "sub": {"state": "running", "on": "box", "size": {"cpu": 2, "memMb": 4096}}});
    let cloud = json!({"id": "p", "kind": "provider", "name": "box", "plan": {"sizes": ["2x4", "4x8", "8x16"]},
        "sizes": [{"cpu": 2, "memMb": 4096}, {"cpu": 8, "memMb": 16384}, {"cpu": 4, "memMb": 8192}, {"cpu": 4, "memMb": 6144}]});
    let snapshot = Snapshot {
        sessions: vec![lead, sub],
        workspaces: vec![json!({"id": "w", "name": "fix-docs", "kind": "cloud", "phase": "running"})],
        places: vec![cloud],
        ..Snapshot::default()
    };
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
    let strip = sidebar.read_with(cx, |s, _| Listing::listed(s.rows(), "t", 10.0, None, &json!({}), "").with_workspace(s, None).strip);
    let strip = strip.expect("no strip for a sub-thread killed for memory");
    let prompt = "Start the work of the sub-thread \"Fix docs\" again on Boat at size 4x8: its agent was killed for memory.";
    let lead_at = ThreadAt { workspace: "w0".into(), thread: "lead".into() };
    assert_eq!(
        strip.acts,
        [transcript::StripAct::StartAgain { size: "4x8".into(), lead: lead_at, harness: "codex".into(), prompt: prompt.into() }]
    );
    RAN.with(|r| r.borrow_mut().clear());
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|_, _| Piece(Box::new(move |_, cx| self::strip(&view, &strip, cx))));
    cx.simulate_resize(size(px(900.), px(700.)));
    cx.run_until_parked();
    let again = bounds(cx, "sub-strip-act-0");
    cx.simulate_click(again.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), [format!("send w0 lead codex {prompt}")], "Start again did not ask the lead");
}
