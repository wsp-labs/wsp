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

/// The Boat machine t0 started, deleted once its sub-thread was done, whose sub-thread was deleted after it: the host
/// keeps the record and no row.
fn deleted_with_its_sub_thread(now: f64) -> Value {
    let mut record = boat(now - 60_000.);
    record["phase"] = json!("released");
    record["machineId"] = json!("");
    record
}

/// Catches: a machine deleted with its sub-thread drawn as a card under the thread that started it, which opened a New
/// thread page on a machine that is gone, and reached by Ctrl+Tab's walk, which steps the drawn rows.
#[gpui_kit::test]
fn a_machine_deleted_with_its_sub_thread_draws_no_row(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = drawn(cx);
    let mut snap = snapshot(2, now);
    snap.workspaces.push(deleted_with_its_sub_thread(now));
    push(&sidebar, Change::Whole(Box::new(snap)), cx);
    assert!(!machine_drawn(&sidebar, cx), "no card under t0");
    assert!(!at_root(&sidebar, cx), "and none at the root");
    assert!(sidebar.read_with(cx, |s, _| s.deleted_sub_thread("w-boat")));
    // The same record with its sub-thread still held keeps its thread's row, today's banner on its page.
    push(&sidebar, Change::Row { id: "r-sub".into(), row: Some(sub(now - 60_000., true)) }, cx);
    assert!(under_lead(&sidebar, cx) && !sidebar.read_with(cx, |s, _| s.deleted_sub_thread("w-boat")), "a held sub-thread stands");
}

/// Catches: a machine deleted with its sub-thread, reached by a link or a kept selection, opening the New thread page
/// with its field on a machine that is gone, where it says its sub-thread was deleted.
#[gpui_kit::test]
fn a_page_on_a_machine_deleted_with_its_sub_thread_says_so(cx: &mut TestAppContext) {
    use crate::screens::composer::screen::ComposerScreen;
    use crate::screens::settings::door::{self, Door};
    use crate::screens::{Host, Opened, Screen};
    cx.executor().allow_parking();
    let now = now_ms();
    let (sidebar, cx) = drawn(cx);
    let mut snap = snapshot(2, now);
    snap.workspaces.push(deleted_with_its_sub_thread(now));
    push(&sidebar, Change::Whole(Box::new(snap)), cx);
    let door = Door::start(json!({}));
    let runtime = door::runtime();
    let host = Host::new(door.client(&runtime), runtime);
    cx.update(|_, cx| crate::screens::terminal::read_workspaces_from(&sidebar, cx));
    let mut page = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let opened = Opened { arg: Some("w-boat".into()), ..Opened::default() };
        let view = cx.new(|cx| <ComposerScreen as Screen>::new(host, opened, window, cx));
        page = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.run_until_parked();
    let page = page.unwrap();
    assert!(cx.debug_bounds("sub-thread-deleted").is_some(), "the page says its sub-thread was deleted");
    assert!(!page.read_with(cx, |p, cx| p.hero().read(cx).follows()), "and draws no New thread field");
}

/// Catches: with the list narrowed to one computer, the Boat machines of threads on another drawn after Settled as
/// cards of their own, the computer pick applied to the threads but not to the workspaces that hold none of them,
/// where today's sidebar narrows every row to the picked computer's.
#[gpui_kit::test]
fn a_computer_pick_draws_nothing_of_another_computer(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = drawn(cx);
    let place = |id: &str| json!({ "id": id, "kind": "computer", "name": id });
    let on = |id: &str, place: &str, kind: &str| json!({ "id": id, "name": id, "kind": kind, "place": place, "project": { "id": "p1" } });
    let row = |id: &str, workspace: &str, parent: Option<&str>| {
        let mut r = json!({ "id": format!("r-{id}"), "threadId": id, "workspaceId": workspace, "harness": "claude", "status": "running",
            "prompt": id, "project": { "id": "p1", "name": "repo" }, "startedAt": now - 60_000. });
        if let Some(parent) = parent {
            r["parentThreadId"] = json!(parent);
            r["rootThreadId"] = json!(parent);
        }
        r
    };
    let mut boat_machine = on("w-boat", "boat", "cloud");
    boat_machine["parentThreadId"] = json!("t-b");
    let mut empty_machine = on("w-empty", "boat", "cloud");
    empty_machine["parentThreadId"] = json!("t-b");
    let snap = Snapshot {
        sessions: crate::screens::sidebar::shared::rows_of(vec![
            row("t-a", "w-a", None),
            row("t-b", "w-b", None),
            row("t-sub", "w-boat", Some("t-b")),
        ]),
        workspaces: vec![on("w-a", "mumbai", "place"), on("w-b", "boat", "place"), boat_machine, empty_machine],
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "mumbai" })],
        places: vec![place("mumbai"), place("boat")],
        preferences: json!({}),
        ..Snapshot::default()
    };
    push(&sidebar, Change::Whole(Box::new(snap)), cx);
    sidebar.update(cx, |s, cx| {
        s.computer = Some("mumbai".into());
        s.refold(cx);
    });
    cx.run_until_parked();
    let drawn = sidebar.read_with(cx, |s, _| {
        s.drawn_rows
            .iter()
            .filter_map(|d| match d {
                Drawn::Line(ix) => match &s.lines[*ix].item {
                    Item::Tile(t) => Some(s.threads[t.thread].key.clone()),
                    Item::Machine { workspace } => Some(format!("machine {workspace}")),
                    _ => None,
                },
                Drawn::Workspace(id) => Some(format!("card {id}")),
                _ => None,
            })
            .collect::<Vec<_>>()
    });
    assert_eq!(drawn, ["t-a"], "only the picked computer's rows");
}
