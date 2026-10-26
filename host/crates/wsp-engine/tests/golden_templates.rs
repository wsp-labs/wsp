// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts "golden templates", "golden disk" and "golden size" (:874-1059): the template a seal promotes, the
//! names it carries, the ready wait, the template road's own calls, the disk each create asks for, and the size each
//! machine is built at.
#![allow(clippy::type_complexity)]
mod golden_roads_common;
mod golden_seal_support;

use golden_roads_common::*;
use golden_seal_support::*;
use serde_json::json;
use wsp_engine::errors::EngineError;
use wsp_engine::golden::{
    build_golden, prepare_builder, promote_version, seal_golden, templates_of, upgrade_builder, Builder, GoldenDelta, GoldenError,
    SealResult, TemplateWait,
};
use wsp_engine::golden_seam::GoldenImport;
use wsp_engine::machine::MachineSpec;
use wsp_engine::snapshot_names::golden_name;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::GoldenVersion;

fn prep(opts: wsp_engine::golden::PrepareBuilderOptions, clock: &Manual) -> Builder {
    run(prepare_builder(opts, clock)).expect("the builder is prepared")
}

fn seal_ok(builder: Builder, opts: wsp_engine::golden::SealGoldenOptions, clock: &Manual) -> SealResult {
    run(seal_golden(builder, opts, clock)).expect("the seal sealed")
}

fn seal_err(builder: Builder, opts: wsp_engine::golden::SealGoldenOptions, clock: &Manual) -> GoldenError {
    run(seal_golden(builder, opts, clock)).expect_err("the seal is refused")
}

fn promoting(stages: &[String]) -> Vec<String> {
    stages.iter().filter(|s| s.starts_with("promoting")).cloned().collect()
}

fn snapshotting(stages: &[String]) -> Vec<String> {
    stages.iter().filter(|s| s.starts_with("snapshotting")).cloned().collect()
}

fn pairs(v: &[(String, String)]) -> Vec<(&str, &str)> {
    v.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect()
}

/// The upgrade delta golden.test.ts:958 and :962 pass: a recipe hash, nothing retired.
fn delta(hash: &str) -> GoldenDelta {
    GoldenDelta {
        import: GoldenImport { recipe_hash: hash.to_string(), ..Default::default() },
        retired: Vec::new(),
        retired_on_image: Vec::new(),
    }
}

/// golden.test.ts:875 `FAST_WAIT`.
fn wait() -> TemplateWait {
    fast_wait()
}

// golden.test.ts:877
#[test]
fn on_a_backend_with_templates_the_seal_promotes_the_snapshot_under_wsp_host_golden_v_n_after_the_builder_is_killed_waits_for_ready_records_the_id_and_the_smoke_fork_boots_from_the_template(
) {
    let rec = recording_backend(Opts { templates: true, ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let mut po = prepare_opts(&rec, "echo setup");
    po.on_stage = Some(st.on_stage.clone());
    let builder = prep(po, &clock);
    let mut so = seal_opts(&rec, "claude --version");
    so.on_stage = Some(st.on_stage.clone());
    so.name = Some("work".to_string());
    so.template_ready_ms = Some(50);
    so.template_poll_ms = Some(1);
    let sealed = seal_ok(builder, so, &clock);
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "kill m1", "promote snap_wsp-h1-work-v1", "create m2", "kill m2"]);
    assert_eq!(pairs(&rec.state.promoted.borrow()), vec![("snap_wsp-h1-work-v1", "wsp-h1-work-v1")]);
    let created = rec.state.created();
    match_object(&created[1], json!({"kind": "sandbox", "template": "tpl_snap_wsp-h1-work-v1"}));
    assert!(created[1].from_snapshot.is_none());
    match_object(&sealed.version, json!({"version": 1, "snapshotId": "snap_wsp-h1-work-v1", "templateId": "tpl_snap_wsp-h1-work-v1"}));
    assert_eq!(
        sans_base(&st.all())[4..].to_vec(),
        vec![
            "snapshotting:syncing the disk",
            "snapshotting:snapshotting, usually under a minute",
            "promoting:saving the image",
            "promoting:the image is saved",
            "smoke-forking:claude --version",
            "smoke-forking:1 agent answers: Claude Code",
            "sealed:v1",
        ]
    );
}

// golden.test.ts:892
#[test]
fn the_snapshot_and_its_template_carry_the_same_name_the_host_and_the_golden_in_it_the_golden_defaulting_to_the_store_s_default_key_one_rule(
) {
    let rec = recording_backend(Opts { templates: true, ..Opts::default() });
    let clock = Manual::new();
    run(build_golden(build_opts(&rec, "true", "true"), &*clock)).expect("the golden builds");
    assert_eq!(pairs(&rec.state.promoted.borrow()).iter().map(|(_, n)| *n).collect::<Vec<_>>(), vec!["wsp-h1-default-v1"]);
    assert_eq!(rec.state.snapshots.borrow().clone(), vec!["wsp-h1-default-v1"]);
    assert_eq!(golden_name("h1", "default", 1).unwrap(), "wsp-h1-default-v1");
    let twelve = golden_name("9f3a1c2b", "default", 12).unwrap();
    assert_eq!(twelve, "wsp-9f3a1c2b-default-v12");
    assert!(
        twelve.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'),
        "{twelve} is lowercase letters, digits and dashes"
    );
}

// golden.test.ts:902
#[test]
fn a_template_still_building_is_read_again_until_ready_each_read_a_promoting_line() {
    let rec = recording_backend(Opts {
        templates: true,
        template_status: Some(Box::new(|_id: &str, nth: u32| if nth < 3 { "building" } else { "ready" })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.template_ready_ms = Some(50);
    so.template_poll_ms = Some(1);
    let sealed = seal_ok(builder, so, &clock);
    assert_eq!(sealed.version.template_id.as_deref(), Some("tpl_snap_wsp-h1-default-v1"));
    assert_eq!(
        promoting(&st.all()),
        vec![
            "promoting:saving the image",
            "promoting:saving the image, the provider says building; asking again",
            "promoting:saving the image, the provider says building; asking again",
            "promoting:the image is saved",
        ]
    );
}

// golden.test.ts:913
#[test]
fn a_rollback_the_provider_refuses_is_its_own_line_on_the_stage_that_failed_with_the_machine_named_in_left_the_failure_s_line_stays_the_failure_s_own(
) {
    let rec = recording_backend(Opts {
        templates: true,
        exec_results: results(vec![("claude --version", exec(1, "", "the fork did not boot"))]),
        refuse_kill: Some(Box::new(|id: &str, _nth: u32| -> Option<EngineError> {
            if id == "m2" {
                return Some(EngineError::Other {
                    message: "getaddrinfo ENOTFOUND api.getsolari.com".to_string(),
                    code: Some("ENOTFOUND".to_string()),
                });
            }
            None
        })),
        ..Opts::default()
    });
    let clock = Manual::new();
    let (on_stage, frames) = frame_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "claude --version");
    so.on_stage = Some(on_stage);
    so.template_ready_ms = Some(50);
    so.template_poll_ms = Some(1);
    let err = seal_err(builder, so, &clock);
    assert_message_has(&err.message(), "the fork did not boot");
    let frames = frames.borrow();
    let failed = frames.last().expect("a failed frame");
    assert_eq!(failed.stage, "failed");
    let detail = failed.detail.as_deref().expect("a detail on the failed frame");
    assert!(detail.starts_with("golden smoke failed ") && detail.ends_with("the fork did not boot"), "{detail}");
    assert!(!detail.contains("ENOTFOUND"), "{detail}");
    assert_eq!(failed.left, Some(vec!["m2".to_string()]));
    assert_eq!(
        frames[frames.len() - 2],
        Frame {
            stage: "smoke-forking".to_string(),
            detail: Some("the machine could not be removed and bills on: getaddrinfo ENOTFOUND api.getsolari.com".to_string()),
            left: None,
        }
    );
}

// golden.test.ts:927
#[test]
fn the_seal_s_lines_say_what_a_person_can_use_the_snapshot_s_size_read_off_the_builder_s_disk_and_that_the_image_is_being_saved_never_the_snapshot_s_or_the_template_s_name(
) {
    let used = wsp_engine::golden_seam::used_kb_cmd();
    let rec = recording_backend(Opts {
        templates: true,
        exec_results: results(vec![(used.as_str(), exec(0, "13631488\n", ""))]),
        ..Opts::default()
    });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.template_ready_ms = Some(50);
    so.template_poll_ms = Some(1);
    seal_ok(builder, so, &clock);
    assert_eq!(
        snapshotting(&st.all()),
        vec!["snapshotting:syncing the disk", "snapshotting:snapshotting about 13 GB, usually under a minute"]
    );
    assert_eq!(promoting(&st.all()), vec!["promoting:saving the image", "promoting:the image is saved"]);
    assert!(!st.all().iter().any(|s| s.contains("wsp-h1") || s.contains("tpl_")));
}

// golden.test.ts:937
#[test]
fn a_template_the_provider_fails_ends_the_seal_with_its_reason_before_any_smoke_fork_and_the_template_goes_before_the_snapshot() {
    let rec =
        recording_backend(Opts { templates: true, template_status: Some(Box::new(|_id: &str, _nth: u32| "failed")), ..Opts::default() });
    let clock = Manual::new();
    let st = stage_recorder();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.on_stage = Some(st.on_stage.clone());
    so.template_ready_ms = Some(50);
    so.template_poll_ms = Some(1);
    let err = seal_err(builder, so, &clock);
    assert_message_has(&err.message(), "the provider failed the template tpl_snap_wsp-h1-default-v1: restore copy failed");
    assert_eq!(rec.state.created().len(), 1);
    assert_eq!(*rec.state.deleted_templates.borrow(), vec!["tpl_snap_wsp-h1-default-v1"]);
    assert_eq!(rec.state.deleted_snapshots(), vec!["snap_wsp-h1-default-v1"]);
    assert_eq!(st.all().last().unwrap(), "failed:the provider failed the template tpl_snap_wsp-h1-default-v1: restore copy failed");
}

// golden.test.ts:948
#[test]
fn a_template_that_never_reads_ready_inside_the_wait_ends_the_seal_naming_the_last_status_and_the_wait() {
    let rec =
        recording_backend(Opts { templates: true, template_status: Some(Box::new(|_id: &str, _nth: u32| "building")), ..Opts::default() });
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "true");
    so.template_ready_ms = Some(20);
    so.template_poll_ms = Some(1);
    let err = seal_err(builder, so, &clock);
    assert_message_has(&err.message(), "the template tpl_snap_wsp-h1-default-v1 still reads building after 20ms");
}

// golden.test.ts:954
// Ported: the upgradeBuilder halves. The forkGolden calls at :957 and :961 have no Rust road and are not run here.
#[test]
fn forkgolden_and_the_update_s_builder_boot_a_durable_version_from_its_template_and_a_version_without_one_from_its_snapshot() {
    let rec = recording_backend(Opts { templates: true, ..Opts::default() });
    let clock = Manual::new();
    let built = run(build_golden(build_opts(&rec, "true", "true"), &*clock)).expect("the golden builds");
    let version: GoldenVersion = built.version.clone();
    run(upgrade_builder(upgrade_opts(&rec, version.clone(), delta("h2"), "true"), &*clock)).expect("the update's builder boots");
    let created = rec.state.created();
    assert_eq!(created[2].template.as_deref(), Some("tpl_snap_wsp-h1-default-v1"));
    assert_eq!(created[2].from_snapshot, None);

    let mut volatile: GoldenVersion = version.clone();
    volatile.template_id = None;
    run(upgrade_builder(upgrade_opts(&rec, volatile, delta("h3"), "true"), &*clock)).expect("the update's builder boots");
    let created = rec.state.created();
    assert!(created[3].template.is_none());
    assert_eq!(created[3].from_snapshot.as_deref(), Some("snap_wsp-h1-default-v1"));
}

// golden.test.ts:966
#[test]
fn a_backend_whose_capabilities_lack_templates_or_that_lacks_one_of_the_calls_has_no_template_road() {
    let rec = recording_backend(Opts { templates: true, ..Opts::default() });
    assert!(templates_of(rec.dyn_backend()).is_some());
    assert!(templates_of(recording().dyn_backend()).is_none());
    let partial = Wrap { no_templates: true, ..Wrap::over(&rec) }.arc();
    assert!(templates_of(partial).is_none());
}

// golden.test.ts:974
#[test]
fn promote_version_always_mints_a_fresh_template_under_this_host_s_name_never_adopting_one_by_name_and_counts_the_templates_that_already_carry_it(
) {
    let rec = recording_backend(Opts { templates: true, ..Opts::default() });
    rec.state.templates.borrow_mut().insert("tpl_his".to_string(), template("tpl_his", "wsp-default-v1", "ready"));
    rec.state.templates.borrow_mut().insert("tpl_other_host".to_string(), template("tpl_other_host", "wsp-h1-default-v1", "ready"));
    let road = templates_of(rec.dyn_backend()).expect("the template road");
    let clock = Manual::new();
    let first = run(promote_version(&*road, "snap_v1", "wsp-h1-default-v1", &wait(), &*clock)).expect("v1 promotes");
    assert_eq!(first.template_id, "tpl_snap_v1");
    assert_eq!(first.sharing, Some(1));
    let second = run(promote_version(&*road, "snap_v2", "wsp-h1-default-v2", &wait(), &*clock)).expect("v2 promotes");
    assert_eq!(second.template_id, "tpl_snap_v2");
    assert_eq!(second.sharing, Some(0));
    assert_eq!(pairs(&rec.state.promoted.borrow()), vec![("snap_v1", "wsp-h1-default-v1"), ("snap_v2", "wsp-h1-default-v2")]);
    assert!(rec.state.templates.borrow().contains_key("tpl_his"));
}

// golden.test.ts:985
#[test]
fn promote_version_deletes_the_template_it_promoted_when_the_ready_wait_fails_or_runs_out_so_nothing_stands_on_the_snapshot_unrecorded_and_the_failure_keeps_the_provider_s_words(
) {
    let clock = Manual::new();
    let failed =
        recording_backend(Opts { templates: true, template_status: Some(Box::new(|_id: &str, _nth: u32| "failed")), ..Opts::default() });
    let road = templates_of(failed.dyn_backend()).expect("the template road");
    let err = run(promote_version(&*road, "snap_v1", "wsp-h1-default-v1", &wait(), &*clock)).expect_err("the promote fails");
    assert_message_has(&err.to_string(), "the provider failed the template tpl_snap_v1: restore copy failed");
    assert_eq!(*failed.state.deleted_templates.borrow(), vec!["tpl_snap_v1"]);
    assert!(failed.state.templates.borrow().is_empty());

    let slow =
        recording_backend(Opts { templates: true, template_status: Some(Box::new(|_id: &str, _nth: u32| "building")), ..Opts::default() });
    let road = templates_of(slow.dyn_backend()).expect("the template road");
    let short = TemplateWait { ready_ms: Some(20), poll_ms: Some(1), on_status: None };
    let err = run(promote_version(&*road, "snap_v1", "wsp-h1-default-v1", &short, &*clock)).expect_err("the promote runs out");
    assert_message_has(&err.to_string(), "the template tpl_snap_v1 still reads building after 20ms");
    assert_eq!(*slow.state.deleted_templates.borrow(), vec!["tpl_snap_v1"]);
}

// golden.test.ts:995
#[test]
fn a_smoke_that_fails_after_the_template_read_ready_still_deletes_the_template_before_the_snapshot() {
    let rec = recording_backend(Opts { templates: true, exec_results: results(vec![("smoke", exec(2, "", "broken"))]), ..Opts::default() });
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let mut so = seal_opts(&rec, "smoke");
    so.template_ready_ms = Some(50);
    so.template_poll_ms = Some(1);
    let err = seal_err(builder, so, &clock);
    assert_message_has(&err.message(), "smoke failed");
    assert_eq!(*rec.state.deleted_templates.borrow(), vec!["tpl_snap_wsp-h1-default-v1"]);
    assert_eq!(rec.state.deleted_snapshots(), vec!["snap_wsp-h1-default-v1"]);
}

// golden.test.ts:1003
#[test]
fn promote_version_leaves_the_count_out_when_the_provider_will_not_give_the_listing_and_the_promote_still_lands() {
    let rec = recording_backend(Opts { templates: true, ..Opts::default() });
    let backend = Wrap { list_fails: true, ..Wrap::over(&rec) }.arc();
    let road = templates_of(backend).expect("the template road");
    let clock = Manual::new();
    let promoted = run(promote_version(&*road, "snap_v1", "wsp-h1-default-v1", &wait(), &*clock)).expect("the promote lands");
    assert_eq!(promoted.template_id, "tpl_snap_v1");
    assert_eq!(promoted.sharing, None);
}

// golden.test.ts:1013
// Ported without the fork: the builder, the smoke fork and the update's builder. The forkGolden create at :1017 has no
// Rust road, so the fourth disk the TS checks is not asked for here.
#[test]
fn every_create_asks_for_the_20_gb_disk_the_builder_the_smoke_fork_a_workspace_fork_and_an_upgrade_fork() {
    let rec = recording();
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    let sealed = seal_ok(builder, seal_opts(&rec, "true"), &clock);
    run(upgrade_builder(upgrade_opts(&rec, sealed.version.clone(), delta("h2"), "true"), &*clock)).expect("the update's builder boots");
    let disks: Vec<Option<f64>> = rec.state.created().iter().map(|c| c.disk_gb.as_ref().map(|d| d.0)).collect();
    assert_eq!(disks, vec![Some(BUILDER_DISK_GB); 3]);
    assert_eq!(BUILDER_DISK_GB, 20.0);
}

/// golden.test.ts:1025 `clamped`: a provider that clamps memory to 2048 MB whatever is asked.
fn clamped(spec: &MachineSpec) -> wsp_engine::machine::MachineShape {
    let cpu = spec.cpu.as_ref().map(|c| c.0).unwrap_or(2.0);
    shape(json!({"cpu": cpu, "memMb": 2048, "createdAt": "2026-09-02T00:00:00.000Z"}))
}

// golden.test.ts:1028
#[test]
fn prepare_asks_for_an_explicit_size_the_pricing_default_when_none_is_named_and_records_what_the_provider_built() {
    let rec = recording_backend(Opts { built: Some(Box::new(clamped)), ..Opts::default() });
    let clock = Manual::new();
    let builder = prep(prepare_opts(&rec, "true"), &clock);
    match_object(&rec.state.created()[0], json!({"cpu": 2, "memMb": 4096}));
    equal_json(&builder.size, json!({"cpu": 2, "memMb": 2048}));
}

// golden.test.ts:1035
#[test]
fn a_backend_that_cannot_describe_machines_is_taken_at_its_word_on_the_request() {
    let rec = recording();
    let clock = Manual::new();
    let mut po = prepare_opts(&rec, "true");
    po.size.mem_mb = Some(JsNumber::from(8192.0));
    let builder = prep(po, &clock);
    match_object(&rec.state.created()[0], json!({"cpu": 2, "memMb": 8192}));
    equal_json(&builder.size, json!({"cpu": 2, "memMb": 8192}));
}

// golden.test.ts:1042
#[test]
fn seal_forks_the_smoke_at_the_builder_s_size_and_records_that_size_on_the_version() {
    let rec = recording_backend(Opts { built: Some(Box::new(clamped)), ..Opts::default() });
    let clock = Manual::new();
    let mut po = prepare_opts(&rec, "true");
    po.size.mem_mb = Some(JsNumber::from(8192.0));
    let builder = prep(po, &clock);
    let sealed = seal_ok(builder, seal_opts(&rec, "true"), &clock);
    match_object(&rec.state.created()[1], json!({"fromSnapshot": "snap_wsp-h1-default-v1", "cpu": 2, "memMb": 2048}));
    equal_json(&sealed.version.size, json!({"cpu": 2, "memMb": 2048}));
}

// golden.test.ts:1050 is `forkGolden inherits the sealed size unless overridden`: it calls forkGolden only, which has
// no Rust road, so it is not ported.
