// SPDX-License-Identifier: AGPL-3.0-only
//! Ctrl+Tab's step through the open thread's own level as the list draws it: the live list's top-level threads,
//! Settled's top-level threads, or the threads one lead started; a Needs you copy steps from its place in its tree.
use super::*;

/// Roots r1 to r4 in the list, r2 leading k1, k2 (which leads g1 and g2), a failed k3 and finished d1 and d2; r3
/// folded over f1 and f2; r4 failed over r4k, so it stands in Needs you too; n1 failed alone in Needs you; s1 to s3
/// settled, s2 over s2k. r2, r3 and r4 run threads, so they stand in Working above r1.
fn family(now: f64) -> Snapshot {
    let row = |id: &str, status: &str, parent: Option<&str>, ago: f64| {
        let mut r = json!({
            "id": format!("r-{id}"), "threadId": id, "workspaceId": "w0", "harness": "claude", "status": status,
            "prompt": format!("thread {id}"), "project": { "id": "p1", "name": "repo" }, "startedAt": now - ago * 1_000.,
        });
        if status != "running" {
            r["endedAt"] = json!(now - 5_000.);
        }
        if status == "completed" {
            r["readAt"] = json!(now - 4_000.);
        }
        if let Some(parent) = parent {
            r["parentThreadId"] = json!(parent);
            r["rootThreadId"] = json!(parent);
        }
        r
    };
    let settled = |mut r: Value| {
        r["settledAt"] = json!(now - 3_000.);
        r
    };
    let mut folded = row("r3", "completed", None, 30.);
    folded["foldedAt"] = json!(now - 3_000.);
    let mut snap = snapshot(0, now);
    snap.sessions = crate::screens::sidebar::shared::rows_of(vec![
        row("r1", "completed", None, 10.),
        row("r2", "completed", None, 20.),
        row("k1", "running", Some("r2"), 21.),
        row("k2", "running", Some("r2"), 22.),
        row("g1", "running", Some("k2"), 23.),
        row("g2", "running", Some("k2"), 24.),
        row("k3", "failed", Some("r2"), 25.),
        row("d1", "completed", Some("r2"), 26.),
        row("d2", "completed", Some("r2"), 27.),
        folded,
        row("f1", "running", Some("r3"), 31.),
        row("f2", "running", Some("r3"), 32.),
        row("r4", "failed", None, 40.),
        row("r4k", "running", Some("r4"), 41.),
        row("n1", "failed", None, 50.),
        settled(row("s1", "completed", None, 60.)),
        settled(row("s2", "completed", None, 70.)),
        settled(row("s2k", "completed", Some("s2"), 71.)),
        settled(row("s3", "completed", None, 80.)),
    ]);
    snap
}

fn listed(cx: &mut TestAppContext) -> (Entity<Sidebar>, &mut VisualTestContext) {
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(family(now_ms()))), cx));
    cx.run_until_parked();
    (sidebar, cx)
}

/// Every thread's tile top to bottom, a Needs you copy marked.
fn order(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Vec<String> {
    sidebar.read_with(cx, |s, _| {
        let tiles = s.drawn_rows.iter().filter_map(|d| match d {
            Drawn::Line(ix) => match &s.lines[*ix].item {
                Item::Tile(t) => Some(format!("{}{}", s.threads[t.thread].key, if t.inbox { "*" } else { "" })),
                _ => None,
            },
            _ => None,
        });
        tiles.collect()
    })
}

/// Opens `from`, then steps `by` once for each of `n`, noting the thread open after each.
fn walk(sidebar: &Entity<Sidebar>, from: &str, by: isize, n: usize, cx: &mut VisualTestContext) -> Vec<String> {
    sidebar.update(cx, |s, cx| _ = s.mark(from, cx));
    (0..n)
        .map(|_| {
            sidebar.update(cx, |s, cx| {
                s.step_level(by, cx);
                s.selected.clone().unwrap_or_default()
            })
        })
        .collect()
}

fn seq(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| k.to_string()).collect()
}

/// Catches: a step landing on a sub-thread (k1 under r2), or from a Needs you copy (r4's, at the top of the list); a
/// walk that stops at either end, or that steps in an order the list does not draw; and none open going nowhere.
#[gpui_kit::test]
fn a_top_level_thread_steps_through_the_top_level_threads_as_drawn(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(cx);
    assert_eq!(order(&sidebar, cx), ["k3*", "r4*", "n1", "r2", "k3", "k1", "k2", "g1", "g2", "r3", "r4", "r4k", "r1"]);
    assert_eq!(walk(&sidebar, "r1", 1, 5, cx), seq(&["n1", "r2", "r3", "r4", "r1"]), "down, wrapping at the end");
    assert_eq!(walk(&sidebar, "r1", -1, 5, cx), seq(&["r4", "r3", "r2", "n1", "r1"]), "up, wrapping at the top");
    sidebar.update(cx, |s, cx| s.unmark(cx));
    assert_eq!(walk(&sidebar, "", 1, 1, cx), seq(&["n1"]), "with no thread open, down starts at the first");
    sidebar.update(cx, |s, cx| s.unmark(cx));
    assert_eq!(walk(&sidebar, "", -1, 1, cx), seq(&["r1"]), "and up at the last");
}

/// Catches: a settled root stepping into the live list, or onto a settled tree's sub-thread (s2k); and a step out of
/// a folded Settled, whose threads are not drawn.
#[gpui_kit::test]
fn a_settled_thread_steps_through_the_settled_top_level_threads(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(cx);
    assert_eq!(walk(&sidebar, "s1", 1, 1, cx), seq(&["s1"]), "Settled folded draws nothing to step to");
    sidebar.update(cx, |s, cx| s.toggle_section(Section::Settled, cx));
    assert_eq!(walk(&sidebar, "s1", 1, 3, cx), seq(&["s2", "s3", "s1"]));
    assert_eq!(walk(&sidebar, "s1", -1, 3, cx), seq(&["s3", "s2", "s1"]));
    assert_eq!(walk(&sidebar, "r4", 1, 1, cx), seq(&["r1"]), "a live root still skips Settled when it is open");
}

/// Catches: a sub-thread stepping onto a thread of another level (a grandchild g1 from k1, or its cousin), or between
/// its lead's live threads and its Finished fold, open or shut, which are levels of their own.
#[gpui_kit::test]
fn a_sub_thread_steps_through_its_leads_threads_as_drawn(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(cx);
    assert_eq!(walk(&sidebar, "k1", 1, 3, cx), seq(&["k2", "k3", "k1"]));
    assert_eq!(walk(&sidebar, "k3", -1, 3, cx), seq(&["k2", "k1", "k3"]));
    assert_eq!(walk(&sidebar, "g1", 1, 2, cx), seq(&["g2", "g1"]), "a lead's threads at any depth");
    assert_eq!(walk(&sidebar, "d1", 1, 1, cx), seq(&["d1"]), "a shut fold draws no sibling to step to");
    sidebar.update(cx, |s, cx| s.toggle_finished("r2", cx));
    assert_eq!(order(&sidebar, cx)[4..9], ["k3", "k1", "k2", "g1", "g2"]);
    assert_eq!(walk(&sidebar, "k1", 1, 4, cx), seq(&["k2", "k3", "k1", "k2"]), "the open fold stays out of the live walk");
    assert_eq!(walk(&sidebar, "d1", 1, 2, cx), seq(&["d2", "d1"]), "and steps on its own");
}

/// Catches: a step from a sub-thread landing on its lead or on a top-level thread, from any sub-thread, either way.
#[gpui_kit::test]
fn a_step_from_a_sub_thread_never_lands_on_its_lead_or_a_top_level_thread(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(cx);
    let lead = |key: &str, cx: &mut VisualTestContext| {
        sidebar.read_with(cx, |s, _| s.threads.iter().find(|t| t.key == key).and_then(|t| t.parent.clone()))
    };
    for from in ["k1", "k2", "k3", "g1", "g2", "r4k", "f1"] {
        let own = lead(from, cx).expect("a sub-thread has a lead");
        for by in [1, -1] {
            for landed in walk(&sidebar, from, by, 4, cx) {
                assert_eq!(lead(&landed, cx).as_deref(), Some(own.as_str()), "{from} stepped {by} onto {landed}");
            }
        }
    }
}

/// Catches: a failed sub-thread stepping from its Needs you copy, the first place it is drawn, where it would land on
/// itself; it steps from its place under r2.
#[gpui_kit::test]
fn a_needs_you_thread_steps_from_its_place_in_its_tree(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(cx);
    assert_eq!(walk(&sidebar, "k3", 1, 1, cx), seq(&["k1"]));
    assert_eq!(walk(&sidebar, "r4", -1, 1, cx), seq(&["r3"]), "a failed root steps from its tree, not its copy");
}
