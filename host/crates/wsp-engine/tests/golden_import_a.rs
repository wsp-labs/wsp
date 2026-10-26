// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts:1113-1698 (inside "golden import stages"): the machine context and its upload, the upload parts, the
//! stage order of a build and an attach, the shell step, the tools' rows and their checks, the agents and the plugin seed.
//! A stage frame's `step` is not carried by the engine's tools loop, so no assertion here reads one.
mod golden_roads_common;

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use golden_roads_common::*;
use sha2::{Digest, Sha256};
use wsp_catalog::context::ContextOutcomeKind;
use wsp_catalog::road_modules::CURL_NET;
use wsp_engine::errors::{EngineError, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::golden::{
    apply_golden_import, prepare_builder, seal_golden, AgentResult, Applied, ApplyImportOptions, Builder, DeployDaemon,
    ImportLedger, ImportResult, ImportStage, PrepareBuilderOptions, ResultFiles, SealGoldenOptions, SealResult,
};
use wsp_engine::golden_seam::{
    custom_installs_for, tool_installs_for_rows, GoldenImport, BrewFormula, BrewTable, HomebrewCheckout, LoginShell, McpPlan, NodeInstall, OnContext,
    PackFiles, PackedFiles, PinRead, PluginSeedPlan, PluginsPlan, RecipeEntry, SeedLinks, SeedPlugin, SeedRead, ShellInstall, SkippedPath,
    SkippedRow, ToolInstall, ToolResult, ToolRoad, ToolSource, TOOLS_PATH,
};
use wsp_engine::golden_seam::LeaveOut;
use wsp_engine::machine::Machine;
use wsp_engine::machine_context::ContextResult;
use wsp_wire::views::golden_image::{CustomLit, GoldenMissingTool, GoldenMissingToolOutcome, RecipeCustomRow};

/// `Uploads`: the fetch the upload runs over. It records every body in order, as the TS `puts` array does, and answers 413
/// to the call whose count before it is in `refuse`.
struct Uploads {
    puts: RefCell<Vec<Vec<u8>>>,
    refuse: RefCell<HashSet<usize>>,
}

impl Uploads {
    fn new(refuse: &[usize]) -> Arc<Self> {
        Arc::new(Uploads { puts: RefCell::new(Vec::new()), refuse: RefCell::new(refuse.iter().copied().collect()) })
    }
    fn puts(&self) -> Vec<Vec<u8>> {
        self.puts.borrow().clone()
    }
    fn count(&self) -> usize {
        self.puts.borrow().len()
    }
    fn refuse_at(&self, before: usize) {
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

fn up_fetch(up: &Arc<Uploads>) -> Option<Arc<dyn Fetch>> {
    Some(up.clone())
}

/// `prepareBuilder({ backend, setup: "true", fetch, import })` with the stages the test listens to set by the caller.
fn prep(rec: &Rec, up: &Arc<Uploads>, import: GoldenImport) -> PrepareBuilderOptions {
    let mut o = prepare_opts(rec, "true");
    o.fetch = up_fetch(up);
    o.import = Some(import);
    o
}

fn build(o: PrepareBuilderOptions) -> Builder {
    run(prepare_builder(o, &*Manual::new())).expect("the build prepares")
}

fn apply(b: &Builder, o: ApplyImportOptions) -> Applied {
    run(apply_golden_import(&b.machine, o, &*Manual::new())).expect("the import applies")
}

fn seal(b: Builder, o: SealGoldenOptions) -> SealResult {
    run(seal_golden(b, o, &*Manual::new())).expect("the golden seals")
}

fn daemon(reply: &'static str) -> DeployDaemon {
    Arc::new(move |_: Arc<dyn Machine>| async move { Ok::<Option<String>, EngineError>(Some(reply.to_string())) }.boxed_local())
}

fn pack_of(p: PackedFiles) -> PackFiles {
    Arc::new(move |_: Option<LeaveOut>| {
        let p = p.clone();
        async move { Ok::<PackedFiles, EngineError>(p) }.boxed_local()
    })
}

type Results = Rc<RefCell<Vec<ImportResult>>>;
type Outcomes = Rc<RefCell<Vec<(Vec<ContextResult>, Option<String>)>>>;

/// `onResult: r => void results.push(r)`.
fn collect_results() -> (Results, Arc<dyn Fn(&ImportResult)>) {
    let results: Results = Rc::default();
    let w = results.clone();
    (results, Arc::new(move |r: &ImportResult| w.borrow_mut().push(r.clone())))
}

/// `onContext: o => void outcomes.push(o)`, with `o` the `Pick<ImportResult, "context" | "contextFailure">`.
fn collect_outcomes() -> (Outcomes, OnContext) {
    let outcomes: Outcomes = Rc::default();
    let w = outcomes.clone();
    (outcomes, Arc::new(move |c: &[ContextResult], f: Option<&str>| w.borrow_mut().push((c.to_vec(), f.map(str::to_string)))))
}

fn ctx_none() -> (Vec<ContextResult>, Option<String>) {
    (Vec::new(), None)
}

fn applied_of(b: &Builder) -> Vec<ImportStage> {
    b.import.as_ref().expect("the builder carries a ledger").applied.clone()
}

fn ledger_of(b: &Builder) -> ImportLedger {
    b.import.clone().expect("the builder carries a ledger")
}

/// `stage` lines a test wants: an exact line, or one a regex matches (written as a hand-checked function).
enum Want {
    Is(&'static str),
    Re(fn(&str) -> bool),
}

fn assert_stages(got: &[String], want: &[Want]) {
    assert_eq!(got.len(), want.len(), "stages: {got:#?}");
    for (g, w) in got.iter().zip(want) {
        match w {
            Want::Is(s) => assert_eq!(g, s),
            Want::Re(f) => assert!(f(g), "{g:?} does not match"),
        }
    }
}

fn is_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())
}

/// `\d+(\.\d+)?`
fn is_number(s: &str) -> bool {
    match s.split_once('.') {
        None => is_digits(s),
        Some((w, f)) => is_digits(w) && is_digits(f),
    }
}

/// `\d+(\.\d)?s`
fn is_seconds(s: &str) -> bool {
    let Some(num) = s.strip_suffix('s') else { return false };
    match num.split_once('.') {
        None => is_digits(num),
        Some((w, f)) => is_digits(w) && f.len() == 1 && is_digits(f),
    }
}

/// `^<head>\d+(\.\d)?s<tail>$`
fn timed(line: &str, head: &str, tail: &str) -> bool {
    line.strip_prefix(head).and_then(|r| r.strip_suffix(tail)).is_some_and(is_seconds)
}

/// `^installing-mcp:machine context: \d+(\.\d+)? KB written for Claude Code, Codex; Pi by its fallback \(~\/\.pi\/agent\/APPEND_SYSTEM\.md already exists\)$`
fn pi_fallback_line(line: &str) -> bool {
    line.strip_prefix("installing-mcp:machine context: ")
        .and_then(|r| r.strip_suffix(" KB written for Claude Code, Codex; Pi by its fallback (~/.pi/agent/APPEND_SYSTEM.md already exists)"))
        .is_some_and(is_number)
}

/// `^write failed: .*HTTP 413`
fn write_failed_413(s: &str) -> bool {
    s.strip_prefix("write failed: ").and_then(|r| r.find("HTTP 413").map(|i| !r[..i].contains('\n'))).unwrap_or(false)
}

/// `^installing-mcp:machine context: not written \(write failed: .*HTTP 413`
fn not_written_413(line: &str) -> bool {
    line.strip_prefix("installing-mcp:machine context: not written (").is_some_and(write_failed_413)
}

/// TS `findIndex(c => c.includes(needle))`, -1 when none.
fn find_index(cmds: &[String], needle: &str) -> isize {
    cmds.iter().position(|c| c.contains(needle)).map_or(-1, |i| i as isize)
}

/// TS `indexOf(exact)`, -1 when none.
fn index_of(cmds: &[String], exact: &str) -> isize {
    cmds.iter().position(|c| c == exact).map_or(-1, |i| i as isize)
}

/// TS `find(c => c.includes(needle))!`.
fn found(cmds: &[String], needle: &str) -> String {
    cmds.iter().find(|c| c.contains(needle)).cloned().unwrap_or_else(|| panic!("no command has {needle:?}"))
}

/// `boundedCommand(seconds, cmd)` from machine-context.ts:197, copied as the TS writes it.
fn bounded(seconds: u32, cmd: &str) -> String {
    format!("set -m; {cmd} & p=$!; ( sleep {seconds}; kill -KILL -- -$p ) >/dev/null 2>&1 & k=$!; set +m; wait $p; r=$?; kill -KILL -- -$p -$k 2>/dev/null; [ $r -ne 137 ]")
}

/// `tally.replace(/ \([^()]*\)/g, "")`: every bracket with no bracket inside it, with the space before it, removed.
fn strip_brackets(s: &str) -> String {
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
fn norm_tools(v: &[ToolResult]) -> Vec<ToolResult> {
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

fn norm_agents(v: &[AgentResult]) -> Vec<AgentResult> {
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
fn done(id: &str, label: &str) -> ToolResult {
    ToolResult { id: id.into(), label: label.into(), outcome: "installed".into(), ms: Some(0), bytes: Some(0.0), ..Default::default() }
}

fn agent_done(id: &str, name: &str) -> AgentResult {
    AgentResult { id: id.into(), name: name.into(), outcome: "installed".into(), note: None, ms: Some(0) }
}

fn inline_of(b: &BackendFor) -> Vec<(String, String, Option<u64>)> {
    b.rec.state.inline.borrow().clone()
}

fn inline_cmds(b: &BackendFor) -> Vec<String> {
    inline_of(b).into_iter().map(|(_, c, _)| c).collect()
}

fn missing_check(b: &BackendFor) -> (String, String, Option<u64>) {
    inline_of(b).into_iter().filter(|(_, c, _)| c.contains("echo \"missing")).last().expect("a check of the tools PATH")
}

fn skipped_tool_raycast() -> SkippedRow {
    SkippedRow { id: "tools/brew-cask/raycast".into(), label: "Raycast".into(), note: "macOS app, no Linux build".into() }
}

fn gopls_and_spoo() -> Vec<ToolInstall> {
    vec![
        ToolInstall { id: "tools/go/gopls".into(), label: "gopls".into(), manager: "go".into(), cmd: "go install gopls".into(), bin: Some("gopls".into()), ..Default::default() },
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

fn tool_entry(id: &str, label: &str, default: &str, version: Option<&str>, linux: Option<&str>) -> RecipeEntry {
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

fn plan_installs(rows: &[RecipeEntry], table: &BrewTable) -> Vec<ToolInstall> {
    tool_installs_for_rows(rows, table, &[], TOOLS_PATH, None).expect("the plan").installs
}

const GH_WSP_ROAD_B: &str = "WSP_ROAD release gh_2.101.0_linux_amd64.tar.gz";

// golden.test.ts:1113
#[test]
fn writes_machine_context_after_the_stages_with_the_plans_skips_and_agent_hooks() {
    let probe = "WSP_CTX\nKERNEL 6.6.30\nDISK 20466256 11720704\nOVERLAY no\nAGENT claude\nAGENT codex\nAGENT pi\nCONFLICT pi\nSHELL zsh\nWSP_CTX_END\n";
    let b = backend_for(vec![("echo WSP_CTX", Answer::Fixed(exec(0, probe, "")))], Free::Kb(mb(3000)));
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { skipped_tools: vec![skipped_tool_raycast()], on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let res = res.borrow();
    assert_eq!(res[0].tools[0], ToolResult { id: "tools/brew-cask/raycast".into(), label: "Raycast".into(), outcome: "skipped".into(), note: Some("macOS app, no Linux build".into()), ..Default::default() });
    assert_eq!(
        res[0].context,
        Some(vec![
            ContextResult {
                agent: "claude".into(),
                outcome: ContextOutcomeKind::Written,
                path: Some("/etc/claude-code/CLAUDE.md".into()),
                note: None,
                skill: "/etc/claude-code/.claude/skills/wsp-machine/SKILL.md".into(),
            },
            ContextResult {
                agent: "codex".into(),
                outcome: ContextOutcomeKind::Written,
                path: Some("/etc/codex/requirements.toml".into()),
                note: None,
                skill: "/etc/codex/skills/wsp-machine/SKILL.md".into(),
            },
            ContextResult {
                agent: "pi".into(),
                outcome: ContextOutcomeKind::Fallback,
                path: Some("/root/.pi/agent/extensions/wsp-machine.ts".into()),
                note: Some("~/.pi/agent/APPEND_SYSTEM.md already exists".into()),
                skill: "/root/.pi/agent/skills/wsp-machine/SKILL.md".into(),
            },
        ])
    );
    assert!(res[0].context_failure.is_none());
    let all = st.all();
    assert!(all.len() >= 2 && pi_fallback_line(&all[all.len() - 2]), "stages: {all:#?}");
    // The texts go up as one archive after the person's files and are untarred at the root before the reach check.
    let cmds = b.cmds();
    let write = find_index(&cmds, "tar xzf - -C '/' ");
    assert!(write > find_index(&cmds, "echo WSP_CTX"));
    assert!(index_of(&cmds, "echo ok") > write);
    assert!(!cmds.iter().any(|c| c.contains("base64 --decode")));
    let puts = up.puts();
    assert_eq!(puts.len(), 2);
    let tgz = &puts[1];
    let skill = file_in(tgz, "etc/wsp/skills/wsp-machine/SKILL.md");
    assert!(skill.contains("- This machine is a golden builder, not a workspace yet."));
    assert!(skill.contains("- Golden: not sealed yet."));
    assert!(skill.contains("- Tools that did not install: Raycast (macOS app, no Linux build)."));
    assert!(names_in(tgz).contains(&"etc/wsp/machine-context.json".to_string()));
    assert!(!names_in(tgz).iter().any(|n| n.ends_with("APPEND_SYSTEM.md")));
}

// golden.test.ts:1143
#[test]
fn a_refused_context_upload_is_a_failure_the_result_counts_not_the_end_of_the_build() {
    let b = backend_default();
    // The person's files go up first and land; the context archive is the second PUT.
    let up = Uploads::new(&[1]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    let failure = "write failed: vault import upload failed: HTTP 413 Payload Too Large; the upload takes at most 1024 bytes per PUT";
    assert!(st.all().contains(&format!("installing-mcp:machine context: not written ({failure})")));
    let res = res.borrow();
    assert_eq!(res[0].context, Some(Vec::new()));
    assert_eq!(res[0].context_failure.as_deref(), Some(failure));
    // The stage is applied only once the context landed, so a later attach runs the write again.
    assert!(!applied_of(&builder).contains(&ImportStage::InstallingMcp));
    assert_eq!(b.cmds().last().map(String::as_str), Some("echo ok"));
}

// golden.test.ts:1163
#[test]
fn a_context_write_that_failed_on_the_build_runs_again_on_attach_and_marks_mcp_once_it_landed() {
    let b = backend_default();
    // The person's files are the first PUT; the build's context archive is the second and is refused, the attach's goes through.
    let up = Uploads::new(&[1]);
    let builder = build(prep(&b.rec, &up, import_of()));
    assert_eq!(applied_of(&builder), vec![ImportStage::ApplyingSetup, ImportStage::UploadingFiles, ImportStage::InstallingHarness, ImportStage::InstallingTools]);
    let before_cmds = b.cmds().len();
    let before_puts = up.count();
    let st = stage_recorder();
    let again = apply(
        &builder,
        ApplyImportOptions { import: Some(import_of()), setup: "true".into(), ledger: builder.import.clone(), fetch: up_fetch(&up), on_stage: Some(st.on_stage.clone()), ..Default::default() },
    );
    assert_stages(
        &st.all(),
        &[
            Want::Is("applying-setup:already applied"),
            Want::Is("uploading-files:already applied"),
            Want::Is("installing-harness:already applied"),
            Want::Is("installing-tools:already applied"),
            Want::Is("installing-mcp:none configured"),
            Want::Re(is_context_written),
        ],
    );
    assert_eq!(up.count(), before_puts + 1);
    assert!(names_in(&up.puts().last().unwrap().clone()).contains(&"etc/wsp/machine-context.json".to_string()));
    let cmds = b.cmds();
    let shape: Vec<&str> = cmds[before_cmds..]
        .iter()
        .map(|c| if c.contains("echo WSP_CTX") { "probe" } else if c.contains("tar xzf - -C '/' ") { "write" } else { c.as_str() })
        .collect();
    assert_eq!(shape, vec!["probe", "write"]);
    assert_eq!(again.ledger.applied, vec![ImportStage::ApplyingSetup, ImportStage::UploadingFiles, ImportStage::InstallingHarness, ImportStage::InstallingTools, ImportStage::InstallingMcp]);
    // With the context on the machine, the next attach runs nothing.
    let third = stage_recorder();
    apply(
        &builder,
        ApplyImportOptions { import: Some(import_of()), setup: "true".into(), ledger: Some(again.ledger.clone()), fetch: up_fetch(&up), on_stage: Some(third.on_stage.clone()), ..Default::default() },
    );
    assert_eq!(third.all().last().map(String::as_str), Some("installing-mcp:already applied"));
    assert_eq!(up.count(), before_puts + 1);

    // A retry that fails again leaves the stage unmarked, so the attach after it tries once more.
    up.refuse_at(up.count() + 1);
    up.refuse_at(up.count() + 2);
    let other = build(prep(&b.rec, &up, import_of()));
    let rec = stage_recorder();
    let still = apply(
        &other,
        ApplyImportOptions { import: Some(import_of()), setup: "true".into(), ledger: other.import.clone(), fetch: up_fetch(&up), on_stage: Some(rec.on_stage.clone()), ..Default::default() },
    );
    assert!(rec.all().last().is_some_and(|l| not_written_413(l)), "stages: {:#?}", rec.all());
    assert!(!still.ledger.applied.contains(&ImportStage::InstallingMcp));
}

// golden.test.ts:1204
#[test]
fn a_retried_context_write_names_what_the_build_left_off_and_hands_its_outcome_over() {
    let b = backend_default();
    let up = Uploads::new(&[1]);
    let skipped_tools = vec![skipped_tool_raycast()];
    let skipped_agents = vec![SkippedRow { id: "agents/zed".into(), label: "Zed".into(), note: "no installer known".into() }];
    let builder = build(prep(&b.rec, &up, GoldenImport { skipped_tools: skipped_tools.clone(), skipped_agents: skipped_agents.clone(), ..import_of() }));
    assert_eq!(
        ledger_of(&builder).missing_tools,
        Some(vec![GoldenMissingTool {
            id: "tools/brew-cask/raycast".into(),
            name: "Raycast".into(),
            outcome: GoldenMissingToolOutcome::Skipped,
            note: "macOS app, no Linux build".into(),
        }])
    );
    let (res, on_result) = collect_results();
    let (outs, on_context) = collect_outcomes();
    let plan = || GoldenImport {
        skipped_tools: skipped_tools.clone(),
        skipped_agents: skipped_agents.clone(),
        on_result: Some(on_result.clone()),
        on_context: Some(on_context.clone()),
        ..import_of()
    };
    let again = apply(&builder, ApplyImportOptions { import: Some(plan()), setup: "true".into(), ledger: builder.import.clone(), fetch: up_fetch(&up), ..Default::default() });
    // The skipped stages ran on the build: the ledger's tools, the plan's agents and files are the facts the guest gets.
    let tgz = up.puts().last().unwrap().clone();
    let context: serde_json::Value = serde_json::from_str(&file_in(&tgz, "etc/wsp/machine-context.json")).expect("the context is JSON");
    assert_eq!(
        context,
        serde_json::json!({
            "tools": [{ "id": "tools/brew-cask/raycast", "label": "Raycast", "note": "macOS app, no Linux build" }],
            "agents": [{ "id": "agents/zed", "label": "Zed", "note": "no installer known" }],
            "files": [{ "path": "~/.bashrc", "note": "no longer on this computer" }],
        })
    );
    let skill = file_in(&tgz, "etc/wsp/skills/wsp-machine/SKILL.md");
    assert!(skill.contains("- Tools that did not install: Raycast (macOS app, no Linux build)."));
    assert!(skill.contains("- Agents that did not install: Zed (no installer known)."));
    // The attach reports no result, so the saved one from the build stands; the write's outcome alone goes out.
    assert!(res.borrow().is_empty());
    assert_eq!(*outs.borrow(), vec![ctx_none()]);
    assert!(again.result.tools.is_empty());
    assert!(again.ledger.applied.contains(&ImportStage::InstallingMcp));

    // A retry that fails again hands the new reason over the same way.
    up.refuse_at(up.count() + 1);
    up.refuse_at(up.count() + 2);
    let other = build(prep(&b.rec, &up, GoldenImport { skipped_tools: skipped_tools.clone(), skipped_agents: skipped_agents.clone(), ..import_of() }));
    apply(&other, ApplyImportOptions { import: Some(plan()), setup: "true".into(), ledger: other.import.clone(), fetch: up_fetch(&up), ..Default::default() });
    assert!(res.borrow().is_empty());
    let outs = outs.borrow();
    assert_eq!(outs.len(), 2);
    assert!(outs[1].0.is_empty() && outs[1].1.as_deref().is_some_and(write_failed_413), "{:?}", outs[1]);
}

// golden.test.ts:1245
#[test]
fn a_quiet_attach_whose_mcp_rewrite_is_refused_hands_nothing_over() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let mcp = McpPlan { agents: Vec::new(), guest_home: "/root".into(), rewrites: Vec::new(), bin_dirs: Vec::new(), tools: Vec::new() };
    let builder = build(prep(&b.rec, &up, GoldenImport { mcp: Some(mcp.clone()), ..import_of() }));
    assert!(applied_of(&builder).contains(&ImportStage::InstallingMcp));
    let (outs, on_context) = collect_outcomes();
    // The MCP plan rewrites the context on every attach; this one is refused.
    up.refuse_at(up.count());
    let st = stage_recorder();
    let again = apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { mcp: Some(mcp.clone()), on_context: Some(on_context.clone()), ..import_of() }),
            setup: "true".into(),
            ledger: builder.import.clone(),
            fetch: up_fetch(&up),
            on_stage: Some(st.on_stage.clone()),
            ..Default::default()
        },
    );
    assert!(st.all().last().is_some_and(|l| not_written_413(l)), "stages: {:#?}", st.all());
    assert!(again.ledger.applied.contains(&ImportStage::InstallingMcp));
    assert!(outs.borrow().is_empty());

    // A rewrite that lands is handed over as before: the same outcome the build saved.
    apply(
        &builder,
        ApplyImportOptions {
            import: Some(GoldenImport { mcp: Some(mcp.clone()), on_context: Some(on_context.clone()), ..import_of() }),
            setup: "true".into(),
            ledger: Some(again.ledger.clone()),
            fetch: up_fetch(&up),
            ..Default::default()
        },
    );
    assert_eq!(*outs.borrow(), vec![ctx_none()]);
}

// golden.test.ts:1270
#[test]
fn an_archive_over_one_upload_part_says_how_many_parts_it_went_up_in() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let mut files = import_of().files.expect("the files");
    files.pack = pack_of(PackedFiles { tar: vec![0u8; 33 * 1024 * 1024], bytes: 33 * 1024 * 1024, unpacked: 4096, ..Default::default() });
    let st = stage_recorder();
    let mut o = prep(&b.rec, &up, GoldenImport { files: Some(files), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert_eq!(up.count(), 3);
    let all = st.all();
    assert!(all.contains(&"uploading-files:part 1 of 2, 32 MB of 33 MB".to_string()));
    assert!(all.contains(&"uploading-files:part 2 of 2, 33 MB of 33 MB".to_string()));
    assert!(all.iter().any(|l| timed(l, "uploading-files:33 MB in 2 parts in ", "; 2.9 GB free")), "stages: {all:#?}");
}

// golden.test.ts:1281
#[test]
fn runs_setup_upload_agents_then_tools_in_order_with_a_detail_and_checks_the_machine_answers() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.deploy_daemon = Some(daemon("node v22"));
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert_stages(
        &sans_base(&st.all()),
        &[
            Want::Is("creating:sandbox from base"),
            Want::Is("deploying-daemon"),
            Want::Is("deploying-daemon:node v22; 2.9 GB free"),
            Want::Is("applying-setup:3 files: identity 1, shell 2"),
            Want::Is("applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)"),
            Want::Is("uploading-files:1 KB"),
            Want::Re(|l| timed(l, "uploading-files:1 KB in ", "; 2.9 GB free")),
            Want::Is("installing-harness"),
            Want::Is("installing-harness:Claude Code (1/2)"),
            Want::Is("installing-harness:Codex (2/2)"),
            Want::Is("installing-harness:Claude Code, Codex installed; caches swept; 2.9 GB free"),
            Want::Is("installing-tools:Homebrew (1/3)"),
            Want::Is("installing-tools:gh (2/3)"),
            Want::Is("installing-tools:bun@1.4.0 (3/3)"),
            Want::Is("installing-tools:3 installed; caches swept; 2.9 GB free"),
            Want::Is("installing-mcp:none configured"),
            Want::Re(is_context_written),
            Want::Is("ready"),
        ],
    );
    let puts = up.puts();
    assert_eq!(puts[0], b"tgz-bytes".to_vec());
    assert_eq!(puts.len(), 2);
    let cmds = b.cmds();
    let untar = found(&cmds, "tar xzf");
    assert!(untar.contains("-C '/root' --no-same-owner"));
    let last_untar = cmds.iter().filter(|c| c.contains("tar xzf")).last().unwrap();
    assert!(last_untar.contains("-C '/' --no-same-owner"));
    assert!(index_of(&cmds, FREE_KB_CMD) < index_of(&cmds, &untar));
    let tool = found(&cmds, "brew install gh");
    assert!(tool.contains("\nsetsid bash -c 'brew install gh' &\np=$!\n"));
    assert!(tool.contains("while [ $t -lt 600 ]"));
    assert_eq!(cmds.iter().filter(|c| c.contains("brew install gh") || c.contains("brew-bootstrap") || c.contains("bun@1.4.0")).count(), 3);
    let agent = found(&cmds, "codex-install");
    assert!(agent.contains("\nsetsid bash -c 'set -euo pipefail\nexport npm_config_fetch_timeout="));
    assert!(agent.contains(&format!("{}\nexport PATH=\"/usr/local/bin:$PATH\"\ncodex-install", CURL_NET.as_str())));
    assert!(agent.contains("while [ $t -lt 900 ]"));
    // Agents and their checks run with the Node the golden installed ahead of any the image shipped.
    assert!(cmds.contains(&"export PATH=\"/usr/local/bin:$PATH\"\nclaude --version".to_string()));
    assert!(cmds.contains(&"export PATH=\"/usr/local/bin:$PATH\"\ncodex --version".to_string()));
    assert!(index_of(&cmds, "true") < index_of(&cmds, &agent));
    // Agents are the point and tools the long tail: the agents go on first, so a tools stage that fills the disk cannot starve them.
    assert!(index_of(&cmds, &agent) < index_of(&cmds, &tool));
    // The reach check is the last thing on the machine before the hand-off.
    assert_eq!(cmds.last().map(String::as_str), Some("echo ok"));
    assert_eq!(
        builder.import,
        Some(ImportLedger {
            recipe_hash: "h1".into(),
            applied: vec![ImportStage::ApplyingSetup, ImportStage::UploadingFiles, ImportStage::InstallingHarness, ImportStage::InstallingTools, ImportStage::InstallingMcp],
            smoke: "claude --version && codex --version && bun --version".into(),
            recipe: None,
            missing_tools: None,
            silenced: None,
            shell_noise: None,
            left_behind: None,
        })
    );
    assert_eq!(builder.setup_sha, hex::encode(Sha256::digest(b"true\nclaude-install\ncodex-install")));
    let res = res.borrow();
    assert_eq!(res.len(), 1);
    let r = &res[0];
    assert_eq!(r.recipe_hash, "h1");
    assert!(r.base.is_some());
    assert_eq!(
        r.files,
        Some(ResultFiles {
            bytes: 1200,
            skipped: vec![SkippedPath { id: "shell/bashrc".into(), path: "~/.bashrc".into(), note: "no longer on this computer".into() }],
            cut: Some(Vec::new()),
            blanked: None,
        })
    );
    assert_eq!(r.homebrew, Some(HomebrewCheckout { tag: "6.0.21".into(), commit: "560147012b9678b42ef5e83b690f0895552d1366".into() }));
    assert_eq!(
        norm_tools(&r.tools),
        vec![done("tools/homebrew", "Homebrew"), done("tools/brew/gh", "gh"), done("tools/npm/bun", "bun@1.4.0")]
    );
    assert_eq!(norm_agents(&r.agents), vec![agent_done("agents/claude", "Claude Code"), agent_done("agents/codex", "Codex")]);
    assert_eq!(r.context, Some(Vec::new()));
    assert!(r.mcp.is_none() && r.retired.is_none() && r.context_failure.is_none());
}

// golden.test.ts:1347
#[test]
fn every_command_that_outlives_one_exec_goes_through_run_and_inline_execs_are_bounded() {
    let locked_once = Rc::new(std::cell::Cell::new(false));
    let l = locked_once.clone();
    let b = backend_for(
        vec![
            (
                "brew install gh",
                Answer::Each(Rc::new(move || {
                    if l.get() {
                        return ok();
                    }
                    l.set(true);
                    exec(1, "", "Error: A `brew install glibc` process has already locked /home/linuxbrew/.linuxbrew/Cellar/x.\n")
                })),
            ),
            ("NODE_HAVE", Answer::Fixed(exec(0, "NODE_HAVE v18\nNODE_INSTALLED v22.0.0\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let up = Uploads::new(&[]);
    let shell = ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() };
    let node = NodeInstall { floor: 22, version: "v22.0.0".into(), agents: vec!["Codex".into()], cmd: "echo NODE_HAVE v$(node -v); node-install".into() };
    let mut o = prep(&b.rec, &up, GoldenImport { shell: Some(shell), node: Some(node), ..import_of() });
    o.setup = "the-setup".into();
    o.deploy_daemon = Some(daemon("node v22"));
    let builder = build(o);
    seal(builder, seal_opts(&b.rec, "unused"));
    let scripts = b.rec.state.ran_on("m1");
    let one_of = |needle: &str| -> String {
        let hits: Vec<&String> = scripts.iter().filter(|s| s.contains(needle)).collect();
        assert_eq!(hits.len(), 1, "{needle}");
        hits[0].clone()
    };
    for body in ["the-setup", "install-zsh", "node-install", "claude-install", "codex-install", "brew-bootstrap", "bun@1.4.0", "autoremove", "cleanup -s --prune=all"] {
        assert!(one_of(body).contains("\nsetsid bash -c '"), "{body}");
    }
    assert_eq!(scripts.iter().filter(|s| s.contains("brew install gh")).count(), 2);
    assert!(one_of("flock -w 600").contains("/home/linuxbrew/.linuxbrew/var/homebrew/locks/*.lock"));
    assert_eq!(scripts.iter().filter(|s| s.contains("tar xzf")).count(), 2);
    assert!(scripts.contains(&"export PATH=\"/usr/local/bin:$PATH\"\nclaude --version".to_string()));
    assert_eq!(b.rec.state.ran_on("m2"), vec!["claude --version && codex --version && bun --version".to_string()]);
    let inline_cmds: Vec<String> = inline_of(&b).into_iter().map(|(_, c, _)| c).collect();
    assert!(inline_cmds.contains(&FREE_KB_CMD.to_string()));
    assert!(inline_cmds.contains(&"rm -f /tmp/wsp-vault-*.tgz".to_string()));
    assert!(inline_cmds.contains(&"echo ok".to_string()));
    assert!(inline_cmds.contains(&"test -x /usr/local/bin/wsp-open".to_string()));
    for (_, cmd, timeout) in inline_of(&b) {
        assert!(timeout.is_some_and(|t| t <= 20_000), "{cmd}: {timeout:?}");
        // The base floor's versions read is a --version call per row, inline and bounded like the rest.
        if cmd.contains("echo \"VERSION curl:") {
            continue;
        }
        assert!(!(cmd.contains("setsid bash -c") || cmd.contains("tar ") || cmd.contains("the-setup") || cmd.contains("--version")), "a long command ran inline: {cmd}");
    }
}

// golden.test.ts:1388
#[test]
fn the_lines_a_run_writes_stream_into_its_stage_frames_named_for_the_tool_or_agent() {
    let rec = recording_backend(Opts {
        stream: true,
        exec: Some(Box::new(|cmd: &str| {
            if cmd.contains("brew install gh") {
                return Some(exec(0, "==> Fetching gh\n==> Pouring gh\n", ""));
            }
            if cmd.contains("claude-install") {
                return Some(exec(0, "claude 1.2.3 installed\n", ""));
            }
            if cmd == FREE_KB_CMD {
                return Some(exec(0, &format!("{}\n", mb(3000)), ""));
            }
            if cmd == "echo ok" {
                return Some(reach_ok());
            }
            Some(ok())
        })),
        ..Opts::default()
    });
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let mut o = prep(&rec, &up, import_of());
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let all = st.all();
    let at = all.iter().position(|s| s == "installing-tools:gh (2/3)").expect("the gh frame");
    assert_eq!(
        all[at..at + 3].to_vec(),
        vec!["installing-tools:gh (2/3)".to_string(), "installing-tools:gh: ==> Fetching gh".to_string(), "installing-tools:gh: ==> Pouring gh".to_string()]
    );
    assert!(all.contains(&"installing-harness:Claude Code: claude 1.2.3 installed".to_string()));
}

// golden.test.ts:1408
#[test]
fn the_shell_step_runs_after_the_pack_and_before_the_files_land_guarded_on_the_guest() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let shell = ShellInstall {
        shell: LoginShell::Zsh,
        frameworks: vec!["shell/oh-my-zsh".into(), "shell/antidote".into()],
        cmd: "set -euo pipefail\ninstall-zsh-and-frameworks".into(),
    };
    let mut o = prep(&b.rec, &up, GoldenImport { shell: Some(shell), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let step = found(&cmds, "install-zsh-and-frameworks");
    assert!(step.contains("\nsetsid bash -c 'set -euo pipefail\ninstall-zsh-and-frameworks' &\np=$!\n"));
    assert!(step.contains("while [ $t -lt 300 ]"));
    let untar = found(&cmds, "tar xzf");
    assert!(index_of(&cmds, &step) < index_of(&cmds, &untar));
    assert!(index_of(&cmds, &step) < index_of(&cmds, &found(&cmds, "claude-install")));
    let all = st.all();
    let from = all.iter().position(|s| s == "applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)").expect("the pack frame");
    let to = all.iter().position(|s| s == "uploading-files:1 KB").expect("the upload frame") + 1;
    assert_eq!(
        all[from..to].to_vec(),
        vec![
            "applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)".to_string(),
            "applying-setup:zsh: installing, with shell/oh-my-zsh, shell/antidote".to_string(),
            "applying-setup:zsh installed as the login shell; shell/oh-my-zsh, shell/antidote reinstalled; 2.9 GB free".to_string(),
            "uploading-files:1 KB".to_string(),
        ]
    );
}

// golden.test.ts:1427
#[test]
fn a_shell_step_that_fails_is_a_warning_with_its_reason_and_the_files_still_land() {
    let b = backend_for(vec![("install-zsh", Answer::Fixed(exec(100, "", "E: Unable to locate package zsh\n")))], Free::Kb(mb(3000)));
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let shell = ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() };
    let mut o = prep(&b.rec, &up, GoldenImport { shell: Some(shell), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert!(st.all().contains(&"applying-setup:zsh step failed, chsh skipped: E: Unable to locate package zsh".to_string()));
    assert!(b.cmds().iter().any(|c| c.contains("tar xzf")));
    assert_eq!(st.all().last().map(String::as_str), Some("ready"));
    assert!(applied_of(&builder).contains(&ImportStage::UploadingFiles));
}

// golden.test.ts:1437
#[test]
fn login_shell_check_runs_after_the_installs_and_records_its_noise_or_none() {
    let noisy = backend_for(
        vec![("zsh -lic true", Answer::Fixed(exec(0, "", "zsh: command not found: starship\nzsh eza plugin: eza not found.\n")))],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let shell = ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() };
    let up = Uploads::new(&[]);
    let mut o = prep(&noisy.rec, &up, GoldenImport { shell: Some(shell.clone()), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    let cmds = noisy.cmds();
    let at = |needle: &str| -> isize {
        let i = find_index(&cmds, needle);
        assert!(i >= 0, "{needle}");
        i
    };
    let check = cmds[at("zsh -lic true") as usize].clone();
    // A login shell like the app's terminal: profile.d puts the tools on PATH before the rc files are read.
    assert!(check.contains("TERM=xterm-256color zsh -lic true </dev/null"));
    // A job the rc file leaves holding the output open ends with the check rather than holding the exec to its deadline.
    assert!(check.contains(&format!("{} || exit 124", bounded(60, "TERM=xterm-256color zsh -lic true </dev/null"))));
    assert!(check.contains("\nsetsid bash -c '"));
    // A tool or an agent the rc calls is on the machine by then: the check runs after the last install of each stage.
    for before in ["tar xzf", "claude-install", "codex-install", "brew install gh", "bun@1.4.0", "autoremove"] {
        assert!(at("zsh -lic true") > at(before), "{before}");
    }
    assert!(st.all().iter().any(|l| l.starts_with("installing-mcp:shell noise: zsh: command not found: starship, 2 lines; machine context:")));
    assert_eq!(ledger_of(&builder).shell_noise.as_deref(), Some("zsh: command not found: starship, 2 lines"));
    let sealed = seal(builder, seal_opts(&noisy.rec, "true"));
    assert_eq!(sealed.version.shell_noise.as_deref(), Some("zsh: command not found: starship, 2 lines"));

    let quiet = backend_default();
    let q = stage_recorder();
    let mut qo = prep(&quiet.rec, &Uploads::new(&[]), GoldenImport { shell: Some(shell.clone()), ..import_of() });
    qo.on_stage = Some(q.on_stage.clone());
    let qb = build(qo);
    assert!(q.all().iter().any(|l| l.starts_with("installing-mcp:zsh starts quiet; machine context:")));
    let hung = backend_for(vec![("zsh -lic true", Answer::Fixed(exec(124, "", "")))], Free::Kb(mb(3000)));
    let h = build(prep(&hung.rec, &Uploads::new(&[]), GoldenImport { shell: Some(shell), ..import_of() }));
    assert_eq!(ledger_of(&h).shell_noise.as_deref(), Some("timed out after 60s"));
    assert!(ledger_of(&qb).shell_noise.is_none());
    let qs = seal(qb, seal_opts(&quiet.rec, "true"));
    assert!(qs.version.shell_noise.is_none());
    let none = backend_default();
    build(prep(&none.rec, &Uploads::new(&[]), import_of()));
    assert!(!none.cmds().iter().any(|c| c.contains("-lic true")));
}

// golden.test.ts:1473
#[test]
fn a_builder_that_already_carries_the_files_does_not_run_the_shell_step_again() {
    let b = backend_default();
    let up = Uploads::new(&[]);
    let imp = GoldenImport { shell: Some(ShellInstall { shell: LoginShell::Zsh, frameworks: Vec::new(), cmd: "install-zsh".into() }), ..import_of() };
    let builder = build(prep(&b.rec, &up, imp.clone()));
    assert_eq!(b.cmds().iter().filter(|c| c.contains("install-zsh")).count(), 1);
    apply(&builder, ApplyImportOptions { import: Some(imp), setup: "true".into(), ledger: builder.import.clone(), fetch: up_fetch(&up), ..Default::default() });
    assert_eq!(b.cmds().iter().filter(|c| c.contains("install-zsh")).count(), 1);
}

// golden.test.ts:1482
#[test]
fn a_failing_tool_is_a_warning_in_the_detail_and_the_next_one_still_runs() {
    // stdout ends on a progress line; the reason is the last stderr line, not that.
    let b = backend_for(
        vec![("brew install gh", Answer::Fixed(exec(1, "==> Installing gh dependency: oniguruma\n", "Error: gh: no bottle available!\n")))],
        Free::Kb(mb(3000)),
    );
    let up = Uploads::new(&[]);
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &up, GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let builder = build(o);
    assert!(st.all().contains(&"installing-tools:2 installed, 1 failed: gh (Error: gh: no bottle available!); caches swept; 2.9 GB free".to_string()));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[1..2]),
        vec![ToolResult { id: "tools/brew/gh".into(), label: "gh".into(), outcome: "failed".into(), note: Some("Error: gh: no bottle available!".into()), ms: Some(0), ..Default::default() }]
    );
    let version = seal(builder, seal_opts(&b.rec, "should-not-run")).version;
    assert_eq!(version.smoke.cmd, "claude --version && codex --version && bun --version");
}

// golden.test.ts:1496
#[test]
fn a_custom_row_that_fails_is_listed_with_its_reason_and_the_golden_still_seals() {
    let custom = custom_installs_for(
        &[RecipeCustomRow {
            kind: CustomLit,
            id: "just".into(),
            name: "just".into(),
            install: vec!["brew install just".into()],
            check: "command -v just".into(),
            manager: None,
            size: None,
            why: "added by the agent".into(),
        }],
        &|_: &RecipeCustomRow| None::<String>,
        TOOLS_PATH,
        None,
    );
    // The needle is the line the row installs with: the prelude around it is quoted again by the guard.
    let b = backend_for(vec![("brew install just", Answer::Fixed(exec(1, "", "Error: no formula just\n")))], Free::Kb(mb(3000)));
    let up = Uploads::new(&[]);
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(custom);
    let builder = build(prep(&b.rec, &up, GoldenImport { tools, on_result: Some(on_result), ..import_of() }));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[res[0].tools.len() - 1..]),
        vec![ToolResult { id: "tools/custom/just".into(), label: "just".into(), outcome: "failed".into(), note: Some("Error: no formula just".into()), ms: Some(0), ..Default::default() }]
    );
    let version = seal(builder, seal_opts(&b.rec, "should-not-run")).version;
    assert_eq!(version.version.0, 1.0);
}

// golden.test.ts:1507
#[test]
fn a_road_install_names_the_road_it_took_in_the_result_and_the_stage_summary() {
    let b = backend_for(
        vec![
            ("releases/tags/v0.1.0", Answer::Fixed(exec(0, &format!("WSP_ROAD release diskbloom_0.1.0_linux_arm64.tar.gz {} v0.1.0 aarch64\n", "a".repeat(64)), ""))),
            ("releases/tags/v1.13.1", Answer::Fixed(exec(0, "go: downloading\nWSP_ROAD go github.com/TheZoraiz/ascii-image-converter@v1.13.1\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let roads = vec![
        ToolInstall {
            id: "tools/brew/zingzy/tap/diskbloom".into(),
            label: "diskbloom".into(),
            manager: "release".into(),
            cmd: "curl https://api.github.com/repos/Zingzy/diskbloom/releases/tags/v0.1.0".into(),
            pin: Some(PinRead { read: None, fixed: true, words: "from its release".into() }),
            ..Default::default()
        },
        ToolInstall {
            id: "tools/brew/thezoraiz/ascii-image-converter/ascii-image-converter".into(),
            label: "ascii-image-converter".into(),
            manager: "release".into(),
            cmd: "curl https://api.github.com/repos/TheZoraiz/ascii-image-converter/releases/tags/v1.13.1".into(),
            ..Default::default()
        },
    ];
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(roads.clone());
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(st.all().contains(&"installing-tools:5 installed (diskbloom from its release, ascii-image-converter with go install); caches swept; 2.9 GB free".to_string()));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[3..]),
        vec![
            // The sum is recorded with the arch whose file it was, so a copy on the other arch never holds its own file to it.
            ToolResult {
                id: roads[0].id.clone(),
                label: "diskbloom".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad {
                    kind: "release".into(),
                    from: "diskbloom_0.1.0_linux_arm64.tar.gz".into(),
                    sha256: Some("a".repeat(64)),
                    tag: Some("v0.1.0".into()),
                    arch: Some("aarch64".into()),
                }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
            ToolResult {
                id: roads[1].id.clone(),
                label: "ascii-image-converter".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad { kind: "go".into(), from: "github.com/TheZoraiz/ascii-image-converter@v1.13.1".into(), ..Default::default() }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
        ]
    );
    // A brew install carries no road: it took the one its plan named.
    assert!(res[0].tools[1].road.is_none());
}

// golden.test.ts:1529
#[test]
fn a_catalog_tool_without_a_local_row_installs_by_its_catalog_road_on_the_guest() {
    let b = backend_for(
        vec![("cli/cli/releases/download/v2.101.0", Answer::Fixed(exec(0, &format!("{GH_WSP_ROAD_B} {} v2.101.0\n", "b".repeat(64)), "")))],
        Free::Kb(mb(3000)),
    );
    let plan = vec![
        RecipeEntry { paths: Vec::new(), default: "skip".into(), ..tool_entry("tools/catalog/gh", "GitHub CLI", "skip", None, Some("yes")) },
        tool_entry("tools/npm/wrangler", "wrangler", "bring", Some("4.1.0"), None),
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let road = found(&cmds, "cli/cli/releases/download/v2.101.0");
    assert!(road.contains("install -m 0755 \"$bin\" \"/usr/local/bin/$name\""));
    // The sum the catalog pinned is checked on the guest before anything the download carries is unpacked.
    assert!(road.contains("sha256sum -c -"));
    assert!(!road.contains("api.github.com"));
    assert!(missing_check(&b).1.contains("for b in 'node' 'gh'; do"));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools),
        vec![
            done("tools/manager/npm", "Node 22 with npm"),
            done("tools/npm/wrangler", "wrangler"),
            ToolResult {
                id: "tools/catalog/gh".into(),
                label: "GitHub CLI".into(),
                outcome: "installed".into(),
                note: Some("by an unmeasured road".into()),
                road: Some(ToolRoad { kind: "release".into(), from: "gh_2.101.0_linux_amd64.tar.gz".into(), sha256: Some("b".repeat(64)), tag: Some("v2.101.0".into()), arch: None }),
                ms: Some(0),
                bytes: Some(0.0),
            },
        ]
    );
    assert!(st.all().contains(&"installing-tools:3 installed (GitHub CLI from its release (by an unmeasured road)); caches swept; 2.9 GB free".to_string()));
}

// golden.test.ts:1554
#[test]
fn a_catalog_go_row_beside_a_go_row_installs_go_once_and_counts_each_install_once() {
    let b = backend_default();
    let plan = vec![
        tool_entry("tools/catalog/go", "Go", "skip", None, Some("yes")),
        RecipeEntry { paths: vec!["golang.org/x/tools/gopls@v0.16.2".into()], ..tool_entry("tools/go/gopls", "gopls", "bring", None, None) },
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert_eq!(cmds.iter().filter(|c| c.contains("brew install go'")).count(), 1);
    assert!(index_of(&cmds, &found(&cmds, "brew install go'")) < index_of(&cmds, &found(&cmds, "go install golang.org/x/tools/gopls@v0.16.2")));
    let res = res.borrow();
    let pairs: Vec<(String, String)> = res[0].tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    let want: Vec<(String, String)> = [
        ("tools/homebrew", "installed"),
        ("tools/brew-toolchain/glibc", "installed"),
        ("tools/brew-toolchain/gcc", "installed"),
        ("tools/catalog/go", "installed"),
        ("tools/go/gopls", "installed"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect();
    assert_eq!(pairs, want);
    assert!(st.all().contains(&"installing-tools:5 installed; caches swept; 2.9 GB free".to_string()));
}

// golden.test.ts:1571
#[test]
fn a_catalog_row_with_a_version_pins_where_the_road_can_and_says_what_it_installed() {
    let b = backend_default();
    let plan = vec![
        tool_entry("tools/catalog/wrangler", "Cloudflare Wrangler", "skip", Some("4.1.0"), Some("yes")),
        tool_entry("tools/catalog/tmux", "tmux", "skip", Some("3.5a"), Some("yes")),
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    assert!(b.cmds().iter().any(|c| c.contains("npm install -g wrangler@4.1.0")));
    let tmux_note = "by an unmeasured road; 3.5a asked, installed by apt at its current version";
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools),
        vec![
            done("tools/manager/npm", "Node 22 with npm"),
            ToolResult { note: Some("by an unmeasured road".into()), ..done("tools/catalog/wrangler", "Cloudflare Wrangler") },
            done("tools/apt-index", "apt index"),
            ToolResult { note: Some(tmux_note.into()), ..done("tools/catalog/tmux", "tmux") },
        ]
    );
    assert!(st.all().contains(&format!("installing-tools:4 installed (Cloudflare Wrangler (by an unmeasured road), tmux ({tmux_note})); caches swept; 2.9 GB free")));
}

// golden.test.ts:1591
#[test]
fn the_tally_names_every_install_once_with_its_notes_inside_their_brackets() {
    let b = backend_for(
        vec![("cli/cli/releases/download/v2.101.0", Answer::Fixed(exec(0, &format!("{GH_WSP_ROAD_B} {} v2.101.0\n", "c".repeat(64)), "")))],
        Free::Kb(mb(3000)),
    );
    let plan = vec![
        tool_entry("tools/catalog/gh", "GitHub CLI", "skip", None, Some("yes")),
        tool_entry("tools/catalog/tmux", "tmux", "skip", Some("3.5a"), Some("yes")),
    ];
    let installs = plan_installs(&plan, &BrewTable::new());
    let st = stage_recorder();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let tmux_note = "by an unmeasured road; 3.5a asked, installed by apt at its current version";
    let all = st.all();
    let tally = all.iter().find(|s| s.starts_with("installing-tools:3 installed")).expect("the tally");
    assert_eq!(tally, &format!("installing-tools:3 installed (GitHub CLI from its release (by an unmeasured road), tmux ({tmux_note})); caches swept; 2.9 GB free"));
    // With the bracketed notes off, every comma left separates two named tools: the list names two, not four.
    assert_eq!(strip_brackets(tally), "installing-tools:3 installed (GitHub CLI from its release, tmux); caches swept; 2.9 GB free");
}

// golden.test.ts:1608
#[test]
fn after_the_loop_every_install_that_names_its_command_is_checked_on_the_tools_path_and_a_miss_fails() {
    let b = backend_for(
        vec![
            ("releases/tags/v0.4.1", Answer::Fixed(exec(0, &format!("WSP_ROAD release spoo_0.4.1_linux_amd64.tar.gz {} v0.4.1\n", "a".repeat(64)), ""))),
            ("echo \"missing", Answer::Fixed(exec(0, "missing spoo\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(gopls_and_spoo());
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let (_, check, timeout) = missing_check(&b);
    let first = check.split_inclusive('\n').next().unwrap_or("");
    assert!(
        first.starts_with("export PATH=/root/.local/bin:") && first.ends_with('\n') && first["export PATH=/root/.local/bin:".len()..].contains("/usr/local/bin"),
        "the check's PATH line: {first:?}"
    );
    assert!(check.contains("for b in 'gopls' 'spoo'; do command -v \"$b\" >/dev/null 2>&1 || echo \"missing $b\"; done"));
    assert_eq!(timeout, Some(20_000));
    let res = res.borrow();
    assert_eq!(
        res[0].tools.iter().find(|t| t.id == "tools/cli/spoo").map(|t| ToolResult { ms: t.ms.map(|_| 0), ..t.clone() }),
        Some(ToolResult { id: "tools/cli/spoo".into(), label: "spoo".into(), outcome: "failed".into(), note: Some("spoo is not on PATH after the install".into()), ms: Some(0), bytes: Some(0.0), ..Default::default() })
    );
    assert_eq!(res[0].tools.iter().find(|t| t.id == "tools/go/gopls").map(|t| t.outcome.as_str()), Some("installed"));
    assert!(st.all().contains(&"installing-tools:4 installed, 1 failed: spoo (spoo is not on PATH after the install); caches swept; 2.9 GB free".to_string()));
    // Nothing of the person's named its command: the only check is the base floor's.
    let plain = backend_default();
    build(prep(&plain.rec, &Uploads::new(&[]), import_of()));
    let checks: Vec<String> = inline_cmds(&plain).into_iter().filter(|c| c.contains("echo \"missing")).collect();
    assert_eq!(checks.len(), 1);
    assert!(checks[0].contains("for b in 'curl' 'uv' 'python3' 'git' 'jq' 'rg' 'cc' 'fd' 'sqlite3' 'wget' 'zip' 'xz' 'rsync'; do"));
}

// golden.test.ts:1636
#[test]
fn two_tap_roads_whose_go_module_is_named_otherwise_pass_the_check_end_to_end() {
    let b = backend_for(
        vec![
            ("releases/tags/v0.4.1", Answer::Fixed(exec(0, "go: downloading\nWSP_ROAD go github.com/spoo-me/spoo-cli@v0.4.1\n", ""))),
            ("releases/tags/v0.2.0", Answer::Fixed(exec(0, "WSP_ROAD go github.com/Zingzy/bloom-cli@v0.2.0\n", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let mut table = BrewTable::new();
    table.insert(
        "zingzy/tap/bloom".into(),
        BrewFormula {
            name: "bloom".into(),
            full_name: "zingzy/tap/bloom".into(),
            deps: Vec::new(),
            macos_only: false,
            source: Some(ToolSource { repo: "Zingzy/bloom-cli".into(), tag: "v0.2.0".into() }),
            ..Default::default()
        },
    );
    table.insert(
        "spoo-me/tap/spoo".into(),
        BrewFormula {
            name: "spoo".into(),
            full_name: "spoo-me/tap/spoo".into(),
            deps: Vec::new(),
            macos_only: false,
            source: Some(ToolSource { repo: "spoo-me/spoo-cli".into(), tag: "v0.4.1".into() }),
            ..Default::default()
        },
    );
    let plan = vec![
        RecipeEntry { bring: Some(true), ..tool_entry("tools/brew/zingzy/tap/bloom", "bloom", "bring", None, Some("unknown")) },
        RecipeEntry { bring: Some(true), ..tool_entry("tools/brew/spoo-me/tap/spoo", "spoo", "bring", None, Some("unknown")) },
    ];
    let installs = plan_installs(&plan, &table);
    assert_eq!(
        installs.iter().map(|i| (i.id.clone(), i.bin.clone())).collect::<Vec<_>>(),
        vec![("tools/brew/zingzy/tap/bloom".to_string(), Some("bloom".to_string())), ("tools/brew/spoo-me/tap/spoo".to_string(), Some("spoo".to_string()))]
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools: installs, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    assert!(found(&cmds, "releases/tags/v0.2.0").contains(r#"mv '\''/usr/local/bin/bloom-cli'\'' "/usr/local/bin/$name""#));
    assert!(found(&cmds, "releases/tags/v0.4.1").contains(r#"mv '\''/usr/local/bin/spoo-cli'\'' "/usr/local/bin/$name""#));
    assert!(missing_check(&b).1.contains("for b in 'bloom' 'spoo'; do"));
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools),
        vec![
            ToolResult {
                id: "tools/brew/zingzy/tap/bloom".into(),
                label: "bloom".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad { kind: "go".into(), from: "github.com/Zingzy/bloom-cli@v0.2.0".into(), ..Default::default() }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
            ToolResult {
                id: "tools/brew/spoo-me/tap/spoo".into(),
                label: "spoo".into(),
                outcome: "installed".into(),
                road: Some(ToolRoad { kind: "go".into(), from: "github.com/spoo-me/spoo-cli@v0.4.1".into(), ..Default::default() }),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
        ]
    );
    assert!(st.all().contains(&"installing-tools:2 installed (bloom with go install, spoo with go install); caches swept; 2.9 GB free".to_string()));
}

// golden.test.ts:1663
#[test]
fn a_check_that_itself_fails_counts_every_named_tool_as_failed() {
    let b = backend_for(
        vec![
            ("releases/tags/v0.4.1", Answer::Fixed(exec(0, &format!("WSP_ROAD release spoo_0.4.1_linux_amd64.tar.gz {} v0.4.1\n", "a".repeat(64)), ""))),
            ("command -v \"$b\"", Answer::Fixed(exec(124, "", ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut tools = import_of().tools;
    tools.extend(gopls_and_spoo());
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { tools, on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let res = res.borrow();
    assert_eq!(
        norm_tools(&res[0].tools[3..]),
        vec![
            ToolResult {
                id: "tools/go/gopls".into(),
                label: "gopls".into(),
                outcome: "failed".into(),
                note: Some("gopls could not be checked on PATH: timed out after 20s".into()),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
            ToolResult {
                id: "tools/cli/spoo".into(),
                label: "spoo".into(),
                outcome: "failed".into(),
                note: Some("spoo could not be checked on PATH: timed out after 20s".into()),
                ms: Some(0),
                bytes: Some(0.0),
                ..Default::default()
            },
        ]
    );
    let all = st.all();
    assert!(all.contains(&"installing-tools:the PATH check failed (timed out after 20s): gopls, spoo count as failed".to_string()));
    assert!(all.contains(
        &"installing-tools:3 installed, 2 failed: gopls (gopls could not be checked on PATH: timed out after 20s), spoo (spoo could not be checked on PATH: timed out after 20s); caches swept; 2.9 GB free".to_string()
    ));
}

// golden.test.ts:1684
#[test]
fn an_agent_that_fails_refuses_the_seal_kills_the_builder_and_never_starts_the_tools() {
    let b = backend_for(vec![("codex-install", Answer::Fixed(exec(124, "", "")))], Free::Kb(mb(3000)));
    let st = stage_recorder();
    let (res, on_result) = collect_results();
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { on_result: Some(on_result), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    let err = run(prepare_builder(o, &*Manual::new())).err().expect("the build refuses");
    assert!(err.message().contains("an agent did not install"), "{}", err.message());
    assert_eq!(st.all().last().map(String::as_str), Some("failed:an agent did not install, so nothing is sealed:\nCodex: timed out after 900s"));
    assert_eq!(b.rec.state.killed(), vec!["m1".to_string()]);
    let res = res.borrow();
    assert_eq!(
        norm_agents(&res[0].agents),
        vec![
            agent_done("agents/claude", "Claude Code"),
            AgentResult { id: "agents/codex".into(), name: "Codex".into(), outcome: "failed".into(), note: Some("timed out after 900s".into()), ms: Some(0) },
        ]
    );
    let cmds = b.cmds();
    assert!(!cmds.iter().any(|c| c.contains("brew-bootstrap") || c.contains("brew install gh") || c.contains("bun@1.4.0")));
}

// golden.test.ts:1698
#[test]
fn switched_on_plugins_are_built_into_the_seed_and_one_that_does_not_build_is_left_out() {
    struct PluginLinks;
    #[async_trait(?Send)]
    impl SeedLinks for PluginLinks {
        async fn links(&self, read: &dyn SeedRead) -> Vec<(String, String)> {
            match read.read("installed_plugins.json").await {
                Some(text) if text.contains("1.0.0") => vec![("cache/acme/v/aaaaaaaaaaaa".to_string(), "1.0.0".to_string())],
                _ => Vec::new(),
            }
        }
    }
    let index = r#"{"version":2,"plugins":{"v@acme":[{"installPath":"/seed/cache/acme/v/1.0.0","gitCommitSha":"aaaaaaaaaaaabbbb"}]}}"#;
    let b = backend_for(
        vec![
            ("seed-build-bad", Answer::Fixed(exec(1, "", "fatal: repository not found"))),
            ("cat '/seed/installed_plugins.json'", Answer::Fixed(exec(0, index, ""))),
        ],
        Free::Kb(mb(3000)),
    );
    let st = stage_recorder();
    let plugins = PluginsPlan {
        dropped: vec!["/root/.claude-cfg/plugins/known_marketplaces.json".into()],
        seed: Some(PluginSeedPlan {
            dir: "/seed".into(),
            plugins: vec![
                SeedPlugin { id: "plugins/v@acme".into(), label: "v@acme".into(), cmd: "seed-build-good".into(), asked: None },
                SeedPlugin { id: "plugins/w@acme".into(), label: "w@acme".into(), cmd: "seed-build-bad".into(), asked: None },
            ],
            links: Arc::new(PluginLinks),
        }),
    };
    let mut o = prep(&b.rec, &Uploads::new(&[]), GoldenImport { plugins: Some(plugins), ..import_of() });
    o.on_stage = Some(st.on_stage.clone());
    build(o);
    let cmds = b.cmds();
    let at = |needle: &str| find_index(&cmds, needle);
    assert!(at("rm -f '/root/.claude-cfg/plugins/known_marketplaces.json'") > at("codex-install"));
    assert!(at("seed-build-good") > at("rm -f '/root/.claude-cfg/plugins/known_marketplaces.json'"));
    assert!(at("seed-build-bad") > at("seed-build-good"));
    assert!(cmds.contains(&"ln -sfn '1.0.0' '/seed/cache/acme/v/aaaaaaaaaaaa'".to_string()));
    assert!(st.all().contains(&"installing-harness:plugins built in: v@acme; left out: w@acme (fatal: repository not found)".to_string()));
}
