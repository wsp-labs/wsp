// SPDX-License-Identifier: AGPL-3.0-only
// The text helpers on edge inputs against tests/fixtures/text.json, which the TS catalog wrote
// (oracle/text.oracle.ts): reading jsonc, writing JSON as JavaScript writes it, the in-place editor, TOML lines,
// bearer names, the travelled copy, cksum, the Codex app server's script and answers, the harness checks, the OAuth
// store, hook words and the plugin path helpers.
mod oracle_common;

use std::cell::RefCell;

use indexmap::IndexMap;
use oracle_common::{fixture, json, opt, s, same};
use serde_json::{json, Value};
use wsp_catalog::config_write::config_sum;
use wsp_catalog::hooks::{hook_files, on_machine, path_of, place_word, runs_a_file, HookCarry, PlacedWord};
use wsp_catalog::json_edit::{edit_json, JsonEdit};
use wsp_catalog::jsonc::{js_stringify, js_stringify_pretty, read_jsonc};
use wsp_catalog::jsonc_tree::PathSeg;
use wsp_catalog::mcp::MCP_SERVERS_JSON;
use wsp_catalog::mcp_bearer::{bearer_of, mcp_bearer_variable, with_bearer_variables};
use wsp_catalog::mcp_check::{McpCheck, McpCheckWord};
use wsp_catalog::mcp_oauth::McpOAuthStore;
use wsp_catalog::mcp_toml::{rewrite_toml_line, toml_open_array, toml_strings, uncomment_toml};
use wsp_catalog::mcp_travelled::{travelled_servers, FromTo};
use wsp_catalog::plugins::{base_name, in_store, join_path, plain_values, plugin_id_parts};
use wsp_catalog::plugins_codex::{answers_of, app_server_script, initialize_request, notification, request, ServerAnswer, ServerStage};
use wsp_catalog::skills::BundledSkills;

fn text() -> Value {
    fixture("text.json")
}

fn path_of_json(v: &Value) -> Vec<PathSeg> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|p| if let Some(n) = p.as_i64() { PathSeg::Index(n) } else { PathSeg::Key(s(p).to_string()) })
        .collect()
}

fn edits_of(v: &Value) -> Vec<JsonEdit> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|e| (path_of_json(&e["path"]), if e["value"] == json!("UNDEFINED") { None } else { Some(e["value"].clone()) }))
        .collect()
}

fn check_word(w: Option<McpCheckWord>) -> Value {
    match w {
        None => Value::Null,
        Some(McpCheckWord::NotSetUp) => json!("not-set-up"),
        Some(McpCheckWord::Auth(a)) => json(a),
    }
}

mod reading_jsonc {
    use super::*;

    #[test]
    fn takes_out_comments_and_trailing_commas_outside_strings_and_errs_where_json_parse_would() {
        for case in text()["jsonc"].as_array().unwrap() {
            let t = s(&case["text"]);
            match (&case["out"].get("ok"), read_jsonc(t)) {
                (Some(want), Ok(got)) => same(&format!("{t:?}"), got, want),
                (None, Err(_)) => {}
                (want, got) => panic!("{t:?}: the TS gave {want:?}, the port {got:?}"),
            }
        }
    }

    #[test]
    fn a_key_named_twice_keeps_its_first_place_and_its_last_value() {
        let v = read_jsonc("{\"a\":1,\"b\":2,\"a\":3}").unwrap();
        assert_eq!(js_stringify(&v), "{\"a\":3,\"b\":2}");
    }
}

mod writing_json_as_javascript {
    use super::*;

    #[test]
    fn writes_numbers_as_javascript_writes_them_compact_and_pretty() {
        let n = text()["numbers"].clone();
        let v = read_jsonc(s(&n["text"])).unwrap();
        assert_eq!(js_stringify(&v), s(&n["compact"]));
        assert_eq!(js_stringify_pretty(&v), s(&n["pretty"]));
    }

    #[test]
    fn escapes_strings_as_javascript_escapes_them() {
        for case in text()["strings"].as_array().unwrap() {
            assert_eq!(js_stringify(&case["value"]), s(&case["out"]), "{}", case["value"]);
        }
    }

    #[test]
    fn lays_out_empty_and_nested_values_as_json_stringify_with_two_spaces() {
        let v = json!({ "a": {}, "b": [], "c": [{}], "d": { "e": [1, { "f": [] }] } });
        assert_eq!(js_stringify_pretty(&v), s(&text()["pretty"]));
    }
}

mod editing_json_in_place {
    use super::*;

    #[test]
    fn every_edit_lands_as_the_ts_editor_lands_it_or_is_refused_with_its_sentence() {
        for case in text()["edits"].as_array().unwrap() {
            let t = s(&case["text"]);
            let got = match edit_json(t, &edits_of(&case["edits"])) {
                Ok(out) => json!({ "ok": out }),
                Err(e) => json!({ "error": e.message }),
            };
            same(&format!("{t:?} with {}", case["edits"]), got, &case["out"]);
        }
    }

    #[test]
    fn an_entrys_canonical_text_sorts_keys_by_utf16_at_every_depth() {
        for case in text()["canonical"].as_array().unwrap() {
            let file = js_stringify(&json!({ "mcpServers": { "s": case["value"] } }));
            same(&format!("{}", case["value"]), opt(MCP_SERVERS_JSON.entry_of(&file, "s", None)), &case["out"]);
        }
    }
}

mod bearer_names {
    use super::*;

    #[test]
    fn a_bearer_name_is_the_header_tokens_name_with_bearer_after_it() {
        let w = text()["bearer"].clone();
        assert_eq!(mcp_bearer_variable("WSP_MCP_X_AUTHORIZATION"), s(&w["variable"]));
        for (name, want) in w["of"].as_object().unwrap() {
            same(&format!("bearerOf {name:?}"), opt(bearer_of(name)), want);
        }
    }

    #[test]
    fn the_bearer_values_go_ahead_of_the_values_and_a_value_named_like_one_keeps_its_place() {
        let values: IndexMap<String, String> = IndexMap::from([
            ("A".into(), "1".into()),
            ("WSP_MCP_X_AUTHORIZATION".into(), "tok".into()),
            ("WSP_MCP_X_AUTHORIZATION_BEARER".into(), "own".into()),
            ("WSP_MCP_Y_AUTHORIZATION".into(), "y".into()),
        ]);
        let got = with_bearer_variables(&values);
        let want = text()["bearer"]["with"].clone();
        same("with", json(&got), &want);
        let order: Vec<&String> = got.keys().collect();
        let want_order: Vec<&String> = want.as_object().unwrap().keys().collect();
        assert_eq!(order, want_order);
    }
}

#[test]
fn the_travelled_copy_gives_the_scopes_servers_or_its_refusal() {
    for case in text()["travelled"].as_array().unwrap() {
        let project: Option<FromTo> = case["project"].as_object().map(|p| FromTo { from: s(&p["from"]).into(), to: s(&p["to"]).into() });
        let got = travelled_servers(s(&case["text"]), s(&case["key"]), project.as_ref());
        match (&case["out"].get("ok"), got) {
            (Some(want), Ok(v)) => same(s(&case["text"]), json(v), want),
            (None, Err(e)) => {
                let said = s(&case["out"]["error"]);
                if said == "the copy that travelled is not a JSON object" {
                    assert_eq!(e.message, said);
                }
            }
            (want, got) => panic!("{}: the TS gave {want:?}, the port {got:?}", case["text"]),
        }
    }
}

mod toml_lines {
    use super::*;

    #[test]
    fn a_comment_ends_a_line_only_outside_quotes() {
        for case in text()["toml"]["uncomment"].as_array().unwrap() {
            assert_eq!(uncomment_toml(s(&case["line"])), s(&case["out"]), "{}", case["line"]);
        }
    }

    #[test]
    fn every_string_on_a_line_is_read_basic_ones_decoded() {
        for case in text()["toml"]["strings"].as_array().unwrap() {
            same(s(&case["text"]), json(toml_strings(s(&case["text"])).unwrap()), &case["out"]);
        }
        for case in text()["toml"]["openArray"].as_array().unwrap() {
            same(s(&case["value"]), json(toml_open_array(s(&case["value"]))), &case["out"]);
        }
    }

    #[test]
    fn a_rewrite_touches_only_the_strings_it_changes_and_leaves_the_comment() {
        for case in text()["toml"]["rewrite"].as_array().unwrap() {
            let (from, to) = (s(&case["from"]).to_string(), s(&case["to"]).to_string());
            let out = rewrite_toml_line(s(&case["line"]), &|v: &str| if v == from { to.clone() } else { v.to_string() }).unwrap();
            assert_eq!(out, s(&case["out"]), "{}", case["line"]);
        }
    }
}

#[test]
fn the_checksum_is_what_posix_cksum_prints() {
    for case in text()["configSum"].as_array().unwrap() {
        assert_eq!(config_sum(s(&case["text"]).as_bytes()), s(&case["out"]), "{:?}", case["text"]);
    }
}

mod codex_app_server {
    use super::*;

    #[test]
    fn requests_and_the_script_are_the_ts_text() {
        let w = text()["appServer"].clone();
        assert_eq!(request(7, "plugin/read", json!({ "pluginName": "p", "marketplacePath": "/m" })), s(&w["request"]));
        assert_eq!(request(2, "plugin/list", json!({})), s(&w["requestNoParams"]));
        assert_eq!(notification("initialized"), s(&w["notification"]));
        assert_eq!(initialize_request(1), s(&w["initialize"]));
        let stages = [
            ServerStage { lines: vec![initialize_request(1), request(2, "x", json!({}))], answers: 2 },
            ServerStage { lines: vec!["it's".into()], answers: 1 },
        ];
        assert_eq!(app_server_script("codex", &stages), s(&w["script"]));
        assert_eq!(app_server_script("/opt/codex", &[ServerStage { lines: vec![], answers: 0 }]), s(&w["scriptOne"]));
    }

    #[test]
    fn answers_are_read_by_numeric_id_the_last_of_an_id_winning_and_noise_skipped() {
        let lines = [
            "{\"id\":1,\"result\":{\"codexHome\":\"/h\"}}",
            "noise",
            "  {\"id\":2,\"error\":{\"code\":-32601,\"message\":\"unknown variant `plugin/list`\",\"data\":{\"x\":1}}}",
            "{\"id\":3,\"error\":{\"message\":5}}",
            "{\"id\":\"4\",\"result\":{}}",
            "{\"id\":5,\"result\":[1]}",
            "{\"id\":1,\"result\":{\"second\":true}}",
            "{broken",
            "{\"method\":\"note\"}",
        ]
        .join("\n");
        let got = answers_of(&lines);
        let as_json: serde_json::Map<String, Value> = got
            .iter()
            .map(|(id, a)| {
                let v = match a {
                    ServerAnswer::Result(r) => json!({ "result": r }),
                    ServerAnswer::Error { code, message, data } => {
                        let mut e = json!({ "message": message });
                        if let Some(c) = code {
                            e["code"] = json!(c);
                        }
                        if let Some(d) = data {
                            e["data"] = json!(d);
                        }
                        json!({ "error": e })
                    }
                };
                (id.to_string(), v)
            })
            .collect();
        same("answers", Value::Object(as_json), &text()["appServer"]["answers"]);
    }
}

#[test]
fn the_harness_checks_read_each_output_as_the_ts_modules_do() {
    let w = text();
    for case in w["check"].as_array().unwrap() {
        let o = s(&case["output"]);
        same(&format!("claude {o:?}"), check_word(McpCheck::Claude.auth(o, "x")), &case["claude"]);
        for (name, key) in [("x", "codexX"), ("y", "codexY"), ("z", "codexZ"), ("w", "codexW")] {
            same(&format!("codex {name} {o:?}"), check_word(McpCheck::Codex.auth(o, name)), &case[key]);
        }
    }
    assert_eq!(McpCheck::Claude.line("it's"), s(&w["checkLines"]["claude"]));
    assert_eq!(McpCheck::Codex.line("x"), s(&w["checkLines"]["codex"]));
}

#[test]
fn the_oauth_store_hashes_type_url_and_headers_in_that_order_and_keys_the_first_16_hex() {
    let w = text()["oauth"].clone();
    let calls = RefCell::new(Vec::new());
    let sha = |t: &str| {
        calls.borrow_mut().push(t.to_string());
        "58a5cc6249f547e0".to_string() + &"0".repeat(48)
    };
    let headers: IndexMap<String, String> = IndexMap::from([("b".into(), "1".into()), ("a".into(), "2".into())]);
    let got = McpOAuthStore::Claude.bearer("linear", "https://mcp.linear.app/mcp", &headers, "/root/.claude-cfg", &sha);
    same("bearer", json(got), &w["bearer"]);
    same("calls", json(calls.into_inner()), &w["calls"]);
}

mod hook_words {
    use super::*;

    fn placer(abs: &str) -> Option<String> {
        if abs.contains("nope") {
            None
        } else {
            Some(format!("/guest{abs}"))
        }
    }

    #[test]
    fn each_word_reads_as_a_path_a_machine_command_or_an_interpreter_as_the_ts_reads_it() {
        for case in text()["hooks"]["words"].as_array().unwrap() {
            let w = s(&case["word"]);
            same(&format!("pathOf {w}"), opt(path_of(w, "/Users/me")), &case["pathOf"]);
            same(&format!("onMachine {w}"), json(on_machine(w)), &case["onMachine"]);
            same(&format!("runsAFile {w}"), json(runs_a_file(w, "/Users/me")), &case["runsAFile"]);
            let placed = match place_word(w, "/Users/me", &placer) {
                PlacedWord::Left(l) => json!({ "left": l }),
                PlacedWord::Word { word, carried } => {
                    let mut v = json!({ "word": word });
                    if let Some(c) = carried {
                        v["carried"] = json!({ "from": c.from, "to": c.to });
                    }
                    v
                }
            };
            same(&format!("placeWord {w}"), placed, &case["placed"]);
        }
    }

    #[test]
    fn claude_and_codex_carry_their_hooks_as_the_ts_modules_do() {
        let w = text()["hooks"].clone();
        let settings = js_stringify_pretty(
            &json!({ "hooks": { "PreToolUse": [{ "matcher": "Bash", "hooks": [{ "type": "command", "command": "~/hooks/a.sh --flag" }, { "type": "command", "command": "bash ~/hooks/b.sh && /opt/nope/c" }] }], "Stop": [{ "hooks": [{ "type": "prompt", "prompt": "x" }] }] } }),
        );
        let carried = |c: wsp_catalog::hooks::CarriedHooks| json!({ "text": c.text, "carried": c.carried.iter().map(|x| json!({ "from": x.from, "to": x.to })).collect::<Vec<_>>(), "left": c.left });
        same("claude", json!({ "ok": carried(HookCarry::Claude.carry(&settings, "/Users/me", &placer).unwrap()) }), &w["claude"]);
        same("claudeFiles", json!({ "ok": hook_files(HookCarry::Claude, &settings, "/Users/me").unwrap() }), &w["claudeFiles"]);
        let codex = "model = \"x\"\n# runs after a turn\nnotify = [\"python3\", \"~/bin/notify.py\"]\n[tui]\nnotify = [\"/opt/nope\"]\n";
        same("codex", carried(HookCarry::Codex.carry(codex, "/Users/me", &placer).unwrap()), &w["codex"]);
        let left = "# doc for notify\n# more\nnotify = [\n  \"/opt/tool/bin/x\",\n  \"a\"\n]\nk = 1\n";
        same("codexLeft", carried(HookCarry::Codex.carry(left, "/Users/me", &placer).unwrap()), &w["codexLeft"]);
        same("codexOpen", carried(HookCarry::Codex.carry("notify = [\"~/x\",\n", "/Users/me", &placer).unwrap()), &w["codexOpen"]);
    }
}

#[test]
fn the_plugin_path_helpers_answer_as_the_ts_helpers() {
    let w = text()["plugins"].clone();
    same(
        "plainValues",
        json(plain_values("{\"installPath\": \"/a\", \"x\": {\"installPath\":\"/b\\\\c\"}, \"installPath\" : \"/d\"}", "installPath")),
        &w["plainValues"],
    );
    for (id, want) in w["idParts"].as_object().unwrap() {
        let (name, marketplace) = plugin_id_parts(id);
        same(&format!("idParts {id:?}"), json!({ "name": name, "marketplace": marketplace }), want);
    }
    for case in w["inStore"].as_array().unwrap() {
        let got = in_store(".claude", s(&case["path"]), "/h", case["store"].as_str());
        same(&format!("inStore {}", case["path"]), json(got), &case["out"]);
    }
    for case in w["joinPath"].as_array().unwrap() {
        let parts: Vec<&str> = case["parts"].as_array().unwrap().iter().map(s).collect();
        same(&format!("joinPath {parts:?}"), json(join_path(&parts)), &case["out"]);
    }
    for (p, want) in w["baseName"].as_object().unwrap() {
        same(&format!("baseName {p:?}"), json(base_name(p)), want);
    }
    same("bundled", json(BundledSkills::Hermes.names("a:hash\n b \n\n:x\nc:d:e\n")), &w["bundled"]);
}

#[test]
fn a_server_needs_its_own_credential_exactly_where_the_ts_says_so() {
    let w = text();
    let signs_in = |url: &str| {
        if url.contains("signs") {
            Some(true)
        } else if url.contains("open") {
            Some(false)
        } else {
            None
        }
    };
    for case in w["credentials"].as_array().unwrap() {
        let server: wsp_catalog::mcp::McpServer = serde_json::from_value(case["server"].clone()).unwrap();
        same(&format!("{}", case["server"]), json(wsp_catalog::mcp_login::needs_own_credential(&server, &signs_in)), &case["needs"]);
    }
    let mut servers: Vec<wsp_catalog::mcp::McpServer> =
        w["credentials"].as_array().unwrap().iter().map(|c| serde_json::from_value(c["server"].clone()).unwrap()).collect();
    servers.push(serde_json::from_value(json!({ "name": "s", "scope": "user", "transport": { "kind": "http", "url": "https://signs.example/mcp", "headers": {} }, "envRefs": ["ACME_PAT"] })).unwrap());
    same("addressesToAsk", json(wsp_catalog::mcp_login::addresses_to_ask(&servers)), &w["addressesToAsk"]);
}
