// SPDX-License-Identifier: AGPL-3.0-only
// Review of wsp-catalog against packages/catalog at a3b870a6e (#2445). Each test asserts the value the TS prints on
// the same input, so each fails on the crate as it stands; the name starts with the finding's number.
use futures::executor::block_on;
use indexmap::{IndexMap, IndexSet};
use serde_json::json;

use wsp_catalog::json_edit::edit_json;
use wsp_catalog::jsonc_tree::PathSeg;
use wsp_catalog::mcp::{EditOutcome, McpEditScope, McpFormat, McpMergeScope, McpServer, McpTransport, MergeOutcome, MCP_SERVERS_JSON};
use wsp_catalog::plugins::{plugin_id_parts, turn_claude, FoundPlugin, PeekGroup, Peeked, PluginIo};
use wsp_catalog::road_modules::{row_road, ToolRow};
use wsp_catalog::signin::SignIn;
use wsp_catalog::signin_rows::sign_in_row;
use wsp_wire::types::agents_report::{PluginBrings, PluginScope};
use wsp_words::exit::{HostError, Kind};

const CX: McpFormat = McpFormat::CodexToml;

fn names(servers: Vec<McpServer>) -> Vec<String> {
    servers.into_iter().map(|s| s.name).collect()
}

fn same(s: &str, _command: bool) -> String {
    s.to_string()
}

// 1. MAJOR. signin.ts:433 `code: /(?<=Your auth code is:\s*)[a-z]+(?:_[a-z]+){2,}/`. fancy-regex refuses a lookbehind
// of unbounded width and js_regex.rs:72 `build` turns the refusal into None, so `find` answers nothing for any text.
// node: ["abc_def_ghi","xyz_uvw_rst"].
#[test]
fn f01_the_doppler_sign_in_code_is_read() {
    let SignIn::Login(row) = sign_in_row("doppler") else { panic!("doppler is a login row") };
    let code = row.code.clone().unwrap();
    assert_eq!(code.find("Your auth code is:\nabc_def_ghi\n").as_deref(), Some("abc_def_ghi"));
    assert_eq!(code.find("abc_def_ghi Your auth code is: xyz_uvw_rst").as_deref(), Some("xyz_uvw_rst"));
}

// 2. MAJOR. plugins-claude.ts:283 lets io.edit's error through with its kind; plugins-acts.ts:50 maps only the
// returned `refused` to usage. A refused write exits 3 (usage), a failed one 1 (provider). plugins.rs:817 returns
// Option<String>, so the two come back the same.
struct EditFails(HostError);

impl PluginIo for EditFails {
    fn home(&self) -> &str {
        "/x"
    }
    fn store(&self) -> Option<&str> {
        Some("/x/claude")
    }
    fn seed(&self) -> Option<&str> {
        None
    }
    async fn peek(&self, _groups: &[PeekGroup]) -> Option<Box<dyn Peeked>> {
        None
    }
    async fn run(&self, _line: &str) -> Option<String> {
        None
    }
    fn can_edit(&self) -> bool {
        true
    }
    async fn edit(&self, _file: &str, _base: &str, _change: &dyn Fn(Option<&str>) -> Result<String, HostError>) -> Result<(), HostError> {
        Err(self.0.clone())
    }
}

fn a_plugin(id: &str) -> FoundPlugin {
    let (name, marketplace) = plugin_id_parts(id);
    let none = Vec::new;
    FoundPlugin {
        id: id.to_string(),
        name,
        marketplace,
        version: None,
        scope: PluginScope::User,
        project: None,
        on: true,
        missing: None,
        path: None,
        set_in: None,
        description: None,
        source: None,
        brings: PluginBrings {
            skills: none(),
            commands: none(),
            subagents: none(),
            hooks: none(),
            servers: none(),
            lsp: none(),
            apps: none(),
        },
        skills: None,
    }
}

#[test]
fn f02_a_refused_plugin_write_keeps_its_kind_apart_from_a_failed_one() {
    let words = "~/.claude/settings.json changed since it was read";
    let refused =
        block_on(turn_claude(&EditFails(HostError { kind: Some(Kind::Usage), ..HostError::new(words) }), &a_plugin("p@m"), false));
    let failed = block_on(turn_claude(&EditFails(HostError::new(words)), &a_plugin("p@m"), false));
    assert_ne!(refused, failed, "the TS exits 3 for the first and 1 for the second");
}

// 3. MAJOR. jsonc.ts:12, plugins.ts:75, signin.ts:290 parse with JSON.parse, which takes a lone surrogate escape
// (JSON.stringify writes one for a string cut inside an emoji), 1e999 (Infinity) and any depth. serde_json refuses all
// three (jsonc.rs:25, plugins.rs:208, signin.rs:486), so the whole file reads as nothing. node: ["a"], ["a"], true.
#[test]
fn f03_a_lone_surrogate_escape_in_claude_json_hides_no_server() {
    let text = r#"{"history":[{"display":"cut \ud83d"}],"mcpServers":{"a":{"command":"x"}}}"#;
    assert_eq!(names(MCP_SERVERS_JSON.read(text, "/h")), vec!["a"]);
}

#[test]
fn f03_a_number_past_f64_and_deep_nesting_still_read() {
    let deep = format!(r#"{{"d":{}{},"mcpServers":{{"a":{{"command":"x"}}}}}}"#, "[".repeat(200), "]".repeat(200));
    assert_eq!(names(MCP_SERVERS_JSON.read(&deep, "/h")), vec!["a"]);
    assert_eq!(names(MCP_SERVERS_JSON.read(r#"{"x":1e999,"mcpServers":{"a":{"command":"x"}}}"#, "/h")), vec!["a"]);
}

#[test]
fn f03_claude_status_with_a_lone_surrogate_escape_is_signed_in() {
    assert!(wsp_catalog::signin::claude_signed_in(r#"{"loggedIn": true, "authMethod": "claude.ai", "org": "\ud800"}"#));
}

// 4. MINOR. JavaScript's `m` flag starts a line after \n, \r, U+2028 and U+2029; the regex crate's (?m) only after \n.
// mcp-check.ts:25,33,48, mcp-login.ts:20, signin.ts:520 against mcp_check.rs:22,59,81, mcp_login.rs:60, js_regex.rs:69.
// node: true for each.
#[test]
fn f04_a_line_after_a_lone_carriage_return_is_a_line() {
    use wsp_catalog::mcp_check::{McpCheck, McpCheckWord};
    use wsp_wire::types::agents_report::McpAuth;
    let claude = "Checking MCP server health...\rnotion:\r  Status: \u{2714} Connected\r";
    assert_eq!(McpCheck::Claude.auth(claude, "x"), Some(McpCheckWord::Auth(McpAuth::SignedIn)));
    let codex = "warning: x\r[{\"name\":\"a\",\"auth_status\":\"o_auth\"}]\n";
    assert_eq!(McpCheck::Codex.auth(codex, "a"), Some(McpCheckWord::Auth(McpAuth::SignedIn)));
    assert!(sign_in_row("pi").status().unwrap().signed_in("x\rprovider model", 0));
}

// 5. MINOR. The words of a parse failure a person reads are the parser's: V8's for JSON (editJson, withClaudeSwitches,
// the plugin switch) and smol-toml's for TOML (mcp.ts:1665 through servers-acts.ts:257, mcp-codex.ts). The Rust shows
// serde_json's and the toml crate's (json_edit.rs, mcp_toml_refer.rs:183, mcp_codex.rs:73).
#[test]
fn f05_a_toml_file_that_does_not_parse_is_refused_in_smol_toml_s_words() {
    let e = CX.refer("[mcp_servers.a]\ncommand = \"x\"\n[mcp_servers.a]\nargs = []\n", Some("a"), &[], &IndexSet::new()).unwrap_err();
    assert_eq!(e.message, "the file is not valid TOML (Invalid TOML document: trying to redefine an already defined table or value)");
}

#[test]
fn f05_a_json_file_that_does_not_parse_is_refused_in_v8_s_words() {
    let e = edit_json("not json", &[(vec![PathSeg::Key("a".into())], Some(json!(1)))]).unwrap_err();
    assert_eq!(e.message, "Unexpected token 'o', \"not json\" is not valid JSON");
}

// 6. MINOR. smol-toml 1.9 reads TOML 1.1 (a multi-line inline table, a trailing comma in one, a time with no
// seconds); the toml 0.8 crate refuses them, so a trusted folder reads untrusted and refer refuses the file.
#[test]
fn f06_a_trailing_comma_in_an_inline_table_is_read() {
    let t11 = "[projects.\"/work/a\"]\ntrust_level = \"trusted\"\nx = {\n  a = 1,\n}\n";
    assert!(wsp_catalog::mcp_codex::codex_trusts(t11, "/work/a"));
    let text = "[mcp_servers.a]\ncommand = \"x\"\nenv = { A = \"1\", }\n";
    assert_eq!(CX.refer(text, Some("a"), &[], &IndexSet::new()).unwrap().text, "[mcp_servers.a]\nenv_vars = [\"A\"]\ncommand = \"x\"\n");
}

// 7. MINOR. mcp.ts:1292 decodeToml: `String.fromCodePoint(NaN)` throws on `\u` or `\U` without its hex digits, so read
// throws (agents-here.ts:35 reads that as no server) and edit and merge refuse; a pair of surrogate escapes is one
// character. mcp_toml.rs:60 keeps the escape as written and reads on.
#[test]
fn f07_a_bad_unicode_escape_is_not_read_on() {
    let text = "[mcp_servers.a]\ncommand = \"C:\\users\\me\\node.exe\"\n";
    assert!(wsp_catalog::mcp_toml::read_codex(text).is_empty());
    let keep = McpEditScope { keep: vec!["a".into()], drop: vec![], project: None };
    assert!(CX.edit(&same, &keep, text).is_err());
}

#[test]
fn f07_two_surrogate_escapes_read_as_one_character() {
    let servers = wsp_catalog::mcp_toml::read_codex("[mcp_servers.a]\ncommand = \"x\"\nargs = [\"\\uD83D\\uDE00\"]\n");
    let McpTransport::Stdio { args, .. } = &servers[0].transport else { panic!("stdio") };
    assert_eq!(args, &vec!["\u{1F600}".to_string()]);
}

// 8. MINOR. mcp-launch.ts:230 joins through `[...new Set([...existing, ...names])]`, so ~/.claude.json gets a name the
// file listed twice once; mcp_launch.rs:294 dedups only the new names.
#[test]
fn f08_disabled_servers_listed_twice_are_written_once() {
    let road = wsp_catalog::mcp_launch::launch_server_road("claude").unwrap();
    let switches: IndexMap<String, Vec<String>> = [("disabledMcpServers".to_string(), vec!["a".to_string()])].into_iter().collect();
    let text = r#"{"projects": {"/w/a": {"disabledMcpServers": ["x", "y", "x"]}}}"#;
    let got = road.switches_put(Some(text), "/w/a", &switches).unwrap().unwrap();
    assert_eq!(got, r#"{"projects": {"/w/a": {"disabledMcpServers": ["x","y","a"]}}}"#);
}

// 9. MINOR. A JavaScript object lists integer-like keys first, ascending, so the TS writes and lists digit-named
// servers and env keys first (mcp.ts:742,1138,1147, jsonc.ts:44); the Rust keeps text order (jsonc.rs js_stringify,
// mcp_json.rs:535,563). The spec accepts it for reads; these are writes.
#[test]
fn f09_digit_named_servers_are_written_first() {
    let scope = McpMergeScope { keep: vec!["b".into(), "1".into()], drop: vec![], project: None, replace: vec![] };
    let travelled = r#"{"mcpServers":{"b":{"command":"x"},"1":{"command":"y"}}}"#;
    let got = MCP_SERVERS_JSON.merge(&same, &scope, Some(r#"{"x": 1}"#), travelled).unwrap();
    assert_eq!(got.text, r#"{"x": 1, "mcpServers": {"1":{"command":"y"},"b":{"command":"x"}}}"#);
}

// 10. MINOR. mcp.ts:802,816,881,1147 read `servers[name]` off a plain object, so a server named for an
// Object.prototype member finds the inherited one. The Rust reads only the file's own keys. Likely an after-port
// ticket rather than a port fix: listed so the choice is made on purpose.
#[test]
fn f10_a_server_named_constructor_reads_as_the_ts_reads_it() {
    let own = r#"{"mcpServers": {"a": {"command": "x"}}}"#;
    let scope = McpEditScope { keep: vec!["constructor".into()], drop: vec![], project: None };
    assert_eq!(MCP_SERVERS_JSON.edit(&same, &scope, own).unwrap().results[0].outcome, EditOutcome::Written);
    let merge = McpMergeScope { keep: vec!["constructor".into()], drop: vec![], project: None, replace: vec![] };
    let got = MCP_SERVERS_JSON.merge(&same, &merge, Some(own), r#"{"mcpServers":{"constructor":{"command":"/b"}}}"#).unwrap();
    assert_eq!(got.results[0].outcome, MergeOutcome::Theirs);
}

// 11. MINOR. readJsonc takes `{,}` and jsonc-parser's parseTree recovers a tree from it, so the TS edits the file;
// jsonc_tree.rs:119 gives up and the edit is refused.
#[test]
fn f11_an_object_holding_only_a_comma_is_edited() {
    assert_eq!(edit_json("{,}", &[(vec![PathSeg::Key("other".into())], Some(json!(5)))]).unwrap(), "{\"other\": 5,}");
}

// 12. MINOR. mcp.ts:1008-1013 refuses a server for its oauth shape before it reads any value; mcp_json_refer.rs:61-63
// reads the values first, so a server with both faults gets the other refusal.
#[test]
fn f12_the_oauth_shape_refusal_comes_before_a_mixed_value() {
    let text = r#"{"mcpServers":{"t":{"url":"https://h/m","headers":{"X":"x${A}y"},"oauth":[{"clientSecret":"s"}]}}}"#;
    let e = MCP_SERVERS_JSON.refer(text, None, &[], &IndexSet::new()).unwrap_err();
    assert_eq!(e.message, "mcpServers.t is written in a shape wsp does not read, so its values cannot be written by name");
}

// 13. MINOR. first-run.ts:68 quiets sed with `2>/dev/null` inside `$(...)`; first_run.rs:97 drops it, so a missing
// launcher prints sed's complaint on the computer's stderr every check.
#[test]
fn f13_the_lazy_run_check_keeps_sed_quiet() {
    let ts = r##"{ case "$(sed -n 2p '/usr/local/lib/ascii-harnesses/claude' 2>/dev/null)" in "# ascii-lazy-harness "*) false ;; esac || { d=$(dirname "$(command -v 'claude')") && PATH=$(printf %s "$PATH" | tr : "\n" | grep -vxF "$d" | paste -sd: -) command -v 'claude' >/dev/null 2>&1; }; }"##;
    assert_eq!(wsp_catalog::first_run::FirstRunInstaller::LazyRun.installed("/usr/local/lib/ascii-harnesses/claude"), ts);
}

// 14. NIT. Fixed patterns on the regex crate's sets where JavaScript's differ: `\s` takes U+0085 and not U+FEFF, and
// (?i) folds U+017F to s and U+212A to k. road-modules.ts:401 against road_modules.rs:345; mcp-login.ts:71 against
// mcp_login.rs:136; mcp.ts value forms against mcp_refs.rs:19.
#[test]
fn f14_fixed_patterns_use_javascript_s_white_space_and_case_folding() {
    let row = ToolRow { name: "old".into(), version: None, paths: vec![], label: "old (a@b\u{feff}c)".into() };
    assert_eq!(serde_json::to_value(row_road("go", &row).unwrap()).unwrap(), json!({ "road": "go" }));
    assert_eq!(wsp_catalog::mcp_refs::value_forms("ba\u{17f}ic x"), vec!["ba\u{17f}ic x"]);
}

// 15. NIT. config-write.ts:46 `file.slice(0, -1)` cuts a UTF-16 unit where the file has no `/`; config_write.rs:46 cuts
// a byte and panics inside a wide character. Every caller passes an absolute path.
#[test]
fn f15_a_landing_beside_a_bare_wide_name_does_not_panic() {
    assert_eq!(wsp_catalog::config_write::config_landing("\u{e9}", "0123456789ab").unwrap(), "/.wsp-config-tmp.0123456789ab");
}

// 16. NIT. mcp-launch.ts:326 `.sort()` orders by UTF-16 units (U+10000 before U+E000); mcp_launch.rs:565 by bytes. The
// spec's rule 11 says UTF-16 order.
#[test]
fn f16_launch_misses_sort_in_utf16_order() {
    let text = "[mcp_servers.a]\ncommand = \"x\"\nenv_vars = [\"\\ue000\", \"\\U00010000\", \"B\", \"a\"]\n";
    let server = &CX.read(text, "/h")[0];
    let held: IndexSet<String> = ["\u{e000}", "\u{10000}", "B", "a"].iter().map(|s| s.to_string()).collect();
    let got = wsp_catalog::mcp_launch::launch_misses("none", CX, server, &held);
    assert_eq!(got, vec!["B", "a", "\u{10000}", "\u{e000}"]);
}

// 17. NIT. checkout_of uses Rust's trim (keeps a BOM, takes U+0085); JavaScript's is the reverse. Git prints neither.
#[test]
fn f17_checkout_of_trims_as_javascript_trims() {
    let got = wsp_catalog::mcp_launch::checkout_of(Some("\u{feff}/a/b\n/a/b/.git"), "/fold");
    assert_eq!(got.top.as_deref(), Some("/a/b"));
}

// 18. NIT. The written-out Codex fallback: smol-toml writes 20.0 as 20; the toml crate keeps 20.0. A value type, not
// the accepted toml-layout class.
#[test]
fn f18_the_fallback_writes_a_whole_float_as_smol_toml_does() {
    let text = "[mcp_servers.a]\ncommand = \"x\"\nstartup_timeout_sec = 20.0\nenv.A = \"1\"\n";
    assert!(CX.refer(text, Some("a"), &[], &IndexSet::new()).unwrap().text.contains("startup_timeout_sec = 20\n"));
}

// 19. NIT. Key order of serialized structs the TS builds in another order: PluginsFound (file before version) and
// ContextHooks (path and note before files). No TS caller serializes either whole today.
#[test]
fn f19_context_hooks_write_their_keys_in_the_ts_order() {
    use wsp_catalog::context::{AgentContext, ContextHookInput, GuestRoots};
    let input = ContextHookInput {
        short: "S".into(),
        skill: "K".into(),
        source: "/src".into(),
        roots: GuestRoots { etc: "/etc".into(), home: "/home".into() },
        conflict: false,
        fish: false,
    };
    let got = serde_json::to_string(&AgentContext::Claude.hooks(&input)).unwrap();
    assert!(got.starts_with(r#"{"outcome":"written","path":"/etc/claude-code/CLAUDE.md","files":"#), "{got}");
}
