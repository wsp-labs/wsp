// SPDX-License-Identifier: AGPL-3.0-only
//! A sub-thread starting on a Boat machine of its own: the machine's record stands under its lead before the thread's
//! row lands (`sub-spawn.ts`), and the two may arrive in either order; neither is ever drawn as a root of the list.
use super::*;

/// The Boat machine t0 started, as its record stands while its thread starts.
fn boat(now: f64) -> Value {
    let made = chrono::DateTime::from_timestamp_millis(now as i64).unwrap().to_rfc3339();
    json!({
        "id": "w-boat", "name": "boat-1", "kind": "cloud", "project": { "id": "p1" }, "createdAt": made,
        "parentThreadId": "t0", "rootThreadId": "t0",
    })
}

/// The sub-thread's row on the Boat machine, with its opener stamped on it or not.
fn sub(now: f64, opener: bool) -> Value {
    let mut row = json!({
        "id": "r-sub", "threadId": "sub", "workspaceId": "w-boat", "harness": "claude", "status": "running",
        "prompt": "build it", "project": { "id": "p1", "name": "repo" }, "startedAt": now,
    });
    if opener {
        row["parentThreadId"] = json!("t0");
        row["rootThreadId"] = json!("t0");
    }
    row
}

/// Whether the Boat machine or its thread is drawn at the root of the list.
fn at_root(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> bool {
    sidebar.read_with(cx, |s, _| {
        s.drawn_rows.iter().any(|d| matches!(d, Drawn::Workspace(id) if id == "w-boat"))
            || s.lines.iter().any(|l| l.depth == 0 && matches!(&l.item, Item::Tile(t) if s.threads[t.thread].key == "sub"))
    })
}

fn under_lead(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> bool {
    sidebar
        .read_with(cx, |s, _| s.lines.iter().any(|l| l.depth > 0 && matches!(&l.item, Item::Tile(t) if s.threads[t.thread].key == "sub")))
}

/// Whether the Boat machine's record is drawn as a row of t0's tree, one level or more in, and laid out in the list.
fn machine_under_lead(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> bool {
    sidebar.read_with(cx, |s, _| {
        let Some(at) = s.lines.iter().position(|l| matches!(&l.item, Item::Machine { workspace } if workspace == "w-boat")) else {
            return false;
        };
        let top = (0..=at).rev().find(|&k| s.lines[k].depth == 0).unwrap();
        let in_tree = matches!(&s.lines[top].item, Item::Tile(t) if s.threads[t.thread].key == "t0");
        let drawn = s.drawn_rows.iter().position(|d| matches!(d, Drawn::Line(l) if *l == at));
        s.lines[at].depth >= 1 && in_tree && drawn.is_some_and(|ix| s.list.bounds_for_item(ix).is_some())
    })
}

fn machine_drawn(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> bool {
    sidebar.read_with(cx, |s, _| s.lines.iter().any(|l| matches!(&l.item, Item::Machine { .. })))
}

fn push(sidebar: &Entity<Sidebar>, change: Change, cx: &mut VisualTestContext) {
    sidebar.update(cx, |s, cx| s.apply(change, cx));
    cx.run_until_parked();
}

/// Catches: the Boat machine's record, standing before its thread's row lands, drawn as a root tile of its own with no
/// agent mark at the top of the list, or drawn nowhere, where today's hangs it under the thread that started it until
/// its thread's row takes its place.
#[gpui_kit::test]
fn a_boat_record_that_lands_before_its_row_is_never_a_root(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = drawn(cx);
    let mut snap = snapshot(2, now);
    snap.workspaces.push(boat(now));
    push(&sidebar, Change::Whole(Box::new(snap)), cx);
    assert!(!at_root(&sidebar, cx), "the record alone is not a root");
    assert!(machine_under_lead(&sidebar, cx), "the record is drawn under t0");
    push(&sidebar, Change::Row { id: "r-sub".into(), row: Some(sub(now, true)) }, cx);
    assert!(!at_root(&sidebar, cx) && under_lead(&sidebar, cx), "its thread stands under t0 alone");
    assert!(!machine_drawn(&sidebar, cx), "the thread takes the record's place");
}

/// Catches: the sub-thread drawn at the root while its row has landed and its record has not, or once the record
/// lands after it.
#[gpui_kit::test]
fn a_boat_row_that_lands_before_its_record_is_never_a_root(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = drawn(cx);
    push(&sidebar, Change::Whole(Box::new(snapshot(2, now))), cx);
    push(&sidebar, Change::Row { id: "r-sub".into(), row: Some(sub(now, true)) }, cx);
    assert!(!at_root(&sidebar, cx) && under_lead(&sidebar, cx), "the row stands under t0");
    push(&sidebar, Change::Workspaces(vec![workspace("w0"), boat(now)]), cx);
    assert!(!at_root(&sidebar, cx) && under_lead(&sidebar, cx), "and stays there once the record lands");
    assert!(!machine_drawn(&sidebar, cx), "the record holds a thread, so it draws no row of its own");
}

/// Catches: a thread with no opener on its row drawn at the root though the record of the machine it runs on names
/// the thread that started it (threadTree.ts `thread.parentThreadId ?? forkedBy`).
#[gpui_kit::test]
fn a_thread_hangs_under_the_thread_that_started_its_machine(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = drawn(cx);
    let mut snap = snapshot(2, now);
    snap.workspaces.push(boat(now));
    snap.sessions.extend(crate::screens::sidebar::shared::rows_of(vec![sub(now, false)]));
    push(&sidebar, Change::Whole(Box::new(snap)), cx);
    assert!(!at_root(&sidebar, cx) && under_lead(&sidebar, cx), "the machine's record puts it under t0");
}
