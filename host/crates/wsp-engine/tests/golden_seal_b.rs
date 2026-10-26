// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts "interactive golden: prepare then seal" (:575-872): the seal's vault, its sign-in refusals, the disk
//! sync before the snapshot, the kills, the snapshot retries and the size the seal records.
#![allow(clippy::type_complexity)]
mod golden_roads_common;
mod golden_seal_support;

use std::cell::RefCell;
use std::rc::Rc;

use golden_roads_common::*;
use golden_seal_support::*;
use serde_json::json;
use wsp_engine::disk_sync::DISK_SYNC_CMD;
use wsp_engine::errors::{classify, EngineError};
use wsp_engine::exec_detached::put::{machine_answer, INLINE_EXEC_MS};
use wsp_engine::golden::{build_golden, prepare_builder, seal_golden, Builder, GoldenError, SealResult};
use wsp_engine::golden_seam::used_kb_cmd;
use wsp_engine::machine::{MachineSpec, MachineState, SnapshotProgress};
use wsp_wire::views::golden_image::GoldenLogin;
use wsp_words::words::image::{credential_on_builder_line, ProviderAnswer};
use wsp_words::words::init::disk_sync_failed_line;

fn prep(opts: wsp_engine::golden::PrepareBuilderOptions, clock: &Manual) -> Builder {
    run(prepare_builder(opts, clock)).expect("the builder is prepared")
}

fn seal_ok(builder: Builder, opts: wsp_engine::golden::SealGoldenOptions, clock: &Manual) -> SealResult {
    run(seal_golden(builder, opts, clock)).expect("the seal sealed")
}

fn seal_err(builder: Builder, opts: wsp_engine::golden::SealGoldenOptions, clock: &Manual) -> GoldenError {
    run(seal_golden(builder, opts, clock)).expect_err("the seal is refused")
}

fn last_n(lines: &[String], n: usize) -> Vec<String> {
    lines[lines.len() - n..].to_vec()
}

fn snapshotting(stages: &[String]) -> Vec<String> {
    stages.iter().filter(|s| s.starts_with("snapshotting")).cloned().collect()
}

struct SnapFail {
    message: String,
    machine_id: String,
    attempts: u32,
    answer: ProviderAnswer,
    reading: wsp_words::words::image::BuilderReading,
    read_error: Option<String>,
}

fn snapshot_failed(err: GoldenError) -> SnapFail {
    match err {
        GoldenError::SnapshotFailed { message, machine_id, attempts, answer, builder_state, read_error } => {
            SnapFail { message, machine_id, attempts, answer, reading: builder_state, read_error }
        }
        other => panic!("want a snapshot failure, got {other:?}"),
    }
}

/// A backend whose `inline` answers every command the same way: `exec` records each command it is asked.
fn asking() -> (Rc<RefCell<Vec<String>>>, Opts) {
    let asked: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let sink = asked.clone();
    let opts = Opts {
        exec: Some(Box::new(move |cmd: &str| {
            sink.borrow_mut().push(cmd.to_string());
            Some(ok())
        })),
        ..Opts::default()
    };
    (asked, opts)
}

// golden.test.ts:575
#[test]
fn a_seal_that_names_no_vault_paths_takes_none_and_asks_the_builder_nothing_about_them() {
    let (asked, opts) = asking();
    let rec = recording_backend(opts);
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let sealed = seal_ok(b, seal_opts(&rec, "true"), &clock);
    assert!(sealed.vault.is_none());
    let asked = asked.borrow();
    assert!(!asked.iter().any(|c| c.starts_with("tar czf")));
    // The one probe such a seal runs is the guard's own, over the paths a builder may never hold.
    assert_eq!(asked.iter().filter(|c| c.starts_with("for p in ")).count(), 1);
}

// golden.test.ts:586
#[test]
fn a_vault_the_builder_would_not_hand_over_leaves_it_alive_and_unsnapshotted_as_a_refused_snapshot_does() {
    let rec = recording_backend(Opts {
        exec: Some(Box::new(|cmd: &str| {
            if cmd.starts_with("tar czf") {
                return Some(exec(2, "", "tar: cannot read"));
            }
            let out = if cmd.contains("/etc/profile.d/wsp-secrets.sh") { "/etc/profile.d/wsp-secrets.sh\n" } else { "" };
            Some(exec(0, out, ""))
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.vault_paths = Some(vec!["/etc/profile.d/wsp-secrets.sh".to_string()]);
    so.fetch = Some(fetch_of(vault_tgz()));
    let err = seal_err(b, so, &clock);
    assert_message_has(&err.message(), "vault export tar failed");
    assert_eq!(rec.state.timeline(), vec!["create m1"]);
    assert!(rec.state.killed().is_empty());
}

// golden.test.ts:596
#[test]
fn a_builder_that_will_not_say_which_vault_paths_it_holds_stops_the_seal_rather_than_sealing_an_image_with_no_sign_ins_in_it() {
    let rec = recording_backend(Opts {
        exec: Some(Box::new(|cmd: &str| {
            if cmd.starts_with("for p in ") && cmd.contains("/root/.codex/auth.json") {
                return Some(exec(127, "", "bash: for: command not found"));
            }
            Some(ok())
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.vault_paths = Some(vec!["/root/.codex/auth.json".to_string()]);
    so.fetch = Some(fetch_of(vault_tgz()));
    let err = seal_err(b, so, &clock);
    assert_message_has(&err.message(), "would not say which");
    assert_eq!(rec.state.timeline(), vec!["create m1"]);
    assert!(rec.state.killed().is_empty());
}

// golden.test.ts:606
#[test]
fn a_builder_holding_a_sign_in_file_is_refused_before_the_snapshot_nothing_is_sealed_and_the_builder_is_left_as_it_is() {
    let rec = recording_backend(Opts {
        exec: Some(Box::new(|cmd: &str| {
            if cmd.contains("/root/.claude-cfg/.credentials.json") {
                return Some(exec(0, "/root/.claude-cfg/.credentials.json\n", ""));
            }
            Some(ok())
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.vault_paths = Some(vec!["/etc/profile.d/wsp-secrets.sh".to_string()]);
    so.fetch = Some(fetch_of(vault_tgz()));
    let err = seal_err(b, so, &clock);
    let (message, paths) = match &err {
        GoldenError::CredentialOnBuilder { message, paths } => (message.clone(), paths.clone()),
        other => panic!("want a credential on the builder, got {other:?}"),
    };
    assert_eq!(paths, vec!["/root/.claude-cfg/.credentials.json"]);
    assert_eq!(message, credential_on_builder_line(&paths));
    assert_eq!(err.message(), message);
    assert_eq!(rec.state.timeline(), vec!["create m1"]);
    assert!(rec.state.killed().is_empty());
}

// golden.test.ts:619
#[test]
fn a_builder_holding_none_of_them_seals_and_the_paths_it_was_asked_about_are_the_catalog_s_own() {
    let (asked, opts) = asking();
    let rec = recording_backend(opts);
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let sealed = seal_ok(b, seal_opts(&rec, "true"), &clock);
    assert_eq!(sealed.version.version.0, 1.0);
    let asked = asked.borrow();
    let probe = asked.iter().find(|c| c.starts_with("for p in ")).expect("the guard's probe");
    for path in NEVER_IN_IMAGE {
        assert!(probe.contains(path), "the probe names {path}: {probe}");
    }
}

// golden.test.ts:629
#[test]
fn seal_syncs_the_builder_s_disk_before_it_asks_for_the_snapshot_and_says_so_on_the_snapshot_stage_first() {
    let order: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let (o_exec, o_snap) = (order.clone(), order.clone());
    let rec = recording_backend(Opts {
        exec: Some(Box::new(move |cmd: &str| {
            if cmd == DISK_SYNC_CMD {
                o_exec.borrow_mut().push("sync".to_string());
                return Some(exec(0, "Dirty: 0\nWriteback: 0\n", ""));
            }
            Some(ok())
        })),
        snapshot: Some(Box::new(move |_id: &str, _nth: u32| -> wsp_engine::errors::Result<()> {
            o_snap.borrow_mut().push("snapshot".to_string());
            Ok(())
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    seal_ok(b, so, &clock);
    assert_eq!(*order.borrow(), vec!["sync", "snapshot"]);
    let syncs: Vec<(String, String, Option<u64>)> =
        rec.state.inline.borrow().iter().filter(|(_, c, _)| c == DISK_SYNC_CMD).cloned().collect();
    assert_eq!(syncs, vec![("m1".to_string(), DISK_SYNC_CMD.to_string(), Some(INLINE_EXEC_MS))]);
    assert_eq!(snapshotting(&st.all()), vec!["snapshotting:syncing the disk", "snapshotting:snapshotting, usually under a minute"]);
}

// golden.test.ts:646
#[test]
fn a_sync_the_builder_fails_stops_the_seal_before_the_snapshot_and_leaves_the_builder_alive_the_sentence_on_the_failed_stage() {
    let answer = || exec(1, "", "sync: Input/output error");
    let rec = recording_backend(Opts { exec_results: results(vec![(DISK_SYNC_CMD, answer())]), ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    let err = seal_err(b, so, &clock);
    assert!(matches!(err, GoldenError::DiskSync { .. }), "{err:?}");
    assert_eq!(rec.state.timeline(), vec!["create m1"]);
    assert!(rec.state.killed().is_empty());
    assert_eq!(st.all().last().unwrap(), &format!("failed:{}", disk_sync_failed_line(&machine_answer(&answer()))));
}

// golden.test.ts:658
#[test]
fn a_disk_a_writer_keeps_dirty_past_the_sync_seals_anyway_with_what_was_left_said_on_the_snapshot_stage() {
    let rec = recording_backend(Opts {
        exec_results: results(vec![(DISK_SYNC_CMD, exec(0, "Dirty: 12288\nWriteback: 0\n", ""))]),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    let sealed = seal_ok(b, so, &clock);
    assert_eq!(sealed.version.snapshot_id, "snap_wsp-h1-default-v1");
    assert!(rec.state.timeline().iter().any(|t| t == "snapshot m1"));
    assert_eq!(
        snapshotting(&st.all()),
        vec![
            "snapshotting:syncing the disk",
            "snapshotting:synced, Dirty 12 MB and Writeback 0 B remain; a writer is still running",
            "snapshotting:snapshotting, usually under a minute",
        ]
    );
}

// golden.test.ts:672
#[test]
fn seal_records_the_disk_the_snapshot_took_read_once_for_the_stage_line_and_the_version() {
    let used = used_kb_cmd();
    let rec = recording_backend(Opts { exec_results: results(vec![(used.as_str(), exec(0, "13631488\n", ""))]), ..Opts::default() });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let sealed = seal_ok(b, seal_opts(&rec, "true"), &clock);
    match_object(&sealed.version, json!({"usedBytes": 13631488u64 * 1024}));
}

// golden.test.ts:679
#[test]
fn a_used_column_of_zero_is_a_disk_nothing_has_been_written_to_not_a_df_that_could_not_be_read() {
    let used = used_kb_cmd();
    let rec = recording_backend(Opts { exec_results: results(vec![(used.as_str(), exec(0, "0\n", ""))]), ..Opts::default() });
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let sealed = seal_ok(b, seal_opts(&rec, "true"), &clock);
    match_object(&sealed.version, json!({"usedBytes": 0}));
}

// golden.test.ts:686
#[test]
fn seal_stamps_the_logins_it_is_given_onto_the_version_name_and_state_only() {
    let rec = recording();
    let clock = Manual::new();
    let b = prep(prepare_opts(&rec, "true"), &clock);
    let logins: Vec<GoldenLogin> = serde_json::from_value(json!([
        {"name": "GitHub CLI login", "state": "signed-in"},
        {"name": "Codex login", "state": "skipped"},
    ]))
    .expect("two logins");
    let mut so = seal_opts(&rec, "true");
    so.logins = Some(logins.clone());
    let sealed = seal_ok(b, so, &clock);
    equal_json(
        &sealed.version.logins,
        json!([
            {"name": "GitHub CLI login", "state": "signed-in"},
            {"name": "Codex login", "state": "skipped"},
        ]),
    );
    let b2 = prep(prepare_opts(&rec, "true"), &clock);
    let sealed2 = seal_ok(b2, seal_opts(&rec, "true"), &clock);
    assert!(sealed2.version.logins.is_none());
}

// golden.test.ts:696
#[test]
fn seal_retries_a_kill_the_provider_accepted_without_acting_on_and_forks_only_once_the_builder_reads_gone() {
    let rec = recording_backend(Opts { ignore_kill: Some(Box::new(|id: &str, nth: u32| id == "m1" && nth == 1)), ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.kill_confirm = Some(fast_kill());
    so.on_stage = Some(st.on_stage.clone());
    let sealed = seal_ok(builder.clone(), so, &clock);
    assert_eq!(sealed.version.version.0, 1.0);
    assert_eq!(st.all().last().unwrap(), "sealed:v1");
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "kill m1", "kill m1", "create m2", "kill m2"]);
    assert!(matches!(run(builder.machine.state()).expect("the builder reads"), MachineState::Gone));
}

// golden.test.ts:707
#[test]
fn seal_fails_with_a_typed_error_when_the_builder_outlives_three_kills_and_never_boots_the_smoke_fork() {
    let rec = recording_backend(Opts { ignore_kill: Some(Box::new(|_id: &str, _nth: u32| true)), ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.kill_confirm = Some(fast_kill());
    so.on_stage = Some(st.on_stage.clone());
    let err = seal_err(builder, so, &clock);
    let message = err.message();
    let GoldenError::Engine(engine) = err else { panic!("want a machine that is still alive, got {err:?}") };
    assert!(matches!(&engine, EngineError::MachineAlive { machine_id, state: MachineState::Running } if machine_id == "m1"), "{engine:?}");
    let host: wsp_words::exit::HostError = engine.into();
    assert_eq!(host.kind.as_ref().map(|k| k.as_str()), Some("machineAlive"));
    assert!(message.contains("m1"), "{message}");
    assert_eq!(rec.state.created().len(), 1);
    assert!(rec.state.timeline().iter().filter(|t| *t == "kill m1").count() >= 2);
    assert_eq!(rec.state.deleted_snapshots(), vec!["snap_wsp-h1-default-v1"]);
    assert!(st.all().last().unwrap().starts_with("failed:"));
}

// golden.test.ts:721
#[test]
fn a_smoke_fork_that_outlives_its_kills_still_seals_the_proven_image_and_the_sealed_stage_names_the_leak() {
    let rec = recording_backend(Opts { ignore_kill: Some(Box::new(|id: &str, _nth: u32| id == "m2")), ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.kill_confirm = Some(fast_kill());
    so.on_stage = Some(st.on_stage.clone());
    let sealed = seal_ok(builder, so, &clock);
    assert_eq!(sealed.manifest.head.0, 1.0);
    assert!(rec.state.deleted_snapshots().is_empty());
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "kill m1", "create m2", "kill m2", "kill m2", "kill m2"]);
    let last = st.all().last().unwrap().clone();
    assert!(last.starts_with("sealed:v1; machine m2 is still running after three kills"), "{last}");
}

// golden.test.ts:732
#[test]
fn a_backend_that_refuses_the_seal_s_snapshot_as_not_first_life_ends_it_with_that_error_and_the_builder_is_left_as_it_was() {
    let rec = recording();
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut stale = builder.clone();
    stale.first_life = false;
    let err = seal_err(stale, seal_opts(&rec, "true"), &clock);
    let GoldenError::Engine(engine) = err else { panic!("want a not-first-life refusal, got {err:?}") };
    assert!(matches!(&engine, EngineError::NotFirstLife { machine_id, .. } if machine_id == "m1"), "{engine:?}");
    let host: wsp_words::exit::HostError = engine.into();
    assert_eq!(host.kind.as_ref().map(|k| k.as_str()), Some("notFirstLife"));
    assert!(rec.state.snapshots.borrow().is_empty());
    assert_eq!(rec.state.timeline(), vec!["create m1"]);
}

// golden.test.ts:743
#[test]
fn seals_a_builder_that_was_resumed_on_a_backend_whose_snapshots_are_a_copy_of_the_disk() {
    let rec = recording_backend(Opts { snapshots_any_life: true, ..Opts::default() });
    let clock = Manual::new();
    let mut builder = prep(prepare_opts(&rec, "true"), &clock);
    builder.first_life = false;
    let sealed = seal_ok(builder, seal_opts(&rec, "true"), &clock);
    assert_eq!(sealed.manifest.head.0, 1.0);
    assert_eq!(*rec.state.snapshots.borrow(), vec!["wsp-h1-default-v1"]);
}

// golden.test.ts:751
#[test]
fn asks_for_no_root_disk_on_a_backend_whose_machines_take_no_disk_request_and_boots_from_the_image_it_names() {
    let rec = recording_backend(Opts {
        no_disk: true,
        base_templates: Some(("ubuntu:24.04".to_string(), "ubuntu:24.04".to_string())),
        ..Opts::default()
    });
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    seal_ok(builder, seal_opts(&rec, "true"), &clock);
    let created = rec.state.created();
    assert!(created[0].disk_gb.is_none());
    assert_eq!(created[0].template.as_deref(), Some("ubuntu:24.04"));
    assert!(created[1].disk_gb.is_none());
}

// golden.test.ts:764
#[test]
fn a_snapshot_the_provider_refuses_with_its_502_is_asked_for_three_times_while_the_builder_reads_running_each_attempt_a_stage_line_with_the_provider_s_answer_then_the_seal_fails_typed_and_the_builder_is_left_as_it_was(
) {
    let rec = recording_backend(Opts {
        snapshot: Some(Box::new(|_id: &str, nth: u32| -> wsp_engine::errors::Result<()> { Err(refused_snapshot(&format!("req_{nth}"))) })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "a"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.snapshot_retry_ms = Some(1);
    let f = snapshot_failed(seal_err(builder, so, &clock));
    assert_eq!(f.attempts, 3);
    assert_eq!(f.machine_id, "m1");
    assert_eq!(reading_word(&f.reading), "running");
    assert_eq!(f.answer.status, 502.0);
    assert_eq!(f.answer.message, "Failed to snapshot sandbox");
    assert_eq!(f.answer.request_id.as_deref(), Some("req_3"));
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "snapshot m1", "snapshot m1"]);
    assert!(rec.state.killed().is_empty());
    assert_eq!(
        last_n(&sans_base(&st.all()), 4),
        vec![
            "snapshotting:snapshotting, usually under a minute",
            "snapshotting:attempt 1 of 3 answered 502 Failed to snapshot sandbox (request req_1); the builder reads running, next attempt in 1ms",
            "snapshotting:attempt 2 of 3 answered 502 Failed to snapshot sandbox (request req_2); the builder reads running, next attempt in 1ms",
            "failed:the snapshot failed 3 times: the provider answered 502 Failed to snapshot sandbox (request req_3) while the builder read running",
        ]
    );
}

// golden.test.ts:781
#[test]
fn a_snapshot_that_lands_on_the_second_attempt_seals_as_any_other() {
    let rec = recording_backend(Opts {
        snapshot: Some(Box::new(|_id: &str, nth: u32| -> wsp_engine::errors::Result<()> {
            if nth == 1 {
                return Err(refused_snapshot("req_1"));
            }
            Ok(())
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "a"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.snapshot_retry_ms = Some(1);
    let sealed = seal_ok(builder, so, &clock);
    assert_eq!(sealed.version.snapshot_id, "snap_wsp-h1-default-v1");
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "snapshot m1", "kill m1", "create m2", "kill m2"]);
    assert_eq!(st.all().last().unwrap(), "sealed:v1");
}

// golden.test.ts:791
#[test]
fn a_refused_snapshot_on_a_builder_the_provider_answers_404_for_ends_at_the_first_attempt_nothing_is_killed_and_the_error_says_the_provider_no_longer_has_it(
) {
    let slot: Rc<RefCell<Option<Rc<State>>>> = Rc::new(RefCell::new(None));
    let hook_slot = slot.clone();
    let rec = recording_backend(Opts {
        snapshot: Some(Box::new(move |id: &str, _nth: u32| -> wsp_engine::errors::Result<()> {
            if let Some(state) = hook_slot.borrow().as_ref() {
                state.gone.borrow_mut().insert(id.to_string());
            }
            Err(refused_snapshot("req_1"))
        })),
        ..Opts::default()
    });
    *slot.borrow_mut() = Some(rec.state.clone());
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "a"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.snapshot_retry_ms = Some(1);
    let f = snapshot_failed(seal_err(builder, so, &clock));
    assert_eq!(f.attempts, 1);
    assert_eq!(reading_word(&f.reading), "gone");
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1"]);
    assert!(rec.state.killed().is_empty());
    assert_eq!(
        st.all().last().unwrap(),
        "failed:the snapshot failed 1 time: the provider answered 502 Failed to snapshot sandbox (request req_1) and no longer has the builder (404)"
    );
}

// golden.test.ts:802
#[test]
fn a_refused_snapshot_whose_read_of_the_builder_fails_with_anything_but_404_ends_at_once_with_the_builder_unread_and_untouched() {
    let rec = recording_backend(Opts {
        snapshot: Some(Box::new(|_id: &str, _nth: u32| -> wsp_engine::errors::Result<()> { Err(refused_snapshot("req_1")) })),
        state: Some(Box::new(|_id: &str| -> wsp_engine::errors::Result<()> {
            Err(EngineError::Provider(classify(503, None, Some("upstream sad"), None)))
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "a"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.snapshot_retry_ms = Some(1);
    let f = snapshot_failed(seal_err(builder, so, &clock));
    assert_eq!(f.attempts, 1);
    assert_eq!(reading_word(&f.reading), "unread");
    assert_eq!(f.read_error.as_deref(), Some("upstream sad"));
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1"]);
    assert!(rec.state.killed().is_empty());
    assert_eq!(
        st.all().last().unwrap(),
        "failed:the snapshot failed 1 time: the provider answered 502 Failed to snapshot sandbox (request req_1) and could not be read about the builder (upstream sad)"
    );
}

// golden.test.ts:813
#[test]
fn any_other_snapshot_failure_is_not_asked_again_and_leaves_the_builder_as_it_was_a_snapshot_that_failed_changed_nothing_on_it() {
    let rec = recording_backend(Opts {
        snapshot: Some(Box::new(|_id: &str, _nth: u32| -> wsp_engine::errors::Result<()> {
            Err(EngineError::Provider(classify(503, None, Some("upstream sad"), None)))
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "a"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.snapshot_retry_ms = Some(1);
    let f = snapshot_failed(seal_err(builder, so, &clock));
    assert_eq!(reading_word(&f.reading), "running");
    assert_eq!(
        f.message,
        format!("the snapshot failed 1 time: the provider answered 503 upstream sad (no request id from the provider, at {}) while the builder read running", f.answer.at)
    );
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1"]);
    assert!(rec.state.killed().is_empty());
}

// golden.test.ts:824
#[test]
fn a_snapshot_job_that_failed_on_a_computer_you_own_with_no_status_of_its_own_is_reported_as_the_provider_s_500_and_leaves_the_builder_the_next_seal_snapshots_the_same_builder(
) {
    let rec = recording_backend(Opts {
        snapshot: Some(Box::new(|_id: &str, nth: u32| -> wsp_engine::errors::Result<()> {
            if nth == 1 {
                return Err(EngineError::Other { message: "machine.snapshotJob on spoo was not answered in 300s".to_string(), code: None });
            }
            Ok(())
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "a"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.snapshot_retry_ms = Some(1);
    let f = snapshot_failed(seal_err(builder.clone(), so, &clock));
    assert_eq!(f.answer.status, 500.0);
    assert_eq!(f.answer.message, "machine.snapshotJob on spoo was not answered in 300s");
    assert!(rec.state.killed().is_empty());
    let mut again = seal_opts(&rec, "true");
    again.snapshot_retry_ms = Some(1);
    let sealed = seal_ok(builder, again, &clock);
    assert_eq!(sealed.version.snapshot_id, "snap_wsp-h1-default-v1");
    assert_eq!(rec.state.timeline()[..3], ["create m1", "snapshot m1", "snapshot m1"]);
}

// golden.test.ts:836
#[test]
fn what_the_provider_says_of_the_snapshot_as_it_writes_it_reaches_the_stage_as_bytes_written_over_the_whole_once_it_is_counted() {
    let rec = recording_backend(Opts {
        snapshot_progress: vec![
            SnapshotProgress { bytes: 0, total: None },
            SnapshotProgress { bytes: 1024 * MIB, total: Some(5 * 1024 * MIB) },
            SnapshotProgress { bytes: 5 * 1024 * MIB + 90 * MIB, total: Some(5 * 1024 * MIB) },
        ],
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    seal_ok(builder, so, &clock);
    assert_eq!(
        snapshotting(&st.all()),
        vec![
            "snapshotting:syncing the disk",
            "snapshotting:snapshotting, usually under a minute",
            "snapshotting:snapshotting, 0 B written",
            "snapshotting:snapshotting, 1 GB of about 5 GB written",
            "snapshotting:snapshotting, 5.1 GB of about 5 GB written",
        ]
    );
}

// golden.test.ts:850
#[test]
fn the_snapshot_s_size_is_what_the_backend_says_the_machine_wrote_where_it_can_say_and_df_inside_is_never_asked_then() {
    let used = used_kb_cmd();
    let rec = recording_backend(Opts {
        exec_results: results(vec![(used.as_str(), exec(0, "50000000\n", ""))]),
        built: Some(Box::new(|_spec: &MachineSpec| shape(json!({"cpu": 2, "memMb": 4096, "usedBytes": 5284823040u64})))),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    let sealed = seal_ok(builder, so, &clock);
    match_object(&sealed.version, json!({"usedBytes": 5284823040u64}));
    assert_eq!(
        snapshotting(&st.all()),
        vec!["snapshotting:syncing the disk", "snapshotting:snapshotting about 4.9 GB, usually under a minute"]
    );
    assert!(!rec.state.inline.borrow().iter().any(|(_, c, _)| c == &used));
}

// golden.test.ts:860
#[test]
fn a_failed_seal_kills_every_machine_drops_the_snapshot_and_leaves_the_prior_manifest_untouched() {
    let rec = recording_backend(Opts { exec_results: results(vec![("smoke", exec(2, "", "broken"))]), ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let one = run(build_golden(build_opts(&rec, "a", "true"), &*clock)).expect("the first version seals");
    let before = serde_json::to_value(&one.manifest).unwrap();
    let builder = prep(prepare_opts(&rec, "b"), &clock);
    let mut so = seal_opts(&rec, "smoke");
    so.manifest = Some(one.manifest.clone());
    so.on_stage = Some(st.on_stage.clone());
    let err = seal_err(builder, so, &clock);
    assert_message_has(&err.message(), "smoke failed");
    assert_eq!(serde_json::to_value(&one.manifest).unwrap(), before);
    assert_eq!(rec.state.killed(), vec!["m1", "m2", "m3", "m4"]);
    assert_eq!(rec.state.deleted_snapshots(), vec!["snap_wsp-h1-default-v2"]);
    assert!(st.all().last().unwrap().starts_with("failed:golden smoke failed"));
}
