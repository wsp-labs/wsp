// SPDX-License-Identifier: AGPL-3.0-only
// The marks, the seed rows, the ecosystems, the git hosts and the machine context against tests/fixtures/marks.json,
// seed.json and context.json, which the TS catalog wrote (oracle/dump.oracle.ts).
mod oracle_common;

use oracle_common::{fixture, json, opt, s, same};
use serde_json::{json, Value};
use wsp_catalog::context::*;
use wsp_catalog::git_hosts::*;
use wsp_catalog::marks::*;
use wsp_catalog::pending::McpRowTransport;
use wsp_catalog::seed::*;
use wsp_catalog::skills::claude_plugin_skills;

#[test]
fn the_brand_provider_and_editor_marks_are_the_ts_marks_in_order() {
    let w = fixture("marks.json");
    same("brand", json(brand_marks()), &w["brand"]);
    same("provider", json(provider_marks()), &w["provider"]);
    same("editor", json(editor_marks()), &w["editor"]);
}

#[test]
fn a_servers_mark_is_the_one_the_ts_finds_by_host_package_or_address() {
    for case in fixture("marks.json")["serverMark"].as_array().unwrap() {
        let t: McpRowTransport = serde_json::from_value(case["transport"].clone()).unwrap();
        same(&format!("{}", case["transport"]), opt(server_mark(&t).map(|m| m.id.clone())), &case["mark"]);
    }
}

#[test]
fn a_command_words_package_is_the_word_without_its_version_or_tag() {
    assert_eq!(package_of_word("@scope/name@1.2"), "@scope/name");
    assert_eq!(package_of_word("@scope/name"), "@scope/name");
    assert_eq!(package_of_word("name@latest"), "name");
    assert_eq!(package_of_word("ghcr.io/github/github-mcp-server:latest"), "ghcr.io/github/github-mcp-server");
    assert_eq!(package_of_word("localhost:5000/img"), "localhost:5000/img");
    assert_eq!(package_of_word("@"), "@");
}

#[test]
fn the_seed_rows_are_the_ts_rows_and_judge_every_path_as_the_ts_does() {
    let w = fixture("seed.json");
    same("rows", json(&*SEED_ROWS), &w["rows"]);
    for case in w["judged"].as_array().unwrap() {
        let path = s(&case["path"]);
        let dir = case["dir"].as_bool().unwrap();
        let got = seed_row_for(path, dir).map(|(row, kind)| (row.id.clone(), kind));
        same(&format!("{path:?} dir {dir}"), json!({ "row": opt(got.as_ref().map(|g| g.0.clone())), "kind": opt(got.map(|g| g.1)) }), &json!({ "row": case["row"], "kind": case["kind"] }));
    }
}

#[test]
fn a_root_picks_one_install_per_ecosystem_the_first_listed_lockfile_winning() {
    for case in fixture("seed.json")["installs"].as_array().unwrap() {
        let names: Vec<String> = case["names"].as_array().unwrap().iter().map(|n| s(n).to_string()).collect();
        let got: Vec<Value> = seed_installs_for(&names).into_iter().map(|(row, install)| json!([row.id, install.lockfile])).collect();
        same(&format!("{names:?}"), json(got), &case["out"]);
    }
}

#[test]
fn the_ecosystems_and_the_git_hosts_are_the_ts_tables() {
    let w = fixture("seed.json");
    same("ecosystems", json(&*ECOSYSTEM_MODULES), &w["ecosystems"]);
    let hosts: Vec<Value> = git_hosts()
        .iter()
        .map(|h| json!({ "id": json(h.id), "cli": h.cli, "sshHosts": h.ssh_hosts, "setupGit": h.setup_git, "status": h.status, "httpsUrl": h.https_url("o/r"), "clone": h.clone_argv("o/r", "/d") }))
        .collect();
    same("gitHosts", json(hosts), &w["gitHosts"]);
    for (remote, want) in w["remoteHost"].as_object().unwrap() {
        same(&format!("remoteHost {remote:?}"), opt(remote_host(remote)), want);
    }
    for (remote, want) in w["gitHostOf"].as_object().unwrap() {
        same(&format!("gitHostOf {remote}"), opt(git_host_of(remote).map(|h| h.id)), want);
    }
    same("noGitCliLine", json(no_git_cli_line(git_host("gitlab").unwrap(), "the box")), &w["noGitCliLine"]);
    assert!(git_host("bitbucket").is_none());
}

#[test]
fn only_the_four_thread_agents_run_threads() {
    for id in ["claude", "codex", "opencode", "cursor"] {
        assert!(runs_threads(id), "{id}");
    }
    for id in ["gemini", "pi", "", "Claude"] {
        assert!(!runs_threads(id), "{id}");
    }
}

#[test]
fn every_agents_context_lands_the_ts_files_with_and_without_a_conflict() {
    let w = fixture("context.json");
    let roots = GuestRoots { etc: "/etc".into(), home: "/root".into() };
    let input = |conflict: bool, fish: bool| ContextHookInput { short: "SHORT\n".into(), skill: "SKILL\n".into(), source: "/etc/wsp/short.md".into(), roots: roots.clone(), conflict, fish };
    let odd = ContextHookInput { short: "a ''' b".into(), skill: "SKILL".into(), source: "/s".into(), roots: GuestRoots { etc: "/x/etc".into(), home: "/x/home".into() }, conflict: false, fish: true };
    for ctx in [AgentContext::Claude, AgentContext::Codex, AgentContext::Gemini, AgentContext::Opencode, AgentContext::Pi, AgentContext::Hermes] {
        let id = json(ctx);
        let want = &w["hooks"][s(&id)];
        same(&format!("{id} cd"), opt(ctx.cd()), &want["cd"]);
        same(&format!("{id} conflict"), opt(ctx.conflict(&roots)), &want["conflict"]);
        same(&format!("{id} plain"), json(ctx.hooks(&input(false, false))), &want["plain"]);
        same(&format!("{id} conflicted"), json(ctx.hooks(&input(true, true))), &want["conflicted"]);
        same(&format!("{id} fish"), json(ctx.hooks(&odd)), &want["fish"]);
    }
    same("pluginSkills", json(claude_plugin_skills()), &w["pluginSkills"]);
}

#[test]
fn a_document_holding_three_quotes_goes_into_toml_as_a_basic_string() {
    assert_eq!(toml_doc_string("plain\n"), "'''\nplain\n'''");
    assert_eq!(toml_doc_string("a ''' b"), "\"a ''' b\"");
    assert_eq!(toml_doc_string(""), "'''\n'''");
}
