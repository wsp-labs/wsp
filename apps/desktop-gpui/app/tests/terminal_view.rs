// SPDX-License-Identifier: AGPL-3.0-only
//! One shell's view in a test window, wrapped in gpui-kit's `Root` as the app wraps every window: output fed to its
//! session lands in cells in the theme's colours, the cursor sits where the engine says, a drag and a double click
//! select what they cover, and keys, paste, the wheel and the link gesture reach the session as today's pane sends
//! them. What the view sends is read off the session's outbox, which the daemon channel drains in the app.

use desktop_gpui::screens::app_shell::keys;
use desktop_gpui::screens::terminal::engine::{CursorShape, Rgb};
use desktop_gpui::screens::terminal::links::Kind;
use desktop_gpui::screens::terminal::look::{hsla_of, Look};
use desktop_gpui::screens::terminal::session::{Out, Session};
use desktop_gpui::screens::terminal::view::{TerminalView, ViewEvent};
use desktop_gpui::ui::{self, Side};
use gpui_kit::component::Root;
use gpui_kit::{
    point, px, size, AppContext, ClipboardItem, Entity, Focusable, Modifiers, MouseButton, Pixels, Point, ScrollDelta, ScrollWheelEvent,
    TestAppContext, TouchPhase, VisualTestContext,
};

fn open(cx: &mut TestAppContext) -> (&mut VisualTestContext, Entity<TerminalView>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.bind_keys(desktop_gpui::screens::terminal::view::key_bindings());
    });
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|_| Session::new("pty_1", "shell 1", 80, 24));
        let look = Look::new(None, None, cx);
        let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
        made = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    let view = made.unwrap();
    view.update_in(cx, |v, window, cx| v.focus_handle(cx).focus(window, cx));
    settle(cx);
    (cx, view)
}

fn settle(cx: &mut VisualTestContext) {
    for _ in 0..2 {
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
    }
    cx.run_until_parked();
}

fn feed(view: &Entity<TerminalView>, bytes: &[u8], cx: &mut VisualTestContext) {
    let session = view.read_with(cx, |v, _| v.session().clone());
    session.update(cx, |s, cx| s.feed(bytes, cx));
    settle(cx);
}

/// What the view wrote toward the pty, joined, after anything sent before.
fn sent(view: &Entity<TerminalView>, cx: &mut VisualTestContext) -> String {
    let session = view.read_with(cx, |v, _| v.session().clone());
    session.update(cx, |s, _| s.take_out()).into_iter().filter_map(|o| if let Out::Write(w) = o { Some(w) } else { None }).collect()
}

fn centre(view: &Entity<TerminalView>, x: u16, y: u16, cx: &mut VisualTestContext) -> Point<Pixels> {
    view.read_with(cx, |v, _| v.cell_bounds(x, y).unwrap().center())
}

#[gpui_kit::test]
fn output_lands_in_cells_in_the_themes_terminal_colours(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"\x1b[31mred\x1b[0m plain", cx);
    let (frame, look) = view.read_with(cx, |v, _| (v.frame().clone(), v.look().clone()));
    assert_eq!(frame.rows[0].plain().trim_end(), "red plain");
    assert_eq!(frame.rows[0].cells[0].fg, Rgb(0xc6, 0x33, 0x40), "Paper's --terminal-ansi-1");
    assert_eq!(frame.rows[0].cells[4].fg, look.colors.foreground);
    assert_eq!(frame.background, Rgb(0xfc, 0xfc, 0xfc), "Paper's --terminal-background");
    let g = view.read_with(cx, |v, _| v.geometry());
    assert!(g.cols > 60 && g.rows > 10, "the grid fills the window: {} x {}", g.cols, g.rows);
}

#[gpui_kit::test]
fn the_ground_follows_a_programs_osc_11_background(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"\x1b]11;rgb:12/34/56\x1b\\", cx);
    let ground = view.read_with(cx, |v, _| v.ground());
    assert_eq!(ground, hsla_of(Rgb(0x12, 0x34, 0x56)), "the program's background, not the theme's");
    let mut glass = view.read_with(cx, |v, _| v.look().clone());
    glass.opacity = 0.5;
    let theme = glass.colors.background;
    view.update(cx, |v, cx| v.set_look(glass, cx));
    settle(cx);
    assert_eq!(view.read_with(cx, |v, _| v.ground()), hsla_of(theme), "under glass the drawer's ground shows, in the theme's colour");
}

#[gpui_kit::test]
fn the_cursor_sits_in_its_cell_in_each_shape(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"ab\r\ncd", cx);
    for (seq, shape) in [(&b"\x1b[2 q"[..], CursorShape::Block), (b"\x1b[6 q", CursorShape::Bar), (b"\x1b[4 q", CursorShape::Underline)] {
        feed(&view, seq, cx);
        let cursor = view.read_with(cx, |v, _| v.frame().cursor.unwrap());
        assert_eq!((cursor.x, cursor.y, cursor.shape), (2, 1, shape));
    }
    let cell = view.read_with(cx, |v, _| v.cell_bounds(2, 1).unwrap());
    let ime = view
        .update_in(cx, |v, window, cx| gpui_kit::EntityInputHandler::bounds_for_range(v, 0..0, v.geometry().bounds, window, cx))
        .unwrap();
    assert_eq!(ime, cell, "the IME's candidate window opens at the cursor's cell");
}

#[gpui_kit::test]
fn a_composition_the_focus_leaves_holds_no_keys_back(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    cx.update(|window, _| window.activate_window());
    settle(cx);
    view.update_in(cx, |v, window, cx| gpui_kit::EntityInputHandler::replace_and_mark_text_in_range(v, None, "にほ", None, window, cx));
    cx.update(|window, cx| window.blur(cx));
    settle(cx);
    view.update_in(cx, |v, window, cx| v.focus_handle(cx).focus(window, cx));
    settle(cx);
    _ = sent(&view, cx);
    let enter = gpui_kit::Keystroke { modifiers: Modifiers::none(), key: "enter".into(), key_char: None };
    cx.simulate_event(gpui_kit::KeyDownEvent { keystroke: enter, is_held: false, prefer_character_input: false });
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(sent(&view, cx), "\r\x03", "Enter and Ctrl+C reach the shell once the focus is back");
}

#[gpui_kit::test]
fn a_drag_and_a_double_click_select_what_they_cover_and_copy_it(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"hello brave world", cx);
    let none = Modifiers::none();
    let (from, to) = (centre(&view, 6, 0, cx), centre(&view, 10, 0, cx));
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.simulate_mouse_move(to, MouseButton::Left, none);
    cx.simulate_mouse_up(to, MouseButton::Left, none);
    assert_eq!(view.read_with(cx, |v, cx| v.selection_text(cx)).as_deref(), Some("brave"));
    cx.simulate_keystrokes(if cfg!(target_os = "macos") { "cmd-c" } else { "ctrl-shift-c" });
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("brave"));
    assert!(view.read_with(cx, |v, cx| v.has_selection(cx)), "the copy chord keeps the selection");

    let word = centre(&view, 13, 0, cx);
    cx.simulate_click(point(word.x + px(40.), word.y), none);
    cx.simulate_click(word, none);
    cx.simulate_click(word, none);
    assert_eq!(view.read_with(cx, |v, cx| v.selection_text(cx)).as_deref(), Some("world"), "a double click takes the word");
    cx.simulate_click(word, none);
    assert_eq!(view.read_with(cx, |v, cx| v.selection_text(cx)).as_deref(), Some("hello brave world"), "a third takes the line");
    cx.simulate_click(point(word.x + px(80.), word.y), none);
    assert!(!view.read_with(cx, |v, cx| v.has_selection(cx)), "a click without a drag clears it");
}

#[cfg(not(target_os = "macos"))]
#[gpui_kit::test]
fn ctrl_c_on_a_selection_copies_and_clears_so_the_next_interrupts(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"hello brave world", cx);
    let none = Modifiers::none();
    let (from, to) = (centre(&view, 0, 0, cx), centre(&view, 4, 0, cx));
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.simulate_mouse_move(to, MouseButton::Left, none);
    cx.simulate_mouse_up(to, MouseButton::Left, none);
    _ = sent(&view, cx);
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("hello"));
    assert_eq!(sent(&view, cx), "", "the copy sends nothing");
    cx.simulate_keystrokes("ctrl-c");
    assert_eq!(sent(&view, cx), "\x03");
}

#[gpui_kit::test]
fn keys_reach_the_shell_as_the_encoder_writes_them(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    _ = sent(&view, cx);
    cx.simulate_input("ls");
    cx.simulate_keystrokes("enter up");
    assert_eq!(sent(&view, cx), "ls\r\x1b[A");
    feed(&view, b"\x1b[?1h", cx);
    cx.simulate_keystrokes("up ctrl-d alt-b");
    assert_eq!(sent(&view, cx), "\x1bOA\x04\x1bb", "cursor keys follow DECCKM");
}

#[gpui_kit::test]
fn tab_and_shift_tab_reach_the_shell_rather_than_moving_the_focus(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    _ = sent(&view, cx);
    cx.simulate_keystrokes("tab");
    let back_tab = gpui_kit::Keystroke { modifiers: Modifiers::shift(), key: "tab".into(), key_char: None };
    cx.simulate_event(gpui_kit::KeyDownEvent { keystroke: back_tab, is_held: false, prefer_character_input: false });
    assert_eq!(sent(&view, cx), "\t\x1b[Z", "the window root's focus keys stand aside for the shell");
    assert!(view.update_in(cx, |v, window, cx| v.focus_handle(cx).is_focused(window)));
}

#[gpui_kit::test]
fn a_paste_is_bracketed_when_the_program_asked(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    _ = sent(&view, cx);
    cx.write_to_clipboard(ClipboardItem::new_string("echo a\necho b".to_owned()));
    let paste = if cfg!(target_os = "macos") { "cmd-v" } else { "ctrl-shift-v" };
    cx.simulate_keystrokes(paste);
    assert_eq!(sent(&view, cx), "echo a\recho b");
    feed(&view, b"\x1b[?2004h", cx);
    cx.simulate_keystrokes(paste);
    assert_eq!(sent(&view, cx), "\x1b[200~echo a\necho b\x1b[201~");
}

fn wheel(view: &Entity<TerminalView>, lines: f32, cx: &mut VisualTestContext) {
    let at = centre(view, 3, 3, cx);
    cx.simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Lines(point(0., lines)),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    settle(cx);
}

#[gpui_kit::test]
fn the_wheel_scrolls_back_and_sends_arrows_on_the_alternate_screen(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let lines: String = (0..60).map(|i| format!("line {i}\r\n")).collect();
    feed(&view, lines.as_bytes(), cx);
    assert!(view.read_with(cx, |v, _| v.frame().scrollbar.at_bottom()));
    wheel(&view, 3., cx);
    let scroll = view.read_with(cx, |v, _| v.frame().scrollbar);
    assert!(!scroll.at_bottom(), "three lines up into the scrollback: {scroll:?}");
    _ = sent(&view, cx);
    feed(&view, b"\x1b[?1049h", cx);
    wheel(&view, -2., cx);
    assert_eq!(sent(&view, cx), "\x1b[B\x1b[B", "two rows down on the alternate screen are two Down arrows");
    feed(&view, b"\x1b[?1000h\x1b[?1006h", cx);
    wheel(&view, 1., cx);
    assert!(sent(&view, cx).starts_with("\x1b[<64;"), "a program that tracks the mouse gets the wheel as button 4");
}

#[gpui_kit::test]
fn a_button_the_program_holds_is_let_go_when_released_off_the_pane(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"\x1b[?1002h\x1b[?1006h", cx);
    _ = sent(&view, cx);
    let at = centre(&view, 4, 2, cx);
    let g = view.read_with(cx, |v, _| v.geometry());
    let below = point(at.x, g.bounds.bottom() + px(30.));
    for (button, code) in [(MouseButton::Right, 2), (MouseButton::Middle, 1)] {
        cx.simulate_mouse_down(at, button, Modifiers::none());
        cx.simulate_mouse_up(below, button, Modifiers::none());
        let want = format!("\x1b[<{code};5;3M\x1b[<{code};5;{}m", g.rows);
        assert_eq!(sent(&view, cx), want, "{button:?} pressed on the grid and let go below it, on the last row");
    }
}

/// The link gesture's key: Cmd on the Mac, Ctrl elsewhere.
fn gesture() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::command()
    } else {
        Modifiers::control()
    }
}

#[gpui_kit::test]
fn a_link_under_the_gesture_is_found_and_underlined(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"see https://wsp.dev/docs now", cx);
    let over = centre(&view, 8, 0, cx);
    cx.simulate_mouse_move(over, None, Modifiers::none());
    assert!(view.read_with(cx, |v, _| v.hovered_link().is_none()), "no link without the gesture");
    let gesture = gesture();
    cx.simulate_modifiers_change(gesture);
    cx.simulate_mouse_move(over, None, gesture);
    let link = view.read_with(cx, |v, _| v.hovered_link().cloned()).expect("the address under the pointer");
    assert_eq!((link.kind, link.text.as_str(), link.start.x, link.end.x), (Kind::Url, "https://wsp.dev/docs", 4, 23));
    cx.simulate_modifiers_change(Modifiers::none());
    assert!(view.read_with(cx, |v, _| v.hovered_link().is_none()), "letting go of the key drops it");
}

#[gpui_kit::test]
fn an_osc_8_link_reaches_the_system_opener_only_for_a_web_address(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let into = opened.clone();
    let _watch = cx.update(|_, cx| {
        cx.subscribe(&view, move |_, event: &ViewEvent, _| {
            if matches!(event, ViewEvent::OpenUrl(_) | ViewEvent::OpenPath(_)) {
                into.borrow_mut().push(event.clone());
            }
        })
    });
    let uris = ["file:///etc/passwd", "javascript:alert(1)", "vscode://file/x", "x-apple.systempreferences:x", "https://wsp.dev/x"];
    let lines: String = uris.iter().map(|uri| format!("\x1b]8;;{uri}\x1b\\link\x1b]8;;\x1b\\\r\n")).collect();
    feed(&view, lines.as_bytes(), cx);
    let gesture = gesture();
    for y in 0..uris.len() as u16 {
        let at = centre(&view, 1, y, cx);
        cx.simulate_mouse_down(at, MouseButton::Left, gesture);
        cx.simulate_mouse_up(at, MouseButton::Left, gesture);
    }
    let want: Vec<ViewEvent> = uris
        .iter()
        .map(|uri| if uri.starts_with("https:") { ViewEvent::OpenUrl(uri.to_string()) } else { ViewEvent::OpenPath(uri.to_string()) })
        .collect();
    assert_eq!(*opened.borrow(), want, "only http and https go to the browser; any other scheme is a path for Files, as today");
}

#[gpui_kit::test]
fn a_path_resolves_against_the_folder_the_shell_reported_and_else_goes_as_written(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let into = opened.clone();
    let _watch = cx.update(|_, cx| {
        cx.subscribe(&view, move |_, event: &ViewEvent, _| {
            if let ViewEvent::OpenPath(path) = event {
                into.borrow_mut().push(path.clone());
            }
        })
    });
    feed(&view, b"see ../lib/x.rs:3:9 now", cx);
    let at = centre(&view, 6, 0, cx);
    for folder in [None, Some("file://myhost/home/ana/w%20x/src")] {
        if let Some(folder) = folder {
            feed(&view, format!("\x1b]7;{folder}\x07").as_bytes(), cx);
        }
        cx.simulate_mouse_down(at, MouseButton::Left, gesture());
        cx.simulate_mouse_up(at, MouseButton::Left, gesture());
    }
    assert_eq!(*opened.borrow(), ["../lib/x.rs:3:9", "/home/ana/w x/lib/x.rs:3:9"]);
}

#[gpui_kit::test]
fn an_osc_8_link_soft_wrapped_over_two_rows_is_one_link_from_either(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let cols = view.read_with(cx, |v, _| v.geometry().cols);
    let lead = " ".repeat(cols as usize - 4);
    feed(&view, format!("{lead}\x1b]8;;https://wsp.dev/x\x1b\\linktext\x1b]8;;\x1b\\ after").as_bytes(), cx);
    cx.simulate_modifiers_change(gesture());
    for (x, y) in [(cols - 2, 0), (1, 1)] {
        let at = centre(&view, x, y, cx);
        cx.simulate_mouse_move(at, None, gesture());
        let link = view.read_with(cx, |v, _| v.hovered_link().cloned()).expect("the link under the pointer");
        let range = ((link.start.x, link.start.y), (link.end.x, link.end.y));
        assert_eq!(range, ((cols - 4, 0), (3, 1)), "hovered at {x},{y}, the range runs over the wrap as today's `linkAt` does");
    }
}

#[gpui_kit::test]
fn the_link_gesture_on_plain_text_selects_nothing(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"hello brave world", cx);
    let (from, to) = (centre(&view, 6, 0, cx), centre(&view, 10, 0, cx));
    cx.simulate_modifiers_change(gesture());
    cx.simulate_mouse_down(from, MouseButton::Left, gesture());
    cx.simulate_mouse_move(to, MouseButton::Left, gesture());
    cx.simulate_mouse_up(to, MouseButton::Left, gesture());
    assert_eq!(view.read_with(cx, |v, cx| v.selection_text(cx)), None, "a press with the link key never starts a selection, as today's");
}

#[gpui_kit::test]
fn the_link_gesture_past_the_last_column_finds_no_link(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let cols = view.read_with(cx, |v, _| v.geometry().cols);
    let url = "https://wsp.dev/x";
    feed(&view, format!("\x1b[{}G{url}", cols as usize - url.len() + 1).as_bytes(), cx);
    cx.simulate_modifiers_change(gesture());
    let at = centre(&view, cols - 1, 0, cx);
    cx.simulate_mouse_move(at, None, gesture());
    assert_eq!(view.read_with(cx, |v, _| v.hovered_link().map(|l| l.text.clone())).as_deref(), Some(url), "the last cell carries it");
    let last = view.read_with(cx, |v, _| v.cell_bounds(cols - 1, 0).unwrap());
    cx.simulate_mouse_move(point(last.right() + px(1.), last.center().y), None, gesture());
    assert!(
        view.read_with(cx, |v, _| v.hovered_link().is_none()),
        "the padding right of the grid is no cell, as today's `terminalGridCellAt`"
    );
}

#[gpui_kit::test]
fn the_scrollbar_shows_with_scrollback_and_its_keys_move_the_view(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    assert!(cx.debug_bounds("terminal-scrollbar").is_none());
    let lines: String = (0..60).map(|i| format!("line {i}\r\n")).collect();
    feed(&view, lines.as_bytes(), cx);
    assert!(cx.debug_bounds("terminal-scrollbar").is_some(), "scrollback brings the scrollbar");
    let bar = view.read_with(cx, |v, _| v.geometry().bounds);
    let thumb = point(bar.right() - px(3.), bar.bottom() - px(8.));
    cx.simulate_mouse_down(thumb, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(thumb, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes("home");
    let top = view.read_with(cx, |v, _| v.frame().scrollbar);
    assert_eq!(top.offset, 0, "Home on the focused scrollbar goes to the first row: {top:?}");
    cx.simulate_keystrokes("down down");
    assert_eq!(view.read_with(cx, |v, _| v.frame().scrollbar.offset), 2);
    cx.simulate_keystrokes("end");
    assert!(view.read_with(cx, |v, _| v.frame().scrollbar.at_bottom()));
}

#[gpui_kit::test]
fn a_drag_past_the_grids_edge_keeps_selecting_and_scrolling(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let lines: String = (0..60).map(|i| format!("line {i}\r\n")).collect();
    feed(&view, lines.as_bytes(), cx);
    let bounds = view.read_with(cx, |v, _| v.geometry().bounds);
    let none = Modifiers::none();
    let at_bottom = view.read_with(cx, |v, _| v.frame().scrollbar);
    let (from, above) = (centre(&view, 2, 3, cx), point(bounds.left() + px(20.), bounds.top() - px(20.)));
    cx.simulate_mouse_down(from, MouseButton::Left, none);
    cx.simulate_mouse_move(above, MouseButton::Left, none);
    cx.executor().advance_clock(std::time::Duration::from_millis(400));
    settle(cx);
    cx.simulate_mouse_up(above, MouseButton::Left, none);
    let scrolled = view.read_with(cx, |v, _| v.frame().scrollbar);
    assert!(scrolled.offset < at_bottom.offset, "a selection dragged above the grid scrolls back: {at_bottom:?} to {scrolled:?}");
    let thumb = point(bounds.right() - px(3.), bounds.bottom() - px(8.));
    cx.simulate_mouse_down(thumb, MouseButton::Left, none);
    cx.simulate_mouse_move(point(thumb.x, bounds.top() - px(50.)), MouseButton::Left, none);
    cx.simulate_mouse_up(point(thumb.x, bounds.top() - px(50.)), MouseButton::Left, none);
    assert_eq!(view.read_with(cx, |v, _| v.frame().scrollbar.offset), 0, "the thumb follows the pointer above the pane to the first row");
}

#[gpui_kit::test]
fn the_ghostty_config_lays_its_colours_padding_and_cursor_over_the_theme(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let config: wsp_host_client::protocol::TerminalConfig = serde_json::from_value(serde_json::json!({
        "files": [], "fontFamily": [], "palette": [null, {"r": 1, "g": 2, "b": 3}, null, null, null, null, null, null, null, null, null, null, null, null, null, null],
        "background": {"r": 16, "g": 17, "b": 18}, "cursorStyle": "bar", "cursorStyleBlink": false,
        "windowPaddingX": {"left": 10, "right": 12}, "backgroundOpacity": 0.5, "selectionBackground": {"r": 200, "g": 100, "b": 0}
    }))
    .unwrap();
    let (glass, solid) = cx.update(|_, cx| {
        let glass =
            Look::with(Some(&config), &desktop_gpui::screens::terminal::look::Choices { glass: true, zoom: 3., ..Default::default() }, cx);
        let solid = Look::with(Some(&config), &Default::default(), cx);
        (glass, solid)
    });
    assert_eq!(glass.colors.background, Rgb(16, 17, 18));
    assert_eq!(glass.colors.palette.iter().find(|(i, _)| *i == 1).unwrap().1, Rgb(1, 2, 3));
    assert_eq!(
        glass.colors.palette.iter().find(|(i, _)| *i == 2).unwrap().1,
        Rgb(0x68, 0x6e, 0x25),
        "a slot the file leaves keeps the theme's"
    );
    assert_eq!((glass.padding.left, glass.padding.right, glass.padding.top), (10., 12., 4.));
    assert_eq!((glass.opacity, solid.opacity), (0.5, 1.), "without glass the pane is opaque");
    assert_eq!(glass.font_size, 17., "14 px and a zoom of 3");
    assert!(!glass.cursor_blink);
    view.update(cx, |v, cx| v.set_look(glass, cx));
    settle(cx);
    let (frame, g) = view.read_with(cx, |v, _| (v.frame().clone(), v.geometry()));
    assert_eq!(frame.background, Rgb(16, 17, 18));
    assert_eq!(frame.cursor.unwrap().shape, CursorShape::Bar);
    assert_eq!(g.x0, 10.);
}

#[gpui_kit::test]
fn the_cursor_holds_still_under_reduced_motion(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    cx.update(|window, cx| {
        window.activate_window();
        cx.set_reduce_motion(true);
    });
    cx.executor().advance_clock(std::time::Duration::from_millis(200));
    settle(cx);
    let blinks = std::rc::Rc::new(std::cell::Cell::new(0));
    let count = blinks.clone();
    let _watch = cx.update(|_, cx| cx.observe(&view, move |_, _| count.set(count.get() + 1)));
    // The blink waits out half a cycle of real time since the last output.
    std::thread::sleep(std::time::Duration::from_millis(600));
    cx.executor().advance_clock(std::time::Duration::from_secs(2));
    cx.run_until_parked();
    assert_eq!(blinks.get(), 0, "no blink redraws the view");
}

#[gpui_kit::test]
fn the_grid_reflows_at_once_and_the_pty_hears_the_size_150_ms_after_it_settles(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let session = view.read_with(cx, |v, _| v.session().clone());
    let resizes = |cx: &mut VisualTestContext| {
        session.update(cx, |s, _| s.take_out()).into_iter().filter(|o| matches!(o, Out::Resize { .. })).collect::<Vec<_>>()
    };
    let wait = |cx: &mut VisualTestContext| {
        cx.executor().advance_clock(std::time::Duration::from_millis(160));
        settle(cx);
    };
    wait(cx);
    resizes(cx);
    cx.simulate_resize(size(px(600.), px(300.)));
    settle(cx);
    let g = view.read_with(cx, |v, _| v.geometry());
    assert_eq!(session.read_with(cx, |s, _| s.engine().size()), (g.cols, g.rows), "the screen reflows in the same frame");
    assert!(resizes(cx).is_empty(), "the pty hears nothing while the size may still move");
    wait(cx);
    assert_eq!(resizes(cx), [Out::Resize { cols: g.cols, rows: g.rows }]);
}

#[gpui_kit::test]
fn a_right_click_asks_for_the_menu_unless_the_program_takes_the_mouse(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    let asked = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let into = asked.clone();
    let _watch = cx.update(|_, cx| {
        cx.subscribe(&view, move |_, event: &ViewEvent, _| {
            if let ViewEvent::Menu(at) = event {
                into.borrow_mut().push(*at);
            }
        })
    });
    let at = centre(&view, 4, 2, cx);
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    settle(cx);
    assert_eq!(asked.borrow().as_slice(), [at], "a right-click asks the pane for the menu where it fell");
    _ = sent(&view, cx);
    feed(&view, b"\x1b[?1000h\x1b[?1006h", cx);
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::none());
    settle(cx);
    assert_eq!(asked.borrow().len(), 1, "no menu while the program takes the mouse");
    assert!(sent(&view, cx).starts_with("\x1b[<2;5;3M"), "the program gets the right button at its cell");
    cx.simulate_mouse_down(at, MouseButton::Right, Modifiers::shift());
    cx.simulate_mouse_up(at, MouseButton::Right, Modifiers::shift());
    settle(cx);
    assert_eq!(asked.borrow().len(), 2, "Shift takes the press back for the menu");
}

#[gpui_kit::test]
fn a_mouse_report_names_the_cell_under_the_pointer_across_the_grid(cx: &mut TestAppContext) {
    let (cx, view) = open(cx);
    feed(&view, b"\x1b[?1000h\x1b[?1006h", cx);
    let cols = view.read_with(cx, |v, _| v.geometry().cols);
    for x in [0, cols / 2, cols - 1] {
        _ = sent(&view, cx);
        let at = centre(&view, x, 2, cx);
        cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
        cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
        assert_eq!(sent(&view, cx), format!("\x1b[<0;{n};3M\x1b[<0;{n};3m", n = x + 1), "a press and release on column {x}");
    }
}

/// A pane around one view that takes the split and new actions, as the drawer does.
struct Pane {
    view: Entity<TerminalView>,
    taken: std::rc::Rc<std::cell::Cell<usize>>,
}

impl gpui_kit::Render for Pane {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{InteractiveElement, ParentElement, Styled};
        let (split, new) = (self.taken.clone(), self.taken.clone());
        gpui_kit::div()
            .size_full()
            .on_action(move |_: &keys::SplitTerminal, _, _| split.set(split.get() + 1))
            .on_action(move |_: &keys::NewTerminal, _, _| new.set(new.get() + 1))
            .child(self.view.clone())
    }
}

#[gpui_kit::test]
fn with_the_macs_keys_bound_cmd_d_and_cmd_n_act_and_ctrl_d_and_ctrl_n_stay_the_shells(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.bind_keys(keys::bindings_of(true, &keys::bound(true, &Default::default())));
    });
    let taken = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|_| Session::new("pty_1", "shell 1", 80, 24));
        let look = Look::new(None, None, cx);
        let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
        made = Some(view.clone());
        let pane = cx.new(|_| Pane { view, taken: taken.clone() });
        Root::new(pane, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    let view = made.unwrap();
    view.update_in(cx, |v, window, cx| v.focus_handle(cx).focus(window, cx));
    settle(cx);
    _ = sent(&view, cx);
    cx.simulate_keystrokes("cmd-d cmd-n");
    assert_eq!(taken.get(), 2, "Cmd+D splits and Cmd+N opens a shell from a focused terminal");
    assert_eq!(sent(&view, cx), "", "neither chord sends the shell a byte");
    cx.simulate_keystrokes("ctrl-d ctrl-n");
    assert_eq!(taken.get(), 2, "the Control chords split nothing");
    assert_eq!(sent(&view, cx), "\x04\x0e", "and reach the shell");
}

#[cfg(not(target_os = "macos"))]
#[gpui_kit::test]
fn with_the_apps_keys_bound_ctrl_d_and_ctrl_n_stay_the_shells(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.bind_keys(keys::bindings_of(false, &keys::bound(false, &Default::default())));
    });
    let taken = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|_| Session::new("pty_1", "shell 1", 80, 24));
        let look = Look::new(None, None, cx);
        let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
        made = Some(view.clone());
        let pane = cx.new(|_| Pane { view, taken: taken.clone() });
        Root::new(pane, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    let view = made.unwrap();
    view.update_in(cx, |v, window, cx| v.focus_handle(cx).focus(window, cx));
    settle(cx);
    _ = sent(&view, cx);
    cx.simulate_keystrokes("ctrl-d ctrl-n");
    assert_eq!(taken.get(), 0, "neither chord splits or opens a shell from a focused terminal off the Mac");
    assert_eq!(sent(&view, cx), "\x04\x0e");
}

/// A pane that takes the zoom chord, as the drawer does.
struct Zooming {
    view: Entity<TerminalView>,
    zooms: std::rc::Rc<std::cell::Cell<usize>>,
}

impl gpui_kit::Render for Zooming {
    fn render(&mut self, _: &mut gpui_kit::Window, _: &mut gpui_kit::Context<Self>) -> impl gpui_kit::IntoElement {
        use gpui_kit::{InteractiveElement, ParentElement, Styled};
        let zooms = self.zooms.clone();
        gpui_kit::div().size_full().on_action(move |_: &keys::ZoomInTerminal, _, _| zooms.set(zooms.get() + 1)).child(self.view.clone())
    }
}

#[gpui_kit::test]
fn a_chord_the_app_takes_sends_the_program_no_release(cx: &mut TestAppContext) {
    let mac = cfg!(target_os = "macos");
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
        cx.bind_keys(keys::bindings_of(mac, &keys::bound(mac, &Default::default())));
    });
    let zooms = std::rc::Rc::new(std::cell::Cell::new(0));
    let mut made = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let session = cx.new(|_| Session::new("pty_1", "shell 1", 80, 24));
        let look = Look::new(None, None, cx);
        let view = cx.new(|cx| TerminalView::new(session, look, window, cx));
        made = Some(view.clone());
        let pane = cx.new(|_| Zooming { view, zooms: zooms.clone() });
        Root::new(pane, window, cx)
    });
    cx.simulate_resize(size(px(800.), px(400.)));
    let view = made.unwrap();
    view.update_in(cx, |v, window, cx| v.focus_handle(cx).focus(window, cx));
    settle(cx);
    feed(&view, b"\x1b[>2u", cx);
    _ = sent(&view, cx);
    let press_and_release = |chord: &str, cx: &mut VisualTestContext| {
        cx.simulate_keystrokes(chord);
        cx.simulate_event(gpui_kit::KeyUpEvent { keystroke: gpui_kit::Keystroke::parse(chord).unwrap() });
    };
    press_and_release(if mac { "cmd-=" } else { "ctrl-=" }, cx);
    assert_eq!(zooms.get(), 1);
    assert_eq!(sent(&view, cx), "", "the program never heard the press, so it hears no release");
    press_and_release("up", cx);
    assert_eq!(sent(&view, cx), "\x1b[1;1:1A\x1b[1;1:3A", "a key the program got still sends its release under Kitty's event flag");
}
