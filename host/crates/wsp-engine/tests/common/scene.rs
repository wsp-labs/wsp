// SPDX-License-Identifier: AGPL-3.0-only
//! The replay half of `oracle/box_scenes.oracle.ts`: a scene's scripted Boat API built into a `FakeBox`, its budgets
//! onto `backend_on`, and what the Rust backend did held against what the TS did, request by request.
use std::sync::Arc;

use serde_json::{json, Value};

use super::fake_box::*;
use wsp_engine::box_backend::{BoxBudgetsAsk, BoxMachine};
use wsp_engine::machine::MachineKind;

pub const ID: &str = "bx_tumrjngm";

pub struct Scene {
    pub want: Value,
    pub api: Arc<FakeBox>,
    pub on: On,
    pub machine: BoxMachine,
}

pub fn scenes() -> Vec<Value> {
    let text = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/scenes/box.json")).unwrap();
    serde_json::from_str::<Vec<Value>>(&text).unwrap()
}

/// The scene named `name`, its routes on a fresh fake and its budgets on a fresh backend with the fake clock at
/// 1,000,000.
pub fn scene(name: &str) -> Scene {
    let want = scenes().into_iter().find(|s| s["name"] == name).unwrap_or_else(|| panic!("no scene {name}"));
    let api = FakeBox::new();
    let b = &want["budgets"];
    let ms = |k: &str| b.get(k).and_then(Value::as_u64);
    let on = backend_on(
        &api,
        "trial",
        BoxBudgetsAsk {
            poll_ms: ms("pollMs"),
            pause_ms: ms("pauseMs"),
            resume_ms: ms("resumeMs"),
            create_ms: ms("createMs"),
            snapshot_ms: ms("snapshotMs"),
            restore_ms: ms("restoreMs"),
            read_ms: ms("readMs"),
            backstop_ms: ms("backstopMs"),
        },
    );
    for (route, replies) in want["routes"].as_object().unwrap() {
        let (method, path) = route.split_once(' ').unwrap();
        let answers = replies
            .as_array()
            .unwrap()
            .iter()
            .map(|r| match r.get("road").and_then(Value::as_str) {
                Some("offline") => Answer::Road(offline()),
                Some("dropped") => Answer::Road(dropped()),
                _ => Answer::Reply(Reply { status: r["status"].as_u64().unwrap() as u16, body: r["body"].clone() }),
            })
            .collect();
        api.on(method, path, answers);
    }
    let machine = BoxMachine::new(on.backend.clone(), ID, MachineKind::Sandbox, None, None, false);
    Scene { want, api, on, machine }
}

/// `/tmp/wsp-put-<12 hex>` and `/tmp/wsp-get-<12 hex>` with the hex as `X`, since the TS pins its random bytes and
/// the Rust draws its own.
pub fn masked(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = ["wsp-put-", "wsp-get-"].iter().filter_map(|m| rest.find(m).map(|i| (i, m.len()))).min() {
        let (i, n) = at;
        out.push_str(&rest[..i + n]);
        rest = &rest[i + n..];
        let hex = rest.chars().take_while(|c| c.is_ascii_hexdigit()).count();
        if hex == 12 {
            out.push('X');
            rest = &rest[12..];
        }
    }
    out.push_str(rest);
    out
}

fn calls_of(api: &FakeBox) -> Vec<Value> {
    api.seen()
        .into_iter()
        .map(|s| {
            json!({
                "method": s.method,
                "path": masked(&s.path),
                "query": s.query.iter().map(|(k, v)| json!([k, masked(v)])).collect::<Vec<_>>(),
                "headers": s.headers,
                "raw": s.raw.as_deref().map(masked),
            })
        })
        .collect()
}

fn want_calls(want: &Value) -> Vec<Value> {
    want["calls"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            json!({
                "method": c["method"],
                "path": masked(c["path"].as_str().unwrap()),
                "query": c["query"].as_array().unwrap().iter().map(|p| json!([p[0], masked(p[1].as_str().unwrap())])).collect::<Vec<_>>(),
                "headers": c["headers"],
                "raw": c["raw"].as_str().map(masked),
            })
        })
        .collect()
}

impl Scene {
    /// Every request, sleep and warn line as the TS made them, in order, and the clock moved as far.
    pub fn same_road(&self) {
        let got = calls_of(&self.api);
        let want = want_calls(&self.want);
        for (i, (g, w)) in got.iter().zip(want.iter()).enumerate() {
            assert_eq!(g, w, "request {i} differs");
        }
        assert_eq!(got.len(), want.len(), "requests: got {got:#?}");
        let sleeps: Vec<u64> = self.want["sleeps"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(self.on.clock.sleeps(), sleeps, "sleeps");
        let warns: Vec<String> = self.want["warns"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(self.on.warned.lines(), warns, "warn lines");
        assert_eq!(self.on.clock.now_ms() - 1_000_000, self.want["elapsed"].as_i64().unwrap(), "time spent");
    }

    /// The answer as the TS gave it, compared as JSON.
    pub fn same_out(&self, got: Value) {
        assert_eq!(got, self.want["out"], "the answer");
        assert!(self.want["err"].is_null(), "the TS failed: {}", self.want["err"]);
    }

    /// The failure's words as the TS gave them.
    pub fn same_err(&self, got: impl std::fmt::Display) {
        assert_eq!(got.to_string(), self.want["err"].as_str().expect("the TS answered"), "the failure's words");
    }
}
