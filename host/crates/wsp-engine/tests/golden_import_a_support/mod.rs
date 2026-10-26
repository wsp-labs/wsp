// SPDX-License-Identifier: AGPL-3.0-only
//! The fixtures of golden.test.ts:1113-1698 that the two files of this block share.
#![allow(dead_code, clippy::type_complexity)]
use crate::golden_roads_common::*;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::golden::{
    apply_golden_import, prepare_builder, seal_golden, AgentResult, Applied, ApplyImportOptions, Builder, DeployDaemon, ImportLedger,
    ImportResult, ImportStage, PrepareBuilderOptions, SealGoldenOptions, SealResult,
};
use wsp_engine::golden_seam::LeaveOut;
use wsp_engine::golden_seam::{
    tool_installs_for_rows, BrewTable, GoldenImport, OnContext, PackFiles, PackedFiles, RecipeEntry, SkippedRow, ToolInstall, ToolResult,
    TOOLS_PATH,
};
use wsp_engine::machine::Machine;
use wsp_engine::machine_context::ContextResult;

/// `Uploads`: the fetch the upload runs over. It records every body in order, as the TS `puts` array does, and answers 413
/// to the call whose count before it is in `refuse`.
pub struct Uploads {
    pub puts: RefCell<Vec<Vec<u8>>>,
    pub refuse: RefCell<HashSet<usize>>,
}

impl Uploads {
    pub fn new(refuse: &[usize]) -> Arc<Self> {
        Arc::new(Uploads { puts: RefCell::new(Vec::new()), refuse: RefCell::new(refuse.iter().copied().collect()) })
    }
    pub fn puts(&self) -> Vec<Vec<u8>> {
        self.puts.borrow().clone()
    }
    pub fn count(&self) -> usize {
        self.puts.borrow().len()
    }
    pub fn refuse_at(&self, before: usize) {
        self.refuse.borrow_mut().insert(before);
    }
}

#[async_trait(?Send)]
impl Fetch for Uploads {
    async fn fetch(&self, req: FetchRequest) -> Result<FetchResponse, RoadError> {
        let before = self.count();
        self.puts.borrow_mut().push(req.body.unwrap_or_default());
        if self.refuse.borrow().contains(&before) {
            return Ok(FetchResponse { status: 413, headers: Vec::new(), body: br#"{"error":"Payload Too Large","limit":1024}"#.to_vec() });
        }
        Ok(FetchResponse { status: 200, headers: Vec::new(), body: Vec::new() })
    }
}

pub fn up_fetch(up: &Arc<Uploads>) -> Option<Arc<dyn Fetch>> {
    Some(up.clone())
}

/// `prepareBuilder({ backend, setup: "true", fetch, import })` with the stages the test listens to set by the caller.
pub fn prep(rec: &Rec, up: &Arc<Uploads>, import: GoldenImport) -> PrepareBuilderOptions {
    let mut o = prepare_opts(rec, "true");
    o.fetch = up_fetch(up);
    o.import = Some(import);
    o
}

pub fn build(o: PrepareBuilderOptions) -> Builder {
    run(prepare_builder(o, &*Manual::new())).expect("the build prepares")
}

pub fn apply(b: &Builder, o: ApplyImportOptions) -> Applied {
    run(apply_golden_import(&b.machine, o, &*Manual::new())).expect("the import applies")
}

pub fn seal(b: Builder, o: SealGoldenOptions) -> SealResult {
    run(seal_golden(b, o, &*Manual::new())).expect("the golden seals")
}

pub fn daemon(reply: &'static str) -> DeployDaemon {
    Arc::new(move |_: Arc<dyn Machine>| async move { Ok::<Option<String>, EngineError>(Some(reply.to_string())) }.boxed_local())
}

pub fn pack_of(p: PackedFiles) -> PackFiles {
    Arc::new(move |_: Option<LeaveOut>| {
        let p = p.clone();
        async move { Ok::<PackedFiles, EngineError>(p) }.boxed_local()
    })
}

pub type Results = Rc<RefCell<Vec<ImportResult>>>;
pub type Outcomes = Rc<RefCell<Vec<(Vec<ContextResult>, Option<String>)>>>;

/// `onResult: r => void results.push(r)`.
pub fn collect_results() -> (Results, Arc<dyn Fn(&ImportResult)>) {
    let results: Results = Rc::default();
    let w = results.clone();
    (results, Arc::new(move |r: &ImportResult| w.borrow_mut().push(r.clone())))
}

/// `onContext: o => void outcomes.push(o)`, with `o` the `Pick<ImportResult, "context" | "contextFailure">`.
pub fn collect_outcomes() -> (Outcomes, OnContext) {
    let outcomes: Outcomes = Rc::default();
    let w = outcomes.clone();
    (outcomes, Arc::new(move |c: &[ContextResult], f: Option<&str>| w.borrow_mut().push((c.to_vec(), f.map(str::to_string)))))
}

pub fn ctx_none() -> (Vec<ContextResult>, Option<String>) {
    (Vec::new(), None)
}

pub fn applied_of(b: &Builder) -> Vec<ImportStage> {
    b.import.as_ref().expect("the builder carries a ledger").applied.clone()
}

pub fn ledger_of(b: &Builder) -> ImportLedger {
    b.import.clone().expect("the builder carries a ledger")
}

/// `stage` lines a test wants: an exact line, or one a regex matches (written as a hand-checked function).
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

/// `\d+(\.\d)?s`
pub fn is_seconds(s: &str) -> bool {
    let Some(num) = s.strip_suffix('s') else { return false };
    match num.split_once('.') {
        None => is_digits(num),
        Some((w, f)) => is_digits(w) && f.len() == 1 && is_digits(f),
    }
}

/// `^<head>\d+(\.\d)?s<tail>$`
pub fn timed(line: &str, head: &str, tail: &str) -> bool {
    line.strip_prefix(head).and_then(|r| r.strip_suffix(tail)).is_some_and(is_seconds)
}

/// `^installing-mcp:machine context: \d+(\.\d+)? KB written for Claude Code, Codex; Pi by its fallback \(~\/\.pi\/agent\/APPEND_SYSTEM\.md already exists\)$`
pub fn pi_fallback_line(line: &str) -> bool {
    line.strip_prefix("installing-mcp:machine context: ")
        .and_then(|r| {
            r.strip_suffix(" KB written for Claude Code, Codex; Pi by its fallback (~/.pi/agent/APPEND_SYSTEM.md already exists)")
        })
        .is_some_and(is_number)
}

/// `^write failed: .*HTTP 413`
pub fn write_failed_413(s: &str) -> bool {
    s.strip_prefix("write failed: ").and_then(|r| r.find("HTTP 413").map(|i| !r[..i].contains('\n'))).unwrap_or(false)
}

/// `^installing-mcp:machine context: not written \(write failed: .*HTTP 413`
pub fn not_written_413(line: &str) -> bool {
    line.strip_prefix("installing-mcp:machine context: not written (").is_some_and(write_failed_413)
}

/// TS `findIndex(c => c.includes(needle))`, -1 when none.
pub fn find_index(cmds: &[String], needle: &str) -> isize {
    cmds.iter().position(|c| c.contains(needle)).map_or(-1, |i| i as isize)
}

/// TS `indexOf(exact)`, -1 when none.
pub fn index_of(cmds: &[String], exact: &str) -> isize {
    cmds.iter().position(|c| c == exact).map_or(-1, |i| i as isize)
}

/// TS `find(c => c.includes(needle))!`.
pub fn found(cmds: &[String], needle: &str) -> String {
    cmds.iter().find(|c| c.contains(needle)).cloned().unwrap_or_else(|| panic!("no command has {needle:?}"))
}

/// `boundedCommand(seconds, cmd)` from machine-context.ts:197, copied as the TS writes it.
pub fn bounded(seconds: u32, cmd: &str) -> String {
    format!("set -m; {cmd} & p=$!; ( sleep {seconds}; kill -KILL -- -$p ) >/dev/null 2>&1 & k=$!; set +m; wait $p; r=$?; kill -KILL -- -$p -$k 2>/dev/null; [ $r -ne 137 ]")
}

/// `tally.replace(/ \([^()]*\)/g, "")`: every bracket with no bracket inside it, with the space before it, removed.
pub fn strip_brackets(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    loop {
        let Some(i) = rest.find(" (") else {
            out.push_str(rest);
            return out;
        };
        let after = &rest[i + 2..];
        match after.find(['(', ')']) {
            Some(j) if after.as_bytes()[j] == b')' => {
                out.push_str(&rest[..i]);
                rest = &after[j + 1..];
            }
            _ => {
                out.push_str(&rest[..i + 2]);
                rest = after;
            }
        }
    }
}

/// The ms a result carries is a number in the TS (`expect.any(Number)`); the test compares that it is there, not its value.
pub fn norm_tools(v: &[ToolResult]) -> Vec<ToolResult> {
    v.iter()
        .cloned()
        .map(|mut t| {
            if t.ms.is_some() {
                t.ms = Some(0);
            }
            t
        })
        .collect()
}

pub fn norm_agents(v: &[AgentResult]) -> Vec<AgentResult> {
    v.iter()
        .cloned()
        .map(|mut a| {
            if a.ms.is_some() {
                a.ms = Some(0);
            }
            a
        })
        .collect()
}

/// An installed tool row: `ms` a number, `bytes` 0.
pub fn done(id: &str, label: &str) -> ToolResult {
    ToolResult { id: id.into(), label: label.into(), outcome: "installed".into(), ms: Some(0), bytes: Some(0.0), ..Default::default() }
}

pub fn agent_done(id: &str, name: &str) -> AgentResult {
    AgentResult { id: id.into(), name: name.into(), outcome: "installed".into(), note: None, ms: Some(0) }
}

pub fn inline_of(b: &BackendFor) -> Vec<(String, String, Option<u64>)> {
    b.rec.state.inline.borrow().clone()
}

pub fn inline_cmds(b: &BackendFor) -> Vec<String> {
    inline_of(b).into_iter().map(|(_, c, _)| c).collect()
}

pub fn missing_check(b: &BackendFor) -> (String, String, Option<u64>) {
    inline_of(b).into_iter().rfind(|(_, c, _)| c.contains("echo \"missing")).expect("a check of the tools PATH")
}

pub fn skipped_tool_raycast() -> SkippedRow {
    SkippedRow { id: "tools/brew-cask/raycast".into(), label: "Raycast".into(), note: "macOS app, no Linux build".into() }
}

pub fn gopls_and_spoo() -> Vec<ToolInstall> {
    vec![
        ToolInstall {
            id: "tools/go/gopls".into(),
            label: "gopls".into(),
            manager: "go".into(),
            cmd: "go install gopls".into(),
            bin: Some("gopls".into()),
            ..Default::default()
        },
        ToolInstall {
            id: "tools/cli/spoo".into(),
            label: "spoo".into(),
            manager: "release".into(),
            cmd: "curl https://api.github.com/repos/spoo-me/spoo-cli/releases/tags/v0.4.1".into(),
            bin: Some("spoo".into()),
            ..Default::default()
        },
    ]
}

pub fn tool_entry(id: &str, label: &str, default: &str, version: Option<&str>, linux: Option<&str>) -> RecipeEntry {
    RecipeEntry {
        rung: "tools".into(),
        id: id.into(),
        label: label.into(),
        default: default.into(),
        bring: Some(true),
        linux: linux.map(Into::into),
        version: version.map(Into::into),
        ..Default::default()
    }
}

pub fn plan_installs(rows: &[RecipeEntry], table: &BrewTable) -> Vec<ToolInstall> {
    tool_installs_for_rows(rows, table, &[], TOOLS_PATH, None).expect("the plan").installs
}

pub const GH_WSP_ROAD_B: &str = "WSP_ROAD release gh_2.101.0_linux_amd64.tar.gz";
