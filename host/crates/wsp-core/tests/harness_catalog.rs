// SPDX-License-Identifier: AGPL-3.0-only
//! harness_catalog: the cases of packages/runtime/test/harness-catalog.test.ts, then the TS's own answers in
//! tests/fixtures/core_rest/harness_catalog.json (written by oracle/core_rest.oracle.ts).
mod rest;

use regex::Regex;
use rest::*;
use serde_json::{json as jv, Value};
use wsp_adapters::claude::catalog::parse_catalog_probe;
use wsp_adapters::port::{HarnessCatalogModelProbe, HarnessCatalogProbe};
use wsp_catalog::git_hosts::THREAD_AGENTS;
use wsp_catalog::roads::CLAUDE_CODE;
use wsp_core::harness_catalog::{catalog_from_probe, harness_catalog, harness_catalogs, model_label, named_only, not_run_refused, smallest_model, tool_asks};
use wsp_wire::types::harness_picks::{efforts_for, every_model, listed_pick, marked_default, model_of};
use wsp_wire::views::harness::{start_picks, HarnessCatalog, HarnessCatalogSource, HarnessModel, HarnessOption, StartPicks};
use wsp_words::words::computer::THIS_COMPUTER;
use wsp_words::words::thread::{catalog_source_line, no_models_line};

const OPEN_MODELS: [&str; 2] = ["opencode", "cursor"];

const MODELS: &str = "Opus 5.5 (claude-opus-5-5), Fable 5.1 (claude-fable-5-1), Sonnet 5.5 (claude-sonnet-5-5), Haiku 5.5 (claude-haiku-5-5)";

fn table(id: &str) -> HarnessCatalog {
    harness_catalog(id).unwrap_or_else(|| panic!("no table row for {id}"))
}

fn s(v: &str) -> Option<String> {
    Some(v.to_string())
}

fn strs(v: &[&str]) -> Option<Vec<String>> {
    Some(v.iter().map(|x| x.to_string()).collect())
}

fn words(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

fn model(value: &str, label: &str) -> HarnessModel {
    HarnessModel {
        value: value.into(),
        label: label.into(),
        description: None,
        is_default: None,
        short: None,
        efforts: None,
        default_effort: None,
        context_windows: None,
        fast: None,
        added: None,
    }
}

fn option(value: &str, label: &str) -> HarnessOption {
    HarnessOption { value: value.into(), label: label.into(), description: None, is_default: None, short: None }
}

fn model_probe(slug: &str, label: &str) -> HarnessCatalogModelProbe {
    HarnessCatalogModelProbe {
        slug: slug.into(),
        label: label.into(),
        description: None,
        efforts: None,
        default_effort: None,
        context_windows: vec![],
        is_default: false,
        fast: None,
    }
}

fn plain_probe(version: &str, models: Vec<HarnessCatalogModelProbe>, efforts: &[&str], modes: &[&str]) -> HarnessCatalogProbe {
    HarnessCatalogProbe { version: s(version), models, efforts: words(efforts), permission_modes: words(modes) }
}

fn probe() -> HarnessCatalogProbe {
    plain_probe(
        "2.1.257",
        vec![
            HarnessCatalogModelProbe {
                description: s("Opus 5.5 with 1M context"),
                efforts: strs(&["low", "high"]),
                context_windows: words(&["200k", "1m"]),
                is_default: true,
                ..model_probe("claude-opus-5-5", "Opus")
            },
            HarnessCatalogModelProbe { efforts: strs(&[]), ..model_probe("claude-haiku-4-5-20251001", "Haiku") },
            HarnessCatalogModelProbe { efforts: strs(&["low", "turbo"]), ..model_probe("claude-next-6", "Next") },
        ],
        &["low", "high", "turbo"],
        &["default", "acceptEdits", "yolo"],
    )
}

fn codex_probe() -> HarnessCatalogProbe {
    plain_probe(
        "0.153.0",
        vec![
            HarnessCatalogModelProbe {
                efforts: strs(&["low", "high"]),
                default_effort: s("low"),
                is_default: true,
                ..model_probe("gpt-5.6-sol", "gpt-5.6-sol")
            },
            HarnessCatalogModelProbe { efforts: strs(&["low", "medium"]), default_effort: s("low"), ..model_probe("gpt-5.5", "gpt-5.5") },
        ],
        &["low", "medium", "high"],
        &["read-only"],
    )
}

fn roundtrip(c: &HarnessCatalog) {
    let back: HarnessCatalog = serde_json::from_value(json(c)).expect("the catalog reads back as the wire type");
    assert_eq!(&back, c);
}

fn picks(model: Option<&str>, effort: Option<&str>, mode: Option<&str>) -> StartPicks {
    StartPicks { model: model.map(String::from), effort: effort.map(String::from), permission_mode: mode.map(String::from), fast: None }
}

fn start(catalog: &HarnessCatalog, asked: StartPicks, opens: bool) -> StartPicks {
    start_picks(Some(catalog), &asked, opens, None).unwrap_or_else(|e| panic!("start refused: {}", e.message))
}

fn refusal(catalog: &HarnessCatalog, asked: StartPicks, opens: bool) -> String {
    start_picks(Some(catalog), &asked, opens, None).expect_err("the start is refused").message
}

fn shown(catalog: &HarnessCatalog, value: Option<&str>) -> Option<String> {
    let model = model_of(catalog, value);
    marked_default(&efforts_for(catalog, model.as_ref())).map(|o| o.value.clone())
}

fn default_effort_of(catalog: &HarnessCatalog, m: &HarnessModel) -> Option<String> {
    marked_default(&efforts_for(catalog, Some(m))).map(|o| o.value.clone())
}

fn plain(options: Vec<wsp_wire::types::thread_defaults::Fields<HarnessOption>>) -> Vec<HarnessOption> {
    options.into_iter().map(wsp_wire::types::thread_defaults::Fields::into_inner).collect()
}

fn values(options: &[HarnessOption]) -> Vec<String> {
    options.iter().map(|o| o.value.clone()).collect()
}

fn model_values(models: &[HarnessModel]) -> Vec<String> {
    models.iter().map(|m| m.value.clone()).collect()
}

fn strip_date(value: &str) -> String {
    Regex::new(r"-\d{8}$").unwrap().replace(value, "").into_owned()
}

fn repo_fixture(rel: &str) -> String {
    let path = format!("{}/../../../packages/adapter-claude/test/fixtures/{rel}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!("no recording at {rel}; a bump of the claude pin needs a recorded handshake and model catalog (record one with the probe command)")
    })
}

fn at_most_one_default_and_unique(what: &str, list: &[(String, Option<bool>)]) {
    let defaults = list.iter().filter(|(_, d)| *d == Some(true)).count();
    assert!(defaults <= 1, "{what}: {defaults} defaults");
    let mut seen: Vec<&String> = list.iter().map(|(v, _)| v).collect();
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), list.len(), "{what}: a value repeats");
}

// TS: harness-catalog.test.ts:13
#[test]
fn names_every_harness_the_recipe_collects_each_parsing_as_the_wire_type_with_at_most_one_default_per_picker() {
    let catalogs = harness_catalogs();
    assert_eq!(catalogs.iter().map(|c| c.harness.as_str()).collect::<Vec<_>>(), ["claude", "codex", "gemini", "opencode", "pi", "hermes", "cursor"]);
    for catalog in &catalogs {
        let id = &catalog.harness;
        roundtrip(catalog);
        assert_eq!(catalog.source, HarnessCatalogSource::Table, "{id}");
        assert_eq!(catalog.refusal, None, "{id}");
        let opts = |list: &[HarnessOption]| list.iter().map(|o| (o.value.clone(), o.is_default)).collect::<Vec<_>>();
        at_most_one_default_and_unique(&format!("{id} models"), &every_model(catalog).iter().map(|m| (m.value.clone(), m.is_default)).collect::<Vec<_>>());
        at_most_one_default_and_unique(&format!("{id} efforts"), &opts(&catalog.efforts));
        at_most_one_default_and_unique(&format!("{id} contextWindows"), &opts(&catalog.context_windows));
        at_most_one_default_and_unique(&format!("{id} permissionModes"), &opts(&catalog.permission_modes));
        assert!(!catalog.legacy_models.iter().flatten().any(|m| m.is_default == Some(true)), "{id} legacy default");
    }
}

// TS: harness-catalog.test.ts:27
#[test]
fn every_agent_wsp_can_run_stands_in_with_its_own_models_pin_and_binary_in_the_footer_never_another_agents() {
    let pin = Regex::new(r"^[^,]+ \d+\.\d+\.\d+(-\w+)?, \d{4}-\d{2}-\d{2}$").unwrap();
    for id in THREAD_AGENTS {
        let table = table(id);
        assert_eq!(!table.models.is_empty(), !OPEN_MODELS.contains(&id), "{id}");
        let version = table.version.clone().unwrap_or_else(|| panic!("{id} has no pin"));
        assert!(pin.is_match(&version), "{id}: {version}");
        let line = catalog_source_line(&table, THIS_COMPUTER);
        assert_eq!(line, format!("{id} table, {version}"));
        assert!(line.chars().count() <= 48, "{line}");
        for other in THREAD_AGENTS.iter().filter(|a| **a != id) {
            assert!(!line.contains(other), "{line} names {other}");
            assert!(!no_models_line(&table).contains(other), "{} names {other}", no_models_line(&table));
        }
    }
}

// TS: harness-catalog.test.ts:46
#[test]
fn the_pin_is_what_was_run_on_the_rows_own_binary_and_a_row_written_from_a_clis_docs_claims_none() {
    assert_eq!(table("claude").version.as_deref(), Some("--help 2.1.296, 2026-10-10"));
    assert_eq!(table("codex").version.as_deref(), Some("app-server 0.153.0, 2026-09-07"));
    assert_eq!(catalog_source_line(&table("codex"), THIS_COMPUTER), "codex table, app-server 0.153.0, 2026-09-07");
    assert_eq!(catalog_source_line(&table("opencode"), THIS_COMPUTER), "opencode table, run --help 1.18.18, 2026-09-27");
    assert_eq!(catalog_source_line(&table("cursor"), THIS_COMPUTER), "cursor table, -h 2026.09.26-dd393fe, 2026-09-27");
    for id in ["gemini", "pi", "hermes"] {
        assert_eq!(table(id).version, None, "{id}");
        assert_eq!(catalog_source_line(&table(id), THIS_COMPUTER), format!("{id} table"));
    }
}

// TS: harness-catalog.test.ts:58
#[test]
fn the_claude_row_is_the_pinned_binarys_own_handshake_so_a_bump_of_the_pin_fails_here() {
    let version = CLAUDE_CODE.version;
    let recorded = parse_catalog_probe(&repo_fixture(&format!("catalog-probe-{version}.txt"))).expect("the recording parses");
    assert_eq!(recorded.version.as_deref(), Some(version));
    let row = table("claude");
    assert!(row.version.as_deref().unwrap().contains(&format!(" {version}, ")), "{:?}", row.version);
    let heard = catalog_from_probe(&row, &recorded);
    let models = |c: &HarnessCatalog| {
        c.models.iter().map(|m| (m.value.clone(), m.is_default == Some(true), m.efforts.clone(), m.context_windows.clone())).collect::<Vec<_>>()
    };
    assert_eq!(models(&row), models(&heard));
    assert_eq!(values(&row.efforts), values(&heard.efforts));
    let mut row_modes = values(&row.permission_modes);
    let mut heard_modes = values(&heard.permission_modes);
    row_modes.sort();
    row_modes.dedup();
    heard_modes.sort();
    heard_modes.dedup();
    assert_eq!(row_modes, heard_modes);
    assert!(model_values(&row.models).contains(&row.small_model.clone().expect("the claude row names a small model")));
}

struct Recorded {
    entries: Vec<Value>,
}

impl Recorded {
    fn read() -> Recorded {
        let text = repo_fixture(&format!("model-catalog-{}.json", CLAUDE_CODE.version));
        Recorded { entries: serde_json::from_str(&text).expect("the model catalog recording is a JSON array") }
    }

    fn of(&self, value: &str) -> Option<&Value> {
        let family = strip_date(value);
        self.entries.iter().find(|e| e["id"] == value || e["id"] == family.as_str())
    }

    fn caps(entry: &Value) -> Vec<String> {
        entry["capabilities"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect()
    }

    fn levels(entry: &Value) -> Vec<String> {
        let caps = Self::caps(entry);
        let has = |c: &str| caps.iter().any(|x| x == c);
        let mut out = vec![];
        if has("effort") {
            out.extend(["low", "medium", "high"].map(String::from));
        }
        if has("xhigh_effort") {
            out.push("xhigh".into());
        }
        if has("max_effort") {
            out.push("max".into());
        }
        out
    }

    fn default_effort(entry: &Value) -> Option<String> {
        entry.get("default_effort").and_then(|v| v.as_str()).map(String::from)
    }
}

// TS: harness-catalog.test.ts:85
#[test]
fn each_current_claude_model_starts_at_the_effort_the_table_gives_it_but_opus_5_5_which_starts_at_the_high_the_cli_documents() {
    let recorded = Recorded::read();
    let row = table("claude");
    let starts_at = |value: &str| shown(&row, Some(value));
    let differs: Vec<(String, Option<String>, Option<String>)> = row
        .models
        .iter()
        .filter_map(|m| {
            let entry = recorded.of(&m.value).unwrap_or_else(|| panic!("{} is not in the recording", m.value));
            let own = Recorded::default_effort(entry);
            let start = starts_at(&m.value);
            (start != own.clone().or(start.clone())).then(|| (m.value.clone(), start, own))
        })
        .collect();
    assert_eq!(differs, vec![("claude-opus-5-5".to_string(), s("high"), s("medium"))]);
    let row_efforts: Vec<Option<Vec<String>>> = row.models.iter().map(|m| m.efforts.clone()).collect();
    let recorded_efforts: Vec<Option<Vec<String>>> = row.models.iter().map(|m| Some(Recorded::levels(recorded.of(&m.value).unwrap()))).collect();
    assert_eq!(row_efforts, recorded_efforts);
}

// TS: harness-catalog.test.ts:97
#[test]
fn haiku_4_5_takes_no_window_though_the_table_takes_its_1m_suffix_its_window_there_is_200k_with_no_1m_beta() {
    let recorded = Recorded::read();
    let haiku = recorded.of("claude-haiku-4-5-20251001").expect("haiku is in the recording");
    assert_eq!(haiku["context"], jv!({ "window": 200_000, "supports_1m_suffix": true }));
    assert_eq!(model_of(&table("claude"), Some("claude-haiku-4-5-20251001")).unwrap().context_windows, Some(vec![]));
}

// TS: harness-catalog.test.ts:103
#[test]
fn the_legacy_rows_are_the_tables_so_a_bump_of_the_pin_fails_here_until_a_recording_and_the_rows_move_with_it() {
    let recorded = Recorded::read();
    let claude = table("claude");
    let current: Vec<Option<Value>> = claude.models.iter().map(|m| recorded.of(&m.value).map(|e| e["id"].clone())).collect();
    let rows: Vec<Value> = claude
        .legacy_models
        .as_ref()
        .expect("the claude row has legacy models")
        .iter()
        .map(|m| {
            let mut row = json(m);
            row["value"] = Value::String(strip_date(&m.value));
            row
        })
        .collect();
    let wanted: Vec<Value> = recorded
        .entries
        .iter()
        .filter(|e| !current.contains(&Some(e["id"].clone())))
        .map(|e| {
            let id = e["id"].as_str().unwrap();
            let one_m = e["context"]["supports_1m_suffix"] == true && id != "claude-haiku-4-5";
            let mut row = jv!({
                "value": id,
                "label": e["display_name"],
                "efforts": Recorded::levels(e),
                "contextWindows": if one_m { jv!(["200k", "1m"]) } else { jv!([]) },
            });
            if let Some(d) = Recorded::default_effort(e) {
                row["defaultEffort"] = Value::String(d);
            }
            if id.starts_with("claude-opus-") {
                row["fast"] = Value::Bool(true);
            }
            row
        })
        .collect();
    assert_eq!(rows, wanted);
}

// TS: harness-catalog.test.ts:123
#[test]
fn codex_offers_the_models_and_reasoning_efforts_its_app_server_reports_and_no_context_window() {
    let codex = table("codex");
    assert_eq!(model_values(&codex.models), ["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]);
    assert_eq!(model_values(codex.legacy_models.as_deref().unwrap()), ["gpt-5.5", "gpt-5.2"]);
    assert_eq!(codex.models.iter().find(|o| o.is_default == Some(true)).map(|m| m.value.as_str()), Some("gpt-5.6-sol"));
    let all = every_model(&codex);
    let six = &["low", "medium", "high", "xhigh", "max", "ultra"];
    let five = &["low", "medium", "high", "xhigh", "max"];
    let four = &["low", "medium", "high", "xhigh"];
    assert_eq!(all.iter().map(|m| m.efforts.clone()).collect::<Vec<_>>(), [strs(six), strs(six), strs(five), strs(four), strs(four)]);
    assert_eq!(all.iter().map(|m| m.default_effort.clone()).collect::<Vec<_>>(), [s("low"), s("medium"), s("medium"), s("medium"), s("medium")]);
    assert_eq!(smallest_model(Some(&codex)), s("gpt-5.2"));
    assert_eq!(values(&codex.efforts), six.map(String::from));
    assert_eq!(codex.efforts.iter().find(|o| o.is_default == Some(true)).map(|o| o.value.as_str()), Some("low"));
    assert_eq!(codex.context_windows, vec![]);
    assert_eq!(values(&codex.permission_modes), ["read-only", "workspace-write", "danger-full-access"]);
}

// TS: harness-catalog.test.ts:146
#[test]
fn claude_code_offers_the_models_effort_levels_context_windows_and_permission_modes_its_cli_takes() {
    let claude = table("claude");
    assert_eq!(model_values(&claude.models), ["claude-opus-5-5", "claude-fable-5-1", "claude-sonnet-5-5", "claude-haiku-5-5"]);
    assert_eq!(claude.models.iter().find(|o| o.is_default == Some(true)).map(|m| m.value.as_str()), Some("claude-opus-5-5"));
    assert_eq!(claude.models.iter().map(|m| m.context_windows.clone()).collect::<Vec<_>>(), vec![Some(vec![]); 4]);
    assert_eq!(claude.models.iter().map(|m| m.efforts.as_ref().map(Vec::len)).collect::<Vec<_>>(), vec![Some(5); 4]);
    assert_eq!(
        model_values(claude.legacy_models.as_deref().unwrap()),
        [
            "claude-opus-5",
            "claude-opus-4-8",
            "claude-opus-4-7",
            "claude-opus-4-6",
            "claude-opus-4-5",
            "claude-fable-5",
            "claude-sonnet-5",
            "claude-sonnet-4-6",
            "claude-sonnet-4-5",
            "claude-haiku-4-5-20251001"
        ]
    );
    assert_eq!(values(&claude.efforts), ["low", "medium", "high", "xhigh", "max"]);
    assert_eq!(claude.efforts.iter().find(|o| o.is_default == Some(true)).map(|o| o.value.as_str()), Some("high"));
    assert_eq!(
        claude.context_windows,
        vec![option("200k", "200k"), HarnessOption { is_default: Some(true), ..option("1m", "1M") }]
    );
    assert_eq!(values(&claude.permission_modes), ["default", "acceptEdits", "bypassPermissions", "auto", "manual", "dontAsk"]);
    for mode in &claude.permission_modes {
        assert!(mode.description.is_some(), "{} has no description", mode.value);
    }
}

// TS: harness-catalog.test.ts:163
#[test]
fn a_cli_without_a_flag_or_with_open_values_leaves_that_list_empty() {
    assert_eq!(table("gemini").efforts, vec![]);
    assert_eq!(table("pi").permission_modes, vec![]);
    assert_eq!(table("opencode").efforts, vec![]);
    assert_eq!(harness_catalog("aider"), None);
}

// TS: harness-catalog.test.ts:172
#[test]
fn models_named_and_never_offered_are_none_an_agents_table_lists() {
    let listed: Vec<String> = harness_catalogs().iter().flat_map(|c| every_model(c)).map(|m| m.value).collect();
    let both: Vec<String> = named_only().iter().filter(|m| listed.contains(&m.value)).map(|m| m.value.clone()).collect();
    assert_eq!(both, Vec::<String>::new());
    for m in named_only() {
        assert_eq!(model_label(&m.value), m.label);
    }
}

// TS: harness-catalog.test.ts:184
#[test]
fn the_default_effort_of_a_pick_is_the_picked_models_own() {
    let codex = table("codex");
    assert_eq!(shown(&codex, Some("gpt-5.5")), s("medium"));
    assert_eq!(shown(&codex, Some("gpt-5.6-terra")), s("medium"));
    assert_eq!(shown(&codex, Some("gpt-5.6-sol")), s("low"));
    let marked: Vec<String> =
        efforts_for(&codex, model_of(&codex, Some("gpt-5.5")).as_ref()).into_iter().filter(|o| o.is_default == Some(true)).map(|o| o.value.clone()).collect();
    assert_eq!(marked, ["medium"]);
}

// TS: harness-catalog.test.ts:192
#[test]
fn the_default_effort_is_the_catalogs_mark_for_a_model_that_names_none_and_for_no_model_at_all() {
    let claude = table("claude");
    let codex = table("codex");
    let marked_model = marked_default(&claude.models).cloned();
    assert_eq!(marked_default(&efforts_for(&claude, marked_model.as_ref())).map(|o| o.value.as_str()), Some("high"));
    assert_eq!(marked_default(&efforts_for(&claude, None)).map(|o| o.value.as_str()), Some("high"));
    assert_eq!(claude.models.iter().map(|m| default_effort_of(&claude, m)).collect::<Vec<_>>(), [s("high"), s("high"), s("medium"), s("medium")]);
    assert_eq!(shown(&claude, Some("claude-haiku-4-5-20251001")), None);
    let sonnet = model_of(&claude, Some("claude-sonnet-4-6")).unwrap();
    assert_eq!(sonnet.label, "Sonnet 4.6");
    assert_eq!(sonnet.context_windows, strs(&["200k", "1m"]));
    let routed = model("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5");
    assert_eq!(default_effort_of(&codex, &routed), s("low"));
    assert_eq!(shown(&codex, None), s("low"));
}

// TS: harness-catalog.test.ts:205
#[test]
fn marks_nothing_when_the_model_names_a_default_its_own_list_does_not_carry() {
    let odd_model = HarnessModel { efforts: strs(&["low", "high"]), default_effort: s("medium"), ..model("gpt-5.5", "GPT-5.5") };
    let odd = HarnessCatalog { models: vec![odd_model.clone()], ..table("codex") };
    let efforts = plain(efforts_for(&odd, Some(&odd_model)));
    assert_eq!(values(&efforts), ["low", "high"]);
    assert!(!efforts.iter().any(|o| o.is_default == Some(true)));
}

// TS: harness-catalog.test.ts:212
#[test]
fn tool_asks_never_at_its_bypass_or_with_no_mode_named_else_before_every_tool_or_before_one_not_read_only() {
    let asks = |h: Option<&str>, m: Option<&str>| tool_asks(h, m).as_str();
    assert_eq!(asks(Some("claude"), None), "never");
    assert_eq!(asks(Some("claude"), Some("bypassPermissions")), "never");
    assert_eq!(asks(Some("claude"), Some("default")), "every");
    assert_eq!(asks(Some("claude"), Some("acceptEdits")), "every");
    assert_eq!(asks(Some("codex"), Some("danger-full-access")), "never");
    assert_eq!(asks(Some("codex"), Some("workspace-write")), "unless-read-only");
    assert_eq!(asks(Some("codex"), Some("read-only")), "unless-read-only");
    assert_eq!(asks(Some("opencode"), table("opencode").bypass_mode.as_deref()), "never");
    assert_eq!(asks(None, Some("default")), "every");
    assert_eq!(asks(Some("nobody"), Some("default")), "every");
}

// TS: harness-catalog.test.ts:232
#[test]
fn an_access_mode_says_what_the_agent_may_do_naming_no_agent_and_nothing_inside_one() {
    let mut words: Vec<Regex> = ["classifier", r"\bCLIs?\b", "print mode", r"\bmachines?\b"].iter().map(|w| Regex::new(&format!("(?i){w}")).unwrap()).collect();
    for c in harness_catalogs() {
        words.push(Regex::new(&format!(r"(?i)\b{}\b", regex::escape(&c.harness))).unwrap());
        words.push(Regex::new(&format!(r"(?i)\b{}\b", regex::escape(&c.label))).unwrap());
    }
    let modes: Vec<(String, Option<String>)> =
        harness_catalogs().iter().flat_map(|c| c.permission_modes.iter().map(|o| (format!("{} {}", c.harness, o.value), o.description.clone())).collect::<Vec<_>>()).collect();
    assert!(modes.len() > 15, "{} modes", modes.len());
    for (where_, description) in modes {
        let description = description.unwrap_or_else(|| panic!("{where_} has no description"));
        for word in &words {
            assert!(!word.is_match(&description), "{where_} against {word}: {description}");
        }
        assert!(!description.contains('.'), "{where_}: {description}");
    }
}

// TS: harness-catalog.test.ts:243
#[test]
fn the_two_modes_a_person_could_not_read_now_read_as_a_person_would_say_them() {
    let claude = table("claude");
    let mode = |value: &str| claude.permission_modes.iter().find(|o| o.value == value).and_then(|o| o.description.clone());
    assert_eq!(mode("auto"), s("The agent decides which actions to ask about; where the account has no such mode it asks as Default does"));
    assert_eq!(mode("manual"), s("Asks before every action"));
}

// TS: harness-catalog.test.ts:252
#[test]
fn every_harness_with_an_access_mode_names_the_mode_that_asks_nothing_and_marks_it() {
    let catalogs = harness_catalogs();
    for catalog in &catalogs {
        let id = &catalog.harness;
        if catalog.permission_modes.is_empty() {
            assert_eq!(catalog.bypass_mode, None, "{id}");
            continue;
        }
        let bypass = catalog.bypass_mode.clone().unwrap_or_else(|| panic!("{id} names no bypass mode"));
        assert!(values(&catalog.permission_modes).contains(&bypass), "{id}");
        assert_eq!(marked_default(&catalog.permission_modes).map(|o| o.value.clone()), Some(bypass), "{id}");
        assert_eq!(catalog.permission_modes.iter().filter(|o| o.is_default == Some(true)).count(), 1, "{id}");
    }
    let named: Vec<(String, Option<String>)> =
        catalogs.iter().filter(|c| !c.permission_modes.is_empty()).map(|c| (c.harness.clone(), c.bypass_mode.clone())).collect();
    let wanted = [("claude", "bypassPermissions"), ("codex", "danger-full-access"), ("gemini", "yolo"), ("opencode", "auto"), ("hermes", "yolo"), ("cursor", "force")];
    assert_eq!(named, wanted.map(|(h, m)| (h.to_string(), s(m))));
}

// TS: harness-catalog.test.ts:272
#[test]
fn each_agent_with_an_adapter_maps_wsps_words_to_modes_it_lists_and_a_word_it_cannot_honour_maps_to_none() {
    let catalogs = harness_catalogs();
    let access: Value = Value::Object(catalogs.iter().filter(|c| c.access.is_some()).map(|c| (c.harness.clone(), json(&c.access))).collect());
    assert_eq!(
        access,
        jv!({
            "claude": { "ask": "default", "auto-edit": "acceptEdits", "full": "bypassPermissions" },
            "codex": { "ask": "workspace-write", "full": "danger-full-access", "plan": "read-only" },
            "opencode": { "full": "auto" },
            "cursor": { "full": "force", "plan": "default" },
        })
    );
    for catalog in &catalogs {
        let modes = values(&catalog.permission_modes);
        let words = json(&catalog.access);
        for mode in words.as_object().into_iter().flat_map(|o| o.values()) {
            assert!(modes.contains(&mode.as_str().unwrap().to_string()), "{}: {mode}", catalog.harness);
        }
        if let Some(full) = words.get("full") {
            assert_eq!(full.as_str(), catalog.bypass_mode.as_deref(), "{}", catalog.harness);
        }
    }
}

// TS: harness-catalog.test.ts:294
#[test]
fn start_picks_a_start_that_opens_a_thread_without_a_pick_runs_every_default_the_composer_shows() {
    let claude = table("claude");
    assert_eq!(start(&claude, StartPicks::default(), true), picks(Some("claude-opus-5-5"), Some("high"), Some("bypassPermissions")));
    assert_eq!(start(&claude, StartPicks::default(), false), StartPicks::default());
    assert_eq!(
        start(&claude, picks(Some("claude-sonnet-5"), Some("low"), Some("acceptEdits")), true),
        picks(Some("claude-sonnet-5"), Some("low"), Some("acceptEdits"))
    );
    assert_eq!(start(&claude, picks(None, Some("max"), None), false), picks(None, Some("max"), None));
    let no_default = HarnessCatalog { models: claude.models.iter().cloned().map(|m| HarnessModel { is_default: None, ..m }).collect(), ..claude.clone() };
    assert_eq!(start(&no_default, StartPicks::default(), true), picks(None, Some("high"), Some("bypassPermissions")));
}

// TS: harness-catalog.test.ts:306
#[test]
fn start_picks_fast_rides_a_start_on_a_model_that_offers_it_is_refused_naming_one_that_does_not() {
    let claude = table("claude");
    let fast = |model: Option<&str>| StartPicks { model: model.map(String::from), fast: Some(true), ..Default::default() };
    assert_eq!(
        start(&claude, fast(None), true),
        StartPicks { fast: Some(true), ..picks(Some("claude-opus-5-5"), Some("high"), Some("bypassPermissions")) }
    );
    assert_eq!(start(&claude, StartPicks { fast: Some(false), ..Default::default() }, true).fast, None);
    assert!(refusal(&claude, fast(Some("claude-haiku-4-5-20251001")), true).contains("Haiku 4.5 has no fast mode"));
    assert_eq!(start(&claude, fast(None), false), StartPicks { fast: Some(true), ..Default::default() });
    assert_eq!(start_picks(Some(&claude), &fast(None), false, Some("claude-opus-5-5")).unwrap(), StartPicks { fast: Some(true), ..Default::default() });
    let refused = start_picks(Some(&claude), &fast(None), false, Some("claude-haiku-4-5-20251001")).expect_err("haiku has no fast mode");
    assert!(refused.message.contains("Haiku 4.5 has no fast mode"), "{}", refused.message);
}

// TS: harness-catalog.test.ts:316
#[test]
fn start_picks_listed_pick_keeps_a_remembered_pick_this_list_carries_and_drops_one_it_does_not() {
    let claude = table("claude");
    assert_eq!(listed_pick(&claude.permission_modes, Some("acceptEdits")), s("acceptEdits"));
    assert_eq!(listed_pick(&claude.permission_modes, Some("read-only")), None);
    assert_eq!(listed_pick(&claude.permission_modes, None), None);
    assert_eq!(listed_pick(&table("pi").permission_modes, Some("acceptEdits")), None);
    let dropped = picks(None, None, listed_pick(&claude.permission_modes, Some("read-only")).as_deref());
    assert_eq!(start(&claude, dropped, true).permission_mode, s("bypassPermissions"));
    assert!(refusal(&claude, picks(None, None, Some("read-only")), true).contains("not one claude takes"));
}

// TS: harness-catalog.test.ts:329
#[test]
fn start_picks_refuses_a_value_the_catalog_does_not_list_naming_the_list_in_the_composers_words() {
    let claude = table("claude");
    assert!(
        refusal(&claude, picks(Some("claude-opus-4-1"), None, None), true)
            .contains(&format!("model \"claude-opus-4-1\" is not one claude takes; one of: {MODELS}"))
    );
    assert!(refusal(&claude, picks(None, Some("ultra"), None), false)
        .contains("effort \"ultra\" is not one claude takes; one of: Low (low), Medium (medium), High (high), Extra high (xhigh), Max (max)"));
    assert!(refusal(&claude, picks(None, None, Some("yolo")), true).contains(
        "access mode \"yolo\" is not one claude takes; one of: Default (default), Accept edits (acceptEdits), Bypass (bypassPermissions), Auto (auto), Manual (manual), Don't ask (dontAsk)"
    ));
    let narrowed = narrowed_catalog();
    let unlisted = start_picks(Some(&claude), &picks(Some("claude-opus-4-1"), None, None), true, None).unwrap_err();
    assert_eq!(unlisted.kind, "usage");
    let no_effort = start_picks(Some(&narrowed), &picks(Some("claude-haiku-4-5"), Some("high"), None), true, None).unwrap_err();
    assert_eq!(no_effort.kind, "usage");
}

fn narrowed_catalog() -> HarnessCatalog {
    HarnessCatalog {
        models: vec![
            HarnessModel { is_default: Some(true), efforts: strs(&["high", "max"]), ..model("claude-opus-5", "Opus 5") },
            HarnessModel { efforts: strs(&[]), ..model("claude-haiku-4-5", "Haiku") },
        ],
        ..table("claude")
    }
}

// TS: harness-catalog.test.ts:348
#[test]
fn start_picks_a_model_that_takes_no_effort_at_all_is_given_none_and_one_that_takes_some_runs_the_default_its_own_list_carries() {
    let narrowed = narrowed_catalog();
    assert_eq!(start(&narrowed, picks(Some("claude-haiku-4-5"), None, None), true), picks(Some("claude-haiku-4-5"), None, Some("bypassPermissions")));
    assert_eq!(start(&narrowed, StartPicks::default(), true), picks(Some("claude-opus-5"), Some("high"), Some("bypassPermissions")));
    let off = HarnessCatalog {
        models: vec![HarnessModel { is_default: Some(true), efforts: strs(&["low", "max"]), ..model("claude-opus-5", "Opus 5") }],
        ..table("claude")
    };
    assert_eq!(start(&off, StartPicks::default(), true), picks(Some("claude-opus-5"), None, Some("bypassPermissions")));
}

// TS: harness-catalog.test.ts:356
#[test]
fn start_picks_an_effort_is_checked_against_the_models_own_list_where_the_model_has_one() {
    let narrowed = narrowed_catalog();
    assert_eq!(start(&narrowed, picks(None, Some("max"), None), true), picks(Some("claude-opus-5"), Some("max"), Some("bypassPermissions")));
    assert!(refusal(&narrowed, picks(None, Some("low"), None), true).contains("effort \"low\" is not one Opus 5 takes; one of: High (high), Max (max)"));
    assert!(refusal(&narrowed, picks(Some("claude-haiku-4-5"), Some("low"), None), true).contains("Haiku takes no effort"));
    assert_eq!(start(&narrowed, picks(None, Some("low"), None), false), picks(None, Some("low"), None));
}

// TS: harness-catalog.test.ts:363
#[test]
fn start_picks_a_list_the_cli_leaves_empty_takes_any_value_since_the_values_are_open_or_the_flag_does_not_exist() {
    let gemini = table("gemini");
    assert_eq!(start(&gemini, picks(Some("gemini-3-pro"), None, None), true), picks(Some("gemini-3-pro"), None, Some("yolo")));
    assert_eq!(start(&gemini, picks(None, Some("anything"), None), false), picks(None, Some("anything"), None));
    let pi = table("pi");
    assert_eq!(start(&pi, picks(None, None, Some("anything")), true), picks(None, None, Some("anything")));
}

// TS: harness-catalog.test.ts:371
#[test]
fn start_picks_a_start_on_the_codex_table_runs_its_default_model_and_refuses_a_model_or_effort_that_table_does_not_carry() {
    let codex = table("codex");
    assert_eq!(start(&codex, StartPicks::default(), true), picks(Some("gpt-5.6-sol"), Some("low"), Some("danger-full-access")));
    let marked_model = marked_default(&codex.models).cloned();
    assert_eq!(marked_default(&efforts_for(&codex, marked_model.as_ref())).map(|o| o.value.as_str()), Some("low"));
    assert_eq!(
        start(&codex, picks(Some("gpt-5.5"), Some("high"), Some("read-only")), true),
        picks(Some("gpt-5.5"), Some("high"), Some("read-only"))
    );
    assert_eq!(start(&codex, picks(Some("gpt-5.5"), None, None), true), picks(Some("gpt-5.5"), Some("medium"), Some("danger-full-access")));
    assert_eq!(start(&codex, picks(Some("gpt-5.5"), None, None), false), picks(Some("gpt-5.5"), None, None));
    assert!(refusal(&codex, picks(Some("gpt-4"), None, None), true).contains("model \"gpt-4\" is not one codex takes"));
    assert!(refusal(&codex, picks(Some("gpt-5.5"), Some("ultra"), None), true).contains("effort \"ultra\" is not one GPT-5.5 takes"));
}

// TS: harness-catalog.test.ts:383
#[test]
fn start_picks_a_start_on_a_legacy_model_runs_that_model_at_the_levels_the_binary_lists_for_it() {
    let claude = table("claude");
    assert_eq!(start(&claude, picks(Some("claude-sonnet-5-5"), None, None), true), picks(Some("claude-sonnet-5-5"), Some("medium"), Some("bypassPermissions")));
    assert_eq!(
        start(&claude, picks(Some("claude-opus-5"), Some("high"), None), true),
        picks(Some("claude-opus-5"), Some("high"), Some("bypassPermissions"))
    );
    assert_eq!(start(&claude, picks(Some("claude-opus-4-7"), None, None), true).effort, s("xhigh"));
    assert_eq!(start(&claude, picks(Some("claude-opus-4-6"), None, None), true).effort, s("high"));
    assert!(refusal(&claude, picks(Some("claude-opus-4-6"), Some("xhigh"), None), true).contains("effort \"xhigh\" is not one Opus 4.6 takes"));
    assert_eq!(start(&claude, picks(Some("claude-sonnet-4-5"), None, None), true), picks(Some("claude-sonnet-4-5"), None, Some("bypassPermissions")));
    assert!(refusal(&claude, picks(Some("claude-sonnet-4-5"), Some("high"), None), true).contains("Sonnet 4.5 takes no effort"));
    assert!(refusal(&claude, picks(Some("claude-opus-4-1"), None, None), true).contains(&format!(
        "model \"claude-opus-4-1\" is not one claude takes; one of: {MODELS}; legacy: Opus 5 (claude-opus-5), Opus 4.8 (claude-opus-4-8),"
    )));
}

// TS: harness-catalog.test.ts:398
#[test]
fn start_picks_a_harness_without_a_catalog_takes_any_value_and_fills_no_default() {
    let claude = table("claude");
    let asked = picks(Some("gpt-9"), Some("high"), None);
    assert_eq!(start_picks(None, &asked, true, None).unwrap(), picks(Some("gpt-9"), Some("high"), None));
    let sonnet = picks(Some("claude-sonnet-5"), Some("high"), None);
    assert_eq!(start(&claude, sonnet, false), picks(Some("claude-sonnet-5"), Some("high"), None));
}

// TS: harness-catalog.test.ts:406
#[test]
fn fast_and_plan_in_the_tables_mark_the_models_that_offer_faster_output() {
    let claude = table("claude");
    assert_eq!(claude.models.iter().filter(|m| m.fast == Some(true)).map(|m| m.value.as_str()).collect::<Vec<_>>(), ["claude-opus-5-5"]);
    assert_eq!(model_of(&claude, Some("claude-opus-4-6")).unwrap().fast, Some(true));
    assert_eq!(model_of(&claude, Some("claude-sonnet-5")).unwrap().fast, None);
    let astra = plain_probe(
        "0.155.1",
        vec![
            HarnessCatalogModelProbe { is_default: true, fast: Some(true), ..model_probe("gpt-6-astra", "GPT-6-Astra") },
            model_probe("gpt-5.2", "GPT-5.2"),
        ],
        &[],
        &["read-only", "workspace-write", "danger-full-access"],
    );
    let heard = catalog_from_probe(&table("codex"), &astra);
    assert_eq!(every_model(&heard).iter().map(|m| (m.value.clone(), m.fast)).collect::<Vec<_>>(), [("gpt-6-astra".to_string(), Some(true)), ("gpt-5.2".to_string(), None)]);
    let bare = plain_probe("2.1.283", vec![HarnessCatalogModelProbe { is_default: true, ..model_probe("claude-opus-5-5", "Opus 5.5") }], &[], &[]);
    assert_eq!(catalog_from_probe(&claude, &bare).models[0].fast, Some(true));
}

// TS: harness-catalog.test.ts:431
#[test]
fn catalog_from_probe_takes_the_binarys_values_and_defaults_borrows_the_tables_labels_and_says_the_binary_answered() {
    let claude = table("claude");
    let catalog = catalog_from_probe(&claude, &probe());
    roundtrip(&catalog);
    assert_eq!(catalog.harness, "claude");
    assert_eq!(catalog.label, "Claude Code");
    assert_eq!(catalog.source, HarnessCatalogSource::Harness);
    assert_eq!(catalog.version, s("2.1.257"));
    assert_eq!(
        catalog.models,
        vec![
            HarnessModel {
                description: s("Opus 5.5 with 1M context"),
                is_default: Some(true),
                efforts: strs(&["low", "high"]),
                context_windows: strs(&["200k", "1m"]),
                fast: Some(true),
                ..model("claude-opus-5-5", "Opus 5.5")
            },
            HarnessModel { efforts: strs(&[]), context_windows: strs(&[]), ..model("claude-haiku-4-5-20251001", "Haiku 4.5") },
            HarnessModel { efforts: strs(&["low", "turbo"]), context_windows: strs(&[]), ..model("claude-next-6", "Next") },
        ]
    );
    assert_eq!(
        catalog.efforts,
        vec![option("low", "Low"), HarnessOption { is_default: Some(true), ..option("high", "High") }, option("turbo", "Turbo")]
    );
    let no_default = catalog_from_probe(&claude, &HarnessCatalogProbe { efforts: words(&["low", "turbo"]), ..probe() });
    assert_eq!(values(&no_default.efforts), ["low", "turbo"]);
    assert!(!no_default.efforts.iter().any(|o| o.is_default == Some(true)));
    assert_eq!(catalog.context_windows, claude.context_windows);
    assert_eq!(values(&catalog.permission_modes), ["default", "acceptEdits", "yolo"]);
    assert_eq!(catalog.permission_modes[1].description, s("Edits files without asking; asks about commands that need permission"));
    assert_eq!(catalog.permission_modes[2], option("yolo", "yolo"));
    assert!(!catalog.permission_modes.iter().any(|o| o.is_default == Some(true)));
}

// TS: harness-catalog.test.ts:452
#[test]
fn catalog_from_probe_keeps_the_tables_legacy_models_on_a_binary_that_lists_none_of_them() {
    let claude = table("claude");
    let catalog = catalog_from_probe(&claude, &probe());
    let wanted: Vec<HarnessModel> = claude.legacy_models.clone().unwrap().into_iter().filter(|m| m.value != "claude-haiku-4-5-20251001").collect();
    assert_eq!(catalog.legacy_models, Some(wanted));
    let mut models = probe().models;
    models.push(HarnessCatalogModelProbe { efforts: strs(&["low"]), ..model_probe("claude-opus-4-8", "Opus") });
    let current = catalog_from_probe(&claude, &HarnessCatalogProbe { models, ..probe() });
    assert!(model_values(&current.models).contains(&"claude-opus-4-8".to_string()));
    assert_eq!(
        model_values(current.legacy_models.as_deref().unwrap()),
        ["claude-opus-5", "claude-opus-4-7", "claude-opus-4-6", "claude-opus-4-5", "claude-fable-5", "claude-sonnet-5", "claude-sonnet-4-6", "claude-sonnet-4-5"]
    );
    assert_eq!(start(&catalog, picks(Some("claude-fable-5"), None, None), false), picks(Some("claude-fable-5"), None, None));
}

// TS: harness-catalog.test.ts:463
#[test]
fn catalog_from_probe_keeps_a_legacy_model_under_the_fold_in_the_binarys_own_words_and_drops_one_it_leaves_out() {
    let codex = table("codex");
    let catalog = catalog_from_probe(&codex, &codex_probe());
    roundtrip(&catalog);
    assert_eq!(model_values(&catalog.models), ["gpt-5.6-sol"]);
    assert_eq!(
        catalog.legacy_models,
        Some(vec![HarnessModel {
            efforts: strs(&["low", "medium"]),
            default_effort: s("low"),
            context_windows: strs(&[]),
            fast: Some(true),
            ..model("gpt-5.5", "GPT-5.5")
        }])
    );
    assert_eq!(start(&catalog, picks(Some("gpt-5.5"), None, None), true), picks(Some("gpt-5.5"), Some("low"), None));
    assert!(refusal(&catalog, picks(Some("gpt-5.2"), None, None), true).contains("model \"gpt-5.2\" is not one codex takes"));
    assert_eq!(smallest_model(Some(&catalog)), None);
    let models = codex_probe().models.into_iter().map(|m| HarnessCatalogModelProbe { is_default: m.slug == "gpt-5.5", ..m }).collect();
    let older = catalog_from_probe(&codex, &HarnessCatalogProbe { models, ..codex_probe() });
    assert_eq!(model_values(&older.models), ["gpt-5.6-sol", "gpt-5.5"]);
    assert_eq!(older.legacy_models, Some(vec![]));
}

// TS: harness-catalog.test.ts:487
#[test]
fn catalog_from_probe_a_probe_without_a_version_says_so_instead_of_pretending_to_the_tables_pin() {
    let claude = table("claude");
    let versionless = catalog_from_probe(&claude, &HarnessCatalogProbe { version: None, ..probe() });
    assert_eq!(versionless.version, None);
    assert_eq!(catalog_source_line(&versionless, THIS_COMPUTER), "Claude Code on this computer");
    assert_eq!(catalog_source_line(&catalog_from_probe(&claude, &probe()), "hetzner"), "Claude Code 2.1.257 on hetzner");
}

// TS: harness-catalog.test.ts:493
#[test]
fn catalog_from_probe_a_model_whose_efforts_the_binary_did_not_name_keeps_none_of_its_own() {
    let routed_model = HarnessCatalogModelProbe { is_default: true, ..model_probe("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5") };
    let others = probe().models.into_iter().map(|m| HarnessCatalogModelProbe { is_default: false, ..m });
    let routed = HarnessCatalogProbe { models: std::iter::once(routed_model).chain(others).collect(), ..probe() };
    let catalog = catalog_from_probe(&table("codex"), &routed);
    roundtrip(&catalog);
    assert_eq!(
        catalog.models[0],
        HarnessModel { is_default: Some(true), context_windows: strs(&[]), ..model("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5") }
    );
    assert_eq!(catalog.models[0].efforts, None);
    assert_eq!(values(&plain(efforts_for(&catalog, Some(&catalog.models[0])))), ["low", "high", "turbo"]);
    assert_eq!(start(&catalog, picks(None, Some("turbo"), None), true), picks(Some("anthropic/claude-sonnet-4.5"), Some("turbo"), None));
    assert!(refusal(&catalog, picks(None, Some("off"), None), true).contains("effort \"off\" is not one codex takes"));
    let haiku = catalog.models.iter().find(|m| m.value == "claude-haiku-4-5-20251001").unwrap();
    assert_eq!(efforts_for(&catalog, Some(haiku)), Vec::<HarnessOption>::new());
}

// TS: harness-catalog.test.ts:507
#[test]
fn catalog_from_probe_carries_each_models_own_default_effort() {
    let models = probe().models.into_iter().map(|m| if m.slug == "claude-next-6" { HarnessCatalogModelProbe { default_effort: s("turbo"), ..m } } else { m }).collect();
    let catalog = catalog_from_probe(&table("claude"), &HarnessCatalogProbe { models, ..probe() });
    roundtrip(&catalog);
    assert_eq!(catalog.models.iter().map(|m| m.default_effort.clone()).collect::<Vec<_>>(), [None, None, s("turbo")]);
    assert_eq!(default_effort_of(&catalog, &catalog.models[2]), s("turbo"));
    assert_eq!(start(&catalog, picks(Some("claude-next-6"), None, None), true), picks(Some("claude-next-6"), Some("turbo"), None));
    assert_eq!(default_effort_of(&catalog, &catalog.models[0]), s("high"));
}

// TS: harness-catalog.test.ts:518
#[test]
fn catalog_from_probe_marks_the_effort_the_binary_reports_for_the_model_it_would_run() {
    let codex = table("codex");
    let with_default = |effort: &str| {
        let models = probe().models.into_iter().map(|m| if m.is_default { HarnessCatalogModelProbe { default_effort: s(effort), ..m } } else { m }).collect();
        HarnessCatalogProbe { models, ..probe() }
    };
    let turbo = catalog_from_probe(&codex, &with_default("turbo"));
    assert_eq!(turbo.efforts.iter().find(|o| o.is_default == Some(true)).map(|o| o.value.as_str()), Some("turbo"));
    assert_eq!(turbo.efforts.iter().filter(|o| o.is_default == Some(true)).count(), 1);
    let kept = catalog_from_probe(&codex, &probe());
    assert_eq!(kept.efforts.iter().find(|o| o.is_default == Some(true)).map(|o| o.value.as_str()), Some("low"));
    let odd = catalog_from_probe(&codex, &with_default("nope"));
    assert_eq!(values(&odd.efforts), ["low", "high", "turbo"]);
    assert!(!odd.efforts.iter().any(|o| o.is_default == Some(true)));
}

// TS: harness-catalog.test.ts:529
#[test]
fn catalog_from_probe_keeps_the_harness_level_flags_the_runtime_set_on_the_table() {
    let flagged = HarnessCatalog { is_default: Some(true), steers: true, ..table("claude") };
    let heard = catalog_from_probe(&flagged, &probe());
    assert_eq!((heard.is_default, heard.steers), (Some(true), true));
    assert_eq!(catalog_from_probe(&table("claude"), &probe()).is_default, None);
}

// fixture: harness_catalog.json catalogs
#[test]
fn the_seven_table_rows_equal_the_ts_rows() {
    let fx = fixture("harness_catalog");
    assert_eq!(json(&harness_catalogs()), fx["catalogs"]);
}

// fixture: harness_catalog.json probes
#[test]
fn every_probe_reads_into_the_catalog_the_ts_made_of_it() {
    let fx = fixture("harness_catalog");
    let claude = table("claude");
    let codex = table("codex");
    let base = probe();
    let with_default_effort = |effort: &str| -> HarnessCatalogProbe {
        let models = base.models.iter().cloned().map(|m| if m.is_default { HarnessCatalogModelProbe { default_effort: s(effort), ..m } } else { m }).collect();
        HarnessCatalogProbe { models, ..probe() }
    };
    let routed = HarnessCatalogProbe {
        models: std::iter::once(HarnessCatalogModelProbe { is_default: true, ..model_probe("anthropic/claude-sonnet-4.5", "anthropic/claude-sonnet-4.5") })
            .chain(base.models.iter().cloned().map(|m| HarnessCatalogModelProbe { is_default: false, ..m }))
            .collect(),
        ..probe()
    };
    let cases: Vec<(&str, HarnessCatalog, HarnessCatalogProbe)> = vec![
        ("claude+probe", claude.clone(), probe()),
        ("claude+probe, efforts low turbo", claude.clone(), HarnessCatalogProbe { efforts: words(&["low", "turbo"]), ..probe() }),
        (
            "claude+probe+opus-4-8",
            claude.clone(),
            HarnessCatalogProbe {
                models: base.models.iter().cloned().chain([HarnessCatalogModelProbe { efforts: strs(&["low"]), ..model_probe("claude-opus-4-8", "Opus") }]).collect(),
                ..probe()
            },
        ),
        ("codex+codexProbe", codex.clone(), codex_probe()),
        (
            "codex+codexProbe, default gpt-5.5",
            codex.clone(),
            HarnessCatalogProbe {
                models: codex_probe().models.into_iter().map(|m| HarnessCatalogModelProbe { is_default: m.slug == "gpt-5.5", ..m }).collect(),
                ..codex_probe()
            },
        ),
        ("claude+probe, no version", claude.clone(), HarnessCatalogProbe { version: None, ..probe() }),
        ("codex+routed", codex.clone(), routed),
        (
            "claude+perModel",
            claude.clone(),
            HarnessCatalogProbe {
                models: base.models.iter().cloned().map(|m| if m.slug == "claude-next-6" { HarnessCatalogModelProbe { default_effort: s("turbo"), ..m } } else { m }).collect(),
                ..probe()
            },
        ),
        ("codex+probe default turbo", codex.clone(), with_default_effort("turbo")),
        ("codex+probe", codex.clone(), probe()),
        ("codex+probe default nope", codex.clone(), with_default_effort("nope")),
        ("claude flagged+probe", HarnessCatalog { is_default: Some(true), steers: true, ..claude.clone() }, probe()),
        (
            "codex+astra",
            codex.clone(),
            plain_probe(
                "0.155.1",
                vec![
                    HarnessCatalogModelProbe { is_default: true, fast: Some(true), ..model_probe("gpt-6-astra", "GPT-6-Astra") },
                    model_probe("gpt-5.2", "GPT-5.2"),
                ],
                &[],
                &["read-only", "workspace-write", "danger-full-access"],
            ),
        ),
        (
            "claude+bare opus",
            claude.clone(),
            plain_probe("2.1.283", vec![HarnessCatalogModelProbe { is_default: true, ..model_probe("claude-opus-5-5", "Opus 5.5") }], &[], &[]),
        ),
        ("claude+empty", claude.clone(), plain_probe("9.9.9", vec![], &[], &[])),
        ("pi+probe", table("pi"), probe()),
    ];
    let wanted: Vec<&str> = fx["probes"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(cases.iter().map(|c| c.0).collect::<Vec<_>>(), wanted);
    for (case, (name, row, asked)) in cases.iter().enumerate() {
        assert_eq!(json(&catalog_from_probe(row, asked)), fx["probes"][case]["value"], "{name}");
    }
}

// fixture: harness_catalog.json labels
#[test]
fn model_label_reads_every_model_the_way_the_ts_does() {
    let fx = fixture("harness_catalog");
    let pairs = fx["labels"].as_array().unwrap();
    assert!(!pairs.is_empty());
    for pair in pairs {
        let model = pair[0].as_str().unwrap();
        assert_eq!(model_label(model), pair[1].as_str().unwrap(), "{model:?}");
    }
}

// fixture: harness_catalog.json toolAsks
#[test]
fn tool_asks_answers_every_pair_as_the_ts_does() {
    let fx = fixture("harness_catalog");
    let triples = fx["toolAsks"].as_array().unwrap();
    assert!(!triples.is_empty());
    for triple in triples {
        let (harness, mode) = (triple[0].as_str(), triple[1].as_str());
        assert_eq!(tool_asks(harness, mode).as_str(), triple[2].as_str().unwrap(), "{harness:?} {mode:?}");
    }
}

// fixture: harness_catalog.json smallest
#[test]
fn smallest_model_answers_every_row_as_the_ts_does() {
    let fx = fixture("harness_catalog");
    let claude = table("claude");
    let codex = table("codex");
    let lists = catalog_from_probe(&codex, &codex_probe());
    let gemini = table("gemini");
    let cases: Vec<(&str, Option<&HarnessCatalog>)> =
        vec![("claude", Some(&claude)), ("codex", Some(&codex)), ("gemini", Some(&gemini)), ("none", None), ("codex+codexProbe", Some(&lists))];
    let wanted = fx["smallest"].as_array().unwrap();
    assert_eq!(wanted.len(), cases.len());
    for (case, (name, catalog)) in cases.into_iter().enumerate() {
        assert_eq!(wanted[case][0], name);
        assert_eq!(json(&smallest_model(catalog)), wanted[case][1], "{name}");
    }
}

// fixture: harness_catalog.json notRun
#[test]
fn not_run_refused_answers_every_pair_as_the_ts_does() {
    let fx = fixture("harness_catalog");
    let codex = table("codex");
    let lists = catalog_from_probe(&codex, &codex_probe());
    let message = |catalog: Option<&HarnessCatalog>, model: Option<&str>| json(&not_run_refused(catalog, model).map(|e| e.message));
    let cases: Vec<(&str, Value)> = vec![
        ("none", message(None, Some("gpt-5.5"))),
        ("codex lists, gpt-5.2", message(Some(&lists), Some("gpt-5.2"))),
        ("codex lists, gpt-5.5", message(Some(&lists), Some("gpt-5.5"))),
        ("codex lists, no model", message(Some(&lists), None)),
        ("codex lists, unknown", message(Some(&lists), Some("gpt-9"))),
        ("codex table, gpt-5.2", message(Some(&codex), Some("gpt-5.2"))),
    ];
    let wanted = fx["notRun"].as_array().unwrap();
    assert_eq!(wanted.len(), cases.len());
    for (case, (name, got)) in cases.into_iter().enumerate() {
        assert_eq!(wanted[case][0], name);
        assert_eq!(got, wanted[case][1], "{name}");
    }
}

// fixture: harness_catalog.json namedOnly
#[test]
fn named_only_is_the_ts_list() {
    let fx = fixture("harness_catalog");
    assert_eq!(json(&named_only()), fx["namedOnly"]);
}

// fixture: harness_catalog.json unknown
#[test]
fn an_unknown_harness_has_no_row_as_in_the_ts() {
    let fx = fixture("harness_catalog");
    assert_eq!(fx["unknown"], Value::Null);
    assert_eq!(json(&harness_catalog("aider")), fx["unknown"]);
}

// fixture: harness_catalog.json catalogs; key order is the TS's (ruling 6402177374 rule 1)
#[test]
fn every_table_row_writes_its_keys_in_the_ts_order() {
    let fx = fixture("harness_catalog");
    let keys = |v: &Value| v.as_object().expect("an object").keys().cloned().collect::<Vec<_>>();
    let wanted = fx["catalogs"].as_array().unwrap();
    let got = harness_catalogs();
    assert_eq!(got.len(), wanted.len());
    for (row, want) in got.iter().zip(wanted) {
        assert_eq!(keys(&json(row)), keys(want), "{}", row.harness);
    }
}

// fixture: harness_catalog.json probes; `catalog_from_probe` keeps the table's order
#[test]
fn a_catalog_from_a_probe_keeps_the_tables_key_order() {
    let table = table("claude");
    let got = catalog_from_probe(&table, &probe());
    let keys = |v: &Value| v.as_object().expect("an object").keys().cloned().collect::<Vec<_>>();
    assert_eq!(keys(&json(&got)), keys(&json(&table)));
}
