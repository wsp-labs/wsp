// SPDX-License-Identifier: AGPL-3.0-only

use super::*;

#[gpui_kit::test]
fn the_threads_section_lists_the_threads_this_one_opened(cx: &mut TestAppContext) {
    let (view, cx) = window(plan("Plan."), cx);
    let row = |id: &str, parent: &str, status: &str| json!({"id": id, "workspaceId": "w2", "harness": "codex", "status": status, "threadId": id, "parentThreadId": parent, "startedAt": 5.0, "harnessTitle": format!("Thread {id}")});
    let rows = [
        json!({"id": "s0", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": 1.0}),
        row("th1", "t", "running"),
        row("th2", "t", "completed"),
        row("th3", "elsewhere", "running"),
    ];
    let listing = Listing::of(&rows, "t", 10.0);
    let ids: Vec<&str> = listing.children.iter().map(|c| c.at.thread.as_str()).collect();
    assert_eq!(ids, ["th1", "th2"]);
    listed(&view, listing, cx);
    let head = bounds(cx, "threads-head");
    assert_eq!(head.size.height, px(32.));
    let live = bounds(cx, "tree-row-th1");
    assert_eq!((live.top() - head.bottom(), live.size.height), (px(0.), px(36.)));
    ROUTED.with(|r| r.borrow_mut().clear());
    cx.simulate_click(live.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(routed(), [Route::Thread { workspace_id: "w2".into(), thread_id: "th1".into() }]);
    cx.simulate_click(head.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("threads-fold-Finished").is_none(), "the head did not shut the section");
    assert!(cx.debug_bounds("threads-peek").is_some(), "the shut section shows no peek");
}

const HOUR: f64 = 3_600_000.;
const NOW: f64 = 1_800_000_000_000.;

/// A lead `t` with a working child `th1` whose own child `th1a` asks, a finished child `th2`, a child `th3` quiet
/// for three hours, a child `th4` on another computer, and a running subagent of the lead's agent.
fn tree_rows() -> Vec<serde_json::Value> {
    let row = |id: &str, parent: &str, status: &str, started: f64| {
        json!({"id": format!("s-{id}"), "workspaceId": format!("w-{id}"), "harness": "codex", "status": status, "threadId": id,
            "parentThreadId": parent, "startedAt": started, "harnessTitle": format!("Thread {id}"), "computerName": "here"})
    };
    let quiet = |mut r: serde_json::Value, ended: f64| {
        r["endedAt"] = json!(ended);
        r["readAt"] = json!(ended);
        r
    };
    let mut asking = row("th1a", "th1", "running", NOW - 60_000.);
    asking["asking"] = json!("Which branch?");
    let mut away = quiet(row("th4", "t", "completed", NOW - 2. * 60_000.), NOW - 60_000.);
    away["computerName"] = json!("studio");
    away["lastLine"] = json!("All done.");
    vec![
        json!({"id": "s-t", "workspaceId": "w", "harness": "claude", "status": "running", "threadId": "t", "startedAt": NOW - HOUR,
            "computerName": "here", "subagents": [{"id": "k1", "title": "Survey it", "state": "running", "parentToolUseId": "call-1", "startedAt": NOW - 30_000.}]}),
        row("th1", "t", "running", NOW - 10. * 60_000.),
        asking,
        quiet(row("th2", "t", "completed", NOW - 20. * 60_000.), NOW - 5. * 60_000.),
        quiet(row("th3", "t", "completed", NOW - 4. * HOUR), NOW - 3. * HOUR),
        away,
    ]
}

fn tree_listing(settle: Option<f64>, preferences: serde_json::Value) -> Listing {
    let rows: Vec<crate::screens::sidebar::model::Row> =
        tree_rows().into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
    Listing::listed(&rows, "t", NOW, settle, &preferences, "here")
}

#[test]
fn the_tree_nests_children_with_subagents_among_them_in_their_parts() {
    let listing = tree_listing(Some(2. * HOUR), json!({}));
    let top: Vec<(&str, ChildPart)> =
        listing.children.iter().map(|c| (c.subagent.as_deref().unwrap_or(c.at.thread.as_str()), c.part)).collect();
    // Live by the most pressing rank under each, then finished by newest end, then settled.
    assert_eq!(
        top,
        [
            ("th1", ChildPart::Live),
            ("call-1", ChildPart::Live),
            ("th4", ChildPart::Finished),
            ("th2", ChildPart::Finished),
            ("th3", ChildPart::Settled)
        ]
    );
    let th1 = &listing.children[0];
    assert_eq!(th1.children.iter().map(|c| c.at.thread.as_str()).collect::<Vec<_>>(), ["th1a"], "the grandchild stands under its parent");
    assert_eq!(th1.children[0].note.as_deref(), Some("Which branch?"));
    assert_eq!(th1.children[0].kind, Kind::NeedsYou);
    let away = &listing.children[2];
    assert_eq!(
        (away.computer.as_deref(), away.note.as_deref()),
        (Some("studio"), Some("All done.")),
        "the lead's computer is left out, another named"
    );
    assert_eq!(listing.children[1].kind, Kind::Working);
    assert_eq!(
        listing.counts(),
        transcript::TreeCounts { needs_you: 1, failed: 0, working: 2, working_subagents: 1, waiting: 0, finished: 2 }
    );
    assert_eq!(listing.most_pressing().map(|n| n.at.thread.as_str()), Some("th1a"), "the peek is the most pressing at any depth");
    assert_eq!(listing.finished(), (vec!["th4".to_owned(), "th2".to_owned()], 2));
}

#[test]
fn a_child_settles_after_the_persons_settle_after() {
    let part = |settle: Option<f64>| tree_listing(settle, json!({})).children.iter().find(|c| c.at.thread == "th3").map(|c| c.part);
    assert_eq!(part(Some(2. * HOUR)), Some(ChildPart::Settled));
    assert_eq!(part(None), Some(ChildPart::Finished), "settleAfter never kept a quiet child out of Settled");
    assert_eq!(part(Some(86_400_000.)), Some(ChildPart::Finished), "settleAfter 1d settled a child quiet for 3h");
    let th2 = |settle| tree_listing(Some(settle), json!({})).children.iter().find(|c| c.at.thread == "th2").map(|c| c.part);
    assert_eq!(th2(15. * 60_000.), Some(ChildPart::Finished), "15 minutes settled a child quiet for 5");
    assert_eq!(th2(60_000.), Some(ChildPart::Settled), "a minute's settle left a child quiet for 5 out of Settled");
}

fn tree_window(listing: Listing, cx: &mut TestAppContext) -> (Entity<ThreadView>, &mut VisualTestContext) {
    RAN.with(|r| r.borrow_mut().clear());
    let (view, cx) = window(vec![base("session.start", 1, json!({"prompt": "go"}))], cx);
    listed(&view, listing, cx);
    (view, cx)
}

#[gpui_kit::test]
fn a_section_the_preferences_shut_says_its_counts_over_a_fading_peek(cx: &mut TestAppContext) {
    let (view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({"threadsShut": {"t": true}})), cx);
    assert!(view.read_with(cx, |v, _| v.rows().iter().any(|r| matches!(r.kind, RowKind::Threads { open: false, .. }))));
    let head = bounds(cx, "threads-head");
    assert!(cx.debug_bounds("tree-counts").is_some(), "the shut head says no counts");
    for count in ["tree-count-needs-you", "tree-count-working"] {
        assert!(cx.debug_bounds(count).is_some(), "{count} not drawn");
    }
    assert!(cx.debug_bounds("tree-count-finished").is_none(), "finished is said only with nothing live");
    assert!(cx.debug_bounds("threads-settle").is_none() && cx.debug_bounds("threads-fold-Finished").is_none());
    let peek = bounds(cx, "threads-peek");
    assert_eq!((peek.top(), peek.size.height), (head.bottom(), px(48.)), "the peek is the asking grandchild's two-line row");
    cx.simulate_click(peek.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["shut t false"], "a press on the peek did not open the section through the preferences");
    assert!(cx.debug_bounds("threads-fold-Finished").is_some(), "the section did not open at once");
}

/// Catches: a fill laid over the peek to stand in for the mask, which shows as a darker band over its lower half
/// wherever the transcript's ground is not that fill: a dark theme on the Mac, solid or on glass.
#[gpui_kit::test]
fn the_peek_fades_under_the_webs_mask_and_paints_nothing_over_its_row(cx: &mut TestAppContext) {
    use crate::screens::app_shell::material::Look;
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({"threadsShut": {"t": true}})), cx);
    for look in [Look { mac: false, glass: false }, Look { mac: true, glass: false }, Look { mac: true, glass: true }] {
        cx.update(|window, cx| {
            crate::ui::theme::set_theme_in("graphite", look, cx);
            window.refresh();
        });
        cx.run_until_parked();
        let peek = bounds(cx, "threads-peek");
        crate::screens::thread_view::rows::assert_fades(cx, peek, peek.size.height, px(0.), &format!("{look:?}: mask-b-from-0%"));
        let laid = cx.update(|window, _| {
            let peek = peek.scale(window.scale_factor());
            window
                .painted_quads()
                .iter()
                .filter(|q| q.bounds.intersect(&peek) == q.bounds && q.bounds.size.height > peek.size.height / 2.)
                .count()
        });
        assert_eq!(laid, 0, "{look:?}: a fill is laid over the peek");
    }
}

#[gpui_kit::test]
fn the_open_head_settles_the_finished_part_and_shuts_through_the_preferences(cx: &mut TestAppContext) {
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    assert!(cx.debug_bounds("tree-counts").is_none(), "the open head says counts");
    let (head, toggle, settle) = (bounds(cx, "threads-head"), bounds(cx, "threads-toggle"), bounds(cx, "threads-settle"));
    assert!(toggle.right() <= settle.left() && settle.right() < head.right(), "Settle stands between the toggle and the chevron");
    assert_eq!(settle.size.height, px(24.));
    cx.simulate_click(settle.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["settle th4 th2"]);
    let at = bounds(cx, "threads-chevron").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["settle th4 th2", "shut t true"]);
    assert!(cx.debug_bounds("threads-peek").is_some(), "the chevron did not shut the section at once");
}

#[gpui_kit::test]
fn a_grandchild_hangs_on_the_rail_under_its_parent(cx: &mut TestAppContext) {
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let (parent, kid, list) = (bounds(cx, "tree-row-th1"), bounds(cx, "tree-row-th1a"), bounds(cx, "tree-list"));
    // `ml-3.5` then the item's `pl-2`: the grandchild's row starts 22 px in from its parent's.
    assert_eq!((list.left() - parent.left(), kid.left() - parent.left()), (px(14.), px(22.)));
    assert_eq!(list.top(), parent.bottom(), "the rail list stands right under its parent's row");
    assert!(cx.debug_bounds("tree-sub-t-call-1").is_some(), "the lead's subagent is not among the children");
}

#[gpui_kit::test]
fn a_childs_state_act_runs_through_the_sidebar_and_stop_takes_two_presses(cx: &mut TestAppContext) {
    let (view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let stop = bounds(cx, "tree-row-th1-act-stop");
    cx.simulate_click(stop.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(ran().is_empty(), "one press stopped the thread");
    assert!(routed_after_clear().is_empty(), "a press on the act opened the thread");
    let at = bounds(cx, "tree-row-th1-act-stop").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["stop s-th1"]);
    let at = bounds(cx, "threads-fold-Finished").center();
    cx.simulate_click(at, Modifiers::none());
    let at = bounds(cx, "threads-fold-Settled").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let at = bounds(cx, "tree-row-th2-act-settle").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let at = bounds(cx, "tree-row-th3-act-restore").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["stop s-th1", "settle th2", "restore th3"]);
    assert!(cx.debug_bounds("tree-row-th1a-act-settle").is_none(), "an asking thread offers Settle");
    assert!(cx.debug_bounds("tree-row-th2-act-more").is_some(), "a thread's row offers no More");
    let _ = view;
}

#[gpui_kit::test]
fn a_quiet_thread_whose_child_works_offers_stop_and_holds_settle(cx: &mut TestAppContext) {
    let row = |id: &str, parent: &str, status: &str| {
        json!({"id": format!("s-{id}"), "workspaceId": "w2", "harness": "codex", "status": status, "threadId": id,
            "parentThreadId": parent, "startedAt": NOW - 60_000., "endedAt": NOW - 30_000., "readAt": NOW})
    };
    let lead =
        json!({"id": "s-t", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": NOW - HOUR});
    let mut kid = row("th5a", "th5", "running");
    kid.as_object_mut().unwrap().remove("endedAt");
    let rows: Vec<crate::screens::sidebar::model::Row> =
        [lead, row("th5", "t", "completed"), kid].into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
    let listing = Listing::listed(&rows, "t", NOW, Some(2. * HOUR), &json!({}), "");
    let th5 = &listing.children[0];
    assert_eq!((th5.running, th5.working), (false, true), "a working child left its quiet parent not working");
    let (_view, cx) = tree_window(listing, cx);
    assert!(cx.debug_bounds("tree-row-th5-act-stop").is_some(), "no Stop on a thread whose child works");
    assert!(cx.debug_bounds("tree-row-th5-act-settle").is_none(), "Settle offered while a thread under it works");
}

#[gpui_kit::test]
fn a_right_click_on_a_row_opens_the_acts_more_holds(cx: &mut TestAppContext) {
    let (view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let at = bounds(cx, "threads-fold-Finished").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let row = bounds(cx, "tree-row-th2").center();
    cx.simulate_event(gpui_kit::MouseDownEvent {
        button: gpui_kit::MouseButton::Right,
        position: row,
        modifiers: Modifiers::none(),
        click_count: 1,
        first_mouse: false,
    });
    cx.run_until_parked();
    assert_eq!(
        view.read_with(cx, |v, _| v.row_menu.asked().to_vec()),
        ["Send a message", "Settle thread"],
        "a right-click asked for no menu"
    );
    // On the Mac AppKit draws the menu, which the test platform cannot drive.
    if cfg!(target_os = "macos") {
        return;
    }
    assert!(view.read_with(cx, |v, _| v.row_menu.is_open()), "a right-click opened no menu");
    // The menu opens on Send a message, its first row; one Down lights Settle thread.
    cx.simulate_keystrokes("down enter");
    cx.run_until_parked();
    assert_eq!(ran(), ["settle th2"]);
    assert!(routed_after_clear().is_empty(), "the right-click opened the thread");
}

#[gpui_kit::test]
fn a_later_arm_of_stop_keeps_its_own_two_seconds(cx: &mut TestAppContext) {
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let stop = bounds(cx, "tree-row-th1-act-stop").center();
    cx.simulate_click(stop, Modifiers::none());
    cx.run_until_parked();
    // The pointer leaves the row, which disarms, and comes back to arm again a second later.
    cx.simulate_mouse_move(gpui_kit::point(px(1.), px(1.)), None, Modifiers::none());
    cx.run_until_parked();
    cx.executor().advance_clock(std::time::Duration::from_millis(1_000));
    cx.simulate_mouse_move(stop, None, Modifiers::none());
    cx.simulate_click(stop, Modifiers::none());
    cx.run_until_parked();
    // The first arm's two seconds run out while the second still has one left.
    cx.executor().advance_clock(std::time::Duration::from_millis(1_100));
    cx.run_until_parked();
    cx.simulate_click(stop, Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["stop s-th1"], "the first arm's lapse cut the second arm short");
}

#[gpui_kit::test]
fn the_listing_follows_the_sidebars_rows_and_the_persons_preferences(cx: &mut TestAppContext) {
    use crate::screens::sidebar::host::{Change, Snapshot};
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let sidebar = cx.new(crate::sidebar::Sidebar::new);
    cx.update(|cx| crate::screens::terminal::read_workspaces_from(&sidebar, cx));
    let snapshot = Snapshot { sessions: tree_rows(), preferences: json!({"settleAfter": "never"}), ..Snapshot::default() };
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
    let view = cx.new(ThreadView::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view.clone(), window, cx));
    view.update(cx, |v, cx| {
        v.show("t", cx);
        v.relist(cx);
    });
    cx.run_until_parked();
    let part = |cx: &mut VisualTestContext, id: &str| {
        view.read_with(cx, |v, _| v.listing().and_then(|l| l.children.iter().find(|c| c.at.thread == id).map(|c| (c.part, c.kind))))
    };
    assert_eq!(part(cx, "th3").map(|p| p.0), Some(ChildPart::Finished), "the sidebar's settleAfter never did not reach the listing");
    // A row the host pushes reaches the listing with no read of its own.
    let mut asks = tree_rows().remove(4);
    asks["status"] = json!("running");
    asks["asking"] = json!("Deploy?");
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "s-th3".into(), row: Some(asks) }, cx));
    cx.run_until_parked();
    assert_eq!(part(cx, "th3"), Some((ChildPart::Live, Kind::NeedsYou)), "the listing did not follow the sidebar's row");
    sidebar.update(cx, |s, cx| s.apply(Change::Preferences(json!({"threadsShut": {"t": true}})), cx));
    cx.run_until_parked();
    assert!(view.read_with(cx, |v, _| v.listing().is_some_and(|l| l.shut)), "threadsShut in the preferences did not shut the section");
    // A switch the sidebar has nothing new for still reads the new thread's listing.
    view.update(cx, |v, cx| {
        v.show("th1", cx);
        v.relist(cx);
    });
    cx.run_until_parked();
    let lead = view.read_with(cx, |v, _| v.listing().map(|l| (l.thread.clone(), l.children.len())));
    assert_eq!(lead, Some(("th1".to_owned(), 1)), "the listing stayed on the thread switched away from");
}

#[gpui_kit::test]
fn an_open_fold_pages_twenty_rows_and_n_more_adds_twenty(cx: &mut TestAppContext) {
    let lead =
        json!({"id": "s-t", "workspaceId": "w", "harness": "claude", "status": "completed", "threadId": "t", "startedAt": NOW - HOUR});
    let mut rows = vec![lead];
    for i in 0..25 {
        rows.push(
            json!({"id": format!("s{i}"), "workspaceId": "w2", "harness": "codex", "status": "completed", "threadId": format!("c{i}"),
            "parentThreadId": "t", "startedAt": NOW - 600_000. + f64::from(i), "endedAt": NOW - 300_000. + f64::from(i), "readAt": NOW}),
        );
    }
    let rows: Vec<crate::screens::sidebar::model::Row> = rows.into_iter().filter_map(crate::screens::sidebar::model::Row::owned).collect();
    let (view, cx) = tree_window(Listing::listed(&rows, "t", NOW, Some(2. * HOUR), &json!({}), ""), cx);
    let at = bounds(cx, "threads-fold-Finished").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    let count = |_: &Entity<ThreadView>, cx: &mut VisualTestContext| {
        (0..25).filter(|i| cx.debug_bounds(Box::leak(format!("tree-row-c{i}").into_boxed_str())).is_some()).count()
    };
    assert_eq!(count(&view, cx), 20, "an open fold draws twenty rows");
    let more = bounds(cx, "threads-more-Finished");
    cx.simulate_click(more.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(count(&view, cx), 25, "N more did not add the rest");
    assert!(cx.debug_bounds("threads-more-Finished").is_none());
}

#[gpui_kit::test]
fn a_childs_send_a_message_opens_a_field_that_enter_sends_and_escape_shuts(cx: &mut TestAppContext) {
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let one = bounds(cx, "tree-row-th1").size.height;
    let send = bounds(cx, "tree-row-th1-act-send");
    cx.simulate_click(send.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-row-th1-send-field").is_some(), "Send a message opened no field");
    assert!(routed_after_clear().is_empty(), "a press on Send a message opened the thread");
    assert!(bounds(cx, "tree-row-th1").size.height > one, "the field does not stand in the second line's place");
    cx.simulate_input("  fix the docs ");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(ran(), ["send w-th1 th1 codex fix the docs"], "Enter did not send the message to the child");
    assert!(cx.debug_bounds("tree-row-th1-send-field").is_none(), "the field stayed after the send");
    let send = bounds(cx, "tree-row-th1-act-send");
    cx.simulate_click(send.center(), Modifiers::none());
    cx.run_until_parked();
    cx.simulate_input("never mind");
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-row-th1-send-field").is_none(), "Escape left the field standing");
    assert_eq!(ran().len(), 1, "Escape sent the message");
}

#[gpui_kit::test]
fn a_running_subagents_row_offers_stop_subagent_and_its_card_after_a_rest(cx: &mut TestAppContext) {
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let stop = bounds(cx, "tree-sub-t-call-1-act-stop");
    cx.simulate_click(stop.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(ran().is_empty(), "one press stopped the subagent");
    let stop = bounds(cx, "tree-sub-t-call-1-act-stop");
    cx.simulate_click(stop.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(ran(), ["stop s-t k1"], "Stop subagent did not interrupt the subagent's task");
    assert!(routed_after_clear().is_empty(), "a press on Stop subagent opened its page");
    let title = bounds(cx, "tree-sub-t-call-1-title");
    cx.simulate_mouse_move(title.center(), None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_millis(300));
    cx.run_until_parked();
    assert!(cx.debug_bounds("subagent-card").is_none(), "the card opened before the pointer rested 450 ms");
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    cx.run_until_parked();
    assert!(cx.debug_bounds("subagent-card").is_some(), "no card after the pointer rested on the subagent's title");
}

#[gpui_kit::test]
fn a_child_on_another_computer_names_it_with_its_glyph(cx: &mut TestAppContext) {
    use crate::screens::sidebar::host::{Change, Snapshot};
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let sidebar = cx.update(|_, cx| cx.new(crate::sidebar::Sidebar::new));
    cx.update(|_, cx| crate::screens::terminal::read_workspaces_from(&sidebar, cx));
    let places = vec![json!({"id": "p1", "kind": "computer", "name": "studio"})];
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(Snapshot { places, ..Snapshot::default() })), cx));
    let at = bounds(cx, "threads-fold-Finished").center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-row-th4-computer").is_some(), "the second line does not name the computer");
    assert!(cx.debug_bounds("tree-row-th4-computer-glyph").is_some(), "the computer's name stands without its glyph");
}

#[gpui_kit::test]
fn the_workspaces_state_reaches_the_listing_off_the_sidebar(cx: &mut TestAppContext) {
    use crate::screens::sidebar::host::{Change, Snapshot};
    use crate::screens::terminal::words::Memory;
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
    });
    let sidebar = cx.new(crate::sidebar::Sidebar::new);
    let row =
        |status: &str| json!({"id": "s1", "workspaceId": "w", "harness": "claude", "status": status, "threadId": "t", "startedAt": 1.0});
    let read = |cx: &mut TestAppContext, w: serde_json::Value, status: Option<serde_json::Value>, row: serde_json::Value, memory| {
        let places = vec![json!({"id": "here", "kind": "computer", "name": "Mac mini"})];
        let snapshot =
            Snapshot { sessions: vec![row], workspaces: vec![w], statuses: status.into_iter().collect(), places, ..Snapshot::default() };
        sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
        sidebar.read_with(cx, |s, _| {
            let l = Listing::listed(s.rows(), "t", 10.0, None, &json!({}), "").with_workspace(s, memory);
            (l.wait.map(|w| (w.label, w.wake, w.elapsed)), l.paused, l.machine)
        })
    };
    let napping = json!({"id": "w", "name": "spoo", "kind": "local", "phase": "napping"});
    assert_eq!(
        read(cx, napping.clone(), None, row("completed"), None).1.as_deref(),
        Some("Stopped"),
        "a nap on a computer that drops memory"
    );
    let mut kept = napping.clone();
    kept["pauseMode"] = json!("memory");
    let (wait, paused, machine) = read(cx, kept, None, row("running"), None);
    assert_eq!(wait, Some(("waiting for spoo to wake".to_owned(), true, false)));
    assert_eq!((paused.as_deref(), machine), (Some("Paused"), None), "a paused machine says a machine line");
    let waking = json!({"id": "w", "name": "spoo", "kind": "local", "phase": "waking"});
    assert_eq!(read(cx, waking, None, row("running"), None).0, Some(("waking spoo on Mac mini".to_owned(), false, true)));
    let running = json!({"id": "w", "name": "spoo", "kind": "local", "phase": "running"});
    let lacks = json!({"id": "w", "phase": "running", "reach": {"state": "unsupported"}, "daemonRefusedAt": {"machineId": "m", "at": "x", "why": "git is missing, and so is node"}});
    let (wait, paused, machine) = read(cx, running.clone(), Some(lacks), row("completed"), None);
    assert_eq!((wait, paused, machine.as_deref()), (None, None, Some("Git is missing")));
    let full = Memory { used: 3.8 * 1024. * 1024. * 1024., total: 4. * 1024. * 1024. * 1024., load1: 2. };
    let answers = Some(json!({"id": "w", "phase": "running"}));
    assert_eq!(read(cx, running, answers, row("completed"), Some(full)).2.as_deref(), Some("Out of memory, 3.8 of 4 GB"));
    let mut sub = row("completed");
    sub["sub"] = json!({"state": "running", "on": "box"});
    assert_eq!(read(cx, napping, None, sub, None).1, None, "a sub-thread's nap said the paused word");
}

#[gpui_kit::test]
fn an_armed_stop_keeps_the_room_the_acts_held(cx: &mut TestAppContext) {
    let (_view, cx) = tree_window(tree_listing(Some(2. * HOUR), json!({})), cx);
    let title = |cx: &mut VisualTestContext| bounds(cx, "tree-row-th1-title").size.width;
    let before = title(cx);
    let stop = bounds(cx, "tree-row-th1-act-stop");
    cx.simulate_click(stop.center(), Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("tree-row-th1-act-send").is_none(), "Send stayed beside the armed Stop");
    assert_eq!(title(cx), before, "arming Stop moved the title's cut");
}
