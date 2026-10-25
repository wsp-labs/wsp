// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/slate.ts` at 65f6b89c7: the slate verbs, a live panel per thread the agent builds and
//! the person reads, presses and fills in. Spec: `spec/slate.md`.

use indexmap::IndexMap;
use serde_json::Value;
use wsp_words::exit::HostError;

use super::client::*;

/// What a slate file sends: the JSX-like text of a `.slate` file, or the stored document of a `.json` one.
#[derive(Clone, Debug, PartialEq)]
pub enum SlateFile {
    Text(String),
    Document(Frame),
}

/// `slateTarget` (slate.ts:19-26): `{threadId}` for a thread named by id or prefix (`threadOf`, its `threadId`
/// else its `id`), else `{turnToken}` from `WSP_TURN` when set and not empty, else nothing.
pub async fn slate_target(client: &dyn HostClient, named: Option<&str>, env: &IndexMap<String, String>) -> Result<Frame, HostError> {
    let _ = (client, named, env);
    todo!("slate.ts:19-26")
}

/// `slateAsk` (slate.ts:29-32): the op's reply with its own `id` and `ok` taken off, every other key in place.
pub async fn slate_ask(client: &dyn HostClient, op: &str, params: Frame) -> Result<Frame, HostError> {
    let _ = (client, op, params);
    todo!("slate.ts:29-32")
}

/// `emitSlate` (slate.ts:35-39): `{text}` as one frame printed as the text, then the rest with no line.
pub fn emit_slate(ctx: &VerbContext, answer: Frame) {
    let _ = (ctx, answer);
    todo!("slate.ts:35-39")
}

/// `slateFile` (slate.ts:42-62): the file a write names, read from the line's folder.
pub fn slate_file(ctx: &VerbContext, path: &str) -> Result<SlateFile, HostError> {
    let _ = (ctx, path);
    todo!("slate.ts:42-62")
}

/// `ifVersionOf` (slate.ts:64-70): `--if-version` as JavaScript's `Number` reads it, a whole number from 0, else
/// refused; absent is `None`. The value is sent as that number.
pub fn if_version_of(ctx: &VerbContext) -> Result<Option<f64>, HostError> {
    let _ = ctx;
    todo!("slate.ts:64-70")
}

/// `stateValue` (slate.ts:73-82): a `<path>=<json>` word, the value parsed as JSON else kept as its text; `None`
/// where the word has no `=` after its first character.
pub fn state_value(word: &str) -> Option<(String, Value)> {
    let _ = word;
    todo!("slate.ts:73-82")
}

/// `SLATE_TOOL_NAMES` (slate.ts:224): the four slate tools.
pub fn slate_tool_names() -> &'static [&'static str] {
    todo!("slate.ts:224")
}

/// The entries of `verbs/slate.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn slate_verbs() -> Vec<Verb> {
    vec![
    // verbs/slate.ts:85
    Verb::Cli(CliVerb {
        name: "slate catalog",
        usage: "wsp slate catalog [<name>]".to_string(),
        about: "what a slate can hold: the index of pieces, sources, steps, functions and rules, or one entry in full".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_slate_catalog,
        tool: Tool {
            description: "What a slate, the live panel beside the chat, can hold: every piece, and the runs that keep it fresh. Call it first whenever the person wants to see, watch, monitor or keep an eye on something, tick things off as they go, or see what's unread, even when another tool fetches the data.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: true, describe: Some("leave out first: the index of every piece; then an entry, like runs".to_string()) },
            ],
            output: vec!["text"],
            stream: vec![],
            call: call_slate_catalog,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/slate.ts:109
    Verb::Cli(CliVerb {
        name: "slate write",
        usage: "wsp slate write [<thread>] [<file>] [--check] [--set <path>=<json>]... [--press <piece>] [--row <n>] [--action <n>] [--if-version <n>]".to_string(),
        about: "writes the thread's slate from a .slate file (a whole <slate> or a patch) or a .json document and prints its sketch; --check stores nothing, and with --set or --press rehearses values and a press on a copy, the file optional".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("check", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("set", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("press", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("row", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("action", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("if-version", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_slate_write,
        tool: Tool {
            description: "Writes this thread's slate, the live panel here beside the chat. Use it to show the person what they want to see, watch, monitor or keep an eye on while you work (live data, traffic, metrics, a price, logs, a PR, progress, status), a dashboard, a form to fill in or a checklist. A run with every= refreshes on a timer with no turns, so nothing polls. A run's script goes in a <file name=\"x.py\">, run as python3 \"$SLATE_DIR/x.py\", never in the project.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("another thread's id".to_string()) },
                ToolInput { name: "text", kind: InputKind::String, optional: true, describe: Some("JSX-like text: a <slate>, or a patch".to_string()) },
                ToolInput { name: "document", kind: InputKind::Json, optional: true, describe: Some("JSON form from slate_read; not for writing".to_string()) },
                ToolInput { name: "check", kind: InputKind::Boolean, optional: true, describe: Some("validate, write nothing".to_string()) },
                ToolInput { name: "values", kind: InputKind::Json, optional: true, describe: Some("with check: $path: value".to_string()) },
                ToolInput { name: "press", kind: InputKind::String, optional: true, describe: Some("with check: piece id to press".to_string()) },
                ToolInput { name: "row", kind: InputKind::Integer { min: None }, optional: true, describe: Some("pressed row, from 0".to_string()) },
                ToolInput { name: "action", kind: InputKind::Integer { min: None }, optional: true, describe: Some("row action, from 0".to_string()) },
                ToolInput { name: "if_version", kind: InputKind::Integer { min: None }, optional: true, describe: Some("only at this version".to_string()) },
            ],
            output: vec!["version", "text", "warnings", "problems", "waiting"],
            stream: vec!["text"],
            call: call_slate_write,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/slate.ts:161
    Verb::Cli(CliVerb {
        name: "slate state",
        usage: "wsp slate state [<thread>] [<path>=<json>...] [--start <run>]... [--if-version <n>]".to_string(),
        about: "sets the slate's $values by path, like '$steps[2].done=true' or 'i=2', starts runs the person let run every time, and prints its sketch".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("if-version", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("start", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None })]),
        run: run_slate_state,
        tool: Tool {
            description: "Sets the slate's live $values by path, so the person sees progress, status or a checklist tick move as you work; reactions fire. start starts a run the person allowed to run always.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("another thread's id".to_string()) },
                ToolInput { name: "values", kind: InputKind::Json, optional: true, describe: Some("$path: new value".to_string()) },
                ToolInput { name: "start", kind: InputKind::StringList { min: None }, optional: true, describe: Some("runs allowed always".to_string()) },
                ToolInput { name: "if_version", kind: InputKind::Integer { min: None }, optional: true, describe: Some("only at this version".to_string()) },
            ],
            output: vec!["version", "text", "problems", "waiting", "notStarted"],
            stream: vec!["text"],
            call: call_slate_state,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/slate.ts:195
    Verb::Cli(CliVerb {
        name: "slate read",
        usage: "wsp slate read [<thread>] [--values <path>]... [--no-text] [--no-sketch] [--document]".to_string(),
        about: "the slate as it stands: the sketch, then the JSX-like form, the values, derived values, runs, problems and the paths named; --no-text leaves out the JSX-like form, --document adds the stored JSON".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("values", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("no-text", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("no-sketch", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("document", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_slate_read,
        tool: Tool {
            description: "Reads this thread's slate: what the person filled in or pressed, live values, run output and logs, and the sketch: the panel's words as the person sees them.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("another thread's id".to_string()) },
                ToolInput { name: "values", kind: InputKind::StringList { min: None }, optional: true, describe: Some("$run.json, $value, source path, or *".to_string()) },
                ToolInput { name: "text", kind: InputKind::Boolean, optional: true, describe: Some("false: no JSX-like form".to_string()) },
                ToolInput { name: "sketch", kind: InputKind::Boolean, optional: true, describe: Some("false: no sketch".to_string()) },
                ToolInput { name: "document", kind: InputKind::Boolean, optional: true, describe: Some("true: add stored JSON".to_string()) },
            ],
            output: vec!["version", "text", "document", "values", "derived", "runs", "state", "problems", "waiting", "comments", "approvals"],
            stream: vec!["text"],
            call: call_slate_read,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    ]
}

/// The `slate_catalog` tool's call (`verbs/slate.ts:101-105`).
fn call_slate_catalog(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/slate.ts:101-105")
    })
}

/// `wsp slate catalog` (`verbs/slate.ts:90-96`).
fn run_slate_catalog(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/slate.ts:90-96")
    })
}

/// The `slate_write` tool's call (`verbs/slate.ts:152-157`).
fn call_slate_write(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/slate.ts:152-157")
    })
}

/// `wsp slate write` (`verbs/slate.ts:114-136`).
fn run_slate_write(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/slate.ts:114-136")
    })
}

/// The `slate_state` tool's call (`verbs/slate.ts:187-191`).
fn call_slate_state(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/slate.ts:187-191")
    })
}

/// `wsp slate state` (`verbs/slate.ts:166-181`).
fn run_slate_state(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/slate.ts:166-181")
    })
}

/// The `slate_read` tool's call (`verbs/slate.ts:213-217`).
fn call_slate_read(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/slate.ts:213-217")
    })
}

/// `wsp slate read` (`verbs/slate.ts:200-207`).
fn run_slate_read(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/slate.ts:200-207")
    })
}
