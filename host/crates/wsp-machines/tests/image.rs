// SPDX-License-Identifier: AGPL-3.0-only
//! image (spec `image.md`, rules I1-I13): the image record, its copies per place, the copy build and the snapshot a
//! fork boots. The TS covers these through whole-host cases (sealed-image, copy-on-press, places-image, places-fork);
//! each test names its rule and the TS line it stands for, and a test named after a TS case stands for a case of
//! those files that reaches this unit and no other.

mod images_support;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt;
use images_support::*;
use serde_json::{json, Value};

use wsp_core::types::api::ImageBuildAsk;
use wsp_core::types::internal::BuilderLife;
use wsp_core::types::pending::Store;
use wsp_engine::errors::EngineError;
use wsp_engine::machine::Size;
use wsp_machines::images::image::{Announce, CopyRow, ImageArea};
use wsp_machines::images::{ImageError, ImageResult, Joined};
use wsp_wire::views::golden_image::{GoldenVersion, SealedImage, SealedImageBuilt};
use wsp_wire::wire::machine_link::{place_builds_no_image_line, vault_unlisted_refusal};
use wsp_wire::wire::place_link::place_went_away_line;
use wsp_words::exit::{refusal, HostError, Kind};
use wsp_words::words::image::{copy_build_fix, image_held_sign_ins_line, image_rebuild_fix};

const H1: &str = "hash-1";
const BUILT_AT: &str = "2026-10-25T09:00:00.000Z";

fn s(x: &str) -> String {
    x.to_string()
}

fn err<T>(r: Result<T, HostError>) -> HostError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected a refusal, got an answer"),
    }
}

fn failed<T>(r: ImageResult<T>) -> ImageError {
    match r {
        Err(e) => e,
        Ok(_) => panic!("expected a failure, got an answer"),
    }
}

/// What a build refused or failed with, as the host error the doors read.
fn refused<T>(r: ImageResult<T>) -> HostError {
    failed(r).into_host()
}

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

// --- the records a test seeds -----------------------------------------------------------------------------------

fn record_json() -> Value {
    json!({
        "name": "default", "version": 1, "hash": H1, "recipeHash": "h1",
        "recipe": {"version": 1, "at": "2026-09-12T00:00:00.000Z", "histories": [], "rows": []},
        "pins": [], "logins": [{"name": "codex", "state": "signed-in"}],
        "sealedAt": "2026-10-25T08:00:00.000Z", "sealedFrom": "host-1",
        "vault": {"sha256": "aa", "bytes": 9, "paths": 1, "held": ["/root/.codex/auth.json"], "takenAt": "2026-10-25T08:00:00.000Z"},
        "place": "boat"
    })
}

fn put_record(w: &World, tweak: impl FnOnce(&mut Value)) -> SealedImage {
    let mut v = record_json();
    tweak(&mut v);
    let image: SealedImage = serde_json::from_value(v).expect("a sealed image");
    w.put_image_now(&image);
    image
}

fn without(v: &mut Value, key: &str) {
    v.as_object_mut().unwrap().remove(key);
}

fn version_with(
    v: u32,
    snap: &str,
    hash: Option<&str>,
    no_secrets: bool,
    template: Option<&str>,
) -> Value {
    let mut j = version_json(v, snap, template);
    if let Some(h) = hash {
        j["imageHash"] = json!(h);
    }
    if no_secrets {
        j["noSecrets"] = json!(true);
    }
    j
}

fn seed_copy(w: &World, place: &str, versions: Vec<Value>, head: u32) {
    w.store.seed(
        "goldens",
        &format!("{place}/default"),
        json!({"head": head, "versions": versions}),
    );
}

fn ask(place: &str) -> ImageBuildAsk {
    ImageBuildAsk {
        place: s(place),
        name: None,
        force: None,
        signal: None,
        starting: None,
    }
}

fn counting() -> (Rc<Cell<u32>>, Rc<dyn Fn()>) {
    let n = Rc::new(Cell::new(0));
    let c = n.clone();
    (n, Rc::new(move || c.set(c.get() + 1)))
}

fn announcing(w: &World) -> Announce {
    let log = w.log.clone();
    Rc::new(move |place: &str, rate: f64| log.borrow_mut().push(format!("announce {place} {rate}")))
}

fn at(w: &World, line: &str) -> usize {
    w.log()
        .iter()
        .position(|l| l == line || l.starts_with(line))
        .unwrap_or_else(|| panic!("no log line {line}: {:?}", w.log()))
}

/// One build the fake golden area and the fake seal will answer: the builder it prepares and the copy its seal files.
struct Built {
    id: &'static str,
    place: &'static str,
    snap: &'static str,
    version: u32,
    hash: &'static str,
    writes: bool,
}

fn built(
    id: &'static str,
    place: &'static str,
    snap: &'static str,
    version: u32,
    hash: &'static str,
) -> Built {
    Built {
        id,
        place,
        snap,
        version,
        hash,
        writes: true,
    }
}

fn script_builds(w: &Rc<World>, builds: Vec<Built>) {
    let mut queue = VecDeque::new();
    for b in builds {
        let entry = live_builder(b.id, "default", b.place, BuilderLife::Own);
        w.add_builder(entry.clone());
        w.prepared_views.borrow_mut().push_back(Ok(view_of(&entry)));
        w.seals
            .borrow_mut()
            .push_back(Ok(seal_result(b.version, b.snap, false)));
        queue.push_back(b);
    }
    let queue = RefCell::new(queue);
    let w2 = w.clone();
    *w.seal_hook.borrow_mut() = Some(Rc::new(move || {
        let Some(b) = queue.borrow_mut().pop_front() else {
            return;
        };
        if !b.writes {
            return;
        }
        let key = format!("{}/default", b.place);
        let mut doc = w2
            .store
            .doc("goldens", &key)
            .unwrap_or(json!({"head": 0, "versions": []}));
        doc["versions"].as_array_mut().unwrap().push(version_with(
            b.version,
            b.snap,
            Some(b.hash),
            false,
            None,
        ));
        doc["head"] = json!(b.version);
        w2.store.seed("goldens", &key, doc);
    }));
}

/// A host with the image sealed at the provider this host forks on (`boat`, which holds the image's own copy), two
/// joined computers behind the door (`p_lab` builds copies, `p_idle` forks nothing), the record's vault on the store
/// and a copy recipe that logs each time it is composed.
struct Scene {
    w: Rc<World>,
    a: Rc<ImageArea>,
}

fn scene() -> Scene {
    let w = World::new();
    let door = w.with_door();
    door.computer("p_lab", "lab", Some(FakeBackend::new("lab")));
    door.computer("p_idle", "idle", Some(FakeBackend::forkless("idle")));
    put_record(&w, |_| {});
    w.store
        .seed_blob("image-vaults", "default@v1", b"TAR-BYTES");
    seed_copy(
        &w,
        "boat",
        vec![version_with(1, "snap-own", Some(H1), true, None)],
        1,
    );
    let log = w.log.clone();
    w.opts.borrow_mut().copy_recipe = Some(Rc::new(move |image, cloud| {
        log.borrow_mut()
            .push(format!("copy_recipe {} cloud={cloud}", image.hash));
        async { Ok(recipe("copy")) }.boxed_local()
    }));
    let a = ImageArea::new(w.clone());
    Scene { w, a }
}

fn lines_of(w: &World, prefix: &str) -> Vec<String> {
    w.lines(prefix)
}

// ---------------------------------------------------------------------------------------------------------------
// I1 to I5: sizes, copies, the record and its vault
// ---------------------------------------------------------------------------------------------------------------

// I1: image.ts:37-41. A row is kept only where its id was asked and it carries `restoredBytes`; no ids or no listing ask nothing.
#[test]
fn copies_carry_the_restored_size_only_where_the_listing_says_it() {
    run(async {
        let w = World::new();
        let a = ImageArea::new(w.clone());
        let boat = w.runtime_backend.clone();
        *boat.snapshots.borrow_mut() = Ok(vec![
            snapshot_row("s1", None, 10.0, Some(4.0)),
            snapshot_row("s2", None, 10.0, None),
            snapshot_row("other", None, 1.0, Some(9.0)),
        ]);
        let sizes = a.snapshot_sizes(boat.clone(), &[s("s1"), s("s2")]).await;
        assert_eq!(
            sizes
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect::<Vec<_>>(),
            vec![(s("s1"), 4.0)]
        );
        assert_eq!(
            boat.listed.borrow().clone(),
            vec![Some(vec![s("s1"), s("s2")])]
        );

        assert!(a.snapshot_sizes(boat.clone(), &[]).await.is_empty());
        assert_eq!(boat.listed.borrow().len(), 1, "no ids, no listing asked");
        let idle = FakeBackend::forkless("idle");
        assert!(a.snapshot_sizes(idle.clone(), &[s("s1")]).await.is_empty());
        assert!(
            idle.listed.borrow().is_empty(),
            "a backend with no listing is not asked"
        );

        seed_copy(
            &w,
            "boat",
            vec![version_with(1, "s1", Some(H1), false, None)],
            1,
        );
        let copies = a.copies_of("default").await.unwrap();
        assert_eq!(to_json(&copies)[0]["sizeBytes"], json!(4));
        seed_copy(
            &w,
            "boat",
            vec![version_with(1, "s2", Some(H1), false, None)],
            1,
        );
        let copies = a.copies_of("default").await.unwrap();
        assert!(
            to_json(&copies)[0].get("sizeBytes").is_none(),
            "a snapshot the provider reports only the stored size of has none"
        );
    });
}

// I1: image.ts:39. A listing that fails answers empty and the copy is still listed.
#[test]
fn a_listing_that_fails_leaves_sizes_out() {
    run(async {
        let w = World::new();
        let a = ImageArea::new(w.clone());
        *w.runtime_backend.snapshots.borrow_mut() = Err(missing("the provider did not answer"));
        assert!(a
            .snapshot_sizes(w.runtime_backend.clone(), &[s("s1")])
            .await
            .is_empty());
        seed_copy(
            &w,
            "boat",
            vec![version_with(1, "s1", Some(H1), false, None)],
            1,
        );
        let copies = to_json(&a.copies_of("default").await.unwrap());
        assert_eq!(copies.as_array().unwrap().len(), 1);
        assert_eq!(copies[0]["snapshotId"], json!("s1"));
        assert!(copies[0].get("sizeBytes").is_none());
    });
}

// I2: image.ts:44-67. Each place's head, the wired provider first and the rest by the root collation of the place.
#[test]
fn copies_list_each_places_head_wired_first_then_by_place() {
    run(async {
        let w = World::new();
        let door = w.with_door();
        let alpha = FakeBackend::new("alpha");
        *alpha.snapshots.borrow_mut() = Ok(vec![snapshot_row("snap-a", None, 9.0, Some(7.0))]);
        for id in ["alpha", "Beta", "zed"] {
            door.computer(
                id,
                id,
                Some(if id == "alpha" {
                    alpha.clone()
                } else {
                    FakeBackend::new(id)
                }),
            );
        }
        seed_copy(
            &w,
            "zed",
            vec![version_with(1, "snap-z", None, false, None)],
            1,
        );
        seed_copy(
            &w,
            "boat",
            vec![
                version_with(1, "b1", Some("old"), false, None),
                version_with(2, "b2", Some(H1), false, Some("tpl-b")),
            ],
            2,
        );
        seed_copy(
            &w,
            "alpha",
            vec![version_with(1, "snap-a", Some(H1), false, None)],
            1,
        );
        seed_copy(
            &w,
            "Beta",
            vec![version_with(3, "snap-b", Some(H1), false, None)],
            3,
        );
        w.store.seed(
            "goldens",
            "alpha/other",
            json!({"head": 1, "versions": [version_with(1, "snap-o", None, false, None)]}),
        );
        let a = ImageArea::new(w.clone());
        let copies = to_json(&a.copies_of("default").await.unwrap());
        let shape: Vec<Value> = copies
            .as_array()
            .unwrap()
            .iter()
            .map(|c| json!([c["place"], c["version"], c["snapshotId"]]))
            .collect();
        assert_eq!(
            shape,
            vec![
                json!(["boat", 2, "b2"]),
                json!(["alpha", 1, "snap-a"]),
                json!(["Beta", 3, "snap-b"]),
                json!(["zed", 1, "snap-z"])
            ]
        );
        assert_eq!(
            copies[0],
            json!({"place": "boat", "version": 2, "hash": H1, "snapshotId": "b2", "templateId": "tpl-b", "builtAt": BUILT_AT})
        );
        assert_eq!(
            copies[0].as_object().unwrap().keys().collect::<Vec<_>>(),
            [
                "place",
                "version",
                "hash",
                "snapshotId",
                "templateId",
                "builtAt"
            ]
        );
        assert_eq!(
            copies[1]["sizeBytes"],
            json!(7),
            "the size is asked of the place's own backend, the door's included"
        );
        assert!(
            copies[3].get("hash").is_none(),
            "a copy built before hashes were kept carries none"
        );

        // A key naming no place is the wired provider's.
        let w = World::new();
        w.store.seed(
            "goldens",
            "default",
            json!({"head": 1, "versions": [version_with(1, "snap-old", None, false, None)]}),
        );
        let copies = to_json(
            &ImageArea::new(w.clone())
                .copies_of("default")
                .await
                .unwrap(),
        );
        assert_eq!(copies[0]["place"], json!("boat"));
    });
}

// I2: image.ts:56. A manifest with no head version is no copy.
#[test]
fn copies_skip_a_manifest_with_no_head() {
    run(async {
        let w = World::new();
        seed_copy(
            &w,
            "boat",
            vec![
                version_with(1, "a", None, false, None),
                version_with(2, "b", None, false, None),
            ],
            3,
        );
        seed_copy(&w, "solari", vec![], 1);
        seed_copy(&w, "box", vec![version_with(1, "c", None, false, None)], 1);
        let copies = to_json(
            &ImageArea::new(w.clone())
                .copies_of("default")
                .await
                .unwrap(),
        );
        let places: Vec<&Value> = copies
            .as_array()
            .unwrap()
            .iter()
            .map(|c| &c["place"])
            .collect();
        assert_eq!(places, vec![&json!("box")]);
    });
}

// I3: image.ts:69.
#[test]
fn conflict_is_kind_conflict() {
    let w = World::new();
    let e = ImageArea::new(w.clone()).conflict("something is held");
    assert_eq!(e.kind, Some(Kind::Conflict));
    assert_eq!(e.message, "something is held");
    assert!(e.fix.is_none());
}

// I4: image.ts:72-79; sealed-image.test.ts:188. No record answers null and no copies; the name defaults to `default`.
#[test]
fn get_answers_null_and_no_copies_for_an_unknown_image() {
    run(async {
        let w = World::new();
        let a = ImageArea::new(w.clone());
        let view = to_json(&a.get(None).await.unwrap());
        assert_eq!(view, json!({"image": null, "copies": []}));
        assert_eq!(
            to_json(&a.get(Some("other")).await.unwrap())["image"],
            Value::Null
        );
        assert_eq!(w.log(), ["ready", "ready"]);

        let record = put_record(&w, |_| {});
        seed_copy(
            &w,
            "boat",
            vec![version_with(1, "snap-own", Some(H1), true, None)],
            1,
        );
        let view = to_json(&a.get(None).await.unwrap());
        assert_eq!(view["image"], to_json(&record));
        assert_eq!(view["copies"][0]["snapshotId"], json!("snap-own"));
        assert_eq!(
            to_json(&a.get(Some("other")).await.unwrap())["copies"],
            json!([])
        );
    });
}

// I5: image.ts:81-90. The three refusals in order, each a conflict, then the record and its archive.
#[test]
fn vault_refuses_in_order_and_answers_the_blob() {
    run(async {
        let w = World::new();
        let a = ImageArea::new(w.clone());
        let e = err(a.vault(None).await);
        assert_eq!(
            (e.kind, e.message.as_str()),
            (
                Some(Kind::Conflict),
                "this host owns no image named default yet"
            )
        );
        let e = err(a.vault(Some("other")).await);
        assert_eq!(e.message, "this host owns no image named other yet");

        put_record(&w, |v| without(v, "vault"));
        let e = err(a.vault(None).await);
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, "default v1 was sealed before its sign-ins were held, so there is nothing to export; cut the next version to hold them");

        let record = put_record(&w, |_| {});
        let e = err(a.vault(None).await);
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, "the vault of default v1 is not on this computer any more; cut the next version to take it again");

        w.store
            .seed_blob("image-vaults", "default@v1", b"TAR-BYTES");
        let (image, tar) = a.vault(Some("default")).await.unwrap();
        assert_eq!(to_json(&image), to_json(&record));
        assert_eq!(tar, b"TAR-BYTES");
    });
}

// ---------------------------------------------------------------------------------------------------------------
// I6 to I9: the build and its joins
// ---------------------------------------------------------------------------------------------------------------

// I6: image.ts:92-103; sealed-image.test.ts:422, 386, 534. Each refusal in the TS order, with its fix apart.
#[test]
fn build_refuses_in_the_ts_order_with_each_fix() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let no_image = refusal(
            "this host owns no image named default yet",
            copy_build_fix::NO_IMAGE,
            Some(Kind::Conflict),
        );
        let no_copy = refusal(
            &place_builds_no_image_line("idle"),
            copy_build_fix::NO_COPY,
            Some(Kind::Conflict),
        );
        let (started, starting) = counting();

        // A place that builds nothing is refused before the record is read, with or without one.
        w.store.delete("images", "default").await.unwrap();
        let e = refused(
            a.build(ImageBuildAsk {
                starting: Some(starting.clone()),
                ..ask("p_idle")
            })
            .await,
        );
        assert_eq!(e, no_copy);
        assert_eq!(e.fix.as_deref(), Some(copy_build_fix::NO_COPY));
        assert_eq!(
            e.message,
            format!(
                "{}. {}",
                place_builds_no_image_line("idle"),
                copy_build_fix::NO_COPY
            )
        );

        // Then no record at all.
        let e = refused(
            a.build(ImageBuildAsk {
                starting: Some(starting.clone()),
                ..ask("p_lab")
            })
            .await,
        );
        assert_eq!(e, no_image);
        let e = refused(
            a.build(ImageBuildAsk {
                name: Some(s("other")),
                ..ask("p_lab")
            })
            .await,
        );
        assert_eq!(
            e.message,
            refusal(
                "this host owns no image named other yet",
                copy_build_fix::NO_IMAGE,
                Some(Kind::Conflict)
            )
            .message
        );

        // A record with no recipe, which also holds no vault, is refused for the recipe first.
        put_record(w, |v| {
            without(v, "recipe");
            without(v, "vault");
        });
        let e = refused(
            a.build(ImageBuildAsk {
                starting: Some(starting.clone()),
                ..ask("p_lab")
            })
            .await,
        );
        assert_eq!(
            e,
            refusal(
                "default v1 was sealed before the image record kept the recipe it was built from, so no other place can build it",
                copy_build_fix::NO_RECIPE,
                Some(Kind::Conflict)
            )
        );

        // A record with a recipe and no vault would ask for every sign-in again.
        put_record(w, |v| without(v, "vault"));
        let e = refused(
            a.build(ImageBuildAsk {
                starting: Some(starting.clone()),
                ..ask("p_lab")
            })
            .await,
        );
        let line =
            "default v1 holds no sign-ins, so a copy at lab would ask for every one of them again";
        assert_eq!(
            e,
            refusal(line, copy_build_fix::NO_VAULT, Some(Kind::Conflict))
        );
        assert!(
            !e.message
                .trim_end_matches(copy_build_fix::NO_VAULT)
                .contains("--force"),
            "what happened names no flag; the fix does"
        );
        assert!(copy_build_fix::NO_VAULT.contains("--force"));

        assert_eq!(started.get(), 0, "no refusal calls starting");
        assert!(lines_of(w, "golden_prepare").is_empty() && lines_of(w, "copy_recipe").is_empty());
        assert!(w.copy_builds.borrow().is_empty());
    });
}

// I6: image.ts:99; sealed-image.test.ts:402. Forced, a record with no sign-ins builds, and nothing of a vault lands.
#[test]
fn build_forced_passes_a_record_without_sign_ins() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        put_record(w, |v| without(v, "vault"));
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        let done = a
            .build(ImageBuildAsk {
                force: Some(true),
                ..ask("p_lab")
            })
            .await
            .unwrap();
        assert_eq!(
            to_json(&done),
            json!({"copy": {"place": "p_lab", "version": 1, "hash": H1, "snapshotId": "snap-new", "builtAt": BUILT_AT}, "built": true})
        );
        assert!(lines_of(w, "golden_prepare").len() == 1);
        assert!(
            lines_of(w, "road import_image_vault").is_empty()
                && lines_of(w, "stage uploading-files").is_empty(),
            "no sign-ins were landed"
        );
        assert!(lines_of(w, "seal_entry")[0].ends_with("logins=1 copy=hash-1"));
    });
}

// I7: image.ts:104-110; sealed-image.test.ts:311. A place whose head is current answers its standing copy and builds nothing.
#[test]
fn build_answers_a_current_copy_unbuilt() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        seed_copy(
            w,
            "p_lab",
            vec![version_with(4, "snap-lab", Some(H1), false, None)],
            4,
        );
        let (started, starting) = counting();
        let done = a
            .build(ImageBuildAsk {
                starting: Some(starting),
                ..ask("p_lab")
            })
            .await
            .unwrap();
        assert_eq!(
            to_json(&done),
            json!({"copy": {"place": "p_lab", "version": 4, "hash": H1, "snapshotId": "snap-lab", "builtAt": BUILT_AT}, "built": false})
        );
        assert_eq!(started.get(), 0);
        assert!(lines_of(w, "golden_prepare").is_empty() && lines_of(w, "copy_recipe").is_empty());
        assert!(w.copy_builds.borrow().is_empty());
        // A copy at an older hash is not current: it builds.
        seed_copy(
            w,
            "p_lab",
            vec![version_with(4, "snap-lab", Some("old"), false, None)],
            4,
        );
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 5, H1)]);
        assert_eq!(
            to_json(&a.build(ask("p_lab")).await.unwrap())["built"],
            json!(true)
        );
    });
}

// I7: image.ts:111. A head current for the place but no copy listed there fails in the TS's plain words.
#[test]
fn build_fails_when_a_current_head_has_no_recorded_copy() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        // A place id with a "/" in it files its copy under a key the listing reads as another place's.
        w.door
            .borrow()
            .as_ref()
            .unwrap()
            .computer("x/y", "lab", Some(FakeBackend::new("xy")));
        seed_copy(
            w,
            "x/y",
            vec![version_with(2, "snap-xy", Some(H1), false, None)],
            2,
        );
        let e = failed(a.build(ask("x/y")).await);
        assert_eq!(
            e.message(),
            "lab holds default v2 and no copy of it was recorded there"
        );
        assert_eq!(e.kind(), None);
    });
}

// I8: image.ts:116-129; copy-on-press.test.ts:127 (a create joins the build). A second caller joins the build in
// flight, starting runs for each, and the build is in `copy_builds` until it settles.
#[test]
fn build_joins_one_in_flight_and_calls_starting_each_time() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        *w.copy_gate.borrow_mut() = Some(
            async move {
                let _ = rx.await;
            }
            .boxed_local()
            .shared(),
        );
        let (n1, s1) = counting();
        let (n2, s2) = counting();
        let first = a.build(ImageBuildAsk {
            starting: Some(s1),
            ..ask("p_lab")
        });
        let second = a.build(ImageBuildAsk {
            starting: Some(s2),
            ..ask("p_lab")
        });
        let release = async {
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            assert!(
                w.copy_builds.borrow().contains_key("p_lab/default"),
                "in flight until it settles"
            );
            tx.send(()).unwrap();
        };
        let (a1, a2, _) = futures::join!(first, second, release);
        let (a1, a2) = (to_json(&a1.unwrap()), to_json(&a2.unwrap()));
        assert_eq!(a1, a2, "the second answers the first's own build");
        assert_eq!(a1["built"], json!(true));
        assert_eq!((n1.get(), n2.get()), (1, 1));
        assert_eq!(lines_of(w, "golden_prepare").len(), 1);
        assert_eq!(lines_of(w, "seal_entry").len(), 1);
        assert!(w.copy_builds.borrow().is_empty());

        // A build already in `copy_builds` is answered whole, and starting still runs.
        let copy = a1["copy"].clone();
        let standing: Joined<SealedImageBuilt> = {
            let answer: SealedImageBuilt =
                serde_json::from_value(json!({"copy": copy, "built": true})).unwrap();
            async move { Ok(answer) }.boxed_local().shared()
        };
        w.copy_builds
            .borrow_mut()
            .insert(s("p_lab/default"), standing);
        seed_copy(
            w,
            "p_lab",
            vec![version_with(1, "snap-new", Some("old"), false, None)],
            1,
        );
        let (n3, s3) = counting();
        let joined = a
            .build(ImageBuildAsk {
                starting: Some(s3),
                ..ask("p_lab")
            })
            .await
            .unwrap();
        assert_eq!(to_json(&joined), a1);
        assert_eq!(n3.get(), 1);
        assert_eq!(lines_of(w, "golden_prepare").len(), 1);
    });
}

fn row(stopped: bool) -> Rc<CopyRow> {
    Rc::new(CopyRow { stopped })
}

// I8: image.ts:121-123; places-image.test.ts:223. A build that fails while the place's row stands and is not stopped
// frames the stop with the place-went-away line where the place is away or the engine says it is absent.
#[test]
fn a_failed_build_frames_the_place_went_away() {
    run(async {
        for how in ["away", "absent"] {
            let sc = scene();
            let (w, a) = (&sc.w, &sc.a);
            w.copy_rows.borrow_mut().insert(s("p_lab"), row(false));
            let cause = if how == "away" {
                w.away.borrow_mut().insert(s("p_lab"));
                ImageError::Host(HostError::new("the link dropped"))
            } else {
                ImageError::Engine(EngineError::PlaceAbsent {
                    message: s("lab is not connected"),
                })
            };
            w.prepared_views.borrow_mut().push_back(Err(cause));
            let e = failed(a.build(ask("p_lab")).await);
            assert!(!e.message().is_empty());
            assert_eq!(
                w.framed.borrow().clone(),
                vec![(s("p_lab"), s("default"), place_went_away_line("lab"))],
                "{how}"
            );
            assert_eq!(
                w.framed.borrow()[0].2,
                "lab went away before the build finished"
            );
            assert!(
                w.copy_builds.borrow().is_empty(),
                "a failed build leaves copy_builds"
            );
        }
    });
}

// I8: image.ts:121-122; copy-on-press.test.ts:149, 216. While the row stands the build frames its own error; with no row
// or a row already stopped it frames nothing, and the error is the build's either way.
#[test]
fn a_failed_build_frames_its_own_error_while_the_row_stands() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        w.copy_rows.borrow_mut().insert(s("p_lab"), row(false));
        w.prepared_views
            .borrow_mut()
            .push_back(Err(ImageError::Host(HostError::new(
                "no room at lab today",
            ))));
        let e = failed(a.build(ask("p_lab")).await);
        assert_eq!(e.message(), "no room at lab today");
        let framed = w.framed.borrow().clone();
        assert_eq!(framed.len(), 1);
        assert_eq!(
            (framed[0].0.as_str(), framed[0].1.as_str()),
            ("p_lab", "default")
        );
        assert!(
            framed[0].2.starts_with("error: ") && framed[0].2.contains("no room at lab today"),
            "{}",
            framed[0].2
        );

        w.framed.borrow_mut().clear();
        w.copy_rows.borrow_mut().insert(s("p_lab"), row(true));
        w.prepared_views
            .borrow_mut()
            .push_back(Err(ImageError::Host(HostError::new(
                "no room at lab today",
            ))));
        failed(a.build(ask("p_lab")).await);
        w.copy_rows.borrow_mut().clear();
        w.prepared_views
            .borrow_mut()
            .push_back(Err(ImageError::Host(HostError::new(
                "no room at lab today",
            ))));
        failed(a.build(ask("p_lab")).await);
        assert!(w.framed.borrow().is_empty());
    });
}

// I9: image.ts:132-158; copy-on-press.test.ts:169. Builds until the copy it built is current against the record as it
// stands after: a cut that landed mid-build leaves a stale copy, built once more.
#[test]
fn keep_current_builds_until_current() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(
            w,
            vec![
                built("b-1", "p_lab", "snap-1", 1, H1),
                built("b-2", "p_lab", "snap-2", 2, "hash-2"),
            ],
        );
        // The first seal files a copy at the hash it was built from, and the cut to the next record lands under it.
        let inner = w.seal_hook.borrow().clone().unwrap();
        let cut = Rc::new(Cell::new(false));
        let w2 = w.clone();
        *w.seal_hook.borrow_mut() = Some(Rc::new(move || {
            inner();
            if !cut.replace(true) {
                put_record(&w2, |v| v["hash"] = json!("hash-2"));
            }
        }));
        a.keep_current("p_lab", None).await;
        assert_eq!(lines_of(w, "golden_prepare").len(), 2);
        let seals = lines_of(w, "seal_entry");
        assert!(
            seals[0].ends_with("copy=hash-1") && seals[1].ends_with("copy=hash-2"),
            "{seals:?}"
        );
        let hashes: Vec<Value> = w
            .copy_now("p_lab", "default")
            .unwrap()
            .versions
            .iter()
            .map(|v| json!(v.image_hash))
            .collect();
        assert_eq!(
            hashes,
            vec![json!(H1), json!("hash-2")],
            "two builds, not one relabelled"
        );
        assert!(w.framed.borrow().is_empty());
    });
}

// I9: image.ts:136-142; places-image.test.ts:268. A place that is away owes nothing now, and neither does one that
// builds no copy at all.
#[test]
fn keep_current_owes_nothing_for_an_away_place() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        w.away.borrow_mut().insert(s("p_lab"));
        a.keep_current("p_lab", None).await;
        assert_eq!(w.log(), ["ready"]);
        a.keep_current("p_idle", None).await;
        assert_eq!(w.log(), ["ready", "ready", "place_at p_idle"]);
        assert!(w.framed.borrow().is_empty());
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

// I9: image.ts:152-156; copy-on-press.test.ts:149. A failure before any build is framed by the caller with the error.
#[test]
fn keep_current_frames_a_refusal_before_any_build() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        a.keep_current("nowhere", None).await;
        let framed = w.framed.borrow().clone();
        assert_eq!(framed.len(), 1);
        assert_eq!(
            (framed[0].0.as_str(), framed[0].1.as_str()),
            ("nowhere", "default")
        );
        assert!(
            framed[0].2.starts_with("error: ") && framed[0].2.contains("no place nowhere"),
            "{}",
            framed[0].2
        );
        assert!(lines_of(w, "row_says_failure").contains(&s("row_says_failure nowhere")));
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

// I9: image.ts:150-152; copy-on-press.test.ts:216. Where the row changed under the call and now reads stopped, the
// build said it and nothing more is framed.
#[test]
fn keep_current_leaves_a_stop_the_build_framed() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        w.copy_rows.borrow_mut().insert(s("p_lab"), row(false));
        w.prepared_views
            .borrow_mut()
            .push_back(Err(ImageError::Host(HostError::new(
                "no room at lab today",
            ))));
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        *w.copy_gate.borrow_mut() = Some(
            async move {
                let _ = rx.await;
            }
            .boxed_local()
            .shared(),
        );
        let keeping = a.keep_current("p_lab", None);
        let stop = async {
            tokio::task::yield_now().await;
            // The row the build framed over: a new one, stopped.
            w.copy_rows.borrow_mut().insert(s("p_lab"), row(true));
            tx.send(()).unwrap();
        };
        futures::join!(keeping, stop);
        assert!(w.framed.borrow().is_empty());
        assert!(
            lines_of(w, "row_says_failure").is_empty(),
            "the build has said it, so the row is not asked"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// I10, I11: the copy build
// ---------------------------------------------------------------------------------------------------------------

// I10: image.ts:176-195; sealed-image.test.ts:275. At a computer the record's archive is checked, composed for, landed on
// the builder after the recipe, and only then sealed under the record's hash.
#[test]
fn the_copy_build_lands_the_vault_before_the_seal() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        let done = a.build(ask("p_lab")).await.unwrap();
        let uploading = format!(
            "stage uploading-files the image's sign-ins, {}",
            wsp_words::words::units::fmt_bytes(9.0)
        );
        assert_eq!(
            w.log(),
            vec![
                s("ready"),
                s("place_at p_lab"),
                s("road refuse_foreign_members import /root/.codex/auth.json"),
                s(&format!("copy_recipe {H1} cloud=false")),
                s("golden_prepare default p_lab copy=true"),
                s("stage_of default p_lab true"),
                uploading,
                s("road import_image_vault b-copy 9"),
                s(&format!("seal_entry b-copy keep=false logins=1 copy={H1}")),
            ]
        );
        assert_eq!(
            to_json(&done),
            json!({"copy": {"place": "p_lab", "version": 1, "hash": H1, "snapshotId": "snap-new", "builtAt": BUILT_AT}, "built": true})
        );
    });
}

// I10: image.ts:172. At a provider the vault is never landed, whatever the record holds, and no archive is read.
#[test]
fn the_copy_build_lands_no_vault_at_a_provider() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        w.places
            .rows
            .borrow_mut()
            .insert(s("solari"), FakeBackend::new("solari"));
        *w.roads.foreign.borrow_mut() = Some(s("etc/cron.d/x is not a member the seal asked for"));
        script_builds(w, vec![built("b-copy", "solari", "snap-sol", 1, H1)]);
        let done = a.build(ask("solari")).await.unwrap();
        assert_eq!(to_json(&done)["built"], json!(true));
        assert_eq!(
            w.log(),
            vec![
                s("ready"),
                s("place_at solari"),
                s(&format!("copy_recipe {H1} cloud=true")),
                s("golden_prepare default solari copy=true"),
                s("stage_of default solari true"),
                s(&format!("seal_entry b-copy keep=false logins=1 copy={H1}")),
            ]
        );
    });
}

// I10: image.ts:173-175; sealed-image.test.ts:494. A vault with no path list is refused in its own sentence, never the
// sentence for a blob this computer lost, and before anything is composed.
#[test]
fn the_copy_build_refuses_an_unlisted_vault_before_reading_the_blob() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        put_record(w, |v| without(&mut v["vault"], "held"));
        w.store
            .delete_blob("image-vaults", "default@v1")
            .await
            .unwrap();
        let e = refused(a.build(ask("p_lab")).await);
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, vault_unlisted_refusal("default", 1));
        assert!(
            lines_of(w, "copy_recipe").is_empty()
                && lines_of(w, "golden_prepare").is_empty()
                && lines_of(w, "road").is_empty()
        );
    });
}

// I10: image.ts:176-183; sealed-image.test.ts:482. An archive carrying a member the seal never asked for boots no
// builder: the refusal is a conflict in the road's own words.
#[test]
fn the_copy_build_refuses_foreign_members() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let said = "the image's sign-in archive is refused at etc/cron.d/x: it is not under a path the seal asked for; nothing of it was imported";
        *w.roads.foreign.borrow_mut() = Some(s(said));
        let e = refused(a.build(ask("p_lab")).await);
        assert_eq!((e.kind, e.message.as_str()), (Some(Kind::Conflict), said));
        assert_eq!(
            lines_of(w, "road"),
            ["road refuse_foreign_members import /root/.codex/auth.json"]
        );
        assert!(lines_of(w, "copy_recipe").is_empty() && lines_of(w, "golden_prepare").is_empty());
    });
}

// I10: image.ts:174; sealed-image.test.ts:504. A blob the computer lost is its own conflict.
#[test]
fn a_blob_this_computer_lost_is_its_own_conflict() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        w.store
            .delete_blob("image-vaults", "default@v1")
            .await
            .unwrap();
        let e = refused(a.build(ask("p_lab")).await);
        assert_eq!(e.kind, Some(Kind::Conflict));
        assert_eq!(e.message, "the vault of default v1 is not on this computer any more; cut the next version to take it again");
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

// I10: image.ts:189-195. A landing that fails kills the builder (its own failure ignored), forgets it if gone, stages
// failed with the message, and fails with the error; nothing is sealed.
#[test]
fn a_failed_vault_landing_kills_forgets_and_stages_failed() {
    run(async {
        for kill_fails in [false, true] {
            let sc = scene();
            let (w, a) = (&sc.w, &sc.a);
            script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
            *w.roads.vault_import_fails.borrow_mut() = Some(missing("the upload was refused"));
            if kill_fails {
                *w.roads.kill_fails.borrow_mut() = Some(missing("already gone"));
            }
            let e = failed(a.build(ask("p_lab")).await);
            assert_eq!(e.message(), "the upload was refused");
            let tail: Vec<String> = w
                .log()
                .into_iter()
                .skip_while(|l| !l.starts_with("road import_image_vault"))
                .collect();
            assert_eq!(
                tail,
                vec![
                    s("road import_image_vault b-copy 9"),
                    s("road kill_until_gone b-copy"),
                    s("forget_if_gone b-copy"),
                    s("stage failed the upload was refused")
                ],
                "kill failing: {kill_fails}"
            );
            assert!(lines_of(w, "seal_entry").is_empty());
        }
    });
}

// I10: image.ts:184-185. A builder the map does not hold after the prepare left no record here, and nothing is sealed.
#[test]
fn a_prepared_builder_the_map_does_not_hold_left_no_record_here() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let stray = live_builder("b-stray", "default", "p_lab", BuilderLife::Own);
        w.prepared_views.borrow_mut().push_back(Ok(view_of(&stray)));
        let e = failed(a.build(ask("p_lab")).await);
        assert_eq!(
            e.message(),
            "the builder b-stray prepared at lab left no record here; nothing was sealed"
        );
        assert_eq!(e.kind(), None);
        assert!(lines_of(w, "seal_entry").is_empty() && lines_of(w, "road").len() == 1);
    });
}

// I11: image.ts:196-204; sealed-image.test.ts:342. A copy at the image's own place keeps the digest that place's head was
// planned from, filed under the new version; at any other place none is filed.
#[test]
fn the_copy_at_the_images_own_place_keeps_the_planned_digest() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let signed_in = json!({"ticks": [{"id": "logins/gh", "choice": "copy"}], "files": []});
        w.store
            .seed("golden-recipes", "boat/default@v1", signed_in.clone());
        w.store
            .seed("golden-recipes", "p_lab/default@v1", signed_in.clone());
        seed_copy(
            w,
            "p_lab",
            vec![version_with(1, "snap-lab", Some("old"), false, None)],
            1,
        );
        script_builds(
            w,
            vec![
                built("b-boat", "boat", "snap-boat-2", 2, H1),
                built("b-lab", "p_lab", "snap-lab-2", 2, H1),
            ],
        );

        let done = a
            .build(ImageBuildAsk {
                force: Some(true),
                ..ask("boat")
            })
            .await
            .unwrap();
        assert_eq!(to_json(&done)["copy"]["version"], json!(2));
        assert_eq!(
            w.store.doc("golden-recipes", "boat/default@v2"),
            Some(signed_in.clone())
        );
        assert_eq!(
            lines_of(w, "put_copy_recipe"),
            ["put_copy_recipe boat default 2"]
        );

        let done = a.build(ask("p_lab")).await.unwrap();
        assert_eq!(to_json(&done)["copy"]["version"], json!(2));
        assert!(
            w.store.doc("golden-recipes", "p_lab/default@v2").is_none(),
            "a copy elsewhere files no digest"
        );
        assert_eq!(lines_of(w, "put_copy_recipe").len(), 1);
    });
}

// I11: image.ts:205. A seal that left no copy at the place fails in the TS's plain words.
#[test]
fn the_copy_build_fails_with_no_recorded_copy_after_the_seal() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(
            w,
            vec![Built {
                writes: false,
                ..built("b-copy", "p_lab", "snap-new", 1, H1)
            }],
        );
        let e = failed(a.build(ask("p_lab")).await);
        assert_eq!(
            e.message(),
            "lab sealed default v1 and no copy of it was recorded there"
        );
        assert_eq!(e.kind(), None);
    });
}

// ---------------------------------------------------------------------------------------------------------------
// I12, I13: the snapshot a fork boots
// ---------------------------------------------------------------------------------------------------------------

// I12: image.ts:210-216, 255-256. The snapshot of the image's own place, or a fork landing there, is answered as asked.
#[test]
fn copy_for_fork_answers_the_snapshot_at_the_images_own_place() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        assert_eq!(
            a.image_head_named("snap-own").await.unwrap(),
            Some(s("default"))
        );
        assert_eq!(a.image_head_named("snap-other").await.unwrap(), None);
        assert_eq!(
            a.copy_for_fork("snap-own", None, None).await.ok().unwrap(),
            "snap-own",
            "no place is the wired provider, the image's own"
        );
        assert_eq!(
            a.copy_for_fork("snap-own", Some("boat"), None)
                .await
                .ok()
                .unwrap(),
            "snap-own"
        );
        assert_eq!(
            a.copy_for_fork("snap-unknown", Some("p_lab"), None)
                .await
                .ok()
                .unwrap(),
            "snap-unknown",
            "no image has this as its head"
        );
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

// I12: image.ts:257-259; copy-on-press.test.ts:421. Without announce a build in flight there is awaited and its copy
// answered, its failure read as none.
#[test]
fn copy_for_fork_waits_for_a_build_in_flight() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let copy = json!({"place": "p_lab", "version": 1, "hash": H1, "snapshotId": "snap-lab", "builtAt": BUILT_AT});
        let ok: SealedImageBuilt =
            serde_json::from_value(json!({"copy": copy, "built": true})).unwrap();
        w.copy_builds.borrow_mut().insert(
            s("p_lab/default"),
            async move { Ok(ok) }.boxed_local().shared(),
        );
        assert_eq!(
            a.copy_for_fork("snap-own", Some("p_lab"), None)
                .await
                .ok()
                .unwrap(),
            "snap-lab"
        );

        let broke: Joined<SealedImageBuilt> =
            async { Err(ImageError::Host(HostError::new("the build broke"))) }
                .boxed_local()
                .shared();
        w.copy_builds.borrow_mut().insert(s("p_lab/default"), broke);
        assert_eq!(
            a.copy_for_fork("snap-own", Some("p_lab"), None)
                .await
                .ok()
                .unwrap(),
            "snap-own",
            "a build that failed is read as none"
        );
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

// I12: image.ts:262-263; copy-on-press.test.ts:438. A copy current against the record is answered as it stands, and says
// nothing about a build; one that is not, or none, answers the snapshot asked where nobody asked to build.
#[test]
fn copy_for_fork_takes_a_current_copy() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let announce = announcing(w);
        seed_copy(
            w,
            "p_lab",
            vec![version_with(1, "snap-lab", Some(H1), false, None)],
            1,
        );
        for asked in [None, Some(announce.clone())] {
            assert_eq!(
                a.copy_for_fork("snap-own", Some("p_lab"), asked)
                    .await
                    .ok()
                    .unwrap(),
                "snap-lab"
            );
        }
        assert!(lines_of(w, "announce").is_empty() && lines_of(w, "golden_prepare").is_empty());

        seed_copy(
            w,
            "p_lab",
            vec![version_with(1, "snap-lab", Some("old"), false, None)],
            1,
        );
        assert_eq!(
            a.copy_for_fork("snap-own", Some("p_lab"), None)
                .await
                .ok()
                .unwrap(),
            "snap-own",
            "a stale copy, nobody asked to build"
        );
        w.store.delete("goldens", "p_lab/default").await.unwrap();
        assert_eq!(
            a.copy_for_fork("snap-own", Some("p_lab"), None)
                .await
                .ok()
                .unwrap(),
            "snap-own"
        );
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

// I12: image.ts:264-269; copy-on-press.test.ts:348. A place holding no current copy builds one first, announcing the
// place and the rate of its default size before anything is made, and the new copy's snapshot is answered. A place that
// builds nothing answers the snapshot asked.
#[test]
fn copy_for_fork_builds_and_announces_with_the_rate() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        let got = a
            .copy_for_fork("snap-own", Some("p_lab"), Some(announcing(w)))
            .await
            .ok()
            .unwrap();
        assert_eq!(got, "snap-new");
        assert_eq!(
            lines_of(w, "announce"),
            ["announce lab 0.1"],
            "the place's name and the default size's rate"
        );
        assert!(
            at(w, "announce lab") < at(w, "golden_prepare"),
            "said before anything boots: {:?}",
            w.log()
        );
        assert_eq!(lines_of(w, "golden_prepare").len(), 1);

        let before = w.log().len();
        assert_eq!(
            a.copy_for_fork("snap-own", Some("p_idle"), Some(announcing(w)))
                .await
                .ok()
                .unwrap(),
            "snap-own"
        );
        assert!(w.log()[before..]
            .iter()
            .all(|l| !l.starts_with("announce") && !l.starts_with("golden_prepare")));
    });
}

// I12: copy-on-press.test.ts:362. At a place that charges nothing the announcement carries a rate of 0.
#[test]
fn at_a_place_that_charges_nothing_the_line_says_no_rate() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let mut free = FakeBackend::with("free", building_caps());
        free.pricing = Arc::new(FakePricing {
            default: Size {
                cpu: 2.0,
                mem_mb: 4096.0,
            },
            rate: 0.0,
            storage: storage_pricing("restored"),
        });
        w.door
            .borrow()
            .as_ref()
            .unwrap()
            .computer("p_free", "free", Some(Arc::new(free)));
        script_builds(w, vec![built("b-copy", "p_free", "snap-free", 1, H1)]);
        assert_eq!(
            a.copy_for_fork("snap-own", Some("p_free"), Some(announcing(w)))
                .await
                .ok()
                .unwrap(),
            "snap-free"
        );
        assert_eq!(lines_of(w, "announce"), ["announce free 0"]);
    });
}

// I12 and I8: copy-on-press.test.ts:372. Two forks at once build one copy, each saying so; a copy a cut left behind is
// built again before the next fork.
#[test]
fn two_forks_at_once_build_one_copy_and_a_copy_a_cut_left_behind_is_built_again_before_the_next_fork(
) {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(w, vec![built("b-1", "p_lab", "snap-1", 1, H1)]);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        *w.copy_gate.borrow_mut() = Some(
            async move {
                let _ = rx.await;
            }
            .boxed_local()
            .shared(),
        );
        let one = a.copy_for_fork("snap-own", Some("p_lab"), Some(announcing(w)));
        let two = a.copy_for_fork("snap-own", Some("p_lab"), Some(announcing(w)));
        let release = async {
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            tx.send(()).unwrap();
        };
        let (x, y, _) = futures::join!(one, two, release);
        assert_eq!(
            (x.ok().unwrap(), y.ok().unwrap()),
            (s("snap-1"), s("snap-1"))
        );
        assert_eq!(lines_of(w, "golden_prepare").len(), 1);
        assert_eq!(
            lines_of(w, "announce").len(),
            2,
            "starting runs for the caller that only joins"
        );

        *w.copy_gate.borrow_mut() = None;
        put_record(w, |v| v["hash"] = json!("hash-2"));
        script_builds(w, vec![built("b-2", "p_lab", "snap-2", 2, "hash-2")]);
        let third = a
            .copy_for_fork("snap-own", Some("p_lab"), Some(announcing(w)))
            .await
            .ok()
            .unwrap();
        assert_eq!(third, "snap-2");
        assert_eq!(lines_of(w, "golden_prepare").len(), 2);
        assert_eq!(lines_of(w, "announce").len(), 3);
    });
}

// I8 and I12: copy-on-press.test.ts:127. A create on a place whose copy is building joins the build, forks the copy it
// seals and starts no second one; so does a keep.
#[test]
fn a_workspace_created_on_a_place_whose_copy_is_building_joins_the_build_forks_the_copy_it_seals_and_starts_no_second_one(
) {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        *w.copy_gate.borrow_mut() = Some(
            async move {
                let _ = rx.await;
            }
            .boxed_local()
            .shared(),
        );
        let building = a.build(ask("p_lab"));
        let forking = a.copy_for_fork("snap-own", Some("p_lab"), None);
        let keeping = a.keep_current("p_lab", None);
        let release = async {
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            tx.send(()).unwrap();
        };
        let (b, f, _, _) = futures::join!(building, forking, keeping, release);
        assert_eq!(
            to_json(&b.unwrap())["copy"]["snapshotId"],
            json!("snap-new")
        );
        assert_eq!(
            f.ok().unwrap(),
            "snap-new",
            "the create forks the copy the build sealed"
        );
        assert_eq!(lines_of(w, "golden_prepare").len(), 1);
        assert_eq!(lines_of(w, "seal_entry").len(), 1);
        assert!(w.copy_builds.borrow().is_empty());
    });
}

// I12 and I8: copy-on-press.test.ts:421. A fork that meets a copy already building there says it waits on that build,
// and forks its copy.
#[test]
fn a_fork_that_meets_a_copy_already_building_there_says_it_waits_on_that_build_and_forks_its_copy()
{
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let copy = json!({"place": "p_lab", "version": 1, "hash": H1, "snapshotId": "snap-lab", "builtAt": BUILT_AT});
        let ok: SealedImageBuilt =
            serde_json::from_value(json!({"copy": copy, "built": true})).unwrap();
        w.copy_builds.borrow_mut().insert(
            s("p_lab/default"),
            async move { Ok(ok) }.boxed_local().shared(),
        );
        let got = a
            .copy_for_fork("snap-own", Some("p_lab"), Some(announcing(w)))
            .await
            .ok()
            .unwrap();
        assert_eq!(got, "snap-lab");
        assert_eq!(lines_of(w, "announce"), ["announce lab 0.1"]);
        assert!(lines_of(w, "golden_prepare").is_empty());
    });
}

fn solari_scene() -> Scene {
    let sc = scene();
    sc.w.places
        .rows
        .borrow_mut()
        .insert(s("solari"), FakeBackend::new("solari"));
    sc
}

fn held_refusal(home: &str) -> HostError {
    refusal(
        &image_held_sign_ins_line(home),
        &image_rebuild_fix(home),
        Some(Kind::Conflict),
    )
}

// I13: image.ts:232-240; copy-on-press.test.ts:387. At a provider a version no provider's seal marked is refused naming
// the place the image was sealed at, at that place and at a copy elsewhere alike.
#[test]
fn copy_for_fork_at_a_provider_refuses_a_version_holding_sign_ins() {
    run(async {
        let sc = solari_scene();
        let (w, a) = (&sc.w, &sc.a);
        // The image's own place, a provider, holds a head sealed before the mark; so does a copy at solari.
        seed_copy(
            w,
            "boat",
            vec![version_with(1, "snap-own", Some(H1), false, None)],
            1,
        );
        seed_copy(
            w,
            "solari",
            vec![version_with(1, "snap-sol", Some(H1), false, None)],
            1,
        );
        let e = refused(a.copy_for_fork("snap-own", None, None).await);
        assert_eq!(e, held_refusal("boat"));
        assert_eq!(
            e.message,
            "Your image on boat was sealed with sign-ins on it. Seal your image again for boat."
        );
        let e = refused(a.copy_for_fork("snap-own", Some("solari"), None).await);
        assert_eq!(
            e,
            held_refusal("boat"),
            "the copy elsewhere names the place the image was sealed at"
        );

        // The record a version is a copy of: the hash it carries, else the record whose own place lists it, else the
        // only record, else none, and then the place the fork lands on is named.
        put_record(w, |v| {
            v["name"] = json!("other");
            v["hash"] = json!("hash-other");
            v["place"] = json!("solari");
        });
        seed_copy(
            w,
            "solari",
            vec![version_with(1, "snap-sol", Some("hash-other"), false, None)],
            1,
        );
        w.store.seed("goldens", "solari/other", json!({"head": 1, "versions": [version_with(1, "snap-sol", Some("hash-other"), false, None)]}));
        let records = vec![
            sealed_image("default", 1, H1, Some("boat")),
            sealed_image("other", 1, "hash-other", Some("solari")),
        ];
        let by_hash: GoldenVersion =
            serde_json::from_value(version_with(1, "snap-x", Some("hash-other"), false, None))
                .unwrap();
        assert_eq!(
            a.record_of_version(&records, Some(&by_hash))
                .await
                .unwrap()
                .map(|r| r.name),
            Some(s("other"))
        );
        let listed: GoldenVersion =
            serde_json::from_value(version_with(1, "snap-sol", None, false, None)).unwrap();
        assert_eq!(
            a.record_of_version(&records, Some(&listed))
                .await
                .unwrap()
                .map(|r| r.name),
            Some(s("other")),
            "the one whose own place lists it"
        );
        let nowhere: GoldenVersion =
            serde_json::from_value(version_with(1, "snap-nowhere", None, false, None)).unwrap();
        assert!(a
            .record_of_version(&records, Some(&nowhere))
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            a.record_of_version(&records[..1], Some(&nowhere))
                .await
                .unwrap()
                .map(|r| r.name),
            Some(s("default")),
            "the only record"
        );
        assert_eq!(
            a.record_of_version(&records[..1], None)
                .await
                .unwrap()
                .map(|r| r.name),
            Some(s("default"))
        );

        // No record to name (two records, none of them this snapshot's): the place the fork lands on.
        w.store.seed(
            "goldens",
            "elsewhere/default",
            json!({"head": 1, "versions": [version_with(1, "snap-nowhere", None, false, None)]}),
        );
        let e = err(a.refuse_held_sign_ins("snap-nowhere", "solari").await);
        assert_eq!(e, held_refusal("solari"));
        assert_eq!(e.fix.as_deref(), Some("Seal your image again for solari."));
    });
}

// I13: image.ts:236-239; sealed-image.test.ts:366. A version a provider's seal marked, no image records at all, and a
// snapshot no manifest names are not judged.
#[test]
fn copy_for_fork_at_a_provider_passes_a_no_secrets_version() {
    run(async {
        let sc = solari_scene();
        let (w, a) = (&sc.w, &sc.a);
        seed_copy(
            w,
            "solari",
            vec![version_with(1, "snap-sol", Some(H1), true, None)],
            1,
        );
        assert_eq!(
            a.copy_for_fork("snap-own", Some("solari"), None)
                .await
                .ok()
                .unwrap(),
            "snap-sol"
        );
        assert_eq!(
            a.copy_for_fork("snap-own", None, None).await.ok().unwrap(),
            "snap-own"
        );
        assert_eq!(
            a.copy_for_fork("snap-unknown", Some("solari"), None)
                .await
                .ok()
                .unwrap(),
            "snap-unknown",
            "a snapshot no manifest names"
        );

        // A host that owns no image record holds only images from before records.
        let w = World::new();
        w.places
            .rows
            .borrow_mut()
            .insert(s("solari"), FakeBackend::new("solari"));
        seed_copy(
            &w,
            "boat",
            vec![version_with(1, "snap-old", None, false, None)],
            1,
        );
        let a = ImageArea::new(w.clone());
        assert_eq!(
            a.copy_for_fork("snap-old", None, None).await.ok().unwrap(),
            "snap-old"
        );
        assert_eq!(
            a.copy_for_fork("snap-old", Some("solari"), None)
                .await
                .ok()
                .unwrap(),
            "snap-old"
        );
    });
}

// ---------------------------------------------------------------------------------------------------------------
// TS cases that reach the copy build at this unit's seam, beyond the spec rows
// ---------------------------------------------------------------------------------------------------------------

// sealed-image.test.ts:297. A copy build runs no sign-in stage of its own and takes no second vault: the only road after
// the prepare is the record's own archive landing, and the record stays as it was.
#[test]
fn a_copy_build_never_runs_a_sign_in_stage_and_takes_no_second_vault_off_its_own_builder() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let before = w.store.doc("images", "default");
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        a.build(ask("p_lab")).await.unwrap();
        assert_eq!(
            lines_of(w, "road"),
            [
                "road refuse_foreign_members import /root/.codex/auth.json",
                "road import_image_vault b-copy 9"
            ]
        );
        let stages = lines_of(w, "stage ");
        assert_eq!(stages.len(), 1, "{stages:?}");
        assert!(stages[0].starts_with("stage uploading-files "));
        assert!(lines_of(w, "seal_entry")[0].contains("keep=false"));
        assert_eq!(w.store.doc("images", "default"), before);
        assert!(w.store.blob("image-vaults", "default@v2").is_none());
        assert!(w.store.blob("image-vaults", "default@v1").is_some());
    });
}

// sealed-image.test.ts:311. A place standing on the record is answered with its copy and builds nothing, on the build
// line and on the road that keeps a place current alike.
#[test]
fn a_place_standing_on_the_record_is_answered_with_its_copy_and_builds_nothing_on_the_build_line_and_on_the_road_that_keeps_a_place_current_alike(
) {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        seed_copy(
            w,
            "p_lab",
            vec![version_with(1, "snap-lab", Some(H1), false, None)],
            1,
        );
        let done = a.build(ask("p_lab")).await.unwrap();
        assert_eq!(to_json(&done)["built"], json!(false));
        a.keep_current("p_lab", None).await;
        assert!(
            lines_of(w, "golden_prepare").is_empty()
                && lines_of(w, "copy_recipe").is_empty()
                && lines_of(w, "road").is_empty()
        );
        assert!(w.framed.borrow().is_empty() && w.copy_builds.borrow().is_empty());
    });
}

// sealed-image.test.ts:324. A forced build at a place standing on the record builds and seals a new copy there and leaves
// the record as it was.
#[test]
fn a_forced_build_at_a_place_standing_on_the_record_builds_and_seals_a_new_copy_there_and_leaves_the_record_as_it_was(
) {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        seed_copy(
            w,
            "p_lab",
            vec![version_with(1, "snap-lab", Some(H1), false, None)],
            1,
        );
        let record = w.store.doc("images", "default");
        script_builds(w, vec![built("b-copy", "p_lab", "snap-lab-2", 2, H1)]);
        let done = a
            .build(ImageBuildAsk {
                force: Some(true),
                ..ask("p_lab")
            })
            .await
            .unwrap();
        let done = to_json(&done);
        assert_eq!(
            (
                done["built"].clone(),
                done["copy"]["version"].clone(),
                done["copy"]["hash"].clone()
            ),
            (json!(true), json!(2), json!(H1))
        );
        assert_eq!(
            lines_of(w, "copy_recipe").len(),
            1,
            "composed afresh, so it takes what this host ships now"
        );
        assert_eq!(w.store.doc("images", "default"), record);
    });
}

// sealed-image.test.ts:355. The image's own place is answered with the copy it holds; a place this host has never heard
// of is refused before anything boots.
#[test]
fn the_place_the_images_own_seal_stands_at_is_answered_with_the_copy_it_holds_and_a_place_this_host_has_never_heard_of_is_refused_before_anything_boots(
) {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        let done = to_json(&a.build(ask("boat")).await.unwrap());
        assert_eq!(
            (
                done["built"].clone(),
                done["copy"]["place"].clone(),
                done["copy"]["version"].clone()
            ),
            (json!(false), json!("boat"), json!(1))
        );
        let e = failed(a.build(ask("nowhere")).await);
        assert_eq!(e.kind(), Some(Kind::Missing));
        assert!(lines_of(w, "copy_recipe").is_empty() && lines_of(w, "golden_prepare").is_empty());
    });
}

// copy-on-press.test.ts:231. A host with no sealed image keeps nothing current, and a place that forks nothing owes no copy:
// nothing composed, nothing built, nothing framed.
#[test]
fn a_host_with_no_sealed_image_keeps_nothing_current_and_a_place_that_forks_nothing_owes_no_copy() {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        w.store.delete("images", "default").await.unwrap();
        a.keep_current("p_lab", None).await;
        assert_eq!(w.log(), ["ready"]);
        put_record(w, |_| {});
        a.keep_current("p_idle", None).await;
        assert!(lines_of(w, "copy_recipe").is_empty() && lines_of(w, "golden_prepare").is_empty());
        assert!(w.framed.borrow().is_empty());
    });
}

// sealed-image.test.ts:402 and copy-on-press.test.ts:410. A record sealed at a provider holds no vault and still copies with
// no force, since nothing is missing from it.
#[test]
fn a_record_sealed_at_a_provider_holds_no_vault_and_still_copies_with_no_force_since_nothing_is_missing_from_it(
) {
    run(async {
        let sc = scene();
        let (w, a) = (&sc.w, &sc.a);
        put_record(w, |v| {
            without(v, "vault");
            v["noSecrets"] = json!(true);
        });
        script_builds(w, vec![built("b-copy", "p_lab", "snap-new", 1, H1)]);
        let done = a.build(ask("p_lab")).await.unwrap();
        assert_eq!(to_json(&done)["built"], json!(true));
        assert!(lines_of(w, "road import_image_vault").is_empty());
    });
}
