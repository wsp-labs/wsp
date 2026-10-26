// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/harness-catalog.ts at 65f6b89c7: what each harness's command line accepts at launch,
//! the table the pickers fall back on, and the binary's lists read through it. Spec: spec/harness_catalog.md.
use wsp_adapters::cursor::command::{DEFAULT_MODE, FORCE_MODE};
use wsp_adapters::opencode::command::AUTO_MODE;
use wsp_adapters::port::HarnessCatalogProbe;
use wsp_catalog::catalog::catalog_agents;
use wsp_catalog::roads::CLAUDE_CODE;
use wsp_wire::types::harness_picks::every_model;
use wsp_wire::views::harness::{HarnessCatalog, HarnessCatalogSource, HarnessModel, HarnessOption};
use wsp_words::exit::HostError;

use crate::protocol_pending::not_run_refusal;

/// When a thread's agent asks before an MCP tool (slate-mcp.ts `ToolAsks`; wsp-slates takes it from here).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolAsks {
    Never,
    Every,
    UnlessReadOnly,
}

impl ToolAsks {
    /// `"never"`, `"every"`, `"unless-read-only"`.
    pub fn as_str(self) -> &'static str {
        match self {
            ToolAsks::Never => "never",
            ToolAsks::Every => "every",
            ToolAsks::UnlessReadOnly => "unless-read-only",
        }
    }
}

/// The seven table rows as the wire reads them, written off harness-catalog.ts:71-223 (`fromTable` applied). The `@..@`
/// marks are the TS's imported constants, which `harness_catalogs` fills in.
const TABLE: &str = r##"[
{"source":"table","version":"--help @CLAUDE_CODE@, 2026-10-10","contextWindows":[{"value":"200k","label":"200k"},{"value":"1m","label":"1M","isDefault":true}],"steers":false,"renames":false,"images":false,"harness":"claude","label":"Claude Code","mcpServers":true,"asides":true,"movesAccess":true,"steersImages":true,"screenCommands":[{"name":"login","control":"sign-in"},{"name":"logout","control":"sign-in"},{"name":"model","control":"model"},{"name":"permissions","control":"access"},{"name":"config","control":"settings"},{"name":"help","control":"docs"}],"smallModel":"claude-haiku-5-5","models":[{"value":"claude-opus-5-5","label":"Opus 5.5","isDefault":true,"fast":true,"efforts":["low","medium","high","xhigh","max"],"contextWindows":[]},{"value":"claude-fable-5-1","label":"Fable 5.1","efforts":["low","medium","high","xhigh","max"],"contextWindows":[]},{"value":"claude-sonnet-5-5","label":"Sonnet 5.5","efforts":["low","medium","high","xhigh","max"],"defaultEffort":"medium","contextWindows":[]},{"value":"claude-haiku-5-5","label":"Haiku 5.5","efforts":["low","medium","high","xhigh","max"],"defaultEffort":"medium","contextWindows":[]}],"legacyModels":[{"value":"claude-opus-5","label":"Opus 5","fast":true,"efforts":["low","medium","high","xhigh","max"],"defaultEffort":"high","contextWindows":["200k","1m"]},{"value":"claude-opus-4-8","label":"Opus 4.8","fast":true,"efforts":["low","medium","high","xhigh","max"],"defaultEffort":"high","contextWindows":["200k","1m"]},{"value":"claude-opus-4-7","label":"Opus 4.7","fast":true,"efforts":["low","medium","high","xhigh","max"],"defaultEffort":"xhigh","contextWindows":["200k","1m"]},{"value":"claude-opus-4-6","label":"Opus 4.6","fast":true,"efforts":["low","medium","high","max"],"contextWindows":["200k","1m"]},{"value":"claude-opus-4-5","label":"Opus 4.5","fast":true,"efforts":[],"contextWindows":["200k","1m"]},{"value":"claude-fable-5","label":"Fable 5","efforts":["low","medium","high","xhigh","max"],"defaultEffort":"high","contextWindows":[]},{"value":"claude-sonnet-5","label":"Sonnet 5","efforts":["low","medium","high","xhigh","max"],"defaultEffort":"high","contextWindows":[]},{"value":"claude-sonnet-4-6","label":"Sonnet 4.6","efforts":["low","medium","high","max"],"contextWindows":["200k","1m"]},{"value":"claude-sonnet-4-5","label":"Sonnet 4.5","efforts":[],"contextWindows":["200k","1m"]},{"value":"claude-haiku-4-5-20251001","label":"Haiku 4.5","efforts":[],"contextWindows":[]}],"efforts":[{"value":"low","label":"Low"},{"value":"medium","label":"Medium"},{"value":"high","label":"High","isDefault":true},{"value":"xhigh","label":"Extra high"},{"value":"max","label":"Max"}],"access":{"ask":"default","auto-edit":"acceptEdits","full":"bypassPermissions"},"bypassMode":"bypassPermissions","readOnlyMode":"dontAsk","permissionModes":[{"value":"default","label":"Default","description":"Asks in the chat about each action that needs permission"},{"value":"acceptEdits","label":"Accept edits","description":"Edits files without asking; asks about commands that need permission"},{"value":"bypassPermissions","label":"Bypass","description":"Runs every action without asking","isDefault":true},{"value":"auto","label":"Auto","description":"The agent decides which actions to ask about; where the account has no such mode it asks as Default does"},{"value":"manual","label":"Manual","description":"Asks before every action"},{"value":"dontAsk","label":"Don't ask","description":"Refuses any action that needs permission, without asking"}]},
{"source":"table","version":"app-server 0.153.0, 2026-09-07","contextWindows":[],"steers":false,"renames":false,"images":false,"harness":"codex","label":"Codex","mcpServers":true,"asides":true,"smallModel":"gpt-5.2","models":[{"value":"gpt-5.6-sol","label":"GPT-5.6-Sol","isDefault":true,"fast":true,"efforts":["low","medium","high","xhigh","max","ultra"],"defaultEffort":"low"},{"value":"gpt-5.6-terra","label":"GPT-5.6-Terra","fast":true,"efforts":["low","medium","high","xhigh","max","ultra"],"defaultEffort":"medium"},{"value":"gpt-5.6-luna","label":"GPT-5.6-Luna","fast":true,"efforts":["low","medium","high","xhigh","max"],"defaultEffort":"medium"}],"legacyListed":true,"legacyModels":[{"value":"gpt-5.5","label":"GPT-5.5","fast":true,"efforts":["low","medium","high","xhigh"],"defaultEffort":"medium"},{"value":"gpt-5.2","label":"GPT-5.2","efforts":["low","medium","high","xhigh"],"defaultEffort":"medium"}],"efforts":[{"value":"low","label":"Low","isDefault":true},{"value":"medium","label":"Medium"},{"value":"high","label":"High"},{"value":"xhigh","label":"Extra high"},{"value":"max","label":"Max"},{"value":"ultra","label":"Ultra"}],"access":{"ask":"workspace-write","full":"danger-full-access","plan":"read-only"},"bypassMode":"danger-full-access","readOnlyToolsFree":true,"readOnlyMode":"read-only","permissionModes":[{"value":"read-only","label":"Read only","description":"Reads only; edits no files and runs no command that writes"},{"value":"workspace-write","label":"Workspace write","description":"Edits files and runs commands inside the working folder"},{"value":"danger-full-access","label":"Full access","description":"Runs every action without asking, with no folder off limits","isDefault":true}]},
{"source":"table","version":null,"contextWindows":[],"steers":false,"renames":false,"images":false,"harness":"gemini","label":"Gemini CLI","models":[],"efforts":[],"bypassMode":"yolo","permissionModes":[{"value":"default","label":"Default","description":"Asks before every action"},{"value":"auto_edit","label":"Auto edit","description":"Edits files without asking"},{"value":"yolo","label":"Yolo","description":"Runs every action without asking","isDefault":true}]},
{"source":"table","version":"run --help 1.18.18, 2026-09-27","contextWindows":[],"steers":false,"renames":false,"images":false,"harness":"opencode","label":"OpenCode","models":[],"efforts":[],"access":{"full":"@AUTO_MODE@"},"bypassMode":"@AUTO_MODE@","permissionModes":[{"value":"@AUTO_MODE@","label":"Auto","description":"Runs every action without asking, save what its own settings deny: a headless run has nobody to ask","isDefault":true}]},
{"source":"table","version":null,"contextWindows":[],"steers":false,"renames":false,"images":false,"harness":"pi","label":"Pi","models":[],"efforts":[{"value":"off","label":"Off"},{"value":"minimal","label":"Minimal"},{"value":"low","label":"Low"},{"value":"medium","label":"Medium"},{"value":"high","label":"High"},{"value":"xhigh","label":"Extra high"},{"value":"max","label":"Max"}],"permissionModes":[]},
{"source":"table","version":null,"contextWindows":[],"steers":false,"renames":false,"images":false,"harness":"hermes","label":"Hermes Agent","models":[],"efforts":[{"value":"none","label":"None"},{"value":"minimal","label":"Minimal"},{"value":"low","label":"Low"},{"value":"medium","label":"Medium"},{"value":"high","label":"High"},{"value":"xhigh","label":"Extra high"},{"value":"max","label":"Max"},{"value":"ultra","label":"Ultra"}],"bypassMode":"yolo","permissionModes":[{"value":"default","label":"Default","description":"Asks before a command that could do damage"},{"value":"yolo","label":"Yolo","description":"Runs a command that could do damage without asking","isDefault":true}]},
{"source":"table","version":"-h 2026.09.26-dd393fe, 2026-09-27","contextWindows":[],"steers":false,"renames":false,"images":false,"harness":"cursor","label":"Cursor","models":[],"efforts":[],"access":{"full":"@FORCE_MODE@","plan":"@DEFAULT_MODE@"},"bypassMode":"@FORCE_MODE@","permissionModes":[{"value":"@DEFAULT_MODE@","label":"Default","description":"Proposes changes and runs no command; applies nothing"},{"value":"@FORCE_MODE@","label":"Force","description":"Edits files and runs every command without asking, save what its own settings deny","isDefault":true}]}
]"##;

/// The seven table rows, in the TS's order: claude, codex, gemini, opencode, pi, hermes, cursor. Every row reads
/// `source: "table"`, `steers`, `renames` and `images` false, and its bypass mode marked default among its permission
/// modes. harness-catalog.ts:71.
pub fn harness_catalogs() -> Vec<HarnessCatalog> {
    let table = TABLE
        .replace("@CLAUDE_CODE@", CLAUDE_CODE.version)
        .replace("@AUTO_MODE@", AUTO_MODE)
        .replace("@FORCE_MODE@", FORCE_MODE)
        .replace("@DEFAULT_MODE@", DEFAULT_MODE);
    serde_json::from_str(&table).expect("the harness table is written in the wire shape")
}

/// The row whose `harness` is that id. harness-catalog.ts:225.
pub fn harness_catalog(harness: &str) -> Option<HarnessCatalog> {
    harness_catalogs().into_iter().find(|c| c.harness == harness)
}

/// "never" with no mode named or at the row's bypass mode; else "unless-read-only" where the row says
/// `readOnlyToolsFree`, else "every". harness-catalog.ts:232.
pub fn tool_asks(harness: Option<&str>, mode: Option<&str>) -> ToolAsks {
    let row = harness.and_then(harness_catalog);
    match mode {
        None => ToolAsks::Never,
        Some(mode) if row.as_ref().and_then(|r| r.bypass_mode.as_deref()) == Some(mode) => ToolAsks::Never,
        Some(_) if row.as_ref().is_some_and(|r| r.read_only_tools_free == Some(true)) => ToolAsks::UnlessReadOnly,
        Some(_) => ToolAsks::Every,
    }
}

/// A model a binary's lists refused that the table carries, said as that binary's version not running it, with its
/// agent's update where the binary is older than this table. harness-catalog.ts:240.
pub fn not_run_refused(lists: Option<&HarnessCatalog>, model: Option<&str>) -> Option<HostError> {
    let lists = lists?;
    let table = harness_catalog(&lists.harness);
    let update = catalog_agents().into_iter().find(|a| a.id == lists.harness).and_then(|a| a.update_line.clone());
    not_run_refusal(Some(lists), table.as_ref(), model, update.as_deref())
}

/// Models newer than any pin, named and never offered: GPT-6-Astra. harness-catalog.ts:245.
pub fn named_only() -> Vec<HarnessOption> {
    vec![option("gpt-6-astra", "GPT-6-Astra")]
}

fn option(value: &str, label: &str) -> HarnessOption {
    HarnessOption { value: value.into(), label: label.into(), description: None, is_default: None, short: None }
}

/// Strips a trailing `-` and eight digits, as `id.replace(/-\d{8}$/, "")` does.
fn undated(id: &str) -> &str {
    let bytes = id.as_bytes();
    if bytes.len() >= 9 && bytes[bytes.len() - 9] == b'-' && bytes[bytes.len() - 8..].iter().all(u8::is_ascii_digit) {
        &id[..id.len() - 9]
    } else {
        id
    }
}

/// A model as a person reads it. harness-catalog.ts:249.
pub fn model_label(model: &str) -> String {
    if model.is_empty() {
        return "Model not named".to_string();
    }
    let bare = model.rsplit('/').next().unwrap_or(model);
    let tables = harness_catalogs();
    let known: Vec<HarnessOption> = tables.iter().flat_map(every_model).map(|m| option(&m.value, &m.label)).chain(named_only()).collect();
    if let Some(found) = known.iter().find(|o| o.value == bare).or_else(|| known.iter().find(|o| undated(&o.value) == undated(bare))) {
        return found.label.clone();
    }
    let words: Vec<&str> = undated(bare).split('-').filter(|w| !w.is_empty()).collect();
    let numeric = |w: &str| !w.is_empty() && w.bytes().all(|b| b.is_ascii_digit());
    let mut out = String::new();
    for (i, w) in words.iter().enumerate() {
        if numeric(w) && i > 0 && numeric(words[i - 1]) {
            out.push('.');
            out.push_str(w);
        } else {
            let mut chars = w.chars();
            let first: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
            out.push(' ');
            out.push_str(&first);
            out.push_str(chars.as_str());
        }
    }
    let trimmed = out.trim_start();
    if trimmed.is_empty() {
        bare.to_string()
    } else {
        trimmed.to_string()
    }
}

/// The row's own small model where the catalog in front of us still lists it. harness-catalog.ts:264.
pub fn smallest_model(catalog: Option<&HarnessCatalog>) -> Option<String> {
    let catalog = catalog?;
    let small = catalog.small_model.as_ref()?;
    every_model(catalog).iter().any(|m| &m.value == small).then(|| small.clone())
}

/// The binary's lists in the wire shape, the table lending labels and descriptions. harness-catalog.ts:274.
pub fn catalog_from_probe(table: &HarnessCatalog, probe: &HarnessCatalogProbe) -> HarnessCatalog {
    let table_models = every_model(table);
    let table_model = |value: &str| table_models.iter().find(|t| t.value == value);
    let heard: Vec<HarnessModel> = probe
        .models
        .iter()
        .map(|m| HarnessModel {
            value: m.slug.clone(),
            label: table_model(&m.slug).map_or_else(|| m.label.clone(), |t| t.label.clone()),
            description: m.description.clone(),
            is_default: m.is_default.then_some(true),
            short: None,
            efforts: m.efforts.clone(),
            default_effort: m.default_effort.clone(),
            context_windows: Some(m.context_windows.clone()),
            fast: (m.fast.or_else(|| table_model(&m.slug).and_then(|t| t.fast)) == Some(true)).then_some(true),
            added: None,
        })
        .collect();
    let lists_legacy = table.legacy_listed == Some(true);
    let folded = |m: &HarnessModel| {
        lists_legacy
            && m.is_default != Some(true)
            && table.legacy_models.as_ref().is_some_and(|legacy| legacy.iter().any(|l| l.value == m.value))
    };
    let models: Vec<HarnessModel> = heard.iter().filter(|m| !folded(m)).cloned().collect();
    let legacy_models: Option<Vec<HarnessModel>> = table.legacy_models.as_ref().map(|legacy| {
        if lists_legacy {
            heard.iter().filter(|m| folded(m)).cloned().collect()
        } else {
            legacy.iter().filter(|l| !heard.iter().any(|m| m.value == l.value)).cloned().collect()
        }
    });
    let default_effort = probe.models.iter().find(|m| m.is_default).and_then(|m| m.default_effort.clone());
    let effort = |value: &str| -> HarnessOption {
        let base = table.efforts.iter().find(|o| o.value == value).cloned().unwrap_or_else(|| option(value, &capitalize(value)));
        match &default_effort {
            None => base,
            Some(default) => HarnessOption { is_default: (value == default.as_str()).then_some(true), ..base },
        }
    };
    let efforts = probe.efforts.iter().map(|value| effort(value.as_str())).collect();
    let permission_modes = probe
        .permission_modes
        .iter()
        .map(|value| table.permission_modes.iter().find(|o| &o.value == value).cloned().unwrap_or_else(|| option(value, value)))
        .collect();
    HarnessCatalog {
        source: HarnessCatalogSource::Harness,
        version: probe.version.clone(),
        models,
        legacy_models,
        efforts,
        permission_modes,
        ..table.clone()
    }
}

/// Effort labels: the value capitalized, `xhigh` as `Extra high`. harness-catalog.ts:37.
fn capitalize(value: &str) -> String {
    if value == "xhigh" {
        return "Extra high".to_string();
    }
    let mut chars = value.chars();
    let first: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    format!("{first}{}", chars.as_str())
}
