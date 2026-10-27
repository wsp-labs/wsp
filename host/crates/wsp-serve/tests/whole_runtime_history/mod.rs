// SPDX-License-Identifier: AGPL-3.0-only
//! The helpers packages/runtime/test/runtime-history.test.ts keeps inside its `describe` (`scripted`, `manual`, `threaded`,
//! `tooled`, `holding`, `countingStore`, `placed`, `onDisk`, `PROBE_OUTPUT`), for the two files that port it. The
//! adapters emit what the TS ones emit, in the same order, at the same moment.
#![allow(dead_code, unused_imports)]
use std::cell::{Cell, RefCell};
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::claude::catalog::{catalog_probe_command, parse_catalog_probe, parse_version, version_probe_command};
use wsp_adapters::claude::landmines::{build_env, ClaudeEnvOptions};
use wsp_adapters::port::{
    AdapterError, ExecFailure, HarnessAdapter, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe, HarnessExec,
    HarnessSession, HarnessStartOptions, OnEvent, SharedFuture, TurnResult,
};
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::SessionHandle;
use wsp_store::sqlite_transcripts::{EventSize, Summed, Total, TranscriptRows};
use wsp_store::store::{BlobMark, Store};
use wsp_wire::views::session_events::SessionEvent;
use wsp_words::exit::HostError;

use crate::common::adapter::{event, result, Started};
use crate::common::scripted::{adapter_of, after_a_turn, Plain, StartFn};
use crate::common::*;

pub const UUID_LEN: usize = 36;

/// `/^[0-9a-f-]{36}$/`.
pub fn is_uuid(text: &str) -> bool {
    text.len() == UUID_LEN && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b) || b == b'-')
}

pub fn opts(prompt: &str) -> StartOptions {
    StartOptions { prompt: prompt.into(), ..Default::default() }
}

/// `first.view().threadId!`.
pub fn thread_of(h: &Rc<dyn SessionHandle>) -> String {
    j(&h.view())["threadId"].as_str().expect("a thread").to_string()
}

/// `for (const e of history) expect(e.x)`: the string at `key`, or nothing where the event has none.
pub fn text_at(e: &Value, key: &str) -> Option<String> {
    e.get(key).and_then(Value::as_str).map(str::to_string)
}

pub fn types_of(events: &[Value]) -> Vec<String> {
    events.iter().map(|e| e["type"].as_str().unwrap_or_default().to_string()).collect()
}

/// `events.reduce(...)` of `threads`: runs of one threadId in wire order.
pub fn thread_runs(events: &[Value]) -> usize {
    events.iter().enumerate().filter(|(i, e)| *i == 0 || e["threadId"] != events[i - 1]["threadId"]).count()
}

/// Every number written as a float, so `27_500` and `27500.0` read as the one number JS has.
pub fn nums(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64()),
        Value::Array(a) => Value::Array(a.iter().map(nums).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, v)| (k.clone(), nums(v))).collect()),
        other => other.clone(),
    }
}

/// `JSON.stringify(e).length` summed, over events that are ASCII.
pub fn json_bytes(events: &[Value]) -> usize {
    events.iter().map(|e| serde_json::to_string(e).expect("an event as text").len()).sum()
}

/// The text of the claude probe's recording, as the TS reads `adapter-claude/test/fixtures/catalog-probe.txt`.
pub fn probe_output() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../wsp-adapters/tests/fixtures/claude/catalog-probe.txt"))
        .expect("the recorded probe")
}

/// `CLAUDE_PIN`.
pub fn claude_pin() -> Value {
    j(&wsp_core::harness_catalog::harness_catalog("claude").expect("a claude row"))["version"].clone()
}

/// `backend.machines.flatMap(m => m.execLog.filter(cmd => cmd.includes("claude --help")))`.
pub fn probes(backend: &Arc<StubBackend>) -> Vec<String> {
    backend.machines().iter().flat_map(|m| m.exec_log().into_iter().filter(|c| c.contains("claude --help"))).collect()
}

/// `backend.machines.flatMap(m => m.execLog.filter(cmd => cmd === "codex --describe"))`.
pub fn codex_probes(backend: &Arc<StubBackend>) -> Vec<String> {
    backend.machines().iter().flat_map(|m| m.exec_log().into_iter().filter(|c| c == "codex --describe")).collect()
}

fn uuid() -> String {
    let n: u128 = rand::random();
    format!(
        "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
        (n >> 96) as u32,
        (n >> 80) as u16,
        (n >> 68) as u16 & 0xfff,
        (n >> 56) as u16 & 0xfff,
        n as u64 & 0xffff_ffff_ffff
    )
}

pub fn sink(o: &HarnessStartOptions) -> OnEvent {
    o.on_event.clone().expect("an event sink")
}

pub fn completed(text: &str) -> Value {
    json!({ "status": "completed", "text": text })
}

pub fn end_of(session: &str) -> Value {
    json!({ "type": "session.end", "sessionId": session, "exitCode": 0, "sawResult": true })
}

pub fn done_of(session: &str, done: &Value) -> Value {
    json!({ "type": "turn.done", "sessionId": session, "result": done })
}

/// `scripted(text)`: every event is said inside `start`, before it returns, under one session id.
pub fn scripted(text: &str) -> HarnessAdapterFactory {
    let text = text.to_string();
    adapter_of(Rc::new(move |o| {
        let id = "22222222-2222-4222-8222-222222222222";
        let on = sink(&o);
        let done = completed(&text);
        for doc in [
            json!({ "type": "session.start", "sessionId": id, "model": "claude-sonnet-4-5" }),
            json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": text }),
            done_of(id, &done),
            end_of(id),
        ] {
            on(event(doc));
        }
        Ok(Plain::of(id, futures::future::ready(result(done)).boxed_local()))
    }))
}

/// `threaded()`'s start: one full turn per start a turn later, under the resume id when given, else a fresh id.
pub fn threaded_start() -> StartFn {
    Rc::new(|o| {
        let id = o.resume.clone().unwrap_or_else(uuid);
        let done = completed(&o.prompt);
        let events = vec![
            json!({ "type": "session.start", "sessionId": id, "model": "claude-sonnet-4-5" }),
            json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": o.prompt }),
            done_of(&id, &done),
            end_of(&id),
        ];
        Ok(Plain::of(&id, after_a_turn(sink(&o), events, done)))
    })
}

pub fn threaded() -> HarnessAdapterFactory {
    adapter_of(threaded_start())
}

/// `tooled(input, output)`: a turn that calls one tool and gets its output back, all said inside `start`.
pub fn tooled(input: &str, output: &str) -> HarnessAdapterFactory {
    let (input, output) = (input.to_string(), output.to_string());
    adapter_of(Rc::new(move |o| {
        let id = "44444444-4444-4444-8444-444444444444";
        let on = sink(&o);
        let done = completed("done");
        for doc in [
            json!({ "type": "session.start", "sessionId": id }),
            json!({ "type": "turn.delta", "sessionId": id, "kind": "tool_use", "text": input, "toolName": "Write", "toolUseId": "tu_1" }),
            json!({ "type": "turn.delta", "sessionId": id, "kind": "tool_result", "text": output, "toolUseId": "tu_1" }),
            done_of(id, &done),
            end_of(id),
        ] {
            on(event(doc));
        }
        Ok(Plain::of(id, futures::future::ready(result(done)).boxed_local()))
    }))
}

/// `holding({ named, hang })`: each turn says what the agent held after two calls, a subagent's between them, then
/// compact; its result names a figure only where `named` says. A turn with `hang` never ends.
pub fn holding(named: Vec<bool>, hang: bool) -> HarnessAdapterFactory {
    let turns = Rc::new(Cell::new(0usize));
    adapter_of(Rc::new(move |o| {
        let id = "44444444-4444-4444-8444-444444444444";
        let n = turns.get();
        turns.set(n + 1);
        let named = named.get(n) == Some(&true);
        let mut done = completed("");
        if named {
            done["tokens"] = json!({ "input": 1, "output": 1, "context": 3_100, "window": 200_000 });
        }
        let on = sink(&o);
        for doc in [
            json!({ "type": "session.start", "sessionId": id }),
            json!({ "type": "turn.delta", "sessionId": id, "kind": "text", "text": "before" }),
            json!({ "type": "turn.usage", "sessionId": id, "tokens": 26_000, "context": 26_000, "window": 200_000 }),
            json!({ "type": "turn.usage", "sessionId": id, "tokens": 900 }),
            json!({ "type": "turn.usage", "sessionId": id, "tokens": 27_500, "context": 27_500, "window": 200_000 }),
            json!({ "type": "turn.compacted", "sessionId": id, "before": 27_500, "after": 3_100 }),
        ] {
            on(event(doc));
        }
        if hang {
            return Ok(Plain::of(id, futures::future::pending().boxed_local()));
        }
        on(event(done_of(id, &done)));
        on(event(end_of(id)));
        Ok(Plain::of(id, futures::future::ready(result(done)).boxed_local()))
    }))
}

/// `heldRows(events)`.
pub fn held_rows(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter_map(|e| match e["type"].as_str() {
            Some("session.context") => Some(json!([e["turnId"], e["context"], e["window"]])),
            Some("session.compacted") => Some(json!(["compacted", e["line"], e["before"], e["after"]])),
            _ => None,
        })
        .collect()
}

type CatalogFn = Rc<dyn Fn(Arc<dyn HarnessExec>) -> LocalBoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>>;
type VersionFn = Rc<dyn Fn(Arc<dyn HarnessExec>) -> LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>>;

/// `{ ...adapter, probeCatalog, probeVersion }`: an adapter literal that opts into the probes, whatever its harness id.
pub struct Probing {
    pub start: StartFn,
    pub catalog: Option<CatalogFn>,
    pub version: Option<VersionFn>,
}

impl HarnessAdapter for Probing {
    fn steers(&self) -> bool {
        false
    }
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        (self.start)(o)
    }
    fn probe_catalog(
        &self,
        exec: Arc<dyn HarnessExec>,
    ) -> Option<LocalBoxFuture<'static, Result<Option<HarnessCatalogAnswer>, ExecFailure>>> {
        self.catalog.as_ref().map(|f| f(exec))
    }
    fn probe_version(&self, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<Option<String>, ExecFailure>>> {
        self.version.as_ref().map(|f| f(exec))
    }
}

/// `codexProbing`: `threaded()` plus a probe that asks `codex --describe` and answers a version line and one model.
pub fn codex_probing() -> HarnessAdapterFactory {
    Arc::new(|_ctx| {
        let catalog: CatalogFn = Rc::new(|exec| {
            async move {
                let out = exec.exec("codex --describe", None).await?;
                Ok(Some(HarnessCatalogAnswer::Probe(HarnessCatalogProbe {
                    version: Some(out.trim().to_string()),
                    models: vec![HarnessCatalogModelProbe {
                        slug: "gpt-5-codex".into(),
                        label: "Codex".into(),
                        description: None,
                        efforts: Some(vec!["high".into()]),
                        default_effort: None,
                        context_windows: vec![],
                        is_default: true,
                        fast: None,
                    }],
                    efforts: vec!["low".into(), "high".into()],
                    permission_modes: vec!["read-only".into()],
                })))
            }
            .boxed_local()
        });
        Ok(Arc::new(Probing { start: threaded_start(), catalog: Some(catalog), version: None }) as Arc<dyn HarnessAdapter>)
    })
}

/// `refusing`: `threaded()` plus a probe that names why it described nothing.
pub fn refusing(why: &str) -> HarnessAdapterFactory {
    let why = why.to_string();
    Arc::new(move |_ctx| {
        let why = why.clone();
        let catalog: CatalogFn = Rc::new(move |exec| {
            let why = why.clone();
            async move {
                exec.exec("codex --describe", None).await?;
                Ok(Some(HarnessCatalogAnswer::Refused(why)))
            }
            .boxed_local()
        });
        Ok(Arc::new(Probing { start: threaded_start(), catalog: Some(catalog), version: None }) as Arc<dyn HarnessAdapter>)
    })
}

/// What `lastStart` held: the fields a test reads, and which keys the start options carried.
#[derive(Clone)]
pub struct Last {
    pub started: Started,
    pub keys: Vec<&'static str>,
}

fn keys_of(o: &HarnessStartOptions) -> Vec<&'static str> {
    [
        ("prompt", true),
        ("resume", o.resume.is_some()),
        ("copy", o.copy),
        ("cwd", o.cwd.is_some()),
        ("model", o.model.is_some()),
        ("effort", o.effort.is_some()),
        ("permissionMode", o.permission_mode.is_some()),
        ("contextWindow", o.context_window.is_some()),
        ("fast", o.fast.is_some()),
        ("title", o.title.is_some()),
        ("images", !o.images.is_empty()),
        ("fork", o.fork.is_some()),
        ("mcpServers", o.mcp_servers.is_some()),
        ("serverValues", o.server_values.is_some()),
        ("offServers", o.off_servers.is_some()),
        ("seed", o.seed.is_some()),
        ("limitDetails", o.limit_details),
        ("promptAfter", o.prompt_after.is_some()),
        ("version", o.version.is_some()),
        ("keep", o.keep),
        ("onEvent", o.on_event.is_some()),
    ]
    .into_iter()
    .filter_map(|(key, set)| set.then_some(key))
    .collect()
}

/// `manual()`: an adapter the test drives by hand, so turn boundaries can arrive without a session.end behind them.
pub struct Manual {
    on_event: RefCell<Option<OnEvent>>,
    last: RefCell<Option<Last>>,
    pub starts: RefCell<Vec<String>>,
    finish: RefCell<Option<oneshot::Sender<TurnResult>>>,
    finished: SharedFuture<TurnResult>,
}

const MANUAL_SESSION: &str = "33333333-3333-4333-8333-333333333333";

impl Manual {
    pub fn new() -> Rc<Manual> {
        let (tx, rx) = oneshot::channel::<TurnResult>();
        let finished: LocalBoxFuture<'static, TurnResult> = async move {
            match rx.await {
                Ok(r) => r,
                Err(_) => futures::future::pending().await,
            }
        }
        .boxed_local();
        Rc::new(Manual {
            on_event: RefCell::default(),
            last: RefCell::default(),
            starts: RefCell::default(),
            finish: RefCell::new(Some(tx)),
            finished: finished.shared(),
        })
    }

    /// `adapter`: asks the claude catalog and version commands under the machine's login env, and starts by hand.
    pub fn factory(self: &Rc<Self>) -> HarnessAdapterFactory {
        let me = self.clone();
        Arc::new(move |ctx| {
            let base = ctx.env.iter().map(|(k, v)| (k.clone(), Some(v.clone()))).collect();
            let env = build_env(&ClaudeEnvOptions { base: Some(base), ..Default::default() });
            let (env_c, env_v) = (env.clone(), env);
            let catalog: CatalogFn = Rc::new(move |exec| {
                let env = env_c.clone();
                async move {
                    let out = exec.exec(&catalog_probe_command(None), Some(&env)).await?;
                    Ok(parse_catalog_probe(&out).map(HarnessCatalogAnswer::Probe))
                }
                .boxed_local()
            });
            let version: VersionFn = Rc::new(move |exec| {
                let env = env_v.clone();
                async move {
                    let out = exec.exec(&version_probe_command(None), Some(&env)).await?;
                    Ok(parse_version(&out))
                }
                .boxed_local()
            });
            let me = me.clone();
            let start: StartFn = Rc::new(move |o| {
                *me.on_event.borrow_mut() = o.on_event.clone();
                *me.last.borrow_mut() = Some(Last { started: Started::of(&o), keys: keys_of(&o) });
                me.starts.borrow_mut().push(o.prompt.clone());
                Ok(Plain::of(MANUAL_SESSION, me.finished.clone().boxed_local()))
            });
            Ok(Arc::new(Probing { start, catalog: Some(catalog), version: Some(version) }) as Arc<dyn HarnessAdapter>)
        })
    }

    pub fn last(&self) -> Option<Last> {
        self.last.borrow().clone()
    }

    fn say_event(&self, doc: Value) {
        let on = self.on_event.borrow().clone().expect("a start came first");
        on(event(doc));
    }

    pub fn start(&self, cwd: Option<&str>) {
        let mut doc = json!({ "type": "session.start", "sessionId": MANUAL_SESSION, "model": "claude-sonnet-4-5" });
        if let Some(cwd) = cwd {
            doc["cwd"] = json!(cwd);
        }
        self.say_event(doc);
    }

    pub fn tool(&self, command: &str, cwd: Option<&str>) {
        let mut doc = json!({
            "type": "turn.delta", "sessionId": MANUAL_SESSION, "kind": "tool_use", "text": json!({ "command": command }).to_string(),
            "toolName": "Bash", "toolUseId": "t1",
        });
        if let Some(cwd) = cwd {
            doc["cwd"] = json!(cwd);
        }
        self.say_event(doc);
    }

    pub fn say(&self, text: &str, message_id: Option<&str>) {
        let mut doc = json!({ "type": "turn.delta", "sessionId": MANUAL_SESSION, "kind": "text", "text": text });
        if let Some(id) = message_id {
            doc["messageId"] = json!(id);
        }
        self.say_event(doc);
    }

    pub fn done(&self, text: &str) {
        self.say_event(done_of(MANUAL_SESSION, &completed(text)));
    }

    pub fn end(&self) {
        self.say_event(end_of(MANUAL_SESSION));
        let tx = self.finish.borrow_mut().take();
        if let Some(tx) = tx {
            let _ = tx.send(result(completed("")));
        }
    }
}

/// `countingStore()`: the store the runtime stands on, whose transcript writes are counted. The Rust store keeps
/// transcripts as rows, so what the TS counts as a `putBlob("transcripts")` is one `append` of the rows here.
pub struct Counted {
    inner: Rc<dyn Store>,
    rows: CountedRows,
}

pub struct CountedRows {
    inner: Rc<dyn Store>,
    appends: Rc<Cell<u32>>,
}

impl Counted {
    pub fn over(inner: &Rc<dyn Store>) -> Rc<Counted> {
        Rc::new(Counted { inner: inner.clone(), rows: CountedRows { inner: inner.clone(), appends: Rc::default() } })
    }

    pub fn store(self: &Rc<Self>) -> Rc<dyn Store> {
        self.clone()
    }

    /// `puts()`.
    pub fn puts(&self) -> u32 {
        self.rows.appends.get()
    }

    /// `stored(id)`: the events the store holds for the workspace, as JSON.
    pub fn stored(&self, id: &str) -> Vec<Value> {
        self.inner.transcripts().expect("a store that keeps transcript rows").all(id).expect("a read").events.iter().map(j).collect()
    }
}

impl CountedRows {
    fn rows(&self) -> &dyn TranscriptRows {
        self.inner.transcripts().expect("a store that keeps transcript rows")
    }
}

impl TranscriptRows for CountedRows {
    fn all(&self, w: &str) -> Result<Summed, HostError> {
        self.rows().all(w)
    }
    fn newest(
        &self,
        w: &str,
        t: &str,
        before: Option<i64>,
        each: &mut dyn FnMut(SessionEvent) -> ControlFlow<()>,
    ) -> Result<(), HostError> {
        self.rows().newest(w, t, before, each)
    }
    fn count(&self, w: &str, t: &str) -> Result<u64, HostError> {
        self.rows().count(w, t)
    }
    fn total(&self, w: &str, t: Option<&str>) -> Result<Total, HostError> {
        self.rows().total(w, t)
    }
    fn newest_pos(&self, w: &str) -> Result<i64, HostError> {
        self.rows().newest_pos(w)
    }
    fn sizes(&self, w: &str, t: Option<&str>) -> Result<Vec<EventSize>, HostError> {
        self.rows().sizes(w, t)
    }
    fn word_events(&self, w: &str, t: &str) -> Result<Vec<SessionEvent>, HostError> {
        self.rows().word_events(w, t)
    }
    fn thread(&self, w: &str, t: &str) -> Result<Summed, HostError> {
        self.rows().thread(w, t)
    }
    fn at(&self, w: &str, positions: &[i64]) -> Result<Vec<SessionEvent>, HostError> {
        self.rows().at(w, positions)
    }
    fn append(&self, w: &str, events: &[SessionEvent]) -> Result<(), HostError> {
        self.appends.set(self.appends.get() + 1);
        self.rows().append(w, events)
    }
    fn remove(&self, w: &str, positions: &[i64]) -> Result<(), HostError> {
        self.rows().remove(w, positions)
    }
    fn clear(&self, w: &str) -> Result<(), HostError> {
        self.rows().clear(w)
    }
    fn index(&self, w: &str) -> Result<Option<String>, HostError> {
        self.rows().index(w)
    }
    fn put_index(&self, w: &str, json: &str) -> Result<(), HostError> {
        self.rows().put_index(w, json)
    }
    fn atomically(&self, f: &mut dyn FnMut() -> Result<(), HostError>) -> Result<(), HostError> {
        self.rows().atomically(f)
    }
}

#[async_trait(?Send)]
impl Store for Counted {
    async fn get(&self, c: &str, id: &str) -> Result<Option<Value>, HostError> {
        self.inner.get(c, id).await
    }
    async fn put(&self, c: &str, id: &str, v: Value) -> Result<(), HostError> {
        self.inner.put(c, id, v).await
    }
    async fn list(&self, c: &str) -> Result<Vec<Value>, HostError> {
        self.inner.list(c).await
    }
    async fn keys(&self, c: &str) -> Result<Vec<String>, HostError> {
        self.inner.keys(c).await
    }
    async fn delete(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete(c, id).await
    }
    async fn get_blob(&self, c: &str, id: &str) -> Result<Option<Vec<u8>>, HostError> {
        self.inner.get_blob(c, id).await
    }
    async fn put_blob(&self, c: &str, id: &str, bytes: Vec<u8>) -> Result<(), HostError> {
        self.inner.put_blob(c, id, bytes).await
    }
    async fn delete_blob(&self, c: &str, id: &str) -> Result<(), HostError> {
        self.inner.delete_blob(c, id).await
    }
    async fn stat_blob(&self, c: &str, id: &str) -> Result<Option<BlobMark>, HostError> {
        self.inner.stat_blob(c, id).await
    }
    fn transcripts(&self) -> Option<&dyn TranscriptRows> {
        self.inner.transcripts().map(|_| &self.rows as &dyn TranscriptRows)
    }
}

/// `placed(events)`: each with its place in the transcript, in order from one.
pub fn placed(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut e = e.clone();
            e["pos"] = json!(i + 1);
            e
        })
        .collect()
}

/// `delta(workspaceId, i, text)`.
pub fn delta(workspace_id: &str, i: usize, text: &str) -> Value {
    json!({
        "type": "session.delta", "workspaceId": workspace_id, "sessionId": "s1", "threadId": "thr_1", "kind": "tool_use",
        "text": text, "toolUseId": format!("u{i}"), "at": i,
    })
}

/// `onDisk(store, id)`: every event the workspace's transcript blobs hold, the head and then the tail.
pub async fn on_disk(store: &Rc<dyn Store>, id: &str) -> Vec<Value> {
    let mut all = vec![];
    for collection in ["transcript-heads", "transcripts"] {
        if let Some(bytes) = store.get_blob(collection, id).await.expect("a read") {
            let doc: Value = serde_json::from_slice(&bytes).expect("a transcript file is JSON");
            all.extend(doc["events"].as_array().cloned().unwrap_or_default());
        }
    }
    all
}
