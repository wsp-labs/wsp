// SPDX-License-Identifier: AGPL-3.0-only
//! The helpers golden_import_b.rs and golden_import_b_b.rs share: the fixtures, the runners, the stage and command matchers,
//! and the two fake machines the TS builds by hand (a backend whose exec fails, and a machine whose free-disk read fails).
#![allow(dead_code, unused_imports)]

use std::cell::{Cell, RefCell};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use crate::golden_roads_common::*;
use indexmap::IndexMap;
use sha2::{Digest, Sha256};
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_catalog::road_modules::{road_step, CURL_NET};
use wsp_catalog::roads::{node_install_script, node_release, RoadName, CLAUDE_INSTALL, GOLDEN_SETUP, NODE_PATH_LINE};
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::fetch::Fetch;
use wsp_engine::golden::{
    apply_golden_import, prepare_builder, seal_golden, AgentResult, Applied, ApplyImportOptions, Builder, GoldenError, ImportLedger,
    ImportResult, ImportStage, PrepareBuilderOptions, ResultFiles, SealGoldenOptions, SealResult, AGENT_INSTALLERS,
};
use wsp_engine::golden_seam::{
    install_tools, tool_installs_for_rows, AgentInstall, BrewTable, GoldenFiles, GoldenImport, GoldenVolatile, InstallToolsOptions,
    McpAgentPlan, McpPlan, McpScope, NodeInstall, PackFiles, PackedFiles, PluginSeedPlan, PluginsPlan, RecipeEntry, SeedLinks, SeedPlugin,
    LeaveOut, SeedRead, SkippedPath, SkippedRow, ToolInstall, ToolResult, ToolStage, TOOLS_PATH,
};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, BaseTemplates, Capabilities, ExecOptions, ExecResult, Machine, MachineBackend, MachineKind, MachineLife,
    MachineListRow, MachineOffers, MachineShape, MachineSpec, MachineState, RunOptions, Seen, SnapshotOptions, TemplateRow, Waiting,
};
use wsp_engine::Cancel;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::{GoldenLeftBehind, GoldenMissingTool, GoldenMissingToolOutcome, GoldenStage};

pub const SWEEP: &str = "rm -rf /root/.npm /root/.cache/uv /root/.cache/go-build /root/.cache/node-gyp";
pub const KNOWN: &str = "/root/.claude-cfg/plugins/known_marketplaces.json";
pub const INSTALLED: &str = "/root/.claude-cfg/plugins/installed_plugins.json";

// --- shared fixtures and runners ---------------------------------------------------------------------------------------

pub type Results = Rc<RefCell<Vec<ImportResult>>>;

/// `onResult: r => void results.push(r)`.
pub fn collect_results() -> (Results, Arc<dyn Fn(&ImportResult)>) {
    let results: Results = Rc::default();
    let w = results.clone();
    (results, Arc::new(move |r: &ImportResult| w.borrow_mut().push(r.clone())))
}

/// `prepareBuilder({ backend, setup: "true", fetch, import })`, the fetch the backend records its uploads on.
pub fn prep(b: &BackendFor, import: GoldenImport) -> PrepareBuilderOptions {
    prep_setup(b, "true", import)
}

pub fn prep_setup(b: &BackendFor, setup: &str, import: GoldenImport) -> PrepareBuilderOptions {
    PrepareBuilderOptions { import: Some(import), fetch: Some(b.dyn_fetch()), ..prepare_opts(&b.rec, setup) }
}

pub fn build(o: PrepareBuilderOptions) -> Builder {
    run(prepare_builder(o, &*Manual::new())).expect("the build prepares")
}

pub fn build_err(o: PrepareBuilderOptions) -> GoldenError {
    run(prepare_builder(o, &*Manual::new())).err().expect("the build fails")
}

pub fn apply(b: &Builder, o: ApplyImportOptions) -> Applied {
    run(apply_golden_import(&b.machine, o, &*Manual::new())).expect("the import applies")
}

pub fn seal(b: Builder, o: SealGoldenOptions) -> SealResult {
    run(seal_golden(b, o, &*Manual::new())).expect("the golden seals")
}

pub fn applied_of(b: &Builder) -> Vec<ImportStage> {
    b.import.as_ref().expect("the builder carries a ledger").applied.clone()
}

pub fn ledger_of(b: &Builder) -> ImportLedger {
    b.import.clone().expect("the builder carries a ledger")
}

/// One stage line a test wants: an exact line, or one a hand-written check accepts.
pub enum Want {
    Is(&'static str),
    Re(fn(&str) -> bool),
}

pub fn assert_stages(got: &[String], want: &[Want]) {
    assert_eq!(got.len(), want.len(), "stages: {got:#?}");
    for (g, w) in got.iter().zip(want) {
        match w {
            Want::Is(s) => assert_eq!(g, s),
            Want::Re(f) => assert!(f(g), "{g:?} does not match"),
        }
    }
}

pub fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// `\d+(\.\d+)?`
pub fn is_number(s: &str) -> bool {
    match s.split_once('.') {
        None => is_digits(s),
        Some((w, f)) => is_digits(w) && is_digits(f),
    }
}

/// `^<head>\d+\.\ds<tail>$`: a duration with exactly one decimal place.
pub fn timed_tenth(line: &str, head: &str, tail: &str) -> bool {
    line.strip_prefix(head)
        .and_then(|r| r.strip_suffix(tail))
        .and_then(|n| n.strip_suffix('s'))
        .and_then(|n| n.split_once('.'))
        .is_some_and(|(w, f)| is_digits(w) && f.len() == 1 && is_digits(f))
}

/// `^installing-mcp:1 skipped; machine context: \d+(\.\d+)? KB written; no agent on the machine$`
pub fn mcp_context_skipped(line: &str) -> bool {
    line.strip_prefix("installing-mcp:1 skipped; machine context: ")
        .and_then(|r| r.strip_suffix(" KB written; no agent on the machine"))
        .is_some_and(is_number)
}

/// TS `findIndex(c => c.includes(needle))`, -1 when none.
pub fn at(cmds: &[String], needle: &str) -> isize {
    cmds.iter().position(|c| c.contains(needle)).map_or(-1, |i| i as isize)
}

/// TS `indexOf(exact)`, -1 when none.
pub fn exact(cmds: &[String], text: &str) -> isize {
    cmds.iter().position(|c| c == text).map_or(-1, |i| i as isize)
}

/// TS `lastIndexOf(exact)`, -1 when none.
pub fn last_exact(cmds: &[String], text: &str) -> isize {
    cmds.iter().rposition(|c| c == text).map_or(-1, |i| i as isize)
}

/// TS `cmds.filter(c => c.includes(needle))`.
pub fn all_with(cmds: &[String], needle: &str) -> Vec<String> {
    cmds.iter().filter(|c| c.contains(needle)).cloned().collect()
}

/// TS `cmds.find(c => c.includes(needle))!`.
pub fn first_with(cmds: &[String], needle: &str) -> String {
    cmds.iter().find(|c| c.contains(needle)).cloned().unwrap_or_else(|| panic!("no command has {needle:?}"))
}

/// TS `text.indexOf(needle)`, -1 when none.
pub fn in_text(text: &str, needle: &str) -> isize {
    text.find(needle).map_or(-1, |i| i as isize)
}

/// TS `stages.filter(s => s.startsWith(prefix))`.
pub fn starting(all: &[String], prefix: &str) -> Vec<String> {
    all.iter().filter(|s| s.starts_with(prefix)).cloned().collect()
}

/// A regex `a.*b.*c` with `s`-flag dots: each literal found after the one before it.
pub fn in_order(text: &str, parts: &[&str]) -> bool {
    let mut rest = text;
    for p in parts {
        match rest.find(p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    true
}

/// `/\|\s*(bash|sh)\b/`
pub fn pipes_into_shell(text: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices('|').any(|(i, _)| {
        let rest = text[i + 1..].trim_start_matches(char::is_whitespace);
        ["bash", "sh"].iter().any(|w| rest.strip_prefix(w).is_some_and(|after| !after.chars().next().is_some_and(word)))
    })
}

/// `/\bcurl +-[A-Za-z]*[fsSL]\b/`
pub fn curl_fsl_flag(s: &str) -> bool {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    s.match_indices("curl").any(|(i, _)| {
        if s[..i].chars().next_back().is_some_and(word) {
            return false;
        }
        let rest = &s[i + 4..];
        let spaces = rest.len() - rest.trim_start_matches(' ').len();
        let Some(flags) = rest[spaces..].strip_prefix('-').filter(|_| spaces > 0) else { return false };
        let letters: String = flags.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
        let Some(last) = letters.chars().last() else { return false };
        "fsSL".contains(last) && !flags[letters.len()..].chars().next().is_some_and(word)
    })
}

/// `s` after at least one whitespace character, then `lit`; the rest of `s` past `lit`.
pub fn ws_then<'a>(s: &'a str, lit: &str) -> Option<&'a str> {
    let t = s.trim_start_matches(char::is_whitespace);
    (t.len() < s.len() && t.starts_with(lit)).then(|| &t[lit.len()..])
}

/// `/kill -KILL[^\n]*\n\s+t=0; while \[ \$t -lt 10 \] && kill -0 \$v[^\n]*\n\s+exit 124\n/`
pub fn kill_then_exit(guard: &str) -> bool {
    guard.match_indices("kill -KILL").any(|(p, _)| {
        let Some(nl) = guard[p..].find('\n') else { return false };
        let Some(r2) = ws_then(&guard[p + nl + 1..], "t=0; while [ $t -lt 10 ] && kill -0 $v") else { return false };
        let Some(nl2) = r2.find('\n') else { return false };
        ws_then(&r2[nl2 + 1..], "exit 124\n").is_some()
    })
}

pub fn node_plan() -> NodeInstall {
    NodeInstall { floor: 22, version: "22.23.2".into(), agents: vec!["Pi".into()], cmd: "node-step".into() }
}

pub fn agent(id: &str, name: &str, install: &str, smoke: &str, road: &str, node: Option<u32>) -> AgentInstall {
    AgentInstall { id: id.into(), name: name.into(), install: install.into(), smoke: smoke.into(), road: road.into(), node, pin: None }
}

/// `{ id, outcome, note }` of a tool row, for a `toEqual` over the rows' three fields.
pub fn row(id: &str, outcome: &str, note: Option<&str>) -> (String, String, Option<String>) {
    (id.to_string(), outcome.to_string(), note.map(str::to_string))
}

pub fn tuples(v: &[ToolResult]) -> Vec<(String, String, Option<String>)> {
    v.iter().map(|t| (t.id.clone(), t.outcome.clone(), t.note.clone())).collect()
}

pub fn missing(id: &str, name: &str, outcome: GoldenMissingToolOutcome, note: &str) -> GoldenMissingTool {
    GoldenMissingTool { id: id.into(), name: name.into(), outcome, note: note.into() }
}

pub fn pack_with(f: impl Fn() -> std::result::Result<PackedFiles, EngineError> + 'static) -> PackFiles {
    Arc::new(move |_: Option<LeaveOut>| {
        let r = f();
        async move { r }.boxed_local()
    })
}

pub fn pack_failing(message: &'static str) -> PackFiles {
    pack_with(move || Err(EngineError::Other { message: message.to_string(), code: None }))
}

pub fn pack_tgz(silenced: Vec<String>) -> PackFiles {
    pack_with(move || Ok(PackedFiles { tar: b"tgz-bytes".to_vec(), bytes: 1200, unpacked: 4096, silenced: silenced.clone(), ..Default::default() }))
}

pub fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

pub fn node_exe() -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|d| d.join("node"))
        .find(|p| p.is_file())
        .expect("a node on this computer's PATH, which the stub runs under as the TS runs it under process.execPath")
}

pub fn write_stub(path: &Path, body: &str) {
    std::fs::write(path, body).expect("the stub is written");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("the stub is executable");
}

/// The fake machine's answers for the pnpm seal: a `pnpm --version` runs for real under `bin` alone on PATH.
pub fn pnpm_answer(bin: &Path, cmd: &str) -> Option<ExecResult> {
    if cmd.contains("pnpm --version") {
        let out = Command::new("bash")
            .args(["-c", cmd])
            .env_clear()
            .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
            .output()
            .expect("bash runs");
        return Some(exec(i64::from(out.status.code().unwrap_or(1)), &String::from_utf8_lossy(&out.stdout), &String::from_utf8_lossy(&out.stderr)));
    }
    Some(if cmd == FREE_KB_CMD {
        exec(0, &format!("{}\n", mb(3000)), "")
    } else if cmd.contains("echo WSP_CTX") {
        exec(0, "WSP_CTX\nWSP_CTX_END\n", "")
    } else if cmd == "echo ok" {
        reach_ok()
    } else {
        ok()
    })
}

/// A seal over the pnpm tool row, on a builder whose smoke is the row's check; returns the recording backend too.
pub fn pnpm_seal(bin: &Path, tools: Vec<ToolInstall>) -> (Rec, std::result::Result<SealResult, GoldenError>) {
    let bin = bin.to_path_buf();
    let rec = recording_backend(Opts { exec: Some(Box::new(move |cmd: &str| pnpm_answer(&bin, cmd))), ..Opts::default() });
    let mut o = prepare_opts(&rec, "true");
    let fetch: Arc<dyn Fetch> = FetchStub::ok();
    o.fetch = Some(fetch);
    o.import = Some(GoldenImport { tools, agents: vec![], ..import_of() });
    let builder = build(o);
    let sealed = run(seal_golden(builder, seal_opts(&rec, "true"), &*Manual::new()));
    (rec, sealed)
}

/// The Rust form of a backend whose `exec` of one command fails the way a dropped connection does.
pub struct FailingExec {
    inner: Arc<dyn MachineBackend>,
    needle: &'static str,
    message: &'static str,
}

#[async_trait(?Send)]
impl MachineBackend for FailingExec {
    fn capabilities(&self) -> Capabilities {
        self.inner.capabilities()
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        self.inner.pricing()
    }
    fn base_templates(&self) -> Option<BaseTemplates> {
        self.inner.base_templates()
    }
    fn offers(&self) -> BackendOffers {
        self.inner.offers()
    }
    async fn create(&self, spec: MachineSpec, waiting: Option<Waiting>, sent: Option<String>) -> Result<Arc<dyn Machine>> {
        let inner = self.inner.create(spec, waiting, sent).await?;
        Ok(Arc::new(FailingExecMachine { inner, needle: self.needle, message: self.message }))
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        let inner = self.inner.get(id).await?;
        Ok(Arc::new(FailingExecMachine { inner, needle: self.needle, message: self.message }))
    }
    async fn list(&self, labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        self.inner.list(labels).await
    }
    async fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.inner.delete_snapshot(id).await
    }
    async fn promote_snapshot(&self, snapshot_id: &str, name: &str) -> Result<String> {
        self.inner.promote_snapshot(snapshot_id, name).await
    }
    async fn get_template(&self, id: &str) -> Result<TemplateRow> {
        self.inner.get_template(id).await
    }
    async fn list_templates(&self) -> Result<Vec<TemplateRow>> {
        self.inner.list_templates().await
    }
    async fn delete_template(&self, id: &str) -> Result<()> {
        self.inner.delete_template(id).await
    }
}

pub struct FailingExecMachine {
    inner: Arc<dyn Machine>,
    needle: &'static str,
    message: &'static str,
}

#[async_trait(?Send)]
impl Machine for FailingExecMachine {
    fn id(&self) -> &str {
        self.inner.id()
    }
    fn kind(&self) -> MachineKind {
        self.inner.kind()
    }
    fn stream_url(&self) -> Option<&str> {
        self.inner.stream_url()
    }
    fn seen(&self) -> Option<&Seen> {
        self.inner.seen()
    }
    fn offers(&self) -> MachineOffers {
        self.inner.offers()
    }
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        if cmd.contains(self.needle) {
            return Err(EngineError::Other { message: self.message.to_string(), code: None });
        }
        self.inner.exec(cmd, opts).await
    }
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        self.inner.run(script, opts).await
    }
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        self.inner.snapshot(name, life, opts).await
    }
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.inner.pause(wait_ms).await
    }
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        self.inner.resume(signal).await
    }
    async fn kill(&self) -> Result<()> {
        self.inner.kill().await
    }
    async fn state(&self) -> Result<MachineState> {
        self.inner.state().await
    }
    async fn download_url(&self, path: &str) -> Result<String> {
        self.inner.download_url(path).await
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        self.inner.upload_url(path).await
    }
    async fn describe(&self) -> Result<MachineShape> {
        self.inner.describe().await
    }
}

/// A machine whose every exec answers, except the free-disk read, which throws the way a lost machine does.
pub struct FreeReadFails {
    pub ran: RefCell<Vec<String>>,
}

pub fn not_on_this_road() -> EngineError {
    EngineError::Other { message: "not on this road".to_string(), code: None }
}

#[async_trait(?Send)]
impl Machine for FreeReadFails {
    fn id(&self) -> &str {
        "spoo"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        if cmd == FREE_KB_CMD {
            return Err(EngineError::Other { message: "exec lost the machine".to_string(), code: None });
        }
        Ok(ok())
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult> {
        self.ran.borrow_mut().push(script.to_string());
        Ok(ok())
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(not_on_this_road())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Err(not_on_this_road())
    }
    async fn resume(&self, _signal: Option<&Cancel>) -> Result<()> {
        Err(not_on_this_road())
    }
    async fn kill(&self) -> Result<()> {
        Err(not_on_this_road())
    }
    async fn state(&self) -> Result<MachineState> {
        Err(not_on_this_road())
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(not_on_this_road())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(not_on_this_road())
    }
}

/// `plugins.seed.links`: the pairs the built plugins are found by, whatever the folder reads.
pub struct FixedLinks(pub Vec<(String, String)>);

#[async_trait(?Send)]
impl SeedLinks for FixedLinks {
    async fn links(&self, _read: &dyn SeedRead) -> Vec<(String, String)> {
        self.0.clone()
    }
}

/// golden.test.ts:1722's `ln -sfn` line: a link step whose transport fails is said on the stage, not fatal.
pub fn links_left_out(line: &str) -> bool {
    let head = "installing-harness:plugins built in: v@acme; left out: the links into /seed (";
    line.strip_prefix(head).is_some_and(|rest| rest.find("socket hang up").is_some_and(|i| !rest[..i].contains('\n')))
}


/// A backend over `inner` whose `exec` of any command containing `needle` fails with `message`.
pub fn failing_exec(inner: Arc<dyn MachineBackend>, needle: &'static str, message: &'static str) -> Arc<dyn MachineBackend> {
    Arc::new(FailingExec { inner, needle, message })
}
