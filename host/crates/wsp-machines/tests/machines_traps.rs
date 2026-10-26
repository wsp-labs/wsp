// SPDX-License-Identifier: AGPL-3.0-only
//! The runtime traps of packages/runtime/CLAUDE.md this slice holds, each pinned on the state it protects.
//! #2278: a replay and a create of one attempt send one key for one box, because both take `hold()` on the purpose
//! and the sweep's replay skips a purpose a create holds (machines.ts:111). The create's own hold while it asks the
//! provider is `claiming_holds_every_claimed_id_until_run_settles` in machines_create.rs.
mod common;
#[path = "support/machines.rs"]
mod support;

use serde_json::json;
use support::{BackendOpts, MachineDef, Opts, Replay, World, REQUEST};

#[test]
fn a_replay_skips_an_attempt_whose_purpose_a_create_holds_and_sends_its_key_once_released() {
    common::run_local(async {
        let old = support::iso(common::T0 - 120_000);
        let w = World::new(Opts {
            store: json!({"creates": {"workspace/ws_9": {
                "key": "workspace/ws_9:0101010101010101", "createdAt": old, "body": "B", "host": "host:ABC-1",
                "pid": "dead", "request": serde_json::from_str::<serde_json::Value>(REQUEST).unwrap(), "image": "wsp-abc-img"
            }}}),
            backend: Some(BackendOpts {
                replays: Some(vec![Replay::Machine(MachineDef::new("m-lost"))]),
                request: true,
                ..BackendOpts::default()
            }),
            ..Opts::default()
        });
        let release = w.area.hold("workspace/ws_9").await;
        let mut failed = Vec::new();
        let deleted = w
            .area
            .replay_creates(&Vec::new, &mut failed)
            .await
            .expect("the sweep's replay answers");
        assert!(
            deleted.is_empty() && failed.is_empty(),
            "nothing replayed while a create holds the purpose"
        );
        assert!(
            !w.log_now().iter().any(|e| e[0] == "replayCreate"),
            "no key sent for the held purpose"
        );
        assert_eq!(
            w.store.table("creates")[0][0],
            "workspace/ws_9",
            "the attempt stays for the next sweep"
        );

        release();
        let deleted = w
            .area
            .replay_creates(&Vec::new, &mut failed)
            .await
            .expect("the sweep's replay answers");
        let sent: Vec<_> = w
            .log_now()
            .into_iter()
            .filter(|e| e[0] == "replayCreate")
            .collect();
        assert_eq!(
            sent.len(),
            1,
            "once released, the attempt's key is sent once"
        );
        assert_eq!(
            sent[0][1], "workspace/ws_9:0101010101010101",
            "under the attempt's own key"
        );
        assert_eq!(
            deleted.len(),
            1,
            "the box no record waits for is reported lost"
        );
        assert_eq!(
            w.store.table("creates"),
            json!([]),
            "and the attempt is gone"
        );
    })
}
