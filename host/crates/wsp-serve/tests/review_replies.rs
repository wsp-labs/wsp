// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the reply shapes: each test names an op whose reply after auth differs from the TS reply for the same op.
//! The TS shape of each is read from the TS source, and run once through `serveRuntime` where the types left it open.
#![allow(dead_code, unused_imports)]
mod support;

include!("serve_ops/world.rs");

// MINOR. TS packages/runtime/src/serve.ts:1552-1554 `send({ id, ok: true, manifest: await rt.golden.get(name) })`;
// golden.get answers `GoldenManifest | undefined` (types/api.ts:`golden.get`), and JSON.stringify drops an undefined
// value, so a host with no golden answers `{"id":N,"ok":true}` (run through serveRuntime). Rust
// host/crates/wsp-serve/src/serve/ops.rs:195 `one("manifest", to_value(&Option<GoldenManifest>))` writes
// `"manifest":null`. The app (client.ts:1288) and the desktop (host-lifecycle.ts:487) read `.manifest` and both
// cope with null, so no client breaks; the bytes differ.
#[test]
fn golden_get_with_no_golden_leaves_manifest_out() {
    block(async {
        let r = rig(|_| {});
        r.rt.script.once::<Result<Option<wsp_wire::views::golden_image::GoldenManifest>, HostError>>("golden.get", Ok(None));
        let (host, sink) = r.host().await;
        request(&host, &sink, 1, "golden.get", json!({ "name": "default" })).await;
        assert_eq!(sink.last_text(), r#"{"id":1,"ok":true}"#);
    })
}

// MAJOR. TS packages/runtime/src/serve.ts:1377-1389 spreads `rt.workspaces.reviewDraft(...)`, which answers
// `{ review?: ReviewDraft }` (types/api.ts `reviewDraft`), so the reply is `{"id":N,"ok":true,"review":{...}}` (run
// through serveRuntime). Rust host/crates/wsp-serve/src/serve/workspaces.rs:283-286 spreads the draft itself:
// wsp-core's `review_draft` answers a bare `Option<ReviewDraft>`, so its keys land beside id and ok and there is
// no `review` key. The app reads `.review` (apps/web/src/protocol/client.ts:1083) and gets nothing, so the review
// panel never shows a draft.
#[test]
fn review_draft_answers_the_draft_under_review() {
    block(async {
        let r = rig(|_| {});
        let draft: wsp_wire::types::start::ReviewDraft = from(json!({ "note": "no review read", "at": 1 }));
        r.rt.script.once::<Result<Option<wsp_wire::types::start::ReviewDraft>, HostError>>("workspaces.reviewDraft", Ok(Some(draft)));
        let (host, sink) = r.host().await;
        request(&host, &sink, 1, "workspaces.reviewDraft", json!({ "workspaceId": "ws_1" })).await;
        assert_eq!(sink.last_text(), r#"{"id":1,"ok":true,"review":{"note":"no review read","at":1}}"#);
    })
}

// MAJOR. TS packages/runtime/src/serve-slates.ts:70-71 spreads `slates.resolve(...)`, which answers
// `{ values: { <path>: <value> } }` (slates.ts:1482-1487, run through serveRuntime:
// `{"id":4,"ok":true,"values":{"$a":null,"$b":null}}`). Rust host/crates/wsp-serve/src/serve_slates.rs:154 spreads
// wsp-slates' `Slates::resolve`, which answers the bare path-to-value map (`IndexMap<String, Value>`), so every path
// becomes a top-level key and there is no `values`. The app parses the reply with `SlatesResolveAnswer`
// (apps/web/src/slate/wire.ts:68) and reads `.values`, so a slate that binds a source never resolves. The
// wsp-slates implementation is still a `todo!()`; this is the contract its type sets.
#[test]
fn slates_resolve_answers_the_values_under_values() {
    block(async {
        let r = rig(|_| {});
        let mut values: IndexMap<String, Value> = IndexMap::new();
        values.insert("$a".into(), Value::Null);
        values.insert("$b".into(), json!(4));
        r.rt.script.once::<Result<IndexMap<String, Value>, HostError>>("slates.resolve", Ok(values));
        let (host, sink) = r.host().await;
        request(&host, &sink, 1, "slates.resolve", json!({ "threadId": "t1", "paths": ["$a", "$b"] })).await;
        assert_eq!(sink.last_text(), r#"{"id":1,"ok":true,"values":{"$a":null,"$b":4}}"#);
    })
}

// NIT. TS packages/runtime/src/serve-projects.ts:43 `{ ...p, label: placeSaid(p) }` puts `label` after every other
// key of a provider's row (run through threadPlaces: `{"id":"box","kind":"provider","name":"box","default":false,
// "label":"Boat"}`). Rust host/crates/wsp-serve/src/serve_projects.rs:93-98 sets `row.label` on the struct, which
// serializes `label` in its schema position, after `name`. A thread's `places.list` and `cost.spend` take this road.
#[test]
fn a_threads_provider_row_ends_with_its_label() {
    block(async {
        let rt = FakeRuntime::with_places();
        let r = rig_over(rt.clone(), |o| o.now = Some(Rc::new(|| 4242)));
        rt.script.once::<Result<WorkspaceView, HostError>>("workspaces.get", Ok(workspace("ws_1", "box1", "cloud", "running", json!({}))));
        rt.script.once::<Result<Vec<PlaceView>, HostError>>(
            "places.list",
            Ok(vec![place("pl_1", "laptop", "computer"), place("box", "box", "provider")]),
        );
        let (thread, sink) = r.thread("ws_1").await;
        let reply = request(&thread, &sink, 1, "places.list", json!({})).await;
        assert_eq!(keys(&reply["places"][0]), vec!["id", "kind", "name", "default", "label"]);
    })
}
