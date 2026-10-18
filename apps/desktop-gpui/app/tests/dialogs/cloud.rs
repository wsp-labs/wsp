// SPDX-License-Identifier: AGPL-3.0-only
//! Add a cloud provider's states for `dialogs_shots`, each a flow staged from fixture answers with no host behind it;
//! `dialogs_add_cloud` drives the same fixtures.
#![allow(dead_code)]

use desktop_gpui::screens::dialogs::add_cloud::flow::{Agent, Check, Facts, Flow, Job, Refused, Setup, Step};
use desktop_gpui::screens::dialogs::add_cloud::{self, AddCloud};
use gpui_kit::{App, Entity, Window};
use serde_json::{json, Value};

use super::State;

/// Where the host runs, as the places list names it.
pub const HERE: &str = "Studio";

pub fn setup(held: bool) -> Setup {
    serde_json::from_value(json!({
        "keys": { "box": held },
        "agents": [{ "id": "claude", "name": "Claude Code", "configured": true }, { "id": "codex", "name": "Codex", "configured": false }],
        "job": null
    }))
    .unwrap()
}

/// A flow on Boat's key, its setup read.
pub fn key_flow(held: bool) -> Flow {
    let mut flow = Flow::new(Some("box"));
    flow.on_setup(setup(held));
    flow.here = HERE.into();
    flow
}

fn item(id: &str, label: &str, size: u64, why: &str) -> Value {
    json!({ "id": id, "label": label, "size": size, "why": why, "detail": [] })
}

/// The three pick screens as the host reads them, with the agents ticked.
fn screens() -> Value {
    json!([
        { "id": "agents", "title": "Agents", "top": "", "ticks": ["claude", "codex"], "answers": {}, "footer": [], "items": [
            item("claude", "Claude Code", 218_103_808, "used in 412 threads"),
            item("codex", "Codex", 477_102_080, "used in 38 threads"),
            item("opencode", "OpenCode", 705_691_648, "installed"),
        ]},
        { "id": "tools", "title": "CLIs", "top": "", "ticks": ["gh", "jq"], "answers": {}, "footer": [], "items": [
            item("gh", "GitHub CLI", 42_000_000, "212 calls"),
            item("jq", "jq", 1_600_000, "87 calls"),
            item("ripgrep", "ripgrep", 6_100_000, "40 calls"),
            { "id": "git", "label": "git", "size": 48_000_000, "why": "always", "detail": [], "lock": "on" },
        ]},
        { "id": "also", "title": "Also", "top": "", "ticks": [], "answers": {}, "footer": [], "items": [
            item("ffmpeg", "ffmpeg", 300_000_000, "brew"),
            item("postgresql", "postgresql", 90_000_000, "brew"),
        ]},
        { "id": "wsp", "title": "wsp", "top": "", "ticks": [], "answers": {}, "footer": [], "items": [
            { "id": "wsp-tools/claude", "label": "Claude Code", "detail": [] }
        ]}
    ])
}

fn row(id: &str, label: &str, state: &str, ms: Option<u64>) -> Value {
    let mut row = json!({ "id": id, "kind": "stage", "label": label, "state": state, "lines": ["$ apt-get install -y git", "done"] });
    if let Some(ms) = ms {
        row["ms"] = json!(ms);
    }
    if state == "failed" {
        row["detail"] = json!("apt-get exited with 100");
    }
    row
}

/// The build's stages at a phase, with what each stage stands at.
pub fn job(phase: &str, rows: Value) -> Job {
    let mut job = json!({ "id": "job-1", "road": "manual", "phase": phase, "step": 0, "keys": { "box": true }, "stoppable": true,
        "screens": screens(), "rows": rows, "progress": { "done": 0, "total": 0 }, "log": [], "place": { "id": "box", "name": "box", "kind": "provider" } });
    if phase == "done" {
        job["golden"] = json!({ "version": 3 });
    }
    if phase == "failed" {
        job["error"] = json!("The machine stopped answering while installing the CLIs. Start over to build again.");
    }
    serde_json::from_value(job).unwrap()
}

pub fn build_rows(states: [&str; 4]) -> Value {
    json!([
        row("stage/boot", "Starting the machine", states[0], (states[0] == "done").then_some(14_200)),
        row("stage/agents", "Installing the agents", states[1], (states[1] == "done").then_some(96_000)),
        row("stage/tools", "Installing the CLIs", states[2], (states[2] == "done").then_some(61_000)),
        row("stage/seal", "Sealing the image", states[3], (states[3] == "done").then_some(81_000)),
    ])
}

/// The agents here: Claude Code signed in, Codex not.
pub fn report(codex_ready: bool) -> Vec<Agent> {
    serde_json::from_value(json!([
        { "id": "claude", "name": "Claude Code", "signIn": "signed-in", "signInRoad": "token", "signInKind": "subscription" },
        { "id": "codex", "name": "Codex", "signIn": if codex_ready { "signed-in" } else { "none" }, "signInRoad": "device" },
        { "id": "opencode", "name": "OpenCode", "signIn": "none", "signInRoad": "terminal" },
    ]))
    .unwrap()
}

/// A flow at `step` on a job at `phase`, its picks read.
pub fn job_flow(step: Step, phase: &str, rows: Value) -> Flow {
    let mut flow = key_flow(true);
    let mut job = job(phase, rows);
    if phase == "answering" {
        job.step = 0;
    }
    flow.on_job(job);
    flow.report = Some(report(true));
    flow.github = Some("zingzy".into());
    flow.go(step);
    flow
}

/// A host that keys two clouds, opened by nobody: the provider step.
pub fn two_clouds() -> Flow {
    let mut flow = Flow::new(None);
    flow.on_setup(serde_json::from_value(json!({ "keys": { "box": false, "solari": false } })).unwrap());
    flow.here = HERE.into();
    flow
}

pub fn refused() -> Flow {
    let mut flow = key_flow(false);
    flow.key = "fake-key-1234".into();
    flow.check =
        Check::Refused(Refused { said: "Boat refused this key. Check you copied all of it.".into(), kind: Some("keyRefused".into()) });
    flow
}

pub fn unreachable() -> Flow {
    let mut flow = key_flow(false);
    flow.key = "fake-key-1234".into();
    flow.check = Check::Unreachable(Refused { said: "Boat could not be reached to check the key: connect ETIMEDOUT".into(), kind: None });
    flow
}

pub fn works() -> Flow {
    let mut flow = key_flow(false);
    flow.key = "fake-key-1234".into();
    flow.check = Check::Works(Facts { account: "zingzy".into(), plan: "the $20 plan".into(), running: 1, limit: 8 });
    flow
}

pub fn summary(codex_ready: bool) -> Flow {
    let mut flow = job_flow(Step::Summary, "answering", json!([]));
    flow.report = Some(report(codex_ready));
    flow.github = None;
    flow
}

pub fn stage(flow: Flow, window: &mut Window, cx: &mut App) -> Entity<AddCloud> {
    add_cloud::open_on(None, flow, window, cx)
}

macro_rules! state {
    ($id:literal, $flow:expr) => {
        State::new($id, |window, cx| {
            stage($flow, window, cx);
            super::page(window, cx)
        })
    };
}

pub fn states() -> Vec<State> {
    vec![
        state!("W2-DLG-027", key_flow(false)),
        state!("W2-DLG-106", two_clouds()),
        state!("W2-DLG-028", refused()),
        state!("W2-DLG-028-unreachable", unreachable()),
        state!("W2-DLG-029-works", works()),
        state!("W2-DLG-029-held", key_flow(true)),
        state!("W2-DLG-029-loading", job_flow(Step::Pick(0), "reading", json!([]))),
        state!("W2-DLG-029-agents", job_flow(Step::Pick(0), "answering", json!([]))),
        state!("W2-DLG-029-tools", job_flow(Step::Pick(1), "answering", json!([]))),
        state!("W2-DLG-029-summary", summary(false)),
        state!("W2-DLG-029-build", job_flow(Step::Building, "building", build_rows(["done", "done", "running", "waiting"]))),
        state!("W2-DLG-029-failed", job_flow(Step::Building, "failed", build_rows(["done", "done", "failed", "waiting"]))),
        state!("W2-DLG-029-ready", job_flow(Step::Ready, "done", build_rows(["done", "done", "done", "done"]))),
    ]
}
