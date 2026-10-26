// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the tests the implementer changed since c721249ce. Each test is written for the TS at 65f6b89c7.
#![allow(dead_code, unused_imports)]
mod support;

include!("serve_ops/world.rs");

use wsp_wire::wire::devices::PAIR_ISSUE_REFUSAL;
use wsp_wire::wire::place_link::{MINT_JOIN_REFUSAL, SSH_HOSTS_REFUSAL};

// BLOCKER (red at HEAD). packages/runtime/src/serve.ts:946 spreads what `places().set` answers, and that answer is
// `{ place }` (places/manage.ts:481 `return { place }`, places/types.ts:672 `Promise<{ place: PlaceView }>`), so the
// reply is `{"id":<the request's id>,"ok":true,"place":{...}}`. The app reads `.place` and waits on the request's own
// id (apps/web/src/protocol/client.ts:1243). node 22 over the real door printed
// `{"id":2,"ok":true,"place":{"id":"p_51e9c3fce8dd307b","kind":"computer",...}}`. Commit 8b6caa317 changed
// places_set_reads_a_recipe_word_as_its_slug_and_none_as_none to expect the view spread under the place's own id
// (a helper `answer_of` takes the last frame because no reply carries the request's id), which pins the src bug at
// src/serve/places.rs:148 `ok_reply(&id, spread(&view)?)`.
#[test]
fn places_set_answers_the_requests_id_with_the_view_under_place() {
    block(async {
        let r = rig_over(FakeRuntime::with_places(), |_| {});
        let view: PlaceView = place("p_1", "spoo", "computer");
        r.rt.script.once::<Result<PlaceView, HostError>>("places.set", Ok(view.clone()));
        let (host, sink) = r.host().await;
        send(&host, frame(1, "places.set", json!({ "placeId": "p_1", "threads": 3 }))).await;
        let frames = sink.frames();
        let last = frames.last().expect("a reply");
        let expected = json!({ "id": 1, "ok": true, "place": serde_json::to_value(&view).unwrap() });
        assert_eq!(last.to_string(), expected.to_string());
        assert!(sink.reply(1).is_some(), "a reply carries the request's own id, or the app's request never settles");
    })
}

// MINOR (coverage dropped; green at HEAD). Commit 8b6caa317 moved three tests from the sentences PAIR_ISSUE_REFUSAL,
// MINT_JOIN_REFUSAL and SSH_HOSTS_REFUSAL to the door's deviceHeldRefusal, which is right for a paired computer
// (pairing.test.ts:133-141, places-door.test.ts:205-206), but nothing in the crate now reaches the three sentences
// (serve.ts:887, 1025, 1041): the socket that does is the browser `wsp init` let in (pairing.test.ts:245-246) or a
// relayed one (pairing.test.ts:99, places-door.test.ts:207-208). The tests that asserted them are gone, so a change
// to src/serve/places.rs:69, 207 or 223 goes unseen.
#[test]
fn pair_issue_places_mint_and_ssh_hosts_keep_their_sentences_off_the_hosts_own_token() {
    block(async {
        let r = rig_over(FakeRuntime::with_places(), |_| {});
        let (browser, browser_sink) = r.browser().await;
        let (ticketed, ticket_sink) = r.ticketed().await;
        let asked = [("pair.issue", PAIR_ISSUE_REFUSAL), ("places.mint", MINT_JOIN_REFUSAL), ("places.sshHosts", SSH_HOSTS_REFUSAL)];
        for (i, (op, sentence)) in asked.iter().enumerate() {
            let id = 10 + i as u64;
            let want = json!({ "id": id, "ok": false, "error": sentence }).to_string();
            assert_eq!(request(&browser, &browser_sink, id, op, json!({})).await.to_string(), want, "{op} on the browser wsp init let in");
            assert_eq!(request(&ticketed, &ticket_sink, id, op, json!({})).await.to_string(), want, "{op} on a relayed socket");
        }
        assert!(r.rt.script.calls_to("devices.issue").is_empty());
    })
}

// NIT (assert loosened; green at HEAD). host_folders_wide_only_for_this_computers_own_window now reads a thread's
// reply as `ok: false` alone. serve.ts:652-655 refuses it at the door in `threadOpRefusal` (protocol runtime.ts
// threadOpRefusal), which is the sentence the test's own comment names.
#[test]
fn a_thread_asking_host_folders_is_refused_in_the_threads_own_sentence() {
    block(async {
        let r = rig(|_| {});
        let (thread, thread_sink) = r.thread("ws_1").await;
        let reply = request(&thread, &thread_sink, 1, "host.folders", json!({ "origin": "here" })).await;
        assert_eq!(reply["ok"], json!(false));
        let said = reply["error"].as_str().expect("a sentence");
        assert!(said.starts_with("host.folders is not a thread's to ask for; this request came in on the token of "), "{said}");
    })
}
