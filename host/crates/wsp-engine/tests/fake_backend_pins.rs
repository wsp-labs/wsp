// SPDX-License-Identifier: AGPL-3.0-only
//! The stand-in's made-up prices at their rounding ties, against the TS (tests/fixtures/scenes/units.json;
//! spec/fake_backend.md rule F-1).
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_engine::machine::{MachineBackend, Size};

const UNIT: &str = "fake_backend";
fn pins() -> serde_json::Value {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/units.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&text).unwrap()[UNIT].clone()
}

/// F-1: `Number(x.toFixed(4))` rounds a tie away from zero on the decimal it prints: 0.03125 is 0.0313 and 0.09375 is
/// 0.0938, where Rust's `{:.4}` gives 0.0312 and 0.0938.
#[test]
fn rates_round_their_ties_as_to_fixed_does() {
    let backend = FakeBackend::new(FakeBackendOptions::default());
    for case in pins()["rates"].as_array().unwrap() {
        let size = Size { cpu: case["in"]["cpu"].as_f64().unwrap(), mem_mb: case["in"]["memMb"].as_f64().unwrap() };
        assert_eq!(backend.pricing().rate_usd_per_hour(size), case["out"].as_f64().unwrap(), "{}", case["in"]);
    }
}

/// F-2: the spec's figures are taken with `??`, so a 0 cpu, memory or disk stays 0 and an empty labels map stays
/// empty, never the default size (the TS's `createdAt` is the clock's and is not compared).
#[tokio::test]
async fn a_zero_figure_is_kept_not_defaulted() {
    let backend = FakeBackend::new(FakeBackendOptions::default());
    let spec: wsp_engine::machine::MachineSpec =
        serde_json::from_value(serde_json::json!({ "kind": "sandbox", "cpu": 0, "memMb": 0, "diskGb": 0, "labels": {} })).unwrap();
    let m = backend.create(spec, None, None).await.unwrap();
    let mut shape = serde_json::to_value(m.describe().await.unwrap()).unwrap();
    shape.as_object_mut().unwrap().shift_remove("createdAt");
    let mut want = pins()["zeros"]["shape"].clone();
    want.as_object_mut().unwrap().shift_remove("createdAt");
    assert_eq!(shape, want);
    let listed = serde_json::to_value(backend.list(None).await.unwrap()).unwrap();
    assert_eq!(listed[0]["labels"], pins()["zeros"]["listed"][0]["labels"]);
}
