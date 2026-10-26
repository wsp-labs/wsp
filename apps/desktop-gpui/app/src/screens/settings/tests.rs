// SPDX-License-Identifier: AGPL-3.0-only
//! Settings drawn off records with no host behind it: the page kept between openings, the sidebar's keys and motion,
//! a found row revealed, the trail's smooth scroll, Escape's order, and the fields Enter sends. Then Settings against a
//! door that answers what each test says: a preference write's answer and the records pushed around it, the reads it
//! refuses, release.check, and the ssh switch's answer.

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::component::Root;
use gpui_kit::{px, size, AppContext as _, Entity, Keystroke, Modifiers, Pixels, TestAppContext, VisualTestContext};
use serde_json::{json, Value};

use super::door::{self, Door};
use super::model::{At, Data, GroupId};
use super::{kept, route, Opening, Said, SettingsScreen};
use crate::local_store::{self, LocalStore};
use crate::screens::app_shell::{Opened, Screen as _};

fn bare() -> Data {
    let preferences = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
        "projectLook": {}, "projectOrder": [], "labs": false });
    Data { preferences: serde_json::from_value(preferences).ok(), ..Data::default() }
}

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::ui::init(crate::ui::Side::Light, cx);
        // gpui-kit's init takes the computer's own Reduce motion; a test that wants it still says so itself.
        cx.set_reduce_motion(false);
    });
}

/// Settings standing on `at` in a window `width` by `height`, its sidebar's lists opened.
fn open(data: Data, at: At, (width, height): (f32, f32), cx: &mut TestAppContext) -> (Entity<SettingsScreen>, &mut VisualTestContext) {
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(data, at, runtime, window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(width), px(height)));
    settle(cx);
    (screen.unwrap(), cx)
}

/// Moves the clock on by `ms` and draws the frame that time asks for, as a display would.
fn tick(cx: &mut VisualTestContext, ms: u64) {
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(ms));
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

/// Runs the window a second and more on, past every opening, glide and flash.
fn settle(cx: &mut VisualTestContext) {
    for _ in 0..6 {
        tick(cx, 200);
    }
}

fn click(cx: &mut VisualTestContext, selector: &'static str) {
    let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).center();
    cx.simulate_click(at, Modifiers::none());
    cx.run_until_parked();
}

/// A key pressed and let go, as a keyboard's press on a focused row needs both.
fn press(cx: &mut VisualTestContext, key: &str) {
    cx.simulate_keystrokes(key);
    cx.simulate_event(gpui_kit::KeyUpEvent { keystroke: Keystroke::parse(key).unwrap() });
    cx.run_until_parked();
}

fn at(screen: &Entity<SettingsScreen>, cx: &mut VisualTestContext) -> At {
    screen.read_with(cx, |s, _| s.at().clone())
}

fn offset(screen: &Entity<SettingsScreen>, cx: &mut VisualTestContext) -> Pixels {
    screen.read_with(cx, |s, _| s.page_scroll.offset().y)
}

#[gpui_kit::test]
fn the_page_is_kept_for_the_next_opening_and_for_a_new_window(cx: &mut TestAppContext) {
    init(cx);
    let path = local_store::scratch("settings-at");
    cx.update(|cx| local_store::init(path.clone(), cx));
    let (screen, vcx) = open(bare(), At::Group(GroupId::Appearance), (1280., 800.), cx);
    let first = vcx.update(|window, _| window.window_handle());
    vcx.update(|window, cx| screen.update(cx, |s, cx| s.go(At::Group(GroupId::Keybindings), window, cx)));
    vcx.run_until_parked();
    assert_eq!(vcx.update(|_, cx| kept::last(first.window_id(), cx)), At::Group(GroupId::Keybindings), "the next opening here");
    assert_eq!(LocalStore::open(&path).get(kept::AT_KEY), Some(&json!("keybindings")), "kept in the app's store, not on the host");

    let second = cx.add_empty_window().update(|window, _| window.window_handle().window_id());
    assert_eq!(cx.update(|cx| kept::last(second, cx)), At::Group(GroupId::Keybindings), "a new window opens where the last move went");

    let mut vcx = VisualTestContext::from_window(first, cx);
    vcx.update(|window, cx| screen.update(cx, |s, cx| s.go(At::Group(GroupId::Privacy), window, cx)));
    assert_eq!(cx.update(|cx| kept::last(second, cx)), At::Group(GroupId::Keybindings), "a window reads the store once");
    let third = cx.add_empty_window().update(|window, _| window.window_handle().window_id());
    assert_eq!(cx.update(|cx| kept::last(third, cx)), At::Group(GroupId::Privacy));
}

#[gpui_kit::test]
fn a_route_naming_no_page_says_so_and_leaves_the_page(cx: &mut TestAppContext) {
    assert_eq!(route(None), Ok((None, 0.)));
    assert_eq!(route(Some("general@588")), Ok((Some(At::Group(GroupId::General)), 588.)));
    assert_eq!(route(Some("computer:here")), Ok((Some(At::Computer("here".into())), 0.)));
    assert_eq!(route(Some("nowhere")), Err(super::NoSuchPage));
    init(cx);
    let (screen, cx) = open(bare(), At::Group(GroupId::Privacy), (1280., 800.), cx);
    let said = cx.update(|window, cx| {
        screen.update(cx, |s, cx| {
            s.show(Opened { arg: Some("nowhere".into()), ..Opened::default() }, window, cx);
            s.notice.as_ref().map(|(words, _)| words.to_string())
        })
    });
    assert_eq!(said.as_deref(), Some(super::NO_SUCH_PAGE));
    assert_eq!(at(&screen, cx), At::Group(GroupId::Privacy));
}

#[gpui_kit::test]
fn a_refused_release_check_says_the_newest_release_was_not_read(cx: &mut TestAppContext) {
    init(cx);
    let (screen, cx) = open(bare(), At::Group(GroupId::General), (1280., 800.), cx);
    let said = cx.update(|_, cx| {
        screen.update(cx, |s, cx| {
            s.release_checked(Err("the network is down".into()), cx);
            s.notice.as_ref().map(|(words, _)| words.to_string())
        })
    });
    assert_eq!(said.as_deref(), Some("Newest release not read: the network is down"));
}

#[test]
fn the_open_group_reads_in_the_foreground_only_under_a_page_of_its_own() {
    use super::frame::group_inks;
    assert_eq!(group_inks(true, true, false), (false, false), "its own page, under a search: muted, its glyph in the icon ink");
    assert_eq!(group_inks(true, false, false), (true, false), "a page under it: the foreground, its glyph still the icon ink");
    assert_eq!(group_inks(true, true, true), (true, true), "lifted");
    assert_eq!(group_inks(false, false, false), (false, false));
}

#[gpui_kit::test]
fn escape_clears_the_search_only_while_the_field_has_the_focus(cx: &mut TestAppContext) {
    init(cx);
    let (screen, cx) = open(bare(), At::Group(GroupId::General), (1280., 800.), cx);
    let closed = std::rc::Rc::new(std::cell::Cell::new(false));
    let heard = closed.clone();
    cx.update(|_, cx| cx.subscribe(&screen, move |_, _: &super::Closed, _| heard.set(true)).detach());
    click(cx, "settings-search");
    cx.simulate_input("theme");
    cx.run_until_parked();
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(screen.read_with(cx, |s, _| s.search().is_empty()) && !closed.get(), "in the field, Escape clears it");

    click(cx, "settings-search");
    cx.simulate_input("theme");
    cx.update(|window, cx| screen.update(cx, |s, cx| s.focus.focus(window, cx)));
    cx.simulate_keystrokes("escape");
    cx.run_until_parked();
    assert!(closed.get(), "anywhere else, Escape closes Settings");
}

#[gpui_kit::test]
fn the_arrows_home_and_end_walk_the_sidebars_rows(cx: &mut TestAppContext) {
    init(cx);
    let (screen, cx) = open(bare(), At::Group(GroupId::Privacy), (1280., 800.), cx);
    click(cx, "settings-search");
    for key in ["down", "down", "up", "up", "down"] {
        press(cx, key);
    }
    press(cx, "enter");
    assert_eq!(at(&screen, cx), At::Group(GroupId::Appearance), "Down from the field enters at the first row, Up stops there");
    settle(cx);
    press(cx, "end");
    press(cx, "enter");
    assert_eq!(at(&screen, cx), At::Group(GroupId::Account), "End reaches the last row");
    settle(cx);
    press(cx, "home");
    press(cx, "enter");
    assert_eq!(at(&screen, cx), At::Group(GroupId::General), "Home the first");
}

#[test]
fn a_walk_never_goes_past_an_end() {
    use super::frame::walk;
    assert_eq!(walk("down", None, 3), Some(0));
    assert_eq!(walk("down", Some(2), 3), Some(2));
    assert_eq!(walk("up", None, 3), None);
    assert_eq!(walk("up", Some(0), 3), Some(0));
    assert_eq!(walk("end", Some(0), 3), Some(2));
    assert_eq!(walk("home", None, 3), None);
    assert_eq!(walk("down", None, 0), None);
}

/// A group's list on its own page opening as one block with the web's motion, its height grown from nothing to its rows'
/// at 36 px each, however many sections the page has on this platform (Appearance's Glass card is the Mac's alone).
fn a_groups_list_opens_from_nothing_to_its_rows(group: GroupId, cx: &mut TestAppContext) -> &mut VisualTestContext {
    init(cx);
    let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(bare(), At::Group(group), runtime, window, cx));
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    let trail: &'static str = Box::leak(format!("settings-trail-{}", group.id()).into_boxed_str());
    let height = |cx: &mut VisualTestContext| cx.debug_bounds(trail).expect("the trail is drawn").size.height;
    assert!(height(cx) < px(1.), "it opens from nothing: {:?}", height(cx));
    tick(cx, 40);
    let midway = height(cx);
    settle(cx);
    let rows: Vec<Pixels> = (0..)
        .map_while(|ix| cx.debug_bounds(Box::leak(format!("settings-section-{ix}").into_boxed_str())).map(|b| b.size.height))
        .collect();
    println!("{trail}: {} rows of {:?}, {:?} in all, {midway:?} at 40 ms", rows.len(), rows, height(cx));
    assert!(!rows.is_empty() && rows.iter().all(|h| *h == px(36.)), "every trail row is 36 px, as today's: {rows:?}");
    let full = px(36.) * rows.len() as f32;
    assert!(midway > px(1.) && midway < full, "on its way at 40 ms of 140: {midway:?} of {full:?}");
    assert_eq!(height(cx), full, "{} trail rows of 36 px", rows.len());
    cx
}

#[gpui_kit::test]
fn a_groups_list_opens_with_the_webs_motion_and_at_once_under_reduced_motion(cx: &mut TestAppContext) {
    let cx = a_groups_list_opens_from_nothing_to_its_rows(GroupId::Appearance, cx);
    cx.update(|_, cx| cx.set_reduce_motion(true));
    click(cx, "settings-keybindings");
    assert!(height_of(cx, "settings-trail-keybindings") > px(1.), "under reduced motion it stands open on its first frame");
}

#[gpui_kit::test]
fn a_list_of_another_length_opens_to_its_own_rows(cx: &mut TestAppContext) {
    a_groups_list_opens_from_nothing_to_its_rows(GroupId::General, cx);
}

fn height_of(cx: &mut VisualTestContext, selector: &'static str) -> Pixels {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} is drawn")).size.height
}

#[gpui_kit::test]
fn a_trail_row_scrolls_smoothly_to_its_card_and_jumps_under_reduced_motion(cx: &mut TestAppContext) {
    init(cx);
    let (screen, cx) = open(bare(), At::Group(GroupId::Appearance), (1280., 500.), cx);
    click(cx, "settings-section-2");
    let start = offset(&screen, cx);
    assert!(start > px(-1.), "the first frame stands where the page was: {start:?}");
    tick(cx, 60);
    let midway = offset(&screen, cx);
    settle(cx);
    let end = offset(&screen, cx);
    assert!(end < px(-100.), "the page went down to the Type card: {end:?}");
    assert!(midway < start && midway > end, "eased, not jumped: {start:?} then {midway:?} then {end:?}");

    cx.update(|_, cx| cx.set_reduce_motion(true));
    click(cx, "settings-section-0");
    let jumped = offset(&screen, cx);
    assert!(jumped > end, "under reduced motion it jumps, on the first frame: {jumped:?}");
    settle(cx);
    assert_eq!(offset(&screen, cx), jumped, "and stays there");
}

#[gpui_kit::test]
fn a_narrow_windows_search_result_opens_its_group_scrolled_to_the_row_and_flashes_it(cx: &mut TestAppContext) {
    init(cx);
    let (screen, cx) = open(bare(), At::Group(GroupId::Privacy), (700., 500.), cx);
    click(cx, "settings-search");
    cx.simulate_input("settle a thread");
    settle(cx);
    assert_eq!(at(&screen, cx), At::Group(GroupId::Privacy), "the centre keeps its page");
    click(cx, "settings-result-general-settle-after");
    for _ in 0..4 {
        tick(cx, 16);
    }
    assert_eq!(at(&screen, cx), At::Group(GroupId::General));
    assert!(screen.read_with(cx, |s, _| s.search().is_empty()), "the move clears the search");
    let row = cx.debug_bounds("settings-revealed").expect("the row the result named");
    let page = cx.debug_bounds("settings-page").expect("the page");
    assert!((row.center().y - page.center().y).abs() < px(2.), "scrolled to the middle: {row:?} in {page:?}");
    assert!(cx.debug_bounds("settings-flash").is_some(), "and it flashes");
    settle(cx);
    assert!(cx.debug_bounds("settings-flash").is_none(), "once");

    // A row near the foot of a page whose layout moves on its first frames: as far down as the page goes.
    click(cx, "settings-search");
    cx.simulate_input("code size");
    settle(cx);
    click(cx, "settings-result-appearance-code-size");
    for _ in 0..6 {
        tick(cx, 16);
    }
    let (y, room) = screen.read_with(cx, |s, _| (s.page_scroll.offset().y, s.page_scroll.max_offset().y));
    assert!(room > px(0.) && (y + room).abs() < px(1.), "at the page's end: {y:?} of {room:?}");
}

#[gpui_kit::test]
fn a_project_pages_last_crumb_wears_the_projects_mark(cx: &mut TestAppContext) {
    init(cx);
    let project = json!({ "id": "p1", "name": "alpha", "computer": "here", "source": { "kind": "folder", "path": "/a" }, "path": "/a",
        "remote": "", "defaultBranch": "main", "memoryKey": "k", "memoryDir": "/m", "createdAt": "2026-10-17T00:00:00Z" });
    let data = Data { projects: vec![serde_json::from_value(project).unwrap()], projects_read: true, ..bare() };
    let (_, cx) = open(data, At::Project("p1".into()), (1280., 800.), cx);
    let glyph = cx.debug_bounds("settings-crumb-glyph").expect("the project's mark in the crumb");
    let page = cx.debug_bounds("settings-crumb-page").expect("the last crumb");
    assert!(glyph.left() == page.left() && glyph.size.width == px(16.), "{glyph:?} leads {page:?}");
}

/// `max-w-[70%]` on the page crumb: a long project name is cut at seven tenths of the crumbs, as the web cuts it.
#[gpui_kit::test]
fn a_long_project_name_is_cut_at_seven_tenths_of_the_crumbs(cx: &mut TestAppContext) {
    init(cx);
    let name = "the nightly release pipeline that builds every package for each platform we ship";
    let project = json!({ "id": "p1", "name": name, "computer": "here", "source": { "kind": "folder", "path": "/a" }, "path": "/a",
        "remote": "", "defaultBranch": "main", "memoryKey": "k", "memoryDir": "/m", "createdAt": "2026-10-17T00:00:00Z" });
    let data = Data { projects: vec![serde_json::from_value(project).unwrap()], projects_read: true, ..bare() };
    let (_, cx) = open(data, At::Project("p1".into()), (1280., 800.), cx);
    let crumbs = cx.debug_bounds("settings-crumbs").expect("the crumbs");
    let page = cx.debug_bounds("settings-crumb-page").expect("the last crumb");
    assert!((page.size.width - crumbs.size.width * 0.7).abs() < px(1.), "{page:?} is 70% of {crumbs:?}");
}

#[gpui_kit::test]
fn restore_defaults_says_itself_on_its_hover(cx: &mut TestAppContext) {
    init(cx);
    let mut data = bare();
    if let Some(p) = data.preferences.as_mut() {
        *p = serde_json::from_value(json!({ "theme": "dark", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {},
            "access": {}, "projectLook": {}, "projectOrder": [], "labs": false }))
        .unwrap();
    }
    let (_, cx) = open(data, At::Group(GroupId::Appearance), (1280., 800.), cx);
    let button = cx.debug_bounds("restore-defaults").expect("Restore defaults, the theme off its default");
    cx.simulate_mouse_move(button.center(), None, Modifiers::none());
    tick(cx, 600);
    tick(cx, 600);
    assert!(cx.debug_bounds("tip:Restore defaults").is_some(), "its tooltip");
}

/// A computer whose image is being built here, its sign-in for Claude Code waiting on a code the page hands back.
fn building_here() -> Data {
    let here = json!({ "id": "here", "kind": "computer", "name": "mac", "default": true, "buildsImages": true });
    let job = json!({ "id": "j1", "road": "manual", "phase": "signing-in", "keys": {}, "step": 0, "stoppable": true, "screens": [],
        "rows": [{ "id": "claude", "kind": "sign-in", "label": "Claude Code", "state": "waiting for you", "finish": "code", "tool": "claude" }],
        "progress": { "done": 0, "total": 3 }, "log": [], "place": { "id": "here", "name": "mac" } });
    let setup = json!({ "keys": { "box": true }, "home": "/h", "agents": [], "pricing": null, "job": job });
    Data {
        places: vec![serde_json::from_value(here).unwrap()],
        places_read: true,
        setup: Some(serde_json::from_value(setup).unwrap()),
        image: Some(serde_json::from_value(json!({ "image": null, "copies": [], "projects": [] })).unwrap()),
        ..bare()
    }
}

#[gpui_kit::test]
fn enter_in_the_builds_code_field_sends_the_code_and_clears_the_field(cx: &mut TestAppContext) {
    init(cx);
    let (_, cx) = open(building_here(), At::Computer("here".into()), (1280., 900.), cx);
    click(cx, "build-code-field");
    cx.simulate_input("4F2-K9");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string("before".into()));
    cx.simulate_keystrokes("secondary-a secondary-c");
    cx.run_until_parked();
    let held = cx.read_from_clipboard().and_then(|c| c.text()).unwrap_or_default();
    assert_ne!(held, "4F2-K9", "Enter sent the code and nothing holds it after");
}

/// A box added over ssh, on its own page.
fn a_box() -> Data {
    let place = json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "road": { "ssh": "me@box1" } });
    Data { places: vec![serde_json::from_value(place).unwrap()], places_read: true, ..bare() }
}

/// A box mid-setup, and with `long` an MCP server not copied for want of many keys, so its row's note runs long.
fn a_box_setting_up(long: bool) -> Data {
    let keys = ["GSC_CREDENTIALS_PATH", "GOOGLE_APPLICATION_CREDENTIALS", "SEARCH_CONSOLE_PROPERTY", "ANALYTICS_ACCOUNT_ID"];
    let applied: Vec<Value> = long
        .then(|| json!({ "id": "agents/mcp/gsc", "label": "Claude Code gsc", "outcome": "skipped", "step": "mcp", "keys": keys }))
        .into_iter()
        .collect();
    let place = json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "road": { "ssh": "me@box1" },
        "setup": { "state": "running", "addId": "a", "startedAt": "x", "waiting": [], "steps": [
            { "step": "floor", "state": "done" }, { "step": "agents", "state": "done" }, { "step": "mcp", "state": "done" },
        ] },
        "applied": { "hash": "h", "at": "x", "rows": applied } });
    Data { places: vec![serde_json::from_value(place).unwrap()], places_read: true, ..bare() }
}

/// Catches: the Setup list sized to its rows, past the page's column to the window's edge where a row runs long, with
/// the long note on one line, and short of the column where every row is short, where today's list fills the column
/// and wraps the note; at a narrow window and a wide one.
#[gpui_kit::test]
fn the_setup_list_fills_the_columns_width_and_wraps_a_long_row(cx: &mut TestAppContext) {
    init(cx);
    let mut wrong = Vec::new();
    for long in [true, false] {
        let (_, cx) = open(a_box_setting_up(long), At::Computer("box1".into()), (900., 900.), cx);
        for width in [900., 1600.] {
            cx.simulate_resize(size(px(width), px(900.)));
            settle(cx);
            let page = cx.debug_bounds("settings-page").expect("the page");
            let column = (page.size.width - px(64.)).min(px(696.));
            let (left, right) = (page.center().x - column / 2., page.center().x + column / 2.);
            let ids: &[&str] = if long { &["floor", "agents", "mcp", "agents/mcp/gsc"] } else { &["floor", "agents", "mcp"] };
            for id in ids {
                let row = cx.debug_bounds(Box::leak(format!("step-row:{id}").into_boxed_str())).expect("the row");
                let (l, r) = (row.left(), row.right());
                if (l - left).abs() > px(1.5) || (r - right).abs() > px(1.5) {
                    wrong.push(format!(
                        "at {width} px, long rows {long}, the {id} row spans {l:?} to {r:?}, the column {left:?} to {right:?}"
                    ));
                }
            }
            if long {
                let note = cx.debug_bounds("step-note:agents/mcp/gsc").expect("the long note");
                if note.size.height <= px(24.) {
                    wrong.push(format!("at {width} px the long note stands on one line: {note:?}"));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Whether a Settings dialog stands on the window's dialogs layer.
fn dialog_open(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| crate::screens::dialogs::layer::is_open(super::DIALOG, cx))
}

#[gpui_kit::test]
fn enter_in_the_name_sheet_saves_and_an_empty_field_holds_it(cx: &mut TestAppContext) {
    init(cx);
    let (_, cx) = open(a_box(), At::Computer("box1".into()), (1280., 900.), cx);
    click(cx, "computer-name-change");
    assert!(dialog_open(cx), "the Name sheet");
    cx.simulate_keystrokes("secondary-a backspace enter");
    cx.run_until_parked();
    assert!(dialog_open(cx), "Save is held while the field is empty, and so is Enter");
    cx.simulate_input("box1");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!dialog_open(cx), "Enter saves, here a name left as it was, which closes the sheet");

    click(cx, "computer-ssh-change");
    cx.simulate_keystrokes("enter");
    cx.run_until_parked();
    assert!(!dialog_open(cx), "the SSH sheet saves on Enter too");
}

#[gpui_kit::test]
fn the_builds_submit_is_held_while_its_code_field_is_empty(cx: &mut TestAppContext) {
    init(cx);
    let (_, cx) = open(building_here(), At::Computer("here".into()), (1280., 900.), cx);
    assert!(cx.debug_bounds("build-code-submit-held").is_some(), "Submit is held over an empty field");
    click(cx, "build-code-field");
    cx.simulate_input("4F2-K9");
    cx.run_until_parked();
    assert!(cx.debug_bounds("build-code-submit").is_some(), "and live once the field holds a code");
    assert!(cx.debug_bounds("build-code-submit-held").is_none());
}

#[test]
fn a_refusal_splits_off_the_fix_its_words_end_with() {
    let mut refused = wsp_host_client::Failure::new("Projects are not listed yet. Run wsp up first.");
    refused.fix = Some("Run wsp up first.".into());
    assert_eq!(super::said_and_fix(refused), ("Projects are not listed yet.".to_owned(), Some("Run wsp up first.".to_owned())));
    let plain = wsp_host_client::Failure::new("Projects are not listed yet.");
    assert_eq!(super::said_and_fix(plain), ("Projects are not listed yet.".to_owned(), None));
}

#[gpui_kit::test]
fn the_adds_whose_dialogs_come_later_are_held_and_say_why_on_their_hover(cx: &mut TestAppContext) {
    init(cx);
    let data = Data { projects_read: true, places_read: true, recipes: Some(Vec::new()), ..bare() };
    let (screen, cx) = open(data, At::Group(GroupId::Projects), (1280., 800.), cx);
    let pages = [
        (At::Group(GroupId::Projects), "add-project-button-held", "Add a project opens in the next build of this app."),
        (At::Group(GroupId::Computers), "add-computer-button-held", "Add a computer opens in the next build of this app."),
        (At::Group(GroupId::Recipes), "add-computer-button-held", "Add a computer opens in the next build of this app."),
    ];
    for (page, held, why) in pages {
        screen.update_in(cx, |s, window, cx| s.go(page.clone(), window, cx));
        settle(cx);
        let button = cx.debug_bounds(held).unwrap_or_else(|| panic!("{held} on {page:?}"));
        cx.simulate_mouse_move(button.center(), None, Modifiers::none());
        tick(cx, 600);
        tick(cx, 600);
        let tip: &'static str = Box::leak(format!("tip:{why}").into_boxed_str());
        assert!(cx.debug_bounds(tip).is_some(), "{held} on {page:?} says why on its hover");
        cx.simulate_mouse_move(gpui_kit::point(px(5.), px(5.)), None, Modifiers::none());
        tick(cx, 600);
    }
}

#[gpui_kit::test]
fn a_pending_computers_row_says_its_state_on_its_whole_hover(cx: &mut TestAppContext) {
    init(cx);
    let pending = json!({ "id": "a1", "address": "me@box2", "name": "box2", "step": "connect", "choices": { "name": "box2" }, "startedAt": "2026-10-18T00:00:00Z" });
    let data = Data { pending: vec![serde_json::from_value(pending).unwrap()], places_read: true, ..bare() };
    let (_, cx) = open(data, At::Group(GroupId::Computers), (1280., 800.), cx);
    let row = cx.debug_bounds("grid-row").expect("the pending row");
    // On the name, well left of the state mark that carries the same words.
    cx.simulate_mouse_move(gpui_kit::point(row.left() + px(120.), row.center().y), None, Modifiers::none());
    tick(cx, 600);
    tick(cx, 600);
    assert!(cx.debug_bounds("tip:Pending: connecting over ssh").is_some(), "the row's hover says the pending state");
}

#[gpui_kit::test]
fn opencodes_model_row_and_models_list_draw_from_the_computers_own_list(cx: &mut TestAppContext) {
    use super::pages::agents::model::records::catalog;
    init(cx);
    let here = json!({ "id": "here", "kind": "computer", "name": "mac", "default": true });
    // wsp's own table, which lists no model for OpenCode.
    let table = catalog(json!({ "harness": "opencode", "label": "OpenCode" }));
    let data = Data { places: vec![serde_json::from_value(here).unwrap()], places_read: true, harnesses: vec![table], ..bare() };
    let (screen, cx) = open(data, At::Agent("opencode".into()), (1280., 2400.), cx);
    // The computer's select heads the page; the Model row's would be the last drawn under it.
    let controls = cx.debug_bounds("select").expect("the computer's select");
    assert!(cx.debug_bounds("model-grip-zeta/one").is_none(), "no Models list off wsp's own table");
    let own = catalog(json!({ "harness": "opencode", "label": "OpenCode", "models": [{ "value": "zeta/one", "label": "Zeta One" }] }));
    screen.update(cx, |s, cx| {
        s.agents.update(cx, |a, cx| {
            a.catalogs.insert("here".into(), vec![own]);
            cx.notify();
        })
    });
    settle(cx);
    let model = cx.debug_bounds("select").unwrap();
    assert!(model.top() > controls.bottom(), "the Model row, off the list the computer's own OpenCode answered: {model:?}");
    assert!(cx.debug_bounds("model-grip-zeta/one").is_some(), "and the Models list");
}

struct Blank;

impl gpui_kit::Render for Blank {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        gpui_kit::div()
    }
}

/// A preferences record as the door's host keeps it: the bare one with `given` laid over.
fn record(given: Value) -> Value {
    let mut whole = json!({ "theme": "system", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
        "projectLook": {}, "projectOrder": [], "labs": false });
    crate::prefs::merge(&mut whole, &given);
    whole
}

fn changed(preferences: &Value) -> Value {
    json!({ "type": "preferences.changed", "preferences": preferences })
}

/// Settings on `at` against `door`, made in a window that never draws it, so a notice stays on the screen where a
/// drawn one hands it to the window's toasts.
fn undrawn<'a>(door: &Door, at: At, cx: &'a mut TestAppContext) -> (Entity<SettingsScreen>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    init(cx);
    let opening = Opening { state: door.state(), env: door.env(), runtime: door::runtime(), at, scroll: 0. };
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        screen = Some(cx.new(|cx| SettingsScreen::open(opening, window, cx)));
        Root::new(cx.new(|_| Blank), window, cx)
    });
    (screen.unwrap(), cx)
}

/// Settings on `at` against `door`, drawn as the app draws it in a window `height` tall.
fn drawn<'a>(door: &Door, at: At, height: f32, cx: &'a mut TestAppContext) -> (Entity<SettingsScreen>, &'a mut VisualTestContext) {
    cx.executor().allow_parking();
    init(cx);
    let opening = Opening { state: door.state(), env: door.env(), runtime: door::runtime(), at, scroll: 0. };
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::open(opening, window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(height)));
    (screen.unwrap(), cx)
}

/// Runs the window in real time, the door answering on its own clock, until `done` holds.
fn waits(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        cx.run_until_parked();
        if done(cx) {
            return;
        }
        assert!(Instant::now() < deadline, "Settings never came to: {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Every read Settings makes as it opens answered, and their notices let go.
fn read_through(door: &Door, screen: &Entity<SettingsScreen>, cx: &mut VisualTestContext) {
    waits(cx, "every read asked", |_| door.asked().len() >= 14);
    std::thread::sleep(Duration::from_millis(200));
    cx.run_until_parked();
    screen.update(cx, |s, _| s.notice = None);
}

/// The notices the window's toasts stand, each with whether it says a failure.
fn toasts(cx: &mut VisualTestContext) -> Vec<(String, bool)> {
    cx.update(|window, cx| {
        let standing = crate::ui::toast::of(window, cx).map(|t| t.read(cx).standing()).unwrap_or_default();
        standing.iter().map(|n| (n.text.to_string(), matches!(n.kind, crate::ui::NoticeKind::Error))).collect()
    })
}

/// The notice standing: its words, and whether it says a failure.
fn notice(screen: &Entity<SettingsScreen>, cx: &mut VisualTestContext) -> Option<(String, bool)> {
    screen.read_with(cx, |s, _| s.notice.as_ref().map(|(words, said)| (words.to_string(), matches!(said, Said::Failed))))
}

fn shown(screen: &Entity<SettingsScreen>, cx: &mut VisualTestContext) -> Option<(String, String)> {
    screen.read_with(cx, |s, _| s.data.preferences.as_ref().map(|p| (p.theme.to_string(), p.light_theme.clone())))
}

/// Catches: a record from before the write that paints over the pick while it is out, and another client's change to
/// another preference that waits for this write to answer.
#[gpui_kit::test]
fn a_record_pushed_while_a_set_is_out_shows_under_the_pick_and_the_last_one_stands(cx: &mut TestAppContext) {
    // Another client's write lands at the host before this one's, and its record comes while this one is out; the
    // answer carries no record, as one that rides another socket can come first.
    let theirs = record(json!({ "lightTheme": "linen" }));
    let door = Door::start(json!({
        "preferences.get": { "reply": { "preferences": record(json!({})) } },
        "preferences.set": { "push": [changed(&theirs)], "delayMs": 600, "reply": {} },
    }));
    let (screen, cx) = undrawn(&door, At::Group(GroupId::Privacy), cx);
    read_through(&door, &screen, cx);
    screen.update(cx, |s, cx| s.set_preferences(json!({ "theme": "dark" }), cx));
    waits(cx, "their record landed", |cx| screen.read_with(cx, |s, _| s.landed.as_ref().is_some_and(|p| p.light_theme == "linen")));
    assert_eq!(screen.read_with(cx, |s, cx| s.prefs.read(cx).writes_out()), 1, "the set has not answered yet");
    assert_eq!(shown(&screen, cx), Some(("dark".into(), "linen".into())), "while the set is out its pick stands over the last record");
    waits(cx, "the set's answer", |cx| screen.read_with(cx, |s, cx| s.prefs.read(cx).writes_out() == 0));
    assert_eq!(shown(&screen, cx), Some(("dark".into(), "linen".into())), "once it answered, the pick the host kept over the last record");
    assert_eq!(toasts(cx), [], "a set that stood says nothing");
}

#[gpui_kit::test]
fn a_refused_set_shows_the_hosts_value_again_and_says_it_was_not_saved(cx: &mut TestAppContext) {
    let door = Door::start(json!({
        "preferences.get": { "reply": { "preferences": record(json!({})) } },
        "preferences.set": { "error": "The state file is read-only." },
    }));
    let (screen, cx) = undrawn(&door, At::Group(GroupId::Privacy), cx);
    read_through(&door, &screen, cx);
    screen.update(cx, |s, cx| s.set_preferences(json!({ "theme": "dark" }), cx));
    assert_eq!(shown(&screen, cx), Some(("dark".into(), "paper".into())), "the pick shows at once");
    waits(cx, "the refusal", |cx| screen.read_with(cx, |s, cx| s.prefs.read(cx).writes_out() == 0));
    assert_eq!(shown(&screen, cx), Some(("system".into(), "paper".into())), "the host's value again");
    assert_eq!(toasts(cx), [("That setting was not saved: The state file is read-only. It shows the host's value again.".into(), true)]);
}

#[gpui_kit::test]
fn a_set_that_answers_a_notice_says_it_as_a_failure(cx: &mut TestAppContext) {
    let mine = record(json!({ "theme": "dark" }));
    let door = Door::start(json!({
        "preferences.get": { "reply": { "preferences": record(json!({})) } },
        "preferences.set": { "push": [changed(&mine)], "reply": { "notice": "Kept, but the login item did not change." } },
    }));
    let (screen, cx) = undrawn(&door, At::Group(GroupId::Privacy), cx);
    read_through(&door, &screen, cx);
    screen.update(cx, |s, cx| s.set_preferences(json!({ "theme": "dark" }), cx));
    waits(cx, "the answer", |cx| screen.read_with(cx, |s, cx| s.prefs.read(cx).writes_out() == 0));
    assert_eq!(toasts(cx), [("Kept, but the login item did not change.".into(), true)]);
    assert_eq!(shown(&screen, cx), Some(("dark".into(), "paper".into())), "what the set kept stands");
}

#[gpui_kit::test]
fn a_refused_preferences_read_says_so(cx: &mut TestAppContext) {
    // Late, so it lands after the reads this door refuses for want of a rule.
    let door = Door::start(json!({ "preferences.get": { "error": "The state file did not read.", "delayMs": 400 } }));
    let (screen, cx) = undrawn(&door, At::Group(GroupId::Privacy), cx);
    let words = "Preferences not read: The state file did not read.".to_owned();
    waits(cx, "the refusal said", |cx| notice(&screen, cx) == Some((words.clone(), true)));
    assert!(screen.read_with(cx, |s, _| s.data.preferences.is_none()));
}

#[gpui_kit::test]
fn a_refused_recipes_read_draws_its_refusal_on_recipes(cx: &mut TestAppContext) {
    let door = Door::start(json!({
        "preferences.get": { "reply": { "preferences": record(json!({})) } },
        "recipes.list": { "error": "The recipes folder did not read." },
    }));
    let (screen, cx) = drawn(&door, At::Group(GroupId::Recipes), 900., cx);
    waits(cx, "the refused read", |cx| screen.read_with(cx, |s, _| s.data.recipes_refused.is_some()));
    waits(cx, "its refusal drawn", |cx| cx.debug_bounds("refusal").is_some());
}

#[gpui_kit::test]
fn settings_opened_straight_on_general_asks_for_the_newest_release(cx: &mut TestAppContext) {
    let door = Door::start(json!({ "release.check": { "reply": { "release": { "state": "off" } } } }));
    let (_screen, cx) = undrawn(&door, At::Group(GroupId::General), cx);
    waits(cx, "release.check asked", |_| door.asked_of("release.check").len() == 1);
}

#[gpui_kit::test]
fn the_ssh_switchs_answer_stands_after_a_page_change(cx: &mut TestAppContext) {
    let door = Door::start(json!({
        "preferences.get": { "reply": { "preferences": record(json!({})) } },
        "ssh.include": { "reply": { "sshInclude": false } },
    }));
    let (screen, cx) = drawn(&door, At::Group(GroupId::General), 3000., cx);
    let ssh = |cx: &mut VisualTestContext| screen.read_with(cx, |s, _| s.data.ssh_include);
    waits(cx, "the ssh config's word", |cx| ssh(cx) == Some(false));
    settle(cx);
    door.rules(json!({ "ssh.include": { "reply": { "sshInclude": true } } }));
    // With no computer named, the startup card holds no switch, so the open-in card's is the page's last.
    click(cx, "switch");
    waits(cx, "the switch's answer", |cx| ssh(cx) == Some(true));
    assert_eq!(door.asked_of("ssh.include").last().map(|f| f["on"].clone()), Some(json!(true)), "the press was the ssh switch's");
    for page in [GroupId::Privacy, GroupId::General] {
        screen.update_in(cx, |s, window, cx| s.go(At::Group(page), window, cx));
        settle(cx);
    }
    assert_eq!(ssh(cx), Some(true), "back on General the switch reads what the host answered");
}
