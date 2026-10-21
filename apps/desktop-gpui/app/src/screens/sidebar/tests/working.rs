// SPDX-License-Identifier: AGPL-3.0-only
//! The Working section: every root whose tree runs stands under its head, below Pinned and Needs you, in the list's
//! own order, and moves only when work starts or stops in its tree.
use super::*;

const MIN: f64 = 60_000.;
const HOUR: f64 = 60. * MIN;
const DAY: f64 = 24. * HOUR;

/// A thread's row, started `ago` ms before `now`: running, or ended half an hour ago and read then.
fn thread(key: &str, parent: Option<&str>, running: bool, ago: f64, now: f64) -> Value {
    let mut r = json!({
        "id": format!("r-{key}"), "threadId": key, "workspaceId": "w0", "harness": "claude",
        "status": if running { "running" } else { "completed" }, "prompt": format!("thread {key}"),
        "project": { "id": "p1", "name": "repo" }, "startedAt": now - ago,
    });
    if !running {
        r["endedAt"] = json!(now - 30. * MIN);
        r["readAt"] = json!(now - 30. * MIN);
    }
    if let Some(parent) = parent {
        r["parentThreadId"] = json!(parent);
        r["rootThreadId"] = json!(parent);
    }
    r
}

/// Roots a, b and c from this morning over `old`, started three days ago, every one finished and read, none settled.
fn morning(now: f64, more: Vec<Value>) -> Snapshot {
    let mut snap = snapshot(0, now);
    let rows = [("a", HOUR), ("b", 2. * HOUR), ("c", 3. * HOUR), ("old", 3. * DAY)].map(|(k, ago)| thread(k, None, false, ago, now));
    snap.sessions = crate::screens::sidebar::shared::rows_of(rows.into_iter().chain(more).collect());
    snap
}

fn shown(snap: Snapshot, cx: &mut TestAppContext) -> (Entity<Sidebar>, &mut VisualTestContext) {
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snap)), cx));
    cx.run_until_parked();
    (sidebar, cx)
}

fn push(sidebar: &Entity<Sidebar>, row: Value, cx: &mut VisualTestContext) {
    let id = row["id"].as_str().unwrap().to_owned();
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id, row: Some(row) }, cx));
    cx.run_until_parked();
}

/// The list's top level as drawn: each head by its section's id after a `#`, each root by its key, a Needs you copy
/// marked `*`.
fn layout(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Vec<String> {
    sidebar.read_with(cx, |s, _| {
        let top = s.drawn_rows.iter().filter_map(|d| match d {
            Drawn::Line(ix) if s.lines[*ix].depth == 0 => match &s.lines[*ix].item {
                Item::Head { section, .. } => Some(format!("#{}", section.id())),
                Item::Tile(t) => Some(format!("{}{}", s.threads[t.thread].key, if t.inbox { "*" } else { "" })),
                _ => None,
            },
            _ => None,
        });
        top.collect()
    })
}

fn seq(keys: &[&str]) -> Vec<String> {
    keys.iter().map(|k| k.to_string()).collect()
}

/// Catches: a tree started days ago whose sub-thread runs left under this morning's finished roots, and a tree kept
/// at the top after its last running thread ends.
#[gpui_kit::test]
fn a_root_from_days_ago_stands_in_working_while_its_sub_thread_runs_and_goes_back_after(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = shown(morning(now, Vec::new()), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old"]));
    push(&sidebar, thread("kid", Some("old"), true, 0., now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "old", "a", "b", "c"]), "the sub-thread runs, so its tree leads the list");
    push(&sidebar, thread("kid", Some("old"), false, HOUR, now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old"]), "it ended, so its tree is back at its start-order place");
}

/// Catches: Working ordered by when the work began, or by the latest event, where it keeps the list's start order:
/// old's sub-thread starts last and its row lands last, and old still stands under the root started after it.
#[gpui_kit::test]
fn a_root_started_later_stands_above_in_working_whichever_work_began_first(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = shown(morning(now, vec![thread("new", None, false, 10. * MIN, now)]), cx);
    push(&sidebar, thread("nk", Some("new"), true, 5. * MIN, now), cx);
    push(&sidebar, thread("kid", Some("old"), true, 0., now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "new", "old", "a", "b", "c"]));
    push(&sidebar, thread("kid", Some("old"), false, HOUR, now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "new", "a", "b", "c", "old"]));
}

/// Catches: a row moved by a streamed event, a new title or the clock's tick while the same threads run: the stream
/// is old's, the lower of two trees in Working. Every row the list draws is counted, sub-threads and folds with the
/// roots.
#[gpui_kit::test]
fn no_row_moves_while_a_running_thread_only_streams(cx: &mut TestAppContext) {
    let now = now_ms();
    let more = vec![
        thread("new", None, false, 10. * MIN, now),
        thread("nk", Some("new"), true, 5. * MIN, now),
        thread("kid", Some("old"), true, 5. * MIN, now),
        thread("done", Some("old"), false, 2. * HOUR, now),
    ];
    let (sidebar, cx) = shown(morning(now, more), cx);
    let rows = |cx: &mut VisualTestContext| sidebar.read_with(cx, |s, _| s.line_ids.clone());
    let before = rows(cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "new", "old", "a", "b", "c"]));
    for i in 0..100 {
        let mut row = thread("kid", Some("old"), true, 5. * MIN, now);
        row["lastLine"] = json!(format!("line {i}"));
        if i == 50 {
            row["harnessTitle"] = json!("A new title");
        }
        push(&sidebar, row, cx);
        assert_eq!(rows(cx), before, "event {i} moved a row");
    }
    sidebar.update(cx, |s, cx| s.refresh(cx));
    assert_eq!(rows(cx), before, "the clock's tick moved a row");
}

/// Catches: a pinned tree pulled out of Pinned by its running work, a waiting thread no longer listed on its own in
/// Needs you, and a lone root that only asks drawn as working.
#[gpui_kit::test]
fn pinned_stays_above_and_needs_you_lists_a_waiting_thread_on_its_own(cx: &mut TestAppContext) {
    let now = now_ms();
    let mut pin = thread("pin", None, false, 2. * DAY, now);
    pin["pinnedAt"] = json!(now - DAY);
    let mut ask = thread("ask", Some("old"), true, MIN, now);
    ask["asking"] = json!(true);
    let mut alone = thread("alone", None, true, 5. * MIN, now);
    alone["asking"] = json!(true);
    let more = vec![pin, thread("pk", Some("pin"), true, 0., now), thread("ok", Some("old"), true, 0., now), ask, alone];
    let (sidebar, cx) = shown(morning(now, more), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#pinned", "pin", "#needs-you", "alone", "ask*", "#working", "old", "a", "b", "c"]));
    push(&sidebar, thread("ok", Some("old"), false, HOUR, now), cx);
    assert_eq!(
        layout(&sidebar, cx),
        seq(&["#pinned", "pin", "#needs-you", "alone", "ask*", "a", "b", "c", "old"]),
        "a tree whose one running thread asks is waiting on the person, not working"
    );
}

/// Catches: a tree that leaves Working kept out of Settled though the settle rule takes it, or one in Settled left
/// there while a thread of it runs.
#[gpui_kit::test]
fn after_its_work_stops_a_tree_goes_where_the_settle_rule_puts_it(cx: &mut TestAppContext) {
    let now = now_ms();
    let mut stale = thread("stale", None, false, 4. * DAY, now);
    stale["endedAt"] = json!(now - 5. * HOUR);
    stale["readAt"] = json!(now - 5. * HOUR);
    let (sidebar, cx) = shown(morning(now, vec![stale]), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old", "#settled"]));
    push(&sidebar, thread("sk", Some("stale"), true, 0., now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "stale", "a", "b", "c", "old"]));
    push(&sidebar, thread("sk", Some("stale"), false, HOUR, now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old", "stale"]), "read half an hour ago, it is not quiet long enough to settle");
    let mut quiet = thread("sk", Some("stale"), false, 4. * HOUR, now);
    quiet["endedAt"] = json!(now - 3. * HOUR);
    quiet["readAt"] = json!(now - 3. * HOUR);
    push(&sidebar, quiet, cx);
    assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old", "#settled"]), "read three hours ago, the tree settles");
}

/// Catches: a machine a tree's thread started that holds no thread yet left out of Working while its start runs, or
/// kept there once it stood or failed, which would hold the tree in Working with nothing running.
#[gpui_kit::test]
fn a_machine_still_starting_under_its_lead_puts_the_tree_in_working(cx: &mut TestAppContext) {
    let now = now_ms();
    let made = chrono::DateTime::from_timestamp_millis(now as i64).unwrap().to_rfc3339();
    let machine = |spawn: Value, extra: Value| {
        let mut w = json!({ "id": "m1", "name": "m1", "kind": "cloud", "machineId": "fk1", "phase": "running", "parentThreadId": "old", "createdAt": made, "sub": { "spawn": spawn } });
        w.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        w
    };
    let mut snap = morning(now, Vec::new());
    snap.workspaces.push(machine(json!({ "startedAt": now }), json!({})));
    let (sidebar, cx) = shown(snap, cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "old", "a", "b", "c"]));
    for (spawn, extra, what) in [
        (json!({ "startedAt": now, "readyAt": now }), json!({}), "stood"),
        (json!({ "startedAt": now, "failed": "no capacity" }), json!({}), "failed"),
        (json!({ "startedAt": now }), json!({ "phase": "gone", "machineId": "" }), "refused before any machine stood"),
    ] {
        let workspaces = vec![workspace("w0"), machine(spawn, extra)];
        sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(workspaces), cx));
        cx.run_until_parked();
        assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old"]), "a machine whose start {what} is not working");
    }
}

/// Catches: a Working root offered Move up, Move down or Move to top, or moved by Alt+Shift+Up, which would key it
/// among Working's roots and land it far from its place in the list once its work stops.
#[gpui_kit::test]
fn a_root_in_working_takes_no_move(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = shown(morning(now, vec![thread("kid", Some("old"), true, 0., now)]), cx);
    writes::WRITTEN.with(|w| w.borrow_mut().clear());
    let items = sidebar.read_with(cx, |s, _| {
        let tile = s.lines.iter().find_map(|l| match &l.item {
            Item::Tile(t) if l.depth == 0 && s.threads[t.thread].key == "old" => Some(t.clone()),
            _ => None,
        });
        menu::items_for(s, &tile.expect("old is drawn"), "old")
    });
    assert!(!items.iter().any(|i| matches!(i.act, menu::Act::Move(_))), "no Move item for a root in Working");
    assert!(items.iter().any(|i| i.act == menu::Act::Pin(true)), "it still pins");
    sidebar.update(cx, |s, cx| {
        for to in [model::Move::Up, model::Move::Down, model::Move::Top] {
            s.move_root("old", to, cx);
        }
    });
    assert_eq!(writes::WRITTEN.with(|w| w.borrow().len()), 0, "a move writes nothing");
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "old", "a", "b", "c"]));
}

/// Catches: Ctrl+Tab stepping the top level in an order the list no longer draws, once Working stands above the rest.
#[gpui_kit::test]
fn ctrl_tab_steps_the_top_level_in_drawn_order_with_working_first(cx: &mut TestAppContext) {
    let now = now_ms();
    let running = vec![thread("bk", Some("b"), true, 0., now), thread("kid", Some("old"), true, 0., now)];
    let (sidebar, cx) = shown(morning(now, running), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "b", "old", "a", "c"]));
    let step = |by: isize, cx: &mut VisualTestContext| {
        sidebar.update(cx, |s, cx| {
            s.step_level(by, cx);
            s.selected.clone().unwrap_or_default()
        })
    };
    assert_eq!(step(1, cx), "b", "with none open, down starts at Working's first root");
    sidebar.update(cx, |s, cx| _ = s.mark("a", cx));
    let down: Vec<String> = (0..4).map(|_| step(1, cx)).collect();
    assert_eq!(down, seq(&["c", "b", "old", "a"]), "down from a: the rest of the list, then Working's roots from the top");
    let up: Vec<String> = (0..4).map(|_| step(-1, cx)).collect();
    assert_eq!(up, seq(&["old", "b", "c", "a"]));
}
