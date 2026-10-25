// SPDX-License-Identifier: AGPL-3.0-only
//! The servers a turn gets in a folder, and the ones a project carries, against what the TS answered over the same
//! computer (tests/fixtures/oracle/turn-servers.json, written by oracle/rest.oracle.ts). No TS test in collect covers
//! these two files; packages/host/test/project-servers.test.ts reaches them through the host. spec/detect_turn_servers.md.
mod common;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_collect::detect::mcp_project::{detect_project_mcp, outside_project_line, ProjectMcpOptions, SWITCHED_OFF_HERE};
use wsp_collect::detect::presence::{presence_of, Presence};
use wsp_collect::detect::turn_servers::{read_checkout, read_turn_servers, TurnRead};

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/oracle/turn-servers.json")).unwrap()
}

fn computer() -> FakeHost {
    let o = oracle();
    let mut laptop = FakeLaptop::new();
    for (path, text) in o["files"].as_object().unwrap() {
        laptop = laptop.file(path, text.as_str().unwrap());
    }
    for (key, out) in o["exec"].as_object().unwrap() {
        laptop = laptop.exec(key, out.as_str().unwrap());
    }
    fake_host(laptop)
}

fn agent(id: &str) -> &'static AgentEntry {
    catalog_agents().into_iter().find(|a| a.id == id).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

const P: &str = "/Users/dev/proj";

#[test]
fn the_checkout_is_what_git_names_for_the_folder_or_the_folder_alone_outside_git() {
    let o = oracle();
    let c = block_on(read_checkout(&computer(), P));
    assert_eq!((c.top.as_deref(), c.key.as_str()), (o["checkout"]["top"].as_str(), o["checkout"]["key"].as_str().unwrap()));
    let outside = fake_host(FakeLaptop::new().exec(o["rev"].as_str().unwrap(), "fatal: not a git repository\n"));
    let c = block_on(read_checkout(&outside, P));
    assert_eq!((c.top, c.key), (None, P.to_string()));
    assert_eq!(outside.calls(), vec![format!("run {}", o["rev"].as_str().unwrap())]);
}

#[test]
fn a_turns_servers_are_read_off_the_agents_own_file_and_every_project_file_in_scope_order() {
    let o = oracle();
    let host = computer();
    let servers = block_on(read_turn_servers(&host, agent("claude"), P, TurnRead::default()));
    assert_eq!(serde_json::to_value(&servers).unwrap(), o["turn"]["servers"]);
    // git once for the checkout, the own file, then each project file whether it is there or not.
    assert_eq!(host.calls(), strings(&o["turn"]["calls"]));
}

#[test]
fn own_reads_the_agents_own_file_alone_and_asks_git_nothing() {
    let o = oracle();
    let host = computer();
    let servers = block_on(read_turn_servers(&host, agent("claude"), HOME, TurnRead { own: true, ..Default::default() }));
    assert_eq!(serde_json::to_value(&servers).unwrap(), o["own"]["servers"]);
    assert_eq!(host.calls(), strings(&o["own"]["calls"]));
}

#[test]
fn a_checkout_handed_in_is_used_and_git_is_not_asked() {
    let host = computer();
    let checkout = wsp_catalog::mcp_launch::Checkout { top: Some(P.to_string()), key: P.to_string() };
    block_on(read_turn_servers(&host, agent("claude"), P, TurnRead { checkout: Some(checkout), own: false }));
    assert!(host.calls().iter().all(|c| !c.starts_with("run ")), "{:?}", host.calls());
}

#[test]
fn a_project_carries_the_servers_its_checkout_does_not_bring_each_with_its_row_and_why_it_stays() {
    let o = oracle();
    let host = computer();
    let out = block_on(detect_project_mcp(&host, P, &ProjectMcpOptions { agents: vec![agent("claude"), agent("codex")], keep: vec![] }));
    let want = o["project"]["result"]["servers"].as_array().unwrap();
    assert_eq!(out.servers.len(), want.len());
    for (got, want) in out.servers.iter().zip(want) {
        assert_eq!(got.agent.id, want["agent"]["id"].as_str().unwrap());
        assert_eq!(serde_json::to_value(&got.turn).unwrap(), want["turn"]);
        // The row's keys are compared as a map: the TS writes them in the order mcpServerRow set them.
        assert_eq!(serde_json::to_value(&got.row).unwrap(), want["row"], "{}", want["turn"]["server"]["name"]);
    }
    // The tracked .mcp.json server travels with the checkout, so it is not one of them.
    assert!(out.servers.iter().all(|s| s.turn.server.name != "tracked"));
    assert_eq!(out.read, strings(&o["project"]["result"]["read"]));
    assert_eq!((out.checkout.top.as_deref(), out.checkout.key.as_str()), (Some(P), P));
    assert_eq!(host.calls(), strings(&o["project"]["calls"]));
}

#[test]
fn the_outside_line_names_every_path_and_the_switched_off_reason_is_fixed() {
    assert_eq!(outside_project_line(&["~/bin/mine".to_string(), "~/data/x.db".to_string()]), oracle()["outsideLine"].as_str().unwrap());
    assert_eq!(SWITCHED_OFF_HERE, "switched off for this folder here");
}

#[test]
fn presence_is_the_config_paths_that_exist_and_whether_the_command_is_on_path() {
    let host = fake_host(FakeLaptop::new().file("~/.b", 5).file("~/.c/x", 7).which(&["tool"]));
    let paths = |p: &[&str]| p.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    block_on(async {
        assert_eq!(
            presence_of(&host, &paths(&["~/.a", "~/.b", "~/.c"]), "none").await,
            Some(Presence { paths: paths(&["~/.b", "~/.c"]), bytes: 12, bin: false })
        );
        assert_eq!(presence_of(&host, &paths(&["~/.a"]), "tool").await, Some(Presence { paths: vec![], bytes: 0, bin: true }));
        assert_eq!(presence_of(&host, &paths(&["~/.a"]), "none").await, None);
        assert_eq!(presence_of(&host, &[], "none").await, None);
    });
    let _ = json!(null);
}
