// SPDX-License-Identifier: AGPL-3.0-only
//! The helpers of packages/engine/src/golden.ts that read a deleted machine or snapshot back until the provider says it
//! is gone: sightMachine, readGone, killUntilGone and snapshotUntilGone. The cases follow gone-watch.test.ts (the reads
//! through a gateway whose copies disagree), solari-backend.test.ts:993-1025 (a delete read back) and golden.test.ts:3101
//! (snapshotUntilGone).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_engine::errors::{classify, EngineError, Result, RetryClock};
use wsp_engine::gone::{
    kill_until_gone, read_gone, sight_machine, snapshot_until_gone, KillConfirm, SnapshotDeleted, GONE_READS, KILL_ASKS,
};
use wsp_engine::machine::{
    BackendPricing, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife, MachineListRow, MachineSpec,
    MachineState, RunOptions, Seen, SnapshotOptions, SnapshotRow, Waiting,
};
use wsp_engine::no_provider_backend::NoProviderPricing;

/// How one `get` answers: the machine with its state on the handle, a handle with none whose own state read answers,
/// or the provider's 404.
#[derive(Clone, Copy)]
enum Get {
    Running,
    Paused,
    Missing,
    Bare(MachineState),
    BareMissing,
    Failed,
}

fn not_found() -> EngineError {
    EngineError::Provider(classify(404, None, Some("Not found"), None))
}

fn refused(status: u16) -> EngineError {
    EngineError::Provider(classify(status, None, Some("refused"), None))
}

#[derive(Default)]
struct Script {
    gets: RefCell<VecDeque<Get>>,
    last: Cell<Option<Get>>,
    calls: RefCell<Vec<String>>,
    kill_answers: RefCell<VecDeque<Result<()>>>,
    delete_answer: RefCell<Option<EngineError>>,
    listings: RefCell<VecDeque<Result<Vec<String>>>>,
    last_listing: RefCell<Option<Result<Vec<String>>>>,
    lists: bool,
}

impl Script {
    fn answer(&self) -> Get {
        let next = self.gets.borrow_mut().pop_front();
        match next {
            Some(a) => {
                self.last.set(Some(a));
                a
            }
            None => self.last.get().expect("a script with an answer"),
        }
    }
    fn count(&self, what: &str) -> usize {
        self.calls.borrow().iter().filter(|c| *c == what).count()
    }
}

struct Handle {
    script: Rc<Script>,
    seen: Option<Seen>,
    state: Result<MachineState>,
}

#[async_trait(?Send)]
impl Machine for Handle {
    fn id(&self) -> &str {
        "m1"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn seen(&self) -> Option<&Seen> {
        self.seen.as_ref()
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        unreachable!("a read-back runs nothing on the machine")
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        unreachable!("a read-back runs nothing on the machine")
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        unreachable!()
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        unreachable!()
    }
    async fn resume(&self, _signal: Option<&wsp_engine::Cancel>) -> Result<()> {
        unreachable!()
    }
    async fn kill(&self) -> Result<()> {
        self.script.calls.borrow_mut().push("kill".into());
        self.script.kill_answers.borrow_mut().pop_front().unwrap_or(Ok(()))
    }
    async fn state(&self) -> Result<MachineState> {
        self.script.calls.borrow_mut().push("state".into());
        self.state.clone()
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        unreachable!()
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        unreachable!()
    }
}

fn seen(state: MachineState) -> Option<Seen> {
    Some(Seen { state, created_at: None, paused_at: None })
}

struct Backend {
    script: Rc<Script>,
}

#[async_trait(?Send)]
impl MachineBackend for Backend {
    fn capabilities(&self) -> Capabilities {
        let mut caps = wsp_engine::no_provider_backend::NoProviderBackend::new(None).capabilities();
        caps.snapshot_listing = self.script.lists;
        caps
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        Arc::new(NoProviderPricing)
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        unreachable!()
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        self.script.calls.borrow_mut().push("get".into());
        let script = self.script.clone();
        let (seen_as, state) = match script.answer() {
            Get::Running => (seen(MachineState::Running), Ok(MachineState::Running)),
            Get::Paused => (seen(MachineState::Paused), Ok(MachineState::Paused)),
            Get::Missing => return Err(not_found()),
            Get::Bare(state) => (None, Ok(state)),
            Get::BareMissing => (None, Err(not_found())),
            Get::Failed => return Err(refused(500)),
        };
        Ok(Arc::new(Handle { script, seen: seen_as, state }))
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        unreachable!()
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        self.script.calls.borrow_mut().push("delete".into());
        match self.script.delete_answer.borrow().clone() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
    async fn list_snapshots(&self, ids: Option<&[String]>) -> Result<Vec<SnapshotRow>> {
        assert_eq!(ids, Some(&["snap_1".to_string()][..]), "asked for the one id");
        self.script.calls.borrow_mut().push("list".into());
        let next = self.script.listings.borrow_mut().pop_front();
        let answer = match next {
            Some(a) => {
                *self.script.last_listing.borrow_mut() = Some(a.clone());
                a
            }
            None => self.script.last_listing.borrow().clone().expect("a listing"),
        };
        answer.map(|ids| {
            ids.into_iter()
                .map(|id| serde_json::from_value(serde_json::json!({ "id": id, "sizeBytes": 1 })).expect("a snapshot row"))
                .collect()
        })
    }
}

struct Manual {
    at: Cell<i64>,
}

#[async_trait(?Send)]
impl RetryClock for Manual {
    fn now(&self) -> i64 {
        self.at.get()
    }
    async fn sleep(&self, ms: u64) {
        self.at.set(self.at.get() + ms as i64);
    }
    fn random(&self) -> f64 {
        0.0
    }
}

fn clock() -> Manual {
    Manual { at: Cell::new(1_000) }
}

fn scripted(gets: &[Get]) -> (Backend, Rc<Script>) {
    let script = Rc::new(Script { gets: RefCell::new(gets.iter().copied().collect()), ..Script::default() });
    (Backend { script: script.clone() }, script)
}

fn run<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(f)
}

// gone-watch.test.ts:104 `a 404 from the copy that never held the machine is not gone`
#[test]
fn a_404_from_the_copy_that_never_held_the_machine_is_not_gone() {
    let (backend, script) = scripted(&[Get::Missing, Get::Running]);
    assert_eq!(run(read_gone(&backend, "m1")).unwrap(), MachineState::Running);
    assert_eq!(script.count("get"), 2);
}

// gone-watch.test.ts:113
#[test]
fn a_machine_neither_copy_holds_reads_gone_only_after_gone_reads_in_a_row() {
    let (backend, script) = scripted(&[Get::Missing]);
    assert_eq!(run(read_gone(&backend, "m1")).unwrap(), MachineState::Gone);
    assert_eq!(script.count("get"), GONE_READS as usize);
}

// gone-watch.test.ts:127 `a 404 from a bare handle's state read counts as a gone read, not a failure`
#[test]
fn a_404_from_a_bare_handles_state_read_counts_as_a_gone_read() {
    let (backend, script) = scripted(&[Get::BareMissing]);
    assert_eq!(run(read_gone(&backend, "m1")).unwrap(), MachineState::Gone);
    assert_eq!((script.count("get"), script.count("state")), (GONE_READS as usize, GONE_READS as usize));
}

// golden.ts:132 readOnce: a handle with no state on it is asked, once
#[test]
fn a_bare_handle_is_asked_its_state_and_a_paused_machine_is_not_gone() {
    let (backend, script) = scripted(&[Get::Bare(MachineState::Paused)]);
    let sighted = run(sight_machine(&backend, "m1")).unwrap();
    assert_eq!(sighted.state, MachineState::Paused);
    assert_eq!(sighted.machine.map(|m| m.id().to_string()), Some("m1".to_string()), "the handle it came on");
    assert_eq!((script.count("get"), script.count("state")), (1, 1));
    let (backend, _) = scripted(&[Get::Paused]);
    assert_eq!(run(read_gone(&backend, "m1")).unwrap(), MachineState::Paused);
}

// golden.ts:141: a failed read that is not a 404 fails the sighting
#[test]
fn a_read_that_fails_and_is_no_404_fails_the_sighting() {
    let (backend, script) = scripted(&[Get::Missing, Get::Failed]);
    let err = run(sight_machine(&backend, "m1")).err().expect("a failure");
    assert_eq!(err.status(), Some(500));
    assert_eq!(script.count("get"), 2);
}

// solari-backend.test.ts:993 `a delete that lands on the copy that never held the machine is not gone`
#[test]
fn a_kill_that_lands_on_the_copy_that_never_held_the_machine_is_asked_again_on_the_read_that_finds_it() {
    // The kill is asked, the first read finds nothing, the second finds the machine still there: asked again at once (not
    // when a minute's grace ran out), then every read agrees it is gone.
    let (backend, script) = scripted(&[Get::Missing, Get::Running, Get::Missing]);
    let machine = Handle { script: script.clone(), seen: None, state: Ok(MachineState::Gone) };
    let clock = clock();
    run(kill_until_gone(&backend, &machine, KillConfirm { grace_ms: Some(60_000), poll_ms: Some(1) }, &clock)).unwrap();
    let calls = script.calls.borrow().clone();
    assert_eq!(&calls[..4], ["kill", "get", "get", "kill"]);
    assert!(calls[4..].iter().all(|c| c == "get") && calls.len() - 4 == GONE_READS as usize, "{calls:?}");
    assert!(clock.now() < 1_000 + 60_000, "asked again on the read, not when the grace ran out");
}

// solari-backend.test.ts:1022 `copies that disagree on every read end in one sentence naming the machine still running`
#[test]
fn a_machine_still_running_after_three_kills_is_machine_alive_naming_it() {
    let (backend, script) = scripted(&[Get::Running]);
    let machine = run(backend.get("m1")).unwrap();
    script.calls.borrow_mut().clear();
    let clock = clock();
    let err = run(kill_until_gone(&backend, machine.as_ref(), KillConfirm { grace_ms: Some(5), poll_ms: Some(1) }, &clock)).err().unwrap();
    assert_eq!(err, EngineError::machine_alive("m1", MachineState::Running));
    assert_eq!(err.name(), "MachineAliveError");
    assert_eq!(err.to_string(), "machine m1 is still running after three kills; it bills until reap or a kill by hand takes");
    assert_eq!(script.count("kill"), KILL_ASKS as usize);
}

// golden.ts:119: a kill the provider says is already missing is no failure; any other refusal is
#[test]
fn a_kill_the_provider_says_is_missing_goes_on_and_any_other_refusal_stops_it() {
    let (backend, script) = scripted(&[Get::Missing]);
    let machine = Handle { script: script.clone(), seen: None, state: Ok(MachineState::Gone) };
    script.kill_answers.borrow_mut().push_back(Err(not_found()));
    run(kill_until_gone(&backend, &machine, KillConfirm::default(), &clock())).unwrap();
    assert_eq!(script.count("kill"), 1);

    let (backend, script) = scripted(&[Get::Missing]);
    let machine = Handle { script: script.clone(), seen: None, state: Ok(MachineState::Gone) };
    script.kill_answers.borrow_mut().push_back(Err(refused(403)));
    let err = run(kill_until_gone(&backend, &machine, KillConfirm::default(), &clock())).err().unwrap();
    assert_eq!(err.status(), Some(403));
    assert_eq!((script.count("kill"), script.count("get")), (1, 0));
}

// golden.ts:119: a failed read ends the kill with the failure
#[test]
fn a_read_that_fails_while_a_kill_is_read_back_ends_it() {
    let (backend, script) = scripted(&[Get::Failed]);
    let machine = Handle { script: script.clone(), seen: None, state: Ok(MachineState::Gone) };
    let err = run(kill_until_gone(&backend, &machine, KillConfirm::default(), &clock())).err().unwrap();
    assert_eq!(err.status(), Some(500));
}

fn listing(lists: bool, listings: Vec<Result<Vec<String>>>) -> (Backend, Rc<Script>) {
    let script = Rc::new(Script { lists, listings: RefCell::new(listings.into()), ..Script::default() });
    (Backend { script: script.clone() }, script)
}

const QUICK: KillConfirm = KillConfirm { grace_ms: Some(30), poll_ms: Some(1) };

fn ids(of: &[&str]) -> Result<Vec<String>> {
    Ok(of.iter().map(|s| s.to_string()).collect())
}

// golden.test.ts:3118
#[test]
fn a_snapshot_is_deleted_once_the_listing_no_longer_holds_its_id_a_failed_read_counting_as_one_that_still_does() {
    let (backend, script) = listing(true, vec![ids(&["snap_1", "snap_2"]), Err(refused(502)), ids(&["snap_2"])]);
    assert_eq!(run(snapshot_until_gone(&backend, "snap_1", QUICK, &clock())).unwrap(), SnapshotDeleted::Deleted);
    assert_eq!((script.count("delete"), script.count("list")), (1, 3));
}

// golden.test.ts:3127
#[test]
fn a_snapshot_the_provider_had_lost_is_missing_without_a_read_and_any_other_refusal_fails() {
    let (backend, script) = listing(true, vec![ids(&["snap_1"])]);
    *script.delete_answer.borrow_mut() = Some(not_found());
    assert_eq!(run(snapshot_until_gone(&backend, "snap_1", QUICK, &clock())).unwrap(), SnapshotDeleted::Missing);
    assert_eq!(script.count("list"), 0);

    let (backend, script) = listing(true, vec![ids(&["snap_1"])]);
    *script.delete_answer.borrow_mut() = Some(refused(409));
    let err = run(snapshot_until_gone(&backend, "snap_1", QUICK, &clock())).err().unwrap();
    assert_eq!(err.status(), Some(409));
    assert_eq!(script.count("list"), 0);
}

// golden.test.ts:3139
#[test]
fn a_snapshot_that_outlives_the_window_is_listed_with_it_and_a_provider_that_lists_nothing_answers_on_the_delete() {
    let (backend, script) = listing(true, vec![ids(&["snap_1"])]);
    assert_eq!(run(snapshot_until_gone(&backend, "snap_1", QUICK, &clock())).unwrap(), SnapshotDeleted::Listed { grace_ms: 30 });
    assert!(script.count("list") > 1);

    let (backend, script) = listing(false, vec![ids(&["snap_1"])]);
    assert_eq!(run(snapshot_until_gone(&backend, "snap_1", QUICK, &clock())).unwrap(), SnapshotDeleted::Deleted);
    assert_eq!(script.count("list"), 0);
}

// exit.ts:37 reads the class off the kind stamped where the error was born, so an engine error keeps it on its way to a door.
#[test]
fn an_engine_error_reaches_a_door_with_its_kind_status_and_code() {
    use wsp_words::exit::{HostError, Kind};
    let alive = HostError::from(EngineError::machine_alive("m1", MachineState::Paused));
    assert_eq!(alive.kind, Some(Kind::MachineAlive));
    assert_eq!(alive.message, "machine m1 is still paused after three kills; it bills until reap or a kill by hand takes");
    let conflict = HostError::from(refused(409));
    assert_eq!((conflict.kind, conflict.status), (Some(Kind::Conflict), Some(409.0)));
    let exists = HostError::from(wsp_engine::vault::dest_exists("/out", 2));
    assert_eq!(
        (exists.kind, exists.message.as_str()),
        (Some(Kind::Exists), "/out already exists on this computer with 2 files; export with replace to overwrite it")
    );
    let plain = HostError::from(EngineError::Other { message: "no".into(), code: Some("ENOENT".into()) });
    assert_eq!((plain.kind, plain.code.as_deref()), (None, Some("ENOENT")));
}
