// SPDX-License-Identifier: AGPL-3.0-only
//! Review of how wsp-collect parses the text of the person's files. Every expected value is what the TS printed for the
//! same text (node 22, packages/collect and packages/catalog at 6ff07bdbc).
mod common;

use common::fake_host::*;
use futures::StreamExt;
use serde_json::{json, Value};
use wsp_collect::detect::agents::detect_agents;
use wsp_collect::detect::logins::{api_key_helper_of, holds_claude_login};
use wsp_collect::detect::mcp::detect_mcp;
use wsp_collect::detect::mcp_project::{detect_project_mcp, ProjectMcpOptions};
use wsp_collect::detect::tools::{package_commands, parse_npm_globals, parse_pipx_list, parse_pnpm_globals, Pkg};
use wsp_collect::history::claude::ClaudeReader;
use wsp_collect::history::codex::CodexReader;
use wsp_collect::history::hermes::HermesReader;
use wsp_collect::history::reader::{Call, CallKind, HistoryReader};
use wsp_collect::project::scan_project;

fn pkgs(p: Vec<Pkg>) -> Value {
    Value::Array(
        p.into_iter()
            .map(|p| {
                let mut o = serde_json::Map::new();
                o.insert("name".into(), json!(p.name));
                if let Some(v) = p.version {
                    o.insert("version".into(), json!(v));
                }
                if let Some(v) = p.path {
                    o.insert("path".into(), json!(v));
                }
                Value::Object(o)
            })
            .collect(),
    )
}

fn call_json(c: &Call) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("session".into(), json!(c.session));
    if let Some(f) = &c.folder {
        o.insert("folder".into(), json!(f));
    }
    if let Some(a) = c.at {
        o.insert("at".into(), if a.fract() == 0.0 { json!(a as i64) } else { json!(a) });
    }
    match &c.kind {
        CallKind::Shell { line } => {
            o.insert("kind".into(), json!("shell"));
            o.insert("line".into(), json!(line));
        }
        CallKind::Other { name } => {
            o.insert("kind".into(), json!("other"));
            o.insert("name".into(), json!(name));
        }
    }
    Value::Object(o)
}

fn hist(reader: &dyn HistoryReader, h: &FakeHost, root: &str, file: &str) -> Value {
    let items: Vec<_> = block_on(async { reader.read(&h.host, root, file).collect::<Vec<_>>().await });
    let mut out = Vec::new();
    for item in items {
        match item {
            Ok(c) => out.push(call_json(&c)),
            Err(e) => return json!({ "error": e.message }),
        }
    }
    json!({ "out": out })
}

const H: &str = "/Users/dev";
const PROJ: &str = "/Users/dev/proj";

/// The three texts JSON.parse reads and serde_json refuses: a number out of f64 range (JavaScript's Infinity), a lone
/// surrogate escape (a JS tool's JSON.stringify writes one where it cut a pair), and nesting past serde_json's 128.
fn poisons() -> Vec<(&'static str, String)> {
    vec![
        ("1e999", "1e999".to_string()),
        ("a lone surrogate escape", r#""\ud83d""#.to_string()),
        ("nesting past 128", format!("{}{}", "[".repeat(129), "]".repeat(129))),
    ]
}

/// `{"zz": <poison>, <pairs>}`.
fn with(poison: &str, pairs: &str) -> String {
    format!("{{\"zz\":{poison},{pairs}}}")
}

/// Runs `run` on the document with each poison in it; every answer must be `want`, the TS's answer.
fn same_as_node(site: &str, want: Value, run: impl Fn(&str) -> Value) {
    let mut differs = Vec::new();
    for (name, poison) in poisons() {
        let got = run(&poison);
        if got != want {
            differs.push(format!("{name}: {got}"));
        }
    }
    assert!(
        differs.is_empty(),
        "{site}: node answered {want} with each of the three; the Rust answered differently for\n{}",
        differs.join("\n")
    );
}

fn on(files: &[(&str, String)]) -> FakeHost {
    let mut laptop = FakeLaptop::new();
    for (path, text) in files {
        laptop = laptop.file(path, text.as_str());
    }
    fake_host(laptop)
}

fn calls_of(reader: &dyn HistoryReader, h: &FakeHost, root: &str, file: &str) -> Value {
    hist(reader, h, root, file)
}

#[test]
fn package_json_with_a_json_text_serde_json_refuses_still_names_its_tools() {
    let want = json!([
        ["pnpm", "packageManager pnpm@9.1.0"],
        ["node", "engines.node >=20"],
        ["docker", "package.json scripts run docker"],
        ["playwright", "package.json depends on playwright 1"]
    ]);
    same_as_node("package.json (project/readers/package_json.rs:38; TS package-json.ts:15)", want, |poison| {
        let pkg = r#""packageManager":"pnpm@9.1.0","engines":{"node":">=20"},"scripts":{"t":"docker compose up"},"devDependencies":{"playwright":"1"}"#;
        let h = on(&[(&format!("{PROJ}/package.json"), with(poison, pkg))]);
        let scan = block_on(scan_project(&h.host, PROJ));
        json!(scan.rows.iter().map(|r| json!([r.id, r.why])).collect::<Vec<_>>())
    });
}

#[test]
fn a_claude_transcript_line_serde_json_refuses_still_yields_its_bash_call() {
    let want = json!({ "out": [{ "session": "s", "folder": "/work", "at": 1767323045000i64, "kind": "shell", "line": "docker ps" }] });
    same_as_node("history/claude.rs:30 try_json (TS claude.ts:34)", want, |poison| {
        let line = format!(
            r#"{{"zz":{poison},"type":"assistant","cwd":"/work","timestamp":"2026-01-02T03:04:05.000Z","message":{{"content":[{{"type":"tool_use","name":"Bash","input":{{"command":"docker ps"}}}},{{"type":"text","text":"x"}}]}}}}"#
        );
        let h = on(&[(&format!("{H}/.claude/projects/p/s.jsonl"), format!("{line}\n"))]);
        calls_of(&ClaudeReader, &h, &format!("{H}/.claude/projects"), &format!("{H}/.claude/projects/p/s.jsonl"))
    });
}

#[test]
fn a_codex_rollout_line_serde_json_refuses_still_yields_its_shell_call() {
    let want = json!({ "out": [{ "session": "r", "at": 1767323045000i64, "kind": "shell", "line": "gh pr list" }] });
    same_as_node("history/codex.rs:49 try_json (TS codex.ts:33)", want, |poison| {
        let line = format!(
            r#"{{"zz":{poison},"timestamp":"2026-01-02T03:04:05.000Z","type":"response_item","payload":{{"type":"function_call","name":"exec_command","arguments":"{{\"cmd\":\"gh pr list\"}}"}}}}"#
        );
        let file = format!("{H}/.codex/sessions/2026/01/02/r.jsonl");
        let h = on(&[(&file, format!("{line}\n"))]);
        calls_of(&CodexReader, &h, &format!("{H}/.codex/sessions"), &file)
    });
}

#[test]
fn a_codex_arguments_string_serde_json_refuses_still_yields_its_shell_call() {
    let want = json!({ "out": [{ "session": "r", "kind": "shell", "line": "gh pr list" }] });
    same_as_node("history/codex.rs:58 try_json of the arguments string (TS codex.ts:38)", want, |poison| {
        let args = format!(r#"{{"cmd":"gh pr list","zz":{poison}}}"#);
        let line = format!(
            r#"{{"type":"response_item","payload":{{"type":"function_call","name":"exec_command","arguments":{}}}}}"#,
            serde_json::to_string(&args).unwrap()
        );
        let file = format!("{H}/.codex/sessions/2026/01/02/r.jsonl");
        let h = on(&[(&file, format!("{line}\n"))]);
        calls_of(&CodexReader, &h, &format!("{H}/.codex/sessions"), &file)
    });
}

const HERMES_QUERY: &str =
    "select m.session_id, m.tool_calls, s.cwd from messages m left join sessions s on s.id = m.session_id where m.tool_calls is not null";

fn hermes_host(sqlite_says: String) -> FakeHost {
    let db = format!("{H}/.hermes/state.db");
    fake_host(FakeLaptop::new().file(&db, 100).exec(&format!("sqlite3 -readonly -json {db} {HERMES_QUERY}"), &sqlite_says))
}

#[test]
fn a_hermes_sqlite_answer_serde_json_refuses_still_yields_its_terminal_call() {
    let want = json!({ "out": [{ "session": "s1", "folder": "/w", "kind": "shell", "line": "gh pr list" }] });
    same_as_node("history/hermes.rs:75 try_json of sqlite3's answer (TS hermes.ts:49)", want, |poison| {
        let calls = r#"[{"function":{"name":"terminal","arguments":"{\"command\":\"gh pr list\"}"}}]"#;
        let row = format!(r#"{{"session_id":"s1","tool_calls":{},"cwd":"/w","zz":{poison}}}"#, serde_json::to_string(calls).unwrap());
        let h = hermes_host(format!("[{row}]"));
        hist(&HermesReader, &h, "", &format!("{H}/.hermes/state.db"))
    });
}

#[test]
fn a_hermes_tool_calls_column_serde_json_refuses_still_yields_its_terminal_call() {
    let want = json!({ "out": [{ "session": "s1", "folder": "/w", "kind": "shell", "line": "gh pr list" }] });
    same_as_node("history/hermes.rs:46,52 try_json of tool_calls and of arguments (TS hermes.ts:23,28)", want, |poison| {
        let args = format!(r#"{{"command":"gh pr list","zz":{poison}}}"#);
        let calls =
            format!(r#"[{{"function":{{"name":"terminal","arguments":{}}},"zz":{poison}}}]"#, serde_json::to_string(&args).unwrap());
        let row = format!(r#"{{"session_id":"s1","tool_calls":{},"cwd":"/w"}}"#, serde_json::to_string(&calls).unwrap());
        let h = hermes_host(format!("[{row}]"));
        hist(&HermesReader, &h, "", &format!("{H}/.hermes/state.db"))
    });
}

#[test]
fn npm_pnpm_and_pipx_listings_serde_json_refuses_still_list_their_packages() {
    let npm = json!([{ "name": "typescript", "version": "5.0.0" }]);
    same_as_node("detect/tools.rs:109 parse_npm_globals via try_json (TS tools.ts:57)", npm, |poison| {
        pkgs(parse_npm_globals(&with(poison, r#""dependencies":{"typescript":{"version":"5.0.0"},"corepack":{"version":"1"}}"#)))
    });
    let pnpm = json!([{ "name": "vite", "version": "5.0.0", "path": "/p/vite" }]);
    same_as_node("detect/tools.rs:160 parse_pnpm_globals via try_json (TS tools.ts:98)", pnpm, |poison| {
        let dir = with(poison, r#""dependencies":{"vite":{"version":"5.0.0","path":"/p/vite"}}"#);
        pkgs(parse_pnpm_globals(&format!("[{dir}]")))
    });
    let pipx = json!([{ "name": "black", "version": "24.1" }]);
    same_as_node("detect/tools.rs:119 parse_pipx_list via try_json (TS tools.ts:69)", pipx, |poison| {
        pkgs(parse_pipx_list(&with(poison, r#""venvs":{"black":{"metadata":{"main_package":{"package_version":"24.1"}}}}"#)))
    });
}

#[test]
fn a_global_packages_own_package_json_serde_json_refuses_still_names_its_commands() {
    same_as_node("detect/tools.rs:410 package_commands via try_json (TS tools.ts:204)", json!(["foo"]), |poison| {
        let h = on(&[("/g/node_modules/foo/package.json", with(poison, r#""name":"foo","bin":{"foo":"cli.js"}"#))]);
        json!(block_on(package_commands(&h.host, "foo", &["/g/node_modules/foo".to_string()])))
    });
}

#[test]
fn claudes_settings_and_credentials_serde_json_refuses_still_read() {
    same_as_node("detect/logins.rs:133 api_key_helper_of (TS logins.ts:93)", json!("/bin/key"), |poison| {
        json!(api_key_helper_of(Some(&with(poison, r#""apiKeyHelper":"/bin/key""#))))
    });
    same_as_node("detect/logins.rs:166 holds_claude_login (TS logins.ts:107)", json!(true), |poison| {
        json!(holds_claude_login(Some(&with(poison, r#""claudeAiOauth":{"accessToken":"t"}"#))))
    });
}

#[test]
fn a_claude_settings_file_serde_json_refuses_still_names_its_hook_scripts() {
    let want = json!(["~/.claude/settings.json", "~/scripts/x.sh"]);
    same_as_node("catalog jsonc.rs:15 read_jsonc under hooks.rs:264 (TS jsonc.ts:41, hooks.ts:138)", want, |poison| {
        let hooks =
            format!(r#""hooks":{{"PreToolUse":[{{"matcher":"Bash","hooks":[{{"type":"command","command":"{H}/scripts/x.sh"}}]}}]}}"#);
        let h = fake_host(
            FakeLaptop::new().file("~/.claude/settings.json", with(poison, &hooks).as_str()).file("~/scripts/x.sh", 10).which(&["claude"]),
        );
        let rows = block_on(detect_agents(&h.host)).unwrap();
        json!(rows.iter().flat_map(|r| r.paths.clone()).collect::<Vec<_>>())
    });
}

fn mcp_rows(h: &FakeHost) -> Value {
    json!(block_on(detect_mcp(&h.host, None)).iter().map(|r| json!([r.id, r.detail])).collect::<Vec<_>>())
}

#[test]
fn a_claude_json_serde_json_refuses_still_lists_its_servers() {
    let want = json!([
        ["agents/mcp/claude/gh", "stdio: npx pkg; runs via npx; carries a secret: env GITHUB_TOKEN (3 B)"],
        ["agents/mcp/claude/remote", "http: r.example/mcp; nothing to install; its sign-in is kept with the Claude Code login"]
    ]);
    same_as_node("catalog jsonc.rs:15 read_jsonc under detect/mcp.rs detect_mcp (TS jsonc.ts:41)", want, |poison| {
        let servers = r#""mcpServers":{"gh":{"command":"npx","args":["-y","pkg"],"env":{"GITHUB_TOKEN":"abc"}},"remote":{"type":"http","url":"https://r.example/mcp"}}"#;
        mcp_rows(&on(&[("~/.claude.json", with(poison, servers))]))
    });
}

#[test]
fn a_gemini_settings_file_serde_json_refuses_still_lists_its_servers() {
    let want = json!([["agents/mcp/gemini/g", "stdio: npx g; runs via npx; carries no secret"]]);
    same_as_node("catalog jsonc.rs:15 read_jsonc under detect/mcp.rs detect_mcp", want, |poison| {
        mcp_rows(&on(&[("~/.gemini/settings.json", with(poison, r#""mcpServers":{"g":{"command":"npx","args":["g"]}}"#))]))
    });
}

#[test]
fn a_project_mcp_json_serde_json_refuses_still_lists_its_server() {
    let want = json!({ "servers": [["claude", "p"]], "read": ["claude"] });
    same_as_node("catalog jsonc.rs:15 read_jsonc under detect/mcp_project.rs (TS jsonc.ts:41)", want, |poison| {
        let h = on(&[(&format!("{PROJ}/.mcp.json"), with(poison, r#""mcpServers":{"p":{"command":"npx","args":["p"]}}"#))]);
        let o = ProjectMcpOptions { agents: wsp_catalog::catalog::mcp_agents(), keep: vec![] };
        let r = block_on(detect_project_mcp(&h.host, PROJ, &o));
        json!({ "servers": r.servers.iter().map(|x| json!([x.agent.id, x.turn.server.name])).collect::<Vec<_>>(), "read": r.read })
    });
}

#[test]
fn server_names_that_read_as_numbers_come_first_in_a_claude_json() {
    // JavaScript lists "1", "2", "10" first and ascending, then the other keys in file order.
    let text = r#"{"mcpServers":{"b":{"command":"npx"},"10":{"command":"npx"},"2":{"command":"npx"},"a":{"command":"npx"},"1":{"command":"npx"}}}"#;
    let rows = block_on(detect_mcp(&on(&[("~/.claude.json", text.to_string())]).host, None));
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        ["agents/mcp/claude/1", "agents/mcp/claude/2", "agents/mcp/claude/10", "agents/mcp/claude/b", "agents/mcp/claude/a"],
        "catalog mcp_json.rs reads `mcpServers` in text order; node lists integer-like names first"
    );
}

fn ids_of(files: &[(&str, String)]) -> Vec<String> {
    block_on(detect_mcp(&on(files).host, None)).iter().map(|r| r.id.clone()).collect()
}

const NUMBERED: &str = r#"{"b":{"command":"npx"},"10":{"command":"npx"},"2":{"command":"npx"},"a":{"command":"npx"}}"#;

#[test]
fn server_names_that_read_as_numbers_come_first_in_a_gemini_settings_file() {
    let got = ids_of(&[("~/.gemini/settings.json", format!(r#"{{"mcpServers":{NUMBERED}}}"#))]);
    assert_eq!(
        got,
        ["agents/mcp/gemini/2", "agents/mcp/gemini/10", "agents/mcp/gemini/b", "agents/mcp/gemini/a"],
        "catalog mcp_json.rs, node's order"
    );
}

#[test]
fn server_names_that_read_as_numbers_come_first_in_an_opencode_file() {
    let servers = r#"{"b":{"type":"local","command":["x"]},"10":{"type":"local","command":["x"]},"2":{"type":"local","command":["x"]},"a":{"type":"local","command":["x"]}}"#;
    let got = ids_of(&[("~/.config/opencode/opencode.json", format!(r#"{{"mcp":{servers}}}"#))]);
    assert_eq!(
        got,
        ["agents/mcp/opencode/2", "agents/mcp/opencode/10", "agents/mcp/opencode/b", "agents/mcp/opencode/a"],
        "catalog mcp_json.rs, node's order"
    );
}

#[test]
fn server_names_that_read_as_numbers_come_first_under_a_claude_home_entry() {
    let got = ids_of(&[("~/.claude.json", format!(r#"{{"projects":{{"{H}":{{"mcpServers":{NUMBERED}}}}}}}"#))]);
    assert_eq!(
        got,
        ["agents/mcp/claude/home/2", "agents/mcp/claude/home/10", "agents/mcp/claude/home/b", "agents/mcp/claude/home/a"],
        "catalog mcp_json.rs, node's order"
    );
}

#[test]
fn server_names_that_read_as_numbers_come_first_in_a_project_mcp_json() {
    let h = on(&[(&format!("{PROJ}/.mcp.json"), format!(r#"{{"mcpServers":{NUMBERED}}}"#))]);
    let o = ProjectMcpOptions { agents: wsp_catalog::catalog::mcp_agents(), keep: vec![] };
    let r = block_on(detect_project_mcp(&h.host, PROJ, &o));
    let got: Vec<&str> = r.servers.iter().map(|x| x.turn.server.name.as_str()).collect();
    assert_eq!(got, ["2", "10", "b", "a"], "catalog mcp_json.rs through detect/mcp_project.rs, node's order");
}

fn codex(text: &str) -> Value {
    mcp_rows(&on(&[("~/.codex/config.toml", text.to_string())]))
}

const NPX_ROW: &str = "stdio: npx; runs via npx; carries no secret";

#[test]
fn a_codex_table_header_with_a_next_line_character_is_not_a_header() {
    // `\s` in a JavaScript regex does not hold U+0085; the Rust regex crate's `\s` does.
    assert_eq!(codex("[mcp_servers.x\u{85}]\ncommand = \"npx\"\n"), json!([]), "catalog mcp_toml.rs:42 header_re: node has no server here");
}

#[test]
fn a_codex_table_header_with_a_byte_order_mark_inside_is_a_header() {
    // `\s` in a JavaScript regex holds U+FEFF; the Rust regex crate's `\s` does not.
    assert_eq!(
        codex("[mcp_servers.x\u{feff}]\ncommand = \"npx\"\n"),
        json!([["agents/mcp/codex/x", NPX_ROW]]),
        "catalog mcp_toml.rs:42 header_re: node reads server x"
    );
}

fn codex_env(key: &str) -> Value {
    codex(&format!("[mcp_servers.x]\ncommand = \"npx\"\nenv = {{ {key} = \"t\" }}\n"))
}

#[test]
fn an_inline_env_key_followed_by_a_next_line_character_is_no_key() {
    assert_eq!(
        codex_env("A_TOKEN\u{85}"),
        json!([["agents/mcp/codex/x", NPX_ROW]]),
        "catalog mcp_toml.rs:147: node finds no key before U+0085, so no secret"
    );
}

#[test]
fn an_inline_env_key_followed_by_a_byte_order_mark_is_a_key() {
    assert_eq!(
        codex_env("A_TOKEN\u{feff}"),
        json!([["agents/mcp/codex/x", "stdio: npx; runs via npx; carries a secret: env A_TOKEN (1 B)"]]),
        "catalog mcp_toml.rs:147: node reads A_TOKEN before U+FEFF"
    );
}

#[test]
fn the_guard_note_reads_as_the_ts_writes_it_now() {
    // packages/collect/src/detect/commands.ts:162 lost " Tick them in wsp init to install them." in 383708c78, after the
    // Rust was ported from a3b870a6e; the recorded fixtures still hold the old line.
    let got = wsp_collect::detect::commands::guard_commands("a\nb\n", &["x".to_string()], &[]);
    let want = "# >>> wsp guard: commands this machine does not have >>>\nx() { if (unset -f x; command -v x) >/dev/null 2>&1; then unset -f x; x \"$@\"; else return 127; fi; }\n# Guarded above: a call to one of these that is not on this machine is silent instead of an error: x.\n# <<< wsp guard <<<\n\na\nb\n";
    assert_eq!(got, want, "detect/commands.rs:355 against commands.ts:162");
}

#[test]
fn a_codex_string_with_a_surrogate_escape_decodes_it_as_the_ts_does() {
    // TS: String.fromCharCode(0xD800), one lone surrogate unit in the argument; Rust's char::from_u32 refuses it and
    // keeps the six characters of the escape.
    let text = "[mcp_servers.x]\ncommand = \"npx\"\nargs = [\"a\\ud800b\"]\n";
    let rows = block_on(detect_mcp(&on(&[("~/.codex/config.toml", text.to_string())]).host, None));
    let detail = rows[0].detail.clone().unwrap();
    assert!(
        !detail.contains("\\ud800"),
        "catalog mcp_toml.rs:67 decode_toml: node decodes the escape to U+D800; the Rust row still says {detail}"
    );
}

fn cache_text(commands: &str, calls: &str, extra: &str) -> String {
    format!(
        r#"{{"version":1,"parse":2,"files":{{"/f":{{"stamp":"1:2","buckets":[{{"session":"s","calls":{calls},"commands":{commands},"installs":{{}}{extra}}}]}}}}}}"#
    )
}

fn cached(text: &str, tag: &str) -> Option<Vec<wsp_collect::history::tally::HistoryBucket>> {
    use wsp_collect::history::cache::{file_history_cache, HistoryCache};
    let dir = std::env::temp_dir().join(format!("rev-parse-cache-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("c.json");
    std::fs::write(&file, text).unwrap();
    let got = file_history_cache(&file).get("/f", "1:2");
    let _ = std::fs::remove_dir_all(&dir);
    got
}

#[test]
fn a_history_cache_with_a_lone_surrogate_escape_is_still_a_hit() {
    // A JS host writes `\ud83d` for a cut pair; node reads the file back and the counts are used. The Rust refuses the file,
    // so every session file is read again (history/cache.rs:99).
    let hit = cached(&cache_text(r#"{"a\ud83d":2}"#, "2", ""), "surrogate");
    assert!(hit.is_some(), "node answered one bucket {{session s, calls 2}}; the Rust cache missed");
}

#[test]
fn a_history_cache_with_nesting_past_128_is_still_a_hit() {
    let deep = format!(r#","zz":{}{}"#, "[".repeat(129), "]".repeat(129));
    let hit = cached(&cache_text(r#"{"git":2}"#, "2", &deep), "deep");
    assert!(hit.is_some(), "node answered one bucket {{session s, calls 2}}; the Rust cache missed");
}

#[test]
fn a_history_cache_command_named_proto_is_dropped_as_node_drops_it() {
    // node: commands {} (the zod record leaves the key out). Rust keeps `__proto__: 2` (history/cache.rs:67).
    let hit = cached(&cache_text(r#"{"__proto__":2}"#, "2", ""), "proto").expect("node answered one bucket");
    assert!(hit[0].commands.is_empty(), "node answered commands {{}}; the Rust bucket holds {:?}", hit[0].commands);
}

#[test]
fn a_history_cache_count_past_u64_is_kept_as_node_keeps_it() {
    // node: calls 1e+20 (a safe-integer check is not made); Rust's `n as u64` saturates at 18446744073709551615.
    let hit = cached(&cache_text(r#"{"git":2}"#, "1e20", ""), "big").expect("node answered one bucket");
    assert_eq!(hit[0].calls as f64, 1e20, "history/cache.rs:61 count()");
}

#[test]
fn a_url_with_a_mongolian_vowel_separator_in_its_host_is_not_a_plain_address() {
    // V8's URL refuses the host (U+180E); the url crate takes it, so the Rust calls the argument plain (mcp_args.rs:243).
    let arg = "https://x.e\u{180e}xample/#\nhttps://x.e\u{180e}xample/#";
    assert!(!wsp_collect::detect::mcp_args::plain_arg(arg), "node answered false");
}

const CODEX_PROJECT_FILE: &str = "[mcp_servers.p]\ncommand = \"npx\"\nargs = [\"p\"]\n";

/// What the project read finds for a user file at ~/.codex/config.toml that holds `body` then trusts the folder.
fn codex_project(body: &str) -> Value {
    let user = format!("{body}\n[projects.\"{PROJ}\"]\ntrust_level = \"trusted\"\n");
    let h = on(&[("~/.codex/config.toml", user), (&format!("{PROJ}/.codex/config.toml"), CODEX_PROJECT_FILE.to_string())]);
    let o =
        ProjectMcpOptions { agents: wsp_catalog::catalog::mcp_agents().into_iter().filter(|a| a.id == "codex").collect(), keep: vec![] };
    let r = block_on(detect_project_mcp(&h.host, PROJ, &o));
    json!({ "servers": r.servers.iter().map(|x| json!([x.agent.id, x.turn.server.name])).collect::<Vec<_>>(), "read": r.read })
}

#[test]
fn a_codex_config_smol_toml_reads_still_trusts_the_folder() {
    // smol-toml (the TS's parser) takes TOML 1.1 forms and impossible dates; the toml crate's 0.8 parser refuses the file,
    // so the project's .codex/config.toml is not read at all. Node: the folder is trusted and server p is found.
    let want = json!({ "servers": [["codex", "p"]], "read": ["codex"] });
    let mut differs = Vec::new();
    for (what, body) in [
        ("an inline table with a trailing comma", "t = { a = 1, }"),
        ("an inline table over two lines", "t = { a = 1,\n b = 2 }"),
        ("the escape \\e", "s = \"\\e\""),
        ("a time with no seconds", "d = 07:32Z"),
        ("February 30", "d = 2024-02-30"),
        ("February 29 in a year with none", "d = 2023-02-29T00:00:00Z"),
        ("a float past f64 range", "e = 1e400"),
    ] {
        let got = codex_project(body);
        if got != want {
            differs.push(format!("{what}: {got}"));
        }
    }
    assert!(
        differs.is_empty(),
        "catalog mcp_codex.rs:65 (toml 0.8) against smol-toml in mcp-codex.ts:44: node answered {want} for each; the Rust answered\n{}",
        differs.join("\n")
    );
}

#[test]
fn a_codex_config_smol_toml_refuses_does_not_trust_the_folder() {
    // smol-toml refuses an integer it cannot hold exactly in a double and a leap second; the toml crate takes both.
    // Node: the file does not parse, the folder is not trusted, no project server is found.
    let want = json!({ "servers": [], "read": [] });
    let mut differs = Vec::new();
    for (what, body) in [
        ("an integer past 2^53", "n = 9007199254740993"),
        ("i64::MAX", "n = 9223372036854775807"),
        ("i64::MIN", "n = -9223372036854775808"),
        ("a leap second", "d = 1979-05-27T07:32:60Z"),
    ] {
        let got = codex_project(body);
        if got != want {
            differs.push(format!("{what}: {got}"));
        }
    }
    assert!(
        differs.is_empty(),
        "catalog mcp_codex.rs:65 (toml 0.8) against smol-toml in mcp-codex.ts:44: node answered {want} for each; the Rust answered\n{}",
        differs.join("\n")
    );
}
