// SPDX-License-Identifier: AGPL-3.0-only
//! golden.test.ts describe("golden update") (:2567-2950): the recipe delta on a fork of the golden or on the kept
//! builder, the seal that keeps the builder, the smoke, the missing-tool and left-behind notes, the floor, and the
//! version lineage.
//!
//! Gaps: `recipeDigest`, `diffRecipes`, `rowsToApply` and `retiredBy` (golden-import.ts:620, golden-diff.ts) have no
//! Rust port. The one test that builds a delta from two recipes (the binary-row toggle) carries the deltas by hand, and
//! its `recipe` digest assertions are left out.
#![allow(clippy::type_complexity)]
mod golden_roads_common;
mod golden_seal_support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use golden_roads_common::*;
use golden_seal_support::*;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_catalog::roads::GOLDEN_SETUP;
use wsp_engine::errors::{classify, EngineError, Result};
use wsp_engine::fetch::Fetch;
use wsp_engine::golden::{
    apply_delta, next_left_behind, next_missing, next_setup_sha, next_smoke, prepare_builder, seal_golden, upgrade_builder, Applied,
    ApplyDeltaOptions, Builder, GoldenDelta, ImportLedger, ImportResult, ImportStage, MachineSize, PrepareBuilderOptions, ResultFiles,
    SealGoldenOptions, SealResult, UpgradeBuilderOptions,
};
use wsp_engine::golden_seam::{
    tool_installs_for_rows, AgentInstall, BrewFormula, BrewTable, GoldenImport, RecipeEntry, SkippedRow, ToolInstall, ToolResult,
    ToolSource, BUILDER_IDLE_MS, TOOLS_PATH,
};
use wsp_engine::machine::{
    BackendOffers, BackendPricing, BaseTemplates, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, TemplateRow, Waiting,
};
use wsp_wire::views::golden_image::{
    GoldenBaseTool, GoldenLeftBehind, GoldenManifest, GoldenMissingTool, GoldenRetired, GoldenVersion, MachineKind, RecipeDigest,
};

/// golden.test.ts:2568 `SNAPSHOT`.
fn snapshot_digest() -> RecipeDigest {
    serde_json::from_value(json!({ "ticks": [], "files": [] })).expect("an empty recipe digest")
}

/// golden.test.ts:2569 `head`.
fn head_version() -> GoldenVersion {
    serde_json::from_value(json!({
        "version": 1,
        "snapshotId": "snap_wsp-h1-default-v1",
        "baseTemplate": "base",
        "kind": "desktop",
        "setupSha": "s1",
        "createdAt": "2026-09-01T00:00:00.000Z",
        "smoke": { "cmd": "claude --version && gemini --version", "exitCode": 0 },
        "size": { "cpu": 2, "memMb": 8192 },
        "base": [{ "name": "node", "version": "22.23.2" }, { "name": "jq", "version": "1.7.1" }]
    }))
    .expect("the head version")
}

fn tool(id: &str, label: &str, manager: &str, cmd: &str) -> ToolInstall {
    ToolInstall { id: id.into(), label: label.into(), manager: manager.into(), cmd: cmd.into(), ..Default::default() }
}

fn agent(id: &str, name: &str, install: &str, smoke: &str, road: &str) -> AgentInstall {
    AgentInstall { id: id.into(), name: name.into(), install: install.into(), smoke: smoke.into(), road: road.into(), ..Default::default() }
}

fn retired(id: &str, name: &str) -> GoldenRetired {
    GoldenRetired { id: id.into(), name: name.into() }
}

/// golden.test.ts:2572 `deltaOf`'s four retired rows, the same on the image and in the recipe.
fn four_retired() -> Vec<GoldenRetired> {
    vec![
        retired("shell/zshrc", "~/.zshrc"),
        retired("tools/npm/bun", "bun"),
        retired("agents/gemini", "Gemini CLI"),
        retired("agents/claude", "Claude Code"),
    ]
}

/// golden.test.ts:2570 `deltaOf()` with no override.
fn delta_of() -> GoldenDelta {
    GoldenDelta {
        import: GoldenImport {
            recipe_hash: "h2".into(),
            recipe: Some(snapshot_digest()),
            tools: vec![tool("tools/brew/jq", "jq", "brew", "brew install jq")],
            agents: vec![agent("agents/codex", "Codex", "codex-install", "codex --version", "npm")],
            ..import_of()
        },
        retired: four_retired(),
        retired_on_image: four_retired(),
    }
}

/// `deltaOf({ retired: [], retiredOnImage: [] })`.
fn delta_retiring_nothing() -> GoldenDelta {
    GoldenDelta { retired: vec![], retired_on_image: vec![], ..delta_of() }
}

fn missing_tool(id: &str, name: &str, outcome: &str, note: &str) -> GoldenMissingTool {
    serde_json::from_value(json!({ "id": id, "name": name, "outcome": outcome, "note": note })).expect("a missing tool")
}

fn gopls() -> GoldenMissingTool {
    missing_tool("tools/brew/gopls", "gopls", "skipped", "no Linux bottle")
}
fn bun() -> GoldenMissingTool {
    missing_tool("tools/npm/bun", "bun", "skipped", "no Linux bottle known")
}
fn jq() -> GoldenMissingTool {
    missing_tool("tools/brew/jq", "jq", "failed", "exit 1: no bottle available")
}
fn raycast() -> GoldenMissingTool {
    missing_tool("tools/brew-cask/raycast", "Raycast", "skipped", "macOS app, no Linux build")
}

fn left_behind_row(id: &str, path: &str, note: &str) -> GoldenLeftBehind {
    GoldenLeftBehind { id: id.into(), path: path.into(), note: note.into() }
}

fn strs(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

/// A fresh machine from the recording backend, as `backend.create({ kind: "sandbox", template: "base" })`.
fn sandbox(rec: &Rec) -> Arc<dyn Machine> {
    let spec: MachineSpec = serde_json::from_value(json!({ "kind": "sandbox", "template": "base" })).expect("a sandbox spec");
    run(rec.dyn_backend().create(spec, None, None)).expect("the sandbox is created")
}

/// `applyDelta`'s options as the TS tests spell them most often: the setup line `true`, the previous smoke, the head's
/// floor and nothing else.
fn apply_opts(previous_smoke: &str) -> ApplyDeltaOptions {
    ApplyDeltaOptions {
        setup: "true".into(),
        previous_smoke: previous_smoke.into(),
        previous_missing: None,
        previous_left_behind: None,
        previous_base: head_version().base,
        fetch: None,
        on_stage: None,
    }
}

fn prep(opts: PrepareBuilderOptions) -> Builder {
    run(prepare_builder(opts, &*Manual::new())).expect("the builder is prepared")
}

fn seal(builder: Builder, opts: SealGoldenOptions) -> SealResult {
    run(seal_golden(builder, opts, &*Manual::new())).expect("the seal sealed")
}

fn upgrade(opts: UpgradeBuilderOptions) -> Builder {
    run(upgrade_builder(opts, &*Manual::new())).expect("the upgrade forks a builder")
}

fn apply(machine: &Arc<dyn Machine>, delta: GoldenDelta, opts: ApplyDeltaOptions) -> Applied {
    run(apply_delta(machine, delta, opts, &*Manual::new())).expect("the delta applies")
}

fn ledger_of(recipe_hash: &str, applied: Vec<ImportStage>, smoke: &str, recipe: Option<RecipeDigest>) -> ImportLedger {
    ImportLedger {
        recipe_hash: recipe_hash.into(),
        applied,
        smoke: smoke.into(),
        recipe,
        missing_tools: None,
        silenced: None,
        shell_noise: None,
        left_behind: None,
    }
}

/// The five stages a full import runs, in the order the ledger lists them.
fn all_stages() -> Vec<ImportStage> {
    vec![
        ImportStage::ApplyingSetup,
        ImportStage::UploadingFiles,
        ImportStage::InstallingHarness,
        ImportStage::InstallingTools,
        ImportStage::InstallingMcp,
    ]
}

/// One expected stage line: exact, a prefix (a `stringMatching` with `^...`), or the machine context line.
#[derive(Debug)]
enum Line {
    Is(&'static str),
    Starts(&'static str),
    ContextWritten,
}

fn assert_lines(got: &[String], want: &[Line]) {
    assert_eq!(got.len(), want.len(), "the number of stage lines; got {got:#?}");
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        let ok = match w {
            Line::Is(s) => g == s,
            Line::Starts(p) => g.starts_with(p),
            Line::ContextWritten => is_context_written(g),
        };
        assert!(ok, "stage line {i}: want {w:?}, got {g:?}");
    }
}

fn last_line(lines: &[String]) -> &str {
    lines.last().map_or("", String::as_str)
}

/// `/\b rm -rf -- /` and `/uninstall/`: the command text a removal would carry.
fn removes_or_uninstalls(text: &str) -> bool {
    let is_word = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    text.contains("uninstall") || text.match_indices("rm -rf -- ").any(|(i, _)| i == 0 || !is_word(text.as_bytes()[i - 1]))
}

/// `/^rm -f (\S+)$/` on one line: the path it removes.
fn rm_f_target(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("rm -f ")?;
    (!rest.is_empty() && !rest.chars().any(char::is_whitespace)).then_some(rest)
}

/// `/^install -D -m 0755 \S+ ([^\s']+)/` on one line: the path it installs to.
fn installed_target(line: &str) -> Option<String> {
    let rest = line.strip_prefix("install -D -m 0755 ")?;
    let (token, after) = rest.split_once(' ')?;
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return None;
    }
    let target: String = after.chars().take_while(|c| !c.is_whitespace() && *c != '\'').collect();
    (!target.is_empty()).then_some(target)
}

/// `/^installing-mcp:machine context: \d+(\.\d+)? KB written for Claude Code, Codex$/`.
fn context_written_for_claude_and_codex(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("installing-mcp:machine context: ") else { return false };
    let Some(size) = rest.strip_suffix(" KB written for Claude Code, Codex") else { return false };
    let (whole, frac) = match size.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (size, None),
    };
    !whole.is_empty()
        && whole.bytes().all(|c| c.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|c| c.is_ascii_digit()))
}

/// The binary-row fixtures' road table: one tap formula with a release tag (golden-import.ts `BrewTable`).
fn road_table() -> BrewTable {
    let mut table = BrewTable::new();
    table.insert(
        "zingzy/tap/diskbloom".to_string(),
        BrewFormula {
            name: "diskbloom".into(),
            full_name: "zingzy/tap/diskbloom".into(),
            deps: vec![],
            macos_only: false,
            source: Some(ToolSource { repo: "zingzy/diskbloom".into(), tag: "v1.2.0".into() }),
            ..Default::default()
        },
    );
    table
}

fn entry(rung: &str, id: &str, paths: &[&str], bytes: u64, linux: Option<&str>) -> RecipeEntry {
    RecipeEntry {
        rung: rung.into(),
        id: id.into(),
        label: id[id.rfind('/').map_or(0, |i| i + 1)..].to_string(),
        paths: paths.iter().map(|p| p.to_string()).collect(),
        bytes,
        default: "bring".into(),
        bring: Some(true),
        linux: linux.map(str::to_string),
        ..Default::default()
    }
}

/// golden.test.ts:2865 `planOf`'s results sink: each result the import reports, in order.
fn sink_of(results: &Rc<RefCell<Vec<ImportResult>>>) -> Arc<dyn Fn(&ImportResult)> {
    let results = results.clone();
    Arc::new(move |r: &ImportResult| results.borrow_mut().push(r.clone()))
}

/// golden.test.ts:2855 `planOf`: a plan of the given rows, tools planned as a first build plans them. The recipe digest
/// is not built here (no Rust port), so `recipe` is absent.
fn plan_of(rows: &[RecipeEntry], recipe_hash: &str, results: &Rc<RefCell<Vec<ImportResult>>>) -> GoldenImport {
    let tools = tool_installs_for_rows(rows, &road_table(), &[], TOOLS_PATH, None).expect("the tool plan").installs;
    GoldenImport {
        recipe_hash: recipe_hash.into(),
        recipe: None,
        tools,
        agents: vec![],
        on_result: Some(sink_of(results)),
        ..Default::default()
    }
}

/// A backend whose first fork from a snapshot is refused as the account's machine cap refuses it (429), and every other
/// call goes to the recording backend it wraps.
struct Capped {
    inner: Arc<dyn MachineBackend>,
    refused: Cell<bool>,
}

#[async_trait(?Send)]
impl MachineBackend for Capped {
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
        if spec.from_snapshot.is_some() && !self.refused.get() {
            self.refused.set(true);
            return Err(EngineError::Provider(classify(429, None, Some("Too many concurrent sessions"), None)));
        }
        self.inner.create(spec, waiting, sent).await
    }
    async fn get(&self, id: &str) -> Result<Arc<dyn Machine>> {
        self.inner.get(id).await
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

// golden.test.ts:2587
#[test]
fn apply_delta_takes_a_retired_tools_version_check_out_of_the_smoke_like_an_agents() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let pnpm = GoldenDelta {
        import: GoldenImport { recipe_hash: "h2".into(), recipe: Some(snapshot_digest()), tools: vec![], agents: vec![], ..import_of() },
        retired: vec![retired("tools/npm/pnpm", "pnpm")],
        retired_on_image: vec![retired("tools/npm/pnpm", "pnpm")],
    };
    let applied =
        apply(&machine, pnpm, ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), ..apply_opts("claude --version && pnpm --version") });
    assert_eq!(applied.ledger.smoke, "claude --version");
}

// golden.test.ts:2595. A kept seal snapshots the builder, boots and kills the fork, and leaves the builder running.
#[test]
fn a_kept_seal_snapshots_the_builder_boots_and_kills_the_fork_and_leaves_the_builder_running() {
    let b = backend_default();
    let builder = prep(PrepareBuilderOptions {
        import: Some(GoldenImport { recipe: Some(snapshot_digest()), ..import_of() }),
        fetch: Some(b.dyn_fetch()),
        ..prepare_opts(&b.rec, "true")
    });
    let stages = stage_recorder();
    let result = seal(
        builder.clone(),
        SealGoldenOptions { keep_builder: true, on_stage: Some(stages.on_stage.clone()), ..seal_opts(&b.rec, "unused") },
    );
    assert!(result.builder_kept);
    assert_eq!(result.version.version.0, 1.0);
    assert_eq!(b.rec.state.killed(), vec!["m2"]);
    assert_eq!(b.rec.state.timeline(), vec!["create m1", "snapshot m1", "create m2", "kill m2"]);
    assert_eq!(last_line(&stages.all()), "sealed:v1; builder kept for one more change");
    assert_eq!(builder.import.as_ref().and_then(|l| l.recipe.clone()), Some(snapshot_digest()));
}

// golden.test.ts:2608
#[test]
fn a_seal_without_keep_consumes_the_builder_and_says_nothing_of_keeping_it() {
    let rec = recording();
    let builder = prep(prepare_opts(&rec, "true"));
    let stages = stage_recorder();
    let result = seal(builder, SealGoldenOptions { on_stage: Some(stages.on_stage.clone()), ..seal_opts(&rec, "true") });
    assert!(!result.builder_kept);
    assert_eq!(rec.state.killed(), vec!["m1", "m2"]);
    assert_eq!(last_line(&stages.all()), "sealed:v1");
}

// golden.test.ts:2618
#[test]
fn when_the_account_cap_refuses_the_fork_beside_a_kept_builder_the_builder_is_killed_first_and_the_fork_tried_again() {
    let rec = recording();
    let capped: Arc<dyn MachineBackend> = Arc::new(Capped { inner: rec.dyn_backend(), refused: Cell::new(false) });
    let builder = prep(PrepareBuilderOptions { backend: capped.clone(), ..prepare_opts(&rec, "true") });
    let stages = stage_recorder();
    let result = seal(
        builder,
        SealGoldenOptions {
            backend: capped.clone(),
            keep_builder: true,
            on_stage: Some(stages.on_stage.clone()),
            ..seal_opts(&rec, "true")
        },
    );
    assert!(!result.builder_kept);
    assert_eq!(rec.state.killed(), vec!["m1", "m2"]);
    assert_eq!(rec.state.timeline(), vec!["create m1", "snapshot m1", "kill m1", "create m2", "kill m2"]);
    assert!(stages.all().contains(&"smoke-forking:true; the account is at its machine cap, so the builder is not kept".to_string()));
    assert_eq!(last_line(&stages.all()), "sealed:v1");
}

// golden.test.ts:2641
#[test]
fn a_smoke_that_fails_on_a_kept_builder_still_kills_the_builder_and_drops_the_snapshot() {
    let mut exec_results = IndexMap::new();
    exec_results.insert("boom --version".to_string(), exec(127, "", "not found"));
    let rec = recording_backend(Opts { exec_results, ..Opts::default() });
    let builder = prep(prepare_opts(&rec, "true"));
    let err = run(seal_golden(builder, SealGoldenOptions { keep_builder: true, ..seal_opts(&rec, "boom --version") }, &*Manual::new()))
        .expect_err("the seal is refused");
    assert!(err.message().contains("smoke"), "want the refusal to name the smoke, got {:?}", err.message());
    assert_eq!(rec.state.killed(), vec!["m1", "m2"]);
    assert_eq!(rec.state.deleted_snapshots(), vec!["snap_wsp-h1-default-v1"]);
}

// golden.test.ts:2649
#[test]
fn apply_delta_leaves_dropped_rows_on_the_image_and_names_them_once_before_the_stages() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let stages = stage_recorder();
    let applied = apply(
        &machine,
        delta_of(),
        ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), on_stage: Some(stages.on_stage.clone()), ..apply_opts(&head_version().smoke.cmd) },
    );
    // Nothing an earlier version installed is taken off: no uninstall, no rm of a file the recipe dropped.
    assert!(!b.rec.state.ran.borrow().iter().any(|(_, s)| removes_or_uninstalls(s)));
    assert!(!b.cmds().iter().any(|c| removes_or_uninstalls(c)));
    let lines = stages.all();
    assert_lines(&lines[..1], &[Line::Is("applying-setup:4 rows left on the image, retired: ~/.zshrc, bun, Gemini CLI, Claude Code")]);
    assert_lines(
        &lines[1..],
        &[
            Line::Is("applying-setup:3 files: identity 1, shell 2"),
            Line::Is("applying-setup:1 KB packed; skipped ~/.bashrc (no longer on this computer)"),
            Line::Is("uploading-files:1 KB"),
            Line::Starts("uploading-files:1 KB in "),
            Line::Is("installing-harness"),
            Line::Is("installing-harness:Codex (1/1)"),
            Line::Is("installing-harness:Codex installed; caches swept; 2.9 GB free"),
            Line::Is("installing-tools:jq (1/1)"),
            Line::Is("installing-tools:1 installed; caches swept; 2.9 GB free"),
            Line::Is("installing-mcp:none configured"),
            Line::ContextWritten,
        ],
    );
    assert_eq!(b.cmds()[0], FREE_KB_CMD);
    // Both agents are still on the image, but the recipe stopped asking for them, so their checks leave the smoke.
    assert_eq!(applied.ledger, ledger_of("h2", all_stages(), "codex --version", Some(snapshot_digest())));
}

// golden.test.ts:2676
#[test]
fn update_of_an_image_with_a_harness_under_home_leaves_only_usr_local_bin_claude() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    apply(
        &machine,
        delta_retiring_nothing(),
        ApplyDeltaOptions { setup: GOLDEN_SETUP.to_string(), fetch: Some(b.dyn_fetch()), ..apply_opts("true") },
    );
    // The machine's disk as the scripts it ran leave it, from an image whose harness sits first on TOOLS_PATH.
    let mut disk: Vec<String> = vec!["/root/.local/bin/claude".into()];
    let scripts: Vec<String> = b.rec.state.ran.borrow().iter().map(|(_, s)| s.clone()).collect();
    for script in &scripts {
        for line in script.split('\n') {
            if let Some(removed) = rm_f_target(line) {
                disk.retain(|p| p != removed);
            }
            if let Some(installed) = installed_target(line) {
                if !disk.contains(&installed) {
                    disk.push(installed);
                }
            }
        }
    }
    assert_eq!(disk, vec!["/usr/local/bin/claude".to_string()]);
}

// golden.test.ts:2693
#[test]
fn a_delta_that_retires_nothing_goes_straight_to_the_stages() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let stages = stage_recorder();
    apply(
        &machine,
        delta_retiring_nothing(),
        ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), on_stage: Some(stages.on_stage.clone()), ..apply_opts("true") },
    );
    assert_eq!(stages.all()[0], "applying-setup:3 files: identity 1, shell 2");
    assert_eq!(b.cmds()[0], FREE_KB_CMD);
}

// golden.test.ts:2702
#[test]
fn an_update_whose_agents_changed_writes_the_machine_context_again_after_its_stages() {
    // The fork carries the previous version's facts, which name both agents as not installed.
    let prior = r#"{"tools":[],"agents":[{"id":"agents/codex","label":"Codex","note":"no installer"},{"id":"agents/gemini","label":"Gemini CLI","note":"no installer"}],"files":[]}"#;
    let probe = format!(
        "WSP_CTX\nAGENT claude\nAGENT codex\nFACTS {}\nWSP_CTX_END\n",
        base64::engine::general_purpose::STANDARD.encode(prior.as_bytes())
    );
    let b = backend_for(vec![("echo WSP_CTX", Answer::Fixed(exec(0, &probe, "")))], Free::Kb(mb(3000)));
    let machine = sandbox(&b.rec);
    let stages = stage_recorder();
    let applied = apply(
        &machine,
        delta_of(),
        ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), on_stage: Some(stages.on_stage.clone()), ..apply_opts(&head_version().smoke.cmd) },
    );
    let lines = stages.all();
    assert!(context_written_for_claude_and_codex(last_line(&lines)), "the last stage line is {:?}", last_line(&lines));
    assert!(applied.ledger.applied.contains(&ImportStage::InstallingMcp));
    let cmds = b.cmds();
    let write = find_index(&cmds, |c| c.contains("tar xzf - -C '/' "));
    assert!(write > find_index(&cmds, |c| c.contains("codex-install")));
    assert!(write > find_index(&cmds, |c| c.contains("npm uninstall -g @google/gemini-cli")));
    let tgz = b.fetch.puts().last().cloned().expect("the machine context was uploaded");
    assert!(names_in(&tgz).iter().any(|n| n == "etc/codex/requirements.toml"));
    let context: Value = serde_json::from_str(&file_in(&tgz, "etc/wsp/machine-context.json")).expect("the context is JSON");
    assert_eq!(context, json!({ "tools": [], "agents": [], "files": [{ "path": "~/.bashrc", "note": "no longer on this computer" }] }));
}

// golden.test.ts:2720
#[test]
fn next_smoke_of_true_with_nothing_removed_and_true_added_is_true() {
    assert_eq!(next_smoke("true", &[], "true"), "true");
}

// golden.test.ts:2720
#[test]
fn next_smoke_drops_a_removed_check_from_a_two_check_smoke_and_keeps_the_other() {
    assert_eq!(next_smoke("a --version && b --version", &strs(&["b --version"]), "true"), "a --version");
}

// golden.test.ts:2720
#[test]
fn next_smoke_appends_an_added_check_to_the_previous_smoke() {
    assert_eq!(next_smoke("a --version", &[], "a --version && c --version"), "a --version && c --version");
}

// golden.test.ts:2720
#[test]
fn next_smoke_from_true_with_one_added_check_is_that_check() {
    assert_eq!(next_smoke("true", &[], "x --version"), "x --version");
}

// golden.test.ts:2720
#[test]
fn next_smoke_removing_the_only_check_and_adding_true_is_true() {
    assert_eq!(next_smoke("a --version", &strs(&["a --version"]), "true"), "true");
}

// golden.test.ts:2734
#[test]
fn next_missing_of_nothing_is_nothing() {
    assert_eq!(next_missing(&[], &delta_of(), &[]), Vec::<GoldenMissingTool>::new());
}

// golden.test.ts:2734
#[test]
fn next_missing_keeps_a_previous_row_the_delta_does_not_touch_and_drops_the_touched_ones() {
    assert_eq!(next_missing(&[gopls(), bun(), jq()], &delta_of(), &[]), vec![gopls()]);
}

// golden.test.ts:2734
#[test]
fn next_missing_keeps_the_untouched_previous_row_beside_a_fresh_one() {
    assert_eq!(next_missing(&[gopls()], &delta_of(), &[raycast()]), vec![gopls(), raycast()]);
}

// golden.test.ts:2734
#[test]
fn next_missing_takes_a_row_planned_again_from_the_fresh_list() {
    assert_eq!(next_missing(&[jq()], &delta_of(), &[jq()]), vec![jq()]);
}

// golden.test.ts:2743
#[test]
fn next_missing_drops_the_tool_whose_id_the_delta_addressed_not_every_tool_sharing_its_label() {
    let brew_gh = missing_tool("tools/brew/gh", "gh", "skipped", "no Linux bottle");
    let cli_gh = missing_tool("tools/cli/gh", "gh", "failed", "exit 1: download refused");
    let retired_gh =
        GoldenDelta { retired: vec![retired("tools/brew/gh", "gh")], retired_on_image: vec![retired("tools/brew/gh", "gh")], ..delta_of() };
    assert_eq!(next_missing(&[brew_gh.clone(), cli_gh.clone()], &retired_gh, &[]), vec![cli_gh.clone()]);

    let replanned = GoldenDelta {
        import: GoldenImport { tools: vec![tool("tools/cli/gh", "gh", "script", "install-gh")], ..delta_of().import },
        ..delta_of()
    };
    assert_eq!(next_missing(&[brew_gh.clone(), cli_gh.clone()], &replanned, &[]), vec![brew_gh.clone()]);

    let set_aside = GoldenDelta {
        import: GoldenImport {
            tools: vec![],
            skipped_tools: vec![SkippedRow { id: "tools/cli/gh".into(), label: "gh".into(), note: "no Linux build".into() }],
            ..delta_of().import
        },
        ..delta_of()
    };
    assert_eq!(next_missing(&[brew_gh.clone(), cli_gh], &set_aside, &[]), vec![brew_gh]);
}

// golden.test.ts:2754
#[test]
fn apply_delta_keeps_previous_missing_tools_the_delta_did_not_touch_beside_this_runs_skips() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let replanned = GoldenDelta {
        import: GoldenImport {
            tools: vec![],
            skipped_tools: vec![SkippedRow { id: "tools/brew/jq".into(), label: "jq".into(), note: "no Linux bottle known".into() }],
            ..delta_of().import
        },
        ..delta_of()
    };
    let applied = apply(
        &machine,
        replanned,
        ApplyDeltaOptions {
            previous_missing: Some(vec![gopls(), bun(), jq()]),
            fetch: Some(b.dyn_fetch()),
            ..apply_opts(&head_version().smoke.cmd)
        },
    );
    assert_eq!(applied.ledger.missing_tools, Some(vec![gopls(), missing_tool("tools/brew/jq", "jq", "skipped", "no Linux bottle known")]));
    let clean_machine = sandbox(&b.rec);
    let clean = apply(
        &clean_machine,
        delta_of(),
        ApplyDeltaOptions {
            previous_missing: Some(vec![bun(), jq()]),
            fetch: Some(b.dyn_fetch()),
            ..apply_opts(&head_version().smoke.cmd)
        },
    );
    assert_eq!(clean.ledger.missing_tools, None);
}

// golden.test.ts:2765
#[test]
fn next_left_behind_keeps_previous_notes_for_untouched_rows_beside_what_this_pack_left_behind() {
    let hook = left_behind_row("agents/claude", "~/.claude/settings.json", "hook left behind: ~/.claude/hooks/gone");
    let codex_note = left_behind_row("agents/codex", "~/.codex/config.toml", "hook left behind: ~/x");
    // delta_of retires agents/claude and plans agents/codex again.
    assert_eq!(next_left_behind(&[hook.clone(), codex_note], &delta_of(), &[]), Vec::<GoldenLeftBehind>::new());
    let untouched = GoldenDelta { retired: vec![], retired_on_image: vec![], import: GoldenImport { agents: vec![], ..delta_of().import } };
    assert_eq!(next_left_behind(std::slice::from_ref(&hook), &untouched, &[]), vec![hook.clone()]);
    let replanned = GoldenDelta {
        retired: vec![],
        retired_on_image: vec![],
        import: GoldenImport {
            agents: vec![agent("agents/claude", "Claude Code", "claude-install", "claude --version", "script")],
            ..delta_of().import
        },
    };
    let fresh = left_behind_row("agents/claude", "~/.claude/settings.json", "hook left behind: ~/.claude/hooks/new");
    assert_eq!(next_left_behind(std::slice::from_ref(&hook), &replanned, std::slice::from_ref(&fresh)), vec![fresh]);
}

// golden.test.ts:2777
#[test]
fn left_behind_notes_ride_apply_delta_upgrade_and_the_next_seal() {
    let b = backend_default();
    let hook = left_behind_row("agents/claude", "~/.claude/settings.json", "hook left behind: ~/.claude/hooks/gone");
    let untouched = GoldenDelta { retired: vec![], retired_on_image: vec![], import: GoldenImport { agents: vec![], ..delta_of().import } };
    let machine = sandbox(&b.rec);
    let applied = apply(
        &machine,
        untouched.clone(),
        ApplyDeltaOptions {
            previous_left_behind: Some(vec![hook.clone()]),
            fetch: Some(b.dyn_fetch()),
            ..apply_opts(&head_version().smoke.cmd)
        },
    );
    assert_eq!(applied.ledger.left_behind, Some(vec![hook.clone()]));
    let builder = upgrade(UpgradeBuilderOptions {
        fetch: Some(b.dyn_fetch()),
        ..upgrade_opts(&b.rec, GoldenVersion { left_behind: Some(vec![hook.clone()]), ..head_version() }, untouched, "true")
    });
    assert_eq!(builder.import.as_ref().and_then(|l| l.left_behind.clone()), Some(vec![hook.clone()]));
    let sealed = seal(builder, seal_opts(&b.rec, "true"));
    assert_eq!(sealed.version.left_behind, Some(vec![hook]));
}

// golden.test.ts:2789
#[test]
fn upgrade_builder_hands_the_heads_missing_tools_on_to_the_next_version() {
    let b = backend_default();
    let builder = upgrade(UpgradeBuilderOptions {
        fetch: Some(b.dyn_fetch()),
        ..upgrade_opts(&b.rec, GoldenVersion { missing_tools: Some(vec![gopls()]), ..head_version() }, delta_retiring_nothing(), "true")
    });
    assert_eq!(builder.import.as_ref().and_then(|l| l.missing_tools.clone()), Some(vec![gopls()]));
    assert_eq!(seal(builder, seal_opts(&b.rec, "true")).version.missing_tools, Some(vec![gopls()]));
}

// golden.test.ts:2796
#[test]
fn base_floor_is_read_at_seal_carried_by_an_upgrade_and_refused_when_absent() {
    let read = "VERSION curl: curl 8.5.0\nVERSION jq: jq-1.7.1\n";
    let rec = recording_backend(Opts {
        exec: Some(Box::new(move |cmd: &str| {
            Some(if cmd.contains("VERSION curl:") && !cmd.contains("WSP_CTX") {
                exec(0, read, "")
            } else if cmd == FREE_KB_CMD {
                exec(0, &format!("{}\n", mb(3000)), "")
            } else if cmd == "echo ok" {
                reach_ok()
            } else {
                ok()
            })
        })),
        ..Opts::default()
    });
    let fetch: Arc<dyn Fetch> = FetchStub::ok();
    let floor = vec![
        GoldenBaseTool { name: "curl".into(), version: "8.5.0".into() },
        GoldenBaseTool { name: "jq".into(), version: "1.7.1".into() },
    ];
    let builder = prep(prepare_opts(&rec, "true"));
    assert_eq!(builder.base, Some(floor.clone()));
    let v1 = seal(builder, seal_opts(&rec, "true"));
    assert_eq!(v1.version.base, Some(floor.clone()));
    let b2 = upgrade(UpgradeBuilderOptions {
        fetch: Some(fetch.clone()),
        ..upgrade_opts(&rec, v1.version.clone(), delta_retiring_nothing(), "true")
    });
    assert_eq!(b2.base, Some(floor.clone()));
    let sealed = seal(b2, SealGoldenOptions { manifest: Some(v1.manifest.clone()), ..seal_opts(&rec, "true") });
    assert_eq!(sealed.version.base, Some(floor));

    let booted = rec.state.created().len();
    let pre_floor = GoldenVersion { base: None, ..v1.version.clone() };
    let refused = run(upgrade_builder(
        UpgradeBuilderOptions { fetch: Some(fetch.clone()), ..upgrade_opts(&rec, pre_floor, delta_retiring_nothing(), "true") },
        &*Manual::new(),
    ))
    .err()
    .expect("the pre-floor version is refused");
    assert_eq!(refused.message(), "golden v1 was sealed before the base tools existed and cannot take an update; seal it again");
    assert_eq!(rec.state.created().len(), booted);
    let machine = sandbox(&rec);
    let err = run(apply_delta(
        &machine,
        delta_retiring_nothing(),
        ApplyDeltaOptions { previous_base: None, fetch: Some(fetch), ..apply_opts("true") },
        &*Manual::new(),
    ))
    .expect_err("the pre-floor golden is refused");
    assert_message_has(&err.to_string(), "this golden was sealed before the base tools existed and cannot take an update; seal it again");
}

// golden.test.ts:2817
#[test]
fn update_reports_a_newly_ticked_covered_row_as_the_bases_by_the_planners_note() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let results: Rc<RefCell<Vec<ImportResult>>> = Rc::new(RefCell::new(Vec::new()));
    let delta = GoldenDelta {
        retired: vec![],
        import: GoldenImport {
            recipe_hash: "h2".into(),
            recipe: Some(snapshot_digest()),
            tools: vec![],
            agents: vec![],
            base_tools: vec![SkippedRow { id: "tools/brew/jq".into(), label: "jq".into(), note: "jq is part of the base".into() }],
            on_result: Some(sink_of(&results)),
            ..import_of()
        },
        ..delta_of()
    };
    apply(&machine, delta, ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), ..apply_opts(&head_version().smoke.cmd) });
    let last = results.borrow().last().cloned().expect("a result was reported");
    assert_eq!(
        last.tools,
        vec![ToolResult {
            id: "tools/brew/jq".into(),
            label: "jq".into(),
            outcome: "installed".into(),
            note: Some("jq is part of the base".into()),
            ..Default::default()
        }]
    );
    let cmds = b.cmds();
    assert!(!cmds.iter().any(|c| c.contains("apt-get install") || c.contains("brew install")));
}

// golden.test.ts:2827
#[test]
fn upgrade_builder_forks_head_at_kind_and_size_and_applies_only_the_delta() {
    let b = backend_default();
    let stages = stage_recorder();
    let labels: IndexMap<String, String> =
        [("wsp", "1"), ("wsp-builder", "1"), ("wsp-owner", "h_me"), ("createdAt", "2026-09-04T00:00:00.000Z")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
    let builder = upgrade(UpgradeBuilderOptions {
        fetch: Some(b.dyn_fetch()),
        on_stage: Some(stages.on_stage.clone()),
        size: MachineSize { labels: Some(labels.clone()), ..Default::default() },
        ..upgrade_opts(&b.rec, head_version(), delta_retiring_nothing(), "true")
    });
    let created = b.rec.state.created();
    match_object(
        &created[0],
        json!({
            "kind": "desktop",
            "fromSnapshot": "snap_wsp-h1-default-v1",
            "cpu": 2,
            "memMb": 8192,
            "onIdle": "kill",
            "idleTimeoutMs": BUILDER_IDLE_MS,
            "labels": labels,
        }),
    );
    assert!(created[0].template.is_none());
    let lines = stages.all();
    assert_eq!(lines[0], "creating:fork of golden v1");
    assert_eq!(last_line(&lines), "ready");
    assert_eq!(b.cmds().iter().filter(|c| c.contains("brew install jq")).count(), 1);
    assert!(!b.cmds().iter().any(|c| c.contains("brew-bootstrap")));
    assert_eq!(builder.kind, MachineKind::Desktop);
    assert_eq!(builder.base_template, "base");
    assert!(builder.first_life);
    match_object(&builder.size, json!({ "cpu": 2, "memMb": 8192 }));
    assert_eq!(
        builder.import,
        Some(ledger_of("h2", all_stages(), "claude --version && gemini --version && codex --version", Some(snapshot_digest())))
    );
    // The version's sha chains the previous version's with what this delta ran, so v(n+1)'s sha says both.
    let planned = next_setup_sha("s1", "true", &delta_retiring_nothing().import);
    assert_eq!(builder.setup_sha, planned);
    let expected = sha256_hex(format!("s1\n{}", sha256_hex(b"true\ncodex-install")).as_bytes());
    assert_eq!(builder.setup_sha, expected);
}

// golden.test.ts:2845
#[test]
fn an_upgrade_whose_delta_fails_kills_the_fork_and_reports_the_failure() {
    let b = backend_for(Vec::new(), Free::Kb(mb(1)));
    let stages = stage_recorder();
    let err = run(upgrade_builder(
        UpgradeBuilderOptions {
            fetch: Some(b.dyn_fetch()),
            on_stage: Some(stages.on_stage.clone()),
            ..upgrade_opts(&b.rec, head_version(), delta_of(), "true")
        },
        &*Manual::new(),
    ))
    .err()
    .expect("the upgrade is refused");
    assert!(err.message().contains("your files need"), "got {:?}", err.message());
    assert_eq!(b.rec.state.killed(), vec!["m1"]);
    assert!(last_line(&stages.all()).starts_with("failed:your files need"), "last stage {:?}", last_line(&stages.all()));
}

// golden.test.ts:2869
#[test]
fn a_binary_row_toggled_after_the_seal_installs_then_retires_on_the_image() {
    let b = backend_default();
    let stages = stage_recorder();
    let results: Rc<RefCell<Vec<ImportResult>>> = Rc::new(RefCell::new(Vec::new()));
    let base = vec![entry("shell", "shell/zshrc", &["~/.zshrc"], 10, None)];
    let gh = entry("tools", "tools/brew/gh", &[], 0, Some("yes"));
    let diskbloom = entry("tools", "tools/brew/zingzy/tap/diskbloom", &[], 0, Some("unknown"));
    let binaries = [gh.clone(), diskbloom.clone()];
    // The setup line runs on every harness stage; what this counts is the rows.
    let guarded_cmds = |from: usize| -> Vec<String> {
        b.cmds()[from..].iter().filter(|c| c.contains("setsid bash -c") && !c.contains("\ntrue' &")).cloned().collect()
    };

    let v1 = seal(
        prep(PrepareBuilderOptions {
            import: Some(plan_of(&base, "h1", &results)),
            fetch: Some(b.dyn_fetch()),
            ..prepare_opts(&b.rec, "true")
        }),
        seal_opts(&b.rec, "true"),
    );
    let n1 = b.cmds().len();
    assert!(!b.cmds().iter().any(|c| c.contains("brew install gh")));

    // The delta the recipe diff of base to base-plus-binaries gives: both binaries added, nothing retired.
    let up = GoldenDelta {
        import: GoldenImport { recipe: None, ..plan_of(&binaries, "h2", &results) },
        retired: vec![],
        retired_on_image: vec![],
    };
    assert!(up.retired.is_empty());
    let b2 = upgrade(UpgradeBuilderOptions {
        fetch: Some(b.dyn_fetch()),
        on_stage: Some(stages.on_stage.clone()),
        ..upgrade_opts(&b.rec, v1.version.clone(), up, "true")
    });
    let installs = guarded_cmds(n1);
    assert!(installs.iter().any(|c| c.contains("brew install gh")));
    assert!(installs
        .iter()
        .any(|c| c.contains("name='\\''diskbloom'\\''") && c.contains(r#"install -m 0755 "$bin" "/usr/local/bin/$name""#)));
    assert!(!installs.iter().any(|c| c.contains("uninstall")));
    let up_result = results.borrow().last().cloned().expect("the up result");
    let binary_outcomes: Vec<(String, String)> =
        up_result.tools.iter().filter(|t| binaries.iter().any(|x| x.id == t.id)).map(|t| (t.id.clone(), t.outcome.clone())).collect();
    assert_eq!(
        binary_outcomes,
        vec![
            ("tools/brew/gh".to_string(), "installed".to_string()),
            ("tools/brew/zingzy/tap/diskbloom".to_string(), "installed".to_string()),
        ]
    );
    assert!(up_result.retired.is_none());
    let v2 = seal(b2, SealGoldenOptions { manifest: Some(v1.manifest.clone()), ..seal_opts(&b.rec, "true") });
    assert_eq!(v2.version.version.0, 2.0);

    let n2 = b.cmds().len();
    stages.lines.borrow_mut().clear();
    // The delta the recipe diff of binaries-plus-base to base gives: both binaries removed, nothing to install.
    let down = GoldenDelta {
        import: GoldenImport { recipe: None, ..plan_of(&[], "h3", &results) },
        retired: vec![retired("tools/brew/gh", "gh"), retired("tools/brew/zingzy/tap/diskbloom", "diskbloom")],
        retired_on_image: vec![retired("tools/brew/gh", "gh"), retired("tools/brew/zingzy/tap/diskbloom", "diskbloom")],
    };
    assert!(down.import.tools.is_empty());
    assert!(down.import.files.is_none());
    let b3 = upgrade(UpgradeBuilderOptions {
        fetch: Some(b.dyn_fetch()),
        on_stage: Some(stages.on_stage.clone()),
        ..upgrade_opts(&b.rec, v2.version.clone(), down, "true")
    });
    // Nothing runs on the machine for the two rows that left the recipe: they keep their bytes and the version records them.
    assert!(guarded_cmds(n2).is_empty());
    let after_n2 = &b.cmds()[n2..];
    assert!(!after_n2.iter().any(|c| c.contains("brew install") || c.contains("uninstall")));
    assert!(stages.all().contains(&"applying-setup:2 rows left on the image, retired: gh, diskbloom".to_string()));
    let down_result = results.borrow().last().cloned().expect("the down result");
    assert_eq!(
        down_result,
        ImportResult {
            recipe_hash: "h3".into(),
            files: Some(ResultFiles { bytes: 0, skipped: vec![], cut: None, blanked: None }),
            tools: vec![],
            agents: vec![],
            retired: Some(vec![retired("tools/brew/gh", "gh"), retired("tools/brew/zingzy/tap/diskbloom", "diskbloom")]),
            context: Some(vec![]),
            ..Default::default()
        }
    );
    let v3 = seal(b3.clone(), SealGoldenOptions { manifest: Some(v2.manifest.clone()), ..seal_opts(&b.rec, "true") });
    assert_eq!(v3.version.version.0, 3.0);
    // The lineage carries what v3's image holds that its recipe does not ask for.
    assert_eq!(v3.version.retired, Some(vec![retired("tools/brew/gh", "gh"), retired("tools/brew/zingzy/tap/diskbloom", "diskbloom")]));
    assert!(v2.version.retired.is_none());
    let ledger = b3.import.clone().expect("the builder carries its ledger");
    assert_eq!(ledger.recipe_hash, "h3");
    assert_eq!(ledger.applied, all_stages());
    assert_eq!(ledger.smoke, "true");
    assert!(ledger.missing_tools.is_none() && ledger.silenced.is_none() && ledger.shell_noise.is_none() && ledger.left_behind.is_none());
}

// golden.test.ts:2923
#[test]
fn every_retired_row_is_in_the_result_beside_what_the_delta_installed() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let results: Rc<RefCell<Vec<ImportResult>>> = Rc::new(RefCell::new(Vec::new()));
    let mut delta = delta_of();
    delta.import.on_result = Some(sink_of(&results));
    apply(&machine, delta, ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), ..apply_opts(&head_version().smoke.cmd) });
    let got = results.borrow().clone();
    assert_eq!(got.len(), 1);
    let pairs: Vec<(String, String)> = got[0].tools.iter().map(|t| (t.id.clone(), t.outcome.clone())).collect();
    assert_eq!(pairs, vec![("tools/brew/jq".to_string(), "installed".to_string())]);
    assert_eq!(got[0].retired, Some(four_retired()));
}

// golden.test.ts:2935
#[test]
fn a_retired_tools_row_named_like_an_agent_leaves_that_agents_check_in_the_smoke() {
    let b = backend_default();
    let machine = sandbox(&b.rec);
    let named = GoldenDelta {
        retired: vec![retired("tools/brew/claude", "claude")],
        retired_on_image: vec![retired("tools/brew/claude", "claude")],
        ..delta_of()
    };
    let applied = apply(&machine, named, ApplyDeltaOptions { fetch: Some(b.dyn_fetch()), ..apply_opts(&head_version().smoke.cmd) });
    assert_eq!(applied.ledger.smoke, "claude --version && gemini --version && codex --version");
}

// golden.test.ts:2943
#[test]
fn the_builder_an_update_forks_carries_the_dropped_rows_and_the_version_it_seals_records_them() {
    let b = backend_default();
    let builder = upgrade(UpgradeBuilderOptions { fetch: Some(b.dyn_fetch()), ..upgrade_opts(&b.rec, head_version(), delta_of(), "true") });
    assert_eq!(builder.retired, Some(four_retired()));
    let manifest: GoldenManifest = serde_json::from_value(json!({ "head": 1, "versions": [head_version()] })).expect("a manifest");
    let sealed = seal(builder, SealGoldenOptions { manifest: Some(manifest), ..seal_opts(&b.rec, "true") });
    match_object(
        &sealed.version,
        json!({
            "version": 2,
            "parentSnapshotId": head_version().snapshot_id,
            "retired": serde_json::to_value(four_retired()).expect("the retired rows"),
        }),
    );
}
