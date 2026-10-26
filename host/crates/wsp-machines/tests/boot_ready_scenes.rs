// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_ready.md: the five scenes of `boot_ready.json`, replayed through the fake runtime of `support/boot.rs`:
//! every call the start makes in order, and the owner, the maps and the store rows it leaves.
mod common;
#[path = "support/boot.rs"]
mod support;

use serde_json::{json, Value};

use support::{norm, row_json, strip_project, strip_workspace, Opts, World};

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

fn strings(v: &Value) -> Vec<&'static str> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|s| leak(s.as_str().unwrap()))
        .collect()
}

fn pairs(v: &Value) -> Vec<(&'static str, &'static str)> {
    v.as_object()
        .into_iter()
        .flatten()
        .map(|(k, x)| (leak(k), leak(x.as_str().unwrap())))
        .collect()
}

/// What the oracle's `after()` read off the world once the scene ended.
fn assert_after(scene: &Value, w: &World) {
    let after = &scene["after"];
    let name = &scene["name"];
    assert_eq!(
        json!(w.rt.owner.borrow().clone()),
        after["owner"],
        "scene {name}: the owner"
    );
    assert_eq!(
        json!(w.rt.reads_since.get() as i64),
        after["readsSince"],
        "scene {name}: the reads stamp"
    );
    let projects: Vec<String> = w.rt.projects_held.borrow().keys().cloned().collect();
    assert_eq!(
        json!(projects),
        after["projects"],
        "scene {name}: the projects held"
    );
    assert_eq!(
        norm(w.rt.thread_records_json()),
        after["threadRecords"],
        "scene {name}: the thread records"
    );
    let marks: Vec<Value> =
        w.rt.viewed_marks
            .borrow()
            .iter()
            .map(|(k, v)| json!([k, v]))
            .collect();
    assert_eq!(
        json!(marks),
        after["viewedMarks"],
        "scene {name}: the viewed marks"
    );
    let rows: Vec<Value> =
        w.rt.sessions
            .borrow()
            .iter()
            .map(|(k, r)| json!([k, row_json(r)]))
            .collect();
    assert_eq!(
        rows.len(),
        after["sessions"].as_array().unwrap().len(),
        "scene {name}: the rows held"
    );
    for (got, want) in rows.iter().zip(after["sessions"].as_array().unwrap()) {
        assert_eq!(got, want, "scene {name}: row {}", want[0]);
    }
    let live: Vec<String> = w.rt.live.borrow().keys().cloned().collect();
    assert_eq!(
        json!(live),
        after["live"],
        "scene {name}: the live workspaces"
    );
    let work: Vec<String> = w.rt.boot_work.borrow().keys().cloned().collect();
    assert_eq!(
        json!(work),
        after["bootWork"],
        "scene {name}: the boot work left"
    );
    let stores = after["stores"].as_object().unwrap();
    for (t, want) in stores {
        let got: Vec<Value> =
            w.rt.mem
                .table(t)
                .as_array()
                .unwrap()
                .iter()
                .map(|p| json!([p[0], store_row(t, p[1].clone())]))
                .collect();
        assert_eq!(json!(got), *want, "scene {name}: the {t} rows");
    }
    let held: Vec<String> =
        w.rt.mem
            .rows
            .borrow()
            .iter()
            .filter(|(t, rows)| !stores.contains_key(*t) && !rows.is_empty())
            .map(|(t, _)| t.clone())
            .collect();
    assert!(
        held.is_empty(),
        "scene {name}: rows the oracle's store never held: {held:?}"
    );
}

fn store_row(table: &str, row: Value) -> Value {
    match table {
        "workspaces" => strip_workspace(row),
        "projects" => strip_project(row),
        _ => row,
    }
}

fn ready(name: &'static str) {
    let fx = common::fixture("boot_ready");
    let scene = common::scene(&fx, name).clone();
    common::run_local(async move {
        let given = &scene["given"];
        let w = World::ready(Opts {
            store: given["store"].clone(),
            states: pairs(&given["states"]),
            reattach: pairs(&given["reattach"]),
            devices: given["devices"].clone(),
            shells: strings(&given["shells"]),
            running: strings(&given["running"]),
            no_local: given["local"] == json!(false),
            ..Opts::default()
        });
        match name {
            "a fresh store mints the owner and the reads stamp and reads nothing else" => {
                w.area.ready().await.expect("the start");
                common::flush().await;
                let before = w.log.take().len();
                w.area.ready().await.expect("the joined start");
                let joined = w.log.take().len() == before;
                w.log.push(json!(["joined", joined]));
            }
            "the sessions documents fill the rows and the thread records, and running rows are re-opened" => {
                w.area.ready().await.expect("the start");
                common::flush().await;
                w.log.push(json!(["mark", "advance the link window"]));
                w.clock.advance(60_000);
                common::flush().await;
            }
            _ => w.area.ready().await.expect("the start"),
        }
        common::flush().await;
        let got = w.log.take();
        common::assert_log(&scene, &got);
        assert_after(&scene, &w);
    });
}

#[test]
fn a_fresh_store_mints_the_owner_and_the_reads_stamp_and_reads_nothing_else() {
    ready("a fresh store mints the owner and the reads stamp and reads nothing else");
}

#[test]
fn a_kept_owner_and_reads_stamp_are_read_back_a_blank_owner_is_minted_again() {
    ready("a kept owner and reads stamp are read back; a blank owner is minted again");
}

#[test]
fn projects_are_held_and_a_folder_project_with_no_repo_kept_is_read_for_one_on_this() {
    ready("projects are held, and a folder project with no repo kept is read for one on this computer");
}

#[test]
fn no_local_wiring_reads_no_repo() {
    ready("no local wiring reads no repo");
}

#[test]
fn the_sessions_documents_fill_the_rows_and_the_thread_records_and_running_rows_are() {
    ready("the sessions documents fill the rows and the thread records, and running rows are re-opened");
}
