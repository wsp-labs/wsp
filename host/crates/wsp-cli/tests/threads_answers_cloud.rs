// SPDX-License-Identifier: AGPL-3.0-only
//! The cases of the tool server's record (daemon/crates/wsp-mcp/tests/answers) recorded with the cloud on, replayed
//! against the thread verbs' tools. A binary of its own: the cloud is read once per process, and every test here
//! turns it on before anything else runs.
#![allow(
    clippy::result_large_err,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::large_enum_variant,
    clippy::regex_creation_in_loops,
    non_snake_case,
    dead_code
)]

#[path = "cli_a_common/mod.rs"]
mod common;

use std::fmt::Display;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Once;
use std::time::Duration;

use serde_json::{json, Value};

use common::{answers, deps_on, in_order, server_line, Case, Script, ScriptedHost};
use wsp_cli::verbs::client::{reads_here, tool_failure, CallToolResult, TextContent, Tool, Verb};
use wsp_cli::verbs::threads::thread_verbs;

/// The variables a guest's tools read: the host's address and token, and the turn's token.
const GUEST_ENV: [&str; 3] = ["WSP_HOST_URL", "WSP_HOST_TOKEN", "WSP_TURN"];

static PREPARED: Once = Once::new();

/// The record's clocks were written in UTC, and its cloud cases were served with the cloud on.
fn prepare() {
    PREPARED.call_once(|| {
        std::env::set_var("WSP_CLOUD", "1");
        std::env::set_var("TZ", "UTC");
    });
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
    panic
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned()))
        .unwrap_or_else(|| "?".to_owned())
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
    format!(
        "first differ at character {at} (got {} characters, want {})\n    got  ...{}\n    want ...{}",
        got.chars().count(),
        want.chars().count(),
        around(got),
        around(want)
    )
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
    let mut env: Vec<(&str, &str)> =
        case.env.iter().filter(|(k, _)| !guest || GUEST_ENV.contains(&k.as_str())).map(|(k, v)| (k.as_str(), v.as_str())).collect();
    if guest {
        env.push(("WSP_HOST_TOKEN", "guest-token"));
    }
    let mut deps = deps_on(host.clone(), &env);
    deps.elsewhere = guest;

    if guest {
        match catch_unwind(AssertUnwindSafe(|| reads_here(verb).is_some())) {
            Err(p) => return m.miss(&name, format!("reads_here panicked: {}", panic_text(&*p))),
            Ok(true) => {
                // The server skips a tool that reads this computer's own files, so the call is never made.
                if not_served(tool) != line {
                    m.miss(
                        &name,
                        format!("a tool that reads here is not served to a guest, and the record's line is not that answer:\n    {line}"),
                    );
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
    for case in recorded.cases.iter().filter(|c| c.cloud == cloud && c.platform.as_deref().is_none_or(|p| p == this_platform())) {
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
                    replay(stringify!($name), true).await;
                }
            )*
        }
    };
}

tools!(run, export);
