// SPDX-License-Identifier: AGPL-3.0-only
//! Review of wsp-machines at 2c4f6cb46: panics that outside input or ordinary concurrent use reach.
#![allow(clippy::await_holding_refcell_ref)]

mod machines_b_support;
mod rest_support;

use std::cell::RefCell;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;

use futures::future::FutureExt;
use serde_json::json;

use machines_b_support::spawn_ctx::*;
use machines_b_support::*;
use wsp_core::clock::Clock;
use wsp_machines::machines::sub_spawn::SpawnAsk;
use wsp_machines::usage::accounts::{Burn, BurnCall, PriceTable, ReplayedStamp};
use wsp_store::store::Store;
use wsp_words::exit::HostError;

fn said_of(panic: Box<dyn std::any::Any + Send>) -> String {
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

/// The future's answer, or the message of the panic that took it.
async fn unwound<F: Future>(fut: F) -> Result<F::Output, String> {
    AssertUnwindSafe(fut).catch_unwind().await.map_err(said_of)
}

fn asked() -> SpawnAsk {
    SpawnAsk {
        name: NAME.to_string(),
        project: "pr_1".to_string(),
        on: "boat".to_string(),
        ..SpawnAsk::default()
    }
}

/// sub-spawn.ts:163 reads `live.get(scope.workspaceId)!` after `computerRows` and `landingPlace` have been awaited; a
/// lead dropped meanwhile (its thread deleted, its machine gone) makes `lead.record` throw a TypeError there, which
/// `spawnSub` answers as a refusal the lead's agent reads. checks.rs `spawn_sub_checked` calls `.expect("a scope's
/// workspace is a live thread")` on the same read, so the same moment panics the request. The fake host's context
/// answers both awaits at once, so the drop is made before the call, which stands for it landing during them.
#[test]
fn a_spawn_whose_lead_was_dropped_during_its_checks_is_refused_not_a_panic() {
    run(async {
        let f = lead_on_this_computer();
        f.host.live.borrow_mut().shift_remove(LEAD_WS);
        let by = lead_caller();
        match unwound(f.area.spawn_sub(asked(), Some(&by))).await {
            Ok(answer) => assert!(
                answer.is_err(),
                "a lead that is not live cannot have spawned a machine"
            ),
            Err(panic) => panic!("the spawn panicked where the TS rejects: {panic}"),
        }
    });
}

/// sub-spawn.ts:433 reads `live.get(record.id)!` once `ctx.fork` answers; the fork's `bind` attached the entry, and the
/// awaits that follow it in `MachinesArea::fork` (hostname, machine context, shape, memory) let a drop of the record
/// remove it. The TS then throws on `entry.machine` and the start fails and drops its machine (`dropMachine`); machine.rs
/// `stand_up` calls `.expect("a forked sub-thread stands in the live set")`, so the panic unwinds out of
/// `stand_up_queued` before `drop_machine`, `forking_remove` and the failure's record are reached, and the box the
/// fork made is left standing. `posted` is what `bind` calls (`landed()`), so removing the entry there is a drop landing
/// right after `ctx.attach`.
#[test]
fn a_forked_sub_thread_dropped_before_its_stand_up_reads_it_fails_the_start_not_a_panic() {
    run(async {
        let f = lead_on_this_computer();
        let record = queued_sub(&f, fresh_spawn(1_760_000_000_000.0));
        let host = f.host.clone();
        let drop_it = Rc::new(move || {
            host.live.borrow_mut().shift_remove(SUB_WS);
        });
        match unwound(f.area.stand_up_queued(&record, drop_it, false)).await {
            Ok(started) => assert!(
                started.is_err(),
                "an entry that is gone cannot have been stood up"
            ),
            Err(panic) => {
                let boxes: Vec<(String, bool)> = f
                    .host
                    .backend
                    .machines
                    .lock()
                    .unwrap()
                    .iter()
                    .map(|m| (m.id.clone(), m.killed()))
                    .collect();
                panic!(
                    "the start panicked where the TS fails it; boxes made (id, killed) {boxes:?}, forking_add {}, forking_remove {}, \
                     phase of the record {}: {panic}",
                    f.host.calls_of("forking_add").len(),
                    f.host.calls_of("forking_remove").len(),
                    serde_json::to_value(record.borrow().phase).unwrap()
                )
            }
        }
    });
}

fn blob_of(value: serde_json::Value) -> Vec<u8> {
    value.to_string().into_bytes()
}

/// usage.ts:506, 515: `Date.now() - fetchedAt < PRICES_TTL_MS` on a kept stamp. accounts.rs `PriceTable::fresh` takes
/// the stamp as `at as i64` (a float beyond i64 saturates) and subtracts it from the clock in i64: a kept copy whose
/// stamp is -1e300 gives `now - i64::MIN`, which overflows and panics in a debug build (a release build wraps it
/// negative, which reads as fresh for ever). The TS reads it as a copy older than a day and asks again.
#[test]
fn a_kept_price_copy_with_a_far_stamp_reads_as_old_not_as_an_overflow() {
    const NOON: i64 = 1_790_683_200_000;
    rest_support::run(async {
        let clock = rest_support::TestClock::at(NOON);
        let store = rest_support::MemStore::new();
        store.seed_blob(
            "prices",
            "litellm",
            &blob_of(json!({ "fetchedAt": -1e300, "table": { "m": { "input": 1, "output": 2 } } })),
        );
        let fetched = Rc::new(RefCell::new(0u32));
        let counted = fetched.clone();
        let fetch: Rc<
            dyn Fn() -> futures::future::LocalBoxFuture<
                'static,
                Result<serde_json::Value, HostError>,
            >,
        > = Rc::new(move || {
            *counted.borrow_mut() += 1;
            async { Err(HostError::new("offline")) }.boxed_local()
        });
        let store_dyn: Rc<dyn Store> = store.clone();
        let table = PriceTable::new(
            store_dyn,
            clock.clone() as Rc<dyn Clock>,
            fetch,
            Rc::new(|_line: &str| {}),
        );
        match unwound(table.get()).await {
            Ok(rates) => {
                assert_eq!(
                    *fetched.borrow(),
                    1,
                    "a copy with that stamp is old, so the table asks again"
                );
                assert!(
                    rates.contains_key("m"),
                    "and the old copy is what is used while it cannot"
                );
            }
            Err(panic) => panic!("the price table panicked on a stored stamp: {panic}"),
        }
    });
}

/// usage.ts:476-479: a replayed call is filed at `min(received, topReceived - (topAt - replayed.at))`. accounts.rs
/// `filed_at` does that sum in i64, and a run whose stamps are `i64::MAX` and `i64::MIN` (a log line's stamp beyond the
/// range, which `as i64` saturates to) makes `top_at - replayed.at` overflow in `Burn::recent`, which every `add` and
/// `of` runs. The same class as the stored price stamp: a debug-build panic, a wrap in a release build.
#[test]
fn a_replayed_call_with_a_far_stamp_is_filed_not_an_overflow() {
    const NOON: i64 = 1_790_683_200_000;
    let clock = rest_support::TestClock::at(NOON);
    let burn = Burn::new(clock.clone() as Rc<dyn Clock>);
    let call = |at: i64| BurnCall {
        account: "claude:vault-token".to_string(),
        thread_id: "t1".to_string(),
        tokens: 1_500.0,
        replayed: Some(ReplayedStamp {
            run: "r1".to_string(),
            at,
        }),
    };
    let went = std::panic::catch_unwind(AssertUnwindSafe(|| {
        burn.add(call(i64::MAX));
        burn.add(call(i64::MIN));
        burn.add(call(0));
        burn.of("claude:vault-token")
    }));
    if let Err(panic) = went {
        panic!(
            "the burn panicked filing replayed calls: {}",
            said_of(panic)
        );
    }
}

/// create.ts:81 stores the person's `idleWindowMs` as given (the wire reads `z.number()`, no bound), and machines.ts:517 and
/// :790-796 hand it to `backstopMs(windowMs) = windowMs * 2` in floats, so a window of 5e18 ms reaches the provider as
/// `idleTimeoutMs: 1e19`, which the provider answers. create.rs `fork_spec` casts it with `as i64` (saturating) and
/// `wsp_core::idle::backstop_ms` multiplies in i64: 5e18 * 2 overflows and panics in a debug build before the create has
/// asked the cloud for anything, and wraps to a negative timeout in a release build.
#[test]
fn a_huge_idle_window_is_sent_as_it_is_not_an_overflow() {
    use wsp_machines::machines::machines::create::ForkSpecIn;
    common::run_local(async {
        let w = machines_support::World::new(machines_support::Opts::default());
        let record = machines_support::record_of("running", json!({ "idleWindowMs": 5e18 }), false);
        let went = std::panic::catch_unwind(AssertUnwindSafe(|| {
            w.area.fork_spec(
                &record,
                ForkSpecIn {
                    kind: wsp_wire::views::golden_image::MachineKind::Sandbox,
                    image: None,
                    over: None,
                    npm_bin: None,
                },
            )
        }));
        match went {
            Ok(spec) => {
                let sent = spec.idle_timeout_ms.map(|ms| ms.0).unwrap_or_default();
                assert!(sent > 0.0, "the backstop sent to the cloud is {sent}");
            }
            Err(panic) => panic!(
                "fork_spec panicked on idleWindowMs 5e18: {}",
                said_of(panic)
            ),
        }
    });
}

/// sub-queue.ts:305-322: `pumpQueue` joins the pump in flight (`if (pumping) { again = true; return pumping; }`), and the
/// pump's own `finally` clears `pumping` however a start ends, a throw included, so the next call runs a fresh pump.
/// sub_queue.rs `pump_queue` keeps the pump as a `Shared` in `self.pumping` and clears it only after `pump_loop().await`
/// returns, so a panic anywhere under it (`stand_up_queued`'s `.expect`s at machine.rs:262 and :363, `show_row`'s) poisons
/// that `Shared`, leaves it in `pumping`, and every later `pump_queue` hands back the dead one: no held start starts again
/// until the host restarts (and the delete of the start that was in flight waits on its `gone` for ever, since the panic
/// skipped `fire0(&h.left)`). The queue fake's `stand_up_queued` panics here (its scripted "gate" with no gate to wait on
/// is an `expect`) where the real one panics at those `expect`s.
#[test]
fn a_panic_in_one_held_start_leaves_the_queue_able_to_pump_again() {
    use serde_json::Value;
    use wsp_core::types::wiring::WorkspaceRecord;
    use wsp_machines::machines::sub_queue::{HoldAsk, SubQueueArea};
    run(async {
        let host = FakeHost::new();
        host.machines_at.borrow_mut().insert("box".to_string(), 1.0);
        host.put_live(
            sub_record("ws_first", "running"),
            FakeMachine::new("bx_first"),
        );
        let area = SubQueueArea::new(host.clone());
        let mut rec = sub_record("ws_b", "running");
        rec["name"] = json!("Second");
        rec["machineId"] = json!("");
        let record: WorkspaceRecord = serde_json::from_value(rec).unwrap();
        let cell = Rc::new(RefCell::new(record));
        let limits = host.backend.limits.lock().unwrap().clone();
        let wait = area.queue_wait("box", limits.as_ref());
        area.enqueue(&cell, wait, false).await.unwrap();
        let mut o = serde_json::Map::new();
        o.insert("prompt".into(), json!("Fix the flaky upload test."));
        o.insert("harness".into(), json!("claude"));
        let ask = HoldAsk {
            workspace_id: "ws_b".to_string(),
            thread_id: "t_b".to_string(),
            tree: indexmap::IndexMap::new(),
            o,
            on_held: None,
            notify: None,
        };
        let _turn = tokio::task::spawn_local(
            area.hold_queued(ask, Some(&caller("t_lead", "ws_lead", "t_lead"))),
        );
        settle().await;
        host.script("stand_up_queued", Ok(Value::String("gate".into())));
        // The first sub-thread naps: its slot is free and ws_b, the head of the queue, is started, which panics.
        let entry = host.live.borrow().get("ws_first").cloned().unwrap();
        let record = entry.borrow().record.clone();
        record.borrow_mut().phase = serde_json::from_value(json!("napping")).unwrap();
        host.emit_now(json!({ "type": "workspace.napped", "workspaceId": "ws_first" }));
        settle().await;
        assert_eq!(
            host.calls_of("stand_up_queued").len(),
            1,
            "the head was started and its start panicked"
        );
        // Anything that moves the queue afterwards (a timer, a bus event, an enqueue) calls `pump_queue` and joins it.
        let pumped = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            unwound(area.pump_queue()),
        )
        .await;
        match pumped {
            Ok(Ok(())) => {}
            Ok(Err(panic)) => panic!(
                "pump_queue hands back the pump that panicked, so the queue is stuck: {panic}"
            ),
            Err(_) => panic!("pump_queue never answered"),
        }
    });
}

/// `MachinesArea::on_event` (machines.rs:287) is the bus listener the wiring calls for every event, and for a running
/// workspace that reads paused it calls `tokio::task::spawn_local`, which panics outside a `LocalSet`. The engine crate
/// keeps its own `try_spawn_local` for this (wsp-engine/src/local_task.rs); no caller of this crate's listeners was
/// found outside the host's task set, so this is the contract made visible, not a reached fault.
#[test]
fn the_status_listener_called_outside_a_local_set_does_not_panic() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let local = tokio::task::LocalSet::new();
    let (area, event) = local.block_on(&rt, async {
        let w = machines_support::World::new(machines_support::Opts::default());
        let _held = w.entry_in(machines_support::EntryOpts::default());
        let event = w.status_event("ws_1", "running", "paused");
        (w.area.clone(), event)
    });
    let went = std::panic::catch_unwind(AssertUnwindSafe(|| area.on_event(&event)));
    if let Err(panic) = went {
        panic!("the listener panicked: {}", said_of(panic));
    }
}

mod common;
#[path = "support/machines.rs"]
mod machines_support;
