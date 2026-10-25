// SPDX-License-Identifier: AGPL-3.0-only
//! What the golden seam's hard tests share: the oracle's fixtures (oracle/golden.oracle.ts), and the scripted machine
//! it ran the TS against, read here by the same rules so a test replays the TS's own calls and compares every one.
#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::{json, Map, Value};

use wsp_engine::errors::{EngineError, Result};
use wsp_engine::golden_seam::{
    AsideRow, DropName, GoldenStage, McpAgentPlan, McpPlan, McpScope, McpToolRow, ProjectMove, StageListener, ToolResult, ToolStage,
};
use wsp_engine::machine::{
    BytesLanded, ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, RunOptions, SnapshotOptions,
};

/// tests/fixtures/oracle/golden_<unit>.json.
pub fn fixture(unit: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oracle").join(format!("golden_{unit}.json"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

/// The fixture's array or object entry by name, for a test that walks every case.
pub fn cases<'a>(v: &'a Value, key: &str) -> &'a Vec<Value> {
    v[key].as_array().unwrap_or_else(|| panic!("{key} is not a list in the fixture"))
}

pub fn s(v: &Value) -> String {
    v.as_str().unwrap_or_else(|| panic!("{v} is not a string")).to_string()
}

pub fn opt_s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

pub fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().map(s).collect()).unwrap_or_default()
}

pub fn res(exit_code: i64, stdout: &str, stderr: &str) -> ExecResult {
    ExecResult { exit_code, stdout: stdout.to_string(), stderr: stderr.to_string() }
}

/// A fixture's `{ exitCode, stdout, stderr }`.
pub fn exec_result(v: &Value) -> ExecResult {
    res(v["exitCode"].as_i64().unwrap(), v["stdout"].as_str().unwrap_or(""), v["stderr"].as_str().unwrap_or(""))
}

/// The machine the oracle scripted: the first rule whose `has` is in a call's text (and whose `kind`, when it names
/// one, is the call's) answers with its next answer, the last one again once they run out; a call no rule matches
/// answers exit 0 with nothing. An answer's `lines` go to the run's `on_line` before it resolves; `throws` rejects.
#[derive(Clone)]
pub struct Scripted {
    rules: Arc<Vec<Value>>,
    used: Arc<Mutex<Vec<usize>>>,
    /// Each call as the oracle recorded it: `{ kind, text, timeoutMs?, deadlineMs?, unlogged? }`.
    pub calls: Arc<Mutex<Vec<Value>>>,
    put: bool,
}

impl Scripted {
    pub fn new(rules: &Value, put: bool) -> Self {
        let rules = rules.as_array().cloned().unwrap_or_default();
        let used = vec![0; rules.len()];
        Scripted { rules: Arc::new(rules), used: Arc::new(Mutex::new(used)), calls: Arc::new(Mutex::new(Vec::new())), put }
    }

    pub fn calls(&self) -> Value {
        Value::Array(self.calls.lock().unwrap().clone())
    }

    fn answer(&self, kind: &str, text: &str, on_line: Option<&wsp_engine::machine::OnLine>) -> Result<ExecResult> {
        let at = self
            .rules
            .iter()
            .position(|r| text.contains(r["has"].as_str().unwrap()) && r.get("kind").and_then(Value::as_str).is_none_or(|k| k == kind));
        let Some(at) = at else { return Ok(res(0, "", "")) };
        let seq = self.rules[at]["seq"].as_array().unwrap();
        let n = {
            let mut used = self.used.lock().unwrap();
            let n = used[at];
            used[at] += 1;
            n
        };
        let a = &seq[n.min(seq.len() - 1)];
        if let Some(words) = a.get("throws").and_then(Value::as_str) {
            return Err(EngineError::Other { message: words.to_string(), code: None });
        }
        if let (Some(lines), Some(on_line)) = (a.get("lines").and_then(Value::as_array), on_line) {
            for line in lines {
                on_line(line.as_str().unwrap());
            }
        }
        Ok(res(
            a.get("exit").and_then(Value::as_i64).unwrap_or(0),
            a.get("stdout").and_then(Value::as_str).unwrap_or(""),
            a.get("stderr").and_then(Value::as_str).unwrap_or(""),
        ))
    }

    fn record(&self, call: Value) {
        self.calls.lock().unwrap().push(call);
    }
}

#[async_trait]
impl Machine for Scripted {
    fn id(&self) -> &str {
        "m1"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { put_bytes: self.put, ..MachineOffers::default() }
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let mut call = Map::new();
        call.insert("kind".into(), json!("exec"));
        call.insert("text".into(), json!(cmd));
        if let Some(t) = opts.timeout_ms {
            call.insert("timeoutMs".into(), json!(t));
        }
        self.record(Value::Object(call));
        self.answer("exec", cmd, None)
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        let mut call = Map::new();
        call.insert("kind".into(), json!("run"));
        call.insert("text".into(), json!(script));
        call.insert("deadlineMs".into(), json!(opts.deadline_ms));
        if opts.unlogged {
            call.insert("unlogged".into(), json!(true));
        }
        self.record(Value::Object(call));
        self.answer("run", script, opts.on_line.as_ref())
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Ok("snap".to_string())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _signal: Option<tokio_util::sync::CancellationToken>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Err(EngineError::Other { message: "no state on the scripted machine".into(), code: None })
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Ok("https://signed.example/dl".to_string())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Ok("https://signed.example/ul".to_string())
    }
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        let mut call = Map::new();
        call.insert("kind".into(), json!("put"));
        call.insert("text".into(), json!(format!("{path}\n{}", String::from_utf8_lossy(bytes))));
        if let Some(t) = timeout_ms {
            call.insert("timeoutMs".into(), json!(t));
        }
        self.record(Value::Object(call));
        self.answer("put", path, None)?;
        Ok(None)
    }
}

/// The oracle's run calls always carry `deadlineMs`; an exec carries `timeoutMs` only when the TS passed one.
pub fn assert_calls(name: &str, got: &Value, want: &Value) {
    let (g, w) = (got.as_array().unwrap(), want.as_array().unwrap());
    for (i, (g, w)) in g.iter().zip(w.iter()).enumerate() {
        assert_eq!(g, w, "{name}: call {i} differs");
    }
    assert_eq!(g.len(), w.len(), "{name}: {} calls, the TS made {}", g.len(), w.len());
}

/// Stage lines as the oracle recorded them, `[stage, detail]`.
#[derive(Clone, Default)]
pub struct Stages(pub Arc<Mutex<Vec<Value>>>);

impl Stages {
    pub fn tool_stage(&self) -> ToolStage {
        let lines = self.0.clone();
        Arc::new(move |stage: GoldenStage, detail: Option<&str>| lines.lock().unwrap().push(json!([stage_word(stage), detail])))
    }
    pub fn listener(&self) -> StageListener {
        self.tool_stage()
    }
    pub fn value(&self) -> Value {
        Value::Array(self.0.lock().unwrap().clone())
    }
}

pub fn stage_word(stage: GoldenStage) -> String {
    serde_json::to_value(stage).unwrap().as_str().unwrap().to_string()
}

/// A result as the oracle wrote it: `ms` as whether the loop set it.
pub fn timeless(r: &ToolResult) -> Value {
    let mut v = serde_json::to_value(r).unwrap();
    if r.ms.is_some() {
        v["ms"] = json!(true);
    }
    v
}

pub fn timeless_all(rows: &[ToolResult]) -> Value {
    Value::Array(rows.iter().map(timeless).collect())
}

/// A ToolResult from a fixture row; `ms: true` reads as a set clock.
pub fn tool_result(v: &Value) -> ToolResult {
    let mut v = v.clone();
    if v.get("ms") == Some(&json!(true)) {
        v["ms"] = json!(1);
    }
    serde_json::from_value(v).unwrap()
}

pub fn format_of(name: &str) -> wsp_catalog::mcp::McpFormat {
    use wsp_catalog::mcp::McpFormat;
    match name {
        "McpServersJson" => McpFormat::McpServersJson,
        "CodexToml" => McpFormat::CodexToml,
        "OpencodeJson" => McpFormat::OpencodeJson,
        other => panic!("no format {other}"),
    }
}

pub fn format_name(f: wsp_catalog::mcp::McpFormat) -> &'static str {
    use wsp_catalog::mcp::McpFormat;
    match f {
        McpFormat::McpServersJson => "McpServersJson",
        McpFormat::CodexToml => "CodexToml",
        McpFormat::OpencodeJson => "OpencodeJson",
        McpFormat::GeminiSettingsJson => "GeminiSettingsJson",
    }
}

/// An McpAgentPlan list from the oracle's JSON (`format` by name; no `edit` closures in any fixture).
pub fn agents_of(v: &Value) -> Vec<McpAgentPlan> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|a| McpAgentPlan {
            id: s(&a["id"]),
            label: s(&a["label"]),
            scopes: a["scopes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|sc| McpScope {
                    files: strings(&sc["files"]),
                    format: format_of(sc["format"].as_str().unwrap()),
                    project: sc.get("project").map(|p| ProjectMove { from: s(&p["from"]), to: s(&p["to"]) }),
                    keep: strings(&sc["keep"]),
                    drop: sc["drop"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|d| DropName { name: s(&d["name"]), reason: s(&d["reason"]), keys: d.get("keys").map(strings) })
                        .collect(),
                    travelled: sc.get("travelled").and_then(opt_s_ref),
                    own: None,
                    folder: sc.get("folder").and_then(opt_s_ref),
                    trust: sc.get("trust").and_then(opt_s_ref),
                    edit: None,
                })
                .collect(),
            aside: a["aside"]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| AsideRow { id: s(&r["id"]), name: s(&r["name"]), reason: s(&r["reason"]) })
                .collect(),
        })
        .collect()
}

fn opt_s_ref(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

pub fn plan_of(v: &Value) -> McpPlan {
    McpPlan {
        agents: agents_of(&v["agents"]),
        guest_home: s(&v["guestHome"]),
        rewrites: v["rewrites"].as_array().unwrap().iter().map(|p| (s(&p[0]), s(&p[1]))).collect(),
        bin_dirs: strings(&v["binDirs"]),
        tools: v["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| McpToolRow { id: s(&t["id"]), ticked: t["ticked"].as_bool().unwrap(), reason: t.get("reason").and_then(opt_s_ref) })
            .collect(),
    }
}

/// An McpAgentPlan list back as the oracle wrote it, so a plan compares as JSON.
pub fn agents_json(agents: &[McpAgentPlan]) -> Value {
    Value::Array(
        agents
            .iter()
            .map(|a| {
                json!({
                    "id": a.id,
                    "label": a.label,
                    "scopes": a.scopes.iter().map(|sc| {
                        let mut o = Map::new();
                        o.insert("files".into(), json!(sc.files));
                        o.insert("format".into(), json!(format_name(sc.format)));
                        if let Some(p) = &sc.project { o.insert("project".into(), json!({ "from": p.from, "to": p.to })); }
                        o.insert("keep".into(), json!(sc.keep));
                        o.insert("drop".into(), Value::Array(sc.drop.iter().map(|d| {
                            let mut m = Map::new();
                            m.insert("name".into(), json!(d.name));
                            m.insert("reason".into(), json!(d.reason));
                            if let Some(k) = &d.keys { m.insert("keys".into(), json!(k)); }
                            Value::Object(m)
                        }).collect()));
                        if let Some(f) = &sc.folder { o.insert("folder".into(), json!(f)); }
                        Value::Object(o)
                    }).collect::<Vec<_>>(),
                    "aside": a.aside.iter().map(|r| json!({ "id": r.id, "name": r.name, "reason": r.reason })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

pub fn plan_json(p: &McpPlan) -> Value {
    json!({
        "agents": agents_json(&p.agents),
        "guestHome": p.guest_home,
        "rewrites": p.rewrites.iter().map(|(a, b)| json!([a, b])).collect::<Vec<_>>(),
        "binDirs": p.bin_dirs,
        "tools": p.tools.iter().map(|t| {
            let mut m = Map::new();
            m.insert("id".into(), json!(t.id));
            m.insert("ticked".into(), json!(t.ticked));
            if let Some(r) = &t.reason { m.insert("reason".into(), json!(r)); }
            Value::Object(m)
        }).collect::<Vec<_>>(),
    })
}
