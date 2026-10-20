// SPDX-License-Identifier: AGPL-3.0-only
use super::*;
use crate::ui::{self, Side};

/// Catches: a thread link naming the wildcard a host bound or an unbracketed IPv6 address, where the dial and
/// today's app name the loopback the lock is dialled on; or a lock whose process is gone read as a page.
#[test]
fn the_host_page_is_where_the_lock_is_dialled() {
    let dir = tempfile::tempdir().unwrap();
    let state = dir.path().join("state.json");
    let lock = |pid: u32, address: &str| {
        let lock = format!(r#"{{"pid":{pid},"port":4100,"address":"{address}","startedAt":"2026-10-19T00:00:00.000Z"}}"#);
        std::fs::write(dir.path().join("host.lock"), lock).unwrap();
        host_page(&state)
    };
    let me = std::process::id();
    assert_eq!(lock(me, "0.0.0.0").as_deref(), Some("http://127.0.0.1:4100/"));
    assert_eq!(lock(me, "::1").as_deref(), Some("http://[::1]:4100/"));
    assert_eq!(lock(me, "127.0.0.2").as_deref(), Some("http://127.0.0.2:4100/"));
    assert_eq!(lock(0, "127.0.0.1"), None);
}
use gpui_kit::component::Root;
use gpui_kit::{size, TestAppContext, VisualTestContext};
use model::Item;
use rows::Drawn;

fn workspace(id: &str) -> Value {
    json!({ "id": id, "name": id, "kind": "local", "project": { "id": "p1" } })
}

/// A finished thread on its own row; every fiftieth and the one after it are one send to two models.
fn row(i: usize, workspace: &str, now: f64) -> Value {
    let attempt = (i >= 4 && (i - 4) % 50 < 2).then(|| format!("a{}", i - (i - 4) % 50));
    json!({
        "id": format!("r{i}"), "threadId": format!("t{i}"), "workspaceId": workspace, "harness": "claude",
        "status": "completed", "prompt": format!("thread {i}"), "project": { "id": "p1", "name": "repo" },
        "startedAt": now - 60_000. - i as f64, "endedAt": now - 30_000., "readAt": now - 30_000., "attempt": attempt,
    })
}

fn snapshot(n: usize, now: f64) -> Snapshot {
    Snapshot {
        sessions: (0..n).map(|i| row(i, "w0", now)).collect(),
        workspaces: vec![workspace("w0")],
        projects: vec![json!({ "id": "p1", "name": "repo", "path": "/repo", "computer": "here" })],
        places: vec![json!({ "id": "here", "kind": "computer", "name": "boat" })],
        preferences: json!({}),
        statuses: vec![json!({ "id": "w0", "checkout": { "branch": "main" } })],
        ..Snapshot::default()
    }
}

/// The entries of every store the sidebar keeps that grows with threads, workspaces or what the person opened.
fn sizes(s: &Sidebar) -> Vec<(&'static str, usize)> {
    let a = &s.around;
    vec![
        ("rows", s.rows.len()),
        ("threads", s.threads.len()),
        ("lines", s.lines.len()),
        ("drawn_rows", s.drawn_rows.len()),
        ("line_ids", s.line_ids.len()),
        ("working", s.working.len()),
        ("folds.finished_open", s.folds.finished_open.len()),
        ("folds.groups_shut", s.folds.groups_shut.len()),
        ("kinds", a.kinds.len()),
        ("workspaces", a.workspaces.len()),
        ("statuses", a.statuses.len()),
        ("checkouts_asked", a.checkouts_asked.len()),
        ("catalogs", a.catalogs.len()),
        ("forwards", a.forwards.len()),
        ("creations", a.creations.len()),
        ("launches", a.launches.len()),
    ]
}

fn drawn(cx: &mut TestAppContext) -> (Entity<Sidebar>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let sidebar = cx.new(Sidebar::new);
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(sidebar.clone(), window, cx));
    cx.simulate_resize(size(px(1280.), px(800.)));
    (sidebar, cx)
}

/// Draws every row of the list once, a window's height at a time, as a person scrolling to its end does.
fn scroll_through(sidebar: &Entity<Sidebar>, cx: &mut VisualTestContext) {
    let n = sidebar.read_with(cx, |s, _| s.drawn_rows.len());
    for at in (0..n).step_by(8).chain([0]) {
        sidebar.update(cx, |s, cx| {
            s.list.scroll_to(ListOffset { item_ix: at, offset_in_item: px(0.) });
            cx.notify();
        });
        cx.run_until_parked();
    }
}

fn png(shade: u8) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(128, 128, image::Rgba([shade, 0, 0, 255])).write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[gpui_kit::test]
fn a_project_icon_no_longer_named_leaves_the_image_cache(cx: &mut TestAppContext) {
    use base64::Engine as _;
    let (sidebar, cx) = drawn(cx);
    let now = now_ms();
    let (first, second) = (png(10), png(20));
    let url = |b: &[u8]| format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(b));
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now))), cx));
    for (hash, bytes) in [("aa", &first), ("bb", &second)] {
        let prefs = json!({ "projectIcon": { "p1": hash } });
        sidebar.update(cx, |s, cx| {
            s.apply(Change::Preferences(prefs.clone()), cx);
            // The reply to projects.icons, as fetch_icons takes it with a host attached.
            s.around.images.take(&json!({ hash: url(bytes) }), &prefs);
            cx.notify();
        });
        scroll_through(&sidebar, cx);
    }
    let cached = |bytes: &[u8], cx: &mut VisualTestContext| {
        let image = Arc::new(gpui_kit::Image::from_bytes(gpui_kit::ImageFormat::Png, bytes.to_vec()));
        cx.update(|_, cx| image.is_asset_cached(cx))
    };
    assert!(cached(&second, cx), "the icon named now is drawn from the image cache");
    assert!(!cached(&first, cx), "the icon no longer named is still decoded in the image cache");
    sidebar.update(cx, |s, cx| s.apply(Change::Preferences(json!({})), cx));
    scroll_through(&sidebar, cx);
    assert!(!cached(&second, cx), "an icon taken off its project is still decoded in the image cache");
}

#[gpui_kit::test]
fn five_hundred_threads_on_their_own_workspaces_come_and_go_and_leave_every_store_as_it_was(cx: &mut TestAppContext) {
    let (sidebar, cx) = drawn(cx);
    let now = now_ms();
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now))), cx));
    scroll_through(&sidebar, cx);
    let before = sidebar.read_with(cx, |s, _| sizes(s));

    let ids: Vec<String> = (4..500).map(|i| format!("w{i}")).collect();
    sidebar.update(cx, |s, cx| {
        for (i, w) in (4..500).zip(&ids) {
            s.apply(Change::Creating(json!({ "type": "workspace.creating", "workspaceId": w, "name": w, "stage": "fork-requested" })), cx);
            s.apply(Change::Created(json!({ "type": "workspace.created", "workspace": { "id": w, "name": w } })), cx);
            let all = std::iter::once("w0").chain(ids[..=i - 4].iter().map(String::as_str)).map(workspace).collect();
            s.apply(Change::Workspaces(all), cx);
            s.apply(Change::Status(json!({ "id": w, "checkout": { "branch": "main" } })), cx);
            s.apply(Change::Forward(json!({ "workspaceId": w, "port": 3000, "kind": "app" })), cx);
            s.apply(Change::Row { id: format!("r{i}"), row: Some(row(i, w, now)) }, cx);
            // What the reply to the workspace's harnesses.list puts in, as fetch_catalogs does with a host attached.
            s.around.catalogs.insert(w.clone(), vec![Catalog { harness: "claude".into(), ..Catalog::default() }]);
        }
        // The person opens some leads' Finished folds and shuts some groups, as their rows' clicks do.
        for i in (4..500).step_by(50) {
            s.toggle_finished(&format!("t{i}"), cx);
            s.toggle_group(&format!("a{i}"), cx);
        }
    });
    scroll_through(&sidebar, cx);
    let full = sidebar.read_with(cx, |s, _| sizes(s));

    // Each thread is deleted, which the host pushes as its row gone, then its workspace.
    sidebar.update(cx, |s, cx| {
        for (i, w) in (4..500).zip(&ids) {
            s.apply(Change::Row { id: format!("r{i}"), row: None }, cx);
            s.apply(Change::WorkspaceDeleted(w.clone()), cx);
        }
    });
    scroll_through(&sidebar, cx);
    let after = sidebar.read_with(cx, |s, _| sizes(s));

    let kept: Vec<String> = before
        .iter()
        .zip(&full)
        .zip(&after)
        .filter(|((b, _), a)| b.1 != a.1)
        .map(|((b, f), a)| format!("{} {} -> {} -> {}", b.0, b.1, f.1, a.1))
        .collect();
    assert!(kept.is_empty(), "not freed, 4 threads -> 500 -> 4: {kept:?}");
}

fn row_of(s: &Sidebar, key: &str) -> Option<usize> {
    s.drawn_rows.iter().position(|d| match d {
        Drawn::Line(ix) => matches!(&s.lines[*ix].item, Item::Tile(t) if s.threads[t.thread].key == key && !t.inbox),
        _ => false,
    })
}

#[gpui_kit::test]
fn the_key_ring_stays_on_its_thread_when_a_row_lands_above_it(cx: &mut TestAppContext) {
    let (sidebar, cx) = drawn(cx);
    let now = now_ms();
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now))), cx));
    cx.run_until_parked();
    sidebar.update_in(cx, |s, window, cx| s.point_at("t2", window, cx));
    let (ring, at) = sidebar.read_with(cx, |s, _| (s.key_row, row_of(s, "t2")));
    assert_eq!(ring, at);
    let newer = json!({
        "id": "r9", "threadId": "t9", "workspaceId": "w0", "harness": "claude", "status": "completed", "prompt": "thread 9",
        "project": { "id": "p1", "name": "repo" }, "startedAt": now - 1_000., "endedAt": now - 500., "readAt": now - 500.,
    });
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "r9".into(), row: Some(newer) }, cx));
    cx.run_until_parked();
    let (ring, moved) = sidebar.read_with(cx, |s, _| (s.key_row, row_of(s, "t2")));
    assert_ne!(moved, at, "the newer thread stands above the one the keys are on");
    assert_eq!(ring, moved, "the ring follows its thread to its new row");
}

fn processes(ends: &dialogs::Ends) -> dialogs::OpenDialog {
    dialogs::OpenDialog::Processes {
        ends: ends.clone(),
        title: "Delete b?".into(),
        line: None,
        left: None,
        busy: Some(Processes::Keep),
        said: None,
        checked: false,
    }
}

#[gpui_kit::test]
fn a_reply_for_another_dialog_leaves_the_open_one_as_it_is(cx: &mut TestAppContext) {
    use wsp_host_client::Failure;
    let (sidebar, cx) = drawn(cx);
    let (a, b) = (dialogs::Ends::Delete("a".into()), dialogs::Ends::Delete("b".into()));
    sidebar.update(cx, |s, cx| {
        s.show_dialog(processes(&b), true, cx);
        s.checked(&a, Err(Failure::of_kind("a still runs a server", "processes-left")), cx);
        s.finished(&a, Ok(()), cx);
        let Some(dialogs::OpenDialog::Processes { ends, left, checked, busy, .. }) = s.dialog.as_ref().map(|d| &d.open) else {
            panic!("the late Ok for a closed b's delete dialog");
        };
        assert_eq!((ends, left, checked, busy), (&b, &None, &false, &Some(Processes::Keep)), "a's check landed on b's dialog");
        s.finished(&b, Ok(()), cx);
        assert!(s.dialog.is_none(), "b's own Ok closes it");
    });

    let (wa, wb) = (vec!["wa".to_owned()], vec!["wb".to_owned()]);
    sidebar.update(cx, |s, cx| {
        let open = dialogs::OpenDialog::Workspaces {
            road: dialogs::Road::Forget,
            ids: wb.clone(),
            title: "Forget wb?".into(),
            line: "".into(),
            holding: Some(Vec::new()),
            busy: true,
            said: None,
        };
        s.show_dialog(open, true, cx);
        s.read_holding(&wa, vec!["wa holds 1 commit not pushed".into()], true, cx);
        s.ended(&wa, None, cx);
        assert_eq!(s.dialog_words().map(|w| w.len()), Some(2), "a's read or end landed on b's dialog: {:?}", s.dialog_words());
        s.ended(&wb, Some("the host said no".into()), cx);
        assert_eq!(s.dialog_words().and_then(|w| w.last().cloned()).as_deref(), Some("the host said no"));
    });
}

#[gpui_kit::test]
fn the_delete_names_the_worktree_only_for_the_last_thread_in_one_wsp_made(cx: &mut TestAppContext) {
    let (sidebar, cx) = drawn(cx);
    let now = now_ms();
    let mut snap = snapshot(2, now);
    snap.workspaces =
        vec![json!({ "id": "w0", "name": "w0", "kind": "local", "project": { "id": "p1" }, "worktree": { "made": true, "path": "/wt" } })];
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snap)), cx));
    let line = |key: &str, cx: &mut VisualTestContext| {
        sidebar.update_in(cx, |s, window, cx| {
            dialogs::delete(s, key, window, cx);
            let words = s.dialog_words().unwrap_or_default();
            s.dialog = None;
            words.get(1).cloned().unwrap_or_default()
        })
    };
    assert!(line("t0", cx).contains("the folder it worked in is left as it is"), "another thread is still in the worktree");
    sidebar.update(cx, |s, cx| s.apply(Change::Row { id: "r1".into(), row: None }, cx));
    assert!(line("t0", cx).contains("the worktree at /wt with it"), "the last thread in it takes the worktree");
    for tree in [json!({ "made": false, "path": "/wt" }), json!({ "made": true, "gone": true, "path": "/wt" })] {
        let w = json!({ "id": "w0", "name": "w0", "kind": "local", "project": { "id": "p1" }, "worktree": tree });
        sidebar.update(cx, |s, cx| s.apply(Change::Workspaces(vec![w]), cx));
        assert!(line("t0", cx).contains("the folder it worked in is left as it is"), "a worktree wsp did not make, or gone, stays");
    }
}

#[gpui_kit::test]
fn the_switcher_head_opens_and_closes_its_popup_and_a_press_outside_closes_it(cx: &mut TestAppContext) {
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now_ms()))), cx));
    cx.run_until_parked();
    let head = cx.debug_bounds("projects").expect("the projects switcher is drawn").center();
    let open = |cx: &mut VisualTestContext| sidebar.read_with(cx, |s, _| s.switcher.is_some());
    let click = |at, cx: &mut VisualTestContext| {
        cx.simulate_click(at, gpui_kit::Modifiers::none());
        cx.run_until_parked();
    };
    click(head, cx);
    assert!(open(cx), "the head opens its popup");
    click(head, cx);
    assert!(!open(cx), "a second press on the head closes it");
    click(head, cx);
    click(gpui_kit::point(gpui_kit::px(900.), gpui_kit::px(700.)), cx);
    assert!(!open(cx), "a press outside closes it");
    click(head, cx);
    assert!(open(cx), "and the head opens it again at once");
}

#[gpui_kit::test]
fn a_right_click_on_a_tile_opens_its_menu_and_a_pick_runs_the_item(cx: &mut TestAppContext) {
    use gpui_kit::{Modifiers, MouseButton};
    let (sidebar, cx) = drawn(cx);
    sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot(4, now_ms()))), cx));
    cx.run_until_parked();
    let tile = cx.debug_bounds("tile-t1").expect("the tile is drawn").center();
    cx.simulate_mouse_down(tile, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(tile, MouseButton::Right, Modifiers::none());
    cx.run_until_parked();
    let (asked, items) = sidebar.read_with(cx, |s, _| {
        let tile = s.lines.iter().find_map(|l| match &l.item {
            Item::Tile(t) if s.threads[t.thread].key == "t1" => Some(t.clone()),
            _ => None,
        });
        let items = menu::items_for(s, &tile.unwrap(), "t1");
        let labels: Vec<_> = items.iter().map(|i| i.label.clone()).collect();
        (s.menu.asked().to_vec(), labels)
    });
    assert_eq!(asked, items, "a right-click on the tile asks for its menu");
    // On the Mac AppKit draws the menu, which the test platform cannot drive.
    if cfg!(target_os = "macos") {
        return;
    }
    assert!(sidebar.read_with(cx, |s, _| s.menu.is_open()), "the tile's menu is open");
    let downs = sidebar.read_with(cx, |s, _| {
        let tile = s.lines.iter().find_map(|l| match &l.item {
            Item::Tile(t) if s.threads[t.thread].key == "t1" => Some(t.clone()),
            _ => None,
        });
        let items = menu::items_for(s, &tile.unwrap(), "t1");
        let rename = items.iter().position(|i| i.act == menu::Act::Rename).expect("a Rename item");
        // The menu opens on its first row that runs, and each Down walks to the next row, held or not.
        rename - items.iter().position(|i| i.refusal.is_none()).expect("a row that runs")
    });
    cx.simulate_keystrokes(&vec!["down"; downs].join(" "));
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert_eq!(sidebar.read_with(cx, |s, cx| s.rename_box(cx)).as_deref(), Some("thread 1"), "Rename opened the box on the thread");
    assert!(sidebar.read_with(cx, |s, _| !s.menu.is_open()));
}
