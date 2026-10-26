// SPDX-License-Identifier: AGPL-3.0-only
//! The live entry, its engine hooks and `drop`: the cases of machines_attach.md. The scene "drop" of
//! oracle/machines.oracle.ts is replayed here by its name (tests/fixtures/oracle/machines_phases.json); every other
//! test reads its expected values off machines.ts (the line is named) at 65f6b89c7. The engine workspace is driven
//! through its own calls (`nap`, `wake`, `upgrade`), since its hooks are the engine's to hold. Spec:
//! spec/machines_attach.md.

mod common;
#[path = "support/machines.rs"]
mod support;

use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};
use support::{from, BackendOpts, EntryOpts, FakeMachine, Opts, World};
use wsp_core::types::wiring::SizeAsk;
use wsp_engine::disk_sync::DISK_SYNC_CMD;
use wsp_engine::errors::EngineError;
use wsp_engine::lifecycle::WorkspacePhase as EnginePhase;
use wsp_engine::machine::{ExecOptions, Machine};
use wsp_machines::machines::machines::attach::engine_phase_of;
use wsp_machines::machines::machines::{MachinesConfig, Refusal};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_words::exit::{HostError, Kind};

fn running() -> wsp_core::types::wiring::WorkspaceRecord {
    support::record_of("running", json!({}), false)
}

fn machine_one() -> Arc<FakeMachine> {
    FakeMachine::quiet("m-1")
}

fn unreachable_line(machine: &str) -> (EngineError, String) {
    let line = format!("{machine} cannot be reached");
    (
        EngineError::machine_unreachable(machine, "no route", 503, &line),
        line,
    )
}

/// The status pushes since the last call.
fn emit_statuses(w: &World) -> Vec<Value> {
    let got = support::log_of(&w.log.take(), &["emitStatus"]);
    w.log.clear();
    got
}

/// machines.ts:274-300: a refusal that the provider cannot reach the machine marks the record and pushes the
/// sentence once; any answer takes the mark off with no push.
#[test]
fn an_unreachable_answer_marks_the_record_once_and_any_answer_clears_it() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let machine = machine_one();
        let entry = w.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        let watched = entry.borrow().machine.clone();
        let (err, line) = unreachable_line("m-1");

        machine.s.borrow_mut().exec.push_back(Err(err.clone()));
        let first = watched
            .exec("ls", ExecOptions::default())
            .await
            .expect_err("the refusal is rethrown");
        assert_eq!(first.to_string(), line);
        {
            let marks = w.rt.unreached.borrow();
            let mark = marks.get("ws_1").expect("the record is marked");
            assert_eq!(
                (mark.machine_id.as_str(), mark.line.as_str()),
                ("m-1", line.as_str())
            );
        }
        assert_eq!(
            emit_statuses(&w),
            vec![json!(["emitStatus", "ws_1", "unreachable", line])],
            "pushed once, with the sentence"
        );

        machine.s.borrow_mut().exec.push_back(Err(err.clone()));
        watched
            .exec("ls", ExecOptions::default())
            .await
            .expect_err("refused again");
        assert!(
            emit_statuses(&w).is_empty(),
            "a mark of this machine already stands: nothing more is pushed"
        );

        watched
            .exec("ls", ExecOptions::default())
            .await
            .expect("an answer");
        assert!(
            w.rt.unreached.borrow().is_empty(),
            "any answer takes the mark off"
        );
        assert!(
            emit_statuses(&w).is_empty(),
            "and pushes nothing: the next tick says it"
        );

        // A record that is not running is marked and not pushed.
        entry.borrow().record.borrow_mut().phase = WorkspacePhase::Napping;
        machine.s.borrow_mut().exec.push_back(Err(err));
        watched
            .exec("ls", ExecOptions::default())
            .await
            .expect_err("refused");
        assert!(w.rt.unreached.borrow().contains_key("ws_1"));
        assert!(
            emit_statuses(&w).is_empty(),
            "no push for a record that is not running"
        );
    })
}

/// machines.ts:293-296: the provider's word that the machine is stopped puts the record to napping, and the error is
/// still the caller's.
#[test]
fn a_stopped_answer_adopts_the_pause() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let machine = machine_one();
        let entry = w.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        let watched = entry.borrow().machine.clone();
        machine
            .s
            .borrow_mut()
            .exec
            .push_back(Err(EngineError::machine_stopped(
                "m-1",
                &support::plain("the machine is stopped"),
            )));

        let err = watched
            .exec("ls", ExecOptions::default())
            .await
            .expect_err("rethrown");
        assert_eq!(err.to_string(), "the machine is stopped");
        assert_eq!(
            entry.borrow().record.borrow().phase,
            WorkspacePhase::Napping,
            "the next verb wakes it"
        );
        assert_eq!(
            entry.borrow().record.borrow().napped_at,
            Some(JsNumber(common::T0 as f64))
        );
        let log = w.log_now();
        assert_eq!(
            support::log_of(&log, &["emit", "emitStatus"]),
            vec![
                json!(["emit", { "type": "workspace.napped", "workspaceId": "ws_1", "found": true }]),
                json!(["emitStatus", "ws_1", "napping", "paused outside wsp"]),
            ]
        );
    })
}

/// machines.ts:276-285: an error from a machine the entry no longer holds changes nothing.
#[test]
fn a_replaced_machines_errors_mark_nothing() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let old = machine_one();
        let entry = w
            .area
            .attach(&support::shared(running()), old.clone() as Arc<dyn Machine>);
        let watched_old = entry.borrow().machine.clone();
        entry.borrow_mut().machine = FakeMachine::quiet("m-2") as Arc<dyn Machine>;

        let (unreachable, _) = unreachable_line("m-1");
        old.s.borrow_mut().exec.push_back(Err(unreachable));
        watched_old
            .exec("ls", ExecOptions::default())
            .await
            .expect_err("rethrown");
        old.s
            .borrow_mut()
            .exec
            .push_back(Err(EngineError::machine_stopped(
                "m-1",
                &support::plain("stopped"),
            )));
        watched_old
            .exec("ls", ExecOptions::default())
            .await
            .expect_err("rethrown");

        assert!(
            w.rt.unreached.borrow().is_empty(),
            "no mark for a machine the entry dropped"
        );
        assert_eq!(
            entry.borrow().record.borrow().phase,
            WorkspacePhase::Running,
            "and no pause adopted"
        );
        assert!(support::log_of(&w.log_now(), &["emit", "emitStatus", "persist"]).is_empty());
    })
}

/// machines.ts:367-384.
#[test]
fn engine_phase_of_maps_every_phase() {
    let cases = [
        (WorkspacePhase::Running, EnginePhase::Running),
        (WorkspacePhase::Napping, EnginePhase::Napping),
        (WorkspacePhase::Waking, EnginePhase::Waking),
        (WorkspacePhase::Pausing, EnginePhase::Napping),
        (WorkspacePhase::Gone, EnginePhase::Napping),
        (WorkspacePhase::Queued, EnginePhase::Napping),
        (WorkspacePhase::Released, EnginePhase::Napping),
    ];
    for (phase, want) in cases {
        assert_eq!(engine_phase_of(phase), want, "{phase:?}");
    }
}

/// machines.ts:386-394: the record takes the engine's machine and life, loses `gone` and the unchecked mark, keeps
/// `diskGone`, and the daemon is synced without being waited for.
#[test]
fn follow_machine_moves_the_record_onto_the_engines_machine() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let e = w.entry_in(EntryOpts {
            phase: "napping",
            record: json!({ "gone": "words", "diskGone": "d", "machineId": "m-old", "firstLife": true }),
            ..Default::default()
        });
        e.entry.borrow_mut().unchecked = true;

        w.area.follow_machine(&e.entry);

        let r = e.entry.borrow();
        assert_eq!(r.record.borrow().phase, WorkspacePhase::Running);
        assert_eq!(
            r.record.borrow().machine_id,
            "m-1",
            "the engine workspace's machine"
        );
        assert!(
            !r.record.borrow().first_life,
            "the engine's own first life, which the entry was made with"
        );
        assert_eq!(r.record.borrow().gone, None);
        assert_eq!(
            r.record.borrow().disk_gone.as_deref(),
            Some("d"),
            "diskGone is recoverGone's to keep"
        );
        assert!(!r.unchecked);
        drop(r);
        assert_eq!(
            support::log_of(&w.log_now(), &["syncDaemon"]),
            vec![json!(["syncDaemon", "ws_1"])]
        );
    })
}

/// machines.ts:395-400, 144: a generation one past the live entry's; a merged or closed pull request stands as the
/// record kept it, read as of its end; a kept checkout is copied.
#[test]
fn attach_counts_generations_and_keeps_a_closed_pull_request() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let first = w.area.attach(
            &support::shared(running()),
            machine_one() as Arc<dyn Machine>,
        );
        assert_eq!(first.borrow().generation, 0);
        let second = w.area.attach(
            &support::shared(running()),
            machine_one() as Arc<dyn Machine>,
        );
        assert_eq!(second.borrow().generation, 1, "one past the live entry's");
        assert!(
            Rc::ptr_eq(w.rt.live.borrow().get("ws_1").unwrap(), &second),
            "set in live, replacing the first"
        );
        assert!(!Rc::ptr_eq(&first, &second));

        let pr = |state: &str, extra: Value| {
            let mut v =
                json!({ "number": 5, "url": "https://x/pull/5", "state": state, "base": "main" });
            for (k, x) in extra.as_object().unwrap() {
                v[k] = x.clone();
            }
            v
        };
        let seen = |extras: Value| {
            let w = &w;
            let record = support::record_of("running", extras, false);
            let entry = w
                .area
                .attach(&support::shared(record), machine_one() as Arc<dyn Machine>);
            let held = entry.borrow().pr.clone();
            held.map(|p| support::norm(&serde_json::to_value(p).unwrap()))
        };

        assert_eq!(
            seen(json!({ "pr": pr("merged", json!({ "mergedAt": 111, "closedAt": 99 })) })),
            Some(
                json!({ "number": 5, "url": "https://x/pull/5", "state": "merged", "base": "main", "mergedAt": 111, "closedAt": 99, "readAt": 111 })
            ),
            "mergedAt first"
        );
        assert_eq!(
            seen(json!({ "pr": pr("closed", json!({ "closedAt": 99 })) }))
                .map(|p| p["readAt"].clone()),
            Some(json!(99)),
            "then closedAt"
        );
        assert_eq!(
            seen(json!({ "pr": pr("closed", json!({})) })).map(|p| p["readAt"].clone()),
            Some(json!(0)),
            "else 0"
        );
        assert_eq!(
            seen(json!({ "pr": pr("open", json!({})) })),
            None,
            "an open one is read again, so nothing is kept"
        );

        let checkout =
            json!({ "branch": "main", "ahead": 1, "behind": 0, "changed": 2, "readAt": 7 });
        let record = support::record_of("running", json!({ "checkout": checkout }), false);
        let entry = w
            .area
            .attach(&support::shared(record), machine_one() as Arc<dyn Machine>);
        let held = entry.borrow().checkout.clone();
        assert_eq!(
            held.map(|c| support::norm(&serde_json::to_value(c).unwrap())),
            Some(checkout)
        );
    })
}

/// machines.ts:39-46, 58-104: a computer that keeps no image forks none: the marks of a vault it never had go from the
/// record, and a nap reads and stores nothing.
#[test]
fn a_computer_that_keeps_no_image_gets_no_vault_hooks_and_loses_its_vault_marks() {
    common::run_local(async {
        let marked = || {
            support::record_of(
                "running",
                json!({ "vaultedAt": "2026-09-01T00:00:00.000Z", "vaultRefused": "old" }),
                false,
            )
        };

        let bare = World::new(Opts::default());
        let machine = machine_one();
        let entry = bare.area.attach(
            &support::shared(marked()),
            machine.clone() as Arc<dyn Machine>,
        );
        assert_eq!(
            (
                entry.borrow().record.borrow().vaulted_at.clone(),
                entry.borrow().record.borrow().vault_refused.clone()
            ),
            (None, None)
        );
        support::ws_nap(&entry).await.expect("a nap lands");
        assert_eq!(
            machine.s.borrow_mut().pauses,
            vec![None],
            "the pause is the machine's own"
        );
        assert!(
            machine.execs().is_empty(),
            "no disk sync: there is no stash"
        );
        assert!(
            bare.rt.cfg.borrow().vault_calls.is_empty(),
            "nothing read off it"
        );
        assert!(
            support::log_of(&bare.log_now(), &["store.putBlob"]).is_empty(),
            "and nothing stored"
        );

        // The same record on a computer that keeps images keeps its marks and stashes before the pause.
        let keeping = World::new(Opts::default());
        keeping.rt.cfg.borrow_mut().keeps_images = true;
        let machine = machine_one();
        let entry = keeping.area.attach(
            &support::shared(marked()),
            machine.clone() as Arc<dyn Machine>,
        );
        assert_eq!(
            entry.borrow().record.borrow().vaulted_at.as_deref(),
            Some("2026-09-01T00:00:00.000Z")
        );
        support::ws_nap(&entry).await.expect("a nap lands");
        assert_eq!(
            machine.execs(),
            vec![DISK_SYNC_CMD.to_string()],
            "the disk is synced first"
        );
        assert_eq!(keeping.rt.cfg.borrow().vault_calls.len(), 1);
    })
}

/// machines.ts:62-70: over the cap the export says so in one line and the replacement goes on with no backup; any
/// other failure is the replacement's; a sign-in never sits in a provider's vault.
#[test]
fn a_vault_over_the_cap_replaces_with_no_backup() {
    common::run_local(async {
        let w = World::new(Opts::default());
        {
            let mut cfg = w.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.names_workspace = true;
            cfg.vault_export.push_back(Err(HostError::with_kind(
                "vault is 5000 bytes, over the 1000 cap",
                Kind::VaultTooLarge,
            )));
        }
        let machine = machine_one();
        let entry = w.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );

        support::ws_upgrade(&entry, None)
            .await
            .expect("the replacement goes on");

        assert_eq!(
            w.warns().first().map(String::as_str),
            Some("vault for ws_1 not taken before the replacement: vault is 5000 bytes, over the 1000 cap; replacing with no backup"),
            "said first, in the one line that names the size and the cap"
        );
        assert_eq!(
            w.rt.cfg.borrow().vault_calls,
            vec![("m-1".to_string(), 1000, None)],
            "the cap is vaultCapBytes, and nothing is left out off a provider"
        );
        assert_eq!(
            entry.borrow().ws.machine_id(),
            "m-made",
            "the replacement is a fresh fork"
        );
        assert_eq!(
            support::log_of(&w.log_now(), &["gone.stop"]),
            vec![json!(["gone.stop", "m-1"])],
            "the old machine is retired"
        );

        // Any other failure is rethrown, with no warning.
        let other = World::new(Opts::default());
        {
            let mut cfg = other.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.vault_export
                .push_back(Err(HostError::new("the machine would not answer")));
        }
        let entry = other.area.attach(
            &support::shared(running()),
            machine_one() as Arc<dyn Machine>,
        );
        let err = support::ws_upgrade(&entry, None)
            .await
            .expect_err("rethrown");
        assert_eq!(err.to_string(), "the machine would not answer");
        assert!(other.warns().is_empty());

        // At a provider the export leaves the sign-in paths out.
        let provider = World::new(Opts::default());
        {
            let mut cfg = provider.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.names_workspace = true;
            cfg.at_provider = true;
            cfg.vault_export.push_back(Err(HostError::with_kind(
                "vault is 5000 bytes, over the 1000 cap",
                Kind::VaultTooLarge,
            )));
        }
        let entry = provider.area.attach(
            &support::shared(running()),
            machine_one() as Arc<dyn Machine>,
        );
        support::ws_upgrade(&entry, None)
            .await
            .expect("the replacement goes on");
        assert_eq!(
            provider.rt.cfg.borrow().vault_calls,
            vec![(
                "m-1".to_string(),
                1000,
                Some((*wsp_catalog::signin_rows::LOGIN_PATHS).clone())
            )]
        );
    })
}

/// machines.ts:76-98: a stash does nothing while a nap refusal stands; it syncs the disk and stops there where the
/// pause keeps the disk; otherwise it stores the vault and stamps the record.
#[test]
fn stash_vault_skips_while_a_refusal_stands_and_a_disk_keeping_pause() {
    common::run_local(async {
        // A refusal stands: the pause is still asked, with no sync and no vault.
        let refused = World::new(Opts::default());
        refused.rt.cfg.borrow_mut().keeps_images = true;
        refused.rt.nap_refusals.borrow_mut().insert(
            "ws_1".into(),
            Refusal {
                machine_id: "m-1".into(),
                said: "no pause".into(),
            },
        );
        let machine = machine_one();
        let entry = refused.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_nap(&entry).await.expect("a nap lands");
        assert!(machine.execs().is_empty());
        assert!(refused.rt.cfg.borrow().vault_calls.is_empty());
        assert_eq!(machine.s.borrow_mut().pauses.len(), 1);

        // A pause that keeps the disk: synced, then nothing read.
        let disk = World::new(Opts::default());
        {
            let mut cfg = disk.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.pause_keeps_disk = true;
        }
        let machine = machine_one();
        let entry = disk.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_nap(&entry).await.expect("a nap lands");
        assert_eq!(machine.execs(), vec![DISK_SYNC_CMD.to_string()]);
        assert!(disk.rt.cfg.borrow().vault_calls.is_empty());
        assert!(support::log_of(&disk.log_now(), &["store.putBlob"]).is_empty());

        // Otherwise: synced, exported under the cap, stored under vaults/<id>, stamped, and the refusal mark gone.
        let stored = World::new(Opts::default());
        stored.rt.cfg.borrow_mut().keeps_images = true;
        let machine = machine_one();
        let entry = stored.area.attach(
            &support::shared(support::record_of(
                "running",
                json!({ "vaultRefused": "earlier" }),
                false,
            )),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_nap(&entry).await.expect("a nap lands");
        assert_eq!(machine.execs(), vec![DISK_SYNC_CMD.to_string()]);
        assert_eq!(
            stored.rt.cfg.borrow().vault_calls,
            vec![("m-1".to_string(), 1000, None)]
        );
        assert_eq!(
            support::log_of(&stored.log_now(), &["store.putBlob"]),
            vec![json!(["store.putBlob", "vaults", "ws_1", "vault"])]
        );
        assert_eq!(
            entry.borrow().record.borrow().vaulted_at.as_deref(),
            Some("2026-09-21T14:13:20.000Z")
        );
        assert_eq!(entry.borrow().record.borrow().vault_refused, None);

        // A disk sync that fails is said and the nap goes on.
        let unsynced = World::new(Opts::default());
        unsynced.rt.cfg.borrow_mut().keeps_images = true;
        let machine = machine_one();
        machine
            .s
            .borrow_mut()
            .exec
            .push_back(Err(support::plain("exec refused")));
        let entry = unsynced.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_nap(&entry).await.expect("a nap lands");
        assert_eq!(
            unsynced.warns(),
            vec![
                "disk sync before the nap of ws_1 failed: exec refused; napping anyway".to_string()
            ]
        );
        assert_eq!(machine.s.borrow_mut().pauses.len(), 1);
    })
}

/// machines.ts:87-98: a stash that fails marks the record and says so, keeps the stamp of the last vault, and the nap
/// goes on.
#[test]
fn a_stash_that_fails_marks_vault_refused() {
    common::run_local(async {
        let w = World::new(Opts::default());
        {
            let mut cfg = w.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.vault_export
                .push_back(Err(HostError::new("export is too big")));
        }
        let machine = machine_one();
        let record = support::record_of(
            "running",
            json!({ "vaultedAt": "2026-09-01T00:00:00.000Z" }),
            false,
        );
        let entry = w.area.attach(
            &support::shared(record),
            machine.clone() as Arc<dyn Machine>,
        );

        support::ws_nap(&entry).await.expect("the nap goes on");

        assert_eq!(
            entry.borrow().record.borrow().vault_refused.as_deref(),
            Some("export is too big")
        );
        assert_eq!(
            entry.borrow().record.borrow().vaulted_at.as_deref(),
            Some("2026-09-01T00:00:00.000Z"),
            "the previous stamp stands"
        );
        assert_eq!(
            w.warns(),
            vec!["nap vault for ws_1 not stored, previous kept: export is too big".to_string()]
        );
        assert!(support::log_of(&w.log_now(), &["store.putBlob"]).is_empty());
        assert_eq!(machine.s.borrow_mut().pauses.len(), 1);
    })
}

fn waking_record() -> wsp_core::types::wiring::WorkspaceRecord {
    support::record_of("napping", json!({}), false)
}

fn one_wake() -> Opts {
    Opts {
        backend: Some(BackendOpts {
            life: Some((1, false, false)),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// machines.ts:109-120: a resume sets the host name again unless the backend names the workspace, goes through
/// `resume_at` when a resize is asked and the machine has it, and an unanswered resume says waking before the error
/// goes on; a pause carries the wait when one is set.
#[test]
fn a_resume_sets_the_hostname_again_and_an_unanswered_one_says_waking() {
    common::run_local(async {
        // The machine answers: resume, then the host name once (an exec), then the wake check passes.
        let w = World::new(one_wake());
        let machine = machine_one();
        let entry = w.area.attach(
            &support::shared(waking_record()),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_wake(&entry).await.expect("the wake lands");
        assert_eq!(machine.s.borrow_mut().resumes, vec![None]);
        let set = machine.execs();
        assert_eq!(set.len(), 1, "the name is set again after the resume");
        assert!(
            set[0].contains("w1"),
            "with the workspace's name: {}",
            set[0]
        );

        // A backend that names the workspace is not told its name again.
        let named = World::new(one_wake());
        named.rt.cfg.borrow_mut().names_workspace = true;
        let machine = machine_one();
        let entry = named.area.attach(
            &support::shared(waking_record()),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_wake(&entry).await.expect("the wake lands");
        assert!(machine.execs().is_empty());

        // A resize asked and a machine that can: resume_at with the size.
        let resized = World::new(one_wake());
        resized.rt.cfg.borrow_mut().names_workspace = true;
        let machine = machine_one();
        machine.s.borrow_mut().resume_at = true;
        let entry = resized.area.attach(
            &support::shared(waking_record()),
            machine.clone() as Arc<dyn Machine>,
        );
        entry.borrow_mut().resize_to = Some(SizeAsk {
            cpu: JsNumber(4.0),
            mem_mb: JsNumber(8192.0),
        });
        support::ws_wake(&entry).await.expect("the wake lands");
        assert_eq!(machine.s.borrow_mut().resumes, vec![Some((4.0, 8192))]);

        // A resume the provider never answered says waking on the row, then fails as it was.
        let silent = World::new(one_wake());
        silent.rt.cfg.borrow_mut().names_workspace = true;
        let machine = machine_one();
        machine.s.borrow_mut().resume_err = Some(EngineError::ResumeUnanswered {
            message: "no answer to the resume".into(),
            cause: None,
        });
        let entry = silent.area.attach(
            &support::shared(waking_record()),
            machine.clone() as Arc<dyn Machine>,
        );
        let err = support::ws_wake(&entry).await.expect_err("the wake fails");
        assert_eq!(err.to_string(), "no answer to the resume");
        assert_eq!(
            support::log_of(&silent.log_now(), &["saysWaking"]),
            vec![json!([
                "saysWaking",
                "ws_1",
                wsp_words::words::computer::RESUME_UNANSWERED
            ])]
        );

        // A pause carries the wait when the entry has one.
        let waits = World::new(Opts::default());
        let machine = machine_one();
        let entry = waits.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        entry.borrow_mut().nap_wait_ms = Some(5000);
        support::ws_nap(&entry).await.expect("a nap lands");
        let again = waits.area.attach(
            &support::shared(running()),
            machine.clone() as Arc<dyn Machine>,
        );
        support::ws_nap(&again).await.expect("a nap lands");
        assert_eq!(machine.s.borrow_mut().pauses, vec![Some(5000), None]);
    })
}

/// machines.ts:121-140: the wake check names the sizes created and the provider's view with every fault it finds, and
/// the daemon's fault carries them when both are known.
#[test]
fn the_wake_check_names_both_shapes_and_the_daemons_fault() {
    common::run_local(async {
        const BOTH: &str =
            r#"created as {"cpu":2,"memMb":4096}, provider view {"cpu":1,"memMb":4096}"#;
        const SAME: &str =
            r#"created as {"cpu":2,"memMb":4096}, provider view {"cpu":2,"memMb":4096}"#;
        let shaped = || {
            support::record_of(
                "napping",
                json!({ "shape": { "cpu": 2, "memMb": 4096 } }),
                false,
            )
        };

        // (record, what the machine describes, the daemon's fault, the line the wake fails with, the warning)
        type Case = (
            wsp_core::types::wiring::WorkspaceRecord,
            Option<Result<Value, &'static str>>,
            Option<&'static str>,
            String,
            Option<String>,
        );
        let cases: Vec<Case> = vec![
            (
                shaped(),
                Some(Err("timeout")),
                None,
                "provider view of m-1 unavailable (timeout)".into(),
                None,
            ),
            (
                shaped(),
                Some(Ok(json!({ "cpu": 1, "memMb": 4096 }))),
                None,
                format!("cpu 1 != 2 on m-1 ({BOTH})"),
                Some(format!("wake check on m-1: cpu 1 != 2; {BOTH}")),
            ),
            (
                shaped(),
                Some(Ok(json!({ "cpu": 2, "memMb": 4096 }))),
                Some("the daemon did not answer"),
                format!("the daemon did not answer ({SAME})"),
                None,
            ),
            (
                support::record_of("napping", json!({}), false),
                None,
                Some("the daemon did not answer"),
                "the daemon did not answer".into(),
                None,
            ),
            (
                shaped(),
                None,
                Some("the daemon did not answer"),
                "the daemon did not answer".into(),
                None,
            ),
        ];
        for (i, (record, describe, ping, want_fault, want_warn)) in cases.into_iter().enumerate() {
            let w = World::new(one_wake());
            w.rt.cfg.borrow_mut().names_workspace = true;
            w.rt.cfg.borrow_mut().ping = ping.map(String::from);
            let machine = machine_one();
            machine.s.borrow_mut().describe = describe.map(|d| d.map(from).map_err(support::plain));
            let entry = w.area.attach(
                &support::shared(record),
                machine.clone() as Arc<dyn Machine>,
            );
            let err = support::ws_wake(&entry)
                .await
                .expect_err("the check fails the wake");
            assert!(
                err.to_string().contains(&want_fault),
                "case {i}: {} does not hold {want_fault}",
                err
            );
            assert_eq!(
                w.warns(),
                want_warn.into_iter().collect::<Vec<_>>(),
                "case {i}"
            );
        }

        // Shapes that agree and a daemon that answers: the wake lands.
        let w = World::new(one_wake());
        w.rt.cfg.borrow_mut().names_workspace = true;
        let machine = machine_one();
        machine.s.borrow_mut().describe = Some(Ok(from(json!({ "cpu": 2, "memMb": 4096 }))));
        let entry = w
            .area
            .attach(&support::shared(shaped()), machine as Arc<dyn Machine>);
        support::ws_wake(&entry)
            .await
            .expect("a healthy machine wakes");
        assert!(w.warns().is_empty());
    })
}

/// machines.ts:148: the record's own window, then the place's, then the default; off is a value, not an absence.
#[test]
fn idle_window_of_takes_the_record_then_the_place_then_the_default() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let mut record = running();

        assert_eq!(
            w.area.idle_window_of(&record),
            Some(1_200_000.0),
            "the default"
        );
        w.rt.cfg.borrow_mut().place_nap_ms = Some(Some(600_000.0));
        assert_eq!(
            w.area.idle_window_of(&record),
            Some(600_000.0),
            "the place's"
        );
        w.rt.cfg.borrow_mut().place_nap_ms = Some(None);
        assert_eq!(
            w.area.idle_window_of(&record),
            None,
            "the place turns it off"
        );

        record.idle_window_ms = Some(Some(JsNumber(300_000.0)));
        assert_eq!(
            w.area.idle_window_of(&record),
            Some(300_000.0),
            "the record's own beats the place's"
        );
        w.rt.cfg.borrow_mut().place_nap_ms = Some(Some(600_000.0));
        record.idle_window_ms = Some(None);
        assert_eq!(
            w.area.idle_window_of(&record),
            None,
            "a record that turns it off stays off over the place's window"
        );

        let off = World::new(Opts {
            config: Some(MachinesConfig {
                default_idle_window_ms: None,
                ..support::config(0)
            }),
            ..Default::default()
        });
        assert_eq!(
            off.area.idle_window_of(&running()),
            None,
            "no default is auto-nap off"
        );
    })
}

/// machines.ts:151-154: every row of the workspace ends in map order, then every exec; another workspace's rows stay.
#[test]
fn end_sessions_ends_every_row_of_the_workspace_in_map_order_then_its_execs() {
    common::run_local(async {
        let w = World::new(Opts::default());
        w.rt.sessions
            .borrow_mut()
            .insert("s1".into(), w.session_row("s1", "ws_1", None));
        w.rt.sessions
            .borrow_mut()
            .insert("s2".into(), w.session_row("s2", "ws_2", None));
        w.rt.sessions
            .borrow_mut()
            .insert("s3".into(), w.session_row("s3", "ws_1", None));
        w.rt.execs.borrow_mut().push("ws_1".into());

        w.area.end_sessions("ws_1", "machine paused");

        assert_eq!(
            w.log_now(),
            vec![
                json!(["session.end", "s1", "machine paused"]),
                json!(["session.end", "s3", "machine paused"]),
                json!(["exec.end", "machine paused"]),
            ]
        );
    })
}

/// The scene "drop: everything the workspace left goes, in order, and the event last" of machines_phases.json:
/// machines.ts:157-218, with the sub-thread record the scene gives it.
#[test]
fn drop_everything_the_workspace_left_goes_in_order_and_the_event_last() {
    common::run_local(async {
        let fx = common::fixture("machines_phases");
        let scene = common::scene(
            &fx,
            "drop: everything the workspace left goes, in order, and the event last",
        );
        let w = World::new(Opts {
            store: support::store_of(scene),
            ..Default::default()
        });
        let e = w.entry_in(EntryOpts {
            sub: true,
            ..Default::default()
        });
        let log = w.log.clone();
        e.entry.borrow_mut().late_read =
            Some(Box::new(move || log.push(json!(["lateRead cancelled"]))));
        let log = w.log.clone();
        e.entry.borrow_mut().pr_poll =
            Some(Box::new(move || log.push(json!(["prPoll cancelled"]))));
        let thread_on = |ws: &str| wsp_machines::machines::machines::ThreadOn {
            workspace_id: ws.into(),
            record: Default::default(),
        };
        w.rt.thread_records
            .borrow_mut()
            .insert("t1".into(), thread_on("ws_1"));
        w.rt.thread_records
            .borrow_mut()
            .insert("t9".into(), thread_on("ws_9"));
        w.rt.sessions
            .borrow_mut()
            .insert("s1".into(), w.session_row("s1", "ws_1", Some("t2")));
        w.rt.sessions
            .borrow_mut()
            .insert("s9".into(), w.session_row("s9", "ws_9", Some("t9")));
        w.rt.kept.borrow_mut().push(("t1".into(), "ws_1".into()));

        w.area.drop("ws_1").await.expect("the drop lands");
        common::flush().await;

        let answer = json!({
            "live": w.live_ids(),
            "threads": w.rt.thread_records.borrow().keys().cloned().collect::<Vec<_>>(),
            "sessions": w.rt.sessions.borrow().keys().cloned().collect::<Vec<_>>(),
            "agentLists": w.store.table("agent-lists"),
        });
        support::assert_log(scene, &w.log_now(), support::CREATES);
        support::assert_answer(scene, &answer, support::CREATES);
        assert_eq!(
            w.unlogged(),
            vec![
                "forget_workspace_maps ws_1".to_string(),
                "forget_viewed ws_1".to_string(),
                "settle_flushes ws_1".to_string(),
                "forget_kept_writes ws_1".to_string(),
            ],
            "the maps the oracle's real collections take without a log line"
        );
    })
}

/// machines.ts:213-216: the lists a machine's agents answered go with the last workspace on that machine, not before.
#[test]
fn drop_keeps_the_agent_lists_while_another_record_shares_the_machine() {
    common::run_local(async {
        let store =
            json!({ "agent-lists": { "m-1:claude": {}, "m-1:codex": {}, "m-2:claude": {} } });
        let w = World::new(Opts {
            store,
            ..Default::default()
        });
        let _first = w.entry_in(EntryOpts::default());
        let _second = w.entry_in(EntryOpts {
            record: json!({ "id": "ws_2", "name": "w2" }),
            ..Default::default()
        });
        assert_eq!(w.live_ids(), vec!["ws_1".to_string(), "ws_2".to_string()]);

        w.area.drop("ws_1").await.expect("the drop lands");
        assert_eq!(
            w.store.table("agent-lists"),
            json!([["m-1:claude", {}], ["m-1:codex", {}], ["m-2:claude", {}]]),
            "ws_2 is still on m-1: its lists stay"
        );
        assert!(support::log_of(&w.log_now(), &["store.delete"])
            .iter()
            .all(|d| d[1] != "agent-lists"));

        w.area.drop("ws_2").await.expect("the drop lands");
        assert_eq!(
            w.store.table("agent-lists"),
            json!([["m-2:claude", {}]]),
            "the last workspace on m-1 takes its lists with it"
        );
        let deleted: Vec<Value> = support::log_of(&w.log_now(), &["store.delete"])
            .into_iter()
            .filter(|d| d[1] == "agent-lists")
            .collect();
        assert_eq!(
            deleted,
            vec![
                json!(["store.delete", "agent-lists", "m-1:claude"]),
                json!(["store.delete", "agent-lists", "m-1:codex"])
            ]
        );
    })
}

/// machines.ts:162-170: a folder kind rewrites the daemon's roots from the entry, right after it leaves `live`; a
/// fork's machine does not.
#[test]
fn drop_of_a_folder_workspace_rewrites_the_daemon_roots_from_its_entry() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let _e = w.entry_in(EntryOpts {
            record: json!({ "kind": "local" }),
            ..Default::default()
        });
        w.area.drop("ws_1").await.expect("the drop lands");
        let log = w.log_now();
        let names = support::names_of(&log);
        let dropped = names
            .iter()
            .position(|n| n == "module.dropped")
            .expect("the kind module was told");
        assert_eq!(
            names[dropped + 1],
            "writeDaemonRoots",
            "roots are rewritten straight after the module's drop: {names:?}"
        );
        assert_eq!(log[dropped + 1], json!(["writeDaemonRoots", "ws_1"]));
    })
}

/// machines.ts:163-165: a kind module that fails to let go only warns, and the drop goes on.
#[test]
fn a_kind_module_that_fails_to_drop_only_warns() {
    common::run_local(async {
        let w = World::new(Opts::default());
        w.rt.cfg.borrow_mut().dropped_err = Some("the ssh child would not close".into());
        let _e = w.entry_in(EntryOpts::default());
        w.area.drop("ws_1").await.expect("the drop lands");
        assert_eq!(
            w.warns(),
            vec![
                "w1's machine m-1 kept something of this host's: the ssh child would not close"
                    .to_string()
            ]
        );
        let log = w.log_now();
        assert_eq!(
            log.last().unwrap(),
            &json!(["emit", { "type": "workspace.deleted", "workspaceId": "ws_1" }]),
            "the event is still last"
        );
        assert!(w.live_ids().is_empty());
    })
}
