// SPDX-License-Identifier: AGPL-3.0-only
//! The reads and the write beside the rows (client.ts:614-680, spec/client-rows.md rules 9, 10, 12): which account
//! a word names, what one computer settings write asks the host, and the reads sent together.

mod common;

use serde_json::{json, Value};

use common::ScriptedHost;
use wsp_cli::verbs::client::{account_named, read_computers, set_computer, ComputerRef, Dialer};
use wsp_wire::types::usage::AccountRow;
use wsp_wire::views::place::PlaceSetAlso;
use wsp_wire::views::place_more::{PlaceSettingWord, PlaceSettingsAsk, PlaceView};
use wsp_words::exit::Kind;

fn rows() -> Vec<AccountRow> {
    serde_json::from_value(json!([
        { "key": "claude:me@x.io", "agent": "claude", "label": "Me", "computers": ["this mac"], "address": "me@x.io" },
        { "key": "codex:team", "agent": "codex", "label": "Team", "computers": [] },
        { "key": "claude:team", "agent": "claude", "label": "team", "computers": [] },
    ]))
    .unwrap()
}

#[test]
fn an_account_is_named_by_its_key_first_then_by_its_label_or_address_in_any_case() {
    let rows = rows();
    assert_eq!(account_named(&rows, "codex:team").unwrap().key, "codex:team", "the exact key wins over a label");
    assert_eq!(account_named(&rows, "ME").unwrap().key, "claude:me@x.io");
    assert_eq!(account_named(&rows, "Me@X.io").unwrap().key, "claude:me@x.io");
}

#[test]
fn a_word_no_account_answers_is_not_found_and_one_two_answer_names_both_keys() {
    let rows = rows();
    let none = account_named(&rows, "nobody").err().expect("refused");
    assert_eq!(none.kind, Some(Kind::NotFound));
    let two = account_named(&rows, "TEAM").err().expect("refused");
    assert_eq!(two.kind, Some(Kind::Usage));
    assert_eq!(two.message, "TEAM names 2 accounts: codex:team and claude:team. Name one by its key.");
}

fn none() -> PlaceSettingsAsk {
    serde_json::from_value(json!({})).unwrap()
}

const PLACES: &str = r#"{"id":1,"ok":true,"places":[{"id":"here","kind":"computer","name":"this mac","default":true},{"id":"place-9","kind":"computer","name":"attic","default":false}]}"#;
const SET: &str = r#"{"id":1,"ok":true,"place":{"id":"place-9","kind":"computer","name":"loft","default":false}}"#;

fn host() -> std::sync::Arc<ScriptedHost> {
    let mut script = common::Script::default();
    script.replies.insert("places.list".into(), PLACES.into());
    script.replies.insert("places.set".into(), SET.into());
    script.replies.insert("cost.spend".into(), r#"{"id":1,"ok":true,"places":[]}"#.into());
    ScriptedHost::new(script)
}

#[tokio::test]
async fn one_settings_write_names_the_computer_by_its_id_then_the_settings_then_reset_only_when_named_then_what_else_was_given() {
    let host = host();
    let client = host.client(None).await.unwrap();
    let set: PlaceSettingsAsk = serde_json::from_value(json!({ "threads": 3 })).unwrap();
    let also = PlaceSetAlso { name: Some("loft".into()), ssh: None, recipe: None };
    let (row, was) = set_computer(client.as_ref(), ComputerRef::Word("attic"), &set, &[], &also).await.unwrap();
    assert_eq!((row.name.as_str(), was.as_str()), ("loft", "attic"), "the row it reads now and the name it was asked by");
    let asked: Vec<String> = host.asked().iter().map(|f| serde_json::to_string(f).unwrap()).collect();
    assert_eq!(asked, vec![r#"{"op":"places.list"}"#.to_owned(), r#"{"placeId":"place-9","threads":3,"name":"loft","op":"places.set"}"#.to_owned()]);
}

#[tokio::test]
async fn a_row_already_read_is_not_listed_again_and_a_reset_rides_as_its_words() {
    let host = host();
    let client = host.client(None).await.unwrap();
    let row: PlaceView = serde_json::from_value(json!({ "id": "place-9", "kind": "computer", "name": "attic", "default": false })).unwrap();
    let reset: Vec<PlaceSettingWord> = serde_json::from_value(json!(["nap", "threads"])).unwrap();
    set_computer(client.as_ref(), ComputerRef::Row(&row), &none(), &reset, &PlaceSetAlso::default()).await.unwrap();
    let asked: Vec<Value> = host.asked();
    assert_eq!(serde_json::to_string(&asked).unwrap(), r#"[{"placeId":"place-9","reset":["nap","threads"],"op":"places.set"}]"#);
}

#[tokio::test]
async fn an_unknown_computer_is_refused_before_anything_is_written() {
    let host = host();
    let client = host.client(None).await.unwrap();
    let e = set_computer(client.as_ref(), ComputerRef::Word("nope"), &none(), &[], &PlaceSetAlso::default()).await.err().expect("refused");
    assert_eq!(e.kind, Some(Kind::Usage));
    assert!(e.message.ends_with(" Run wsp computers."), "{}", e.message);
    assert_eq!(host.ops(), vec!["places.list"]);
}

#[tokio::test]
async fn the_computers_read_asks_the_list_and_the_spend_together_and_reads_no_pending_as_none() {
    let host = host();
    let client = host.client(None).await.unwrap();
    let read = read_computers(client.as_ref()).await.unwrap();
    assert_eq!(read.computers.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["this mac", "attic"]);
    assert!(read.spend.is_empty() && read.pending.is_empty());
    let mut ops = host.ops();
    ops.sort();
    assert_eq!(ops, vec!["cost.spend", "places.list"]);
}
