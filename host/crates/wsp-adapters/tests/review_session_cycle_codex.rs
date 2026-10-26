// SPDX-License-Identifier: AGPL-3.0-only
//! The same cycle as review_session_cycle.rs on the Codex adapter: its registry holds each session and each turn holds the
//! registry (codex/turn.rs `Sessions`), so a finished turn and the forward it was handed outlived the adapter.
#![allow(dead_code, unused_imports)]

#[path = "common/codex_wire.rs"]
mod codex_wire;

use std::sync::Arc;

use codex_wire::*;
use serde_json::Value;
use wsp_adapters::codex::adapter::{create_codex_adapter, CodexAdapterDeps};
use wsp_adapters::port::{HarnessAdapter, HarnessStartOptions, OnEvent};

const THREAD: &str = "01a0e2c1-5d10-7b42-9a6e-3f1c2d4b5a60";

fn thread_answer() -> String {
    format!(
        r#"{{"id":"wsp-thread","result":{{"thread":{{"id":"{THREAD}","model":"gpt-5.6-sol","cwd":"/root/app","turns":[]}},"model":"gpt-5.6-sol","cwd":"/root/app"}}}}"#
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_finished_codex_turn_lets_go_of_its_forward_once_its_adapter_is_dropped() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let launch = launcher(scripted_server(WireOpts {
                on_write: Some(Arc::new(|m: &Value, w: &Wire| match m.get("method").and_then(Value::as_str).unwrap_or("") {
                    "thread/resume" => w.push(&[thread_answer()]),
                    "turn/start" => {
                        let thread = m["params"]["threadId"].as_str().unwrap_or("").to_string();
                        w.push(&[
                            format!(r#"{{"method":"item/completed","params":{{"item":{{"type":"agentMessage","id":"m1","text":"ok"}},"threadId":"{thread}","turnId":"t1"}}}}"#),
                            format!(r#"{{"method":"turn/completed","params":{{"threadId":"{thread}","turn":{{"id":"t1","items":[],"status":"completed"}}}}}}"#),
                        ])
                    }
                    _ => {}
                })),
                ..Default::default()
            }));
            let adapter = create_codex_adapter(CodexAdapterDeps::new(launch.factory(), "/root/.codex", "codex login --device-auth")).unwrap();
            let forward: OnEvent = Arc::new(|_| {});
            let held = Arc::downgrade(&forward);
            let session = adapter
                .start(HarnessStartOptions { prompt: "go".into(), resume: Some(THREAD.into()), on_event: Some(forward), ..Default::default() })
                .unwrap();
            let _ = session.finished().await;
            for _ in 0..64 {
                tokio::task::yield_now().await;
            }
            drop((session, adapter, launch));
            assert!(
                held.upgrade().is_none(),
                "the forward of a finished codex turn is still held after its adapter was dropped (strong count {})",
                held.strong_count()
            );
        })
        .await
}
