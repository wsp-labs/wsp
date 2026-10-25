// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/activity.ts` at 65f6b89c7: `wsp activity` and `wsp activity end` on the command line
//! and the tools, this computer's processes and a box's by the thread that started them, and the `--processes`
//! choice settle and delete hand the host. Spec: `spec/activity.md`.

use wsp_wire::views::activity::{ActivityEnded, ActivityView};
use wsp_words::exit::HostError;

use super::client::*;

/// `STARTED_WORDS` (activity.ts:25): what each door says the start is.
pub const STARTED_WORDS: &str = "the start activity listed it with, its startedAt in epoch milliseconds";

/// `readActivity` (activity.ts:27-29): `activity.read` with `{computer?, thread?}` in that key order, its
/// `activity` field parsed as the view.
pub async fn read_activity(client: &dyn HostClient, computer: Option<&str>, thread: Option<&str>) -> Result<ActivityView, HostError> {
    let _ = (client, computer, thread);
    todo!("activity.ts:27-29")
}

/// `endProcess` (activity.ts:31-33): `activity.end` with `{pid, startedAt, computer?}` in that key order (the
/// caller's spread order), the whole reply parsed as `ActivityEnded`.
pub async fn end_process(client: &dyn HostClient, pid: i64, started_at: f64, computer: Option<&str>) -> Result<ActivityEnded, HostError> {
    let _ = (client, pid, started_at, computer);
    todo!("activity.ts:31-33")
}

/// `activityLines` (activity.ts:42-48): the tool's text for a read.
pub fn activity_lines(view: &ActivityView) -> String {
    let _ = view;
    todo!("activity.ts:42-48")
}

/// `endedLines` (activity.ts:50-58): what an end says, a freed kept agent first.
pub fn ended_lines(ended: &ActivityEnded) -> String {
    let _ = ended;
    todo!("activity.ts:50-58")
}

/// `processesAsked` (activity.ts:65): `oneOf("processes", PROCESSES_CHOICES, value, on)`.
pub fn processes_asked(value: Option<&str>, on: Option<&str>) -> Result<Option<String>, HostError> {
    let _ = (value, on);
    todo!("activity.ts:65")
}

/// `ProcessesIn` (activity.ts:67): the optional `processes` input of settle and delete, its words through
/// `cloud_text` with the cloud as this process reads it.
pub fn processes_in() -> ToolInput {
    todo!("activity.ts:67")
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
        let _ = (args, deps);
        todo!("verbs/activity.ts:91-94")
    })
}

/// `wsp activity` (`verbs/activity.ts:77-82`).
fn run_activity(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/activity.ts:77-82")
    })
}

/// The `activity_end` tool's call (`verbs/activity.ts:123-126`).
fn call_activity_end(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/activity.ts:123-126")
    })
}

/// `wsp activity end` (`verbs/activity.ts:103-114`).
fn run_activity_end(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/activity.ts:103-114")
    })
}
