// Review of the `computer` area: one failing test per finding, each asserting the exact value node printed for the TS.

use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_words::words::computer::place_root_shell_refusal;
use wsp_words::workspace_state::{absent_computer, away_ms_of, on_delete_of, waited_line, CopyAt};

// node: M["workspace-state"].awayMsOf({ lastSeenAt: "2026-10-25" }, 1792951200000)  ->  64800000
// Date.parse reads a date with no time (UTC) and several other forms; the Rust reads RFC 3339 only, so it says None.
#[test]
fn away_ms_of_reads_a_date_with_no_time_as_date_parse_does() {
    assert_eq!(away_ms_of(Some("2026-10-25"), 1792951200000.0), Some(64800000.0));
}

// node: M["words/computer"].placeRootShellRefusal("root@box", "constructor")
// SHELL_STARTUP_FILE["constructor"] is Object's own function, not undefined, so the TS says it "reads function Object() { [native code] }".
#[test]
fn place_root_shell_refusal_reads_a_shell_named_like_an_object_property_as_the_ts_does() {
    assert_eq!(
        place_root_shell_refusal("root@box", "constructor"),
        "root@box runs constructor as its root's login shell, and sshd hands every command wsp sends to that shell: constructor reads function Object() { [native code] } before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root; run chsh -s /bin/bash root on it and add it again"
    );
}

// node: M["workspace-state"].absentComputer("spoo", NaN).line  ->  "no answer NaN d, is it on?"
// Math.max(0, NaN) is NaN in JS; Rust's f64::max(NaN, 0.0) is 0.0, so offline_for (words/places.rs:436) says "0 min".
#[test]
fn absent_computer_dates_a_nan_silence_as_the_ts_does() {
    assert_eq!(absent_computer("spoo", Some(f64::NAN)).line, "no answer NaN d, is it on?");
}

// node: M["workspace-state"].waitedLine(499.99999999999994)  ->  "Waited 0 s for your computer"
// Math.round(0.49999999999999994) is 0; (x + 0.5).floor() is 1.
#[test]
fn waited_line_rounds_as_math_round_does() {
    assert_eq!(waited_line(499.99999999999994), "Waited 0 s for your computer");
}

// node: M["workspace-state"].onDeleteOf("local", { path: "/w/a\x00b" }).done("m_1")
//   ->  "its worktree at /w/a\x00b is removed and the project folder is left as it is"
// done_for swaps the first NUL for the machine id, so a NUL that is part of the path is swapped too.
#[test]
fn on_delete_done_keeps_a_nul_that_belongs_to_the_path() {
    let held = on_delete_of(&WorkspaceKind::Local, Some(&CopyAt { path: "/w/a\u{0}b".to_string() }), None, None);
    assert_eq!(held.done_for("m_1"), "its worktree at /w/a\u{0}b is removed and the project folder is left as it is");
}
