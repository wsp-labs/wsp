// SPDX-License-Identifier: AGPL-3.0-only
//! The thread verbs' tools against the tool server's record (daemon/crates/wsp-mcp/tests/answers), replayed as the TS
//! server answers them, and `thread_verbs()` against the table the oracle wrote. Cases recorded with the cloud on are
//! in threads_answers_cloud.rs, a binary of its own, since the cloud is read once per process.

mod common;

use std::fmt::Display;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Once;
use std::time::Duration;

use serde_json::{json, Value};

use common::{answers, deps_on, fixture, in_order, server_line, Case, Script, ScriptedHost};
use wsp_cli::cloud_text::cloud_text;
use wsp_cli::verbs::client::{reads_here, tool_failure, CallToolResult, FlagRow, InputKind, Page, TextContent, Tool, ToolInput, Verb};
use wsp_cli::verbs::threads::thread_verbs;

/// The variables a guest's tools read: the host's address and token, and the turn's token.
const GUEST_ENV: [&str; 3] = ["WSP_HOST_URL", "WSP_HOST_TOKEN", "WSP_TURN"];

static PREPARED: Once = Once::new();

/// The record's clocks were written in UTC; a read prints the time of day in this process's zone.
fn prepare() {
    PREPARED.call_once(|| std::env::set_var("TZ", "UTC"));
}

fn this_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

#[derive(Default)]
struct Misses(Vec<String>);

impl Misses {
    fn case(&mut self, name: impl Display, f: impl FnOnce() -> Result<(), String>) {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(Ok(())) => {}
            Ok(Err(why)) => self.0.push(format!("{name}: {why}")),
            Err(panic) => self.0.push(format!("{name}: panicked: {}", panic_text(&*panic))),
        }
    }

    fn miss(&mut self, name: impl Display, why: impl Display) {
        self.0.push(format!("{name}: {why}"));
    }

    fn done(self) {
        assert!(self.0.is_empty(), "{} case(s) wrong:\n{}", self.0.len(), self.0.join("\n"));
    }
}

fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_else(|| "?".to_owned())
}

fn same(got: &Value, want: &Value) -> Result<(), String> {
    if got == want {
        Ok(())
    } else {
        Err(format!("got {got}\n    want {want}"))
    }
}

/// Where two long texts part, with what is around it.
fn parted(got: &str, want: &str) -> String {
    let at = got.chars().zip(want.chars()).position(|(a, b)| a != b).unwrap_or_else(|| got.chars().count().min(want.chars().count()));
    let around = |s: &str| -> String { s.chars().skip(at.saturating_sub(60)).take(180).collect() };
    format!("first differ at character {at} (got {} characters, want {})\n    got  ...{}\n    want ...{}", got.chars().count(), want.chars().count(), around(got), around(want))
}

// ---------------------------------------------------------------------------------------------------------------
// The record, replayed.
// ---------------------------------------------------------------------------------------------------------------

/// The verb a tool is, its tool, and the usage the TS server hands `toolFailure` (a `Cli` verb's own).
fn verb_of(tool: &str) -> (Verb, Tool, Option<String>) {
    for v in thread_verbs() {
        let (name, t, usage) = match &v {
            Verb::Cli(c) => (c.name, c.tool.clone(), Some(c.usage.clone())),
            Verb::ToolOnly(t) => (t.name, t.tool.clone(), None),
            Verb::CommandTool(t) => (t.name, t.tool.clone(), None),
            Verb::CliOnly(_) => continue,
        };
        if name.replace(' ', "_") == tool {
            return (v.clone(), t, usage);
        }
    }
    panic!("thread_verbs() has no tool called {tool}")
}

/// What the TS server prints for a tool it does not serve to a session inside a machine.
fn not_served(tool: &str) -> String {
    server_line(&CallToolResult {
        content: vec![TextContent { kind: "text".to_owned(), text: format!("MCP error -32602: Tool {tool} not found") }],
        structured_content: None,
        is_error: Some(true),
    })
}

async fn one(m: &mut Misses, tool: &str, verb: &Verb, t: &Tool, usage: &Option<String>, case: &Case, guest: bool) {
    let name = format!("{tool} / {}{}", case.case, if guest { " / as a guest" } else { "" });
    let (line, asked) = match (&case.guest, guest) {
        (Some(g), true) => (g.line.clone(), g.asked.clone()),
        _ => (case.line.clone(), case.asked.clone()),
    };
    let host = ScriptedHost::new(Script { replies: case.replies.clone(), pushed: case.pushed.clone(), closes: case.closes.clone() });
    let env: Vec<(&str, &str)> = case.env.iter().filter(|(k, _)| !guest || GUEST_ENV.contains(&k.as_str())).map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let mut deps = deps_on(host.clone(), &env);
    deps.elsewhere = guest;

    if guest {
        match catch_unwind(AssertUnwindSafe(|| reads_here(verb).is_some())) {
            Err(p) => return m.miss(&name, format!("reads_here panicked: {}", panic_text(&*p))),
            Ok(true) => {
                // The server skips a tool that reads this computer's own files, so the call is never made.
                if not_served(tool) != line {
                    m.miss(&name, format!("a tool that reads here is not served to a guest, and the record's line is not that answer:\n    {line}"));
                }
                if !asked.is_empty() {
                    m.miss(&name, "the record asks the host something of a tool that is not served");
                }
                return;
            }
            Ok(false) => {}
        }
    }

    let frame = case.arguments.as_object().expect("arguments are an object").clone();
    let call = t.call;
    let usage = usage.clone();
    let handle = tokio::spawn(async move {
        match call(frame, &deps).await {
            Ok(answer) => answer,
            Err(e) => tool_failure(&e, usage.as_deref()),
        }
    });
    let answer = match tokio::time::timeout(Duration::from_secs(30), handle).await {
        Err(_) => return m.miss(&name, "no answer in 30 seconds"),
        Ok(Err(joined)) => return m.miss(&name, format!("the call panicked: {joined}")),
        Ok(Ok(answer)) => answer,
    };
    let printed = server_line(&answer);
    if printed != line {
        m.miss(&name, format!("the printed line\n    {}", parted(&printed, &line)));
    }
    let (got, want) = (in_order(host.asked()), in_order(asked));
    if got != want {
        m.miss(&name, format!("what the host was asked\n    got  {}\n    want {}", json!(got), json!(want)));
    }
}

/// Every case of the tool recorded for this state of the cloud and this platform, then each guest variant.
async fn replay(tool: &str, cloud: bool) {
    prepare();
    let recorded = answers(tool);
    assert_eq!(recorded.tool, tool);
    let (verb, t, usage) = verb_of(tool);
    let mut m = Misses::default();
    let mut replayed = 0;
    for case in recorded.cases.iter().filter(|c| c.cloud == cloud && c.platform.as_deref().map_or(true, |p| p == this_platform())) {
        one(&mut m, tool, &verb, &t, &usage, case, false).await;
        replayed += 1;
        if case.guest.is_some() {
            one(&mut m, tool, &verb, &t, &usage, case, true).await;
            replayed += 1;
        }
    }
    assert!(replayed > 0, "no recorded case of {tool} with the cloud {}", if cloud { "on" } else { "off" });
    m.done();
}

macro_rules! tools {
    ($($name:ident),* $(,)?) => {
        mod tools {
            use super::*;
            $(
                #[tokio::test]
                async fn $name() {
                    replay(stringify!($name), false).await;
                }
            )*
        }
    };
}

tools!(run, send, exec, worktree, worktree_remove, thread_read, thread_head, thread_rename, thread_forget, thread_settle, thread_restore, thread_allow, thread_deny, restart, stop, folders, setup, terminal_config);

// ---------------------------------------------------------------------------------------------------------------
// The table, raw, against the oracle's.
// ---------------------------------------------------------------------------------------------------------------

fn kind_json(kind: &InputKind) -> Value {
    match kind {
        InputKind::String => json!({ "type": "string" }),
        InputKind::Boolean => json!({ "type": "boolean" }),
        InputKind::Number => json!({ "type": "number" }),
        InputKind::Integer { min } => json!({ "type": "integer", "min": min }),
        InputKind::StringList { min } => json!({ "type": "stringList", "min": min }),
        InputKind::Enum(options) => json!({ "type": "enum", "options": options }),
        InputKind::Literal(v) => json!({ "type": "literal", "value": v }),
        InputKind::Json => json!({ "type": "json" }),
    }
}

fn input_json(i: &ToolInput, cloud: bool) -> Value {
    json!({ "name": i.name, "kind": kind_json(&i.kind), "optional": i.optional, "describe": i.describe.as_deref().map(|d| cloud_text(d, cloud)) })
}

fn option_json(name: &str, row: &FlagRow) -> Value {
    json!({
        "name": name,
        "type": match row.kind { wsp_cli::verbs::client::FlagKind::String => "string", wsp_cli::verbs::client::FlagKind::Boolean => "boolean" },
        "multiple": row.multiple,
        "short": row.short.map(|c| c.to_string()),
        "valueWith": row.value_with,
    })
}

fn tool_json(t: &Tool, cloud: bool) -> Value {
    json!({
        "description": t.description,
        "input": t.input.iter().map(|i| input_json(i, cloud)).collect::<Vec<_>>(),
        "output": t.output,
        "stream": t.stream,
    })
}

fn page_word(p: Page) -> &'static str {
    match p {
        Page::Front => "front",
        Page::Agent => "agent",
        Page::Host => "host",
        Page::Dev => "dev",
        Page::App => "app",
    }
}

/// An entry in the oracle's shape, its describes cut for the cloud given and every other string as written.
fn entry_json(v: &Verb, cloud: bool) -> Value {
    let (shape, name, line, tool, reads, cli_only, host_side, tool_only) = match v {
        Verb::Cli(c) => ("cli", c.name, Some((c.usage.as_str(), c.about.as_str(), c.page, &c.options, c.starts_no_host, c.any_release, c.cloud, &c.cloud_flags)), Some(&c.tool), c.reads_here, None, None, None),
        Verb::CliOnly(c) => ("cliOnly", c.name, Some((c.usage.as_str(), c.about.as_str(), c.page, &c.options, c.starts_no_host, c.any_release, c.cloud, &c.cloud_flags)), None, c.reads_here, Some(c.cli_only), c.host_side, None),
        Verb::ToolOnly(t) => ("toolOnly", t.name, None, Some(&t.tool), t.reads_here, None, None, Some(t.tool_only)),
        Verb::CommandTool(t) => ("commandTool", t.name, None, Some(&t.tool), None, None, None, None),
    };
    json!({
        "name": name,
        "shape": shape,
        "usage": line.map(|l| l.0),
        "about": line.map(|l| l.1),
        "page": line.map(|l| page_word(l.2)),
        "options": line.map(|l| l.3 .0.iter().map(|(n, r)| option_json(n, r)).collect::<Vec<_>>()).unwrap_or_default(),
        "readsHere": reads,
        "startsNoHost": line.and_then(|l| l.4),
        "anyRelease": line.and_then(|l| l.5),
        "cloud": line.map_or(false, |l| l.6),
        "cloudFlags": line.map(|l| l.7.clone()).unwrap_or_default(),
        "cliOnly": cli_only,
        "hostSide": host_side,
        "toolOnly": tool_only,
        "tool": tool.map(|t| tool_json(t, cloud)),
    })
}

fn entry_misses(mine: &Value, want: &Value) -> Result<(), String> {
    let mut wrong = Vec::new();
    for (key, w) in want.as_object().unwrap() {
        let g = &mine[key.as_str()];
        if key == "tool" && w.is_object() && g.is_object() {
            for (k, wt) in w.as_object().unwrap() {
                let gt = &g[k.as_str()];
                if k == "input" {
                    let (gi, wi) = (gt.as_array().unwrap(), wt.as_array().unwrap());
                    if gi.len() != wi.len() {
                        wrong.push(format!("tool.input has {} fields, want {}: {:?} against {:?}", gi.len(), wi.len(), gi.iter().map(|i| i["name"].clone()).collect::<Vec<_>>(), wi.iter().map(|i| i["name"].clone()).collect::<Vec<_>>()));
                    }
                    for (a, b) in gi.iter().zip(wi) {
                        if a != b {
                            wrong.push(format!("tool.input {}\n      got  {a}\n      want {b}", b["name"]));
                        }
                    }
                } else if gt != wt {
                    wrong.push(format!("tool.{k}\n      got  {gt}\n      want {wt}"));
                }
            }
        } else if g != w {
            wrong.push(format!("{key}\n      got  {g}\n      want {w}"));
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(format!("\n  {}", wrong.join("\n  ")))
    }
}

#[test]
fn thread_verbs_are_the_table_the_oracle_wrote() {
    let verbs = thread_verbs();
    let mut m = Misses::default();
    for cloud in [false, true] {
        let fx = fixture("thread_verbs", cloud);
        let want = fx.as_array().unwrap();
        m.case(format!("(cloud {cloud}) how many entries"), || same(&json!(verbs.len()), &json!(want.len())));
        for (i, w) in want.iter().enumerate() {
            m.case(format!("{} (cloud {cloud})", w["name"].as_str().unwrap()), || {
                let verb = verbs.get(i).ok_or("no entry at this place in thread_verbs()")?;
                entry_misses(&entry_json(verb, cloud), w)
            });
        }
    }
    m.done();
}
