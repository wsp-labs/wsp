// SPDX-License-Identifier: AGPL-3.0-only
//! The road with no provider on an empty line, against the TS (tests/fixtures/scenes/units.json; spec/machine.md rule
//! N-1).
use wsp_engine::machine::MachineBackend;
use wsp_engine::no_provider_backend::NoProviderBackend;

const UNIT: &str = "machine";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// N-1: the default line stands in only for an absent one; an empty line is kept and refuses with no words.
#[tokio::test]
async fn an_empty_line_is_kept_and_refuses_with_no_words() {
    let e = NoProviderBackend::new(Some(String::new())).get("x").await.err().expect("refused");
    assert_eq!(e.to_string(), pins()["emptyLine"]["err"].as_str().unwrap());
}
