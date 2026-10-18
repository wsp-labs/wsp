// SPDX-License-Identifier: AGPL-3.0-only
//! Pty output that reaches an assertion in Ghostty 3425025e (`Screen.zig:1777`): a wide character a resize cut, then an
//! insert at the start of its line. The ReleaseFast library the app ships compiles the check out and goes on.

use desktop_gpui::screens::terminal::engine::Engine;

/// Aborts on a Debug library: `LIBGHOSTTY_VT_SYS_OPTIMIZE=Debug CARGO_TARGET_DIR=<scratch> cargo test --test terminal_ghostty_debug -- --ignored`.
#[test]
#[ignore = "run by hand against a Debug build of libghostty-vt"]
fn an_insert_on_a_line_whose_wide_character_a_resize_cut_leaves_the_terminal_answering() {
    let mut engine = Engine::new(4, 3);
    engine.write(b"\x1b[?1049h");
    engine.write("ab\u{4e16}".as_bytes());
    engine.resize(3, 3, 8, 16);
    engine.write(b"\x1b[1;1H\x1b[@");
    engine.write(b"\x1b[6n");
    assert_eq!(engine.take_replies(), b"\x1b[1;1R");
}
