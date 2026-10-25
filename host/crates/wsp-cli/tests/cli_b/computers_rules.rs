// SPDX-License-Identifier: AGPL-3.0-only
// spec/computers.md's `size_asked` (computers.ts:36-42) on each kind of row, outside a whole line.

use serde_json::{json, Value};
use wsp_cli::verbs::computers::size_asked;
use wsp_wire::views::place_more::{PlaceSettingWord, PlaceView};

use crate::common::case;

fn places() -> Vec<PlaceView> {
    let c = case("computers", "computers_set", "a size word that names no size");
    let list: Value = serde_json::from_str(&c.replies["places.list"]).unwrap();
    serde_json::from_value(list["places"].clone()).unwrap()
}

fn row(id: &str) -> PlaceView {
    places().into_iter().find(|p| p.id == id).unwrap()
}

#[test]
fn size_asked_with_no_word_sets_nothing_and_keeps_the_resets() {
    let asked = size_asked(&row("box"), None, &[PlaceSettingWord::Threads]).unwrap();
    assert_eq!(asked.size, None);
    assert_eq!(asked.reset, [PlaceSettingWord::Threads]);
}

#[test]
fn size_asked_takes_a_word_that_names_a_size_on_any_row() {
    for id in ["box", "place-9"] {
        let asked = size_asked(&row(id), Some("4x8"), &[]).unwrap();
        assert_eq!(serde_json::to_value(asked.size.unwrap()).unwrap(), json!({ "cpu": 4, "memMb": 8192 }), "{id}");
        assert!(asked.reset.is_empty());
    }
}

#[test]
fn size_asked_refuses_a_word_naming_no_size_over_the_row_sizes_where_the_row_takes_one() {
    let e = size_asked(&row("box"), Some("0x4"), &[]).unwrap_err();
    assert_eq!(e.message, "0x4 is not a size this provider offers; the sizes are 2x4 ($0.018/hr), 4x8 ($0.036/hr), 8x16 ($0.072/hr). Name one of those with --size.");
    assert_eq!(e.exit_code(), 3);
}

#[test]
fn size_asked_turns_a_word_naming_no_size_into_one_reset_where_the_row_takes_none() {
    let asked = size_asked(&row("place-9"), Some("0x4"), &[PlaceSettingWord::Nap]).unwrap();
    assert_eq!(asked.size, None);
    assert_eq!(asked.reset, [PlaceSettingWord::Nap, PlaceSettingWord::Size]);
    let again = size_asked(&row("place-9"), Some("big"), &[PlaceSettingWord::Size]).unwrap();
    assert_eq!(again.reset, [PlaceSettingWord::Size], "size is never reset twice");
}
