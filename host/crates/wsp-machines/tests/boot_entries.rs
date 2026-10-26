// SPDX-License-Identifier: AGPL-3.0-only
//! Spec boot_entries.md: the door scenes of `boot_rows.json` (entry of, child of, entry of row, tree records, computer
//! rows, image head) and the tests the spec names beside them.
mod common;
#[path = "support/boot.rs"]
mod support;

use serde_json::{json, Value};

use support::{check_rows_scene, kind_word, short, thread_caller, Opts, World};
use wsp_core::types::internal::TreeTalk;
use wsp_machines::machines::boot::entries::{EntryAsk, RowAt};
use wsp_machines::machines::boot::ComputerRow;
use wsp_wire::views::place_kind::PlaceKind;

async fn world(live: Value, records: Value) -> World {
    let w = World::rows(Opts {
        live,
        records,
        ..Opts::default()
    });
    w.started().await;
    w
}

fn row(thread: &str, ws: &str) -> RowAt {
    RowAt {
        thread_id: Some(thread.to_string()),
        workspace_id: ws.to_string(),
    }
}

#[test]
fn entry_of_refusals_by_caller_a_create_still_landing_and_the_boot_work_waited() {
    check_rows_scene(
        "entry of: refusals by caller, a create still landing, and the boot work waited",
    );
}

#[test]
fn child_of_a_lead_s_own_child_by_lead_or_by_parent_else_refused_naming_both() {
    check_rows_scene("child of: a lead's own child by lead or by parent, else refused naming both");
}

#[test]
fn entry_of_row_a_thread_caller_reaches_only_its_tree_and_answers_nothing_otherwise() {
    check_rows_scene(
        "entry of row: a thread caller reaches only its tree, and answers nothing otherwise",
    );
}

#[test]
fn computer_rows_with_no_door_this_computer_and_the_wired_provider() {
    check_rows_scene("computer rows: with no door this computer and the wired provider");
}

#[test]
fn computer_rows_a_door_that_lists_the_provider_is_read_as_it_is() {
    check_rows_scene("computer rows: a door that lists the provider is read as it is");
}

#[test]
fn image_head_none_sealed_refuses_with_no_image_yet() {
    check_rows_scene("image head: none sealed refuses with no image yet");
}

#[test]
fn tree_records_a_thread_s_tree_by_root_on_live_workspaces_it_reaches() {
    check_rows_scene("tree records: a thread's tree by root, on live workspaces it reaches");
}

/// runtime-create.test.ts:210: until a create is done its workspace is not reachable, and reachable once it is. The
/// refusal reads as a workspace this host does not hold; `being_made` is the one door that opens early.
#[test]
fn until_ready_a_creating_workspace_is_neither_listed_nor_reachable() {
    common::run_local(async {
        let w = world(json!({"c": {}}), json!({})).await;
        let entry = w.rt.live.borrow().get("c").cloned().unwrap();
        entry.borrow_mut().creating = true;
        let refused = w
            .area
            .entry_of("c", None, EntryAsk::default())
            .await
            .err()
            .expect("a creating workspace is refused");
        assert_eq!(refused.message, "no such workspace: c");
        assert_eq!(kind_word(&refused), json!("not-found"));
        let early = w
            .area
            .entry_of(
                "c",
                None,
                EntryAsk {
                    being_made: true,
                    ..EntryAsk::default()
                },
            )
            .await
            .expect("being made opens early");
        assert!(std::rc::Rc::ptr_eq(&early, &entry));
        entry.borrow_mut().creating = false;
        let ready = w
            .area
            .entry_of("c", None, EntryAsk::default())
            .await
            .expect("reachable once ready");
        assert!(std::rc::Rc::ptr_eq(&ready, &entry));
    });
}

/// spawn-guard.test.ts:565: an id this host does not hold reads a thread exactly as one outside its tree: the same
/// refusal with no id in it, and the same absence from a row door.
#[test]
fn an_id_this_host_does_not_hold_reads_as_one_outside_the_callers_tree() {
    common::run_local(async {
        let w = world(json!({"a": {}, "theirs": {}}), json!({})).await;
        let thread = thread_caller();
        let nobody = w
            .area
            .entry_of("ws_00000000", Some(&thread), EntryAsk::default())
            .await
            .err()
            .expect("an id nobody holds");
        assert_eq!(nobody.message, "no such workspace");
        assert!(!nobody.message.contains("ws_00000000"));
        assert_eq!(kind_word(&nobody), json!("not-found"));
        let person = w
            .area
            .entry_of("ws_00000000", None, EntryAsk::default())
            .await
            .err()
            .expect("the person reads the id");
        assert_eq!(person.message, "no such workspace: ws_00000000");
        let outside = w
            .area
            .entry_of_row(&row("t-b", "theirs"), Some(&thread), None, false)
            .await
            .expect("a door answer");
        let unheld = w
            .area
            .entry_of_row(&row("t-b", "ws_00000000"), Some(&thread), None, false)
            .await
            .expect("a door answer");
        assert!(
            outside.is_none() && unheld.is_none(),
            "a row outside the tree and a row nobody holds read alike"
        );
        let own = w
            .area
            .entry_of_row(&row("t-a-own", "a"), Some(&thread), None, false)
            .await
            .expect("a door answer");
        assert_eq!(own.as_ref().map(short), Some(json!("entry:a")));
    });
}

/// boot.ts:656: a row reaches once its thread is driven, then by the rule of its workspace: a row on no live record
/// reaches; a `talk` reaches by the tree where the caller talks to its tree; else the workspace's own act rule.
#[test]
fn a_row_reaches_by_its_thread_then_by_the_rule_of_its_workspace() {
    common::run_local(async {
        let w = world(json!({"a": {}}), json!({})).await;
        let thread = thread_caller();
        let caller = support::caller_json(Some(&thread));
        assert!(
            w.area
                .reaches_row(
                    &RowAt {
                        thread_id: Some("t-a-own".into()),
                        workspace_id: "none".into()
                    },
                    Some(&thread),
                    None
                )
                .expect("the tree answers"),
            "no live record reaches"
        );
        assert!(
            !w.area
                .reaches_row(
                    &RowAt {
                        thread_id: Some("t-b".into()),
                        workspace_id: "none".into()
                    },
                    Some(&thread),
                    None
                )
                .expect("the tree answers"),
            "a thread outside the tree is not driven"
        );
        assert!(
            w.area
                .reaches_row(
                    &RowAt {
                        thread_id: Some("t-a-own".into()),
                        workspace_id: "a".into()
                    },
                    Some(&thread),
                    Some(TreeTalk::Send)
                )
                .expect("the tree answers"),
            "by the workspace's act rule, talk or not, while the tree is not talked to"
        );
        w.rt.talks_to_tree.set(true);
        assert!(w
            .area
            .reaches_row(
                &RowAt {
                    thread_id: Some("t-a-own".into()),
                    workspace_id: "a".into()
                },
                Some(&thread),
                Some(TreeTalk::List)
            )
            .expect("the tree answers"));
        w.rt.talks_to_tree.set(false);
        assert_eq!(
            w.log.take(),
            vec![
                json!(["drivesThread", "t-a-own", caller]),
                json!(["drivesThread", "t-b", caller]),
                json!(["drivesThread", "t-a-own", caller]),
                json!(["actsOn", "a", caller]),
                json!(["drivesThread", "t-a-own", caller]),
            ]
        );
    });
}

/// boot.ts:1017: records of one workspace only when one is named.
#[test]
fn tree_records_name_only_the_workspace_asked() {
    common::run_local(async {
        let w = world(
            json!({"a": {}, "b": {}}),
            json!({"t-1": {"workspaceId": "a", "harness": "claude"}, "t-2": {"workspaceId": "b", "harness": "claude"}, "t-3": {"workspaceId": "b", "harness": "codex"}}),
        )
        .await;
        let ids = |v: Vec<(String, String, wsp_core::types::internal::ThreadRecord)>| {
            v.into_iter().map(|(t, ws, _)| (t, ws)).collect::<Vec<_>>()
        };
        assert_eq!(
            ids(w
                .area
                .tree_records(None, None, Some("b"))
                .expect("the tree answers")),
            vec![
                ("t-2".to_string(), "b".to_string()),
                ("t-3".to_string(), "b".to_string())
            ]
        );
        assert_eq!(
            ids(w
                .area
                .tree_records(None, None, None)
                .expect("the tree answers"))
            .len(),
            3
        );
    });
}

/// boot.ts:1038: a door that does not list the wired provider gets it appended under its own word.
#[test]
fn computer_rows_append_the_wired_provider_a_door_does_not_list() {
    common::run_local(async {
        let w = world(json!({}), json!({})).await;
        *w.rt.door_rows.borrow_mut() = Some(vec![ComputerRow {
            id: "here".into(),
            name: "Mac".into(),
            kind: PlaceKind::Computer,
        }]);
        let rows = w.area.computer_rows().await;
        let got: Vec<Value> = rows.iter().map(support::computer_json).collect();
        assert_eq!(
            got,
            vec![
                json!({"id": "here", "name": "Mac", "kind": "computer"}),
                json!({"id": "solari", "name": "solari", "kind": "provider"})
            ]
        );
    });
}

/// boot.ts:1051: the head of the default image is its snapshot id.
#[test]
fn image_head_is_the_default_images_snapshot() {
    common::run_local(async {
        let w = world(json!({}), json!({})).await;
        *w.rt.default_head.borrow_mut() = Some("snap_head".into());
        assert_eq!(
            w.area.image_head_or_none().await.expect("head"),
            Some("snap_head".to_string())
        );
        assert_eq!(w.area.image_head().await.expect("head"), "snap_head");
    });
}

/// boot.ts:1062: this computer and the wired provider land on no place; every other computer lands on the place the
/// door names for it.
#[test]
fn landing_place_names_a_place_only_for_a_joined_computer() {
    common::run_local(async {
        let w = world(json!({}), json!({})).await;
        w.rt.places_for
            .borrow_mut()
            .insert("box-1".into(), Some("place-box-1".into()));
        assert_eq!(w.area.landing_place("here").await.expect("here"), None);
        assert_eq!(w.area.landing_place("solari").await.expect("wired"), None);
        assert!(
            w.rt.extras.take().is_empty(),
            "the door is not asked for either"
        );
        assert_eq!(
            w.area
                .landing_place("box-1")
                .await
                .expect("a joined computer"),
            Some("place-box-1".to_string())
        );
        assert_eq!(w.rt.extras.take(), vec![json!(["placeFor", "box-1"])]);
    });
}
