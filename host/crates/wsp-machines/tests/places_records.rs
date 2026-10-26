// SPDX-License-Identifier: AGPL-3.0-only
//! places/records (spec `places_records.md`, rules R1-R34). The TS covers these through whole-host tests
//! (places.test.ts, places-setup.test.ts, places-remove.test.ts); each case here names the rule and the TS case it
//! stands for.

mod places_b_common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use places_b_common::*;
use serde_json::{json, Value};
use wsp_core::types::pending::{BlobMark, Store};
use wsp_machines::places::records::{PlaceRecordsArea, Reached};
use wsp_machines::places::types::*;
use wsp_store::keys::{fresh_ephemeral, verify_place_bytes};
use wsp_wire::views::place::PlaceAddJob;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::wire::place_link::{
    place_link_transcript, place_refusal_transcript, LinkEphemerals, LinkRole,
};
use wsp_words::exit::HostError;

fn job(id: &str, state: &str) -> PlaceAddJob {
    serde_json::from_value(json!({ "addId": id, "address": "10.0.0.5", "startedAt": "2026-10-02T00:00:00.000Z", "state": state, "steps": [] })).unwrap()
}

/// The place file a held read prints after the login's own banner.
fn held_read(place_id: &str, host_public_key: &str) -> String {
    format!(
        "Welcome to Ubuntu\n\nWSP_PLACE_HELD\n{}\n",
        serde_json::to_string_pretty(&json!({ "placeId": place_id, "name": "old-macbook", "hostName": "zingzys-mac", "hostUrls": ["http://192.168.1.10:14621"], "hostPublicKey": host_public_key, "keyPath": "/home/maya/.wsp/place-key", "joinedAt": "2026-10-01T00:00:00.000Z" })).unwrap()
    )
}

fn run_over(
    answer: impl Fn(&PlaceLogin, &str, u64, Option<&str>) -> PlaceResult<RunOver> + 'static,
) -> Rc<RunOverRoad> {
    Rc::new(RunOverRoad(Box::new(answer), RefCell::new(Vec::new())))
}

fn login() -> PlaceLogin {
    PlaceLogin {
        ssh: "maya@10.0.0.5".into(),
        key_path: Some("/home/dev/.ssh/id".into()),
        host_key: Some("ssh-ed25519 AAAA".into()),
    }
}

/// The oracle row with what `with_cap` adds taken off, as `rows_of` hands it in.
fn bare(row: &Value) -> PlaceView {
    let mut row = row.clone();
    for k in [
        "cap",
        "capDefault",
        "settings",
        "napMs",
        "napDefault",
        "turnLimitMs",
        "turnLimitDefault",
        "spawn",
        "spawnDefault",
        "running",
        "size",
        "sizeDefault",
        "deleteAfterMs",
    ] {
        row.as_object_mut().unwrap().shift_remove(k);
    }
    serde_json::from_value(row).unwrap()
}

/// The store a disk or a network would be: every read and write yields first, so a read-then-write that is not in its
/// turn lets another writer in between (places-fixture.ts `yielding`).
struct Yielding(Rc<dyn Store>);

#[async_trait(?Send)]
impl Store for Yielding {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        settle().await;
        self.0.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        settle().await;
        self.0.put(c, id, v).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        settle().await;
        self.0.list(c).await
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.0.keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        settle().await;
        self.0.delete(c, id).await
    }
    async fn get_blob(&self, c: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.0.get_blob(c, id).await
    }
    async fn put_blob(&self, c: &str, id: &str, b: Vec<u8>) -> Result<(), HostError> {
        self.0.put_blob(c, id, b).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.0.delete_blob(c, id).await
    }
    async fn stat_blob(&self, c: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.0.stat_blob(c, id).await
    }
}

// ---------------------------------------------------------------- R1-R3

#[test]
fn records_sorted_by_joined_at_skipping_non_records() {
    // spec: R1. TS: places.test.ts lists computers oldest join first.
    run(async {
        let d = door(|s| {
            s.records = vec![
                record_json(
                    json!({ "id": PLACE2, "name": "late", "joinedAt": "2026-10-05T00:00:00.000Z" }),
                ),
                record_json(json!({ "joinedAt": "2026-10-01T00:00:00.000Z" })),
            ];
        });
        d.store
            .seed(PLACES, "junk", json!({ "id": "junk", "name": "no key" }));
        d.store.seed(PLACES, "arr", json!([1, 2]));
        let ids: Vec<String> = d
            .records
            .records()
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.id)
            .collect();
        assert_eq!(ids, vec![PLACE.to_string(), PLACE2.to_string()]);
    });
}

#[test]
fn records_tie_keeps_store_order() {
    // spec: R1, stable sort on equal joinedAt.
    run(async {
        let d = door(|s| {
            s.records = vec![
                record_json(json!({ "id": "p_000000000000000b", "name": "b" })),
                record_json(json!({ "id": "p_000000000000000a", "name": "a" })),
            ];
        });
        let names: Vec<String> = d
            .records
            .records()
            .await
            .unwrap()
            .into_iter()
            .map(|r| r.name)
            .collect();
        assert_eq!(names, vec!["b", "a"]);
    });
}

#[test]
fn record_of_reads_only_records() {
    // spec: R2.
    run(async {
        let d = door(|_| {});
        d.store
            .seed(PLACES, "half", json!({ "id": "half", "publicKey": "k" }));
        assert_eq!(
            d.records.record_of(PLACE).await.unwrap(),
            Some(record(json!({})))
        );
        assert_eq!(d.records.record_of("half").await.unwrap(), None);
        assert_eq!(d.records.record_of("p_nope").await.unwrap(), None);
    });
}

#[test]
fn settings_of_reads_empty_for_absent_or_unparsed() {
    // spec: R3. TS: places-settings.test.ts reads caps through PlaceSettings.safeParse.
    run(async {
        let d = door(|s| {
            s.caps = vec![
                (PLACE.into(), json!({ "threads": 3, "napMs": null })),
                ("bad".into(), json!({ "threads": "many" })),
            ]
        });
        assert_eq!(
            js(&d.records.settings_of(PLACE).await.unwrap()),
            json!({ "threads": 3, "napMs": null })
        );
        assert_eq!(js(&d.records.settings_of("bad").await.unwrap()), json!({}));
        assert_eq!(
            js(&d.records.settings_of("p_nope").await.unwrap()),
            json!({})
        );
        // Read from the store each time, not the held copy.
        d.store.seed(CAPS, PLACE, json!({ "threads": 1 }));
        assert_eq!(
            js(&d.records.settings_of(PLACE).await.unwrap()),
            json!({ "threads": 1 })
        );
    });
}

// ---------------------------------------------------------------- R4-R9

fn table(
    wired: &str,
    rows: &[(&str, Arc<dyn wsp_engine::machine::MachineBackend>)],
) -> Rc<Providers> {
    Rc::new(Providers {
        wired: wired.into(),
        rows: rows
            .iter()
            .map(|(id, b)| (id.to_string(), b.clone()))
            .collect(),
    })
}

#[test]
fn provider_ids_wired_and_table() {
    // spec: R5: wired and listed, the table's whole list in its order.
    run(async {
        let d = door(|s| {
            s.providers = Some(table(
                "boat",
                &[
                    ("solari", StubBackend::new().arc()),
                    ("boat", StubBackend::new().arc()),
                ],
            ))
        });
        assert_eq!(d.records.provider_ids(), vec!["solari", "boat"]);
        assert_eq!(d.records.wired_provider().as_deref(), Some("boat"));
    });
}

#[test]
fn provider_ids_no_wired_drops_table_wired() {
    // spec: R5: a host wired to nowhere lists every other provider it holds a key for.
    run(async {
        let d = door(|s| {
            s.provider = None;
            s.providers = Some(table(
                "stand-in",
                &[
                    ("stand-in", StubBackend::new().arc()),
                    ("solari", StubBackend::new().arc()),
                ],
            ));
        });
        assert_eq!(d.records.provider_ids(), vec!["solari"]);
        let none = door(|s| s.provider = None);
        assert!(none.records.provider_ids().is_empty());
    });
}

#[test]
fn provider_ids_wired_not_in_table() {
    // spec: R5. The oracle's door has no table: the wired provider alone.
    run(async {
        let d = door(|_| {});
        assert_eq!(d.records.provider_ids(), vec!["boat"]);
        let other =
            door(|s| s.providers = Some(table("solari", &[("solari", StubBackend::new().arc())])));
        assert_eq!(other.records.provider_ids(), vec!["boat"]);
    });
}

#[test]
fn provider_rate_wired_reads_wiring() {
    // spec: R7: the wired id's rate is the wiring's, the table's backend for it never read.
    run(async {
        let mut b = StubBackend::new();
        b.rate = 9.0;
        let d = door(|s| s.providers = Some(table("boat", &[("boat", b.arc())])));
        assert_eq!(d.records.provider_rate("boat"), Some(0.25));
    });
}

#[test]
fn provider_rate_other_reads_backend_default_size() {
    // spec: R7.
    run(async {
        let mut b = StubBackend::new();
        b.rate = 1.5;
        let d = door(|s| {
            s.providers = Some(table(
                "boat",
                &[("boat", StubBackend::new().arc()), ("solari", b.arc())],
            ))
        });
        assert_eq!(d.records.provider_rate("solari"), Some(1.5));
        assert_eq!(d.records.provider_rate("nowhere"), None);
    });
}

#[test]
fn provider_sizes_empty_without_backend() {
    // spec: R8.
    run(async {
        let d = door(|s| s.providers = Some(table("boat", &[("boat", StubBackend::new().arc())])));
        assert_eq!(
            js(&d.records.provider_sizes("boat")),
            json!([{ "cpu": 2, "memMb": 4096, "rateUsdPerHour": 0 }])
        );
        assert!(d.records.provider_sizes("solari").is_empty());
    });
}

#[test]
fn image_facts_build_stopped_only_when_stopped() {
    // spec: R9. TS: copy-on-press.test.ts:127, :149 read build and buildStopped off a row.
    run(async {
        let d = door(|s| {
            s.opts = Box::new(|o| {
                o.copy_build = Some(Rc::new(|id: &str| match id {
                    "a" => Some(CopyBuild {
                        line: "copying the image: 40%".into(),
                        stopped: false,
                    }),
                    "b" => Some(CopyBuild {
                        line: "the copy stopped: disk full".into(),
                        stopped: true,
                    }),
                    _ => None,
                }))
            })
        });
        let base = bare(&at("/rows/plain/2"));
        let backend = StubBackend::new().arc();
        let mut a = base.clone();
        d.records.image_facts(&mut a, "a", None);
        assert_eq!(
            (a.build.as_deref(), a.build_stopped, a.builds_images),
            (Some("copying the image: 40%"), None, None)
        );
        let mut b = base.clone();
        d.records.image_facts(&mut b, "b", Some(&backend));
        assert_eq!(
            (b.build.as_deref(), b.build_stopped, b.builds_images),
            (Some("the copy stopped: disk full"), Some(true), Some(true))
        );
        let mut c = base;
        d.records.image_facts(&mut c, "c", None);
        assert_eq!(
            (c.build, c.build_stopped, c.builds_images),
            (None, None, None)
        );
    });
}

// ---------------------------------------------------------------- R10-R13

#[test]
fn row_ids_here_records_providers_in_order() {
    // spec: R10.
    run(async {
        let d = door(|s| {
            s.records = vec![
                record_json(
                    json!({ "id": PLACE2, "name": "b", "joinedAt": "2026-10-09T00:00:00.000Z" }),
                ),
                record_json(json!({})),
            ]
        });
        let ids: Vec<(String, String)> = d
            .records
            .row_ids()
            .await
            .unwrap()
            .into_iter()
            .map(|r| (r.id, js(&r.kind).as_str().unwrap().to_string()))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("here".into(), "computer".into()),
                (PLACE.into(), "computer".into()),
                (PLACE2.into(), "computer".into()),
                ("boat".into(), "provider".into())
            ]
        );
    });
}

#[test]
fn with_cap_computer_row_fields() {
    // spec: R11. Oracle /rows/set/1: threads 3 and a null nap set on the joined computer.
    run(async {
        let d = door(|s| {
            s.caps = vec![
                (PLACE.into(), json!({ "threads": 3, "napMs": null })),
                (
                    "boat".into(),
                    json!({ "machines": 2, "size": { "cpu": 4, "memMb": 8192 } }),
                ),
            ];
            s.recording.running.borrow_mut().insert(PLACE.into(), 2);
        });
        let want = at("/rows/set/1");
        let ids = d.records.row_ids().await.unwrap();
        let got = d.records.with_cap(bare(&want), &ids).await.unwrap();
        assert_eq!(js(&got), want);
    });
}

#[test]
fn with_cap_provider_row_fields() {
    // spec: R11, R13. Oracle /rows/set/2: machines 2 and a size on the wired cloud, no backend for its default.
    run(async {
        let d = door(|s| {
            s.caps = vec![(
                "boat".into(),
                json!({ "machines": 2, "size": { "cpu": 4, "memMb": 8192 } }),
            )]
        });
        let want = at("/rows/set/2");
        let ids = d.records.row_ids().await.unwrap();
        assert_eq!(
            js(&d.records.with_cap(bare(&want), &ids).await.unwrap()),
            want
        );
    });
}

#[test]
fn with_cap_settings_only_when_set() {
    // spec: R11: no caps document is no `settings` key; this computer takes no nap and no delete-after.
    run(async {
        let d = door(|_| {});
        let want = at("/rows/plain/0");
        let ids = d.records.row_ids().await.unwrap();
        let got = js(&d.records.with_cap(bare(&want), &ids).await.unwrap());
        assert_eq!(got, want);
        assert!(got.get("settings").is_none());
        assert!(got.get("napMs").is_none());
        assert!(got.get("deleteAfterMs").is_none());
    });
}

fn limits(plan_key: &str) -> wsp_wire::views::place_more::CloudLimits {
    serde_json::from_value(json!({ "plan": { "key": plan_key, "trial": false, "machinesAtOnce": 4, "sizes": ["2x4"] }, "creditHours": 12.5 })).unwrap()
}

fn limited(
    answer: Result<Option<wsp_wire::views::place_more::CloudLimits>, String>,
) -> Arc<StubBackend> {
    let mut b = StubBackend::new();
    b.offers.place_limits = true;
    *b.limits.lock().unwrap() = Some(answer);
    Arc::new(b)
}

#[test]
fn with_limits_first_call_unchanged_then_kept() {
    // spec: R12. TS: sub-spawn.test.ts:331 reads plan and creditHours off the cloud's row.
    run(async {
        let b = limited(Ok(Some(limits("pro"))));
        let backend: Arc<dyn wsp_engine::machine::MachineBackend> = b.clone();
        let d = door(|s| s.providers = Some(table("boat", &[("boat", backend)])));
        let row = bare(&at("/rows/plain/2"));
        let first = d.records.with_limits(row.clone());
        assert_eq!(
            js(&first),
            js(&row),
            "the first listing never waits on the read"
        );
        settle().await;
        let second = js(&d.records.with_limits(row));
        assert_eq!(
            second["plan"],
            json!({ "key": "pro", "trial": false, "machinesAtOnce": 4, "sizes": ["2x4"] })
        );
        assert_eq!(second["creditHours"], json!(12.5));
        assert!(second.get("starts").is_none());
        // A computer row is never read.
        let computer = bare(&at("/rows/plain/1"));
        assert_eq!(js(&d.records.with_limits(computer.clone())), js(&computer));
    });
}

#[test]
fn with_limits_failed_read_drops_kept() {
    // spec: R12: a failed read leaves the fields out rather than an answer nobody can say is still true.
    run(async {
        let b = limited(Ok(Some(limits("pro"))));
        let backend: Arc<dyn wsp_engine::machine::MachineBackend> = b.clone();
        let d = door(|s| s.providers = Some(table("boat", &[("boat", backend)])));
        let row = bare(&at("/rows/plain/2"));
        d.records.with_limits(row.clone());
        settle().await;
        *b.limits.lock().unwrap() = Some(Err("boat said 500".into()));
        assert!(
            js(&d.records.with_limits(row.clone()))
                .get("plan")
                .is_some(),
            "the kept answer rides this call"
        );
        settle().await;
        assert!(js(&d.records.with_limits(row)).get("plan").is_none());
    });
}

#[test]
fn with_limits_one_read_in_flight() {
    // spec: R12: a second ask while a read is out starts none.
    run(async {
        let b = limited(Ok(Some(limits("pro"))));
        let backend: Arc<dyn wsp_engine::machine::MachineBackend> = b.clone();
        let d = door(|s| s.providers = Some(table("boat", &[("boat", backend)])));
        let row = bare(&at("/rows/plain/2"));
        d.records.with_limits(row.clone());
        d.records.with_limits(row.clone());
        d.records.with_limits(row);
        settle().await;
        assert_eq!(
            b.asked
                .lock()
                .unwrap()
                .iter()
                .filter(|a| *a == "placeLimits")
                .count(),
            1
        );
        assert!(d.records.limits_asking.borrow().is_empty());
        assert!(d.records.limits_kept.borrow().contains_key("boat"));
    });
}

#[test]
fn size_facts_person_over_default() {
    // spec: R13.
    run(async {
        let d = door(|s| s.providers = Some(table("boat", &[("boat", StubBackend::new().arc())])));
        let mut row = bare(&at("/rows/plain/2"));
        d.records.size_facts(
            &mut row,
            "boat",
            &serde_json::from_value(json!({ "size": { "cpu": 4, "memMb": 8192 } })).unwrap(),
        );
        assert_eq!(js(&row.size), json!({ "cpu": 4, "memMb": 8192 }));
        assert_eq!(js(&row.size_default), json!({ "cpu": 2, "memMb": 4096 }));
        let mut none = bare(&at("/rows/plain/2"));
        d.records.size_facts(
            &mut none,
            "solari",
            &serde_json::from_value(json!({})).unwrap(),
        );
        assert_eq!((none.size, none.size_default), (None, None));
    });
}

// ---------------------------------------------------------------- R14

#[test]
fn until_daemon_version_answers_once_current() {
    // spec: R14. TS: places-update.test.ts waits for the new version in the record.
    run(async {
        let dv = daemon_version();
        let d = door(|s| {
            s.records = vec![record_json(
                json!({ "report": report(json!({ "daemonVersion": dv - 2 })) }),
            )]
        });
        let records = d.records.clone();
        let waiting = tokio::task::spawn_local(async move {
            records.until_daemon_version(PLACE, dv - 2, 60_000).await
        });
        pass(&d.clock, 1_000).await;
        assert!(!waiting.is_finished());
        // A version that moved but is still behind keeps the wait going.
        let mut doc = d.stored(PLACE).unwrap();
        doc["report"]["daemonVersion"] = json!(dv - 1);
        d.store.seed(PLACES, PLACE, doc.clone());
        pass(&d.clock, 1_000).await;
        assert!(!waiting.is_finished());
        doc["report"]["daemonVersion"] = json!(dv);
        d.store.seed(PLACES, PLACE, doc);
        pass(&d.clock, 600).await;
        assert_eq!(waiting.await.unwrap().unwrap(), dv);
    });
}

#[test]
fn until_daemon_version_gives_up_at_deadline() {
    // spec: R14: what it still reads when the wait runs out.
    run(async {
        let dv = daemon_version();
        let d = door(|s| {
            s.records = vec![record_json(
                json!({ "report": report(json!({ "daemonVersion": dv - 1 })) }),
            )]
        });
        let records = d.records.clone();
        let waiting = tokio::task::spawn_local(async move {
            records.until_daemon_version(PLACE, dv - 3, 5_000).await
        });
        pass(&d.clock, 4_000).await;
        assert!(!waiting.is_finished());
        pass(&d.clock, 1_500).await;
        assert_eq!(waiting.await.unwrap().unwrap(), dv - 1);
    });
}

#[test]
fn until_daemon_version_record_gone_reads_from() {
    // spec: R14: a record gone is `from`.
    run(async {
        let d = door(|s| s.records = vec![]);
        let records = d.records.clone();
        let waiting = tokio::task::spawn_local(async move {
            records.until_daemon_version(PLACE, 12, 1_000).await
        });
        pass(&d.clock, 1_500).await;
        assert_eq!(waiting.await.unwrap().unwrap(), 12);
    });
}

// ---------------------------------------------------------------- R15-R19, R24

#[test]
fn put_add_trims_finished_to_twenty_keeping_running() {
    // spec: R16. TS: places-install.test.ts keeps ADDS_KEPT finished adds.
    run(async {
        let d = door(|_| {});
        d.records
            .adds
            .borrow_mut()
            .insert("a_run".into(), job("a_run", "running"));
        for i in 0..22 {
            let id = format!("a_{i:02}");
            d.records
                .adds
                .borrow_mut()
                .insert(id.clone(), job(&id, "running"));
        }
        for i in 0..22 {
            let id = format!("a_{i:02}");
            d.records.put_add(&id, |mut j| {
                j.state = serde_json::from_value(json!("done")).unwrap();
                j
            });
        }
        let ids: Vec<String> = d.records.adds.borrow().keys().cloned().collect();
        let mut want = vec!["a_run".to_string()];
        want.extend((2..22).map(|i| format!("a_{i:02}")));
        assert_eq!(
            ids, want,
            "the two oldest finished went, the running one stayed first"
        );
    });
}

#[test]
fn put_add_unknown_id_is_nothing() {
    // spec: R16.
    run(async {
        let d = door(|_| {});
        d.records.put_add("a_nope", |_| panic!("no job, no call"));
        assert!(d.records.adds.borrow().is_empty());
    });
}

#[test]
fn keep_writes_install_road_onto_record() {
    // spec: R17, R18, R24. TS: places-install.test.ts "the record carries the login it was installed over".
    run(async {
        let d = door(|s| s.records = vec![]);
        let waiting = Rc::new(RefCell::new(
            wsp_machines::places::records::AwaitingInstall::default(),
        ));
        waiting.borrow_mut().place_id = Some(PLACE.into());
        waiting.borrow_mut().login = Some(PlaceLogin {
            ssh: "root@10.0.0.7".into(),
            key_path: Some("/k2".into()),
            host_key: None,
        });
        waiting.borrow_mut().back =
            Some(serde_json::from_value(json!({ "boxPort": 14700 })).unwrap());
        d.records
            .awaiting
            .borrow_mut()
            .insert("CODE1".into(), waiting);
        let kept = d
            .records
            .keep(record(
                json!({ "road": { "from": "10.0.0.7", "hostKey": "ssh-ed25519 OLD" } }),
            ))
            .await
            .unwrap();
        let road = json!({ "ssh": "root@10.0.0.7", "from": "10.0.0.7", "back": { "boxPort": 14700 }, "keyPath": "/k2", "hostKey": "ssh-ed25519 OLD" });
        assert_eq!(js(kept.road.as_ref().unwrap()), road);
        assert_eq!(d.stored(PLACE).unwrap()["road"], road);
        assert_eq!(
            d.ctx
                .kept
                .borrow()
                .get(PLACE)
                .unwrap()
                .road
                .as_ref()
                .unwrap()
                .ssh
                .as_deref(),
            Some("root@10.0.0.7")
        );
    });
}

#[test]
fn with_road_keeps_fields_install_lacks() {
    // spec: R17.
    let held = record(json!({}));
    let same = PlaceRecordsArea::with_road(held.clone(), None);
    assert_eq!(same, held);
    let no_login = PlaceRecordsArea::with_road(held.clone(), Some((&None, &None)));
    assert_eq!(no_login, held);
    let moved = PlaceRecordsArea::with_road(
        held,
        Some((
            &Some(PlaceLogin {
                ssh: "maya@10.0.0.9".into(),
                key_path: None,
                host_key: None,
            }),
            &None,
        )),
    );
    assert_eq!(
        js(moved.road.as_ref().unwrap()),
        json!({ "ssh": "maya@10.0.0.9", "keyPath": "/home/dev/.ssh/id", "hostKey": "ssh-ed25519 AAAA", "from": "10.0.0.5" })
    );
}

#[test]
fn login_of_empty_ssh_is_none() {
    // spec: R19.
    assert_eq!(
        PlaceRecordsArea::login_of(&record(json!({}))),
        Some(login())
    );
    assert_eq!(
        PlaceRecordsArea::login_of(&record(json!({ "road": { "ssh": "", "keyPath": "/k" } }))),
        None
    );
    assert_eq!(
        PlaceRecordsArea::login_of(&record(json!({ "road": null }))),
        None
    );
    assert_eq!(
        PlaceRecordsArea::login_of(&record(json!({ "road": { "ssh": "a@b" } }))),
        Some(PlaceLogin {
            ssh: "a@b".into(),
            key_path: None,
            host_key: None
        })
    );
}

// ---------------------------------------------------------------- R20-R23

#[test]
fn root_over_taken_answers_password() {
    // spec: R20. TS: places-remove.test.ts "a sudo that takes the password".
    run(async {
        let none = door(|_| {});
        assert_eq!(
            none.records
                .root_over(&login(), Some("pw"), SudoVerb::Remove, "old-macbook")
                .await
                .unwrap(),
            None
        );
        let taken = door(|s| {
            s.wiring = Box::new(|w| {
                w.sudo_over = Some(Rc::new(Sudo(Box::new(|_, pw, _, _| {
                    Ok(if pw.is_some() {
                        "taken".into()
                    } else {
                        "root".into()
                    })
                }))))
            })
        });
        assert_eq!(
            taken
                .records
                .root_over(&login(), Some("pw"), SudoVerb::Update, "old-macbook")
                .await
                .unwrap(),
            Some("pw".into())
        );
        assert_eq!(
            taken
                .records
                .root_over(&login(), None, SudoVerb::Update, "old-macbook")
                .await
                .unwrap(),
            None
        );
        let changed = door(|s| {
            s.wiring = Box::new(|w| {
                w.sudo_over = Some(Rc::new(Sudo(Box::new(|_, _, _, _| {
                    Err(place_host_key_changed("the key moved"))
                }))))
            })
        });
        let e = changed
            .records
            .root_over(&login(), None, SudoVerb::Remove, "old-macbook")
            .await
            .unwrap_err();
        assert!(is_place_host_key_changed(&e));
        assert_eq!(e.message, "the key moved");
    });
}

#[test]
fn login_answers_false_on_dial_failure_or_bound() {
    // spec: R21.
    run(async {
        let no_road = door(|_| {});
        assert!(no_road.records.login_answers(&login()).await);
        let ok =
            door(|s| s.wiring = Box::new(|w| w.dial = Some(Rc::new(Dial(Box::new(|_| Ok(())))))));
        assert!(ok.records.login_answers(&login()).await);
        let refused = door(|s| {
            s.wiring = Box::new(|w| {
                w.dial = Some(Rc::new(Dial(Box::new(|_| {
                    Err(HostError::new(
                        "ssh: connect to host 10.0.0.5 port 22: Connection refused",
                    ))
                }))))
            })
        });
        assert!(!refused.records.login_answers(&login()).await);
    });
}

#[test]
fn reached_over_order_of_checks() {
    // spec: R22 steps 1 and 2.
    run(async {
        let no_road = door(|_| {});
        assert_eq!(
            no_road
                .records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "this host holds no ssh road to read it over".into(),
                refused: false
            }
        );
        let road = run_over(|_, _, _, _| Ok(RunOver::default()));
        let r2 = road.clone();
        let no_home = door(move |s| s.wiring = Box::new(move |w| w.run_over = Some(r2)));
        let homeless = record(json!({ "report": report(json!({ "login": { "USER": "maya" } })) }));
        assert_eq!(
            no_home
                .records
                .reached_over(&homeless, &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "old-macbook never said where its home is".into(),
                refused: false
            }
        );
        assert!(
            road.1.borrow().is_empty(),
            "nothing ran before the home was known"
        );
    });
}

#[test]
fn reached_over_names_other_record() {
    // spec: R22 steps 6 and 7. TS: places-remove.test.ts "a login that reaches another computer added here".
    run(async {
        let key = wsp_store::keys::new_place_key_pair();
        let public = key.public_key.clone();
        let p2 = public.clone();
        let road = run_over(move |_, _, _, _| {
            Ok(RunOver {
                exit_code: 0,
                stdout: held_read(PLACE2, &p2),
                stderr: String::new(),
            })
        });
        let r2 = road.clone();
        let d = door(move |s| {
            s.host_key = key;
            s.records = vec![
                record_json(json!({})),
                record_json(json!({ "id": PLACE2, "name": "box2" })),
            ];
            s.wiring = Box::new(move |w| w.run_over = Some(r2));
        });
        assert_eq!(
            d.records
                .reached_over(&record(json!({})), &login(), Some("pw"))
                .await
                .unwrap(),
            Reached::Other {
                other: Some("box2".into())
            }
        );
        let same = d
            .records
            .reached_over(
                &record(json!({ "id": PLACE2, "name": "box2" })),
                &login(),
                None,
            )
            .await
            .unwrap();
        assert_eq!(same, Reached::Same);
        // The script read the place file under the record's home.
        assert!(
            road.1.borrow()[0].contains("/home/maya"),
            "{}",
            road.1.borrow()[0]
        );
        assert!(road.1.borrow()[0].starts_with("echo; echo WSP_PLACE_HELD"));
    });
}

#[test]
fn reached_over_refused_marks_login_refusal() {
    // spec: R22 step 3.
    run(async {
        let refused = door(|s| {
            s.wiring = Box::new(|w| {
                w.run_over = Some(run_over(|_, _, _, _| {
                    Err(place_login_refused(
                        "maya@10.0.0.5: Permission denied (publickey).\nmore",
                    ))
                }))
            })
        });
        assert_eq!(
            refused
                .records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "maya@10.0.0.5: Permission denied (publickey).\nmore".into(),
                refused: true
            }
        );
        let sudo = door(|s| {
            s.wiring = Box::new(|w| {
                w.run_over = Some(run_over(|_, _, _, _| {
                    Err(HostError::new("sudo: a password is required"))
                }))
            })
        });
        assert_eq!(
            sudo.records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "sudo: a password is required".into(),
                refused: false
            }
        );
        let other_key = door(|s| {
            s.wiring = Box::new(|w| {
                w.run_over = Some(run_over(|_, _, _, _| {
                    Ok(RunOver {
                        exit_code: 0,
                        stdout: held_read(PLACE, "SOMEONE ELSE"),
                        stderr: String::new(),
                    })
                }))
            })
        });
        assert_eq!(
            other_key
                .records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Other { other: None }
        );
    });
}

#[test]
fn reached_over_timed_out_and_exit_lines() {
    // spec: R22 steps 4 and 5.
    run(async {
        let timed = door(|s| {
            s.wiring = Box::new(|w| {
                w.run_over = Some(run_over(|_, _, _, _| {
                    Ok(RunOver {
                        exit_code: wsp_engine::child_exec::CHILD_TIMED_OUT,
                        stdout: String::new(),
                        stderr: String::new(),
                    })
                }))
            })
        });
        assert_eq!(
            timed
                .records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "the read timed out after 20s".into(),
                refused: false
            }
        );
        let said = door(|s| {
            s.wiring = Box::new(|w| {
                w.run_over = Some(run_over(|_, _, _, _| {
                    Ok(RunOver {
                        exit_code: 1,
                        stdout: String::new(),
                        stderr: "first\nsudo: 3 incorrect password attempts\n".into(),
                    })
                }))
            })
        });
        assert_eq!(
            said.records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "sudo: 3 incorrect password attempts".into(),
                refused: false
            }
        );
        let quiet = door(|s| {
            s.wiring = Box::new(|w| {
                w.run_over = Some(run_over(|_, _, _, _| {
                    Ok(RunOver {
                        exit_code: 255,
                        stdout: String::new(),
                        stderr: String::new(),
                    })
                }))
            })
        });
        assert_eq!(
            quiet
                .records
                .reached_over(&record(json!({})), &login(), None)
                .await
                .unwrap(),
            Reached::Unread {
                unread: "the read exited 255".into(),
                refused: false
            }
        );
    });
}

#[test]
fn hold_back_moved_writes_back_on_same_login_records_only() {
    // spec: R23. TS: places-remove.test.ts holds a box's forward while a record dials through it.
    run(async {
        let back = Rc::new(Back::default());
        let b2 = back.clone();
        let with_back = |id: &str, ssh: &str| {
            record_json(json!({ "id": id, "road": { "ssh": ssh, "back": { "boxPort": 14700 } } }))
        };
        let d = door(move |s| {
            s.records = vec![
                with_back(PLACE, "maya@10.0.0.5"),
                with_back(PLACE2, "root@10.0.0.9"),
            ];
            s.wiring = Box::new(move |w| w.back = Some(b2));
        });
        let held = d.records.record_of(PLACE).await.unwrap().unwrap();
        d.records.hold_back(&held);
        settle().await;
        assert_eq!(
            *back.log.borrow(),
            vec![format!("hold maya@10.0.0.5 /home/maya {PLACE}")]
        );
        // Nothing for a record with no forward.
        d.records.hold_back(&record(json!({})));
        settle().await;
        assert_eq!(back.log.borrow().len(), 1);
    });
}

// ---------------------------------------------------------------- R25-R28: the record turn, invariant 6

#[test]
fn change_one_at_a_time_per_place_no_field_lost() {
    // spec: R25, R26; runtime invariant 6 (packages/runtime/AGENTS.md): a computer's record changes only in its turn, so
    // no writer drops another's fields. TS: places-setup.test.ts runs a setup over a yielding store.
    run(async {
        let d = door(|s| s.opts = Box::new(|o| o.store = Rc::new(Yielding(o.store.clone()))));
        let (a, b, c) = (d.records.clone(), d.records.clone(), d.records.clone());
        let one = tokio::task::spawn_local(async move {
            a.change(PLACE, |now| {
                Some(PlaceRecord {
                    recipe: Some("api".into()),
                    ..now
                })
            })
            .await
        });
        let two = tokio::task::spawn_local(async move {
            b.change(PLACE, |now| {
                Some(PlaceRecord {
                    last_seen_at: "2026-10-09T00:00:00.000Z".into(),
                    ..now
                })
            })
            .await
        });
        let three = tokio::task::spawn_local(async move { c.write_seen(PLACE, NOW).await });
        one.await.unwrap().unwrap();
        two.await.unwrap().unwrap();
        three.await.unwrap().unwrap();
        let stored = d.stored(PLACE).unwrap();
        assert_eq!(
            stored["recipe"],
            json!("api"),
            "the first writer's field survived"
        );
        assert_eq!(
            stored["lastSeenAt"],
            json!("2023-11-14T22:13:20.000Z"),
            "the last writer read what the others wrote"
        );
        assert_eq!(
            d.ctx.kept.borrow().get(PLACE).unwrap().recipe.as_deref(),
            Some("api")
        );
        assert!(
            d.records.record_turns.keys().is_empty(),
            "the turn's entry goes once its last write settles"
        );
    });
}

#[test]
fn change_gone_record_writes_nothing() {
    // spec: R26.
    run(async {
        let d = door(|s| s.records = vec![]);
        assert_eq!(d.records.change(PLACE, Some).await.unwrap(), None);
        assert!(d.store.writes_to(PLACES).is_empty());
    });
}

#[test]
fn change_move_none_writes_nothing() {
    // spec: R26; and a move that answers the record unchanged still writes.
    run(async {
        let d = door(|_| {});
        assert_eq!(d.records.change(PLACE, |_| None).await.unwrap(), None);
        assert!(d.store.writes_to(PLACES).is_empty());
        let same = d.records.change(PLACE, Some).await.unwrap();
        assert_eq!(same, Some(record(json!({}))));
        assert_eq!(
            d.store.writes_to(PLACES),
            vec![format!("put places/{PLACE}")]
        );
    });
}

#[test]
fn change_chain_survives_a_failure() {
    // spec: R25: the caller gets its own failure and the next write still goes.
    run(async {
        let d = door(|_| {});
        d.store
            .fail_writes
            .borrow_mut()
            .insert(PLACES.into(), "disk full".into());
        let e = d
            .records
            .change(PLACE, |now| {
                Some(PlaceRecord {
                    recipe: Some("x".into()),
                    ..now
                })
            })
            .await
            .unwrap_err();
        assert_eq!(e.message, "disk full");
        let ok = d
            .records
            .change(PLACE, |now| {
                Some(PlaceRecord {
                    recipe: Some("y".into()),
                    ..now
                })
            })
            .await
            .unwrap();
        assert_eq!(ok.unwrap().recipe.as_deref(), Some("y"));
        assert_eq!(d.stored(PLACE).unwrap()["recipe"], json!("y"));
    });
}

#[test]
fn login_listed_appends_sorted_once() {
    // spec: R28. TS: places-agents.test.ts:570 lists a codex login landed.
    run(async {
        let d = door(|s| {
            s.records = vec![record_json(
                json!({ "report": report(json!({ "logins": ["zz/later.json", "a/first.json"] })) }),
            )]
        });
        let written = d
            .records
            .login_listed(PLACE, "codex")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            js(&written.report.logins),
            json!(["a/first.json", "codex/auth.json", "zz/later.json"])
        );
        assert_eq!(
            d.stored(PLACE).unwrap()["report"]["logins"],
            json!(["a/first.json", "codex/auth.json", "zz/later.json"])
        );
        assert_eq!(
            d.records.login_listed(PLACE, "codex").await.unwrap(),
            None,
            "already listed"
        );
        assert_eq!(
            d.records.login_listed(PLACE, "claude").await.unwrap(),
            None,
            "claude shares no login file"
        );
        assert_eq!(d.store.writes_to(PLACES).len(), 1);
    });
}

#[test]
fn login_listed_old_daemon_writes_nothing() {
    // spec: R28: a report with no logins list is an older daemon.
    run(async {
        let d = door(|s| {
            s.records = vec![record_json(
                json!({ "report": report(json!({ "logins": null })) }),
            )]
        });
        assert_eq!(d.records.login_listed(PLACE, "codex").await.unwrap(), None);
        assert!(d.store.writes_to(PLACES).is_empty());
    });
}

// ---------------------------------------------------------------- R29-R32: the default mark

#[test]
fn mark_held_drops_gone_row() {
    // spec: R29, R32. TS: copy-on-press.test.ts:292 the default falls back when its row goes.
    run(async {
        for (mark, want) in [
            ("here", Some("here")),
            ("boat", Some("boat")),
            (PLACE, Some(PLACE)),
            ("p_00000000000000ff", None),
        ] {
            let d = door(|s| s.mark = Some(mark.into()));
            assert_eq!(d.records.default_id().await.unwrap().as_deref(), Some(mark));
            assert_eq!(
                d.records.mark_held().await.unwrap().as_deref(),
                want,
                "{mark}"
            );
            assert_eq!(
                d.mark(),
                Some(json!({ "placeId": mark })),
                "the mark itself stays"
            );
        }
        let none = door(|_| {});
        assert_eq!(none.records.mark_held().await.unwrap(), None);
        none.store
            .seed(DEFAULT_COLLECTION, DEFAULT_ID, json!({ "placeId": 7 }));
        assert_eq!(none.records.default_id().await.unwrap(), None);
    });
}

#[test]
fn mark_writes_take_turns() {
    // spec: R30, R31: a check-then-write in the mark's turn never lands over a mark set meanwhile.
    run(async {
        let d = door(|s| s.opts = Box::new(|o| o.store = Rc::new(Yielding(o.store.clone()))));
        let (a, b) = (d.records.clone(), d.records.clone());
        let check = tokio::task::spawn_local(async move {
            let inner = a.clone();
            a.in_turn(Box::pin(async move {
                if inner.default_id().await.unwrap().is_none() {
                    inner
                        .ctx
                        .store
                        .put(DEFAULT_COLLECTION, DEFAULT_ID, json!({ "placeId": "here" }))
                        .await
                        .unwrap();
                }
            }))
            .await
        });
        settle().await;
        let mark = tokio::task::spawn_local(async move { b.mark_default(PLACE).await });
        check.await.unwrap();
        mark.await.unwrap().unwrap();
        assert_eq!(
            d.mark(),
            Some(json!({ "placeId": PLACE })),
            "the later mark landed after the check-then-write"
        );
    });
}

// ---------------------------------------------------------------- R33-R34: the handshake

#[test]
fn challenge_none_without_ephemeral() {
    // spec: R33.
    let key = wsp_store::keys::new_place_key_pair();
    assert!(PlaceRecordsArea::challenge(&key, PLACE, "bm9uY2U=", None).is_none());
    assert!(PlaceRecordsArea::challenge(&key, PLACE, "bm9uY2U=", Some("not a key")).is_none());
}

#[test]
fn challenge_signature_verifies_and_expect_is_place_transcript() {
    // spec: R33. TS: places-link.test.ts proves the host's half of the handshake.
    let key = wsp_store::keys::new_place_key_pair();
    let (_mine, theirs) = fresh_ephemeral();
    let a = PlaceRecordsArea::challenge(&key, PLACE, "dGhlaXItbm9uY2U=", Some(&theirs)).unwrap();
    let b = PlaceRecordsArea::challenge(&key, PLACE, "dGhlaXItbm9uY2U=", Some(&theirs)).unwrap();
    assert_ne!(a.nonce, b.nonce);
    assert_ne!(a.ephemeral, b.ephemeral);
    assert_eq!(a.host_public_key, key.public_key);
    let signed = place_link_transcript(
        LinkRole::Host,
        PLACE,
        "dGhlaXItbm9uY2U=",
        &a.nonce,
        LinkEphemerals {
            challenger: &theirs,
            answerer: &a.ephemeral,
        },
    );
    assert!(verify_place_bytes(&key.public_key, &signed, &a.signature));
    let expect = place_link_transcript(
        LinkRole::Place,
        PLACE,
        &a.nonce,
        "dGhlaXItbm9uY2U=",
        LinkEphemerals {
            challenger: &a.ephemeral,
            answerer: &theirs,
        },
    );
    assert_eq!(a.expect, expect);
}

#[test]
fn signed_refusal_verifies() {
    // spec: R34. TS: places-link.test.ts "a refusal the place can prove".
    let key = wsp_store::keys::new_place_key_pair();
    let r = PlaceRecordsArea::signed_refusal(
        &key,
        PLACE,
        "bm9uY2U=",
        "this host holds no place by that id",
    );
    assert_eq!(js(&r.host_public_key).as_str().unwrap(), key.public_key);
    let bytes = place_refusal_transcript(PLACE, "bm9uY2U=", "this host holds no place by that id");
    assert!(verify_place_bytes(
        &key.public_key,
        &bytes,
        js(&r.signature).as_str().unwrap()
    ));
}

// ---------------------------------------------------------------- restart

#[test]
fn restart_keeps_records_caps_mark_only() {
    // spec: what survives a restart: the store's places, caps and mark; adds, awaiting and limits start empty.
    run(async {
        let d = door(|s| {
            s.caps = vec![(PLACE.into(), json!({ "threads": 2 }))];
            s.mark = Some(PLACE.into());
        });
        d.records
            .adds
            .borrow_mut()
            .insert("a_1".into(), job("a_1", "running"));
        d.records
            .change(PLACE, |now| {
                Some(PlaceRecord {
                    recipe: Some("api".into()),
                    ..now
                })
            })
            .await
            .unwrap();
        let store: Rc<dyn Store> = d.store.clone();
        let docs = d.store.docs.borrow().clone();
        let again = door(move |s| {
            s.records = vec![];
            s.opts = Box::new(move |o| o.store = store);
        });
        assert!(
            again.store.docs.borrow().is_empty(),
            "the second fixture's own store is unused"
        );
        assert_eq!(
            again.records.records().await.unwrap()[0].recipe.as_deref(),
            Some("api")
        );
        assert_eq!(
            js(&again.records.settings_of(PLACE).await.unwrap()),
            json!({ "threads": 2 })
        );
        assert_eq!(
            again.records.default_id().await.unwrap().as_deref(),
            Some(PLACE)
        );
        assert!(again.records.adds.borrow().is_empty());
        assert!(again.records.awaiting.borrow().is_empty());
        assert!(again.records.limits_kept.borrow().is_empty());
        drop(docs);
    });
}
