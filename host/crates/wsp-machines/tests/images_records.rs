// SPDX-License-Identifier: AGPL-3.0-only
//! images_records: spec/images_records.md. Every value is read off images/records.ts and images/held.ts at 65f6b89c7.
mod images_support;

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{json, Value};

use images_support::*;
use wsp_engine::machine::MachineShape;
use wsp_machines::images::held::HeldImages;
use wsp_machines::images::records::{LookKey, RecordsArea};
use wsp_machines::places::types::HeldImage;
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};

fn area(w: &Rc<World>) -> RecordsArea {
    RecordsArea::new(w.clone())
}

fn sizes(offers: &[(u32, u32)]) -> Vec<MachineSizeOffer> {
    offers
        .iter()
        .map(|(cpu, mem)| {
            serde_json::from_value(json!({"cpu": cpu, "memMb": mem, "rateUsdPerHour": 0.1}))
                .expect("offer")
        })
        .collect()
}

fn size(cpu: u32, mem: u32) -> WorkspaceSize {
    serde_json::from_value(json!({"cpu": cpu, "memMb": mem})).expect("size")
}

// R1: records.ts:88-125.
#[test]
fn view_carries_every_present_fact_in_the_ts_key_order() {
    let w = World::new();
    w.daemon_notes
        .borrow_mut()
        .insert("w1".into(), "the daemon is old".into());
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.glyph = Some(serde_json::from_value(json!("code")).expect("glyph"));
    r.parent_thread_id = Some("t-parent".into());
    r.place = Some("boat".into());
    let view = area(&w).view(&r).expect("the view answers");
    let text = serde_json::to_value(&view).expect("view json");
    let keys: Vec<String> = text.as_object().expect("object").keys().cloned().collect();
    // The TS literal's order: folder, the home spread, id ... project, then each optional fact, pauseMode last.
    assert_eq!(
        keys,
        [
            "folder",
            "home",
            "id",
            "name",
            "machineId",
            "phase",
            "kind",
            "golden",
            "createdAt",
            "project",
            "glyph",
            "daemonNote",
            "parentThreadId",
            "place",
            "provider"
        ]
        .map(String::from)
        .to_vec()
        .into_iter()
        .chain(text.get("pauseMode").map(|_| "pauseMode".to_string()))
        .collect::<Vec<_>>()
    );
    assert_eq!(text["folder"], json!("/root/landing"));
    assert_eq!(text["daemonNote"], json!("the daemon is old"));
    assert_eq!(text["provider"], json!("boat"));
}

// R1: an absent fact is left out, never written null.
#[test]
fn view_leaves_out_every_absent_fact() {
    let w = World::new();
    let r = workspace_record("w1", "landing", "snap-1");
    let text =
        serde_json::to_value(area(&w).view(&r).expect("the view answers")).expect("view json");
    for key in [
        "worktree",
        "from",
        "review",
        "branchOfRecord",
        "claudeSessionId",
        "screen",
        "queue",
        "gone",
        "theme",
        "glyph",
        "daemonNote",
        "daemonRefusedAt",
        "vaultedAt",
        "vaultRefused",
        "wakeRefused",
        "diskGone",
        "agents",
        "parentThreadId",
        "rootThreadId",
        "parentWorkspaceId",
        "place",
    ] {
        assert!(text.get(key).is_none(), "{key} should be absent: {text}");
    }
    assert_eq!(text["id"], json!("w1"));
    assert_eq!(text["golden"], json!("snap-1"));
}

// R2: records.ts:80-87.
#[test]
fn view_leaves_out_pause_mode_when_the_backend_cannot_be_read() {
    let w = World::new();
    w.runtime_backend.caps.borrow_mut().pause_mode =
        Some(serde_json::from_value(json!("memory")).expect("mode"));
    w.backend_for_fails.set(true);
    let text = serde_json::to_value(
        area(&w)
            .view(&workspace_record("w1", "landing", "snap-1"))
            .expect("the view answers"),
    )
    .expect("view json");
    assert!(text.get("pauseMode").is_none(), "{text}");
    assert_eq!(text["name"], json!("landing"));
}

// R2.
#[test]
fn view_carries_the_backends_pause_mode() {
    let w = World::new();
    w.runtime_backend.caps.borrow_mut().pause_mode =
        Some(serde_json::from_value(json!("memory")).expect("mode"));
    let text = serde_json::to_value(
        area(&w)
            .view(&workspace_record("w1", "landing", "snap-1"))
            .expect("the view answers"),
    )
    .expect("view json");
    assert_eq!(text["pauseMode"], json!("memory"));
    assert_eq!(
        text.as_object()
            .expect("object")
            .keys()
            .next_back()
            .map(String::as_str),
        Some("pauseMode")
    );
}

// R3: records.ts:129-133.
#[test]
fn put_look_sets_clears_and_leaves() {
    let w = World::new();
    let a = area(&w);
    let mut r = workspace_record("w1", "landing", "snap-1");
    a.put_look(&mut r, LookKey::Glyph, Some(Some(json!("terminal"))));
    assert_eq!(
        serde_json::to_value(&r).expect("record")["glyph"],
        json!("terminal")
    );
    a.put_look(&mut r, LookKey::Glyph, None);
    assert_eq!(
        serde_json::to_value(&r).expect("record")["glyph"],
        json!("terminal")
    );
    a.put_look(&mut r, LookKey::Theme, Some(None));
    a.put_look(&mut r, LookKey::Glyph, Some(Some(json!("code"))));
    assert_eq!(
        serde_json::to_value(&r).expect("record")["glyph"],
        json!("code")
    );
    a.put_look(&mut r, LookKey::Glyph, Some(None));
    let cleared = serde_json::to_value(&r).expect("record");
    assert!(cleared.get("glyph").is_none(), "{cleared}");
    assert!(cleared.get("theme").is_none(), "{cleared}");
}

// R4: records.ts:135-142.
#[test]
fn persist_bumps_the_generation_then_writes_record_and_name() {
    let w = World::new();
    let r = workspace_record("w1", "landing", "snap-1");
    w.live.borrow_mut().push(Rc::new(RefCell::new(r.clone())));
    run(area(&w).persist(&r)).expect("persist");
    assert_eq!(w.generations.borrow().get("w1"), Some(&1));
    assert_eq!(
        w.store.calls(),
        vec!["put workspaces/w1", "put workspace-names/w1"]
    );
    assert_eq!(
        w.store.doc("workspaces", "w1"),
        Some(serde_json::to_value(&r).expect("record"))
    );
    let named = w.store.doc("workspace-names", "w1").expect("name document");
    assert_eq!(
        serde_json::to_string(&named).expect("text"),
        r#"{"workspaceId":"w1","name":"landing","project":"p_spoo"}"#
    );
    assert_eq!(w.lines("bump_generation"), vec!["bump_generation w1"]);
}

// R4.
#[test]
fn persist_without_a_live_entry_writes_both() {
    let w = World::new();
    let r = workspace_record("w2", "other", "snap-1");
    run(area(&w).persist(&r)).expect("persist");
    assert!(w.generations.borrow().is_empty());
    assert_eq!(w.store.ids("workspaces"), vec!["w2"]);
    assert_eq!(w.store.ids("workspace-names"), vec!["w2"]);
}

// R4: a failed first put fails the call and writes no name document.
#[test]
fn persist_that_fails_writes_no_name_document() {
    let w = World::new();
    w.store.fail_puts.borrow_mut().push("workspaces".into());
    let r = workspace_record("w1", "landing", "snap-1");
    let e = run(area(&w).persist(&r)).expect_err("the put fails");
    assert_eq!(e.message, "put workspaces refused");
    assert!(w.store.doc("workspace-names", "w1").is_none());
}

// R5: records.ts:144-147.
#[test]
fn shape_of_answers_none_when_describe_fails() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    assert_eq!(run(area(&w).shape_of(m.clone())), None);
    let shape: MachineShape =
        serde_json::from_value(json!({"cpu": 4, "memMb": 8192})).expect("shape");
    *m.describe.borrow_mut() = Some(Ok(shape.clone()));
    assert_eq!(run(area(&w).shape_of(m)), Some(shape));
}

// R6: records.ts:150-153.
#[test]
fn size_built_takes_the_providers_word_else_the_ask() {
    let w = World::new();
    let a = area(&w);
    let asked = size(2, 4096);
    assert_eq!(a.size_built(None, &asked), asked);
    let half: MachineShape = serde_json::from_value(json!({"cpu": 4})).expect("shape");
    assert_eq!(a.size_built(Some(&half), &asked), size(4, 4096));
    let whole: MachineShape =
        serde_json::from_value(json!({"cpu": 8, "memMb": 16384})).expect("shape");
    assert_eq!(a.size_built(Some(&whole), &asked), size(8, 16384));
}

// R7: records.ts:158-162.
#[test]
fn read_memory_warns_the_exit_and_the_stderr_tail() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    let long = format!("{}{}", "x".repeat(50), "y".repeat(200));
    m.answer(Ok(exec_exit(2, "", &format!("  {long}\n"))));
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(2, 4096);
    let before = r.clone();
    assert!(!run(area(&w).read_memory(&mut r, m, &sizes(&[(2, 4096)]))));
    assert_eq!(
        serde_json::to_value(&r).expect("r"),
        serde_json::to_value(&before).expect("before")
    );
    let words = wsp_words::words::units::size_word(&size(2, 4096));
    assert_eq!(
        w.warned.lines(),
        vec![format!(
            "workspace w1: memory not read on m1 (exit 2: {}); the row keeps {words}",
            "y".repeat(200)
        )]
    );
}

// R7.
#[test]
fn read_memory_warns_a_rejected_exec() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    m.answer(Err(missing("machine m1 is gone")));
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(2, 4096);
    assert!(!run(area(&w).read_memory(&mut r, m, &sizes(&[(2, 4096)]))));
    let words = wsp_words::words::units::size_word(&size(2, 4096));
    assert_eq!(
        w.warned.lines(),
        vec![format!(
            "workspace w1: memory not read on m1 (machine m1 is gone); the row keeps {words}"
        )]
    );
}

// R7: exit 0 with no count answers false and warns nothing.
#[test]
fn read_memory_with_no_count_answers_false_quietly() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    m.answer(Ok(exec_ok("nothing here\n")));
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(2, 4096);
    assert!(!run(area(&w).read_memory(&mut r, m, &sizes(&[(2, 4096)]))));
    assert!(w.warned.lines().is_empty());
    assert_eq!(r.size, size(2, 4096));
}

// R7: MEM_READ with the inline timeout.
#[test]
fn read_memory_runs_mem_read_with_the_inline_timeout() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    m.answer(Ok(exec_ok("memkb 4128768\n")));
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(2, 4096);
    run(area(&w).read_memory(&mut r, m.clone(), &sizes(&[(2, 4096)])));
    assert_eq!(
        m.execs(),
        vec![format!("{} [20000ms]", wsp_engine::machine_facts::MEM_READ)]
    );
}

// R8: 4032 MB read on a 4096 MB machine is the 4096 offer (within a sixteenth).
#[test]
fn read_memory_snaps_to_an_offer_within_a_sixteenth() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    m.answer(Ok(exec_ok("memkb 4128768\n"))); // 4032 MB
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(2, 2048);
    assert!(run(area(&w).read_memory(
        &mut r,
        m,
        &sizes(&[(2, 4096), (4, 8192)])
    )));
    assert_eq!(r.size, size(2, 4096));
}

// R8: a figure past every offer is capped at the largest.
#[test]
fn read_memory_caps_at_the_largest_offer() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    m.answer(Ok(exec_ok("memkb 67108864\n"))); // 65536 MB
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(4, 8192);
    assert!(run(area(&w).read_memory(
        &mut r,
        m,
        &sizes(&[(2, 4096), (4, 8192)])
    )));
    assert_eq!(r.size, size(4, 8192));
}

// R8: no offers keeps the count as read.
#[test]
fn read_memory_with_no_offers_keeps_the_count() {
    let w = World::new();
    let m = FakeMachine::new("m1");
    m.answer(Ok(exec_ok("memkb 3145728\n"))); // 3072 MB
    let mut r = workspace_record("w1", "landing", "snap-1");
    r.size = size(2, 4096);
    assert!(run(area(&w).read_memory(&mut r, m, &[])));
    assert_eq!(r.size, size(2, 3072));
}

// R9: records.ts:173.
#[test]
fn forked_from_names_every_live_fork_of_the_snapshot_in_order() {
    let w = World::new();
    for (id, name, golden) in [
        ("w1", "b-first", "snap-1"),
        ("w2", "other", "snap-2"),
        ("w3", "a-second", "snap-1"),
    ] {
        let mut r = workspace_record(id, name, golden);
        if id == "w3" {
            r.phase = serde_json::from_value(json!("napping")).expect("phase");
        }
        w.live.borrow_mut().push(Rc::new(RefCell::new(r)));
    }
    assert_eq!(area(&w).forked_from("snap-1"), vec!["b-first", "a-second"]);
    assert!(area(&w).forked_from("snap-9").is_empty());
}

// R10: records.ts:179-190.
#[test]
fn copies_and_recipes_are_filed_under_the_place() {
    let w = World::new();
    let a = area(&w);
    let m = manifest(&[(1, "snap-1", None)]);
    run(a.put_copy("boat", "default", &m)).expect("put copy");
    assert_eq!(w.store.ids("goldens"), vec!["boat/default"]);
    assert_eq!(run(a.copy_of("boat", "default")).expect("copy"), Some(m));
    assert_eq!(run(a.copy_of("pl_spoo", "default")).expect("copy"), None);
    let digest = serde_json::from_value(json!({"ticks": [], "files": []}))
        .unwrap_or_else(|e| panic!("a digest: {e}"));
    run(a.put_copy_recipe("boat", "default", 3, &digest)).expect("put recipe");
    assert_eq!(w.store.ids("golden-recipes"), vec!["boat/default@v3"]);
    assert_eq!(
        run(a.copy_recipe_of("boat", "default", 3)).expect("recipe"),
        Some(digest)
    );
}

// R10.
#[test]
fn drop_copy_recipe_deletes_only_that_version() {
    let w = World::new();
    w.store.seed(
        "golden-recipes",
        "boat/default@v2",
        json!({"ticks": [], "files": []}),
    );
    w.store.seed(
        "golden-recipes",
        "boat/default@v3",
        json!({"ticks": [], "files": []}),
    );
    run(area(&w).drop_copy_recipe("boat", "default", 2)).expect("drop");
    assert_eq!(w.store.ids("golden-recipes"), vec!["boat/default@v3"]);
    assert_eq!(
        w.store.calls(),
        vec!["delete golden-recipes/boat/default@v2"]
    );
}

// R10: Proposal 3 drops the bare-name fallback, even for the wired place.
#[test]
fn a_bare_key_is_never_read() {
    let w = World::new();
    w.store.seed(
        "goldens",
        "default",
        serde_json::to_value(manifest(&[(1, "snap-1", None)])).expect("json"),
    );
    w.store.seed(
        "golden-recipes",
        "default@v1",
        json!({"ticks": [], "files": []}),
    );
    assert_eq!(
        run(area(&w).copy_of("boat", "default")).expect("copy"),
        None
    );
    assert_eq!(
        run(area(&w).copy_recipe_of("boat", "default", 1)).expect("recipe"),
        None
    );
    run(area(&w).drop_copy_recipe("boat", "default", 1)).expect("drop");
    assert_eq!(w.store.ids("golden-recipes"), vec!["default@v1"]);
}

// R11: records.ts:226-228; the backfill from a copy is dropped.
#[test]
fn record_of_reads_the_stored_record_and_nothing_else() {
    let w = World::new();
    w.put_copy_now("boat", "default", &manifest(&[(1, "snap-1", None)]));
    assert_eq!(run(area(&w).record_of("default")).expect("record"), None);
    let image = sealed_image("default", 1, "h1", Some("boat"));
    w.put_image_now(&image);
    assert_eq!(
        run(area(&w).record_of("default")).expect("record"),
        Some(image)
    );
}

// R12: records.ts:248-258.
#[test]
fn recorded_images_holds_versions_templates_and_live_goldens() {
    let w = World::new();
    w.put_copy_now(
        "boat",
        "default",
        &manifest(&[(1, "snap-1", Some("t-1")), (2, "snap-2", None)]),
    );
    w.put_copy_now("pl_spoo", "default", &manifest(&[(1, "snap-9", None)]));
    w.live
        .borrow_mut()
        .push(Rc::new(RefCell::new(workspace_record("w1", "a", "snap-2"))));
    w.live
        .borrow_mut()
        .push(Rc::new(RefCell::new(workspace_record(
            "w2",
            "b",
            "snap-live",
        ))));
    let ids: Vec<String> = run(area(&w).recorded_images())
        .expect("ids")
        .into_iter()
        .collect();
    assert_eq!(ids, vec!["snap-1", "t-1", "snap-2", "snap-9", "snap-live"]);
}

// R13: records.ts:261-277.
#[test]
fn image_of_finds_the_version_in_the_first_manifest_holding_it() {
    let w = World::new();
    w.put_copy_now(
        "boat",
        "default",
        &manifest(&[(1, "snap-1", None), (2, "snap-2", Some("t-2"))]),
    );
    w.put_copy_now("pl_spoo", "default", &manifest(&[(7, "snap-2", None)]));
    let of = run(area(&w).image_of("snap-2")).expect("image of");
    assert_eq!(of.golden, "snap-2");
    assert_eq!(of.version, Some(version(2, "snap-2", Some("t-2"))));
}

// R13.
#[test]
fn image_of_an_unknown_snapshot_has_no_version() {
    let w = World::new();
    w.put_copy_now("boat", "default", &manifest(&[(1, "snap-1", None)]));
    let of = run(area(&w).image_of("snap-x")).expect("image of");
    assert_eq!((of.golden.as_str(), of.version), ("snap-x", None));
}

fn held(w: &Rc<World>) -> HeldImages {
    HeldImages::new(w.clone())
}

// H1: held.ts:21-25.
#[test]
fn held_on_waits_for_ready_and_lists_the_places_versions() {
    let w = World::new();
    w.put_copy_now(
        "pl_spoo",
        "default",
        &manifest(&[(1, "snap-1", Some("t-1")), (2, "snap-2", None)]),
    );
    w.put_copy_now("pl_spoo", "work", &manifest(&[(1, "snap-w", None)]));
    let on = run(held(&w).on("pl_spoo")).expect("held");
    assert_eq!(
        on,
        vec![
            HeldImage {
                id: "snap-1".into(),
                template_id: Some("t-1".into())
            },
            HeldImage {
                id: "snap-2".into(),
                template_id: None
            },
            HeldImage {
                id: "snap-w".into(),
                template_id: None
            },
        ]
    );
    assert_eq!(w.log().first().map(String::as_str), Some("ready"));
}

// H1.
#[test]
fn held_on_skips_other_places() {
    let w = World::new();
    w.put_copy_now("boat", "default", &manifest(&[(1, "snap-b", None)]));
    w.put_copy_now("pl_spoo", "default", &manifest(&[(1, "snap-s", None)]));
    let on = run(held(&w).on("boat")).expect("held");
    assert_eq!(
        on,
        vec![HeldImage {
            id: "snap-b".into(),
            template_id: None
        }]
    );
}

// H2: held.ts:26-34.
#[test]
fn held_drop_writes_back_the_rest_and_drops_each_recipe() {
    let w = World::new();
    let mut m =
        serde_json::to_value(manifest(&[(1, "snap-1", None), (2, "snap-2", None)])).expect("json");
    m["head"] = json!(2);
    w.store.seed("goldens", "pl_spoo/default", m);
    w.store.seed(
        "golden-recipes",
        "pl_spoo/default@v1",
        json!({"ticks": [], "files": []}),
    );
    run(held(&w).drop("pl_spoo", "snap-1")).expect("drop");
    let left = w
        .store
        .doc("goldens", "pl_spoo/default")
        .expect("the copy stays");
    assert_eq!(left["head"], json!(2));
    assert_eq!(
        left["versions"].as_array().map(|v| v
            .iter()
            .map(|x| x["snapshotId"].clone())
            .collect::<Vec<Value>>()),
        Some(vec![json!("snap-2")])
    );
    assert_eq!(
        w.lines("drop_copy_recipe"),
        vec!["drop_copy_recipe pl_spoo default 1"]
    );
    assert!(!w.log().iter().any(|l| l == "ready"));
}

// H2: a copy left with no version is deleted.
#[test]
fn held_drop_deletes_a_copy_left_empty() {
    let w = World::new();
    w.put_copy_now("pl_spoo", "default", &manifest(&[(1, "snap-1", None)]));
    run(held(&w).drop("pl_spoo", "snap-1")).expect("drop");
    assert!(w.store.doc("goldens", "pl_spoo/default").is_none());
    assert_eq!(w.store.calls()[0], "delete goldens/pl_spoo/default");
    assert_eq!(
        w.lines("drop_copy_recipe"),
        vec!["drop_copy_recipe pl_spoo default 1"]
    );
}

// H2: a copy without the snapshot is not written.
#[test]
fn held_drop_leaves_copies_without_the_snapshot_unwritten() {
    let w = World::new();
    w.put_copy_now("pl_spoo", "default", &manifest(&[(1, "snap-1", None)]));
    w.put_copy_now("pl_spoo", "work", &manifest(&[(1, "snap-w", None)]));
    run(held(&w).drop("pl_spoo", "snap-w")).expect("drop");
    assert_eq!(
        w.store.calls(),
        vec![
            "delete goldens/pl_spoo/work",
            "delete golden-recipes/pl_spoo/work@v1"
        ]
    );
    assert!(w.store.doc("goldens", "pl_spoo/default").is_some());
}

// Restart: nothing here is memory.
#[test]
fn a_new_area_over_the_same_store_reads_every_copy_back() {
    let w = World::new();
    run(area(&w).put_copy("boat", "default", &manifest(&[(1, "snap-1", None)]))).expect("put");
    let again = RecordsArea::new(w.clone());
    assert_eq!(
        run(again.copy_of("boat", "default")).expect("copy"),
        Some(manifest(&[(1, "snap-1", None)]))
    );
    assert_eq!(
        run(held(&w).on("boat")).expect("held"),
        vec![HeldImage {
            id: "snap-1".into(),
            template_id: None
        }]
    );
}
