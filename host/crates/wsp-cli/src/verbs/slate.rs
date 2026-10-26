// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/slate.ts` at 65f6b89c7: the slate verbs, a live panel per thread the agent builds and
//! the person reads, presses and fills in. Spec: `spec/slate.md`.

use crate::json_whole::{parse_document, parse_value};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_wire::types::js_number::JsNumber;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::to_number;

use super::client::*;
use super::io::{as_text, resolve_path};
use super::workspaces_help::thread_of;

/// What a slate file sends: the JSX-like text of a `.slate` file, or the stored document of a `.json` one.
#[derive(Clone, Debug, PartialEq)]
pub enum SlateFile {
    Text(String),
    Document(Frame),
}

/// `slateTarget` (slate.ts:19-26): `{threadId}` for a thread named by id or prefix (`threadOf`, its `threadId`
/// else its `id`), else `{turnToken}` from `WSP_TURN` when set and not empty, else nothing.
pub async fn slate_target(client: &dyn HostClient, named: Option<&str>, env: &IndexMap<String, String>) -> Result<Frame, HostError> {
    if let Some(reference) = named {
        let thread = thread_of(client, reference).await?;
        return Ok(pick(vec![("threadId", Some(Value::from(thread_id_of(&thread))))]));
    }
    let token = env.get(wsp_wire::types::env::TURN_TOKEN_ENV).filter(|token| !token.is_empty());
    Ok(pick(vec![("turnToken", token.map(|token| Value::from(token.as_str())))]))
}

/// `slateAsk` (slate.ts:29-32): the op's reply with its own `id` and `ok` taken off, every other key in place.
pub async fn slate_ask(client: &dyn HostClient, op: &str, params: Frame) -> Result<Frame, HostError> {
    let mut answer = client.request(op, params).await?;
    answer.shift_remove("id");
    answer.shift_remove("ok");
    Ok(answer)
}

/// `emitSlate` (slate.ts:35-39): `{text}` as one frame printed as the text, then the rest with no line.
pub fn emit_slate(ctx: &VerbContext, answer: Frame) {
    let mut rest = answer;
    let text = rest.shift_remove("text");
    let line = text.as_ref().and_then(Value::as_str).map(str::to_owned);
    let sketch = text.map_or_else(|| json!({}), |text| json!({ "text": text }));
    ctx.out.emit(&sketch, line.as_deref());
    ctx.out.emit(&Value::Object(rest), None);
}

/// `slateFile` (slate.ts:42-62): the file a write names, read from the line's folder.
pub fn slate_file(ctx: &VerbContext, path: &str) -> Result<SlateFile, HostError> {
    if ctx.elsewhere {
        let said = format!("{path} is a file on your machine, which this host cannot read.");
        return Err(usage_refusal(&said, "Pass the slate to slate_write as text instead."));
    }
    let cwd = match &ctx.cwd {
        Some(cwd) => cwd.clone(),
        None => std::env::current_dir().map(|dir| dir.to_string_lossy().into_owned()).unwrap_or_default(),
    };
    let at = resolve_path(&cwd, path);
    if !path.ends_with(".slate") && !path.ends_with(".json") {
        let said = format!("{path} is neither a .slate nor a .json file.");
        return Err(usage_refusal(&said, "Write the JSX-like form to a .slate file or the stored form to a .json file."));
    }
    let text = match std::fs::read(&at) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => {
            let said = format!("there is no file at {at}.");
            return Err(usage_refusal(&said, "Name a .slate or .json file that is there."));
        }
    };
    if path.ends_with(".slate") {
        return Ok(SlateFile::Text(text));
    }
    let parsed = parse_document(&text).map_err(|message| {
        usage_refusal(&format!("{path} does not parse as JSON ({message})."), "Fix the JSON or write the JSX-like form to a .slate file.")
    })?;
    match parsed {
        Value::Object(document) => match js_value(Value::Object(document)) {
            Value::Object(document) => Ok(SlateFile::Document(document)),
            _ => unreachable!("js_value keeps an object an object"),
        },
        _ => Err(usage_refusal(
            &format!("{path} holds no JSON object."),
            "Write the stored document, an object with schema, root and pieces.",
        )),
    }
}

/// `ifVersionOf` (slate.ts:64-70): `--if-version` as JavaScript's `Number` reads it, a whole number from 0, else
/// refused; absent is `None`. The value is sent as that number.
pub fn if_version_of(ctx: &VerbContext) -> Result<Option<f64>, HostError> {
    let Some(raw) = flag(&ctx.flags, "if-version") else { return Ok(None) };
    let n = to_number(raw);
    if !n.is_finite() || n.fract() != 0.0 || n < 0.0 {
        let said = format!("--if-version takes a version number, and {raw} is not one.");
        return Err(usage_refusal(&said, &usage_is(ctx)));
    }
    Ok(Some(n))
}

/// `stateValue` (slate.ts:73-82): a `<path>=<json>` word, the value parsed as JSON else kept as its text; `None`
/// where the word has no `=` after its first character.
pub fn state_value(word: &str) -> Option<(String, Value)> {
    let at = word.find('=').filter(|at| *at > 0)?;
    let raw = &word[at + 1..];
    let value = parse_value(raw).map_or_else(|| Value::from(raw), js_value);
    Some((word[..at].to_owned(), value))
}

/// A parsed JSON value as JavaScript holds it: every number is a double, which a frame writes as `String(n)` does.
fn js_value(mut value: Value) -> Value {
    let mut work = vec![&mut value];
    while let Some(at) = work.pop() {
        match at {
            Value::Number(n) if n.is_f64() => {
                let text = wsp_words::js::number_string(n.as_f64().unwrap_or_default());
                if let Ok(read) = serde_json::from_str(&text) {
                    *n = read;
                }
            }
            Value::Array(items) => work.extend(items.iter_mut()),
            Value::Object(map) => work.extend(map.values_mut()),
            _ => {}
        }
    }
    value
}

/// `SLATE_TOOL_NAMES` (slate.ts:224): the four slate tools.
pub fn slate_tool_names() -> &'static [&'static str] {
    wsp_wire::types::wsp_tools::SLATE_TOOLS
}

/// A number as JavaScript writes it: an integral value below 2^53 without a point.
fn js_number_value(n: f64) -> Value {
    serde_json::to_value(JsNumber(n)).expect("a number writes as JSON")
}

fn arg_str<'a>(args: &'a Frame, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
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
        let client = deps.client(None).await?;
        let mut params = slate_target(&*client, None, &deps.env).await?;
        params.extend(pick(vec![("name", args.get("name").cloned())]));
        let read = slate_ask(&*client, "slates.catalog", params).await?;
        let text = read.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
        Ok(as_text(&text, Value::Object(read)))
    })
}

/// `wsp slate catalog` (`verbs/slate.ts:90-96`).
fn run_slate_catalog(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp slate catalog takes one name at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let mut params = slate_target(&*client, None, &ctx.deps.env).await?;
        params.extend(pick(vec![("name", ctx.args.first().map(|name| Value::from(name.as_str())))]));
        let read = slate_ask(&*client, "slates.catalog", params).await?;
        let line = read.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
        ctx.out.emit(&Value::Object(read), Some(&line));
        Ok(0)
    })
}

/// The `slate_write` tool's call (`verbs/slate.ts:152-157`).
fn call_slate_write(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let pressed = args.get("press").and_then(Value::as_str).map(|piece| {
            pick(vec![("piece", Some(Value::from(piece))), ("index", args.get("row").cloned()), ("action", args.get("action").cloned())])
        });
        let mut params = slate_target(&*client, arg_str(&args, "thread"), &deps.env).await?;
        params.extend(pick(vec![
            ("text", args.get("text").cloned()),
            ("document", args.get("document").cloned()),
            ("check", args.get("check").cloned()),
            ("values", args.get("values").cloned()),
            ("press", pressed.map(Value::Object)),
            ("ifVersion", args.get("if_version").cloned()),
        ]));
        let wrote = slate_ask(&*client, "slates.write", params).await?;
        let text = wrote.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
        Ok(as_text(&text, Value::Object(wrote)))
    })
}

/// `wsp slate write` (`verbs/slate.ts:114-136`).
fn run_slate_write(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let mut values: IndexMap<String, Value> = IndexMap::new();
        for word in flag_list(&ctx.flags, "set") {
            let Some((path, value)) = state_value(&word) else {
                return Err(usage_refusal(&format!("--set {word} is not <path>=<json>."), "Write it like --set '$i=2'."));
            };
            values.insert(path, value);
        }
        let piece = flag(&ctx.flags, "press");
        let row = flag(&ctx.flags, "row").map(to_number);
        let action = flag(&ctx.flags, "action").map(to_number);
        for (name, n) in [("row", row), ("action", action)] {
            if let Some(n) = n {
                if piece.is_none() || !n.is_finite() || n.fract() != 0.0 || n < 0.0 {
                    return Err(usage_refusal(&format!("--{name} goes with --press, a whole number from 0:"), &usage_is(ctx)));
                }
            }
        }
        let rehearsal = !values.is_empty() || piece.is_some();
        let (first, second) = (ctx.args.first(), ctx.args.get(1));
        if (first.is_none() && !rehearsal) || ctx.args.len() > 2 {
            return Err(usage_refusal("wsp slate write takes a file, after a thread where it is not yours.", &usage_is(ctx)));
        }
        let (reference, file) = match (first, second) {
            (Some(a), Some(b)) => (Some(a.as_str()), Some(b.as_str())),
            (Some(a), None) if a.ends_with(".slate") || a.ends_with(".json") => (None, Some(a.as_str())),
            (a, None) => (a.map(String::as_str), None),
            (None, Some(_)) => (None, None),
        };
        if file.is_none() && !rehearsal {
            return Err(usage_refusal(
                "wsp slate write takes a .slate or .json file, after a thread where it is not yours.",
                &usage_is(ctx),
            ));
        }
        let client = ctx.client(None).await?;
        let if_version = if_version_of(ctx)?;
        let mut params = slate_target(&*client, reference, &ctx.deps.env).await?;
        if let Some(file) = file {
            match slate_file(ctx, file)? {
                SlateFile::Text(text) => params.insert("text".to_owned(), Value::from(text)),
                SlateFile::Document(document) => params.insert("document".to_owned(), Value::Object(document)),
            };
        }
        if flag_set(&ctx.flags, "check") {
            params.insert("check".to_owned(), Value::Bool(true));
        }
        if !values.is_empty() {
            params.insert("values".to_owned(), Value::Object(values.into_iter().collect()));
        }
        if let Some(piece) = piece {
            let press = pick(vec![
                ("piece", Some(Value::from(piece))),
                ("index", row.map(js_number_value)),
                ("action", action.map(js_number_value)),
            ]);
            params.insert("press".to_owned(), Value::Object(press));
        }
        if let Some(version) = if_version {
            params.insert("ifVersion".to_owned(), js_number_value(version));
        }
        let wrote = slate_ask(&*client, "slates.write", params).await?;
        emit_slate(ctx, wrote);
        Ok(0)
    })
}

/// The `slate_state` tool's call (`verbs/slate.ts:187-191`).
fn call_slate_state(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let mut params = slate_target(&*client, arg_str(&args, "thread"), &deps.env).await?;
        params.extend(pick(vec![
            ("values", args.get("values").cloned()),
            ("start", args.get("start").cloned()),
            ("ifVersion", args.get("if_version").cloned()),
        ]));
        let wrote = slate_ask(&*client, "slates.state", params).await?;
        let text = wrote.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
        Ok(as_text(&text, Value::Object(wrote)))
    })
}

/// `wsp slate state` (`verbs/slate.ts:166-181`).
fn run_slate_state(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let mut values: IndexMap<String, Value> = IndexMap::new();
        let mut rest: Vec<String> = Vec::new();
        for word in &ctx.args {
            match state_value(word) {
                None => rest.push(word.clone()),
                Some((path, value)) => {
                    values.insert(path, value);
                }
            }
        }
        let start = flag_list(&ctx.flags, "start");
        if rest.len() > 1 || (values.is_empty() && start.is_empty()) {
            let said = "wsp slate state takes $path=<json> words or --start <run>, after a thread where it is not yours.";
            return Err(usage_refusal(said, &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let if_version = if_version_of(ctx)?;
        let mut params = slate_target(&*client, rest.first().map(String::as_str), &ctx.deps.env).await?;
        if !values.is_empty() {
            params.insert("values".to_owned(), Value::Object(values.into_iter().collect()));
        }
        if !start.is_empty() {
            params.insert("start".to_owned(), json!(start));
        }
        if let Some(version) = if_version {
            params.insert("ifVersion".to_owned(), js_number_value(version));
        }
        let wrote = slate_ask(&*client, "slates.state", params).await?;
        emit_slate(ctx, wrote);
        Ok(0)
    })
}

/// The `slate_read` tool's call (`verbs/slate.ts:213-217`).
fn call_slate_read(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let mut params = slate_target(&*client, arg_str(&args, "thread"), &deps.env).await?;
        params.extend(pick(vec![
            ("values", args.get("values").cloned()),
            ("text", args.get("text").cloned()),
            ("sketch", args.get("sketch").cloned()),
            ("document", args.get("document").cloned()),
        ]));
        let read = slate_ask(&*client, "slates.read", params).await?;
        let text = read.get("text").and_then(Value::as_str).unwrap_or_default().to_owned();
        Ok(as_text(&text, Value::Object(read)))
    })
}

/// `wsp slate read` (`verbs/slate.ts:200-207`).
fn run_slate_read(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp slate read takes one thread at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let values = flag_list(&ctx.flags, "values");
        let mut params = slate_target(&*client, ctx.args.first().map(String::as_str), &ctx.deps.env).await?;
        if !values.is_empty() {
            params.insert("values".to_owned(), json!(values));
        }
        if flag_set(&ctx.flags, "no-text") {
            params.insert("text".to_owned(), Value::Bool(false));
        }
        if flag_set(&ctx.flags, "no-sketch") {
            params.insert("sketch".to_owned(), Value::Bool(false));
        }
        if flag_set(&ctx.flags, "document") {
            params.insert("document".to_owned(), Value::Bool(true));
        }
        let read = slate_ask(&*client, "slates.read", params).await?;
        emit_slate(ctx, read);
        Ok(0)
    })
}
