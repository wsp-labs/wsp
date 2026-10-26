// SPDX-License-Identifier: AGPL-3.0-only
//! The public runtime's own decisions, the sweep and the close: spec/runtime_of.md. `PublicCtx`, `StatusCtx` and
//! `HarnessesCtx` are faked over the script; each test asserts the calls in order and the answer.
mod support;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::{json, Value};
use support::{block, Script};
use wsp_core::types::api::CatalogAt;
use wsp_core::types::internal::BuilderLife;
use wsp_core::types::wiring::AdoptedMachine;
use wsp_engine::orphans::{ListedMachine, ReapFailure, ReapReason, ReapResult, ReapedMachine};
use wsp_serve::runtime::public::{
    close, harnesses_list, reap, status_history, status_list, CloseStep, HarnessesCtx, MachineRead, PublicCtx, StatusCtx, SweepAnswer,
    SweptBuilder, SweptEntry, KEPT_CLOSE_WAIT_MS,
};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::workspace::{Caller, WorkspacePhase};
use wsp_wire::views::workspace_more::{WorkspaceOrigin, WorkspaceStatus};
use wsp_words::exit::HostError;

const SIX_HOURS: i64 = 6 * 60 * 60 * 1000;

fn reaped(id: &str, reason: ReapReason) -> ReapedMachine {
    ReapedMachine { id: id.into(), labels: None, builder: false, reason, age_ms: None, name: None }
}

/// The sweep's world, scripted: builders, entries, places and their listings, and every call recorded.
struct Sweep {
    script: Rc<Script>,
    now: i64,
    builders: Vec<SweptBuilder>,
    entries: Vec<SweptEntry>,
    places: Vec<(String, bool)>,
    stop_fails: Vec<String>,
}

impl Sweep {
    fn new() -> Sweep {
        Sweep {
            script: Script::new(),
            now: 1_000_000_000,
            builders: vec![],
            entries: vec![],
            places: vec![("default".into(), true)],
            stop_fails: vec![],
        }
    }
}

#[async_trait(?Send)]
impl PublicCtx for Sweep {
    async fn ready(&self) -> Result<(), HostError> {
        self.script.record("ready", json!({}));
        Ok(())
    }
    fn set_say_stops(&self, _say: Rc<dyn Fn(&str)>) {
        self.script.record("setSayStops", json!({}));
    }
    fn wall_now(&self) -> i64 {
        self.now
    }
    fn clock_now(&self) -> i64 {
        self.now
    }
    async fn refresh_builders(&self) -> Result<(), HostError> {
        self.script.record("refreshBuilders", json!({}));
        Ok(())
    }
    async fn expire_grace(&self) -> Result<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError> {
        self.script.call("expireGrace", json!({}))
    }
    fn builders(&self) -> Vec<SweptBuilder> {
        self.builders.clone()
    }
    async fn hold(&self, builder_id: &str) -> Result<(), HostError> {
        self.script.record("hold", json!({ "id": builder_id }));
        Ok(())
    }
    async fn stop_builder(&self, builder_id: &str, place: &str) -> Result<(), HostError> {
        self.script.record("stopBuilder", json!({ "id": builder_id, "place": place }));
        if self.stop_fails.iter().any(|f| f == builder_id) {
            return Err(HostError::new("provider said 500"));
        }
        Ok(())
    }
    async fn forget_builder(&self, builder_id: &str) -> Result<(), HostError> {
        self.script.record("forgetBuilder", json!({ "id": builder_id }));
        Ok(())
    }
    async fn replay_creates(&self, _failed: &mut Vec<ReapFailure>) -> Result<Vec<ReapedMachine>, HostError> {
        let call = self.script.record("replayCreates", json!({}));
        self.script.answer(&call).unwrap_or(Ok(vec![]))
    }
    fn sweep_places(&self) -> Vec<(String, bool)> {
        self.places.clone()
    }
    fn wired_place(&self) -> String {
        "default".into()
    }
    fn place_name(&self, place: &str) -> String {
        format!("{place}-name")
    }
    async fn list_at(&self, place: &str) -> Result<Vec<ListedMachine>, HostError> {
        let call = self.script.record("listAt", json!({ "place": place }));
        self.script.answer(&call).unwrap_or(Ok(vec![]))
    }
    fn entries(&self) -> Vec<SweptEntry> {
        self.entries.clone()
    }
    async fn reread_held(&self, workspace_id: &str, why: &str) -> Result<(), HostError> {
        self.script.record("rereadHeld", json!({ "id": workspace_id, "why": why }));
        Ok(())
    }
    async fn recover_gone(&self, workspace_id: &str) -> Result<(), HostError> {
        self.script.record("recoverGone", json!({ "id": workspace_id }));
        Ok(())
    }
    async fn read_machine(&self, workspace_id: &str) -> MachineRead {
        let call = self.script.record("readMachine", json!({ "id": workspace_id }));
        self.script.answer(&call).unwrap_or(MachineRead::Failed)
    }
    async fn adopt_gone(&self, workspace_id: &str, words: String) -> Result<(), HostError> {
        self.script.record("adoptGone", json!({ "id": workspace_id, "words": words }));
        Ok(())
    }
    fn known_ids(&self) -> Vec<String> {
        self.script.record("knownIds", json!({}));
        vec!["m-known".into()]
    }
    async fn adopt_lost(
        &self,
        place: &str,
        _rows: &[ListedMachine],
        _claimed: &mut Vec<String>,
        _failed: &mut Vec<ReapFailure>,
    ) -> Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError> {
        let call = self.script.record("adoptLost", json!({ "place": place }));
        self.script.answer(&call).unwrap_or(Ok((vec![], vec![])))
    }
    async fn engine_reap(
        &self,
        place: &str,
        _rows: Vec<ListedMachine>,
        known: Vec<String>,
        older_than_ms: Option<i64>,
    ) -> Result<ReapResult, HostError> {
        let call = self.script.record("engineReap", json!({ "place": place, "known": known, "olderThanMs": older_than_ms }));
        self.script.answer(&call).unwrap_or(Ok(ReapResult { reaped: vec![], spared: vec![], failed: None }))
    }
    async fn sweep_strays(
        &self,
        known: Vec<String>,
        _failed: &mut Vec<ReapFailure>,
        places: &[String],
    ) -> Result<Vec<ReapedMachine>, HostError> {
        let call = self.script.record("sweepStrays", json!({ "known": known, "places": places }));
        self.script.answer(&call).unwrap_or(Ok(vec![]))
    }
    async fn close_step(&self, step: CloseStep) -> Result<(), HostError> {
        self.script.record("close", json!(format!("{step:?}")));
        Ok(())
    }
    fn kept_agents(&self) -> Vec<String> {
        let call = self.script.record("keptAgents", json!({}));
        self.script.answer(&call).unwrap_or_default()
    }
    fn reap_kept(&self, thread_id: &str) -> futures::future::LocalBoxFuture<'static, ()> {
        self.script.record("reapKept", json!({ "threadId": thread_id }));
        let hangs = !self.script.calls_to("keptHangs").is_empty();
        Box::pin(async move {
            if hangs {
                futures::future::pending::<()>().await;
            }
        })
    }
}

fn builder(id: &str, life: BuilderLife, created_label: Option<String>) -> SweptBuilder {
    SweptBuilder {
        id: id.into(),
        life,
        created_label,
        created_at: "not a date".into(),
        building: false,
        place: None,
        heartbeat_held: false,
    }
}

fn entry(id: &str, machine: &str, phase: WorkspacePhase) -> SweptEntry {
    SweptEntry {
        id: id.into(),
        machine_id: machine.into(),
        entry_machine_id: machine.into(),
        creating: false,
        napping: false,
        waking: false,
        deleting: false,
        absent_machine: false,
        phase,
    }
}

fn iso(ms: i64) -> String {
    jiff::Timestamp::from_millisecond(ms).unwrap().to_string()
}

#[test]
fn reap_order_grace_builders_replays_listings() {
    // U9, U11, U12, U14. runtime.ts:1105-1211
    block(async {
        let mut world = Sweep::new();
        world.builders = vec![builder("b-own", BuilderLife::Own, Some(iso(1_000_000_000 - 1_000)))];
        world.places = vec![("default".into(), true), ("hetzner".into(), true), ("away".into(), false)];
        world.script.once::<Result<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>>(
            "expireGrace",
            Ok((vec![reaped("m-grace", ReapReason::Grace)], vec![])),
        );
        world.script.once::<Result<Vec<ReapedMachine>, HostError>>("replayCreates", Ok(vec![reaped("m-replay", ReapReason::Lost)]));
        let said = Rc::new(|_: &str| {});
        let answer = reap(&world, Some(5_000), Some(said)).await.unwrap();
        let names: Vec<String> = world.script.names().into_iter().filter(|n| n != "knownIds").collect();
        assert_eq!(
            names,
            vec![
                "setSayStops",
                "ready",
                "refreshBuilders",
                "expireGrace",
                "hold",
                "replayCreates",
                "listAt",
                "listAt",
                "adoptLost",
                "adoptLost",
                "engineReap",
                "engineReap",
                "sweepStrays"
            ]
        );
        assert_eq!(
            world.script.calls_to("listAt").iter().map(|c| c.args["place"].clone()).collect::<Vec<_>>(),
            vec![json!("default"), json!("hetzner")],
            "a place with no backend is not listed"
        );
        assert_eq!(world.script.calls_to("engineReap")[0].args["olderThanMs"], json!(5_000));
        assert_eq!(answer.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["m-grace", "m-replay"]);
        assert_eq!(answer.failed, None, "failed only when not empty");
        assert_eq!(answer.adopted, None, "adopted only when not empty");
    })
}

#[test]
fn reap_a_grace_that_fails_is_named_and_reaps_nothing_for_it() {
    // U9. runtime.ts:1115-1118
    block(async {
        let world = Sweep::new();
        world.script.once::<Result<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>>("expireGrace", Err(HostError::new("store locked")));
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(answer.failed, Some(vec![ReapFailure { id: None, message: "grace sweep: store locked".into() }]));
        assert!(answer.reaped.is_empty());
    })
}

#[test]
fn reap_a_stale_builder_reasons() {
    // U10. runtime-builders.test.ts; runtime.ts:1127-1139
    block(async {
        let mut world = Sweep::new();
        world.script.on("expireGrace", |_| Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![], vec![])));
        let mut unfinished = builder("b-unfinished", BuilderLife::Stale, None);
        unfinished.building = true;
        world.builders = vec![unfinished, builder("b-recorded", BuilderLife::Stale, None)];
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(
            answer.reaped,
            vec![
                ReapedMachine {
                    id: "b-unfinished".into(),
                    labels: None,
                    builder: true,
                    reason: ReapReason::Unfinished,
                    age_ms: None,
                    name: None
                },
                ReapedMachine {
                    id: "b-recorded".into(),
                    labels: None,
                    builder: true,
                    reason: ReapReason::Recorded,
                    age_ms: None,
                    name: None
                },
            ]
        );
        assert_eq!(
            world.script.calls_to("stopBuilder").iter().map(|c| c.args.clone()).collect::<Vec<_>>(),
            vec![json!({ "id": "b-unfinished", "place": "default" }), json!({ "id": "b-recorded", "place": "default" })]
        );
        assert_eq!(world.script.calls_to("forgetBuilder").len(), 2);
        assert!(world.script.calls_to("hold").is_empty(), "only an own builder gets its heartbeat");
    })
}

#[test]
fn reap_a_reusable_builder_expires_at_six_hours() {
    // U10. runtime.ts:1124-1126 (BUILDER_IDLE_MS, at least)
    block(async {
        let mut world = Sweep::new();
        world.script.on("expireGrace", |_| Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![], vec![])));
        let now = world.now;
        world.builders = vec![
            builder("b-young", BuilderLife::Reusable, Some(iso(now - SIX_HOURS + 1))),
            builder("b-six", BuilderLife::Reusable, Some(iso(now - SIX_HOURS))),
            builder("b-ageless", BuilderLife::Reusable, None),
        ];
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(
            answer.reaped,
            vec![
                ReapedMachine {
                    id: "b-six".into(),
                    labels: None,
                    builder: true,
                    reason: ReapReason::Expired,
                    age_ms: Some(SIX_HOURS),
                    name: None
                },
                ReapedMachine {
                    id: "b-ageless".into(),
                    labels: None,
                    builder: true,
                    reason: ReapReason::Expired,
                    age_ms: None,
                    name: None
                },
            ]
        );
        assert!(world.script.calls_to("stopBuilder").iter().all(|c| c.args["id"] != json!("b-young")));
    })
}

#[test]
fn reap_a_builder_stop_that_fails_stays_recorded() {
    // U10. runtime.ts:1128-1133
    block(async {
        let mut world = Sweep::new();
        world.script.on("expireGrace", |_| Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![], vec![])));
        let mut b = builder("b-stuck", BuilderLife::Stale, None);
        b.place = Some("hetzner".into());
        world.builders = vec![b];
        world.stop_fails = vec!["b-stuck".into()];
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(
            world.script.calls_to("stopBuilder")[0].args,
            json!({ "id": "b-stuck", "place": "hetzner" }),
            "a builder is stopped at its own place"
        );
        assert!(world.script.calls_to("forgetBuilder").is_empty());
        assert!(answer.reaped.is_empty());
        assert_eq!(
            answer.failed,
            Some(vec![ReapFailure {
                id: Some("b-stuck".into()),
                message: "could not stop: provider said 500; stays recorded, retried next sweep".into()
            }])
        );
    })
}

#[test]
fn reap_the_wired_listing_failing_answers_at_once() {
    // U12. runtime.ts:1161-1164
    block(async {
        let mut world = Sweep::new();
        world.script.on("expireGrace", |_| {
            Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![reaped("m-grace", ReapReason::Grace)], vec![]))
        });
        world.places = vec![("default".into(), true), ("hetzner".into(), true)];
        world.script.once::<Result<Vec<ListedMachine>, HostError>>("listAt", Err(HostError::new("401 from the provider")));
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(
            answer.reaped.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
            vec!["m-grace"],
            "what was reaped before the listing stays"
        );
        assert!(answer.spared.is_empty());
        assert_eq!(answer.failed, Some(vec![ReapFailure { id: None, message: "401 from the provider".into() }]));
        assert_eq!(world.script.calls_to("listAt").len(), 1, "no other place is listed");
        assert!(world.script.calls_to("engineReap").is_empty());
    })
}

#[test]
fn reap_another_listing_failing_goes_on() {
    // U12. runtime.ts:1165
    block(async {
        let mut world = Sweep::new();
        world.script.on("expireGrace", |_| Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![], vec![])));
        world.places = vec![("default".into(), true), ("hetzner".into(), true)];
        world.script.once::<Result<Vec<ListedMachine>, HostError>>("listAt", Ok(vec![]));
        world.script.once::<Result<Vec<ListedMachine>, HostError>>("listAt", Err(HostError::new("timed out")));
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(answer.failed, Some(vec![ReapFailure { id: None, message: "hetzner-name: timed out".into() }]));
        assert_eq!(world.script.calls_to("engineReap").iter().map(|c| c.args["place"].clone()).collect::<Vec<_>>(), vec![json!("default")]);
    })
}

#[test]
fn reap_reads_each_record_by_its_state() {
    // U13. runtime.ts:1171-1191, gone.test.ts, reconcile.test.ts
    block(async {
        let mut world = Sweep::new();
        world.script.on("expireGrace", |_| Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![], vec![])));
        let mut busy = entry("ws-busy", "m-busy", WorkspacePhase::Running);
        busy.waking = true;
        let mut absent = entry("ws-absent", "", WorkspacePhase::Running);
        absent.absent_machine = true;
        world.entries = vec![
            busy,
            absent,
            entry("ws-listed-gone", "m-listed", WorkspacePhase::Gone),
            entry("ws-gone", "m-gone", WorkspacePhase::Gone),
            entry("ws-missing", "m-missing", WorkspacePhase::Running),
            entry("ws-failed", "m-failed", WorkspacePhase::Running),
        ];
        let listed: ListedMachine =
            serde_json::from_value(json!({ "id": "m-listed", "state": "running", "labels": {} })).expect("a listing row");
        world.script.once::<Result<Vec<ListedMachine>, HostError>>("listAt", Ok(vec![listed]));
        world.script.once::<MachineRead>("readMachine", MachineRead::Missing { answer: Some("404 not found".into()) });
        world.script.once::<MachineRead>("readMachine", MachineRead::Failed);
        let _ = reap(&world, None, None).await.unwrap();
        assert_eq!(world.script.calls_to("rereadHeld")[0].args, json!({ "id": "ws-absent", "why": "sweep" }));
        assert_eq!(world.script.calls_to("recoverGone")[0].args, json!({ "id": "ws-listed-gone" }));
        assert_eq!(
            world.script.calls_to("readMachine").iter().map(|c| c.args["id"].clone()).collect::<Vec<_>>(),
            vec![json!("ws-missing"), json!("ws-failed")]
        );
        let adopted = world.script.calls_to("adoptGone");
        assert_eq!(adopted.len(), 1);
        assert_eq!(adopted[0].args["id"], json!("ws-missing"));
        let seen = wsp_words::words::thread::GoneSighting {
            by: wsp_words::words::thread::GoneSeenBy::Sweep,
            at: world.now as f64,
            answer: Some("404 not found".into()),
        };
        assert_eq!(adopted[0].args["words"], json!(wsp_words::words::thread::gone_words("m-missing", Some(&seen))));
    })
}

#[test]
fn reap_answer_omits_empty_failed_and_adopted() {
    // U15. runtime.ts:1142-1145
    let answer = SweepAnswer { reaped: vec![], spared: vec![], failed: None, adopted: None };
    let result: wsp_core::types::wiring::SweepResult = answer.into();
    assert!(result.adopted.is_none());
    assert!(result.reaped.failed.is_none());
}

#[test]
fn reap_a_failure_in_the_engine_stretch_keeps_the_adopted() {
    // U14. runtime.ts:1212-1215
    block(async {
        let world = Sweep::new();
        world.script.on("expireGrace", |_| Ok::<(Vec<ReapedMachine>, Vec<ReapFailure>), HostError>((vec![], vec![])));
        let adopted =
            AdoptedMachine { id: "m-lost".into(), workspace_id: "ws-lost".into(), name: "lost".into(), phase: WorkspacePhase::Running };
        world.script.once::<Result<(Vec<AdoptedMachine>, Vec<ReapedMachine>), HostError>>("adoptLost", Ok((vec![adopted.clone()], vec![])));
        world.script.once::<Result<ReapResult, HostError>>("engineReap", Err(HostError::new("engine broke")));
        let answer = reap(&world, None, None).await.unwrap();
        assert_eq!(answer.adopted, Some(vec![adopted]));
        assert_eq!(answer.failed, Some(vec![ReapFailure { id: None, message: "engine broke".into() }]));
        assert!(answer.spared.is_empty());
    })
}

#[test]
fn close_runs_its_steps_in_order() {
    // U16. runtime.ts:1217-1275
    block(async {
        let world = Sweep::new();
        world.script.once::<Vec<String>>("keptAgents", vec!["t-kept".into()]);
        close(&world).await.unwrap();
        let steps: Vec<Value> = world
            .script
            .calls()
            .into_iter()
            .filter(|c| c.name == "close" || c.name == "reapKept")
            .map(|c| if c.name == "close" { c.args } else { json!("reapKept") })
            .collect();
        let expected: Vec<Value> = [
            "MarkClosing",
            "ClearRootsRecheck",
            "RunHeldAtClose",
            "AwaitCopiesMoving",
            "StopSweep",
            "AwaitSweeping",
            "StopDiskWatch",
            "CloseIdle",
            "CloseAlerts",
            "CloseSlates",
            "CloseAgentsReader",
            "SettleDaemonSyncs",
            "SettleKeptWrites",
            "SettleAgentAsks",
        ]
        .iter()
        .map(|s| json!(s))
        .chain(std::iter::once(json!("reapKept")))
        .chain(
            [
                "StopMachineReadings",
                "ClosePipes",
                "CloseLocal",
                "ClosePlaceDoor",
                "MarkClosed",
                "CancelBeat",
                "CancelTimers",
                "StopPrPolls",
                "CloseGoneWatch",
                "AwaitTicking",
                "FreeOwnBuilders",
                "FlushTranscripts",
                "AwaitTranscriptWrites",
            ]
            .iter()
            .map(|s| json!(s)),
        )
        .collect();
        assert_eq!(steps, expected);
    })
}

#[test]
fn close_waits_at_most_the_kept_wait_for_a_kept_agent() {
    // U16. runtime.ts:1242-1247
    assert_eq!(KEPT_CLOSE_WAIT_MS, 2_000);
    block(async {
        let world = Sweep::new();
        world.script.once::<Vec<String>>("keptAgents", vec!["t-slow".into()]);
        world.script.record("keptHangs", json!({}));
        let started = std::time::Instant::now();
        close(&world).await.unwrap();
        let took = started.elapsed();
        assert!(took >= std::time::Duration::from_millis(1_900) && took < std::time::Duration::from_millis(3_500), "{took:?}");
        assert_eq!(
            world.script.calls_to("close").last().unwrap().args,
            json!("AwaitTranscriptWrites"),
            "the close goes on past a kept agent that never exits"
        );
    })
}

#[test]
fn close_with_no_kept_agent_waits_for_none() {
    // U16. runtime.ts:1247 (the race only when there are some)
    block(async {
        let world = Sweep::new();
        let started = std::time::Instant::now();
        close(&world).await.unwrap();
        assert!(started.elapsed() < std::time::Duration::from_millis(500));
        assert!(world.script.calls_to("reapKept").is_empty());
    })
}

#[test]
fn close_leaves_machine_turns_running() {
    // U17. runtime.ts:1248-1252: the close stops readings, never ends a turn
    block(async {
        let world = Sweep::new();
        close(&world).await.unwrap();
        let names = world.script.names();
        assert!(names.iter().all(|n| !n.contains("endSessions") && !n.contains("end")), "{names:?}");
        assert!(world.script.calls_to("close").iter().any(|c| c.args == json!("StopMachineReadings")));
    })
}

// ---- status ----

struct Status {
    script: Rc<Script>,
    rows: Vec<WorkspaceStatus>,
    listed: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl StatusCtx for Status {
    async fn probe_list(&self, _o: Option<wsp_core::status::StatusListOptions>) -> Result<Vec<WorkspaceStatus>, HostError> {
        self.script.record("probeList", json!({}));
        // A record dropped while the probes ran: it leaves the caller's list too.
        self.listed.borrow_mut().retain(|id| id != "ws-dropped");
        Ok(self.rows.clone())
    }
    fn listed_for(&self, origin: Option<&Caller>) -> Vec<String> {
        self.script.record("listedFor", json!({ "origin": support::script::caller_json(&origin.cloned()) }));
        self.listed.borrow().clone()
    }
    fn refuse_named(&self, workspace_id: &str, _origin: Option<&Caller>) -> Result<(), HostError> {
        self.script.record("refuseNamed", json!({ "id": workspace_id }));
        if workspace_id == "ws-not-yours" {
            return Err(HostError::new("not yours"));
        }
        Ok(())
    }
    async fn history(&self, workspace_id: &str) -> Result<Vec<Value>, HostError> {
        self.script.record("history", json!({ "id": workspace_id }));
        Ok(vec![json!({ "t": 1 })])
    }
}

fn status_row(id: &str) -> WorkspaceStatus {
    serde_json::from_value(json!({
        "id": id, "name": id, "machineId": format!("m-{id}"), "phase": "running", "golden": "g", "createdAt": "2026-10-01T00:00:00.000Z",
        "project": { "id": "p-1", "name": "app", "path": "/src/app", "computer": "here" },
        "machineState": "running", "reach": { "state": "reachable" }, "size": { "cpu": 2, "memMb": 4096 }, "rateUsdPerHour": 0
    }))
    .unwrap_or_else(|e| panic!("build a WorkspaceStatus for {id}: {e}"))
}

#[test]
fn status_list_filters_after_the_probes() {
    // U5. runtime.ts:1066-1070
    block(async {
        let ctx = Status {
            script: Script::new(),
            rows: vec![status_row("ws-b"), status_row("ws-a"), status_row("ws-hidden")],
            listed: RefCell::new(vec!["ws-a".into(), "ws-b".into()]),
        };
        let rows = status_list(&ctx, None, Some(Caller::Road(WorkspaceOrigin::Relayed))).await.unwrap();
        assert_eq!(rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["ws-b", "ws-a"], "the probes' order, the caller's rows");
        assert_eq!(ctx.script.names(), vec!["probeList", "listedFor"]);
    })
}

#[test]
fn a_record_dropped_mid_probe_leaves_the_list() {
    // U5. runtime.ts:1063-1065
    block(async {
        let ctx = Status {
            script: Script::new(),
            rows: vec![status_row("ws-dropped"), status_row("ws-a")],
            listed: RefCell::new(vec!["ws-a".into(), "ws-dropped".into()]),
        };
        let rows = status_list(&ctx, None, None).await.unwrap();
        assert_eq!(rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>(), vec!["ws-a"]);
    })
}

#[test]
fn status_history_refuses_off_the_named_rule() {
    // U6. runtime.ts:1073-1076
    block(async {
        let ctx = Status { script: Script::new(), rows: vec![], listed: RefCell::new(vec![]) };
        assert_eq!(status_history(&ctx, "ws-gone-from-host", None).await.unwrap(), vec![json!({ "t": 1 })]);
        let refused = status_history(&ctx, "ws-not-yours", None).await.err().unwrap();
        assert_eq!(refused.message, "not yours");
        assert_eq!(ctx.script.names(), vec!["refuseNamed", "history", "refuseNamed"], "a refused id reads no history");
    })
}

// ---- harnesses ----

struct Harnesses {
    script: Rc<Script>,
    prefs: Value,
    lists: Vec<HarnessCatalog>,
    talks_to_tree: bool,
}

#[async_trait(?Send)]
impl HarnessesCtx for Harnesses {
    async fn preferences(&self) -> Result<wsp_wire::views::preferences::Preferences, HostError> {
        self.script.record("preferences", json!({}));
        let mut full = serde_json::to_value(wsp_wire::views::preferences::default_preferences()).expect("the defaults write");
        for (key, value) in self.prefs.as_object().expect("the picks are an object") {
            full[key] = value.clone();
        }
        Ok(serde_json::from_value(full).expect("preferences parse"))
    }
    fn talks_to_tree_on(&self, workspace_id: &str, _origin: Option<&Caller>) -> bool {
        self.script.record("talksToTreeOn", json!({ "id": workspace_id }));
        self.talks_to_tree
    }
    async fn entry_of(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Option<(String, String)>, HostError> {
        self.script.record("entryOf", json!({ "id": workspace_id, "origin": support::script::caller_json(&origin) }));
        Ok(Some((workspace_id.into(), "p-1".into())))
    }
    async fn project_home(&self, project: &str, _origin: Option<Caller>) -> Result<Option<(String, String)>, HostError> {
        self.script.record("projectHome", json!({ "project": project }));
        Ok(Some(("ws-home".into(), project.into())))
    }
    async fn computer_home(&self, computer: &str, _origin: Option<Caller>) -> Result<Option<(String, String)>, HostError> {
        self.script.record("computerHome", json!({ "computer": computer }));
        Ok(None)
    }
    async fn agent_lists(&self, entry: Option<&(String, String)>) -> Result<Vec<HarnessCatalog>, HostError> {
        self.script.record("agentLists", json!({ "entry": entry.map(|e| e.0.clone()) }));
        Ok(self.lists.clone())
    }
    fn default_agent_of(&self, _prefs: &wsp_wire::views::preferences::Preferences, _entry: Option<&(String, String)>) -> String {
        "codex".into()
    }
    fn shaped(
        &self,
        catalog: &HarnessCatalog,
        _prefs: &wsp_wire::views::preferences::Preferences,
        project: Option<&str>,
    ) -> serde_json::Map<String, Value> {
        self.script.record("shaped", json!({ "harness": catalog.harness, "project": project }));
        json!({ "harness": catalog.harness, "shaped": true }).as_object().unwrap().clone()
    }
}

fn catalog(harness: &str) -> HarnessCatalog {
    serde_json::from_value(json!({
        "harness": harness, "label": harness, "source": "table", "version": null, "models": [{ "value": "m", "label": "M" }], "efforts": [],
        "contextWindows": [], "permissionModes": [], "steers": false, "renames": false, "images": false
    }))
    .unwrap_or_else(|e| panic!("build a HarnessCatalog: {e}"))
}

#[test]
fn harnesses_list_marks_the_default_and_unshaped_only_where_set() {
    // U7. runtime.ts:1092-1096, thread-defaults.test.ts
    block(async {
        let prefs = json!({ "agentDefaults": { "claude": { "models": [] } }, "projectDefaults": {}, "projectLook": {} });
        let ctx = Harnesses { script: Script::new(), prefs, lists: vec![catalog("claude"), catalog("codex")], talks_to_tree: false };
        let rows = harnesses_list(&ctx, None, None, None).await.unwrap();
        let claude = serde_json::to_value(&rows[0]).unwrap();
        let codex = serde_json::to_value(&rows[1]).unwrap();
        assert_eq!(claude.to_string(), json!({ "harness": "claude", "shaped": true, "unshaped": { "models": [{ "value": "m", "label": "M" }], "efforts": [], "permissionModes": [] } }).to_string());
        assert_eq!(codex.to_string(), json!({ "harness": "codex", "shaped": true, "isDefault": true }).to_string());
        assert!(ctx.script.calls_to("entryOf").is_empty(), "no workspace, project or computer reads no entry");
    })
}

#[test]
fn harnesses_list_reads_a_project_or_computer_home() {
    // U7. runtime.ts:1081-1088
    block(async {
        let prefs = json!({ "agentDefaults": {}, "projectDefaults": {}, "projectLook": {} });
        let ctx = Harnesses { script: Script::new(), prefs: prefs.clone(), lists: vec![], talks_to_tree: true };
        harnesses_list(&ctx, Some("ws-1".into()), Some(Caller::Road(WorkspaceOrigin::Relayed)), None).await.unwrap();
        assert_eq!(
            ctx.script.calls_to("entryOf")[0].args,
            json!({ "id": "ws-1", "origin": null }),
            "a caller that talks to the tree reads the entry with no origin"
        );
        let ctx = Harnesses { script: Script::new(), prefs: prefs.clone(), lists: vec![], talks_to_tree: false };
        harnesses_list(&ctx, Some("ws-1".into()), Some(Caller::Road(WorkspaceOrigin::Relayed)), None).await.unwrap();
        assert_eq!(ctx.script.calls_to("entryOf")[0].args, json!({ "id": "ws-1", "origin": "relayed" }));
        let ctx = Harnesses { script: Script::new(), prefs: prefs.clone(), lists: vec![], talks_to_tree: false };
        harnesses_list(&ctx, None, None, Some(CatalogAt { project: Some("p-9".into()), computer: Some("c-1".into()) })).await.unwrap();
        assert_eq!(
            ctx.script.names().iter().filter(|n| n.ends_with("Home")).cloned().collect::<Vec<_>>(),
            vec!["projectHome"],
            "a project wins over a computer"
        );
        let ctx = Harnesses { script: Script::new(), prefs, lists: vec![], talks_to_tree: false };
        harnesses_list(&ctx, None, None, Some(CatalogAt { project: None, computer: Some("c-1".into()) })).await.unwrap();
        assert_eq!(ctx.script.calls_to("agentLists")[0].args, json!({ "entry": null }), "a computer with no home reads the lists for none");
    })
}
