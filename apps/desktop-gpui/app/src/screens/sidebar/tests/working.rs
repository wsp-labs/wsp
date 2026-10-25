// SPDX-License-Identifier: AGPL-3.0-only
//! The Working section: every root whose tree runs stands under its head, below Pinned and Needs you, in start order
//! whatever a move wrote, and moves only when work starts or stops in its tree.
use super::drag::{drop_at, head, hover, lift, lift_at, marks, row_bounds, written};
use super::*;
use crate::local_store::{self, LocalStore};
use gpui_kit::Modifiers;

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
/// marked `*`, a workspace being made by its key after a `+`.
fn layout(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Vec<String> {
    sidebar.read_with(cx, |s, _| {
        let top = s.drawn_rows.iter().filter_map(|d| match d {
            Drawn::Line(ix) if s.lines[*ix].depth == 0 => match &s.lines[*ix].item {
                Item::Head { section, .. } => Some(format!("#{}", section.id())),
                Item::Tile(t) => Some(format!("{}{}", s.threads[t.thread].key, if t.inbox { "*" } else { "" })),
                _ => None,
            },
            Drawn::Creation(i) => Some(format!("+{}", s.around.creations[*i].key)),
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

/// Catches: a tree held in Working by a machine one of its threads started that holds no thread yet, which today's
/// does not count as working (threadTree.ts workingIn counts threads); the machine still stands under its lead.
#[gpui_kit::test]
fn a_machine_still_starting_under_its_lead_leaves_the_tree_out_of_working(cx: &mut TestAppContext) {
    let now = now_ms();
    let made = chrono::DateTime::from_timestamp_millis(now as i64).unwrap().to_rfc3339();
    let machine = json!({ "id": "m1", "name": "m1", "kind": "cloud", "machineId": "fk1", "phase": "running", "parentThreadId": "old", "createdAt": made, "sub": { "spawn": { "startedAt": now } } });
    let mut snap = morning(now, Vec::new());
    snap.workspaces.push(machine);
    let (sidebar, cx) = shown(snap, cx);
    assert_eq!(layout(&sidebar, cx), seq(&["a", "b", "c", "old"]));
    assert!(sidebar
        .read_with(cx, |s, _| s.lines.iter().any(|l| l.depth == 1 && matches!(&l.item, Item::Machine { workspace } if workspace == "m1"))));
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

/// Catches: Working sorted by the list's key, which a move writes, where today's keeps start order: old is moved to
/// the list's top, and in Working it still stands under b, started after it.
#[gpui_kit::test]
fn a_move_written_on_a_root_does_not_reorder_working(cx: &mut TestAppContext) {
    let now = now_ms();
    let running = vec![thread("bk", Some("b"), true, 0., now), thread("kid", Some("old"), true, 0., now)];
    let (sidebar, cx) = shown(morning(now, running), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "b", "old", "a", "c"]));
    let mut moved = thread("old", None, false, 3. * DAY, now);
    moved["order"] = json!(now + HOUR);
    push(&sidebar, moved, cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "b", "old", "a", "c"]), "the move is the list's, not Working's");
    push(&sidebar, thread("kid", Some("old"), false, HOUR, now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "b", "old", "a", "c"]), "once its work stops the move holds in the list");
}

/// Catches: a snoozed tree whose thread runs drawn at the foot of the list, where today's draws its root alone in
/// Working, in its start place, carrying how many work.
#[gpui_kit::test]
fn a_snoozed_tree_with_a_running_thread_stands_in_working_in_its_start_place(cx: &mut TestAppContext) {
    let now = now_ms();
    let mut b = thread("b", None, false, 2. * HOUR, now);
    b["snoozedUntil"] = json!(now + DAY);
    let more = vec![b, thread("bk", Some("b"), true, 0., now), thread("kid", Some("old"), true, 0., now)];
    let (sidebar, cx) = shown(morning(now, more), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "b", "old", "a", "c"]));
    let snoozed = sidebar.read_with(cx, |s, _| {
        s.lines.iter().find_map(|l| match &l.item {
            Item::Tile(t) if s.threads[t.thread].key == "b" => Some((t.snoozed_working, t.moves, l.section)),
            _ => None,
        })
    });
    assert_eq!(snoozed, Some((Some(1), false, Section::Working)), "one works in it, and it takes no move");
    assert_eq!(heads_counted(&sidebar, cx), vec![(Section::Working, 3)], "b alone, old and its running kid");
}

/// Each head drawn, with the count it shows.
fn heads_counted(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Vec<(Section, usize)> {
    sidebar.read_with(cx, |s, _| {
        s.lines.iter().filter_map(|l| if let Item::Head { section, count, .. } = l.item { Some((section, count)) } else { None }).collect()
    })
}

/// Catches: a workspace being made drawn at the top of the list outside Working, uncounted, or no Working head over
/// it when nothing else works; and one kept drawn under a folded Working, or a refused one under a folded Needs you.
#[gpui_kit::test]
fn a_workspace_being_made_stands_under_working_and_counts_in_its_head(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = shown(morning(now, Vec::new()), cx);
    let frame = json!({ "workspaceId": "w9", "name": "copy", "stage": "hostname-set" });
    sidebar.update(cx, |s, cx| s.apply(Change::Creating(frame), cx));
    cx.run_until_parked();
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "+creating:w9", "a", "b", "c", "old"]), "it holds Working open on its own");
    assert_eq!(heads_counted(&sidebar, cx), vec![(Section::Working, 1)]);
    push(&sidebar, thread("kid", Some("old"), true, 0., now), cx);
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "+creating:w9", "old", "a", "b", "c"]), "it leads the working roots");
    assert_eq!(heads_counted(&sidebar, cx), vec![(Section::Working, 3)]);
    sidebar.update(cx, |s, cx| s.toggle_section(Section::Working, cx));
    assert_eq!(layout(&sidebar, cx), seq(&["#working", "a", "b", "c"]));
    let mut asks = thread("asks", None, true, MIN, now);
    asks["asking"] = json!(true);
    push(&sidebar, asks, cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Creating(json!({ "workspaceId": "w8", "name": "gone", "stage": "failed" })), cx));
    cx.run_until_parked();
    assert_eq!(layout(&sidebar, cx), seq(&["#needs-you", "+creating:w8", "asks", "#working", "a", "b", "c"]));
    sidebar.update(cx, |s, cx| s.toggle_section(Section::NeedsYou, cx));
    assert_eq!(layout(&sidebar, cx), seq(&["#needs-you", "#working", "a", "b", "c"]));
}

/// Catches: Working taking no drop, so a pinned tree that works cannot be dragged out of Pinned onto it; a drop there
/// from the list, from the Needs you copy of a pinned root, or of a pinned tree that does not work, writing anything.
#[gpui_kit::test]
fn a_pinned_working_tree_dropped_on_working_is_unpinned(cx: &mut TestAppContext) {
    let now = now_ms();
    let mut pin = thread("pin", None, false, 2. * DAY, now);
    pin["pinnedAt"] = json!(now - DAY);
    let mut idle = thread("idle", None, false, 2. * DAY, now);
    idle["pinnedAt"] = json!(now - 2. * DAY);
    let mut asks = thread("asks", None, true, 2. * DAY, now);
    asks["pinnedAt"] = json!(now - 3. * DAY);
    asks["asking"] = json!(true);
    let kids = [thread("pk", Some("pin"), true, 0., now), thread("kid", Some("old"), true, 0., now)];
    let (sidebar, cx) = shown(morning(now, [pin, idle, asks].into_iter().chain(kids).collect()), cx);
    written();
    assert_eq!(layout(&sidebar, cx), seq(&["#pinned", "pin", "idle", "asks", "#needs-you", "asks*", "#working", "old", "a", "b", "c"]));
    let on_working = |cx: &mut VisualTestContext| row_bounds(&sidebar, head(Section::Working), cx).center();
    let copy = row_bounds(
        &sidebar,
        |s, l| l.section == Section::NeedsYou && matches!(&l.item, Item::Tile(t) if s.threads[t.thread].key == "asks"),
        cx,
    );
    let lifts = [
        (Some("a"), false, "a root of the list"),
        (Some("idle"), true, "a pinned tree with nothing running"),
        (None, false, "the Needs you copy of a pinned root"),
    ];
    for (key, shows, what) in lifts {
        match key {
            Some(key) => lift(key, cx),
            None => lift_at(copy.center(), cx),
        }
        let to = on_working(cx);
        hover(to, cx);
        assert_eq!(cx.debug_bounds("drop-over-working").is_some(), shows, "{what} over Working");
        drop_at(to, cx);
        assert_eq!(marks(&written()), Vec::new(), "{what} dropped on Working writes nothing");
    }
    lift("pin", cx);
    let to = on_working(cx);
    hover(to, cx);
    assert!(cx.debug_bounds("drop-over-working").is_some(), "Working takes the drop and shows it");
    drop_at(to, cx);
    assert_eq!(marks(&written()), vec![(vec!["pin".to_owned()], json!({ "pinned": false }))]);
}

/// Catches: Working's fold kept in memory alone, so a new launch opens it again; Setting up's left out of the key
/// today's keeps it in; or one window writing its whole set over a fold another window kept.
#[gpui_kit::test]
fn the_working_fold_is_kept_for_the_next_launch(cx: &mut TestAppContext) {
    let now = now_ms();
    let path = local_store::scratch("sidebar-folded");
    cx.update(|cx| local_store::init(path.clone(), cx));
    let mut snap = morning(now, vec![thread("kid", Some("old"), true, 0., now)]);
    snap.places.push(json!({ "id": "m2", "kind": "computer", "name": "m2", "setup": { "state": "running", "steps": [] } }));
    let (sidebar, cx) = shown(snap, cx);
    let other = cx.update(|_, cx| cx.new(Sidebar::new));
    sidebar.update(cx, |s, cx| s.toggle_section(Section::Working, cx));
    other.update(cx, |s, cx| s.toggle_section(Section::Pinned, cx));
    let setting_up = cx.debug_bounds("head-setting-up").expect("Setting up's head is drawn");
    cx.simulate_click(setting_up.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(LocalStore::open(&path).get(model::FOLDED_KEY), Some(&json!(["settled", "working", "pinned", "setting-up"])));
    let again = cx.update(|_, cx| cx.new(Sidebar::new));
    let kept = again.read_with(cx, |s, _| (s.folds.sections.clone(), s.foot_folds.contains("setting-up")));
    assert_eq!(kept, (HashSet::from([Section::Working, Section::Pinned, Section::Settled]), true), "a new sidebar opens with them folded");
}
