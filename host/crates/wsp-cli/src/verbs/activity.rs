// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/activity.ts` at 65f6b89c7: `wsp activity` and `wsp activity end` on the command line
//! and the tools, this computer's processes and a box's by the thread that started them, and the `--processes`
//! choice settle and delete hand the host. Spec: `spec/activity.md`.

use serde_json::Value;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::activity::{end_gone_said, ActivityEnded, ActivityView, END_GRACE_WORD, PROCESSES_CHOICES};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::js::to_number;
use wsp_words::words::computer::thread_word;
use wsp_words::words::units::fmt_bytes;

use super::activity_table::{activity_table, cpu_word, kind_word, TableOpts};
use super::client::*;
use super::io::as_text;
use super::workspaces_help::thread_of;

/// `STARTED_WORDS` (activity.ts:25): what each door says the start is.
pub const STARTED_WORDS: &str = "the start activity listed it with, its startedAt in epoch milliseconds";

/// `readActivity` (activity.ts:27-29): `activity.read` with `{computer?, thread?}` in that key order, its
/// `activity` field parsed as the view.
pub async fn read_activity(client: &dyn HostClient, computer: Option<&str>, thread: Option<&str>) -> Result<ActivityView, HostError> {
    let mut params = Frame::new();
    if let Some(computer) = computer {
        params.insert("computer".to_owned(), Value::from(computer));
    }
    if let Some(thread) = thread {
        params.insert("thread".to_owned(), Value::from(thread));
    }
    let reply = client.request("activity.read", params).await?;
    let view = reply.get("activity").cloned().and_then(|value| serde_json::from_value(value).ok());
    view.ok_or_else(|| HostError::new(other_version("activity.read")))
}

/// `endProcess` (activity.ts:31-33): `activity.end` with `{pid, startedAt, computer?}` in that key order (the
/// caller's spread order), the whole reply parsed as `ActivityEnded`.
pub async fn end_process(client: &dyn HostClient, pid: i64, started_at: f64, computer: Option<&str>) -> Result<ActivityEnded, HostError> {
    let mut params = Frame::new();
    params.insert("pid".to_owned(), Value::from(pid));
    params.insert("startedAt".to_owned(), serde_json::to_value(JsNumber(started_at)).expect("a number writes as JSON"));
    if let Some(computer) = computer {
        params.insert("computer".to_owned(), Value::from(computer));
    }
    let reply = client.request("activity.end", params).await?;
    serde_json::from_value(Value::Object(reply)).map_err(|_| HostError::new(other_version("activity.end")))
}

/// `readFor` (activity.ts:35-39): the read, its thread named by a `threadOf` read first.
async fn read_for(client: &dyn HostClient, computer: Option<&str>, reference: Option<&str>) -> Result<ActivityView, HostError> {
    let thread = match reference {
        Some(reference) => Some(thread_of(client, reference).await?.id),
        None => None,
    };
    read_activity(client, computer, thread.as_deref()).await
}

/// One process of a read, as the tool's text names it: its depth, pid, kind, command, ports, memory and CPU.
fn proc_line(p: &wsp_wire::views::activity::ActivityProc) -> String {
    let ports = if p.ports.is_empty() {
        String::new()
    } else {
        format!("  :{}", p.ports.iter().map(|port| port.get().to_string()).collect::<Vec<_>>().join(" :"))
    };
    format!(
        "{}{}  {}  {}{ports}  {}  {}",
        "  ".repeat(p.depth.get() as usize + 1),
        p.pid.get(),
        kind_word(p.kind),
        p.command,
        fmt_bytes(p.memory.0),
        cpu_word(p.cpu.0)
    )
}

/// `activityLines` (activity.ts:42-48): the tool's text for a read.
pub fn activity_lines(view: &ActivityView) -> String {
    let mut lines: Vec<String> = Vec::new();
    for t in &view.threads {
        lines.push(format!("thread {} {}  {}  {}", thread_word(&t.thread_id), t.title, fmt_bytes(t.memory.0), cpu_word(t.cpu.0)));
        lines.extend(t.processes.iter().map(proc_line));
    }
    if !view.wsp.is_empty() {
        lines.push("wsp".to_owned());
        lines.extend(view.wsp.iter().map(proc_line));
    }
    if !view.unowned.is_empty() {
        lines.push("no thread".to_owned());
        lines.extend(view.unowned.iter().map(proc_line));
    }
    if lines.is_empty() {
        return format!("no thread runs a process on {}", view.computer);
    }
    lines.join("\n")
}

/// `endedLines` (activity.ts:50-58): what an end says, a freed kept agent first.
pub fn ended_lines(ended: &ActivityEnded) -> String {
    if let Some(freed) = &ended.freed {
        return format!("freed the agent thread {} kept", thread_word(freed));
    }
    let Some((first, under)) = ended.ended.split_first() else {
        return end_gone_said(ended.pid.get());
    };
    let under_word = if under.is_empty() { String::new() } else { format!(" and {} under it", under.len()) };
    let mut lines = vec![format!("ended {} {}{under_word}", first.pid.get(), first.name)];
    let pids = |list: &[wsp_wire::types::js_number::JsInt]| list.iter().map(|p| p.get().to_string()).collect::<Vec<_>>().join(" ");
    if !ended.forced.is_empty() {
        lines.push(format!("KILL after TERM's {END_GRACE_WORD}: {}", pids(&ended.forced)));
    }
    if !ended.left.is_empty() {
        lines.push(format!("still running after KILL: {}", pids(&ended.left)));
    }
    lines.join("\n")
}

/// `processesAsked` (activity.ts:65): `oneOf("processes", PROCESSES_CHOICES, value, on)`.
pub fn processes_asked(value: Option<&str>, on: Option<&str>) -> Result<Option<String>, HostError> {
    one_of("processes", PROCESSES_CHOICES, value, on)
}

/// `ProcessesIn` (activity.ts:67): the optional `processes` input of settle and delete, its describe with the cloud
/// span still in it: `in_cloud_words` cuts it where the TS cuts it at load.
pub fn processes_in() -> ToolInput {
    ToolInput {
        name: "processes",
        kind: InputKind::Enum(PROCESSES_CHOICES.to_vec()),
        optional: true,
        describe: Some(
            "what to do with the processes a thread still runs beside its agent: stop ends each by identity, and on a box ends everything in the thread's cgroup there; keep leaves them running; refused without one while any run<!-- cloud -->. A thread on a cloud machine is not read and goes as before<!-- /cloud -->".to_string(),
        ),
    }
}

/// The entries of `verbs/activity.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn activity_verbs() -> Vec<Verb> {
    vec![
    // verbs/activity.ts:72
    Verb::Cli(CliVerb {
        name: "activity",
        usage: "wsp activity [<thread>] [--computer <computer>] [--wide]".to_string(),
        about: "this computer's processes, or a box's, as Settings > Activity lists them: wsp's own, then each thread heaviest first with what it started, then what has no thread, each with its kind, ports, memory and CPU; a thread named shows its own alone, on this computer or the box it runs on".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("computer", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("wide", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_activity,
        tool: Tool {
            description: "This computer's processes, or a box's with computer, grouped under the thread that started them: each thread's tree from its agent down to what the agent ran (a dev server, a test run, a tool server), with each process's pid, ppid, depth in the tree, kind (agent, tool, dev, test, build, slate, terminal, tab, app, wsp, other), command, memory in bytes, CPU as a share of one core, the TCP ports it listens on, and startedAt, which activity_end takes back with the pid. A thread's memory and cpu are its processes' together. wsp holds wsp's own processes, and unowned what the daemon names as wsp's with no thread; what the person started outside wsp is not listed. With thread, that thread's alone, on the computer it runs on. On a box a thread's processes are what stands in its cgroup there. A thread reads itself and the threads under it alone.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("the thread, by its id or a prefix of it that names one, whose processes alone to list".to_string()) },
                ToolInput { name: "computer", kind: InputKind::String, optional: true, describe: Some("the computer: this one, which is the default, or a box wsp computers lists".to_string()) },
            ],
            output: vec!["computer", "at", "threads", "wsp", "unowned"],
            stream: vec![],
            call: call_activity,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/activity.ts:98
    Verb::Cli(CliVerb {
        name: "activity end",
        usage: "wsp activity end <pid> --started <ms> [--computer <computer>]".to_string(),
        about: "ends one process on this computer and everything under it, by the pid and the start wsp activity --json listed: TERM, then KILL after 2 s; refuses a process wsp did not start, an agent, which wsp stop ends, and a pid that names another process now".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("started", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("computer", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_activity_end,
        tool: Tool {
            description: "Ends one process on this computer and every process under it, by its pid and the startedAt activity listed it with: TERM to each, then KILL to whatever of them is still there after 2 s, each only while it is still the process listed. Never a process group. Refused, with nothing signalled: a pid no longer running, a process wsp did not start (one activity does not list), a pid that names another process now (read activity again), an agent from its turn's process down to the agent's binary, or a process with one under it, which only the thread's stop ends, and wsp's own processes. A thread's kept agent, the one it keeps warm for its next turn, is freed through its thread instead. A thread ends only what it and the threads under it run. The answer lists each process ended, those KILL went to and any still running after it.".to_string(),
            input: vec![
                ToolInput { name: "pid", kind: InputKind::Integer { min: Some(1) }, optional: false, describe: Some("the process's pid, as activity lists it".to_string()) },
                ToolInput { name: "started", kind: InputKind::Number, optional: false, describe: Some("the start activity listed it with, its startedAt in epoch milliseconds".to_string()) },
                ToolInput { name: "computer", kind: InputKind::String, optional: true, describe: Some("the computer, which must be this one; a box ends a thread's processes when its thread settles with processes stop".to_string()) },
            ],
            output: vec!["pid", "ended", "forced", "left", "freed"],
            stream: vec![],
            call: call_activity_end,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    ]
}

/// The `activity` tool's call (`verbs/activity.ts:91-94`).
fn call_activity(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let computer = args.get("computer").and_then(Value::as_str);
        let thread = args.get("thread").and_then(Value::as_str);
        let view = read_for(&*client, computer, thread).await?;
        Ok(as_text(&activity_lines(&view), json_value(&view)))
    })
}

/// `wsp activity` (`verbs/activity.ts:77-82`).
fn run_activity(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp activity takes one thread at most.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let view = read_for(&*client, flag(&ctx.flags, "computer"), ctx.args.first().map(String::as_str)).await?;
        let width = ctx.io.redraw().map(|redraw| usize::from(redraw.columns()));
        let table = activity_table(&view, TableOpts { width, wide: flag_set(&ctx.flags, "wide") });
        ctx.out.emit(&json_value(&view), Some(&table));
        Ok(0)
    })
}

fn json_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a view writes as JSON")
}

/// The `activity_end` tool's call (`verbs/activity.ts:123-126`).
fn call_activity_end(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let pid = args.get("pid").and_then(Value::as_f64).unwrap_or_default() as i64;
        let started = args.get("started").and_then(Value::as_f64).unwrap_or_default();
        let computer = args.get("computer").and_then(Value::as_str);
        let ended = end_process(&*deps.client(None).await?, pid, started, computer).await?;
        Ok(as_text(&ended_lines(&ended), json_value(&ended)))
    })
}

/// `wsp activity end` (`verbs/activity.ts:103-114`).
fn run_activity_end(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let pid = match ctx.args.as_slice() {
            [word] => to_number(word),
            _ => f64::NAN,
        };
        if ctx.args.len() != 1 || !pid.is_finite() || pid.fract() != 0.0 || pid <= 0.0 {
            return Err(usage_refusal("wsp activity end takes one pid.", &usage_is(ctx)));
        }
        let started = flag(&ctx.flags, "started");
        let started_at = started.map_or(f64::NAN, to_number);
        if started.is_none_or(|word| word.is_empty()) || !started_at.is_finite() {
            return Err(usage_refusal(&format!("wsp activity end takes --started, {STARTED_WORDS}."), &usage_is(ctx)));
        }
        let computer = flag(&ctx.flags, "computer");
        let ended = end_process(&*ctx.client(None).await?, pid as i64, started_at, computer).await?;
        ctx.out.emit(&json_value(&ended), Some(&ended_lines(&ended)));
        Ok(0)
    })
}
