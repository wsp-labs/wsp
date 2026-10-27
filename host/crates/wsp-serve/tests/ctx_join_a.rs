// SPDX-License-Identifier: AGPL-3.0-only
//! create_runtime join A (#2476): what the join changed on the context, read over a runtime on `FakeBackend`. The
//! ctx_* files of the packets are at the 1,500 line limit, so the join's own tests live here.
use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::sync::Arc;

use futures::FutureExt as _;
use serde_json::json;
use wsp_account::account::channels::ChannelsCtx;
use wsp_account::projects::workspaces::phases::PhasesCtx;
use wsp_account::projects::workspaces::seam::{Base, Entry};
use wsp_adapters::port::{AdapterError, HarnessAdapter, HarnessSession, HarnessStartOptions};
use wsp_core::types::api::Runtime as _;
use wsp_core::types::wiring::{RuntimeOptions, WorkspaceRecord};
use wsp_engine::fake_backend::{FakeBackend, FakeBackendOptions};
use wsp_machines::machines::boot::BootRuntime;
use wsp_serve::runtime::ctx::Ctx;
use wsp_serve::{create_runtime, AssembledRuntime};
use wsp_store::store::memory_store;
use wsp_threads::threads::launch::LaunchCtx;
use wsp_threads::threads::turns::{BurnAsk, TurnsCtx};

fn run<Fut: Future<Output = ()>>(f: impl FnOnce(Rc<AssembledRuntime>, Rc<Ctx>) -> Fut) {
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a tokio runtime");
    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        let backend = Arc::new(FakeBackend::new(FakeBackendOptions::default()));
        let opts = RuntimeOptions { backend: Some(backend), store: Some(Rc::new(memory_store())), ..Default::default() };
        let runtime = create_runtime(opts).expect("a runtime");
        let ctx = runtime.context();
        f(runtime, ctx).await
    });
}

fn record(kind: &str) -> WorkspaceRecord {
    serde_json::from_value(json!({
        "id": "w_1", "name": "api", "machineId": "m_1", "phase": "running", "golden": "g", "createdAt": "2026-10-26T00:00:00.000Z",
        "project": "p_1", "spec": {}, "size": { "cpu": 2, "memMb": 4096 }, "firstLife": false, "kind": kind,
    }))
    .expect("a workspace record")
}

/// A live entry over a machine of the runtime's own backend.
async fn live(rt: &AssembledRuntime, c: &Rc<Ctx>, kind: &str) -> Entry {
    let spec = serde_json::from_value(json!({ "kind": "sandbox" })).expect("a machine spec");
    let machine = rt.backend().create(spec, None, None).await.expect("a machine");
    let mut held = record(kind);
    held.machine_id = machine.id().to_string();
    let entry = BootRuntime::attach(&**c, &Rc::new(RefCell::new(held)), machine).expect("an entry");
    BootRuntime::live(&**c).borrow_mut().insert("w_1".into(), entry.clone());
    entry
}

/// The refusal of records.ts:88 for a record whose project this host does not hold.
#[test]
fn base_view_of_a_record_whose_project_is_not_held_is_refused() {
    run(|_, c| async move {
        let base: &dyn Base = &*c;
        let said = base.view(&record("cloud")).expect_err("a project this host does not hold").message;
        assert_eq!(said, "no project \"p_1\"; wsp add <folder> records one");
    });
}

/// A kind with no restart answers the words of workspaces.ts:634, the area's own refusal, never a sentence of the context's.
#[test]
fn restart_daemon_of_a_kind_with_no_restart_says_the_words_workspaces_ts_says() {
    run(|rt, c| async move {
        let entry = live(&rt, &c, "cloud").await;
        let phases: &dyn PhasesCtx = &*c;
        let said = phases.restart_daemon_of(&entry).await.expect_err("refused");
        assert_eq!(said.to_string(), "api's daemon runs on a machine wsp forks, which this host does not hold the process of");
    });
}

/// A turn's burn call lands in the one burn the usage area shares (usage.ts:46-50, turns.ts:476), not in a warning.
#[test]
fn turns_burn_add_files_into_the_burn_the_usage_area_shares() {
    run(|_, c| async move {
        let ask = |tokens: f64, replayed: Option<(String, f64)>| BurnAsk {
            account: "claude@here".into(),
            thread_id: "t_1".into(),
            tokens,
            replayed,
        };
        TurnsCtx::burn_add(&*c, ask(1_200.0, None));
        TurnsCtx::burn_add(&*c, ask(300.0, Some(("turn-1".into(), 5_000.0))));
        let parts = c.usage_parts();
        let calls: Vec<(String, f64, bool)> =
            parts.burn.calls.borrow().iter().map(|(_, call)| (call.account.clone(), call.tokens, call.replayed.is_some())).collect();
        assert_eq!(calls, vec![("claude@here".to_string(), 1_200.0, false), ("claude@here".to_string(), 300.0, true)]);
    });
}

/// The public marketplaces the branch-of-record area read are in the env of a brokered turn (runtime.ts:653), where they
/// were left out until the area had a slot.
#[test]
fn channels_brokered_env_names_the_public_marketplaces_the_area_read() {
    run(|rt, c| async move {
        let entry = live(&rt, &c, "cloud").await;
        let before = ChannelsCtx::brokered_env(&*c, "tok-1", &entry);
        assert!(!before.values().any(|v| v.contains("acme/lab.git")));
        *c.branch_area().marketplaces.borrow_mut() = Some(wsp_machines::projects::branch_of_record::MarketplacesRead {
            at: 0,
            repos: Rc::new(RefCell::new(vec!["acme/lab".to_string()])),
            read: futures::future::ready(()).boxed_local().shared(),
            first: false,
        });
        let after = ChannelsCtx::brokered_env(&*c, "tok-1", &entry);
        assert!(after.values().any(|v| v.contains("acme/lab.git")), "the marketplace repo reaches git's config: {after:?}");
        assert_eq!(after, wsp_machines::machines::brokered::brokered_env("tok-1", None, &["acme/lab".to_string()]));
    });
}

/// `status.spend` rejects when a row's view throws (records.ts:88), and the spawn that asked is refused with it.
#[test]
fn channels_spend_today_is_the_refusal_of_a_row_that_cannot_be_viewed() {
    run(|rt, c| async move {
        live(&rt, &c, "cloud").await;
        let refused = ChannelsCtx::spend_today(&*c, &[], 1_000).await.expect_err("a refusal");
        assert!(refused.message.starts_with("no project \"p_1\""), "{}", refused.message);
    });
}

struct Steers;

impl HarnessAdapter for Steers {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("this adapter starts nothing".into()))
    }
    fn steers(&self) -> bool {
        true
    }
}

struct Plain;

impl HarnessAdapter for Plain {
    fn start(&self, _options: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        Err(AdapterError("this adapter starts nothing".into()))
    }
    fn steers(&self) -> bool {
        false
    }
}

/// The adapter the caller names is the one the catalog is read over, since the trait hands the area its own `Arc`.
#[test]
fn launch_a_catalog_is_read_over_the_adapter_it_is_handed() {
    run(|rt, c| async move {
        let entry = live(&rt, &c, "cloud").await;
        let launch: &dyn LaunchCtx = &*c;
        let plain = launch.catalog_on("claude", &entry, Arc::new(Plain)).await.expect("no refusal").expect("a catalog");
        assert_eq!(plain.harness, "claude");
        assert!(!plain.steers);
        let steering = launch.catalog_on("claude", &entry, Arc::new(Steers)).await.expect("no refusal").expect("a catalog");
        assert!(steering.steers, "the catalog follows the adapter handed in");
    });
}
