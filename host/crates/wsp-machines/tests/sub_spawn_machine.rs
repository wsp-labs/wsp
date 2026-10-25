// SPDX-License-Identifier: AGPL-3.0-only
//! spec/sub_spawn_machine.md: the machine of a spawn the queue let through and of a send into a released sub-thread,
//! ported from packages/runtime/test/sub-spawn.test.ts (the fork and its faults), sub-resume.test.ts, sub-oom.test.ts
//! and sub-start-events.test.ts's retried first start.
mod machines_b_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use serde_json::{json, Value};

use machines_b_support::life_ctx::claude_session_file;
use machines_b_support::spawn_ctx::*;
use machines_b_support::*;
use wsp_engine::box_backend::BOX_WORK_DIR;
use wsp_engine::errors::{EngineError, ErrorKind, WspError};
use wsp_machines::machines::core_types::*;
use wsp_machines::machines::sub_life::sub_session_land_command;
use wsp_machines::machines::sub_spawn::machine::{fault_of, Resumed};
use wsp_machines::machines::sub_spawn::*;
use wsp_words::exit::HostError;
use wsp_words::sub_thread::*;

const T: u64 = 1_760_000_000_000;
const FIX: &str = " Start it again in a few minutes.";
const OPENING: &str = "This thread runs on a machine of its own on Boat.\n\nRun the suite.";
const TAR: &str = "tar -xzf '/tmp/wsp-seed-ws_sub.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-ws_sub.tgz'";
const INSTALL: &str = "install -d -m 0755 '/root/web'";
const SESSION_AT: &str = "/root/.claude-cfg/projects/-root-web/s2.jsonl";
const SESSION_PUT: &str = "/tmp/wsp-session-t_sub.gz";
const OOM: &str = "the agent on Fix the flaky upload test was killed for memory: its machine is small, 2 vCPU, 4 GB, and Boat's own processes hold part of that. Start the work again with size 4x8.";

type Rec = Rc<RefCell<WorkspaceRecord>>;

/// A spawn the queue let through: the record under its lead holding the stand-in, the picks on this computer.
fn queued(extra: Value) -> (Fixture, Rec) {
    let f = lead_on_this_computer();
    f.host.pick(&["CLAUDE.local.md", "local.properties"]);
    let mut spawn = fresh_spawn(T as f64);
    if let Value::Object(m) = extra {
        for (k, v) in m {
            spawn[k.as_str()] = v;
        }
    }
    let record = queued_sub(&f, spawn);
    (f, record)
}

async fn start(f: &Fixture, record: &Rec, waited: bool) -> (AreaResult<()>, Rc<Cell<u32>>) {
    let posted = Rc::new(Cell::new(0));
    let p = posted.clone();
    let r = f.area.stand_up_queued(record, Rc::new(move || p.set(p.get() + 1)), waited).await;
    (r, posted)
}

fn spawn_of(record: &Rec) -> Value {
    serde_json::to_value(record.borrow().sub.as_ref().unwrap().spawn.as_ref().expect("the record has a spawn")).unwrap()
}

fn phase_of(record: &Rec) -> Value {
    serde_json::to_value(&record.borrow().phase).unwrap()
}

fn machines(f: &Fixture) -> Vec<Arc<FakeMachine>> {
    f.host.backend.machines.lock().unwrap().clone()
}

fn killed(f: &Fixture) -> Vec<bool> {
    machines(f).iter().map(|m| m.killed()).collect()
}

fn creates(f: &Fixture) -> usize {
    f.host.backend.creates.lock().unwrap().len()
}

fn small() -> SubSize {
    sizes_sold(&["small"]).remove(0)
}

fn resume_preamble(pushed: bool, sent: &[&str]) -> String {
    sub_resume_preamble(&SubResumePreamble {
        on: "Boat".into(),
        size: small(),
        folder: FOLDER.into(),
        sent: sent.iter().map(|s| s.to_string()).collect(),
        setup_git: "gh auth setup-git".into(),
        repo: REPO.into(),
        base: BaseAt { branch: "feat/upload".into(), sha: BASE_SHA.into() },
        branch: BRANCH.into(),
        pushed,
        delete_after_minutes: 30.0,
        brokered: None,
    })
}

fn unusable(id: &str) -> SubFault {
    SubFault::Unusable { r#box: id.to_string(), why: format!("{BOX_WORK_DIR} is missing"), request_id: None }
}

fn host_error(e: AreaError) -> HostError {
    host_of(e)
}

fn failed_resume(r: Result<Resumed, HostError>) -> HostError {
    r.err().expect("the send went ahead")
}

fn resume(f: &Fixture, entry: &Live, harness: &str, prompt: &str) -> tokio::task::JoinHandle<Result<Resumed, HostError>> {
    let (area, entry, harness, prompt) = (f.area.clone(), entry.clone(), harness.to_string(), prompt.to_string());
    tokio::task::spawn_local(async move { area.resume_sub(&entry, SUB_THREAD, &harness, Some(&prompt)).await })
}

async fn send(f: &Fixture, entry: &Live, prompt: &str) -> Result<Resumed, HostError> {
    f.area.resume_sub(entry, SUB_THREAD, "claude", Some(prompt)).await
}

fn record_of(entry: &Live) -> Rec {
    entry.record.clone()
}

fn sub_json(entry: &Live) -> Value {
    serde_json::to_value(entry.record.borrow().sub.as_ref().unwrap()).unwrap()
}

/// A released sub-thread whose first start failed: it ran nothing, holds no session, and keeps its opening message.
fn first_start_failed() -> (Fixture, Live) {
    let f = lead_on_this_computer();
    {
        let world = w();
        let mut world = world.borrow_mut();
        world.menu = vec![(".env".into(), 120, "secret".into()), ("CLAUDE.local.md".into(), 1100, "config".into())];
    }
    f.host.pick(&[".env", "CLAUDE.local.md"]);
    f.host.thread_records.borrow_mut().push((SUB_THREAD.to_string(), SUB_WS.to_string()));
    let mut rec = released_record(SUB_WS);
    let sub = rec["sub"].as_object_mut().unwrap();
    sub.remove("session");
    sub.remove("result");
    sub.insert("opening".to_string(), json!(OPENING));
    let entry = f.host.put_dead(rec);
    (f, entry)
}

#[test]
fn forks_the_image_head_lands_the_picks_by_the_file_road_and_names_only_paths_in_commands() {
    run(async {
        let (f, record) = queued(json!({}));
        let (r, posted) = start(&f, &record, false).await;
        r.unwrap();
        // One more start at the cloud, forked from the head at the record's size.
        let made = f.host.backend.creates.lock().unwrap().clone();
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].from_snapshot.as_deref(), Some("wsp-host-default-v1"));
        assert_eq!((made[0].cpu.map(|n| n.0), made[0].mem_mb.map(|n| n.0)), (Some(2.0), Some(4096.0)));
        // The picked files went up as one archive by the file road, and only their names are in a command.
        let machine = machines(&f)[0].clone();
        assert_eq!(machine.files(), vec![("/tmp/wsp-seed-ws_sub.tgz".to_string(), tar_of(&[("CLAUDE.local.md", 1100), ("local.properties", 143)]))]);
        assert_eq!(machine.log(), vec![INSTALL.to_string(), TAR.to_string()]);
        assert!(["git clone", "git push", "git fetch", "npm install", "pnpm install", "npm ci", "pnpm ci"].iter().all(|c| !machine.ran(c)));
        // The record stands on the machine, stored once all of it does.
        assert_eq!((record.borrow().machine_id.clone(), phase_of(&record)), ("bx_1".to_string(), json!("running")));
        let stored = f.host.stored(SUB_WS).expect("the record was persisted");
        assert_eq!(stored["machineId"], json!("bx_1"));
        assert!(stored["sub"]["spawn"]["readyAt"].is_number() && stored["sub"]["spawn"].get("asked").is_none());
        let entry = f.host.live.borrow().get(SUB_WS).cloned().unwrap();
        assert_eq!((entry.machine.id().to_string(), entry.creating.get()), ("bx_1".to_string(), false));
        assert_eq!(posted.get(), 1);
        // A first start announces the record; nothing wakes.
        let created = f.host.events_of("workspace.created");
        assert_eq!((created.len(), created[0]["workspace"]["id"].clone()), (1, json!(SUB_WS)));
        assert!(f.host.events_of("workspace.woken").is_empty() && f.host.calls_of("emit_status").is_empty());
        assert_eq!(
            f.host.reported(),
            vec![
                ("fork-requested".to_string(), "starting Fix the flaky upload test on Boat".to_string(), None),
                ("project-cloned".to_string(), "copying 2 files".to_string(), None),
                ("ready".to_string(), "ready".to_string(), None),
            ]
        );
        // The stages are told under the lead that asked, rooted at the record's own root.
        let reporter = f.host.calls_of("stage_reporter");
        assert_eq!(reporter.len(), 1);
        assert_eq!(reporter[0][..3], [SUB_WS.to_string(), NAME.to_string(), T.to_string()]);
        assert_eq!(serde_json::from_str::<Value>(&reporter[0][3]).unwrap(), json!({ "kind": "thread", "threadId": LEAD_THREAD, "workspaceId": LEAD_WS, "rootThreadId": LEAD_THREAD }));
        // The name is held for the call, and no daemon is asked where the machine has no road to it.
        assert!(f.host.at("forking_add") < f.host.at("fork") && f.host.at("fork") < f.host.at("forking_remove"));
        assert!(f.host.calls_of("ping_daemon").is_empty() && f.host.calls_of("sync_daemon").is_empty());
    });
}

#[test]
fn writes_each_step_on_the_record_as_it_goes() {
    run(async {
        let (f, record) = queued(json!({ "uncommitted": 3 }));
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.states = vec!["provisioning".into(), "cloning".into(), "cloning".into(), "ready".into()];
            world.fork_ms = 250.0;
        }
        let (r, _) = start(&f, &record, false).await;
        r.unwrap();
        let spawned = spawn_of(&record);
        assert_eq!(
            spawned,
            json!({
                "startedAt": T,
                "branch": { "branch": "feat/upload", "sha": BASE_SHA, "ms": 0 },
                "machine": { "since": T, "id": "bx_1", "states": ["provisioning", "cloning", "ready"], "ms": 250 },
                "files": { "since": T + 250, "paths": ["CLAUDE.local.md", "local.properties"], "bytes": 1243, "ms": 0 },
                "readyAt": T + 250,
                "uncommitted": 3,
                "log": [
                    "starting Fix the flaky upload test on Boat", "Boat's machine bx_1 is provisioning", "Boat's machine bx_1 is cloning", "Boat's machine bx_1 is cloning",
                    "Boat's machine bx_1 is ready", "copying 2 files", "ready"
                ]
            })
        );
        assert!(spawned.get("failed").is_none());
        // Each step is told to the thread's row as it is written: the start, the machine, its four states, the folder, the files, the ready.
        let told = f.host.calls_of("push_head");
        assert_eq!(told.len(), 13);
        assert!(told.iter().all(|c| c == &[SUB_THREAD.to_string()]));
        // The stored record holds the steps up to its ready, which is written after it.
        let stored = f.host.stored(SUB_WS).unwrap()["sub"]["spawn"].clone();
        assert_eq!((stored["readyAt"].clone(), stored["machine"]["ms"].clone(), stored["files"]["ms"].clone()), (json!(T + 250), json!(250), json!(0)));
    });
}

#[test]
fn is_deleted_and_replaced_once_for_each_fault() {
    run(async {
        // Which fault a failure is, and which no fresh machine cures.
        let box_gone = AreaError::Engine(EngineError::guest_unusable("bx_9", "Boat", "the disk is missing", 200));
        assert_eq!(fault_of(&box_gone), Some(SubFault::Unusable { r#box: "bx_9".into(), why: "the disk is missing".into(), request_id: None }));
        let env = |code: i64| AreaError::Engine(EngineError::EnvNotLanded { machine_id: "bx_9".into(), exit_code: code, detail: String::new() });
        assert_eq!(fault_of(&env(124)), Some(SubFault::Env { r#box: "bx_9".into() }));
        assert_eq!(fault_of(&env(1)), None);
        let cancelled = |why: &str| AreaError::Engine(EngineError::CreateCancelled { machine_id: "bx_9".into(), why: why.into(), line: String::new() });
        assert_eq!(fault_of(&cancelled("no capacity")), Some(SubFault::Cancelled { r#box: "bx_9".into(), why: "no capacity".into() }));
        assert_eq!(fault_of(&cancelled("")), Some(SubFault::Cancelled { r#box: "bx_9".into(), why: "no reason given".into() }));
        assert_eq!(fault_of(&AreaError::Engine(engine_error("boom"))), None);
        assert_eq!(fault_of(&AreaError::Host(HostError::new("boom"))), None);

        for fault in [fault("unusable"), fault("env"), fault("cancelled"), replayed("unusable"), replayed("env")] {
            let (f, record) = queued(json!({}));
            w().borrow_mut().faults.push_back(fault.clone());
            let (r, posted) = start(&f, &record, false).await;
            r.unwrap();
            let made = machines(&f);
            assert_eq!(creates(&f), 2, "{fault:?}");
            // The broken box is deleted, by Boat's own cleanup or, where its replayed key left it standing, by wsp.
            assert_eq!(killed(&f), [true, false], "{fault:?}");
            assert_eq!(record.borrow().machine_id, made[1].id);
            let line = match fault.kind {
                "unusable" => format!("Boat gave {} with nothing able to run on it ({BOX_WORK_DIR} is missing); wsp deleted it and started another.", made[0].id),
                "env" => format!("Boat's machine {} did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another.", made[0].id),
                _ => format!("Boat could not get a machine for {NAME} and cancelled the create (no capacity); wsp started another."),
            };
            assert_eq!(spawn_of(&record)["replaced"], json!([line]), "{fault:?}");
            assert_eq!(f.host.warns.borrow().clone(), vec![format!("workspace {SUB_WS}: {line}")]);
            assert!(f.host.reported().contains(&("fork-requested".to_string(), line.clone(), Some(json!({ "notice": line })))));
            // A fresh key, so the next create is a new machine and not a replay of the one deleted.
            assert_eq!(f.host.calls_of("delete_create"), vec![vec![format!("workspace/{SUB_WS}")]]);
            assert_eq!(posted.get(), 1, "only the machine that stood was announced");
            let entry = f.host.live.borrow().get(SUB_WS).cloned().unwrap();
            assert_eq!(entry.machine.id(), made[1].id);
            assert_eq!(f.host.calls_of("forking_remove").len(), 1);
        }
    });
}

#[test]
fn fails_on_a_second_failure_naming_both_and_leaves_neither_standing() {
    run(async {
        // A files step an earlier attempt left goes with the machine it was on.
        let (f, record) = queued(json!({ "files": { "since": T, "paths": ["CLAUDE.local.md"], "bytes": 1100 } }));
        w().borrow_mut().faults.extend([replayed("unusable"), fault("unusable")]);
        let (r, posted) = start(&f, &record, false).await;
        let e = host_error(r.expect_err("the start went ahead"));
        let made = machines(&f);
        let message = format!("Boat gave two machines in a row that could not run commands ({}, {}); Boat is restoring badly right now.{FIX}", made[0].id, made[1].id);
        assert_eq!(e.message, message);
        assert_eq!(e, sub_twice_refusal(&unusable(&made[0].id), &unusable(&made[1].id), NAME, "Boat"));
        assert_eq!(class_of(&e), "provider");
        assert_eq!((creates(&f), killed(&f)), (2, vec![true, true]));
        assert_eq!(record.borrow().machine_id, "");
        assert_eq!(posted.get(), 0);
        // The step it failed on stays on the record with the two halves of why.
        let spawned = spawn_of(&record);
        assert_eq!(spawned["failed"], json!({ "step": "machine", "said": message.strip_suffix(FIX).unwrap(), "fix": FIX.trim_start() }));
        assert!(spawned.get("files").is_none());
        assert_eq!(spawned["replaced"].as_array().unwrap().len(), 1);
        assert_eq!(f.host.reported().last().cloned(), Some(("failed".to_string(), message, None)));
        // The key is dropped once, between the two attempts.
        assert_eq!(f.host.calls_of("delete_create").len(), 1);
        assert_eq!(f.host.calls_of("forking_remove").len(), 1);
    });
}

#[test]
fn names_both_faults_when_they_differ() {
    run(async {
        let (f, record) = queued(json!({}));
        w().borrow_mut().faults.extend([fault("env"), fault("cancelled")]);
        let (r, _) = start(&f, &record, false).await;
        let e = host_error(r.expect_err("the start went ahead"));
        let made = machines(&f);
        assert_eq!(e.message, format!("Boat gave two machines in a row that failed before the sub-thread started ({}: setup not taken, exit 124; {}: no capacity).{FIX}", made[0].id, made[1].id));
        assert_eq!(killed(&f), [true, true]);
        assert_eq!(class_of(&e), "provider");
    });
}

#[test]
fn a_send_while_the_new_machine_is_made_waits_for_it() {
    run(async {
        let (f, entry) = released_sub();
        let g = gate();
        *f.host.stand_up_gate.borrow_mut() = Some(g.clone());
        let a = resume(&f, &entry, "claude", "what was the word?");
        settle().await;
        let b = resume(&f, &entry, "claude", "and the second word?");
        settle().await;
        // Nothing has launched while the machine is being made: one start asked, the send held under its own message.
        assert_eq!((f.host.calls_of("fork").len(), creates(&f)), (1, 0));
        let held = spawn_of(&record_of(&entry));
        assert_eq!(phase_of(&record_of(&entry)), json!("running"));
        assert_eq!((held["machine"]["states"].clone(), held.get("branch").cloned(), held["asked"].clone()), (json!([]), None, json!("what was the word?")));
        open(&g);
        let (ra, rb) = (a.await.unwrap().unwrap(), b.await.unwrap().unwrap());
        // The turn with the preamble runs first; the second opens with none of its own.
        assert_eq!(ra.preamble, Some(resume_preamble(true, &["CLAUDE.local.md"])));
        assert_eq!(rb.preamble, None);
        assert!(ra.notices.is_empty() && rb.notices.is_empty());
        let now = f.host.live.borrow().get(SUB_WS).cloned().unwrap();
        assert!(Rc::ptr_eq(&ra.entry, &now) && Rc::ptr_eq(&rb.entry, &now));
        assert_eq!(now.machine.id(), "bx_1");
        assert_eq!((f.host.calls_of("fork").len(), creates(&f)), (1, 1));
        // A start that stands drops the asked text: the turn's own row and transcript hold it from here.
        let spawned = spawn_of(&record_of(&entry));
        assert!(spawned["readyAt"].is_number() && spawned["machine"]["ms"].is_number() && spawned.get("asked").is_none());
        // A send once it runs has nothing to make.
        let again = send(&f, &entry, "one more").await.unwrap();
        assert!(Rc::ptr_eq(&again.entry, &entry) && again.preamble.is_none() && again.notices.is_empty());
        assert_eq!(f.host.calls_of("fork").len(), 1);
    });
}

#[test]
fn makes_a_new_machine_from_the_head_lands_picks_and_session_and_resumes_under_the_preamble() {
    run(async {
        let (f, entry) = released_sub();
        w().borrow_mut().image.as_mut().unwrap().head = 2;
        let r = send(&f, &entry, "what was the word?").await.unwrap();
        // One more start at the cloud, forked from the head as it stands now, at the record's size.
        let made = f.host.backend.creates.lock().unwrap().clone();
        assert_eq!(made.len(), 1);
        assert_eq!(made[0].from_snapshot.as_deref(), Some("wsp-host-default-v2"));
        assert_eq!((made[0].cpu.map(|n| n.0), made[0].mem_mb.map(|n| n.0)), (Some(2.0), Some(4096.0)));
        // The picked files, and the kept session put once, then landed where it stood with any other copy taken away.
        let machine = machines(&f)[0].clone();
        let files = machine.files();
        assert_eq!(files.iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["/tmp/wsp-seed-ws_sub.tgz", SESSION_PUT]);
        assert_eq!(files[0].1, tar_of(&[("CLAUDE.local.md", 1100)]));
        assert_eq!(files[1].1, SESSION_BYTES);
        let land = sub_session_land_command(&claude_session_file("s2"), SESSION_AT, SESSION_PUT);
        assert!(land.contains(&format!("find '/root/.claude-cfg/projects' -maxdepth 2 -type f -name '*s2.jsonl' ! -path '{SESSION_AT}' -delete")));
        assert!(land.contains(&format!("gunzip -c '{SESSION_PUT}' > '{SESSION_AT}'")));
        assert_eq!(machine.log(), vec![INSTALL.to_string(), "tar -xzf '/tmp/wsp-seed-ws_sub.tgz' -C '/root/web' && rm -f '/tmp/wsp-seed-ws_sub.tgz'".to_string(), land]);
        // The resume preamble, word for word, goes before the message.
        let preamble = [
            "This thread's machine was deleted after its last turn, and the thread goes on here, on a new machine on Boat (small, 2 vCPU, 4 GB). Your conversation came with it; the code did not. The folder you are in, /root/web, holds only the git-ignored files the person picked: CLAUDE.local.md. Get your branch back first, in this folder:".to_string(),
            String::new(),
            "gh auth setup-git".to_string(),
            "git init -q".to_string(),
            "git remote add origin https://github.com/acme/web.git".to_string(),
            format!("git fetch -q --filter=blob:none origin {BRANCH}"),
            format!("git checkout -q -b {BRANCH} FETCH_HEAD"),
            String::new(),
            format!("Push to {BRANCH} as before. This machine is deleted 30 minutes after your last turn ends."),
            String::new(),
            "The message:".to_string(),
        ]
        .join("\n");
        assert_eq!(r.preamble, Some(preamble.clone()));
        assert_eq!(preamble, resume_preamble(true, &["CLAUDE.local.md"]));
        assert!(r.notices.is_empty());
        // The record runs on the new machine, from the new head, with what the machine before left behind gone.
        let record = record_of(&entry);
        let rec = record.borrow();
        assert_eq!((rec.machine_id.as_str(), rec.golden.as_str(), rec.first_life, rec.disk_gone.clone()), ("bx_1", "wsp-host-default-v2", Some(true), None));
        assert_eq!((serde_json::to_value(&rec.phase).unwrap(), rec.sub.as_ref().unwrap().lost.clone()), (json!("running"), None));
        let spawned = serde_json::to_value(rec.sub.as_ref().unwrap().spawn.as_ref().unwrap()).unwrap();
        assert_eq!((spawned["startedAt"].clone(), spawned.get("asked").cloned(), spawned.get("notSent").cloned()), (json!(T), None, None));
        assert!(spawned["readyAt"].is_number());
        assert_eq!(f.host.stored(SUB_WS).unwrap()["machineId"], json!("bx_1"));
        // Woken as a wake is, so the host's link and the broker's pipes dial the new machine; it is no new workspace.
        assert_eq!(f.host.events_of("workspace.woken"), vec![json!({ "type": "workspace.woken", "workspaceId": SUB_WS, "machineId": "bx_1" })]);
        assert!(f.host.events_of("workspace.created").is_empty());
        assert_eq!(f.host.calls_of("emit_status"), vec![vec![SUB_WS.to_string(), "reachable".to_string()]]);
        assert_eq!(r.entry.machine.id(), "bx_1");
        assert_eq!(f.host.calls_of("place_guard"), vec![vec!["box".to_string()]]);
        assert_eq!(f.host.calls_of("copy_of"), vec![vec!["box".to_string(), "default".to_string()]]);
        // The stages are told with no thread to ask them.
        assert_eq!(f.host.calls_of("stage_reporter")[0][3], "");
    });
}

#[test]
fn lifts_the_broker_before_the_new_machine() {
    run(async {
        let (f, entry) = released_sub();
        // The machine deleted at the release was cut off at the broker.
        w().borrow_mut().broker_off.insert(SUB_WS.to_string());
        send(&f, &entry, "what was the word?").await.unwrap();
        assert_eq!(f.host.calls_of("broker_lift"), vec![vec![SUB_WS.to_string()]]);
        assert!(f.host.at("broker_lift") < f.host.at("fork"));
        assert_eq!(w().borrow().off_at_fork, [false]);
        assert!(w().borrow().broker_off.is_empty());
    });
}

#[test]
fn names_a_stayed_pick_in_the_notice_of_the_send() {
    run(async {
        let (f, entry) = released_sub();
        w().borrow_mut().menu = vec![(".env".into(), 120, "secret".into()), ("CLAUDE.local.md".into(), 1100, "config".into())];
        f.host.pick(&[".env", "CLAUDE.local.md"]);
        let r = send(&f, &entry, "what was the word?").await.unwrap();
        assert_eq!(r.notices, vec![sub_stayed_line(&[".env"])]);
        assert_eq!(r.preamble, Some(resume_preamble(true, &["CLAUDE.local.md"])));
        assert_eq!(machines(&f)[0].files()[0].1, tar_of(&[("CLAUDE.local.md", 1100)]));
    });
}

#[test]
fn a_retried_failed_first_start_names_a_stayed_pick() {
    run(async {
        let (f, entry) = first_start_failed();
        w().borrow_mut().faults.extend([replayed("unusable"), fault("unusable")]);
        let e = failed_resume(send(&f, &entry, OPENING).await);
        assert!(e.message.starts_with("Boat gave two machines in a row"));
        assert_eq!(spawn_of(&record_of(&entry))["asked"], json!(OPENING));
        // Try again sends the first message as it was: it opens on a new machine with no second preamble, and says what stayed.
        let tried = send(&f, &entry, OPENING).await.unwrap();
        assert_eq!(tried.notices, vec![sub_stayed_line(&[".env"])]);
        assert_eq!(tried.preamble, None);
        assert_eq!(sub_json(&entry).get("opening"), None);
        assert_eq!(f.host.stored(SUB_WS).unwrap()["sub"].get("opening"), None);
        assert_eq!((creates(&f), phase_of(&record_of(&entry))), (3, json!("running")));
    });
}

#[test]
fn lands_codexs_rollout_at_its_dated_path() {
    run(async {
        let (f, entry) = released_sub();
        let at = "/root/.codex/sessions/2026/10/12/rollout-2026-10-12T06-00-00-s2.jsonl";
        {
            let record = record_of(&entry);
            let mut r = record.borrow_mut();
            let session = r.sub.as_mut().unwrap().session.as_mut().unwrap();
            session.agent = "codex".to_string();
            session.path = Some(at.to_string());
        }
        let r = f.area.resume_sub(&entry, SUB_THREAD, "codex", Some("what was the word?")).await.unwrap();
        let machine = machines(&f)[0].clone();
        assert_eq!(machine.files().iter().filter(|(p, _)| p == SESSION_PUT).map(|(_, b)| b.clone()).collect::<Vec<_>>(), vec![SESSION_BYTES.to_vec()]);
        assert!(machine.log().contains(&sub_session_land_command(&codex_session_file("s2"), at, SESSION_PUT)));
        assert_eq!(r.entry.machine.id(), "bx_1");
    });
}

#[test]
fn replaces_a_broken_box_once_and_lands_the_session_on_the_second_alone() {
    run(async {
        let (f, entry) = released_sub();
        w().borrow_mut().faults.push_back(fault("unusable"));
        let r = send(&f, &entry, "what was the word?").await.unwrap();
        assert_eq!(creates(&f), 2);
        let made = machines(&f);
        assert!(made[0].killed() && !made[1].killed());
        // Nothing was put on, or run on, the broken one; the session landed on the second alone.
        assert!(made[0].files().is_empty() && made[0].log().is_empty());
        assert_eq!(made[1].files().iter().filter(|(p, _)| p == SESSION_PUT).count(), 1);
        assert_eq!(record_of(&entry).borrow().machine_id, made[1].id);
        assert_eq!(r.entry.machine.id(), made[1].id);
        assert_eq!(r.preamble, Some(resume_preamble(true, &["CLAUDE.local.md"])));
        // The line of the replacement rides the record, not the send's notice.
        assert_eq!(spawn_of(&record_of(&entry))["replaced"].as_array().unwrap().len(), 1);
    });
}

#[test]
fn puts_the_failed_starts_row_out_before_the_refusal() {
    run(async {
        let (f, entry) = released_sub();
        w().borrow_mut().faults.extend([fault("unusable"), fault("unusable")]);
        let e = failed_resume(send(&f, &entry, "what was the word?").await);
        assert!(e.message.starts_with("Boat gave two machines in a row"));
        // The slow row went out whole before the refusal came back, and held the failed record.
        assert_eq!(w().borrow().rows_sent_done, 1);
        let at = w().borrow().at_rows_sent[0].clone().expect("the record was stored before its row went out");
        assert_eq!((at["phase"].clone(), at["machineId"].clone(), at["sub"]["spawn"]["failed"]["step"].clone()), (json!("released"), json!(""), json!("machine")));
        let names = f.host.names();
        let last = |n: &str| names.iter().rposition(|x| x == n).unwrap();
        assert!(last("attach_dead") < last("persist") && last("persist") < last("rows_sent"));
    });
}

#[test]
fn fails_after_a_second_broken_box_and_stays_released_with_asked_and_failed() {
    run(async {
        let (f, entry) = released_sub();
        w().borrow_mut().faults.extend([fault("unusable"), fault("unusable")]);
        let e = failed_resume(send(&f, &entry, "what was the word?").await);
        let made = machines(&f);
        assert_eq!(e, sub_twice_refusal(&unusable(&made[0].id), &unusable(&made[1].id), NAME, "Boat"));
        assert_eq!(creates(&f), 2);
        let record = record_of(&entry);
        assert_eq!((phase_of(&record), record.borrow().machine_id.clone()), (json!("released"), String::new()));
        // The step it failed on stays with the two halves of why, and the message the send asked, for Try again after a reload too.
        let spawned = spawn_of(&record);
        assert_eq!(spawned["asked"], json!("what was the word?"));
        assert_eq!(spawned["failed"], json!({ "step": "machine", "said": e.message.strip_suffix(FIX).unwrap(), "fix": FIX.trim_start() }));
        assert_eq!(f.host.calls_of("attach_dead").len(), 1);
        assert_eq!(f.host.live.borrow().get(SUB_WS).unwrap().machine.id(), "");
        // The next send makes a machine as if the first had not been tried.
        let again = send(&f, &entry, "what was the word?").await.unwrap();
        assert_eq!(again.entry.machine.id(), "bx_3");
        assert_eq!((phase_of(&record), creates(&f)), (json!("running"), 3));
    });
}

#[test]
fn asks_gh_here_again_before_a_new_machine_while_brokered() {
    run(async {
        let (f, entry) = released_sub();
        // The broker is ready and gh on this computer holds no login any more.
        {
            let world = w();
            world.borrow_mut().broker_ready = true;
        }
        let e = failed_resume(send(&f, &entry, "what was the word?").await);
        assert_eq!(e, sub_host_no_sign_in_refusal("Boat", "acme/web"));
        assert_eq!(class_of(&e), "usage");
        assert_eq!(w().borrow().local.log(), vec!["gh auth token --hostname github.com > /dev/null 2>&1".to_string()]);
        assert_eq!(f.host.calls_of("brokered"), vec![vec!["cloud".to_string(), "box".to_string()]]);
        // No machine was made, and the record is as it was.
        assert!(machines(&f).is_empty() && f.host.calls_of("fork").is_empty());
        let record = record_of(&entry);
        assert_eq!((phase_of(&record), record.borrow().machine_id.clone()), (json!("released"), String::new()));
        assert!(f.host.calls_of("attach_dead").is_empty() && f.host.calls_of("rows_sent").is_empty());
    });
}

#[test]
fn refuses_an_agent_the_broker_adds_no_key_for_before_a_new_machine() {
    run(async {
        let (f, entry) = released_sub();
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.broker_ready = true;
            world.image.as_mut().unwrap().agents = vec!["claude".into(), "codex".into(), "opencode".into()];
        }
        w().borrow().gh_signed_in.store(true, std::sync::atomic::Ordering::SeqCst);
        let e = failed_resume(f.area.resume_sub(&entry, SUB_THREAD, "opencode", Some("what was the word?")).await);
        assert_eq!(e, wsp_words::place_name::brokered_agent_refusal("OpenCode", &["Claude Code", "Codex"]));
        assert_eq!(class_of(&e), "usage");
        assert!(machines(&f).is_empty() && f.host.calls_of("fork").is_empty());
        assert_eq!(phase_of(&record_of(&entry)), json!("released"));
    });
}

#[test]
fn with_no_session_kept_lands_none() {
    run(async {
        // The agent's session was too large to keep: kept is null.
        let (f, entry) = released_sub();
        record_of(&entry).borrow_mut().sub.as_mut().unwrap().session.as_mut().unwrap().kept = None;
        let r = send(&f, &entry, "what was the word?").await.unwrap();
        let machine = machines(&f)[0].clone();
        assert_eq!(machine.files().iter().map(|(p, _)| p.as_str()).collect::<Vec<_>>(), ["/tmp/wsp-seed-ws_sub.tgz"]);
        assert_eq!(machine.log(), vec![INSTALL.to_string(), TAR.to_string()]);
        assert_eq!(r.preamble, Some(resume_preamble(true, &["CLAUDE.local.md"])));

        // A copy the host no longer holds.
        let (g, held) = released_sub();
        g.host.blobs.borrow_mut().clear();
        send(&g, &held, "what was the word?").await.unwrap();
        assert_eq!(machines(&g)[0].log(), vec![INSTALL.to_string(), TAR.to_string()]);

        // A path outside the folder the agent keeps its sessions in is not landed.
        let (h, out) = released_sub();
        record_of(&out).borrow_mut().sub.as_mut().unwrap().session.as_mut().unwrap().path = Some("/elsewhere/s2.jsonl".to_string());
        send(&h, &out, "what was the word?").await.unwrap();
        assert!(machines(&h)[0].files().iter().all(|(p, _)| p != SESSION_PUT));
        assert!(!machines(&h)[0].ran("gunzip"));
    });
}

#[test]
fn a_sub_that_pushed_nothing_starts_again_from_the_leads_commit() {
    run(async {
        let (f, entry) = released_sub();
        record_of(&entry).borrow_mut().sub.as_mut().unwrap().result.as_mut().unwrap().pushed_sha = None;
        let r = send(&f, &entry, "go on").await.unwrap();
        let preamble = r.preamble.expect("a resumed send opens under the resume preamble");
        assert_eq!(preamble, resume_preamble(false, &["CLAUDE.local.md"]));
        assert!(preamble.contains(&format!("git fetch -q --filter=blob:none origin feat/upload\ngit checkout -q -b {BRANCH} {BASE_SHA}\n")));
        assert!(preamble.contains(&format!("Work on {BRANCH}, then commit and run git push -u origin {BRANCH} before your turn ends.")));
    });
}

/// A running sub-thread whose agent was killed, on a machine whose kernel log answers `reply`.
async fn killed_for(reply: Reply, record: Value) -> (Option<String>, Rc<FakeHost>, Arc<FakeMachine>) {
    let f = lead_on_this_computer();
    let machine = FakeMachine::new("bx_1");
    machine.answer("journalctl", reply);
    let entry = f.host.put_live(record, machine.clone());
    let said = f.area.sub_killed_why(&entry, 1_759_999_940_123.0).await;
    (said, f.host.clone(), machine)
}

#[test]
fn oom_reads_the_kernel_log_once_and_says_the_oom_line() {
    run(async {
        let (said, _, machine) = killed_for(ok("1\n"), sub_record(SUB_WS, "running")).await;
        assert_eq!(said.as_deref(), Some(OOM));
        // One reading, since the turn started.
        assert_eq!(machine.log(), vec!["journalctl -k --since @1759999940 --no-pager | grep -ci 'out of memory'".to_string()]);
        // A count of any size, with its newline, is a kill.
        assert_eq!(killed_for(ok("2\n"), sub_record(SUB_WS, "running")).await.0.as_deref(), Some(OOM));
    });
}

#[test]
fn the_kernel_counting_no_oom_keeps_the_agents_own_line() {
    run(async {
        for reply in ["0\n", "", "none\n"] {
            let (said, _, machine) = killed_for(ok(reply), sub_record(SUB_WS, "running")).await;
            assert_eq!((said, machine.log().len()), (None, 1), "{reply:?}");
        }
        // A reading that fails is said to the host's log and the agent's own line stands.
        let (said, host, _) = killed_for(Reply::Fail("ssh dropped".into()), sub_record(SUB_WS, "running")).await;
        assert_eq!(said, None);
        assert_eq!(host.warns.borrow().clone(), vec!["the kernel log of Fix the flaky upload test was not read after its agent was killed: ssh dropped".to_string()]);
    });
}

#[test]
fn names_the_next_size_up_and_none_on_the_largest() {
    run(async {
        let mut default = sub_record(SUB_WS, "running");
        default["size"] = json!({ "cpu": 4, "memMb": 8192 });
        let (said, _, _) = killed_for(ok("1\n"), default).await;
        assert_eq!(said.as_deref(), Some("the agent on Fix the flaky upload test was killed for memory: its machine is default, 4 vCPU, 8 GB, and Boat's own processes hold part of that. Start the work again with size 8x16."));
        // The plan sells small, default and large: 16x32 is not one of them.
        let mut large = sub_record(SUB_WS, "running");
        large["size"] = json!({ "cpu": 8, "memMb": 16384 });
        let (said, _, _) = killed_for(ok("1\n"), large).await;
        assert_eq!(said.as_deref(), Some("the agent on Fix the flaky upload test was killed for memory: its machine is large, 8 vCPU, 16 GB, and Boat's own processes hold part of that. Boat sells no bigger size, so the work has to use less memory."));
    });
}

#[test]
fn a_failed_first_start_retried_opens_with_its_preamble_and_task() {
    run(async {
        let (f, entry) = first_start_failed();
        w().borrow_mut().faults.extend([replayed("unusable"), fault("unusable")]);
        let e = failed_resume(send(&f, &entry, OPENING).await);
        assert!(e.message.starts_with("Boat gave two machines in a row"));
        // Kept on its row under the lead, saying why, with the first message for Try again.
        let record = record_of(&entry);
        assert_eq!((phase_of(&record), record.borrow().machine_id.clone()), (json!("released"), String::new()));
        let spawned = spawn_of(&record);
        assert_eq!((spawned["failed"]["step"].clone(), spawned["asked"].clone()), (json!("machine"), json!(OPENING)));
        assert_eq!(sub_json(&entry)["opening"], json!(OPENING));
        // A second start that fails the same way keeps the first message as the one Try again sends.
        w().borrow_mut().faults.extend([replayed("unusable"), fault("unusable")]);
        failed_resume(send(&f, &entry, "Try once more, the suite is flaky.").await);
        assert_eq!(spawn_of(&record)["asked"], json!(OPENING));
        // A send whose words are not the first message opens with that message's own preamble and task.
        let sent = send(&f, &entry, "Try once more, the suite is flaky.").await.unwrap();
        assert_eq!(sent.preamble.as_deref(), Some(OPENING));
        assert!(sent.preamble.unwrap().contains("Run the suite."));
        assert_eq!(sub_json(&entry).get("opening"), None);
        assert_eq!(f.host.stored(SUB_WS).unwrap()["sub"].get("opening"), None);
        assert_eq!(phase_of(&record), json!("running"));
        // The failed start is cleared by the one that stands.
        assert!(spawn_of(&record).get("failed").is_none());
    });
}

#[test]
fn a_cap_refusal_from_the_queued_start_is_rethrown_untouched() {
    run(async {
        // A create the cloud refused at its cap waits in the queue again; it did not fail.
        let (f, record) = queued(json!({}));
        let cap = AreaError::Engine(EngineError::Provider(WspError {
            kind: ErrorKind::Concurrency,
            status: 429,
            code: None,
            message: "Boat is at its machine cap".into(),
            request_id: None,
            window: None,
            said: None,
        }));
        f.host.script("fork", Err(cap));
        let (r, posted) = start(&f, &record, false).await;
        let e = r.expect_err("the start went ahead");
        assert_eq!(said_of(&e), "Boat is at its machine cap");
        assert!(spawn_of(&record).get("failed").is_none());
        assert!(f.host.reported().iter().all(|(stage, _, _)| stage != "failed"));
        assert_eq!((posted.get(), creates(&f)), (0, 0));
        assert!(f.host.stored(SUB_WS).is_none(), "nothing was written");
        assert!(f.host.calls_of("delete_create").is_empty());
        assert_eq!(f.host.calls_of("forking_remove").len(), 1);
        assert_eq!((phase_of(&record), record.borrow().machine_id.clone()), (json!("running"), String::new()));

        // Any other failure is written on the record, reported, and thrown.
        let (g, held) = queued(json!({}));
        g.host.script("fork", Err(failed("fetch failed: Connect Timeout Error")));
        let (r, _) = start(&g, &held, false).await;
        assert_eq!(said_of(&r.expect_err("the start went ahead")), "fetch failed: Connect Timeout Error");
        assert_eq!(spawn_of(&held)["failed"], json!({ "step": "machine", "said": "fetch failed: Connect Timeout Error" }));
        assert_eq!(g.host.reported().last().cloned(), Some(("failed".to_string(), "fetch failed: Connect Timeout Error".to_string(), None)));
        assert_eq!(g.host.calls_of("forking_remove").len(), 1);
    });
}

#[test]
fn stand_up_queued_restarts_the_clock_after_a_wait() {
    run(async {
        // A wait in the queue is the strip's to say: the start's own time counts from its end.
        let (f, record) = queued(json!({ "notSent": "an old why" }));
        f.host.store.borrow_mut().entry(SEED_CHOICES_KEY.to_string()).or_default().clear();
        f.host.clock.advance(300_000.0).await;
        let (r, _) = start(&f, &record, true).await;
        r.unwrap();
        let spawned = spawn_of(&record);
        assert_eq!(spawned["startedAt"], json!(T + 300_000));
        // The picks are read again now: with none, the why is the new one.
        assert_eq!(spawned["notSent"], json!("nothing is picked for web"));
        assert_eq!(f.host.calls_of("stage_reporter")[0][2], (T + 300_000).to_string());

        // Started at once, the clock stands where the spawn put it, and picks again found clear a why left before.
        let (g, held) = queued(json!({ "notSent": "an old why" }));
        g.host.clock.advance(300_000.0).await;
        let (r, _) = start(&g, &held, false).await;
        r.unwrap();
        let spawned = spawn_of(&held);
        assert_eq!(spawned["startedAt"], json!(T));
        assert!(spawned.get("notSent").is_none());
    });
}

/// A start whose machine answers `needle` with `reply`, run to its failure; `unfork` is what deleting the machine answers.
async fn broken(needle: &str, reply: Reply, unfork: Option<AreaError>) -> (Fixture, Rec, AreaError) {
    let (f, record) = queued(json!({}));
    w().borrow_mut().machine_answers.push((needle.to_string(), reply));
    if let Some(e) = unfork {
        f.host.script("unfork", Err(e));
    }
    let (r, _) = start(&f, &record, false).await;
    let e = r.expect_err("the start went ahead");
    (f, record, e)
}

#[test]
fn a_machine_that_cannot_make_the_folder_is_dropped_and_the_start_fails_with_what_it_said() {
    run(async {
        let (f, record, e) = broken("install -d", exit(1, "no space left on device\n"), None).await;
        let said = "/root/web was not made on bx_1 (exit 1): no space left on device";
        assert_eq!(said_of(&e), said);
        // The machine the record held is deleted and let go, with no retry: only the three faults a fresh machine cures are.
        assert_eq!(f.host.calls_of("unfork"), vec![vec![SUB_WS.to_string(), "bx_1".to_string()]]);
        assert_eq!(f.host.calls_of("remove_live"), vec![vec![SUB_WS.to_string()]]);
        assert_eq!((killed(&f), creates(&f)), (vec![true], 1));
        assert_eq!(record.borrow().machine_id, "");
        assert!(f.host.calls_of("delete_create").is_empty() && f.host.warns.borrow().is_empty());
        assert_eq!(spawn_of(&record)["failed"], json!({ "step": "machine", "said": said }));
        assert_eq!(f.host.reported().last().cloned(), Some(("failed".to_string(), said.to_string(), None)));
        assert!(f.host.stored(SUB_WS).is_none());

        // With nothing on stderr it is what the command wrote.
        let (_, _, e) = broken("install -d", Reply::Out { code: 1, stdout: "read-only file system\n".into(), stderr: String::new() }, None).await;
        assert_eq!(said_of(&e), "/root/web was not made on bx_1 (exit 1): read-only file system");

        // The files that did not land are the files step's failure, once the archive was put.
        let (_, record, e) = broken("tar -xzf", exit(2, "tar: short read\n"), None).await;
        assert_eq!(said_of(&e), "the picked files did not land in /root/web (exit 2): tar: short read");
        let failed = spawn_of(&record)["failed"].clone();
        assert_eq!((failed["step"].clone(), failed["said"].clone()), (json!("files"), json!("the picked files did not land in /root/web (exit 2): tar: short read")));
    });
}

#[test]
fn says_a_machine_it_could_not_delete_and_stays_quiet_about_one_already_gone() {
    run(async {
        let (f, _, e) = broken("install -d", exit(1, "full"), Some(failed("the provider refused"))).await;
        assert_eq!(said_of(&e), "/root/web was not made on bx_1 (exit 1): full");
        assert_eq!(f.host.warns.borrow().clone(), vec![format!("workspace {SUB_WS}: bx_1 was not deleted: the provider refused")]);
        // A machine the provider no longer holds is gone, and that is no warning.
        let gone = AreaError::Engine(EngineError::Provider(WspError { kind: ErrorKind::Missing, status: 404, code: None, message: "machine bx_1 is gone".into(), request_id: None, window: None, said: None }));
        let (g, record, _) = broken("install -d", exit(1, "full"), Some(gone)).await;
        assert!(g.host.warns.borrow().is_empty());
        assert_eq!(record.borrow().machine_id, "");
        assert_eq!(g.host.calls_of("remove_live").len(), 1);
    });
}

#[test]
fn asks_the_daemon_where_the_machine_has_a_road_to_it() {
    run(async {
        let (f, record) = queued(json!({}));
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.daemon_road = true;
            world.ping_fault = Some("nothing answered on port 7001".into());
        }
        let (r, _) = start(&f, &record, false).await;
        r.unwrap();
        // It is asked once, and the answer, or what it lacked, is the stage's; then the daemon's own sync starts unawaited.
        assert_eq!(f.host.calls_of("ping_daemon"), vec![vec![SUB_WS.to_string()]]);
        assert_eq!(f.host.calls_of("sync_daemon"), vec![vec![SUB_WS.to_string()]]);
        assert!(f.host.at("ping_daemon") < f.host.at("sync_daemon"));
        let heard: Vec<_> = f.host.reported().into_iter().filter(|(stage, _, _)| stage == "daemon-answering").collect();
        assert_eq!(heard, vec![("daemon-answering".to_string(), "Daemon did not answer.".to_string(), Some(json!({ "notice": "nothing answered on port 7001" })))]);
        assert!(spawn_of(&record)["log"].as_array().unwrap().contains(&json!("Daemon did not answer.")));
        // Before the folder is made: the ping is ahead of the first command on the machine.
        assert_eq!(machines(&f)[0].log()[0], INSTALL);

        let (g, held) = queued(json!({}));
        {
            let world = w();
            let mut world = world.borrow_mut();
            world.daemon_road = true;
            world.ping_fault = None;
        }
        let (r, _) = start(&g, &held, false).await;
        r.unwrap();
        assert!(spawn_of(&held)["log"].as_array().unwrap().contains(&json!("Daemon answered.")));
    });
}
