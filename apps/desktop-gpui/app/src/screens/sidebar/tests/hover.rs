// SPDX-License-Identifier: AGPL-3.0-only
//! What the pointer resting on a tile shows: its Settle act on the rows today's offers it on, and its card, which
//! follows the row the list scrolls under a still pointer.
use super::*;
use gpui_kit::{point, Modifiers, ScrollDelta, ScrollWheelEvent};

fn sel(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

/// A failed sub-thread under a read lead, a running root, a read lead whose child runs, and a settled root.
fn family(now: f64) -> Snapshot {
    let row = |id: &str, status: &str, parent: Option<&str>, read: bool, settled: bool| {
        let mut r = json!({
            "id": format!("r-{id}"), "threadId": id, "workspaceId": "w0", "harness": "claude", "status": status,
            "prompt": format!("thread {id}"), "project": { "id": "p1", "name": "repo" }, "startedAt": now - 120_000.,
        });
        if status != "running" {
            r["endedAt"] = json!(now - 60_000.);
        }
        if read {
            r["readAt"] = json!(now - 30_000.);
        }
        if settled {
            r["settledAt"] = json!(now - 20_000.);
        }
        if let Some(parent) = parent {
            r["parentThreadId"] = json!(parent);
            r["rootThreadId"] = json!(parent);
        }
        r
    };
    let mut snap = snapshot(0, now);
    snap.sessions = vec![
        row("lead", "completed", None, true, false),
        row("sub", "failed", Some("lead"), false, false),
        row("run", "running", None, false, false),
        row("busy", "completed", None, true, false),
        row("kid", "running", Some("busy"), false, false),
        row("done", "completed", None, true, true),
    ];
    snap
}

/// Catches: Settle offered on a root's tile alone (`tile.moves`), where today's offers it on a quiet child too, in
/// Needs you and under its lead (`childActs`); and Settle on a working tile, a tree that works, or a settled tile.
#[gpui_kit::test]
fn a_failed_sub_thread_offers_settle_in_needs_you_and_under_its_lead(cx: &mut TestAppContext) {
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| {
        s.apply(Change::Whole(Box::new(family(now_ms()))), cx);
        if s.lines.iter().any(|l| matches!(l.item, Item::Head { section: Section::Settled, folded: true, .. })) {
            s.toggle_section(Section::Settled, cx);
        }
    });
    cx.run_until_parked();
    let drawn = |key: &str, cx: &mut VisualTestContext| cx.debug_bounds(sel(format!("tile-{key}"))).is_some();
    for key in ["lead", "sub", "run", "busy", "kid", "done"] {
        assert!(drawn(key, cx), "{key} is drawn");
    }
    let settles = |key: &str, cx: &mut VisualTestContext| cx.debug_bounds(sel(format!("settle-{key}"))).is_some();
    assert!(cx.debug_bounds("settle-inbox-sub").is_some(), "the failed sub-thread's Needs you row offers Settle");
    assert!(settles("sub", cx), "its row under the lead offers Settle");
    assert!(settles("lead", cx), "a quiet lead offers Settle");
    for key in ["run", "busy", "kid", "done"] {
        assert!(!settles(key, cx), "{key} offers no Settle: it works, its tree works, or it is settled");
    }
}

fn card(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Option<String> {
    sidebar.read_with(cx, |s, _| s.card.clone())
}

/// The thread whose tile is under `at`.
fn under(sidebar: &Entity<Sidebar>, at: gpui_kit::Point<Pixels>, cx: &mut VisualTestContext) -> String {
    let keys: Vec<String> = sidebar.read_with(cx, |s, _| s.threads.iter().map(|t| t.key.clone()).collect());
    keys.into_iter().find(|k| cx.debug_bounds(sel(format!("tile-{k}"))).is_some_and(|b| b.contains(&at))).expect("a tile under the pointer")
}

/// Catches: the card kept on the row the pointer left, or none for the card's delay, when the pointer moves to the
/// next tile or the list scrolls another row under the still pointer; today's moves it at once to each row that
/// passes under it, as Base UI's provider opens the next tooltip with no delay while one shows or just closed.
#[gpui_kit::test]
fn the_card_follows_the_row_the_list_scrolls_under_a_resting_pointer(cx: &mut TestAppContext) {
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(40, now_ms()))), cx));
    cx.run_until_parked();
    let pointer = cx.debug_bounds("tile-t1").expect("t1 is drawn").center();
    cx.simulate_mouse_move(pointer, None, Modifiers::none());
    cx.executor().advance_clock(std::time::Duration::from_millis(450));
    cx.run_until_parked();
    assert_eq!(card(&sidebar, cx).as_deref(), Some("t:t1:false"), "the card opens after the pointer rests");
    let next = cx.debug_bounds("tile-t2").expect("t2 is drawn").center();
    for (to, key) in [(next, "t2"), (pointer, "t1")] {
        cx.simulate_mouse_move(to, None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(card(&sidebar, cx), Some(format!("t:{key}:false")), "the next tile's card opens at once while one stands");
    }
    for dy in [-52., -52., 52.] {
        let was = under(&sidebar, pointer, cx);
        cx.simulate_event(ScrollWheelEvent { position: pointer, delta: ScrollDelta::Pixels(point(px(0.), px(dy))), ..Default::default() });
        cx.run_until_parked();
        let now = under(&sidebar, pointer, cx);
        assert_ne!(now, was, "the list scrolled another row under the pointer");
        assert_eq!(card(&sidebar, cx), Some(format!("t:{now}:false")), "the card stands on the row now under the pointer");
    }
}
