use std::rc::Rc;

use std::sync::Arc;

use futures::future::LocalBoxFuture;

use serde_json::{json, Value};

use support::agents_ctx::{entry, plain_factory, table, Ability, FakeAdapter, FakeAgentsCtx, Gate};

use support::{json_of, local, row, sessions_of, settle, silent, view, FakeMachine};

use wsp_adapters::port::{
    AttachmentRoad, ExecFailure, ExecFailureKind, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe, SessionRenameWrite,
};

use wsp_core::console;

use wsp_threads::threads::agents::lists::{
    known_catalog, source_of, title_rows, CarriedTitle, AGENT_LISTS, AGENT_VERSION_READ_MS, CATALOG_PROBE_TIMEOUT_MS, CATALOG_TTL_MS,
    FIRST_RUN_READ_MS, SESSION_TITLE_REFRESH_MAX, SESSION_TITLE_TIMEOUT_MS, SESSION_TITLE_TTL_MS, TITLE_MAKE_TIMEOUT_MS,
};

use wsp_wire::views::harness::{HarnessCatalog, ScreenCommand, ScreenControl};

use wsp_wire::views::session::TitleSource;

const OPENING: &str = "make a server, and its tests";

const LOST_HEAD: &str = "Your earlier session for this thread was lost, so it continues here in a new one.";

// ---- fixtures ----------------------------------------------------------------------------------------------------

fn claude_table() -> HarnessCatalog {
    table(
        "claude",
        "Claude Code",
        json!({
            "version": "--help 2.1.257, 2026-10-10",
            "models": [
                { "value": "claude-opus-5-5", "label": "Opus 5.5", "isDefault": true },
                { "value": "claude-sonnet-5-5", "label": "Sonnet 5.5" },
            ],
            "efforts": [{ "value": "low", "label": "Low" }, { "value": "high", "label": "High", "isDefault": true }],
            "permissionModes": [
                { "value": "default", "label": "Default" },
                { "value": "bypassPermissions", "label": "Bypass", "isDefault": true },
            ],
            "smallModel": "claude-haiku-5-5",
        }),
    )
}

fn codex_table() -> HarnessCatalog {
    table(
        "codex",
        "Codex",
        json!({
            "version": "app-server 0.153.0, 2026-09-07",
            "models": [
                { "value": "gpt-5.6-sol", "label": "GPT-5.6-Sol", "isDefault": true },
                { "value": "gpt-5.6-terra", "label": "GPT-5.6-Terra" },
            ],
            "efforts": [{ "value": "low", "label": "Low", "isDefault": true }, { "value": "high", "label": "High" }],
            "permissionModes": [{ "value": "read-only", "label": "Read only", "description": "Reads only; edits no files and runs no command that writes" }],
        }),
    )
}

fn model(slug: &str, label: &str, is_default: bool, efforts: &[&str]) -> HarnessCatalogModelProbe {
    HarnessCatalogModelProbe {
        slug: slug.to_string(),
        label: label.to_string(),
        description: None,
        efforts: Some(efforts.iter().map(|e| e.to_string()).collect()),
        default_effort: None,
        context_windows: vec![],
        is_default,
        fast: None,
    }
}

fn probe(version: &str) -> HarnessCatalogProbe {
    HarnessCatalogProbe {
        version: Some(version.to_string()),
        models: vec![model("claude-opus-5-5", "Opus", true, &["low", "high"]), model("claude-haiku-4-5", "Haiku 4.5", false, &[])],
        efforts: vec!["low".into(), "high".into()],
        permission_modes: vec!["default".into(), "bypassPermissions".into()],
    }
}

fn answered(version: &str) -> Ability<Option<HarnessCatalogAnswer>> {
    Ability::answering(Some(HarnessCatalogAnswer::Probe(probe(version))))
}

fn values(c: &Value, list: &str) -> Vec<String> {
    c[list].as_array().expect("a list").iter().map(|m| m["value"].as_str().expect("a value").to_string()).collect()
}

fn other(message: &str) -> ExecFailure {
    ExecFailure { kind: ExecFailureKind::Other, message: message.to_string() }
}

/// The machine's commands that contain `needle`, each with the timeout it was run under.
fn runs(machine: &FakeMachine, needle: &str) -> Vec<Option<u64>> {
    machine.execs.borrow().iter().filter(|(c, _)| c.contains(needle)).map(|(_, t)| *t).collect()
}

/// A machine whose `claude` command is a wrapper whose first run installs the agent.
fn installing(machine: &FakeMachine) {
    machine.rules.borrow_mut().clear();
    machine.answer("head -c 4096", 0, "\u{1e}#!/bin/sh\n# ascii-harness-shim\nexec /usr/local/lib/ascii-harnesses/claude \"$@\"\n\u{1e}");
    machine.answer("ascii-lazy-harness", 0, "1\n");
}

/// A live running workspace in the context, answering for `harness` with this adapter.
fn running(ctx: &Rc<FakeAgentsCtx>, harness: &str, adapter: &Arc<FakeAdapter>) -> (wsp_threads::threads::threads::Entry, Arc<FakeMachine>) {
    let (e, machine) = entry(json!({}));
    ctx.add_entry(e.clone());
    ctx.add_adapter(harness, plain_factory(adapter.clone()));
    (e, machine)
}

fn seed(ctx: &Rc<FakeAgentsCtx>, rows: Vec<Value>) {
    *ctx.sessions.borrow_mut() =
        sessions_of(rows.into_iter().enumerate().map(|(i, v)| row(view(v), &format!("u{i}"))).collect()).borrow().clone();
}

fn row_json(ctx: &Rc<FakeAgentsCtx>, id: &str) -> Value {
    json_of(&ctx.sessions.borrow().get(id).expect("a row").borrow().view)
}

/// The calls that keep, push or write a title, in order.
fn effects(ctx: &Rc<FakeAgentsCtx>) -> Vec<String> {
    ctx.called().into_iter().filter(|c| ["persist", "pushHead", "writeSession"].iter().any(|p| c.starts_with(p))).collect()
}

async fn ready_within<T>(mut f: LocalBoxFuture<'static, T>) -> Option<T> {
    for _ in 0..50 {
        if let std::task::Poll::Ready(v) = futures::poll!(&mut f) {
            return Some(v);
        }
        tokio::task::yield_now().await;
    }
    None
}

// ---- L5: reading a row's title off the harness's store ---------------------------------------------------------------

fn titled(title: &str) -> Ability<Option<String>> {
    Ability::answering(Some(title.to_string())).execing("wsp-title-read S1")
}

fn thread_rows() -> Vec<Value> {
    vec![
        json!({ "id": "r1", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 10 }),
        json!({ "id": "r2", "threadId": "t1", "claudeSessionId": "S1", "prompt": OPENING, "startedAt": 20 }),
        json!({ "id": "r3", "threadId": "t2", "claudeSessionId": "S9", "prompt": "another thread", "startedAt": 30 }),
    ]
}

// ---- L8: the title the harness makes ---------------------------------------------------------------------------------

fn maker(title: &str) -> Ability<Option<String>> {
    Ability::answering(Some(title.to_string()))
}
