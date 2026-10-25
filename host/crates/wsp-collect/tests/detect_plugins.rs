// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/plugins.test.ts at a3b870a6e: every agent's plugins on a computer, read off a home
//! laid out from the recorded Claude Code config.
mod common;

use std::cell::RefCell;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use serde_json::{json, Value};
use tempfile::TempDir;
use wsp_catalog::plugins::{PeekDir, PeekGroup, PluginIo};
use wsp_collect::detect::plugins::{detect_plugins, plugin_io, PluginsRead};
use wsp_collect::host::{Host, HostExec, RunOptions};
use wsp_collect::live_host::node_host;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../packages/catalog/test/fixtures/plugins/claude-2.1.296.json");

/// The recorded Claude Code config: its files by path, each under `/x/`.
fn claude_files() -> Vec<(String, String)> {
    let recorded: Value = serde_json::from_str(&fs::read_to_string(FIXTURE).unwrap()).unwrap();
    recorded["files"].as_object().unwrap().iter().map(|(path, text)| (path.clone(), text.as_str().unwrap().to_string())).collect()
}

fn put(path: &str, text: &str) {
    fs::create_dir_all(Path::new(path).parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

type Codex = Rc<dyn Fn(&str) -> Option<String>>;

/// Every run whose last argument asks for Codex is kept and answered by `answer` (nothing where there is none); every
/// other line runs.
struct CodexAnswers {
    inner: Rc<dyn HostExec>,
    lines: Rc<RefCell<Vec<String>>>,
    answer: Option<Codex>,
}

impl HostExec for CodexAnswers {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        self.inner.which(bin)
    }

    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        Box::pin(async move {
            let line = args.last().map(String::as_str).unwrap_or("");
            if !line.contains("command -v codex") {
                return self.inner.run(cmd, args, opts).await;
            }
            self.lines.borrow_mut().push(line.to_string());
            self.answer.as_ref().and_then(|answer| answer(line))
        })
    }
}

/// Every run's last argument kept, and the text each run printed.
struct Tap {
    inner: Rc<dyn HostExec>,
    lines: Rc<RefCell<Vec<String>>>,
    said: Rc<RefCell<Vec<String>>>,
}

impl HostExec for Tap {
    fn which<'a>(&'a self, bin: &'a str) -> LocalBoxFuture<'a, bool> {
        self.inner.which(bin)
    }

    fn run<'a>(&'a self, cmd: &'a str, args: &'a [String], opts: RunOptions) -> LocalBoxFuture<'a, Option<String>> {
        Box::pin(async move {
            self.lines.borrow_mut().push(args.last().cloned().unwrap_or_default());
            let out = self.inner.run(cmd, args, opts).await;
            if let Some(o) = &out {
                self.said.borrow_mut().push(o.clone());
            }
            out
        })
    }
}

struct Tapped {
    host: Host,
    lines: Rc<RefCell<Vec<String>>>,
    said: Rc<RefCell<Vec<String>>>,
}

fn tapped(host: &Host) -> Tapped {
    let lines = Rc::new(RefCell::new(Vec::new()));
    let said = Rc::new(RefCell::new(Vec::new()));
    let exec = Tap { inner: host.exec.clone(), lines: lines.clone(), said: said.clone() };
    Tapped { host: Host { exec: Rc::new(exec), ..host.clone() }, lines, said }
}

fn not_codex(lines: &Rc<RefCell<Vec<String>>>) -> Vec<String> {
    lines.borrow().iter().filter(|l| !l.contains("command -v codex")).cloned().collect()
}

struct Home {
    host: Host,
    root: String,
    codex_lines: Rc<RefCell<Vec<String>>>,
    _dir: TempDir,
}

/// The recorded Claude Code config written under a fresh home, its paths moved there; Codex's lines are kept and
/// answered by `codex`, every other line runs.
fn home(store: bool, codex: Option<Codex>) -> Home {
    let dir = tempfile::Builder::new().prefix("wsp-plugins-").tempdir().unwrap();
    let root = dir.path().to_str().unwrap().to_string();
    for (path, text) in claude_files() {
        let at = match path.strip_prefix("/x/") {
            Some(rest) => format!("{root}/{rest}"),
            None => path.clone(),
        };
        put(&at, &text.replace("/x/", &format!("{root}/")));
    }
    let live = node_host().unwrap();
    let codex_lines = Rc::new(RefCell::new(Vec::new()));
    let exec = CodexAnswers { inner: live.exec.clone(), lines: codex_lines.clone(), answer: codex };
    let stores =
        store.then(|| IndexMap::from([("claude".to_string(), format!("{root}/claude")), ("codex".to_string(), format!("{root}/codex"))]));
    let host = Host { home: root.clone(), stores, exec: Rc::new(exec), ..live };
    Home { host, root, codex_lines, _dir: dir }
}

fn rows(read: &PluginsRead) -> Vec<Value> {
    serde_json::to_value(&read.plugins).unwrap().as_array().unwrap().clone()
}

fn claude_rows(read: &PluginsRead) -> Vec<Value> {
    rows(read).into_iter().filter(|p| p["agent"] == "claude").collect()
}

fn row(read: &PluginsRead, id: &str) -> Value {
    rows(read).into_iter().find(|p| p["id"] == id).unwrap_or_else(|| panic!("no plugin {id}"))
}

/// vitest's toMatchObject: every key wanted is there and matches, arrays alike in length.
fn matches(actual: &Value, wanted: &Value) -> bool {
    match (actual, wanted) {
        (Value::Object(a), Value::Object(w)) => w.iter().all(|(k, v)| a.get(k).is_some_and(|x| matches(x, v))),
        (Value::Array(a), Value::Array(w)) => a.len() == w.len() && a.iter().zip(w).all(|(x, y)| matches(x, y)),
        _ => actual == wanted,
    }
}

fn assert_matches(actual: &Value, wanted: &Value) {
    assert!(matches(actual, wanted), "{actual:#} does not match {wanted:#}");
}

#[tokio::test]
async fn keeps_claude_codes_rows_when_codexs_app_server_does_not_answer() {
    let h = home(true, None);
    let read = detect_plugins(&h.host, None, &[]).await;
    assert_eq!(claude_rows(&read).len(), 13);
    assert!(!rows(&read).iter().any(|p| p["agent"] == "codex"));
    assert_eq!(read.refused, vec!["plugins: Codex's app server did not answer, so Codex's plugins were not read"]);
    let vercel = row(&read, "vercel@claude-plugins-official");
    assert_matches(
        &vercel,
        &json!({ "agent": "claude", "path": "~/claude/plugins/cache/claude-plugins-official/vercel/0.50.0", "on": true, "scope": "user" }),
    );
    assert_eq!(vercel["brings"]["skills"].as_array().unwrap().len(), 37);
    assert_eq!(row(&read, "rust-analyzer-lsp@claude-plugins-official")["brings"]["lsp"], json!(["rust-analyzer"]));
}

#[tokio::test]
async fn reads_claude_code_under_its_store_in_one_line_and_asks_codexs_app_server_under_the_stores_codex_home() {
    let h = home(true, None);
    let counted = tapped(&h.host);
    detect_plugins(&counted.host, None, &[]).await;
    assert_eq!(not_codex(&counted.lines).len(), 1);
    let codex_lines = h.codex_lines.borrow();
    assert!(!codex_lines.is_empty());
    let start = format!("export CODEX_HOME='{}/codex'; ", h.root);
    assert!(codex_lines.iter().all(|l| l.starts_with(&start)), "{codex_lines:?}");
    // Under the home, with no store named, nothing of the store is read.
    let bare = home(false, None);
    assert_eq!(claude_rows(&detect_plugins(&bare.host, None, &[]).await), Vec::<Value>::new());
}

#[tokio::test]
async fn asks_a_second_time_by_name_for_an_install_path_the_peek_could_not_read_off_the_index() {
    let h = home(true, None);
    let index = format!("{}/claude/plugins/installed_plugins.json", h.root);
    // A path JSON writes with an escape: the shell's read of the index leaves it, and the reader asks for it itself.
    let plain = format!("\"{}/claude/plugins/cache/brag/brag/0.4.0\"", h.root);
    let escaped = serde_json::to_string(&format!("{}/claude/plugins/cache/brag/brag/0.4.0", h.root)).unwrap().replace('/', "\\/");
    let text = fs::read_to_string(&index).unwrap();
    assert!(text.contains(&plain));
    fs::write(&index, text.replace(&plain, &escaped)).unwrap();
    let counted = tapped(&h.host);
    let read = detect_plugins(&counted.host, None, &[]).await;
    let brag = row(&read, "brag@brag");
    assert_eq!(brag.get("missing"), None);
    assert_eq!(brag["brings"]["skills"], json!(["brag:brag", "brag:brag-slim"]));
    assert_eq!(not_codex(&counted.lines).len(), 2);
}

mod on_a_workspace_whose_image_built_a_seed {
    use super::*;

    struct Guest {
        host: Host,
        root: String,
        _dir: TempDir,
    }

    /// A guest's config folder and seed as Claude Code 2.1.296 left them: the seed one build line made, and the config
    /// folder before any turn (settings alone) or after two (the seed's marketplaces copied, an index naming a cache
    /// folder of its own that no turn filled).
    fn guest(turns: u8, enabled: bool) -> Guest {
        let dir = tempfile::Builder::new().prefix("wsp-plugins-seed-").tempdir().unwrap();
        let root = dir.path().to_str().unwrap().to_string();
        let (cfg, seed) = (format!("{root}/claude-cfg"), format!("{root}/claude-plugin-seed"));
        let marketplace = json!({ "source": { "source": "github", "repo": "acme/tools" }, "installLocation": format!("{seed}/marketplaces/acme-tools"), "lastUpdated": "2026-10-16T04:10:21.229Z" });
        let record = |at: &str| json!({ "lint-guard@acme-tools": [{ "scope": "user", "installPath": format!("{at}/cache/acme-tools/lint-guard/0.1.0"), "version": "0.1.0", "gitCommitSha": "27e58cf80e619ff7fa2b116b21105562ade2a9d7" }] });
        let mut files: Vec<(String, String)> = vec![
            (format!("{cfg}/settings.json"), if enabled { json!({ "enabledPlugins": { "lint-guard@acme-tools": true } }) } else { json!({}) }.to_string()),
            (format!("{seed}/installed_plugins.json"), json!({ "version": 2, "plugins": record(&seed) }).to_string()),
            (format!("{seed}/known_marketplaces.json"), json!({ "acme-tools": marketplace }).to_string()),
            (
                format!("{seed}/marketplaces/acme-tools/.claude-plugin/marketplace.json"),
                json!({ "name": "acme-tools", "owner": { "name": "Acme" }, "plugins": [{ "name": "lint-guard", "source": "./", "description": "Stops a lint-free commit" }] }).to_string(),
            ),
            (format!("{seed}/cache/acme-tools/lint-guard/0.1.0/.claude-plugin/plugin.json"), json!({ "name": "lint-guard", "version": "0.1.0" }).to_string()),
            (
                format!("{seed}/cache/acme-tools/lint-guard/0.1.0/hooks/hooks.json"),
                json!({ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "true" }] }] } }).to_string(),
            ),
            (format!("{seed}/cache/acme-tools/lint-guard/0.1.0/skills/lint/SKILL.md"), "---\nname: lint\ndescription: lint\n---\n".to_string()),
        ];
        if turns == 2 {
            let mut copied = marketplace.clone();
            copied["autoUpdate"] = json!(false);
            files.push((format!("{cfg}/plugins/known_marketplaces.json"), json!({ "acme-tools": copied }).to_string()));
            files.push((
                format!("{cfg}/plugins/installed_plugins.json"),
                json!({ "version": 2, "plugins": record(&format!("{cfg}/plugins")) }).to_string(),
            ));
        }
        for (at, text) in &files {
            put(at, text);
        }
        let live = node_host().unwrap();
        let exec = CodexAnswers { inner: live.exec.clone(), lines: Rc::new(RefCell::new(Vec::new())), answer: None };
        let stores = IndexMap::from([("claude".to_string(), cfg)]);
        let seeds = IndexMap::from([("claude".to_string(), format!("{root}/elsewhere:{seed}"))]);
        let host = Host { home: root.clone(), stores: Some(stores), seeds: Some(seeds), exec: Rc::new(exec), ..live };
        Guest { host, root, _dir: dir }
    }

    #[tokio::test]
    async fn lists_each_seeded_plugin_as_installed_from_the_seed_after_turns() {
        for turns in [0u8, 2] {
            let g = guest(turns, true);
            let read = detect_plugins(&g.host, None, &[]).await;
            let claude = claude_rows(&read);
            assert_eq!(claude.len(), 1, "after {turns} turns");
            assert_matches(
                &claude[0],
                &json!({
                    "id": "lint-guard@acme-tools",
                    "version": "0.1.0",
                    "scope": "user",
                    "on": true,
                    "path": "~/claude-plugin-seed/cache/acme-tools/lint-guard/0.1.0",
                    "source": "acme/tools",
                    "description": "Stops a lint-free commit",
                }),
            );
            let lint = row(&read, "lint-guard@acme-tools");
            assert_eq!(lint.get("missing"), None, "after {turns} turns");
            assert_matches(&lint["brings"], &json!({ "hooks": ["PreToolUse"], "skills": ["lint-guard:lint"] }));
        }
    }

    #[tokio::test]
    async fn reads_a_seeded_plugin_no_settings_switch_on_as_off_which_no_turn_loads() {
        let g = guest(0, false);
        let read = detect_plugins(&g.host, None, &[]).await;
        assert_matches(&row(&read, "lint-guard@acme-tools"), &json!({ "on": false }));
    }

    #[tokio::test]
    async fn reads_a_marketplace_the_seeds_list_and_the_config_folders_both_name_once() {
        let g = guest(2, true);
        let counted = tapped(&g.host);
        detect_plugins(&counted.host, None, &[]).await;
        let said = counted.said.borrow().join("");
        let named = regex::Regex::new(r"\x1ef[^\x1f]*marketplace\.json\x1f").unwrap();
        assert_eq!(named.find_iter(&said).count(), 1, "{said:?}");
    }

    #[tokio::test]
    async fn reads_the_config_folders_own_copy_where_it_has_one_which_a_turn_loads_before() {
        let g = guest(2, true);
        let own = format!("{}/claude-cfg/plugins/cache/acme-tools/lint-guard/0.1.0/.claude-plugin", g.root);
        put(&format!("{own}/plugin.json"), &json!({ "name": "lint-guard" }).to_string());
        let read = detect_plugins(&g.host, None, &[]).await;
        assert_matches(&row(&read, "lint-guard@acme-tools"), &json!({ "path": "~/claude-cfg/plugins/cache/acme-tools/lint-guard/0.1.0" }));
    }
}

#[tokio::test]
async fn peeks_a_folders_names_two_levels_down_and_a_files_text_and_nothing_of_what_is_not_there() {
    let h = home(true, None);
    let root = &h.root;
    let brag = format!("{root}/claude/plugins/cache/brag/brag/0.4.0");
    let group = PeekGroup {
        roots: Some(vec![brag.clone(), format!("{root}/gone")]),
        roots_from: None,
        files: Some(vec![".claude-plugin/plugin.json".to_string()]),
        dirs: Some(vec![PeekDir { path: "skills".to_string(), depth: 2 }]),
    };
    let seen = plugin_io(&h.host, "claude").peek(&[group]).await.unwrap();
    let mut listed = seen.list(&format!("{brag}/skills")).unwrap();
    listed.sort();
    assert_eq!(listed, vec!["brag", "brag-slim", "brag-slim/SKILL.md", "brag/SKILL.md", "brag/slim.md"]);
    let plugin: Value = serde_json::from_str(&seen.text(&format!("{brag}/.claude-plugin/plugin.json")).unwrap()).unwrap();
    assert_matches(&plugin, &json!({ "name": "brag" }));
    assert_eq!(seen.list(&format!("{root}/gone/skills")), None);
    assert!(seen.looked(&format!("{root}/gone")));
}

/// Rules of spec/plugins.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use super::*;
    use crate::common::fake_host::*;
    use wsp_catalog::pending::{PluginBrings, PluginScope};
    use wsp_catalog::plugins::Peeked;
    use wsp_catalog::plugins::{FoundPlugin, PluginProject, PluginSkillDirs, RootsFrom};
    use wsp_collect::detect::plugins::{peek_line, peek_records, plugin_row, PEEK};

    fn groups() -> Vec<PeekGroup> {
        vec![
            PeekGroup {
                roots: Some(vec!["/r 1".into(), "it's".into()]),
                roots_from: None,
                files: Some(vec!["a.json".into(), "b c".into()]),
                dirs: Some(vec![PeekDir { path: "skills".into(), depth: 2 }]),
            },
            PeekGroup {
                roots: None,
                roots_from: Some(RootsFrom { files: vec!["/i.json".into(), "/j's.json".into()], key: "installPath".into() }),
                files: None,
                dirs: Some(vec![PeekDir { path: ".".into(), depth: 1 }]),
            },
            PeekGroup::default(),
        ]
    }

    const CALLS: &str = "peek 'a.json\nb c' '2 skills' '/r 1' 'it'\\''s'\nfrom 'installPath' '/i.json' '/j'\\''s.json' | while IFS= read -r r; do peek '' '1 .' \"$r\"; done\nprintf '\\036END\\n'";

    #[test]
    fn peek_line_quotes_every_group_and_ends_with_the_end_mark() {
        assert_eq!(peek_line(&groups()), format!("{PEEK}\n{CALLS}"));
        // A group with neither roots nor roots_from adds no call.
        assert_eq!(peek_line(&[PeekGroup::default()]), format!("{PEEK}\nprintf '\\036END\\n'"));
    }

    #[test]
    fn peek_records_reads_records_by_joined_path_and_refuses_an_answer_cut_short() {
        let said = "\u{1e}r/r 1\u{1f}\u{1e}f/r 1/a.json\u{1f}{\"x\":1}\u{1e}f/r 1/a.json\u{1f}second\u{1e}d/r 1//skills/\u{1f}one\u{1f}one/SKILL.md\u{1f}\n\u{1e}rno-cut\u{1e}r/r2\u{1f}\u{1e}END\n";
        let p = peek_records(Some(said)).expect("a whole answer");
        // A later record for the same file replaces the first; every lookup goes through join_path.
        assert_eq!(p.text("/r 1/a.json"), Some("second".to_string()));
        assert_eq!(p.text("/r 1//a.json"), Some("second".to_string()));
        assert_eq!(p.list("/r 1/skills"), Some(vec!["one".to_string(), "one/SKILL.md".to_string()]));
        assert!(p.looked("/r 1"));
        assert!(p.looked("/r2/"));
        assert!(!p.looked("rno-cut"));
        assert_eq!(p.text("/nope"), None);
        assert!(peek_records(None).is_none());
        assert!(peek_records(Some("")).is_none());
        assert!(peek_records(Some("\u{1e}f/a\u{1f}x\u{1e}EN")).is_none());
        let empty = peek_records(Some("\u{1e}END\n")).expect("an empty answer is whole");
        assert_eq!(empty.text("/a"), None);
    }

    #[test]
    fn plugin_io_runs_peek_under_sh_and_lines_under_bash_with_a_30_second_timeout() {
        let mut host = fake_host(FakeLaptop::new());
        host.host.stores = Some([("claude".to_string(), "/Users/dev/store".to_string())].into_iter().collect());
        let io = plugin_io(&host, "claude");
        assert_eq!((io.home(), io.store(), io.seed()), ("/Users/dev", Some("/Users/dev/store"), None));
        assert!(!io.can_edit());
        assert!(block_on(io.peek(&groups())).is_none());
        assert_eq!(host.calls(), vec![format!("run sh -c {PEEK}\n{CALLS} (30000 ms)")]);
        assert_eq!(block_on(io.run("echo hi")), None);
        let last = host.calls().last().cloned().unwrap();
        assert!(last.starts_with("run bash -c ") && last.ends_with("echo hi (30000 ms)"), "{last}");
    }

    #[test]
    fn plugin_row_puts_home_paths_under_tilde_and_drops_skills() {
        let brings =
            PluginBrings { skills: vec![], commands: vec![], subagents: vec![], hooks: vec![], servers: vec![], lsp: vec![], apps: vec![] };
        let found = FoundPlugin {
            id: "x@m".into(),
            name: "x".into(),
            marketplace: "m".into(),
            version: Some("1".into()),
            scope: PluginScope::Project,
            project: Some(PluginProject { id: "p".into(), name: "P".into(), path: "/h/code/p".into() }),
            on: false,
            missing: None,
            path: Some("/h/.claude/plugins/x".into()),
            set_in: Some("/h/code/p/.claude/settings.json".into()),
            description: Some("d".into()),
            source: None,
            brings,
            skills: Some(PluginSkillDirs { dirs: vec!["/s".into()], prefix: "x".into() }),
        };
        let row = serde_json::to_value(plugin_row("/h", "claude", &found)).unwrap();
        assert_eq!(
            row,
            json!({"agent": "claude", "id": "x@m", "name": "x", "marketplace": "m", "version": "1", "scope": "project", "on": false, "description": "d",
                   "brings": {"skills": [], "commands": [], "subagents": [], "hooks": [], "servers": [], "lsp": [], "apps": []},
                   "project": {"id": "p", "name": "P", "path": "~/code/p"}, "path": "~/.claude/plugins/x", "setIn": "~/code/p/.claude/settings.json"})
        );
    }

    #[test]
    fn detect_plugins_reads_nothing_for_agents_without_a_shelf() {
        let mut a = wsp_catalog::catalog::catalog_agents()[0].clone();
        a.id = "shelfless".into();
        a.plugin_shelf = None;
        let host = empty_host();
        let read = block_on(detect_plugins(&host, Some(&[&a]), &[]));
        assert!(read.plugins.is_empty() && read.refused.is_empty() && read.agents.is_empty());
        assert_eq!(host.calls(), Vec::<String>::new());
    }
}

/// Every case of tests/fixtures/oracle/w2/plugins.json that reaches a pub function (oracle/w2-plugins.oracle.ts): spec
/// plugins's wave-2 rules. `plugin_row` needs a `FoundPlugin` built from JSON, which wsp-catalog cannot read yet, and
/// `plugin_io` needs the catalog's `PluginIo` calls; their cases wait on wsp-catalog.
mod w2_oracle {
    use super::*;

    fn agent(id: &str) -> &'static wsp_catalog::catalog::AgentEntry {
        wsp_catalog::catalog::catalog_agents().into_iter().find(|a| a.id == id).unwrap()
    }
    use common::fake_host::block_on;
    use common::oracle::{cases, host, strs, w2};
    use wsp_catalog::plugins::{Peeked, RootsFrom};
    use wsp_collect::detect::plugins::{peek_line, peek_records};

    fn group(v: &Value) -> PeekGroup {
        PeekGroup {
            roots: v.get("roots").map(strs),
            roots_from: v.get("rootsFrom").map(|r| RootsFrom { files: strs(&r["files"]), key: r["key"].as_str().unwrap().into() }),
            files: v.get("files").map(strs),
            dirs: v.get("dirs").map(|d| {
                d.as_array().unwrap().iter().map(|d| PeekDir { path: d["path"].as_str().unwrap().into(), depth: d["depth"].as_u64().unwrap() as u8 }).collect()
            }),
        }
    }

    #[test]
    fn the_peek_line_and_its_records_are_the_ts_s() {
        let f = w2("plugins");
        for case in cases(&f, "peek_line") {
            let groups: Vec<PeekGroup> = case["args"][0].as_array().unwrap().iter().map(group).collect();
            assert_eq!(json!(peek_line(&groups)), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "peek_records") {
            let got = match peek_records(case["args"][0].as_str()) {
                None => Value::Null,
                Some(r) => {
                    let p = &case["probes"];
                    let texts: serde_json::Map<String, Value> = strs(&p["f"]).iter().filter_map(|k| r.text(k).map(|t| (k.clone(), json!(t)))).collect();
                    let lists: serde_json::Map<String, Value> = strs(&p["d"]).iter().filter_map(|k| r.list(k).map(|l| (k.clone(), json!(l)))).collect();
                    let looked: serde_json::Map<String, Value> = strs(&p["r"]).iter().map(|k| (k.clone(), json!(r.looked(k)))).collect();
                    json!({ "f": texts, "d": lists, "r": looked })
                }
            };
            assert_eq!(got, case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn detect_plugins_answers_and_runs_what_the_ts_did() {
        for case in cases(&w2("plugins"), "detect_plugins") {
            let h = host(&case["laptop"]);
            let agents: Vec<&'static wsp_catalog::catalog::AgentEntry> = strs(&case["args"][0]).iter().map(|id| agent(id)).collect();
            let projects: Vec<wsp_wire::types::agents_report::AgentsProject> = serde_json::from_value(case["args"][1].clone()).unwrap();
            let got: PluginsRead = block_on(detect_plugins(&h, Some(&agents), &projects));
            let agents: Vec<Value> = got
                .agents
                .iter()
                .map(|a| json!({ "agent": a.agent, "found": { "plugins": a.found.plugins, "refused": a.found.refused } }))
                .collect();
            assert_eq!(json!({ "plugins": got.plugins, "refused": got.refused, "agents": agents }), case["out"], "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }
}
