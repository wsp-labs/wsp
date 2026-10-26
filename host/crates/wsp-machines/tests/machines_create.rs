// SPDX-License-Identifier: AGPL-3.0-only
//! The cases of machines_create.md that no oracle scene replays: the fork spec, `hold`, `claiming`, the exec log
//! listener, `fork` and `unfork`. Each expected value is read off machines.ts (the line is named) at 65f6b89c7. The
//! scenes are in machines_create_scenes.rs. Spec: spec/machines_create.md.

mod common;
#[path = "support/machines.rs"]
mod support;

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::Arc;

use futures::future::FutureExt;
use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{from, BackendOpts, MachineDef, Made, Opts, World};
use wsp_core::types::harness::login_env_on;
use wsp_core::types::wiring::{GoldenExec, WorkspaceRecord, WorkspaceSpec};
use wsp_engine::machine::{
    ExecOptions, Machine, MachineBackend, MachineKind, MachineSpec, MachineSpecOnIdle, RunOptions,
};
use wsp_machines::machines::area_error::AreaError;
use wsp_machines::machines::machines::create::ForkSpecIn;
use wsp_machines::machines::machines::ImageRead;
use wsp_wire::types::js_number::JsNumber;

fn pairs(labels: &Option<IndexMap<String, String>>) -> Vec<(String, String)> {
    labels
        .as_ref()
        .map(|l| l.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default()
}

fn strs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn env(items: &[(&str, &str)]) -> IndexMap<String, String> {
    items
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

fn record_with(extras: Value) -> WorkspaceRecord {
    support::record_of("running", extras, false)
}

fn fork_in<'a>(
    image: Option<&'a MachineSpec>,
    over: Option<&'a WorkspaceSpec>,
    npm_bin: Option<&'a str>,
) -> ForkSpecIn<'a> {
    ForkSpecIn {
        kind: MachineKind::Sandbox,
        image,
        over,
        npm_bin,
    }
}

/// machines.ts:23-39: the image's own keys, then the record's, then the override's, in the TS spread order.
#[test]
fn fork_spec_keys_and_precedence_match_the_ts() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let image: MachineSpec = from(
            json!({ "kind": "sandbox", "template": "img-1", "diskGb": 10, "binds": [{ "source": "/img", "target": "/img" }] }),
        );
        let over = WorkspaceSpec {
            cpu: Some(JsNumber(8.0)),
            mem_mb: None,
            envs: Some(env(&[("B", "over"), ("C", "over")])),
            ..Default::default()
        };
        let record = record_with(json!({
            "golden": "snap_g",
            "spec": {
                "envs": { "A": "rec", "B": "rec" },
                "labels": { "custom": "x" },
                "binds": [{ "source": "/a", "target": "/b" }],
                "copy": { "from": "/f", "at": "/t" }
            },
            "shape": { "cpu": 4, "memMb": 8192 },
            "size": { "cpu": 2, "memMb": 4096 }
        }));
        let spec = w
            .area
            .fork_spec(&record, fork_in(Some(&image), Some(&over), None));
        let v = support::norm(&serde_json::to_value(&spec).unwrap());

        assert_eq!(v["template"], "img-1", "the image's own keys stay");
        assert_eq!(v["diskGb"], 10);
        assert_eq!(v["kind"], "sandbox");
        assert_eq!(
            v["binds"],
            json!([{ "source": "/a", "target": "/b" }]),
            "the record's binds replace the image's"
        );
        assert_eq!(v["copy"], json!({ "from": "/f", "at": "/t" }));
        assert_eq!(v["cpu"], 8, "override.cpu first");
        assert_eq!(v["memMb"], 8192, "then the shape's memory, not the row's");
        assert_eq!(v["onIdle"], "pause");

        let mut want_env = login_env_on(None, None);
        for (k, x) in [("A", "rec"), ("B", "rec"), ("B", "over"), ("C", "over")] {
            want_env.insert(k.to_string(), x.to_string());
        }
        assert_eq!(
            spec.envs.clone().unwrap().into_iter().collect::<Vec<_>>(),
            want_env.into_iter().collect::<Vec<_>>(),
            "the login env, then the record's envs, then the override's, a repeated key staying where it first stood"
        );
        assert_eq!(
            pairs(&spec.labels),
            strs(&[
                ("custom", "x"),
                ("wsp", "1"),
                ("wsp-owner", "h_0a993e4c"),
                ("wsp-workspace", "ws_1"),
                ("wsp-name", "w1"),
                ("wsp-golden", "snap_g"),
                ("createdAt", "2026-09-21T14:13:20.000Z"),
            ])
        );

        // No override: the shape's size; no shape either: the row's size.
        let no_over = w.area.fork_spec(&record, fork_in(Some(&image), None, None));
        let v = support::norm(&serde_json::to_value(&no_over).unwrap());
        assert_eq!(
            (v["cpu"].clone(), v["memMb"].clone()),
            (json!(4), json!(8192))
        );
        let mut bare = record.clone();
        bare.shape = None;
        let from_size = w.area.fork_spec(&bare, fork_in(None, None, None));
        let v = support::norm(&serde_json::to_value(&from_size).unwrap());
        assert_eq!(
            (v["cpu"].clone(), v["memMb"].clone()),
            (json!(2), json!(4096))
        );
        assert!(v.get("template").is_none(), "no image, no image keys");

        // A record with no binds, or an empty list of them, leaves the image's binds; no copy leaves none.
        let mut unbound = record.clone();
        unbound.spec.binds = Some(Vec::new());
        unbound.spec.copy = None;
        let kept = w
            .area
            .fork_spec(&unbound, fork_in(Some(&image), None, None));
        let v = support::norm(&serde_json::to_value(&kept).unwrap());
        assert_eq!(v["binds"], json!([{ "source": "/img", "target": "/img" }]));
        assert!(v.get("copy").is_none());
    })
}

/// machines.ts:37 with idle.ts:14-18: `backstopMs(null)` is six hours, else twice the window.
#[test]
fn fork_spec_backstop_is_twice_the_window_or_six_hours_off() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let mut record = record_with(json!({}));
        let cases: [(Option<Option<JsNumber>>, i64); 3] = [
            (Some(Some(JsNumber(300_000.0))), 600_000),
            (Some(None), 6 * 60 * 60_000),
            (None, 2_400_000),
        ];
        for (window, want) in cases {
            record.idle_window_ms = window;
            let spec = w.area.fork_spec(&record, fork_in(None, None, None));
            assert_eq!(
                spec.on_idle,
                Some(MachineSpecOnIdle::Pause),
                "window {window:?}"
            );
            assert_eq!(
                support::norm(&serde_json::to_value(&spec).unwrap())["idleTimeoutMs"],
                json!(want),
                "window {window:?}"
            );
        }
    })
}

/// machines.ts:45-58: a purpose is held by one at a time; every waiter wakes at a release and re-checks, so one takes
/// the hold and the rest wait again; the answer releases it.
#[test]
fn hold_waits_for_the_holder_and_rechecks_after_waking() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let release = w.area.hold("workspace/ws_1").await;
        let (a, b) = (w.area.clone(), w.area.clone());
        let second = support::started(async move { a.hold("workspace/ws_1").await }).await;
        let third = support::started(async move { b.hold("workspace/ws_1").await }).await;
        common::flush().await;
        assert!(
            !second.is_finished() && !third.is_finished(),
            "both wait while the purpose is held"
        );

        let other = w
            .area
            .hold("workspace/ws_2")
            .now_or_never()
            .expect("another purpose is free");
        other();

        release();
        common::flush().await;
        assert_eq!(
            [second.is_finished(), third.is_finished()]
                .iter()
                .filter(|d| **d)
                .count(),
            1,
            "one takes the hold at the release, the other re-checks and waits again"
        );

        let (winner, loser) = if second.is_finished() {
            (second, third)
        } else {
            (third, second)
        };
        let release_winner = winner.await.expect("the winner ran");
        common::flush().await;
        assert!(
            !loser.is_finished(),
            "the loser waits while the winner holds"
        );
        release_winner();
        common::flush().await;
        assert!(loser.is_finished(), "and wakes at the winner's release");
        (loser.await.expect("the loser ran"))();
        assert!(
            w.area.hold("workspace/ws_1").now_or_never().is_some(),
            "every release leaves the purpose free"
        );
    })
}

/// machines.ts:185-231: the handle's create holds the purpose and claims every box a waiting line names and the
/// machine, each once; whatever it claimed leaves `inflight` when `run` settles, either way.
#[test]
fn claiming_holds_every_claimed_id_until_run_settles() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let blocked = Rc::new(RefCell::new(Vec::<bool>::new()));

        let area = w.area.clone();
        let flags = blocked.clone();
        let inside = {
            let (area_in, spec) = (w.area.clone(), support::spec_default());
            let b = w.provider(BackendOpts::default());
            w.area
                .claiming(
                    "workspace/ws_1",
                    move |h| {
                        async move {
                            let waiting: wsp_engine::machine::Waiting = Arc::new(move |_line: &str, _box: Option<&wsp_engine::machine::ProviderBoxState>| {
                                flags.borrow_mut().push(area.hold("workspace/ws_1").now_or_never().is_none());
                            });
                            h.create(spec.clone(), Some(waiting))
                        .await
                        .map_err(AreaError::into_host)?;
                            let once = area_in.inflight();
                            h.create(spec, None).await.map_err(AreaError::into_host)?;
                            Ok::<_, wsp_words::exit::HostError>((once, area_in.inflight()))
                        }
                        .boxed_local()
                    },
                    Some(b as Arc<dyn MachineBackend>),
                    None,
                )
                .await
                .expect("the run answers")
        };
        assert_eq!(
            inside.0,
            vec!["box-early".to_string(), "m-made".to_string()],
            "the waiting line's box, then the machine"
        );
        assert_eq!(inside.1, inside.0, "a second create claims each id once");
        assert_eq!(
            *blocked.borrow(),
            vec![true],
            "the purpose is held while the provider is asked"
        );
        assert!(
            w.area.inflight().is_empty(),
            "run settled: nothing is in flight"
        );
        assert!(
            w.area.hold("workspace/ws_1").now_or_never().is_some(),
            "and the hold is released"
        );

        let failing = w.provider(BackendOpts::default());
        let area = w.area.clone();
        let spec = support::spec_default();
        let err = w
            .area
            .claiming(
                "workspace/ws_1",
                move |h| {
                    async move {
                        h.create(spec, None).await.map_err(AreaError::into_host)?;
                        assert_eq!(
                            area.inflight(),
                            vec!["box-early".to_string(), "m-made".to_string()]
                        );
                        Err::<(), _>(wsp_words::exit::HostError::new("run failed"))
                    }
                    .boxed_local()
                },
                Some(failing as Arc<dyn MachineBackend>),
                None,
            )
            .await
            .expect_err("run's failure is the answer");
        assert_eq!(err.message, "run failed");
        assert!(
            w.area.inflight().is_empty(),
            "a failed run releases its claims too"
        );
    })
}

/// machines.ts:187-200: the handle reaches `at`'s own calls, bound to it, and `at` is the runtime's provider when no
/// provider is named.
#[test]
fn claiming_forwards_the_optional_calls_bound() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let at = w.provider(BackendOpts {
            request: true,
            replays: Some(Vec::new()),
            named: Some(Vec::new()),
            ..Default::default()
        });
        at.b.borrow_mut().names_workspace = true;
        let want_caps = at.capabilities();
        let names: BTreeSet<String> = ["wsp-n".to_string()].into_iter().collect();
        let spec = support::spec_default();
        let seen = w
            .area
            .claiming(
                "workspace/ws_1",
                |h| {
                    async move {
                        let b = h.backend();
                        b.get("g1").await.expect("get is at's");
                        b.named_after(&names).await.expect("named_after is at's");
                        let request = b
                            .create_request(&spec)
                            .await
                            .expect("create_request is at's");
                        Ok::<_, wsp_words::exit::HostError>((
                            b.names_workspace(),
                            b.offers().keyed_create,
                            request,
                            b.capabilities(),
                        ))
                    }
                    .boxed_local()
                },
                Some(at.clone() as Arc<dyn MachineBackend>),
                None,
            )
            .await
            .expect("the run answers");
        assert_eq!(
            (seen.0, seen.1, seen.2.as_str()),
            (true, true, support::REQUEST)
        );
        assert_eq!(seen.3, want_caps);
        let log = w.log_now();
        assert_eq!(
            support::log_of(&log, &["backend.get", "namedAfter", "createRequest"]),
            vec![
                json!(["backend.get", "g1"]),
                json!(["namedAfter", ["wsp-n"]]),
                json!(["createRequest", { "a": "1", "createdAt": "stamp" }]),
            ]
        );
        assert!(
            w.backend.b.borrow_mut().gets.is_empty(),
            "the runtime's own provider was not asked"
        );

        w.area
            .claiming(
                "workspace/ws_1",
                |h| {
                    async move {
                        h.backend()
                            .get("g2")
                            .await
                            .map(|_| ())
                            .map_err(|e| wsp_words::exit::HostError::new(e.to_string()))
                    }
                    .boxed_local()
                },
                None,
                None,
            )
            .await
            .expect("the run answers");
        assert_eq!(
            w.backend.b.borrow_mut().gets,
            vec!["g2".to_string()],
            "with no provider named it is the runtime's"
        );
    })
}

/// machines.ts:232-301: each exec and run is reported with its result (a run that says unlogged, only its exit code)
/// or its error; the machine's answer is untouched; a listener that throws is warned once per machine.
#[test]
fn observed_reports_each_exec_and_run_once_and_a_throwing_listener_is_warned_once() {
    common::run_local(async {
        let w = World::new(Opts::default());
        let heard: Rc<RefCell<Vec<GoldenExec>>> = Rc::default();
        let sink = heard.clone();
        *w.rt.on_exec.borrow_mut() = Some(Rc::new(move |e: &GoldenExec| {
            sink.borrow_mut().push(e.clone());
            Ok(())
        }));
        let machine = support::FakeMachine::quiet("m-x");
        {
            let mut s = machine.s.borrow_mut();
            s.exec.push_back(Ok(from(
                json!({ "exitCode": 0, "stdout": "out", "stderr": "err" }),
            )));
            s.exec.push_back(Err(support::plain("exec refused")));
            s.exec.push_back(Ok(from(
                json!({ "exitCode": 3, "stdout": "secret", "stderr": "e" }),
            )));
        }
        let seen = w.area.observed(machine.clone() as Arc<dyn Machine>);
        assert_eq!(seen.id(), "m-x");

        let r1 = seen
            .exec("ls", ExecOptions::default())
            .await
            .expect("answers as the machine did");
        assert_eq!(r1.stdout, "out");
        let r2 = seen
            .exec("bad", ExecOptions::default())
            .await
            .expect_err("the machine's error is rethrown");
        assert_eq!(r2.to_string(), "exec refused");
        let r3 = seen
            .run(
                "cat secret",
                RunOptions {
                    unlogged: true,
                    ..Default::default()
                },
            )
            .await
            .expect("answers as the machine did");
        assert_eq!(r3.stdout, "secret", "the caller still gets the output");

        let ran = |cmd: &str| GoldenExec {
            machine_id: "m-x".into(),
            cmd: cmd.into(),
            ms: JsNumber(0.0),
            ..Default::default()
        };
        assert_eq!(
            *heard.borrow(),
            vec![
                GoldenExec {
                    exit_code: Some(0),
                    stdout: Some("out".into()),
                    stderr: Some("err".into()),
                    ..ran("ls")
                },
                GoldenExec {
                    error: Some("exec refused".into()),
                    ..ran("bad")
                },
                GoldenExec {
                    exit_code: Some(3),
                    ..ran("cat secret")
                },
            ],
            "a run that is unlogged reports its exit and nothing it printed"
        );
        assert!(w.warns().is_empty());

        // No listener: the machine as it is.
        let bare = World::new(Opts::default());
        let plain = support::FakeMachine::quiet("m-y") as Arc<dyn Machine>;
        assert!(Arc::ptr_eq(&bare.area.observed(plain.clone()), &plain));

        // A listener that throws is warned once for that machine, and changes no result.
        let loud = World::new(Opts::default());
        *loud.rt.on_exec.borrow_mut() = Some(Rc::new(|_e: &GoldenExec| Err("boom".to_string())));
        let (m1, m2) = (
            support::FakeMachine::quiet("m-1"),
            support::FakeMachine::quiet("m-2"),
        );
        let (o1, o2) = (
            loud.area.observed(m1 as Arc<dyn Machine>),
            loud.area.observed(m2 as Arc<dyn Machine>),
        );
        for _ in 0..2 {
            assert_eq!(
                o1.exec("a", ExecOptions::default())
                    .await
                    .expect("unchanged")
                    .exit_code
                    .0
                     .0,
                0.0
            );
        }
        o2.exec("b", ExecOptions::default())
            .await
            .expect("unchanged");
        assert_eq!(
            loud.warns(),
            vec![
                "exec log for m-1 failed, its execs go on unlogged: boom".to_string(),
                "exec log for m-2 failed, its execs go on unlogged: boom".to_string(),
            ]
        );
    })
}

/// machines.ts:307-355: `bind` runs once the machine answers and before the claim is released, so no sweep sees it
/// unclaimed; the claim is released when the fork returns.
#[test]
fn fork_binds_before_the_claim_is_released() {
    common::run_local(async {
        let w = World::new(Opts::default());
        w.rt.cfg.borrow_mut().names_workspace = true;
        let record = support::shared(record_with(json!({})));
        let bound: Rc<RefCell<Vec<(String, Vec<String>, Value, String)>>> = Rc::default();
        let (sink, area, store) = (bound.clone(), w.area.clone(), w.store.clone());
        let bind = move |m: &Arc<dyn Machine>, r: &WorkspaceRecord| {
            sink.borrow_mut().push((
                m.id().to_string(),
                area.inflight(),
                store.table("creates"),
                r.id.clone(),
            ));
        };
        let machine = w
            .area
            .fork(&record, &bind, None, None, false)
            .await
            .expect("the fork lands");

        assert_eq!(machine.id(), "m-made");
        assert_eq!(
            *bound.borrow(),
            vec![(
                "m-made".to_string(),
                vec!["box-early".to_string(), "m-made".to_string()],
                json!([]),
                "ws_1".to_string()
            )],
            "bound once, with the machine claimed, the attempt already ended, and the record"
        );
        assert!(
            w.area.inflight().is_empty(),
            "released when the fork returns"
        );
        let made = w.backend.created();
        assert_eq!(made.len(), 1);
        assert_eq!(
            made[0].idempotency_key.as_deref(),
            Some("workspace/ws_1:0101010101010101"),
            "keyed under workspace/<id>"
        );
        assert_eq!(made[0].on_idle, Some(MachineSpecOnIdle::Pause));
    })
}

/// machines.ts:341-345: a place that holds no copy of the image answers missing for a reference it never had, and the
/// fork says where it would land, kind invalid; anywhere else the provider's own refusal stands.
#[test]
fn fork_at_a_place_without_the_image_says_where_it_would_land() {
    common::run_local(async {
        let refusing = || BackendOpts {
            creates: vec![Made::Fail(support::missing("404 not found"))],
            ..Default::default()
        };

        let w = World::new(Opts {
            backend: Some(refusing()),
            ..Default::default()
        });
        {
            let mut cfg = w.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.names_workspace = true;
            cfg.image = ImageRead { version: None };
        }
        let record = support::shared(record_with(json!({ "place": "pl1", "golden": "snap_g" })));
        let err = w
            .area
            .fork(&record, &|_m, _r| {}, None, None, false)
            .await
            .err()
            .expect("the fork is refused");
        let made = w.backend.created();
        let asked = made[0]
            .from_snapshot
            .clone()
            .or(made[0].template.clone())
            .unwrap_or_else(|| "snap_g".into());
        assert_eq!(
            err.message(),
            wsp_wire::wire::machine_link::place_holds_no_image_line("name-of-pl1", &asked)
        );
        assert_eq!(err.kind().as_deref(), Some("invalid"));

        // A fork on this host's own provider (no place) says what the provider said.
        let own = World::new(Opts {
            backend: Some(refusing()),
            ..Default::default()
        });
        {
            let mut cfg = own.rt.cfg.borrow_mut();
            cfg.keeps_images = true;
            cfg.names_workspace = true;
        }
        let here = support::shared(record_with(json!({ "golden": "snap_g" })));
        let err = own
            .area
            .fork(&here, &|_m, _r| {}, None, None, false)
            .await
            .err()
            .expect("the fork is refused");
        assert_eq!(err.message(), "404 not found");

        // A computer that keeps no image has no image to be missing.
        let bare = World::new(Opts {
            backend: Some(refusing()),
            ..Default::default()
        });
        bare.rt.cfg.borrow_mut().names_workspace = true;
        let there = support::shared(record_with(json!({ "place": "pl1", "golden": "snap_g" })));
        let err = bare
            .area
            .fork(&there, &|_m, _r| {}, None, None, false)
            .await
            .err()
            .expect("the fork is refused");
        assert_eq!(err.message(), "404 not found");
    })
}

/// machines.ts:351-367 of `fork`: the npm bin, shape, size and screen are written from what the machine and the image
/// say; a value that is absent deletes its key; memory is read only with `reads_memory` and no place.
#[test]
fn fork_writes_shape_size_and_screen_and_deletes_what_is_absent() {
    common::run_local(async {
        let shaped: wsp_engine::machine::MachineShape = from(json!({ "cpu": 2, "memMb": 4096 }));
        let built: wsp_wire::wire::capabilities::WorkspaceSize =
            from(json!({ "cpu": 2, "memMb": 4096 }));

        let w = World::new(Opts {
            backend: Some(BackendOpts {
                creates: vec![Made::Machine(
                    MachineDef::new("m-1").stream("https://stream"),
                )],
                ..Default::default()
            }),
            ..Default::default()
        });
        {
            let mut cfg = w.rt.cfg.borrow_mut();
            cfg.names_workspace = true;
            cfg.shape = Some(shaped.clone());
            cfg.size_built = Some(built.clone());
        }
        let record = support::shared(record_with(
            json!({ "npmBin": "/old/bin", "screen": { "streamUrl": "https://old" } }),
        ));
        w.area
            .fork(&record, &|_m, _r| {}, None, None, true)
            .await
            .expect("the fork lands");
        assert_eq!(
            record.borrow().shape,
            Some(shaped.clone()),
            "the shape the provider reports"
        );
        assert_eq!(
            record.borrow().size,
            built,
            "the size the shape and the ask came to"
        );
        assert_eq!(
            record
                .borrow()
                .screen
                .as_ref()
                .map(|s| s.stream_url.as_str()),
            Some("https://stream")
        );
        assert_eq!(
            record.borrow().npm_bin,
            None,
            "no image says no npm bin: the old one goes"
        );
        {
            let cfg = w.rt.cfg.borrow();
            assert_eq!(cfg.size_built_calls.len(), 1);
            assert_eq!(cfg.size_built_calls[0].0, Some(shaped.clone()));
            assert_eq!(
                support::norm(&serde_json::to_value(&cfg.size_built_calls[0].1).unwrap()),
                json!({ "cpu": 1, "memMb": 2048 }),
                "asked what the spec asked"
            );
            assert_eq!(
                cfg.read_memory_calls,
                vec![("ws_1".to_string(), "m-1".to_string())],
                "memory is read once the machine is up"
            );
        }

        // The next machine reports no shape and has no stream: both keys go, and nothing is read where the caller said so.
        let bare = World::new(Opts::default());
        bare.rt.cfg.borrow_mut().names_workspace = true;
        let again = support::shared(record_with(
            json!({ "shape": { "cpu": 2, "memMb": 4096 }, "screen": { "streamUrl": "https://old" } }),
        ));
        bare.area
            .fork(&again, &|_m, _r| {}, None, None, false)
            .await
            .expect("the fork lands");
        assert_eq!(again.borrow().shape, None);
        assert_eq!(again.borrow().screen, None);
        assert!(
            bare.rt.cfg.borrow().read_memory_calls.is_empty(),
            "reads_memory off reads nothing"
        );

        // A fork at a place never reads memory.
        let placed = World::new(Opts::default());
        placed.rt.cfg.borrow_mut().names_workspace = true;
        let at_place = support::shared(record_with(json!({ "place": "pl1" })));
        placed
            .area
            .fork(&at_place, &|_m, _r| {}, None, None, true)
            .await
            .expect("the fork lands");
        assert!(placed.rt.cfg.borrow().read_memory_calls.is_empty());
    })
}

/// machines.ts:356-365: the machine is killed until the provider reads it gone, then marked dropped; a mark the store
/// refuses is said and the machine is still gone.
#[test]
fn unfork_marks_the_machine_dropped_after_it_is_gone_and_a_failed_mark_only_warns() {
    common::run_local(async {
        let w = World::new(Opts {
            reads: Some(Vec::new()),
            ..Default::default()
        });
        let e = w.entry_in(support::EntryOpts::default());
        w.area.unfork(&e.entry).await.expect("the machine is gone");

        assert!(
            e.machine.s.borrow_mut().kills >= 1,
            "the machine was asked to die"
        );
        let log = w.log_now();
        let puts = support::log_of(&log, &["store.put"]);
        assert_eq!(
            puts,
            vec![
                json!(["store.put", "dropped-machines", "m-1", { "machineId": "m-1", "at": "2026-09-21T14:13:20.000Z" }])
            ]
        );
        let read_at = log
            .iter()
            .position(|l| l[0] == "readGone")
            .expect("the provider was read");
        let put_at = log.iter().position(|l| l[0] == "store.put").unwrap();
        assert!(read_at < put_at, "marked only once it reads gone");
        assert_eq!(
            w.store.table("dropped-machines"),
            json!([["m-1", { "machineId": "m-1", "at": "2026-09-21T14:13:20.000Z" }]])
        );
        assert!(w.warns().is_empty());

        let refused = World::new(Opts {
            reads: Some(Vec::new()),
            ..Default::default()
        });
        refused.rt.cfg.borrow_mut().fail_put =
            Some(("dropped-machines".into(), "disk full".into()));
        let e = refused.entry_in(support::EntryOpts::default());
        refused
            .area
            .unfork(&e.entry)
            .await
            .expect("a mark that fails does not fail the unfork");
        assert_eq!(
            refused.warns(),
            vec!["machine m-1 is gone but its mark was not stored (disk full); should the provider list it running again, the sweep reports it rather than killing it".to_string()]
        );
    })
}
