// SPDX-License-Identifier: AGPL-3.0-only
//! `GoneWatch` (golden.ts:200-260), ported from gone-watch.test.ts at 65f6b89c7: a background stop read back through a
//! gateway whose copies disagree (the TS's `splitGateway`, a fetch over the real `SolariBackend`), and the machines the
//! sweeps and a rebuilt workspace hand to it. The reads of `readGone` the same TS file holds are in tests/gone.rs.
#[path = "rest_common/mod.rs"]
mod common;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use indexmap::IndexMap;
use serde_json::json;
use tokio::task::LocalSet;
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::gone::{KillConfirm, KILL_ASKS};
use wsp_engine::gone_watch::{GoneWatch, GoneWatchOptions, GONE_RETRY_MS};
use wsp_engine::lifecycle::{Workspace, WorkspaceHooks, WorkspaceInitial};
use wsp_engine::machine::{Machine, MachineBackend, MachineListRow, MachineSpec};
use wsp_engine::orphans::{reap, ReapOptions};
use wsp_engine::solari_backend::{SolariBackend, SolariBackendOptions};

use common::{reply, FakeFetch, Play, Seen, Warned};

const CONFIRM: KillConfirm = KillConfirm { grace_ms: Some(50), poll_ms: Some(1) };

#[derive(Clone, Copy, PartialEq)]
enum Copy_ {
    Holder,
    Empty,
}

/// split-gateway.ts: two copies behind one address, one holding every running sandbox and one that never heard of
/// them. The empty copy answers a delete with the same 200 and every read with a 404; only a delete that lands on the
/// holder ends the machine. `route` names the copy each call after a create lands on.
struct Gateway {
    fetch: Arc<FakeFetch>,
    holder: Rc<RefCell<IndexMap<String, String>>>,
}

impl Gateway {
    fn new(route: impl Fn(u32, &str) -> Copy_ + 'static) -> Gateway {
        let holder: Rc<RefCell<IndexMap<String, String>>> = Rc::default();
        let (state, call, made) = (holder.clone(), Cell::new(0u32), Cell::new(0u32));
        let fetch = FakeFetch::handled(move |seen: &Seen| -> Option<Play> {
            if seen.method == "POST" {
                made.set(made.get() + 1);
                let id = format!("sb{}", made.get());
                state.borrow_mut().insert(id.clone(), "running".into());
                return Some(reply(201, json!({ "sandboxId": id, "kind": "sandbox" })));
            }
            let id = seen.path.split('/').nth(2).unwrap_or("").to_string();
            let n = call.get();
            call.set(n + 1);
            let known = route(n, &seen.method) == Copy_::Holder && state.borrow().contains_key(&id);
            if seen.method == "DELETE" {
                if known {
                    state.borrow_mut().shift_remove(&id);
                }
                return Some(reply(200, json!({ "ok": true })));
            }
            Some(if known {
                reply(200, json!({ "sandboxId": id, "kind": "sandbox", "state": state.borrow()[&id] }))
            } else {
                reply(404, json!({ "error": "Not found" }))
            })
        });
        Gateway { fetch, holder }
    }

    fn deletes(&self) -> usize {
        self.fetch.sent("DELETE /sandboxes/sb1") + self.fetch.sent("DELETE /sandboxes/sb2")
    }

    fn held(&self) -> Vec<String> {
        self.holder.borrow().keys().cloned().collect()
    }

    fn backend(&self) -> Arc<SolariBackend> {
        SolariBackend::new(SolariBackendOptions {
            api_key: "k".into(),
            fetch: Some(self.fetch.clone()),
            warn: Some(Warned::default().sink()),
            ..Default::default()
        })
    }
}

/// Every read reaches the holder; the first `empty` deletes land on the copy that never held the machine.
fn empty_deletes(empty: u32) -> impl Fn(u32, &str) -> Copy_ {
    let seen = Cell::new(0u32);
    move |_call, method| {
        if method == "DELETE" {
            seen.set(seen.get() + 1);
            if seen.get() <= empty {
                return Copy_::Empty;
            }
        }
        Copy_::Holder
    }
}

fn sandbox() -> MachineSpec {
    serde_json::from_value(json!({ "kind": "sandbox" })).unwrap()
}

fn watch(retry_ms: Option<u64>, warn: Option<Arc<Mutex<Vec<String>>>>) -> GoneWatch {
    GoneWatch::new(GoneWatchOptions {
        confirm: CONFIRM,
        retry_ms,
        warn: Some(match warn {
            Some(said) => Arc::new(move |line: &str| said.lock().unwrap().push(line.to_string())),
            None => Arc::new(|_: &str| {}),
        }),
        clock: None,
    })
}

fn local<T>(f: impl std::future::Future<Output = T>) -> T {
    tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap().block_on(LocalSet::new().run_until(f))
}

fn alive_words(id: &str) -> String {
    EngineError::machine_alive(id, wsp_engine::machine::MachineState::Running).to_string()
}

// gone-watch.test.ts:20
#[test]
fn a_stop_resolves_on_the_providers_first_yes_and_keeps_asking_behind_it_until_the_holder_lets_the_machine_go() {
    local(async {
        let g = Gateway::new(empty_deletes(1));
        let b = g.backend();
        let machine = b.create(sandbox(), None, None).await.unwrap();
        let w = watch(None, None);
        w.stop(b.clone(), machine.clone()).await.unwrap();
        assert_eq!(g.deletes(), 1);
        assert_eq!(w.ids(), vec![machine.id().to_string()]);
        w.settled().await;
        assert_eq!(g.held(), Vec::<String>::new());
        assert_eq!(g.deletes(), 2);
        assert_eq!(w.ids(), Vec::<String>::new());
    });
}

// gone-watch.test.ts:36
#[test]
fn a_read_back_that_ends_with_the_machine_still_there_is_said_and_the_stop_is_asked_again_after_the_wait_until_it_reads_gone() {
    local(async {
        let g = Gateway::new(empty_deletes(KILL_ASKS));
        let b = g.backend();
        let machine = b.create(sandbox(), None, None).await.unwrap();
        let said: Arc<Mutex<Vec<String>>> = Arc::default();
        let w = watch(Some(5), Some(said.clone()));
        w.stop(b.clone(), machine).await.unwrap();
        w.settled().await;
        assert_eq!(g.held(), Vec::<String>::new());
        assert_eq!(g.deletes(), KILL_ASKS as usize + 1);
        assert_eq!(
            *said.lock().unwrap(),
            vec![format!("{}; asking again in 0 s", alive_words("sb1")), "machine sb1 read gone after asking again".to_string()]
        );
    });
}

// gone-watch.test.ts:51
#[test]
fn a_watch_closed_while_it_waits_to_ask_again_asks_nothing_more() {
    local(async {
        let g = Gateway::new(empty_deletes(u32::MAX));
        let b = g.backend();
        let machine = b.create(sandbox(), None, None).await.unwrap();
        let w = watch(Some(GONE_RETRY_MS), None);
        w.stop(b.clone(), machine).await.unwrap();
        while g.deletes() < KILL_ASKS as usize {
            tokio::task::yield_now().await;
        }
        w.close();
        w.settled().await;
        assert_eq!(g.deletes(), KILL_ASKS as usize);
        assert_eq!(g.held(), vec!["sb1".to_string()]);
    });
}

// gone-watch.test.ts:63
#[test]
fn the_orphan_reap_hands_each_machine_to_the_watch_and_a_delete_the_empty_copy_took_is_asked_again() {
    local(async {
        let g = Gateway::new(empty_deletes(1));
        let b = g.backend();
        let machine = b.create(sandbox(), None, None).await.unwrap();
        let w = Rc::new(watch(None, None));
        let listing: Vec<MachineListRow> = serde_json::from_value(json!([{
            "id": machine.id(), "state": "running",
            "labels": { "wsp": "1", "wsp-owner": "h_me", "createdAt": "2026-09-01T00:00:00Z" }
        }]))
        .unwrap();
        let (stopper, backend) = (w.clone(), b.clone() as Arc<dyn MachineBackend>);
        let done = reap(ReapOptions {
            backend: backend.clone(),
            listing: Some(listing),
            known_ids: Box::new(Vec::new),
            owner: "h_me".into(),
            stop: Arc::new(move |m| {
                let (stopper, backend) = (stopper.clone(), backend.clone());
                async move { stopper.stop(backend, m).await }.boxed_local()
            }),
            older_than_ms: None,
            now: None,
        })
        .await
        .unwrap();
        assert_eq!(done.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec![machine.id().to_string()]);
        w.settled().await;
        assert_eq!(g.held(), Vec::<String>::new());
    });
}

// gone-watch.test.ts:77
#[test]
fn a_rebuilt_workspace_retires_the_machine_it_replaced_through_the_watch_so_the_old_one_ends_gone_at_the_holder() {
    local(async {
        let g = Gateway::new(empty_deletes(1));
        let b = g.backend();
        let old = b.create(sandbox(), None, None).await.unwrap();
        let w = Rc::new(watch(None, None));
        let (maker, stopper, backend) = (b.clone(), w.clone(), b.clone() as Arc<dyn MachineBackend>);
        let hooks = WorkspaceHooks {
            golden_snapshot: "snap_g".into(),
            wake_attempts: 1,
            resurrect: Some(Arc::new(move |_spec| {
                let maker = maker.clone();
                async move { maker.create(sandbox(), None, None).await }.boxed_local()
            })),
            vault_export: None,
            vault_import: None,
            stash_vault: None,
            restore_vault: None,
            wake_check: None,
            move_hook: None,
            retire: Some(Arc::new(move |m| {
                let (stopper, backend) = (stopper.clone(), backend.clone());
                async move { stopper.stop(backend, m).await }.boxed_local()
            })),
        };
        let ws = Workspace::new(old, hooks, WorkspaceInitial::default());
        ws.rebuild().await.unwrap();
        w.settled().await;
        assert_eq!(ws.machine_id(), "sb2");
        assert_eq!(g.held(), vec!["sb2".to_string()]);
    });
}

/// A machine whose kill is refused from its second ask on, as the provider answers a key that stopped working.
struct Refusing {
    kills: Cell<u32>,
    status: u16,
    kind: ErrorKind,
}

#[async_trait::async_trait(?Send)]
impl Machine for Refusing {
    fn id(&self) -> &str {
        "m1"
    }
    fn kind(&self) -> wsp_engine::machine::MachineKind {
        wsp_engine::machine::MachineKind::Sandbox
    }
    fn seen(&self) -> Option<&wsp_engine::machine::Seen> {
        static RUNNING: std::sync::OnceLock<wsp_engine::machine::Seen> = std::sync::OnceLock::new();
        Some(RUNNING.get_or_init(|| wsp_engine::machine::Seen {
            state: wsp_engine::machine::MachineState::Running,
            created_at: None,
            paused_at: None,
        }))
    }
    async fn exec(&self, _: &str, _: wsp_engine::machine::ExecOptions) -> wsp_engine::errors::Result<wsp_engine::machine::ExecResult> {
        unreachable!()
    }
    async fn run(&self, _: &str, _: wsp_engine::machine::RunOptions) -> wsp_engine::errors::Result<wsp_engine::machine::ExecResult> {
        unreachable!()
    }
    async fn snapshot(
        &self,
        _: &str,
        _: wsp_engine::machine::MachineLife,
        _: wsp_engine::machine::SnapshotOptions,
    ) -> wsp_engine::errors::Result<String> {
        unreachable!()
    }
    async fn pause(&self, _: Option<u64>) -> wsp_engine::errors::Result<()> {
        unreachable!()
    }
    async fn resume(&self, _: Option<&wsp_engine::Cancel>) -> wsp_engine::errors::Result<()> {
        unreachable!()
    }
    async fn kill(&self) -> wsp_engine::errors::Result<()> {
        self.kills.set(self.kills.get() + 1);
        if self.kills.get() > 1 {
            return Err(EngineError::Refused {
                message: format!("refused {}", self.status),
                kind: Some(self.kind),
                status: Some(self.status),
                code: None,
            });
        }
        Ok(())
    }
    async fn state(&self) -> wsp_engine::errors::Result<wsp_engine::machine::MachineState> {
        Ok(wsp_engine::machine::MachineState::Running)
    }
    async fn download_url(&self, _: &str) -> wsp_engine::errors::Result<String> {
        unreachable!()
    }
    async fn upload_url(&self, _: &str) -> wsp_engine::errors::Result<String> {
        unreachable!()
    }
}

/// A backend whose `get` hands back the machine it was made over; the rest of the trait is never reached.
struct OneMachine(Arc<Refusing>);

#[async_trait::async_trait(?Send)]
impl MachineBackend for OneMachine {
    fn capabilities(&self) -> wsp_engine::machine::Capabilities {
        wsp_engine::no_provider_backend::NoProviderBackend::new(None).capabilities()
    }
    fn pricing(&self) -> Arc<dyn wsp_engine::machine::BackendPricing> {
        Arc::new(wsp_engine::no_provider_backend::NoProviderPricing)
    }
    async fn create(
        &self,
        _: MachineSpec,
        _: Option<wsp_engine::machine::Waiting>,
        _: Option<String>,
    ) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        unreachable!()
    }
    async fn get(&self, _id: &str) -> wsp_engine::errors::Result<Arc<dyn Machine>> {
        Ok(self.0.clone())
    }
    async fn list(&self, _: Option<&IndexMap<String, String>>) -> wsp_engine::errors::Result<Vec<MachineListRow>> {
        unreachable!()
    }
    async fn delete_snapshot(&self, _: &str) -> wsp_engine::errors::Result<()> {
        unreachable!()
    }
}

// gone-watch.test.ts:94 `it.each([401, 402, 403])`
#[test]
fn a_refusal_of_the_account_on_a_later_ask_ends_the_watch_with_one_line_and_asks_no_more() {
    for (status, kind) in [(401u16, ErrorKind::Auth), (402, ErrorKind::Plan), (403, ErrorKind::Plan)] {
        local(async move {
            let machine = Arc::new(Refusing { kills: Cell::new(0), status, kind });
            let backend = Arc::new(OneMachine(machine.clone()));
            let said: Arc<Mutex<Vec<String>>> = Arc::default();
            let w = watch(Some(5), Some(said.clone()));
            w.stop(backend, machine.clone()).await.unwrap();
            w.settled().await;
            assert_eq!(machine.kills.get(), 2, "{status}");
            assert_eq!(
                *said.lock().unwrap(),
                vec![format!("refused {status}; not asked again, the next start's sweep stops machine m1")],
                "{status}"
            );
            assert_eq!(w.ids(), Vec::<String>::new());
        });
    }
}

// golden.ts:223 `stop`: a kill the provider answers 404 for is a machine already gone; any other refusal of the first ask
// is the caller's.
#[test]
fn a_missing_machine_is_no_failure_and_any_other_refusal_of_the_first_ask_is_thrown_with_nothing_watched() {
    local(async {
        let g = Gateway::new(|_, _| Copy_::Holder);
        let b = g.backend();
        let machine = b.create(sandbox(), None, None).await.unwrap();
        g.holder.borrow_mut().clear();
        let w = watch(None, None);
        w.stop(b.clone(), machine).await.unwrap();
        w.settled().await;
        assert_eq!(w.ids(), Vec::<String>::new());

        let refused = SolariBackend::new(SolariBackendOptions {
            api_key: "k".into(),
            fetch: Some(FakeFetch::handled(|seen| match seen.method.as_str() {
                "POST" => Some(reply(201, json!({ "sandboxId": "sb1", "kind": "sandbox" }))),
                _ => Some(reply(409, json!({ "error": "busy" }))),
            })),
            warn: Some(Warned::default().sink()),
            ..Default::default()
        });
        let machine = refused.create(sandbox(), None, None).await.unwrap();
        let e = w.stop(refused.clone(), machine).await.expect_err("a refusal");
        assert_eq!(e.status(), Some(409));
        assert_eq!(w.ids(), Vec::<String>::new());
    });
}

// golden.ts:223 `stop`: a stop for a machine already watched, or after close(), sends the kill and starts no second watch.
#[test]
fn a_second_stop_of_a_watched_machine_and_a_stop_after_close_start_no_watch() {
    local(async {
        let g = Gateway::new(empty_deletes(u32::MAX));
        let b = g.backend();
        let machine = b.create(sandbox(), None, None).await.unwrap();
        let w = watch(Some(GONE_RETRY_MS), None);
        w.stop(b.clone(), machine.clone()).await.unwrap();
        w.stop(b.clone(), machine.clone()).await.unwrap();
        assert_eq!(w.ids(), vec!["sb1".to_string()]);
        assert_eq!(g.deletes(), 2, "each stop asks the provider");
        w.close();
        w.settled().await;
        let before = g.deletes();
        w.stop(b.clone(), machine).await.unwrap();
        assert_eq!(g.deletes(), before + 1, "a stop after close still sends its first ask");
        assert_eq!(w.ids(), Vec::<String>::new());
    });
}
