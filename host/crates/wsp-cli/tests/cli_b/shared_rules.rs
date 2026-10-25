// SPDX-License-Identifier: AGPL-3.0-only
// Rules over every entry of cli-b's eight files at once: the table's order and kinds, the tool text and inputs
// against the TS server's record (daemon/crates/wsp-mcp/record/tools), and host invariant 7 (packages/host/AGENTS.md:
// with the cloud off no page, tool or skill line names a cloud).

use serde_json::Value;
use wsp_cli::verbs::activity::activity_verbs;
use wsp_cli::verbs::agents::agent_verbs;
use wsp_cli::verbs::client::{has_tool, Verb};
use wsp_cli::verbs::computers::computer_verbs;
use wsp_cli::verbs::conversations::conversation_verbs;
use wsp_cli::verbs::projects::project_verbs;
use wsp_cli::verbs::slate::{slate_tool_names, slate_verbs};
use wsp_cli::verbs::verbs_for;
use wsp_cli::verbs::workspaces::workspace_verbs;

fn mine() -> Vec<(&'static str, Vec<Verb>)> {
    vec![
        ("workspaces", workspace_verbs()),
        ("agents", agent_verbs()),
        ("computers", computer_verbs()),
        ("projects", project_verbs()),
        ("slate", slate_verbs()),
        ("activity", activity_verbs()),
        ("conversations", conversation_verbs()),
    ]
}

/// Which variant each entry is, by name, as the TS tables declare them (CliVerb, a cliOnly, a toolOnly, `add`).
#[test]
fn every_entry_in_ts_order_and_of_its_kind() {
    let want: &[(&str, &[(&str, &str)])] = &[
        (
            "workspaces",
            &[
                ("rename", "cli"),
                ("fork", "cli"),
                ("commit", "cli"),
                ("discard", "cli"),
                ("fix", "cli"),
                ("merge in", "cli"),
                ("merge", "cli"),
                ("start", "cli"),
                ("review", "cli"),
                ("review post", "cli"),
                ("update", "cli"),
                ("pause", "cli"),
                ("resize", "cli"),
                ("wake", "cli"),
                ("rebuild", "cli"),
                ("image", "cli"),
                ("image build", "cli"),
                ("forget", "cli"),
                ("delete", "cli"),
            ],
        ),
        (
            "agents",
            &[
                ("agents", "cli"),
                ("skills", "cli"),
                ("skills search", "cli"),
                ("skills show", "cli"),
                ("skills add", "cli"),
                ("skills remove", "cli"),
                ("skills disable", "cli"),
                ("skills enable", "cli"),
                ("servers", "cli"),
                ("agents signin", "cli-only"),
                ("agents key", "cli-only"),
                ("agents addtools", "cli"),
                ("agents default", "cli"),
                ("agents set", "cli"),
                ("agents setup", "cli"),
                ("servers signin", "cli-only"),
                ("servers tools", "cli"),
                ("servers add", "cli"),
                ("servers remove", "cli"),
                ("servers disable", "cli"),
                ("servers enable", "cli"),
                ("plugins", "cli"),
                ("plugins disable", "cli"),
                ("plugins enable", "cli"),
            ],
        ),
        (
            "computers",
            &[
                ("computers", "cli"),
                ("computers set", "cli"),
                ("add", "command"),
                ("recipes", "cli"),
                ("recipes show", "cli"),
                ("recipes save", "cli"),
                ("recipes remove", "cli"),
                ("usage", "cli"),
                ("usage reset", "cli-only"),
            ],
        ),
        (
            "projects",
            &[
                ("projects", "cli"),
                ("projects add", "tool-only"),
                ("projects remove", "cli"),
                ("projects set", "cli"),
                ("projects seed", "cli"),
                ("threads", "cli"),
                ("threads wait", "cli"),
            ],
        ),
        ("slate", &[("slate catalog", "cli"), ("slate write", "cli"), ("slate state", "cli"), ("slate read", "cli")]),
        ("activity", &[("activity", "cli"), ("activity end", "cli")]),
        ("conversations", &[("conversations", "cli")]),
    ];
    for ((file, got), (wfile, want)) in mine().iter().zip(want) {
        assert_eq!(file, wfile);
        let got: Vec<(&str, &str)> = got
            .iter()
            .map(|v| {
                let kind = match v {
                    Verb::Cli(_) => "cli",
                    Verb::CliOnly(_) => "cli-only",
                    Verb::ToolOnly(_) => "tool-only",
                    Verb::CommandTool(_) => "command",
                };
                (v.name(), kind)
            })
            .collect();
        assert_eq!(got, *want, "{file}");
    }
}

#[test]
fn the_cloud_verbs_are_marked_and_delete_carries_its_cloud_input() {
    let cloud: Vec<&str> =
        mine().iter().flat_map(|(_, v)| v.iter().filter(|v| v.is_cloud()).map(|v| v.name()).collect::<Vec<_>>()).collect();
    assert_eq!(cloud, ["rename", "fork", "pause", "wake", "rebuild", "forget"]);
    let off: Vec<&str> = verbs_for(false).iter().map(|v| v.name()).collect();
    for name in ["rename", "fork", "pause", "wake", "rebuild", "forget"] {
        assert!(!off.contains(&name), "{name} is served with the cloud off");
    }
    let delete = verbs_for(false).into_iter().find(|v| v.name() == "delete").unwrap();
    let inputs: Vec<&str> = has_tool(&delete).unwrap().input.iter().map(|i| i.name).collect();
    assert_eq!(inputs, ["thread", "processes", "confirm"], "workspace leaves with the cloud off");
    let delete = verbs_for(true).into_iter().find(|v| v.name() == "delete").unwrap();
    let inputs: Vec<&str> = has_tool(&delete).unwrap().input.iter().map(|i| i.name).collect();
    assert_eq!(inputs, ["thread", "workspace", "processes", "confirm"]);
}

fn record(tool: &str) -> Value {
    let path = format!("{}/../../../daemon/crates/wsp-mcp/record/tools/{tool}.json", env!("CARGO_MANIFEST_DIR"));
    serde_json::from_str(&std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("{path}"))).unwrap()
}

/// Each tool's description, its inputs in order, which are required and each input's words, with the cloud off
/// and on, as the TS server lists them; the record is what the TS server wrote.
#[test]
fn tool_text_and_inputs_match_the_ts_server_record() {
    let names: Vec<&str> =
        mine().iter().flat_map(|(_, v)| v.iter().filter(|v| has_tool(v).is_some()).map(|v| v.name()).collect::<Vec<_>>()).collect();
    for cloud in [false, true] {
        let table = verbs_for(cloud);
        for name in &names {
            let tool = name.replace(' ', "_");
            let want = &record(&tool)[if cloud { "cloudOn" } else { "cloudOff" }];
            let entry = table.iter().find(|v| v.name() == *name);
            let Some(entry) = entry else {
                assert!(want.is_null(), "{tool}: served by the TS with cloud {cloud} but not here");
                continue;
            };
            assert!(!want.is_null(), "{tool}: served here with cloud {cloud} but not by the TS");
            let t = has_tool(entry).unwrap();
            assert_eq!(Value::from(t.description.clone()), want["description"], "{tool} cloud {cloud}: description");
            let props = want["inputSchema"]["properties"].as_object().cloned().unwrap_or_default();
            let got: Vec<&str> = t.input.iter().map(|i| i.name).collect();
            let keys: Vec<&str> = props.keys().map(String::as_str).collect();
            assert_eq!(got, keys, "{tool} cloud {cloud}: inputs in order");
            let required: Vec<Value> = want["inputSchema"]["required"].as_array().cloned().unwrap_or_default();
            for i in &t.input {
                assert_eq!(!i.optional, required.contains(&Value::from(i.name)), "{tool}.{}: required", i.name);
                assert_eq!(
                    i.describe.clone().map(Value::from).unwrap_or(Value::Null),
                    props[i.name].get("description").cloned().unwrap_or(Value::Null),
                    "{tool}.{}: words",
                    i.name
                );
            }
        }
    }
}

const CLOUD_ALLOWED: &[&str] =
    &["and on a cloud how many", "may run on that cloud at once", "that cloud may spend", "sub-thread's machine"];

/// Host invariant 7 (packages/host/AGENTS.md, test/cloud.test.ts:106-120): with the cloud off no page or tool of
/// these files names Solari, or says cloud but for the uses allowed there.
#[test]
fn invariant_7_cloud_off_names_no_cloud() {
    let table = verbs_for(false);
    let names: Vec<&str> = mine().iter().flat_map(|(_, v)| v.iter().map(|v| v.name()).collect::<Vec<_>>()).collect();
    let mut said = Vec::new();
    for v in table.iter().filter(|v| names.contains(&v.name())) {
        let mut texts = Vec::new();
        if let Some(line) = v.line() {
            texts.push(line.usage().to_string());
            texts.push(line.about().to_string());
        }
        if let Some(t) = has_tool(v) {
            texts.push(t.description.clone());
            texts.extend(t.input.iter().filter_map(|i| i.describe.clone()));
        }
        for text in texts {
            assert!(!text.contains("Solari"), "{}: {text}", v.name());
            assert!(!text.contains("<!--"), "{}: a cloud mark left in {text}", v.name());
            for sentence in text.split(['.', '\n']) {
                let words: Vec<String> = sentence.split(|c: char| !c.is_alphanumeric()).map(str::to_lowercase).collect();
                if words.iter().any(|w| w == "cloud" || w == "clouds") && !CLOUD_ALLOWED.iter().any(|a| sentence.contains(a)) {
                    said.push(format!("{}: {}", v.name(), sentence.trim()));
                }
            }
        }
    }
    assert_eq!(said, Vec::<String>::new());
}

#[test]
fn slate_tool_names_are_the_four_slate_tools() {
    assert_eq!(slate_tool_names(), ["slate_catalog", "slate_write", "slate_state", "slate_read"]);
}
