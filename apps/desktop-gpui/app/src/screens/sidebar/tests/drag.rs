// SPDX-License-Identifier: AGPL-3.0-only
//! A root tree dragged in the list (`treeDrag.ts`): the drag starts off a pressed move, and its drop writes the marks
//! today's writes for the place it lands on.
use super::*;
use gpui_kit::{point, Bounds, Modifiers, MouseButton, Point};

fn sel(selector: String) -> &'static str {
    Box::leak(selector.into_boxed_str())
}

fn tile(key: &str, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    cx.debug_bounds(sel(format!("tile-{key}"))).unwrap_or_else(|| panic!("tile-{key} is drawn"))
}

fn written() -> Vec<(&'static str, Value)> {
    writes::WRITTEN.with(|w| std::mem::take(&mut *w.borrow_mut()))
}

fn listed(snap: Snapshot, cx: &mut TestAppContext) -> (Entity<Sidebar>, &mut VisualTestContext) {
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snap)), cx));
    cx.run_until_parked();
    written();
    (sidebar, cx)
}

/// Presses on the tile of `key` and moves past the drag threshold: the drag starts and the heads stand.
fn lift(key: &str, cx: &mut VisualTestContext) {
    let from = tile(key, cx).center();
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(from + point(px(0.), px(10.)), MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn hover(to: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_move(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

fn drop_at(to: Point<Pixels>, cx: &mut VisualTestContext) {
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.run_until_parked();
}

/// The marks `sessions.mark` was asked to write, each with the threads it names.
fn marks(writes: &[(&'static str, Value)]) -> Vec<(Vec<String>, Value)> {
    writes
        .iter()
        .filter(|(op, _)| *op == "sessions.mark")
        .map(|(_, p)| (serde_json::from_value(p["threadIds"].clone()).unwrap(), p["marks"].clone()))
        .collect()
}

fn lower_half(b: Bounds<Pixels>) -> Point<Pixels> {
    point(b.center().x, b.top() + b.size.height * 0.75)
}

fn upper_half(b: Bounds<Pixels>) -> Point<Pixels> {
    point(b.center().x, b.top() + b.size.height * 0.25)
}

/// Where drawn row `ix` was laid out.
fn row_bounds(sidebar: &Entity<Sidebar>, find: impl Fn(&Sidebar, &Line) -> bool, cx: &mut VisualTestContext) -> Bounds<Pixels> {
    sidebar.read_with(cx, |s, _| {
        let ix = s.drawn_rows.iter().position(|d| matches!(d, Drawn::Line(l) if find(s, &s.lines[*l]))).expect("the row is drawn");
        s.list.bounds_for_item(ix).expect("the row is laid out")
    })
}

fn head(section: Section) -> impl Fn(&Sidebar, &Line) -> bool {
    move |_, l| matches!(l.item, Item::Head { section: s, .. } if s == section)
}

fn heads(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) -> Vec<Section> {
    sidebar.read_with(cx, |s, _| {
        s.lines.iter().filter_map(|l| if let Item::Head { section, .. } = l.item { Some(section) } else { None }).collect()
    })
}

/// The roots of the list as drawn, top to bottom.
fn roots(sidebar: &Entity<Sidebar>, section: Section, cx: &mut VisualTestContext) -> Vec<String> {
    sidebar.read_with(cx, |s, _| {
        s.lines
            .iter()
            .filter(|l| l.depth == 0 && l.section == section)
            .filter_map(|l| if let Item::Tile(t) = &l.item { Some(s.threads[t.thread].key.clone()) } else { None })
            .collect()
    })
}

fn expected(sidebar: &Entity<Sidebar>, key: &str, section: Section, at: usize, cx: &mut VisualTestContext) -> Vec<(Vec<String>, Value)> {
    sidebar.read_with(cx, |s, _| {
        s.view().move_root(key, model::Move::To { section, at }).unwrap().into_iter().map(|m| (m.thread_ids, m.marks)).collect()
    })
}

/// [`snapshot`]'s rows with `edit` applied to the row of each thread it names.
fn edited(n: usize, now: f64, edit: impl Fn(&str, &mut Value)) -> Snapshot {
    let mut snap = snapshot(n, now);
    let sent = (0..n).map(|i| {
        let mut row = row(i, "w0", now);
        let key = row["threadId"].as_str().unwrap().to_owned();
        edit(&key, &mut row);
        row
    });
    snap.sessions = crate::screens::sidebar::shared::rows_of(sent.collect());
    snap
}

/// Catches: a pressed move on a root's tile that starts no drag, or a drop on the lower half of the root under it
/// that writes nothing (the owner's "drags do nothing"), where today's marks the edge after that root, moves the tree
/// past it with one `sessions.mark` of its new key, and draws it there before the host's row lands.
#[gpui_kit::test]
fn a_root_dragged_onto_the_lower_half_of_the_next_lands_after_it(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = listed(snapshot(4, now), cx);
    lift("t0", cx);
    assert!(cx.update(|_, cx| cx.has_active_drag()), "a pressed move on the tile starts a drag");
    let t1 = tile("t1", cx);
    hover(lower_half(t1), cx);
    let edge = cx.debug_bounds("drop-edge-t1-after").expect("the edge after t1 is marked");
    assert!((edge.top() - (t1.bottom() - px(1.))).abs() < px(1.), "the edge stands on t1's foot: {edge:?} {t1:?}");
    assert_eq!(edge.size.height, px(2.));
    let want = expected(&sidebar, "t0", Section::Threads, 1, cx);
    drop_at(lower_half(t1), cx);
    let got = marks(&written());
    assert_eq!(got, want);
    let key = got[0].1["order"].as_f64().unwrap();
    assert!(key < now - 60_001. && key > now - 60_002., "t0 lands between t1 and t2: {key}");
    assert_eq!(roots(&sidebar, Section::Threads, cx), ["t1", "t0", "t2", "t3"], "the list holds the move until the host's rows land");
    assert!(cx.debug_bounds("drop-edge-t1-after").is_none(), "the edge goes with the drop");
}

/// Catches: a drop on the upper half of a root landing after it, or no edge marked before it.
#[gpui_kit::test]
fn a_root_dragged_onto_the_upper_half_of_a_root_above_lands_before_it(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = listed(snapshot(4, now), cx);
    lift("t2", cx);
    let to = upper_half(tile("t0", cx));
    hover(to, cx);
    assert!(cx.debug_bounds("drop-edge-t0-before").is_some(), "the edge before t0 is marked");
    let want = expected(&sidebar, "t2", Section::Threads, 0, cx);
    drop_at(to, cx);
    let got = marks(&written());
    assert_eq!(got, want);
    assert!(got[0].1["order"].as_f64().unwrap() > now - 60_000., "t2 takes a key over t0's");
    assert_eq!(roots(&sidebar, Section::Threads, cx), ["t2", "t0", "t1", "t3"]);
}

/// Catches: the edges on either side of the dragged tree, or the tree itself, read as a place, where today's marks
/// nothing there and a drop there writes nothing.
#[gpui_kit::test]
fn the_edges_beside_the_dragged_tree_are_no_place(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(snapshot(4, now_ms()), cx);
    lift("t1", cx);
    for (to, what) in [(upper_half(tile("t2", cx)), "before t2"), (lower_half(tile("t0", cx)), "after t0"), (tile("t1", cx).center(), "t1")]
    {
        hover(to, cx);
        assert_eq!(sidebar.read_with(cx, |s, _| s.drag.at.clone()), None, "{what} is no place");
        assert!(cx.debug_bounds("drop-edge-t2-before").is_none() && cx.debug_bounds("drop-edge-t0-after").is_none(), "{what}: no edge");
    }
    drop_at(tile("t1", cx).center(), cx);
    assert_eq!(marks(&written()), Vec::new());
}

/// Catches: no Pinned head to drop on while Pinned is empty, or a drop there that pins nothing; today's shows every
/// section's head while a tree is dragged, fills the section under the pointer, and pins the tree.
#[gpui_kit::test]
fn a_root_dropped_on_the_empty_pinned_head_is_pinned(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(snapshot(4, now_ms()), cx);
    lift("t1", cx);
    let to = row_bounds(&sidebar, head(Section::Pinned), cx).center();
    hover(to, cx);
    assert!(cx.debug_bounds("drop-over-pinned").is_some(), "the Pinned section takes the row hover's fill");
    drop_at(to, cx);
    assert_eq!(marks(&written()), vec![(vec!["t1".to_owned()], serde_json::json!({ "pinned": true }))]);
    assert_eq!(roots(&sidebar, Section::Pinned, cx), ["t1"], "Pinned holds it until the host's row lands");
}

/// Catches: a pinned root dropped beside a root of the list that stays pinned, or lands with no key of the list.
#[gpui_kit::test]
fn a_pinned_root_dropped_on_an_edge_of_the_list_leaves_pinned(cx: &mut TestAppContext) {
    let now = now_ms();
    let snap = edited(4, now, |key, row| {
        if key == "t0" {
            row["pinnedAt"] = serde_json::json!(now - 1_000.);
        }
    });
    let (sidebar, cx) = listed(snap, cx);
    assert_eq!(roots(&sidebar, Section::Pinned, cx), ["t0"]);
    lift("t0", cx);
    let to = lower_half(tile("t2", cx));
    hover(to, cx);
    let want = expected(&sidebar, "t0", Section::Threads, 2, cx);
    drop_at(to, cx);
    let got = marks(&written());
    assert_eq!(got, want);
    assert_eq!(got[0].1["pinned"], Value::Bool(false), "{got:?}");
    assert!(got[0].1["order"].is_number(), "{got:?}");
    assert_eq!(roots(&sidebar, Section::Threads, cx), ["t1", "t2", "t0", "t3"]);
}

/// Catches: no Settled head to drop on while nothing is settled, a drop there that settles nothing, or a working
/// tree settled by a drop, which today's refuses with the menu's sentence.
#[gpui_kit::test]
fn a_root_dropped_on_settled_is_settled_and_a_working_one_is_refused(cx: &mut TestAppContext) {
    let snap = edited(4, now_ms(), |key, row| {
        if key == "t2" {
            row["status"] = serde_json::json!("running");
            row.as_object_mut().unwrap().remove("endedAt");
        }
    });
    let (sidebar, cx) = listed(snap, cx);
    assert!(!heads(&sidebar, cx).contains(&Section::Settled), "nothing is settled");
    lift("t1", cx);
    let to = row_bounds(&sidebar, head(Section::Settled), cx).center();
    hover(to, cx);
    assert!(cx.debug_bounds("drop-over-settled").is_some(), "the Settled row takes the fill");
    drop_at(to, cx);
    assert_eq!(written(), vec![("sessions.settle", serde_json::json!({ "threadIds": ["t1"], "processes": "ask" }))]);
    lift("t2", cx);
    let to = row_bounds(&sidebar, head(Section::Settled), cx).center();
    hover(to, cx);
    drop_at(to, cx);
    assert_eq!(written(), Vec::new(), "a working tree is not settled");
    assert_eq!(sidebar.read_with(cx, |s, _| s.refusal.clone()).as_deref(), Some("A thread in it is still working"));
}

/// Catches: the dragged tile drawn as it rests, its tree still drawn under it, or only the heads of sections that
/// hold something standing; today's dims the tile, folds its tree and shows every head that takes a drop, and puts
/// all back after. t0's running child stands it in Working, whose head shows only while it holds a row.
#[gpui_kit::test]
fn while_dragged_the_tile_dims_its_tree_folds_and_every_head_stands(cx: &mut TestAppContext) {
    let snap = edited(4, now_ms(), |key, row| {
        if key == "t3" {
            row["parentThreadId"] = serde_json::json!("t0");
            row["rootThreadId"] = serde_json::json!("t0");
            row["status"] = serde_json::json!("running");
            row.as_object_mut().unwrap().remove("endedAt");
        }
    });
    let (sidebar, cx) = listed(snap, cx);
    assert!(cx.debug_bounds("tile-t3").is_some(), "the child is drawn under t0");
    assert_eq!(heads(&sidebar, cx), [Section::Working]);
    lift("t0", cx);
    assert!(cx.debug_bounds("drag-source-t0").is_some(), "the carried tile is dimmed");
    assert!(cx.debug_bounds("tile-t3").is_none(), "its tree is folded");
    assert_eq!(heads(&sidebar, cx), [Section::Pinned, Section::NeedsYou, Section::Working, Section::Settled]);
    drop_at(tile("t0", cx).center(), cx);
    assert_eq!(sidebar.read_with(cx, |s, _| s.dragging().map(str::to_owned)), None);
    assert!(cx.debug_bounds("drag-source-t0").is_none() && cx.debug_bounds("tile-t3").is_some(), "the tree stands again");
    assert_eq!(heads(&sidebar, cx), [Section::Working]);
    assert_eq!(marks(&written()), Vec::new());
}

/// Catches: the head of a group one send to several models opened that cannot be dragged; today's drags it as one.
#[gpui_kit::test]
fn a_group_head_drags_its_members_as_one(cx: &mut TestAppContext) {
    let (sidebar, cx) = listed(snapshot(6, now_ms()), cx);
    let from = cx.debug_bounds("group-a4").expect("the group's head is drawn").center();
    cx.simulate_mouse_move(from, None, Modifiers::none());
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    hover(from + point(px(0.), px(10.)), cx);
    assert_eq!(sidebar.read_with(cx, |s, _| s.dragging().map(str::to_owned)).as_deref(), Some("attempt:a4"));
    let to = row_bounds(&sidebar, head(Section::Pinned), cx).center();
    hover(to, cx);
    drop_at(to, cx);
    assert_eq!(marks(&written()), vec![(vec!["t5".to_owned(), "t4".to_owned()], serde_json::json!({ "pinned": true }))]);
}

/// Catches: a drop taken by Working, which is drawn from what runs; an empty Working head standing to take one; or
/// a root dragged out of Working that writes other marks than the same drop from the list, or leaves Working while
/// its tree still runs.
#[gpui_kit::test]
fn working_takes_no_drop_and_a_root_dragged_out_writes_the_lists_marks(cx: &mut TestAppContext) {
    let now = now_ms();
    let (sidebar, cx) = listed(snapshot(4, now), cx);
    lift("t0", cx);
    assert_eq!(heads(&sidebar, cx), [Section::Pinned, Section::NeedsYou, Section::Settled], "no Working head while none runs");
    drop_at(tile("t0", cx).center(), cx);
    let mut kid = row(3, "w0", now);
    kid["parentThreadId"] = serde_json::json!("t2");
    kid["status"] = serde_json::json!("running");
    kid.as_object_mut().unwrap().remove("endedAt");
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "r3".into(), row: Some(kid) }, cx));
    cx.run_until_parked();
    assert_eq!(
        (roots(&sidebar, Section::Working, cx), roots(&sidebar, Section::Threads, cx)),
        (vec!["t2".into()], vec!["t0".into(), "t1".into()])
    );

    lift("t0", cx);
    for at in [upper_half(tile("t2", cx)), lower_half(tile("t2", cx)), row_bounds(&sidebar, head(Section::Working), cx).center()] {
        hover(at, cx);
        assert_eq!(sidebar.read_with(cx, |s, _| s.drag.at.clone()), None, "Working takes no drop at {at:?}");
        assert!(cx.debug_bounds("drop-over-working").is_none());
    }
    drop_at(lower_half(tile("t2", cx)), cx);
    assert_eq!(marks(&written()), Vec::new());

    lift("t2", cx);
    hover(lower_half(tile("t0", cx)), cx);
    let want = expected(&sidebar, "t2", Section::Threads, 1, cx);
    drop_at(lower_half(tile("t0", cx)), cx);
    let got = marks(&written());
    assert_eq!(got, want);
    let key = got[0].1["order"].as_f64().unwrap();
    assert_eq!(got[0].1, serde_json::json!({ "order": key }), "only the list's order key, as a drop from the list writes");
    assert!(key < now - 60_000. && key > now - 60_001., "between t0 and t1: {key}");
    assert_eq!(roots(&sidebar, Section::Working, cx), ["t2"], "it stays in Working while its tree runs");
}
