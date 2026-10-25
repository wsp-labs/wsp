// Review tests for the `places` area. Every test here fails today for the reason in its comment; the node answer was
// run against /root/rvw2445/ts/bundle.mjs (see /root/rvw2445/places/probe/final_node.mjs).

use serde_json::json;
use wsp_wire::views::project_bundle::TerminalRgb;
use wsp_wire::views::workspace::WorkspaceView;
use wsp_words::place_name::workspace_computer_name_with;
use wsp_words::place_word::provider_key_name;
use wsp_words::views::place_setup_words::provision_log_line;
use wsp_words::words::places::{hex_color, host_beat_word, hosts_table, offline_for, HostsTableRow};

mod support;

// words/places.ts:212 offlineFor(NaN) is "NaN d": Math.max(0, NaN) is NaN. places.rs:436 uses f64::max, which drops the NaN and says "0 min".
#[test]
fn offline_for_nan_reads_nan_d() {
    assert_eq!(offline_for(f64::NAN), "NaN d");
}

// words/places.ts:562 hostBeatWord(NaN) is "away NaN d" (NaN <= 120000 is false, then offlineFor(NaN)); places.rs:1220 answers "away 0 min".
#[test]
fn host_beat_word_nan_reads_away_nan_d() {
    assert_eq!(host_beat_word(Some(f64::NAN)), "away NaN d");
}

// words/places.ts:569 hostsTable: the STATE cell of a row whose awayMs is NaN. Same root as offline_for.
#[test]
fn hosts_table_row_with_nan_away_reads_away_nan_d() {
    let row = HostsTableRow { host: "a".into(), away_ms: Some(Some(f64::NAN)), ..Default::default() };
    assert_eq!(hosts_table(&[row])[1], ["a", "waits on its next start", "away NaN d", "", "", "", ""]);
}

// words/places.ts:127 hexColor({r:-1,g:0,b:0}) is "#-10000" (toString(16) of -1 is "-1"); places.rs:261 prints the i64 two's complement.
#[test]
fn hex_color_of_a_negative_channel() {
    let rgb: TerminalRgb = serde_json::from_value(json!({ "r": -1, "g": 0, "b": 0 })).unwrap();
    assert_eq!(hex_color(&rgb), "#-10000");
}

// words/places.ts:127 hexColor({r:1e21,g:0,b:0}) is "#3635c9adc5dea000000000"; places.rs:261 goes through JsInt::get(), an i64 cast that saturates at 7fffffffffffffff.
#[test]
fn hex_color_of_a_channel_past_i64() {
    let rgb: TerminalRgb = serde_json::from_value(json!({ "r": 1e21, "g": 0, "b": 0 })).unwrap();
    assert_eq!(hex_color(&rgb), "#3635c9adc5dea000000000");
}

// place-setup-words.ts:286 provisionLogLine(253402300799999, "x") is "9999-12-31T23:59:59Z x". place_setup_words.rs:754 calls iso_seconds (words/thread/forks.rs:754), whose `.expect("a moment jiff holds")` panics past jiff's range.
#[test]
fn provision_log_line_at_the_last_instant_of_year_9999() {
    assert_eq!(provision_log_line(253402300799999.0, "x"), "9999-12-31T23:59:59Z x");
}

// toISOString writes years past 9999 as +YYYYYY: provisionLogLine(8.64e15, "x") is "+275760-09-13T00:00:00Z x" (the largest Date). Rust panics in the same expect.
#[test]
fn provision_log_line_at_the_largest_date() {
    assert_eq!(provision_log_line(8.64e15, "x"), "+275760-09-13T00:00:00Z x");
}

// toISOString writes a year before 0 as -YYYYYY: provisionLogLine(-62198755200000, "x") is "-000001-01-01T00:00:00Z x"; the Rust strftime writes "-001-01-01T00:00:00Z x".
#[test]
fn provision_log_line_in_year_minus_one() {
    assert_eq!(provision_log_line(-62198755200000.0, "x"), "-000001-01-01T00:00:00Z x");
}

// place-word.ts:38 PROVIDER_KEY_WORDS is a plain object, so providerKeyName("constructor") finds Object and answers its name, "Object". place_word.rs:43 answers "constructor".
#[test]
fn provider_key_name_of_an_inherited_key() {
    assert_eq!(provider_key_name("constructor"), "Object");
}

// place-name.ts:60 workspaceComputerName passes the whole workspace to whereWord, which reads facts.os when the word is "this computer"; the record that
// carries facts is a WorkspaceStatus or a WorkspaceListing, so the Rust takes them beside the WorkspaceView (a view has none).
// node: workspaceComputerName([], { kind: "cloud", provider: "this computer", facts: { os: "Darwin 23.1" }, ... }) is "this Mac"
#[test]
fn workspace_computer_name_reads_the_machines_os() {
    let workspace: WorkspaceView =
        serde_json::from_value(support::over(support::workspace_view(), json!({ "kind": "cloud", "provider": "this computer" }))).unwrap();
    let facts: wsp_wire::views::workspace_more::MachineFacts =
        serde_json::from_value(json!({ "os": "Darwin 23.1", "uptimeMs": 1, "folder": "/" })).unwrap();
    assert_eq!(workspace_computer_name_with(&[], &workspace, Some(&facts)), "this Mac");
    assert_eq!(workspace_computer_name_with(&[], &workspace, None), "this computer");
}
