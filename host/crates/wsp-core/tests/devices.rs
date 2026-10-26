// SPDX-License-Identifier: AGPL-3.0-only
//! spec/devices.md; the three "one writer over the device records" cases of packages/runtime/test/pairing.test.ts
//! (:454-489), the store facts of its "pairing codes" cases (:163-194), and the cases the spec pins.
mod support;

use std::rc::Rc;

use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::{run_local, MemStore, SeqIds};
use wsp_core::devices::{
    make_devices, safe_equal, thread_of, token_digest, DeviceDoor, HeldDevice, MintMore, DEVICES, NO_DEVICE_DOOR, PAIRINGS, REVOKED,
};
use wsp_wire::views::workspace_more::{ThreadScope, WorkspaceOrigin};
use wsp_wire::wire::devices::{DeviceVia, PAIR_CODE_ALPHABET};

fn door() -> (Rc<MemStore>, Rc<SeqIds>, Rc<dyn DeviceDoor>) {
    let store = MemStore::new();
    let ids = SeqIds::new();
    let door = make_devices(store.clone(), ids.clone());
    (store, ids, door)
}

/// The code the `n`th draw of eight bytes makes.
fn code_of(n: u64) -> String {
    SeqIds::expected(n, 8).iter().map(|b| PAIR_CODE_ALPHABET.as_bytes()[(b & 31) as usize] as char).collect()
}

fn token_of(n: u64) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(SeqIds::expected(n, 24))
}

fn id_of(n: u64) -> String {
    format!("d_{}", hex::encode(SeqIds::expected(n, 8)))
}

fn scope(thread: &str) -> ThreadScope {
    serde_json::from_value(json!({"kind": "thread", "threadId": thread, "workspaceId": "ws_1a2b3c4d", "rootThreadId": thread})).unwrap()
}

fn via(fingerprint: &str) -> DeviceVia {
    serde_json::from_value(
        json!({"kind": "account", "relayDeviceId": "rd_1", "fingerprint": fingerprint, "publicKey": "pk", "admittedBy": "fp_host"}),
    )
    .unwrap()
}

#[test]
fn a_revoke_that_lands_while_a_last_seen_write_is_in_flight_stays_revoked() {
    run_local(async {
        let (store, _, door) = door();
        let code = door.issue(1_000, 60_000, false).await.unwrap().code;
        let paired = door.redeem(&code, "laptop", 1_000).await.unwrap().unwrap();
        // A paired client redialing with backoff while somebody at the host revokes it: both roads read the record,
        // and without one writer the later write puts the revoked device back.
        let (_, revoked) = futures::join!(door.seen(&paired.device_id, 2_000), door.revoke(&paired.device_id));
        assert!(revoked.unwrap());
        assert_eq!(door.list().await.unwrap(), vec![]);
        assert_eq!(door.r#match(&paired.device_token).await.unwrap(), None);
        assert_eq!(store.text(DEVICES, &paired.device_id), None);
    });
}

#[test]
fn holds_in_the_other_order_too_and_a_seen_after_a_revoke_writes_nothing_back() {
    run_local(async {
        let (store, _, door) = door();
        let code = door.issue(1_000, 60_000, false).await.unwrap().code;
        let paired = door.redeem(&code, "laptop", 1_000).await.unwrap().unwrap();
        let puts = store.puts.get();
        let (revoked, moved) = futures::join!(door.revoke(&paired.device_id), door.seen(&paired.device_id, 2_000));
        assert!(revoked.unwrap());
        assert_eq!(moved.unwrap(), None);
        assert_eq!(store.puts.get(), puts, "a seen after the revoke wrote nothing");
        assert_eq!(door.list().await.unwrap(), vec![]);
    });
}

#[test]
fn two_sockets_spending_one_code_get_one_device_between_them() {
    run_local(async {
        let (_, _, door) = door();
        let code = door.issue(1_000, 60_000, false).await.unwrap().code;
        let (one, two) = futures::join!(door.redeem(&code, "one", 1_000), door.redeem(&code, "two", 1_000));
        let (one, two) = (one.unwrap(), two.unwrap());
        assert!(one.is_some(), "the first redeem in the chain spends the code");
        assert_eq!(two, None);
        let listed = door.list().await.unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "one");
    });
}

#[test]
fn issue_mints_eight_symbols_of_the_alphabet_and_stores_the_code_with_its_expiry() {
    run_local(async {
        let (store, _, door) = door();
        let issued = door.issue(1_000, 600_000, false).await.unwrap();
        assert_eq!(issued.code, code_of(1));
        assert_eq!(issued.expires_at, 601_000);
        assert_eq!(store.text(PAIRINGS, &issued.code).unwrap(), format!(r#"{{"code":"{}","expiresAt":601000}}"#, issued.code));
        let here = door.issue(1_000, 600_000, true).await.unwrap();
        assert_eq!(store.text(PAIRINGS, &here.code).unwrap(), format!(r#"{{"code":"{}","expiresAt":601000,"here":true}}"#, here.code));
    });
}

#[test]
fn clears_codes_that_ran_out_when_the_next_one_is_minted() {
    run_local(async {
        let (store, _, door) = door();
        let stale = door.issue(1_000, 600_000, false).await.unwrap().code;
        let live = door.issue(1_000, 3_600_000, false).await.unwrap().code;
        // Exactly at its expiry a code is still held; one past it, the next mint clears it.
        door.issue(601_000, 600_000, false).await.unwrap();
        assert!(store.text(PAIRINGS, &stale).is_some());
        let fresh = door.issue(601_001, 600_000, false).await.unwrap().code;
        assert_eq!(store.text(PAIRINGS, &stale), None);
        assert!(store.text(PAIRINGS, &live).is_some());
        assert!(store.text(PAIRINGS, &fresh).is_some());
    });
}

#[test]
fn a_code_redeemed_at_its_expiry_spends_and_one_past_it_is_refused_and_forgotten() {
    run_local(async {
        let (store, _, door) = door();
        let on_time = door.issue(1_000, 60_000, false).await.unwrap().code;
        assert!(door.redeem(&on_time, "laptop", 61_000).await.unwrap().is_some());
        let late = door.issue(1_000, 60_000, false).await.unwrap().code;
        assert_eq!(door.redeem(&late, "laptop", 61_001).await.unwrap(), None);
        assert_eq!(store.text(PAIRINGS, &late), None, "a guess never gets two tries");
        assert_eq!(door.redeem(&late, "laptop", 1_000).await.unwrap(), None, "a later clock does not let it in");
        assert_eq!(door.list().await.unwrap().len(), 1);
    });
}

#[test]
fn spend_answers_true_once_and_false_after() {
    run_local(async {
        let (store, _, door) = door();
        let code = door.issue(1_000, 60_000, false).await.unwrap().code;
        assert!(door.spend(&code, 2_000).await.unwrap());
        assert!(!door.spend(&code, 2_000).await.unwrap());
        assert_eq!(door.redeem(&code, "x", 2_000).await.unwrap(), None, "one code store: spent for both roads");
        assert!(store.ids(DEVICES).is_empty(), "a spend buys no device");
    });
}

#[test]
fn a_malformed_pairing_row_is_no_code_and_stays_where_it_is() {
    run_local(async {
        let (store, _, door) = door();
        store.seed(PAIRINGS, "ABCDEFGH", json!({"code": "ABCDEFGH", "expiresAt": "never"}));
        assert_eq!(door.redeem("ABCDEFGH", "x", 1).await.unwrap(), None);
        assert!(store.text(PAIRINGS, "ABCDEFGH").is_some());
    });
}

#[test]
fn keeps_only_the_tokens_hash_never_the_token() {
    run_local(async {
        let (store, _, door) = door();
        let code = door.issue(1_000, 60_000, false).await.unwrap().code;
        let paired = door.redeem(&code, "laptop", 1_000).await.unwrap().unwrap();
        // Draws: the code, then the token, then the id.
        assert_eq!(paired.device_token, token_of(2));
        assert_eq!(paired.device_token.len(), 32);
        assert_eq!(paired.device_id, id_of(3));
        let text = store.text(DEVICES, &paired.device_id).unwrap();
        assert!(!text.contains(&paired.device_token));
        let digest = hex::encode(Sha256::digest(paired.device_token.as_bytes()));
        assert_eq!(token_digest(&paired.device_token), digest);
        assert!(text.contains(&format!(r#""tokenHash":"{digest}""#)));
    });
}

#[test]
fn match_finds_the_device_a_token_names_and_writes_nothing() {
    run_local(async {
        let (store, _, door) = door();
        let first = door.admit("first", 1_000).await.unwrap();
        let second = door.admit("second", 2_000).await.unwrap();
        let (puts, deletes) = (store.puts.get(), store.deletes.get());
        let held = door.r#match(&second.device_token).await.unwrap().unwrap();
        assert_eq!(held.view, second.device);
        assert_eq!(held.road, None);
        assert_eq!(held.view.last_seen_at, "1970-01-01T00:00:02.000Z", "a match moves no last seen");
        assert_eq!(door.r#match(&first.device_token).await.unwrap().unwrap().view.id, first.device_id);
        assert_eq!(door.r#match("not-a-token").await.unwrap(), None);
        assert_eq!((store.puts.get(), store.deletes.get()), (puts, deletes));
    });
}

#[test]
fn a_device_row_without_a_token_hash_is_no_device() {
    run_local(async {
        let (store, _, door) = door();
        store.seed(
            DEVICES,
            "d_0000000000000000",
            json!({"id": "d_0000000000000000", "name": "ghost", "createdAt": "x", "lastSeenAt": "x"}),
        );
        assert_eq!(door.list().await.unwrap(), vec![]);
        assert_eq!(door.seen("d_0000000000000000", 5).await.unwrap(), None);
        assert!(!door.revoke("d_0000000000000000").await.unwrap());
    });
}

#[test]
fn a_device_record_keeps_its_keys_in_the_written_order() {
    run_local(async {
        let (store, _, door) = door();
        let code = door.issue(1_000, 60_000, true).await.unwrap().code;
        let paired = door.redeem(&code, " laptop ", 1_000).await.unwrap().unwrap();
        let hash = token_digest(&paired.device_token);
        assert_eq!(
            store.text(DEVICES, &paired.device_id).unwrap(),
            format!(
                r#"{{"id":"{}","name":"laptop","tokenHash":"{hash}","createdAt":"1970-01-01T00:00:01.000Z","lastSeenAt":"1970-01-01T00:00:01.000Z","here":true}}"#,
                paired.device_id
            )
        );
        let minted = door.mint("turn", scope("t1"), 3_000, MintMore { road: Some(WorkspaceOrigin::Here), aside: true }).await.unwrap();
        let hash = token_digest(&minted.device_token);
        assert_eq!(
            store.text(DEVICES, &minted.device_id).unwrap(),
            format!(
                r#"{{"id":"{}","name":"turn","tokenHash":"{hash}","createdAt":"1970-01-01T00:00:03.000Z","lastSeenAt":"1970-01-01T00:00:03.000Z","scope":{{"kind":"thread","threadId":"t1","workspaceId":"ws_1a2b3c4d","rootThreadId":"t1"}},"road":"here","aside":true}}"#,
                minted.device_id
            )
        );
    });
}

#[test]
fn a_seen_moves_only_the_last_seen_and_keeps_the_key_order() {
    run_local(async {
        let (store, _, door) = door();
        let paired = door.admit("laptop", 1_000).await.unwrap();
        let before = store.text(DEVICES, &paired.device_id).unwrap();
        let held = door.seen(&paired.device_id, 9_000).await.unwrap().unwrap();
        assert_eq!(held.view.last_seen_at, "1970-01-01T00:00:09.000Z");
        assert_eq!(held.view.created_at, "1970-01-01T00:00:01.000Z");
        assert_eq!(
            store.text(DEVICES, &paired.device_id).unwrap(),
            before.replace("lastSeenAt\":\"1970-01-01T00:00:01.000Z", "lastSeenAt\":\"1970-01-01T00:00:09.000Z")
        );
    });
}

#[test]
fn a_blank_name_reads_a_paired_computer() {
    run_local(async {
        let (_, _, door) = door();
        assert_eq!(door.admit("   ", 1).await.unwrap().device.name, "a paired computer");
        assert_eq!(door.admit("\u{feff} laptop \n", 2).await.unwrap().device.name, "laptop", "trimmed as JavaScript trims");
    });
}

#[test]
fn a_code_issued_here_admits_a_device_read_as_here() {
    run_local(async {
        let (_, _, door) = door();
        let code = door.issue(1_000, 60_000, true).await.unwrap().code;
        let paired = door.redeem(&code, "browser", 1_000).await.unwrap().unwrap();
        let listed: Value = serde_json::to_value(&door.list().await.unwrap()[0]).unwrap();
        assert_eq!(listed["here"], json!(true));
        assert_eq!(serde_json::to_value(&paired.device).unwrap()["here"], json!(true));
        let plain = door.admit("other", 2_000).await.unwrap();
        assert!(serde_json::to_value(&plain.device).unwrap().get("here").is_none());
    });
}

#[test]
fn mint_scopes_a_token_to_its_thread_with_its_road_and_admit_never_carries_one() {
    run_local(async {
        let (_, _, door) = door();
        let minted = door.mint("turn", scope("t1"), 1_000, MintMore { road: Some(WorkspaceOrigin::Here), aside: false }).await.unwrap();
        let view = serde_json::to_value(&minted.device).unwrap();
        assert_eq!(view["scope"]["threadId"], json!("t1"));
        assert!(view.get("road").is_none() && view.get("aside").is_none(), "the listing leaves the road out");
        let held = door.r#match(&minted.device_token).await.unwrap().unwrap();
        assert_eq!(held.road, Some(WorkspaceOrigin::Here));
        let read = thread_of(&held).unwrap();
        assert_eq!((read.by, read.road), (scope("t1"), WorkspaceOrigin::Here));
        let plain = door.admit("laptop", 2_000).await.unwrap();
        let held = door.r#match(&plain.device_token).await.unwrap().unwrap();
        assert_eq!((held.view.scope.clone(), held.road), (None, None));
        assert!(thread_of(&held).is_none());
    });
}

#[test]
fn thread_of_reads_relayed_for_a_scoped_device_with_no_road() {
    let view = serde_json::from_value(json!({"id": "d_1", "name": "n", "createdAt": "a", "lastSeenAt": "a", "scope": {"kind": "thread", "threadId": "t", "workspaceId": "ws_1a2b3c4d", "rootThreadId": "t"}})).unwrap();
    let read = thread_of(&HeldDevice { view, road: None }).unwrap();
    assert_eq!(read.road, WorkspaceOrigin::Relayed);
}

#[test]
fn revoke_asides_takes_away_only_the_side_question_tokens() {
    run_local(async {
        let (_, _, door) = door();
        let aside = door.mint("aside", scope("t1"), 1_000, MintMore { road: None, aside: true }).await.unwrap();
        let turn = door.mint("turn", scope("t2"), 2_000, MintMore::default()).await.unwrap();
        let laptop = door.admit("laptop", 3_000).await.unwrap();
        door.revoke_asides().await.unwrap();
        let left: Vec<String> = door.list().await.unwrap().into_iter().map(|d| d.id).collect();
        assert_eq!(left, vec![turn.device_id, laptop.device_id]);
        assert_eq!(door.r#match(&aside.device_token).await.unwrap(), None);
    });
}

#[test]
fn revoking_an_account_device_remembers_its_key_and_refuses_it_after() {
    run_local(async {
        let (store, _, door) = door();
        let admitted = door.admit_account("desk", via("fp_desk"), 1_000).await.unwrap();
        assert_eq!(serde_json::to_value(&admitted.device).unwrap()["via"]["fingerprint"], json!("fp_desk"));
        assert!(!door.refuses("fp_desk").await.unwrap());
        assert!(door.revoke(&admitted.device_id).await.unwrap());
        assert!(door.refuses("fp_desk").await.unwrap());
        assert_eq!(store.text(REVOKED, "fp_desk").unwrap(), r#"{"fingerprint":"fp_desk"}"#);
        assert!(!door.revoke(&admitted.device_id).await.unwrap(), "nothing left to take away");
        // A plain device's revoke remembers no key.
        let plain = door.admit("laptop", 2_000).await.unwrap();
        assert!(door.revoke(&plain.device_id).await.unwrap());
        assert_eq!(store.ids(REVOKED), vec!["fp_desk".to_string()]);
    });
}

#[test]
fn list_orders_devices_by_when_they_were_made() {
    run_local(async {
        let (_, _, door) = door();
        let c = door.admit("c", 3_000).await.unwrap();
        let a = door.admit("a", 1_000).await.unwrap();
        let b = door.admit("b", 2_000).await.unwrap();
        let order: Vec<String> = door.list().await.unwrap().into_iter().map(|d| d.id).collect();
        assert_eq!(order, vec![a.device_id, b.device_id, c.device_id]);
    });
}

#[test]
fn safe_equal_is_false_for_different_lengths_and_true_for_equal_bytes() {
    assert!(safe_equal("abc", "abc"));
    assert!(!safe_equal("abc", "abd"));
    assert!(!safe_equal("abc", "abcd"));
    assert!(!safe_equal("é", "e"), "byte lengths differ");
    assert!(safe_equal("", ""));
    assert_eq!(NO_DEVICE_DOOR, "this runtime keeps no devices; the host that serves the app wires one");
}

#[test]
fn a_failed_write_rejects_its_caller_and_the_chain_goes_on() {
    run_local(async {
        let (store, _, door) = door();
        store.fail_put.set(true);
        assert!(door.admit("lost", 1_000).await.is_err());
        store.fail_put.set(false);
        let kept = door.admit("kept", 2_000).await.unwrap();
        assert_eq!(door.list().await.unwrap().into_iter().map(|d| d.id).collect::<Vec<_>>(), vec![kept.device_id]);
    });
}

#[test]
fn invariant_3_a_requests_origin_is_read_off_the_token_it_presents() {
    run_local(async {
        let (_, _, door) = door();
        let here = door.mint("turn here", scope("t1"), 1_000, MintMore { road: Some(WorkspaceOrigin::Here), aside: false }).await.unwrap();
        let relayed = door
            .mint("turn on a machine", scope("t2"), 1_000, MintMore { road: Some(WorkspaceOrigin::Relayed), aside: false })
            .await
            .unwrap();
        let laptop = door.admit("laptop", 1_000).await.unwrap();
        // The road comes from the record the token names, so a token minted for a machine reads relayed whatever the
        // request that carries it says about itself.
        let read = |token: String| {
            let door = door.clone();
            async move { door.r#match(&token).await.unwrap().and_then(|held| thread_of(&held)).map(|t| (t.by.thread_id, t.road)) }
        };
        assert_eq!(read(here.device_token).await, Some(("t1".to_string(), WorkspaceOrigin::Here)));
        assert_eq!(read(relayed.device_token).await, Some(("t2".to_string(), WorkspaceOrigin::Relayed)));
        assert_eq!(read(laptop.device_token).await, None, "a person's computer holds no thread scope");
        assert_eq!(read("forged".to_string()).await, None);
    });
}
