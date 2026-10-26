// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/plugins.test.ts.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use futures::executor::block_on;
use indexmap::IndexMap;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};

use wsp_catalog::catalog::catalog_entry;
use wsp_catalog::plugins::{
    in_store, join_path, plain_values, FoundPlugin, PeekGroup, Peeked, PluginIo, PluginProject, PluginShelf, PluginSkillDirs, PluginsFound,
};
use wsp_wire::types::agents_report::PluginBrings;
use wsp_wire::types::agents_report::PluginMissing;
use wsp_wire::types::agents_report::PluginScope;
use wsp_words::agents_report::config_changed_refusal;
use wsp_words::exit::HostError;

#[derive(Deserialize)]

struct ClaudeInit {
    plugins: Vec<InitPlugin>,
    agents: Vec<String>,
    skills: Vec<String>,
    slash_commands: Vec<String>,
    mcp_servers: Vec<InitServer>,
}

#[derive(Deserialize)]
struct InitPlugin {
    name: String,
    source: String,
    path: String,
}

#[derive(Deserialize)]
struct InitServer {
    name: String,
    source: String,
}

#[derive(Deserialize)]
struct ClaudeFixture {
    files: IndexMap<String, String>,
    init: ClaudeInit,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CodexFixture {
    plugin_list: Value,
    config_read: Value,
    plugin_read: IndexMap<String, Value>,
    write_ok: Value,
    write_stale: Value,
}

fn fixture<T: for<'de> Deserialize<'de>>(name: &str) -> T {
    let path = format!("{}/tests/fixtures/ts/plugins/{name}", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn claude() -> ClaudeFixture {
    fixture("claude-2.1.296.json")
}

fn codex() -> Arc<CodexFixture> {
    Arc::new(fixture("codex-0.162.1.json"))
}

fn unique(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    items.into_iter().filter(|i| seen.insert(i.clone())).collect()
}

struct PeekedFiles {
    texts: HashMap<String, String>,
    lists: HashMap<String, Vec<String>>,
    looked: HashSet<String>,
}

impl Peeked for PeekedFiles {
    fn text(&self, path: &str) -> Option<String> {
        self.texts.get(&join_path(&[path])).cloned()
    }
    fn list(&self, dir: &str) -> Option<Vec<String>> {
        self.lists.get(&join_path(&[dir])).cloned()
    }
    fn looked(&self, root: &str) -> bool {
        self.looked.contains(&join_path(&[root]))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum EditMode {
    Apply,
    Record,
}

struct Edit {
    file: String,
    base: String,
}

type RunHandler = Box<dyn Fn(&str) -> Option<String>>;

/// A computer holding exactly these files, every folder above one there too, answering only what a peek asks;
/// `runs` records each line run and `peeks` each peek.
struct FilesIo {
    files: IndexMap<String, String>,
    store: Option<String>,
    handler: Option<RunHandler>,
    runs: Mutex<Vec<String>>,
    peeks: AtomicUsize,
    edit_mode: Option<EditMode>,
    text: Mutex<Option<String>>,
    edits: Mutex<Vec<Edit>>,
    sent: Arc<Mutex<Vec<(String, Value)>>>,
}

fn files_io(files: IndexMap<String, String>, store: Option<&str>) -> FilesIo {
    FilesIo {
        files,
        store: store.map(str::to_string),
        handler: None,
        runs: Mutex::new(vec![]),
        peeks: AtomicUsize::new(0),
        edit_mode: None,
        text: Mutex::new(None),
        edits: Mutex::new(vec![]),
        sent: Arc::new(Mutex::new(vec![])),
    }
}

impl PluginIo for FilesIo {
    fn home(&self) -> &str {
        "/x"
    }

    fn store(&self) -> Option<&str> {
        self.store.as_deref()
    }

    fn seed(&self) -> Option<&str> {
        None
    }

    async fn peek(&self, groups: &[PeekGroup]) -> Option<Box<dyn Peeked>> {
        self.peeks.fetch_add(1, Ordering::SeqCst);
        let mut texts = HashMap::new();
        let mut lists = HashMap::new();
        let mut looked = HashSet::new();
        for g in groups {
            let from = match &g.roots_from {
                Some(rf) => {
                    unique(rf.files.iter().flat_map(|f| plain_values(self.files.get(f).map(String::as_str).unwrap_or(""), &rf.key)))
                }
                None => vec![],
            };
            let roots: Vec<String> = g.roots.iter().flatten().cloned().chain(from).collect();
            for r in roots {
                looked.insert(join_path(&[&r]));
                for f in g.files.iter().flatten() {
                    let p = join_path(&[&r, f]);
                    if let Some(t) = self.files.get(&p) {
                        texts.insert(p, t.clone());
                    }
                }
                for d in g.dirs.iter().flatten() {
                    let dir = join_path(&[&r, &d.path]);
                    let under: Vec<Vec<String>> = self
                        .files
                        .keys()
                        .filter(|x| x.starts_with(&format!("{dir}/")))
                        .map(|x| x[dir.len() + 1..].split('/').map(str::to_string).collect())
                        .collect();
                    if !under.is_empty() {
                        let all = under.iter().flat_map(|segs| (1..=segs.len().min(d.depth as usize)).map(move |i| segs[..i].join("/")));
                        lists.insert(dir, unique(all));
                    }
                }
            }
        }
        Some(Box::new(PeekedFiles { texts, lists, looked }))
    }

    async fn run(&self, line: &str) -> Option<String> {
        self.runs.lock().unwrap().push(line.to_string());
        self.handler.as_ref().and_then(|h| h(line))
    }

    fn can_edit(&self) -> bool {
        self.edit_mode.is_some()
    }

    async fn edit(&self, file: &str, base: &str, change: &dyn Fn(Option<&str>) -> Result<String, HostError>) -> Result<(), HostError> {
        match self.edit_mode {
            Some(EditMode::Apply) => {
                let mut held = self.text.lock().unwrap();
                let next = change(held.as_deref())?;
                *held = Some(next);
            }
            Some(EditMode::Record) => {}
            None => return Err(HostError::new("this road writes no settings file")),
        }
        self.edits.lock().unwrap().push(Edit { file: file.to_string(), base: base.to_string() });
        Ok(())
    }
}

fn read(shelf: PluginShelf, io: &FilesIo, projects: &[PluginProject]) -> PluginsFound {
    block_on(shelf.read(io, projects))
}

fn ids(rows: &[&FoundPlugin]) -> Vec<String> {
    let mut v: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    v.sort();
    v
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

fn named<'a>(read: &'a PluginsFound, name: &str) -> &'a FoundPlugin {
    read.plugins.iter().find(|r| r.name == name).unwrap_or_else(|| panic!("no plugin named {name}"))
}

fn brings(
    skills: &[&str],
    commands: &[&str],
    subagents: &[&str],
    hooks: &[&str],
    servers: &[&str],
    lsp: &[&str],
    apps: &[&str],
) -> PluginBrings {
    PluginBrings {
        skills: strings(skills),
        commands: strings(commands),
        subagents: strings(subagents),
        hooks: strings(hooks),
        servers: strings(servers),
        lsp: strings(lsp),
        apps: strings(apps),
    }
}

fn no_read() -> PluginsFound {
    PluginsFound { plugins: vec![], refused: vec![], version: None, file: None }
}

fn a_plugin(id: &str) -> FoundPlugin {
    let (name, marketplace) = id.split_once('@').unwrap();
    FoundPlugin {
        id: id.to_string(),
        name: name.to_string(),
        marketplace: marketplace.to_string(),
        version: None,
        scope: PluginScope::User,
        project: None,
        on: true,
        missing: None,
        path: None,
        set_in: None,
        description: None,
        source: None,
        brings: brings(&[], &[], &[], &[], &[], &[], &[]),
        skills: None,
    }
}

mod claude_codes_plugins_off_its_files {
    use super::*;

    #[test]
    fn lists_the_owners_13_user_installs_out_of_18_plugins_in_30_records_6_on_and_none() {
        let fx = claude();
        let index: Value = serde_json::from_str(&fx.files["/x/claude/plugins/installed_plugins.json"]).unwrap();
        let plugins = index["plugins"].as_object().unwrap();
        assert_eq!(plugins.len(), 18);
        assert_eq!(plugins.values().flat_map(|v| v.as_array().unwrap()).count(), 30);
        let io = files_io(fx.files.clone(), Some("/x/claude"));
        let read = read(PluginShelf::Claude, &io, &[]);
        assert_eq!(read.plugins.len(), 13);
        assert!(read.plugins.iter().all(|r| r.scope == PluginScope::User && r.project.is_none()));
        let on: Vec<&FoundPlugin> = read.plugins.iter().filter(|r| r.on).collect();
        assert_eq!(
            ids(&on),
            strings(&[
                "brag@brag",
                "code-simplifier@claude-plugins-official",
                "no-claude-coauthor@acme",
                "rust-analyzer-lsp@claude-plugins-official",
                "skill-creator@claude-plugins-official",
                "vercel@claude-plugins-official"
            ])
        );
        assert!(!read.plugins.iter().any(|r| r.id == "code-review@claude-plugins-official"));
        assert_eq!(read.refused, Vec::<String>::new());
    }

    #[test]
    fn never_runs_claude_plugin_list_or_details_it_reads_files_alone() {
        let io = files_io(claude().files, Some("/x/claude"));
        read(PluginShelf::Claude, &io, &[PluginProject { id: "p1".into(), name: "lab".into(), path: "/x/lab".into() }]);
        assert_eq!(*io.runs.lock().unwrap(), Vec::<String>::new());
        // The config and every plugin's folder in one peek: one round trip on a computer reached over a link.
        assert_eq!(io.peeks.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn brings_what_the_recorded_turn_loaded_for_each_plugin_the_turn_loaded() {
        let fx = claude();
        let read = read(PluginShelf::Claude, &files_io(fx.files.clone(), Some("/x/claude")), &[]);
        let loaded: Vec<&InitPlugin> = fx.init.plugins.iter().filter(|p| p.path != "builtin").collect();
        // What a turn loads is exactly the plugins that are on and whose folder is there: skill-creator is on, its folder is not.
        let standing: Vec<&FoundPlugin> = read.plugins.iter().filter(|r| r.on && r.missing.is_none()).collect();
        assert_eq!(ids(&standing), sorted(loaded.iter().map(|p| p.source.clone()).collect()));
        for p in &loaded {
            let row = read.plugins.iter().find(|r| r.id == p.source).unwrap();
            let own = |names: &[String]| -> Vec<String> {
                sorted(names.iter().filter(|n| n.starts_with(&format!("{}:", p.name))).cloned().collect())
            };
            assert_eq!(row.brings.skills, own(&fx.init.skills), "{}", p.name);
            assert_eq!(row.brings.subagents, own(&fx.init.agents), "{}", p.name);
            let got: Vec<String> = sorted(unique(row.brings.skills.iter().chain(row.brings.commands.iter()).cloned()));
            assert_eq!(got, sorted(unique(own(&fx.init.slash_commands))), "{}", p.name);
            let servers: Vec<String> = fx
                .init
                .mcp_servers
                .iter()
                .filter(|s| s.source == "plugin" && s.name.starts_with(&format!("plugin:{}:", p.name)))
                .map(|s| s.name.clone())
                .collect();
            assert_eq!(row.brings.servers, sorted(servers), "{}", p.name);
            assert_eq!(row.path.as_deref(), Some(p.path.as_str()));
        }
        let vercel = named(&read, "vercel");
        assert_eq!(vercel.brings.skills.len(), 37);
        assert_eq!(vercel.brings.commands, strings(&["vercel:bootstrap", "vercel:deploy", "vercel:env", "vercel:status"]));
        assert_eq!(vercel.brings.subagents, strings(&["vercel:ai-architect", "vercel:deployment-expert", "vercel:performance-optimizer"]));
        assert_eq!(vercel.brings.hooks, strings(&["PostToolUse", "SessionEnd", "SessionStart"]));
        assert_eq!(vercel.brings.servers, strings(&["plugin:vercel:vercel"]));
        assert_eq!(vercel.source.as_deref(), Some("anthropics/claude-plugins-official"));
        assert_eq!(vercel.description.as_deref(), Some("Build and deploy web apps and agents"));
        assert_eq!(
            named(&read, "brag").skills,
            Some(PluginSkillDirs { dirs: strings(&["/x/claude/plugins/cache/brag/brag/0.4.0/skills"]), prefix: "brag".into() })
        );
        assert_eq!(named(&read, "no-claude-coauthor").brings.hooks, strings(&["PreToolUse"]));
    }

    #[test]
    fn takes_a_plugin_with_no_plugin_json_from_its_marketplace_entry_its_language_serve() {
        let files = claude().files;
        assert!(!files.keys().any(|f| f.contains("rust-analyzer-lsp/1.0.0/.claude-plugin")));
        let read = read(PluginShelf::Claude, &files_io(files, Some("/x/claude")), &[]);
        let rust = named(&read, "rust-analyzer-lsp");
        assert_eq!(rust.brings, brings(&[], &[], &[], &[], &[], &["rust-analyzer"], &[]));
        assert_eq!(rust.description.as_deref(), Some("Rust language server for code intelligence and analysis"));
    }

    #[test]
    fn reads_every_plugin_missing_where_the_cache_is_not_there_as_on_the_boat_machine_k() {
        let boat: IndexMap<String, String> = claude().files.into_iter().filter(|(path, _)| !path.contains("/plugins/cache/")).collect();
        let read = read(PluginShelf::Claude, &files_io(boat, Some("/x/claude")), &[]);
        assert_eq!(read.plugins.len(), 13);
        assert!(read.plugins.iter().all(|r| r.missing == Some(PluginMissing::Folder)));
        assert_eq!(read.plugins.iter().filter(|r| r.on).count(), 6);
    }

    #[test]
    fn merges_a_projects_switch_local_over_project_over_user_and_names_the_file_that_se() {
        let mut files = claude().files;
        files.insert("/x/lab/.claude/settings.json".into(), json!({ "enabledPlugins": { "posthog-lab@lab-local": false } }).to_string());
        let project = PluginProject { id: "p1".into(), name: "lab".into(), path: "/x/lab".into() };
        let found = read(PluginShelf::Claude, &files_io(files.clone(), Some("/x/claude")), std::slice::from_ref(&project));
        let shared = found.plugins.iter().find(|r| r.scope == PluginScope::Local).unwrap();
        assert_eq!(shared.id, "posthog-lab@lab-local");
        assert_eq!(shared.project, Some(project.clone()));
        assert!(!shared.on);
        assert_eq!(shared.set_in.as_deref(), Some("/x/lab/.claude/settings.json"));
        assert_eq!(shared.missing, Some(PluginMissing::Folder));
        files.insert(
            "/x/lab/.claude/settings.local.json".into(),
            json!({ "enabledPlugins": { "posthog-lab@lab-local": true } }).to_string(),
        );
        let found = read(PluginShelf::Claude, &files_io(files, Some("/x/claude")), std::slice::from_ref(&project));
        let local = found.plugins.iter().find(|r| r.scope == PluginScope::Local).unwrap();
        assert!(local.on);
        assert_eq!(local.set_in.as_deref(), Some("/x/lab/.claude/settings.local.json"));
    }

    #[test]
    fn falls_back_to_the_plugins_defaultenabled_where_no_settings_file_names_it() {
        let mut files = claude().files;
        files.insert("/x/claude/settings.json".into(), "{}".into());
        files.insert(
            "/x/claude/plugins/cache/brag/brag/0.4.0/.claude-plugin/plugin.json".into(),
            json!({ "name": "brag", "defaultEnabled": false }).to_string(),
        );
        let read = read(PluginShelf::Claude, &files_io(files, Some("/x/claude")), &[]);
        assert!(!named(&read, "brag").on);
        assert!(named(&read, "vercel").on);
    }

    #[test]
    fn reads_under_the_home_where_no_store_is_named() {
        let home: IndexMap<String, String> =
            claude().files.into_iter().map(|(path, text)| (path.replacen("/x/claude/", "/x/.claude/", 1), text)).collect();
        let read = read(PluginShelf::Claude, &files_io(home, None), &[]);
        assert_eq!(read.plugins.len(), 13);
        let state_home = catalog_entry("claude").unwrap().as_agent().unwrap().state_home.clone();
        assert_eq!(in_store(&state_home, "~/.claude/settings.json", "/x", Some("/s")), "/s/settings.json");
    }

    #[test]
    fn switches_by_writing_enabledplugins_in_the_user_settings_under_the_store_every_ot() {
        let row = a_plugin("brag@brag");
        let mut io = files_io(IndexMap::new(), Some("/x/claude"));
        io.edit_mode = Some(EditMode::Apply);
        *io.text.lock().unwrap() = Some("{\n  \"model\": \"opus\",\n  \"enabledPlugins\": {\n    \"brag@brag\": true,\n    \"vercel@claude-plugins-official\": true\n  }\n}\n".to_string());
        assert_eq!(block_on(PluginShelf::Claude.turn(&io, &row, false, &no_read())), Ok(None));
        {
            let edits = io.edits.lock().unwrap();
            assert_eq!(
                edits.iter().map(|e| (e.file.as_str(), e.base.as_str())).collect::<Vec<_>>(),
                vec![("/x/claude/settings.json", "/x")]
            );
        }
        let text = io.text.lock().unwrap().clone().unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap(),
            json!({ "model": "opus", "enabledPlugins": { "brag@brag": false, "vercel@claude-plugins-official": true } })
        );
        // Already off is done: the same text again.
        let before = io.text.lock().unwrap().clone();
        block_on(PluginShelf::Claude.turn(&io, &row, false, &no_read())).unwrap();
        assert_eq!(*io.text.lock().unwrap(), before);
        // claude plugin enable and disable download every user plugin whose folder is missing before they write (2.1.296).
        assert_eq!(*io.runs.lock().unwrap(), Vec::<String>::new());
        *io.text.lock().unwrap() = None;
        block_on(PluginShelf::Claude.turn(&io, &row, true, &no_read())).unwrap();
        let text = io.text.lock().unwrap().clone().unwrap_or_default();
        assert_eq!(serde_json::from_str::<Value>(&text).unwrap(), json!({ "enabledPlugins": { "brag@brag": true } }));
        // A store outside the home, a box's logins folder, is its own write's base.
        let mut logins = files_io(IndexMap::new(), Some("/logins/claude"));
        logins.edit_mode = Some(EditMode::Record);
        block_on(PluginShelf::Claude.turn(&logins, &row, true, &no_read())).unwrap();
        let edits = logins.edits.lock().unwrap();
        let last = edits.last().unwrap();
        assert_eq!(last.file, "/logins/claude/settings.json");
        assert_eq!(last.base, "/logins/claude");
        let unwritable = files_io(IndexMap::new(), None);
        let refused = block_on(PluginShelf::Claude.turn(&unwritable, &row, true, &no_read()));
        assert!(refused.unwrap().unwrap().contains("brag@brag was not switched"));
    }
}

#[derive(Default, Clone, Copy)]
struct CodexOpts {
    stale: bool,
    dead: bool,
    none: bool,
    old: bool,
    uninstalled: Option<&'static str>,
}

/// A computer whose codex app server answers each request in a script as 0.162.1 did.
fn codex_io(o: CodexOpts) -> FilesIo {
    let fx = codex();
    let mut io = files_io(IndexMap::new(), None);
    let sent = io.sent.clone();
    let request = Regex::new(r#"'(\{"jsonrpc"[^']*\})'"#).unwrap();
    io.handler = Some(Box::new(move |line: &str| {
        if o.none {
            return Some("__WSP_NO_CODEX__\n".to_string());
        }
        if o.dead {
            return None;
        }
        let mut answers: Vec<String> = vec![];
        for m in request.captures_iter(line) {
            let req: Value = serde_json::from_str(&m[1]).unwrap();
            let method = req["method"].as_str().unwrap_or("").to_string();
            sent.lock().unwrap().push((method.clone(), req.get("params").cloned().unwrap_or(Value::Null)));
            let mut answer = serde_json::Map::new();
            if let Some(id) = req.get("id") {
                answer.insert("id".into(), id.clone());
            }
            let params = &req["params"];
            let (key, body): (&str, Option<Value>) = match method.as_str() {
                "initialize" => ("result", Some(json!({ "codexHome": "/x/codex" }))),
                "plugin/list" if o.uninstalled.is_some() => {
                    let mut list = fx.plugin_list.clone();
                    for market in list["marketplaces"].as_array_mut().unwrap() {
                        for p in market["plugins"].as_array_mut().unwrap() {
                            if p["id"].as_str() == o.uninstalled {
                                p["installed"] = json!(false);
                                p["enabled"] = json!(false);
                            }
                        }
                    }
                    ("result", Some(list))
                }
                "plugin/list" if o.old => (
                    "error",
                    Some(
                        json!({ "code": -32600, "message": "Invalid request: unknown variant `plugin/list`, expected one of `initialize`, `thread/start`" }),
                    ),
                ),
                "plugin/list" => ("result", Some(fx.plugin_list.clone())),
                "config/read" => ("result", Some(fx.config_read.clone())),
                "plugin/read" => {
                    ("result", fx.plugin_read.values().find(|r| r["plugin"]["summary"]["name"] == params["pluginName"]).cloned())
                }
                "config/value/write" if o.stale => ("error", Some(fx.write_stale.clone())),
                "config/value/write" => ("result", Some(fx.write_ok.clone())),
                _ => ("error", Some(json!({ "message": "unknown" }))),
            };
            if let Some(body) = body {
                answer.insert(key.into(), body);
            }
            answers.push(Value::Object(answer).to_string());
        }
        Some(answers.join("\n"))
    }));
    io
}

fn sent_of(io: &FilesIo) -> Vec<(String, Value)> {
    io.sent.lock().unwrap().clone()
}

mod codexs_plugins_through_its_app_server {
    use super::*;

    #[test]
    fn lists_brag_and_no_claude_coauthor_with_what_each_brings_from_plugin_list_and_plu() {
        let io = codex_io(CodexOpts::default());
        let read = read(PluginShelf::Codex, &io, &[]);
        let brag = read.plugins.iter().find(|r| r.id == "brag@brag").unwrap();
        assert_eq!(brag.name, "brag");
        assert_eq!(brag.marketplace, "brag");
        assert_eq!(brag.version.as_deref(), Some("0.4.0"));
        assert_eq!(brag.scope, PluginScope::User);
        assert!(brag.on);
        assert_eq!(brag.path.as_deref(), Some("/x/codex/plugins/cache/brag/brag/0.4.0"));
        assert_eq!(brag.source.as_deref(), Some("/x/mk/brag"));
        assert_eq!(brag.missing, None);
        assert_eq!(brag.brings, brings(&["brag:brag", "brag:brag-slim"], &[], &[], &[], &[], &[], &[]));
        assert_eq!(read.plugins.iter().find(|r| r.id == "no-claude-coauthor@acme").unwrap().brings.hooks, strings(&["preToolUse"]));
        let paths: Vec<Value> =
            sent_of(&io).into_iter().filter(|(m, _)| m == "plugin/read").map(|(_, p)| p["marketplacePath"].clone()).collect();
        assert_eq!(paths, vec![json!("/x/mk/brag/.claude-plugin/marketplace.json"), json!("/x/mk/acme/.claude-plugin/marketplace.json")]);
        assert!(read.version.unwrap().starts_with("sha256:"));
    }

    #[test]
    fn reads_the_six_codex_app_plugins_config_toml_names_and_plugin_list_does_not_retur() {
        let read = read(PluginShelf::Codex, &codex_io(CodexOpts::default()), &[]);
        let missing: Vec<(String, bool, Option<PluginMissing>)> =
            read.plugins.iter().filter(|r| r.missing.is_some()).map(|r| (r.id.clone(), r.on, r.missing)).collect();
        let m = Some(PluginMissing::Marketplace);
        assert_eq!(
            missing,
            vec![
                ("browser@openai-bundled".to_string(), true, m),
                ("chrome@openai-bundled".to_string(), true, m),
                ("computer-use@openai-bundled".to_string(), false, m),
                ("documents@openai-primary-runtime".to_string(), true, m),
                ("presentations@openai-primary-runtime".to_string(), true, m),
                ("spreadsheets@openai-primary-runtime".to_string(), true, m),
            ]
        );
    }

    #[test]
    fn reads_a_plugin_config_toml_names_and_plugin_list_returns_as_not_installed_as_mis() {
        let read = read(PluginShelf::Codex, &codex_io(CodexOpts { uninstalled: Some("brag@brag"), ..CodexOpts::default() }), &[]);
        let brags: Vec<&FoundPlugin> = read.plugins.iter().filter(|r| r.id == "brag@brag").collect();
        assert_eq!(brags.len(), 1);
        assert_eq!(brags[0].marketplace, "brag");
        assert!(brags[0].on);
        assert_eq!(brags[0].missing, Some(PluginMissing::Uninstalled));
        assert_eq!(brags[0].source.as_deref(), Some("/x/mk/brag"));
        assert_eq!(
            wsp_words::agents_report::plugin_missing_line(&brag_row(brags[0]), "Codex", "lab-box"),
            "The config.toml on lab-box names brag@brag, but Codex has it in its marketplace brag and not installed, so no turn loads it."
        );
    }

    #[test]
    fn says_codexs_plugins_were_not_read_where_its_app_server_did_not_answer_and_nothin() {
        assert_eq!(
            read(PluginShelf::Codex, &codex_io(CodexOpts { dead: true, ..CodexOpts::default() }), &[]),
            PluginsFound {
                plugins: vec![],
                refused: strings(&["plugins: Codex's app server did not answer, so Codex's plugins were not read"]),
                version: None,
                file: None
            }
        );
        assert_eq!(read(PluginShelf::Codex, &codex_io(CodexOpts { none: true, ..CodexOpts::default() }), &[]), no_read());
        // A Codex older than its plugins answers plugin/list as a method it does not know, the way 0.162.1 answers any such.
        assert_eq!(read(PluginShelf::Codex, &codex_io(CodexOpts { old: true, ..CodexOpts::default() }), &[]), no_read());
    }

    #[test]
    fn switches_with_config_value_write_on_plugins_id_enabled_with_the_version_it_read() {
        let io = codex_io(CodexOpts::default());
        let read = read(PluginShelf::Codex, &io, &[]);
        let brag = read.plugins.iter().find(|r| r.id == "brag@brag").unwrap();
        assert_eq!(block_on(PluginShelf::Codex.turn(&io, brag, false, &read)), Ok(None));
        let last = sent_of(&io).pop().unwrap();
        assert_eq!(
            last,
            (
                "config/value/write".to_string(),
                json!({ "keyPath": "plugins.brag@brag.enabled", "value": false, "mergeStrategy": "upsert", "expectedVersion": read.version })
            )
        );
        let stale = codex_io(CodexOpts { stale: true, ..CodexOpts::default() });
        assert_eq!(
            block_on(PluginShelf::Codex.turn(&stale, brag, true, &read)),
            Ok(Some(config_changed_refusal(read.file.as_deref().unwrap())))
        );
    }
}

// Every row both shelves read off the recorded fixtures, held to tests/fixtures/plugins.json, which the TS shelves
// wrote over the same fakes (oracle/plugins.oracle.ts): the counts above, and every field of every row.
mod the_whole_read_is_the_ts_read {
    use super::*;

    fn want(key: &str) -> Value {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/plugins.json");
        serde_json::from_str::<Value>(&std::fs::read_to_string(path).unwrap()).unwrap()[key].clone()
    }

    #[track_caller]
    fn same(got: &PluginsFound, key: &str) {
        let got = serde_json::to_value(got).unwrap();
        let want = want(key);
        let order = |v: &Value| v["plugins"].as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert_eq!(order(&got), order(&want), "{key}: the rows and their order");
        assert_eq!(got, want, "{key}");
    }

    #[test]
    fn claude_codes_read_under_its_store_is_every_ts_row_field_for_field() {
        same(&read(PluginShelf::Claude, &files_io(claude().files.clone(), Some("/x/claude")), &[]), "claudeStore");
    }

    #[test]
    fn claude_codes_read_with_a_project_adds_the_projects_own_rows_field_for_field() {
        let lab = [PluginProject { id: "p1".into(), name: "lab".into(), path: "/x/lab".into() }];
        same(&read(PluginShelf::Claude, &files_io(claude().files.clone(), Some("/x/claude")), &lab), "claudeStoreLab");
    }

    #[test]
    fn claude_codes_read_under_the_home_finds_no_index_and_no_rows() {
        same(&read(PluginShelf::Claude, &files_io(claude().files.clone(), None), &[]), "claudeNoStore");
    }

    #[test]
    fn codexs_read_is_every_ts_row_with_its_config_version_and_file() {
        same(&read(PluginShelf::Codex, &codex_io(CodexOpts::default()), &[]), "codex");
        same(
            &read(PluginShelf::Codex, &codex_io(CodexOpts { uninstalled: Some("brag@brag"), ..CodexOpts::default() }), &[]),
            "codexUninstalled",
        );
    }
}

fn brag_row(found: &FoundPlugin) -> wsp_wire::types::agents_report::PluginRow {
    let none = || Vec::new();
    wsp_wire::types::agents_report::PluginRow {
        agent: "codex".to_string(),
        id: found.id.clone(),
        name: "brag".to_string(),
        marketplace: found.marketplace.clone(),
        version: None,
        scope: PluginScope::User,
        on: found.on,
        missing: found.missing,
        description: None,
        source: found.source.clone(),
        brings: PluginBrings {
            skills: none(),
            commands: none(),
            subagents: none(),
            hooks: none(),
            servers: none(),
            lsp: none(),
            apps: none(),
        },
        project: None,
        path: None,
        set_in: None,
    }
}

// Review of wsp-labs/wsp-map#2448: a plugin manifest or a marketplace entry holding object levels to the 500-level cap
// aborts the read, because plugins.rs:416-472 and :559 clone the parsed entry and a Value of objects clones to 627 on a
// 1 MiB debug stack. Marketplace files come from a marketplace's repo. node (plugins.ts) reads all 13 plugins.
fn in_child_2448(name: &str, body: fn()) {
    if std::env::var("REVIEW_2448_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env("REVIEW_2448_CHILD", name)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the whole process died ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("")
    );
}

fn objects_2448(n: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n))
}

fn manifest_cap_objects_body() {
    let mut files = claude().files;
    files.insert(
        "/x/claude/plugins/cache/brag/brag/0.4.0/.claude-plugin/plugin.json".to_string(),
        format!(r#"{{"name":"brag","version":"0.4.0","description":"d","extra":{}}}"#, objects_2448(499)),
    );
    let read = block_on(PluginShelf::Claude.read(&files_io(files, Some("/x/claude")), &[]));
    assert_eq!(read.plugins.len(), 13);
}

#[test]
fn a_plugin_manifest_holding_object_levels_to_the_cap_is_read() {
    in_child_2448("a_plugin_manifest_holding_object_levels_to_the_cap_is_read", manifest_cap_objects_body);
}

fn marketplace_cap_objects_body() {
    let mut files = claude().files;
    files.insert(
        "/x/claude/plugins/marketplaces/brag/.claude-plugin/marketplace.json".to_string(),
        format!(r#"{{"name":"brag","plugins":[{{"name":"brag","source":"./","extra":{}}}]}}"#, objects_2448(497)),
    );
    let read = block_on(PluginShelf::Claude.read(&files_io(files, Some("/x/claude")), &[]));
    assert_eq!(read.plugins.len(), 13);
}

#[test]
fn a_marketplace_entry_holding_object_levels_to_the_cap_is_read() {
    in_child_2448("a_marketplace_entry_holding_object_levels_to_the_cap_is_read", marketplace_cap_objects_body);
}
