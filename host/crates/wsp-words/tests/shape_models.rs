// SPDX-License-Identifier: AGPL-3.0-only
//! shapeModels (thread-defaults.ts:275-296 at 65f6b89c7): the model picker's list. Each case in
//! fixtures/shape_models.jsonl is the catalog, the picker and what node 22 answered for the TS, one case a line.

use serde_json::Value;
use wsp_wire::types::thread_defaults::ModelPicker;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_words::thread_defaults::shape_models;

#[test]
fn shape_models_answers_what_the_ts_answers() {
    let mut seen = 0;
    for line in include_str!("fixtures/shape_models.jsonl").lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        let catalog: HarnessCatalog = serde_json::from_value(case["catalog"].clone()).unwrap();
        let picker: Option<ModelPicker> = (!case["picker"].is_null()).then(|| serde_json::from_value(case["picker"].clone()).unwrap());
        let got = serde_json::to_value(shape_models(&catalog, picker.as_ref())).unwrap();
        assert_eq!(got, case["want"], "{} with {}", case["name"], case["picker"]);
        seen += 1;
    }
    assert_eq!(seen, 14);
}

// thread-defaults.test.ts:192: hidden models leave the list, the person's order comes first, a hidden default hands its mark on.
#[test]
fn shape_models_hides_orders_and_moves_the_mark() {
    let line = include_str!("fixtures/shape_models.jsonl").lines().nth(1).unwrap();
    let case: Value = serde_json::from_str(line).unwrap();
    let catalog: HarnessCatalog = serde_json::from_value(case["catalog"].clone()).unwrap();
    let picker: ModelPicker = serde_json::from_value(case["picker"].clone()).unwrap();
    let shaped = shape_models(&catalog, Some(&picker));
    let values: Vec<&str> = shaped.models.iter().map(|m| m.value.as_str()).collect();
    assert_eq!(values, ["haiku", "sonnet"]);
    assert_eq!(shaped.models[0].is_default, Some(true));
    let hidden = shaped.hidden_models.expect("opus is kept apart");
    assert_eq!(hidden.iter().map(|m| (m.value.as_str(), m.is_default)).collect::<Vec<_>>(), [("opus", None)]);
}

// thread-defaults.test.ts:207: no picker leaves the catalog as it is.
#[test]
fn shape_models_with_no_picker_is_the_catalog() {
    let line = include_str!("fixtures/shape_models.jsonl").lines().next().unwrap();
    let case: Value = serde_json::from_str(line).unwrap();
    let catalog: HarnessCatalog = serde_json::from_value(case["catalog"].clone()).unwrap();
    assert_eq!(shape_models(&catalog, None), catalog);
}
