// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/threads.ts` at 65f6b89c7: the thread verbs, their command lines and their tools.
//! Spec: `spec/threads-a.md` (run to the answer verbs, threads.ts:1-448) and `spec/threads-b.md` (send to export,
//! threads.ts:449-723). The table's words are copied by script from the TS module (tests/fixtures/thread_verbs.json);
//! every run and call body is the implementer's.

use std::sync::{Arc, Mutex};

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_collect::ghostty_config::read_ghostty_config;
use wsp_collect::live_host::node_host;
use wsp_wire::types::conversations::{ResumeAsk, RESUME_HERE_FIX, RESUME_HERE_LINE};
use wsp_wire::types::sub_thread::SubOut;
use wsp_wire::views::activity::ProcessesAsk;
use wsp_wire::views::project_bundle::{TerminalConfig, TerminalScheme};
use wsp_wire::views::session::SessionOrigin;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView};
use wsp_wire::wire::runtime::{ExecEvent, SessionSettleResult};
use wsp_words::exit::{usage_refusal, ExitClass, HostError};
use wsp_words::js_date::DateZone;
use wsp_words::projects::worktree_removed_line;
use wsp_words::sub_thread::{sub_beside_refusal, sub_from_no_cloud_refusal, sub_opened_line, sub_opening, sub_size_no_cloud_refusal};
use wsp_words::words::init::init_setup_lines;
use wsp_words::words::places::terminal_config_lines;
use wsp_words::words::thread::{
    exec_folder_line, AT_WITHOUT_FORK_FIX, AT_WITHOUT_FORK_LINE, BESIDE_ALONE_FIX, BESIDE_ALONE_LINE, EMPTY_MESSAGE_LINE, FORK_BESIDE_FIX,
    FORK_BESIDE_LINE, FORK_RESUME_FIX, FORK_RESUME_LINE,
};
use wsp_words::workspace_state::turn_spend_word;

use super::cloud_of;

use super::activity;
use super::activity::{ended_lines, processes_asked};
use super::client::{
    absolute_folder, absolute_path, flag, flag_list, flag_set, host_back, other_version, usage_is, BoxFuture, CallToolResult, CliOnlyVerb,
    CliVerb, FlagKind, FlagRow, FlagTable, FlagValue, Frame, HostClient, InputKind, Page, Tool, ToolInput, Verb, VerbContext, VerbDeps,
};
use super::io;
use super::io::{
    as_json, as_text, confirmed, detached_out, exec_on, folder_lines, host_folders, init_setup, noticed, resolve_path, turn_out, turn_text,
    ExecSink, ExportRequest, FoldersAsked, TurnOut,
};
use super::turns_help::{
    answer_open_ask, answer_roads, before_sending, checked_start, detach_verb, files_from, follow, follow_verb, fork_for, head_line,
    host_restarted_line, message_to, notify_of, opening_of, pick_flags, read_line, read_thread, replaced_of, restart_host, resume_asked,
    run_target, start_detached, sub_for, thread_at_in, thread_fork, thread_head, turn_view, worktree_for, AnswerRoad, AtWord, FollowOn,
    Opened, OpeningAt, OpeningOpts, Picks, ResumeWhere, RunTarget, RunWhere, SaidAbout, SharedTurn, SubAsk, SubMade, ThreadRead,
};
use super::workspaces_help::{
    awake, forget_thread, nap_after_dead_launch, pipe_bytes, rename, rename_line, restore_threads, restored_lines, settle_threads,
    settled_lines, ssh_workspace_of, stop, stop_line, thread_cwd, thread_forgot_line, thread_of, threads_of, with_line, workspace_of,
    Woken, SSH_PIPES_HERE_LINE,
};

fn input(name: &'static str, kind: InputKind, optional: bool, describe: Option<&str>) -> ToolInput {
    ToolInput { name, kind, optional, describe: describe.map(str::to_string) }
}

/// The first flag beside `--on` a sub-thread takes none of, as the line or the tool named it: the first pair whose
/// value is given and not `false` (`besideOn`, threads.ts:84).
pub fn beside_on(given: &[(&str, Option<serde_json::Value>)]) -> Option<String> {
    given.iter().find(|(_, value)| matches!(value, Some(v) if *v != serde_json::Value::Bool(false))).map(|(name, _)| (*name).to_owned())
}

/// `THREAD_VERBS` (threads.ts:104-723): run, worktree, worktree remove, thread read, thread head, thread rename,
/// thread forget, thread settle, thread restore, thread allow, thread deny (`ANSWER_VERBS`, threads.ts:56-81, one per
/// `ANSWER_ROADS` row that carries an answer), the slate verbs (`SLATE_VERBS`), then send, restart, stop, ssh, exec,
/// folders, setup, terminal config, export. Usage, about and descriptions keep their cloud spans; `in_cloud_words`
/// cuts them, the tool inputs' describes included.
pub fn thread_verbs() -> Vec<Verb> {
    let mut verbs = vec![
        Verb::Cli(CliVerb {
            name: "run",
            usage: "wsp run [<project>] [--on boat [--size <cpu>x<memGb>] [--from <ref>]] [--beside <thread>] [--fork <thread> [--at <n>]] [--branch <branch>] [--cwd <path>] [--agent <id>] [--model, --effort, --access <word>] [--fast] [--notify <thread|me>] [--title <title>] [--replaces <thread>] [--file <path>] [--resume <id> [--copy]] [--detach] \"<message>\"".to_string(),
            about: "an agent works in the project's folder and you read its reply: a thread with the agent, model, effort and access the app offers; a project on a box runs in its folder there as the login the box was added with<!-- cloud -->, and a project on a cloud gets a new machine forked from the image, named off the message as the app's New thread names one<!-- /cloud -->; --on boat starts it as a sub-thread of the thread asking, on a machine of its own on Boat forked from your image at --size or Boat's own size, which fetches your pushed branch, or with --from a branch, tag or commit the remote holds, and pushes --branch or wsp/<slug>; --beside runs it in the folder another thread works in, on that thread's computer; --branch runs it in a worktree of the project's repo on that branch, made under wsp's folder unless one already holds it, and --cwd in a folder inside the project or one of its worktrees; --fork starts it from another thread's conversation through its latest finished turn, or the one --at counts from 1, on that thread's agent in its folder, or with --branch in a new worktree whose files are that turn's; with no project, run from inside one of your project folders, or from a thread, beside it; follows its first turn, or with --detach prints the id and returns".to_string(),
            page: Page::Front,
            options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("size", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("from", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("beside", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("fork", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("at", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("branch", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("cwd", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("access", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("fast", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("notify", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("title", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("replaces", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("file", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("resume", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("copy", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("detach", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
            run: run_run,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Opens a thread under the named agent, on the model, effort and access mode named or the catalog's defaults (a cheaper model for a review, say), and follows its first turn; returns the reply text as soon as it is complete, with the thread id for send. It runs in the project's folder, on a box in its folder there as the login the box was added with<!-- cloud -->, and on a project on a cloud in a new machine forked from the image, named off the message as the app's New thread names one<!-- /cloud -->; with beside, in the folder that thread works in, on its computer, which is how a fresh thread picks up where a long one stands; with branch, in a worktree of the project's repo on that branch (one that already holds the branch, wherever it is, else one wsp makes under its own folder with the dependencies carried in); with cwd, in that folder, which must be inside the project or one of its worktrees. With no project, from a thread, it runs beside that thread in its folder. With detach true it returns the thread id the moment the turn is started, without the reply: the road for a turn that runs for minutes or an hour. A turn ends when the agent process exits, not at its reply, and the thread reads running until then. With notify, each turn of the thread sends one line (outcome, duration, cost, and the reply whole into a thread or its last line to the person) to every target named, so a caller need not wait here or poll. The notify line goes once, at the reply; a reply given with background tasks still running says so, and a second line goes at the turn's end. A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report. With replaces, the thread restarts a stopped or failed one, which settles once it starts, so the person sees one row. With fork, the thread starts from another thread's conversation through one of its finished turns, the latest or the one at counts, and nothing after, on that thread's agent and in its folder, or with branch in a new worktree whose files are that turn's checkpoint, leaving that thread as it was. A start on a computer already running as many threads as its threads at once waits for a slot and starts on its own when one frees (outcome `held`), with capped beside it, the computer and what runs there against its number, which the thread's row in threads carries while it waits; a thread that follows a run or a send to its end, not detached, lends that thread's next turn its own slot, one turn at a time, a turn already held ahead of the message included, and every other start, a child started detached included, waits for a slot of its own. When the person asks for another agent, on another model or another harness, start it as a wsp thread with run, so it shows in their sidebar; your own subagent tool is for your own sub-steps.".to_string(),
                input: vec![
                    io::run_project_in("project"),
                    input("beside", InputKind::String, true, Some("a thread, by its id or a prefix of it, beside which the new thread runs: in the folder that thread works in, on the computer it runs on; never with project or branch")),
                    input("fork", InputKind::String, true, Some("a thread to fork, by id or a prefix of it, as threads lists them: the new thread runs on that thread's agent, at its model and effort unless named, with its conversation through one finished turn and nothing after, in that thread's folder, or with branch in a new worktree on that branch whose files are that turn's checkpoint; never with project, beside, cwd or replaces, and an agent other than the thread's is refused")),
                    input("at", InputKind::Integer { min: Some(1) }, true, Some("with fork, the finished turn the fork carries the conversation through, counted from 1 in the order thread_read lists them; absent, the thread's latest finished turn")),
                    input("branch", InputKind::String, true, Some("a branch other than the one the project's folder has checked out: the thread runs in the worktree holding it, made under wsp's folder from the project folder's current commit for a new branch")),
                    input("on", InputKind::String, true, Some("boat, to start the thread as a sub-thread of the thread asking on a machine of its own on Boat, forked from the person's image. It fetches your pushed branch, or the ref from names, and pushes a branch of its own; without from your branch must be pushed first. It carries the thread's own project, which project may name or leave out; another project is refused. Never with beside, fork, at, cwd, resume, copy or replaces")),
                    input("size", InputKind::String, true, Some("with on, the machine's size as <cpu>x<memGb>: 2x4, 4x8, 8x16 or 16x32, those the person's plan sells; Boat's own size setting without it. A thread that builds or runs a test suite wants 4x8")),
                    input("from", InputKind::String, true, Some("with on, a branch, tag or commit the remote already holds (origin/main, main, v1.2, a commit id) that the sub-thread starts from instead of your branch, which then need not be pushed; a ref the remote does not hold is refused naming it")),
                    input("cwd", InputKind::String, true, Some("a folder inside the project or one of its worktrees, absolute, which the thread starts in")),
                    input("message", InputKind::String, false, None),
                    input("agent", InputKind::String, true, Some("the agent to run in the thread, one of claude, codex, opencode, cursor; absent means the host's default")),
                    input("model", InputKind::String, true, Some("the model the turn runs on, by the agent's own slug (claude-sonnet-5); absent on a new thread means the project's, else the one set for that agent, else the catalog's default, on send the thread's own")),
                    input("effort", InputKind::String, true, Some("the reasoning effort, by the agent's own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default, high for claude; on send, the thread's own")),
                    input("access", InputKind::String, true, Some("how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full")),
                    input("fast", InputKind::Boolean, true, Some("true runs the turn in the agent's fast mode, on a model that offers one, and is refused naming the model otherwise; absent runs at the agent's usual speed")),
                    input("notify", InputKind::StringList { min: Some(1) }, true, Some("who is told each time a turn of the new thread ends, each one a thread (by id, or a prefix of it) the line goes into as a message carrying that turn's whole reply, or me, which is the thread this call came out of when it came out of one and otherwise the person's app, where the line is its last reply line alone. Several targets each get the line once, which is how a builder's end reaches its orchestrator and a reviewer together; a target whose thread is gone by then falls back to the person, and the new thread's own id is refused. A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report.")),
                    input("title", InputKind::String, true, Some("the thread's name, as a person's: it shows in the sidebar and in the agent's own list from the first second, and the title the host asks the agent for as the turn starts never replaces it; absent lets the thread be titled by its opening words until, seconds in, the agent names it")),
                    input("replaces", InputKind::String, true, Some("a stopped or failed thread this new thread restarts, by id or a prefix of it, as threads lists them: the new thread's row names it, and once the new one starts the host settles it, so the person sees one row for the job; a restart the person starts of a child stands under that child's lead. Refused while that thread or one under it is working, for a thread that already has a restart (name that restart instead), and from a thread for one that is not under it, each before a machine is forked or woken")),
                    input("files", InputKind::StringList { min: None }, true, Some("paths on this computer, absolute or relative to the folder wsp runs in, of files to send with the message, at most 5. An image (PNG, JPEG, GIF or WebP, up to 10 MB) goes to the agent as an image; any other file, up to 10 MB, lands in the folder the thread works in, under .wsp-files where git lists none of it, and the message names its path. The host reads each file and sends its bytes, so the machine never reaches back for this computer’s files; an image to an agent that reads none is refused naming that agent.")),
                    input("resume", InputKind::String, true, Some("a conversation the agent kept in the project's folder outside wsp, by the id conversations lists: the new thread opens on it, in the folder it ran in, under its own name, with its newest messages written above the message, and nothing of wsp's is written into the agent's own store until the thread is renamed here. Refused with beside, branch or cwd, since the conversation names its folder, where the agent holds no such conversation, where it is a wsp thread already, and where another app holds it open, unless copy")),
                    input("copy", InputKind::Boolean, true, Some("with resume, true runs the thread on a copy of the conversation the agent makes (Claude Code's --fork-session, Codex's thread/fork), the original left byte for byte as it was: the answer to one open in another app")),
                    input("detach", InputKind::Boolean, true, Some("true answers with the thread id the moment the turn is started, without the reply, and the turn's end reaches whoever notify named; for a turn that runs for minutes or an hour, so this call does not block for it. A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report."))
                ],
                output: vec!["threadId", "workspaceId", "harness", "text", "outcome", "capped", "afterCut", "notice", "sub"],
                stream: vec![],
                call: call_run,
            },
        }),
        Verb::Cli(CliVerb {
            name: "worktree",
            usage: "wsp worktree <project> <branch>".to_string(),
            about: "a worktree of the project's repo on the branch, for an agent to work in by path or to start a thread in: the one git already has the branch checked out in, wherever it is, else one wsp makes under its own folder, from the project folder's current commit for a new branch, with the folder's .env files and installed dependencies carried in; prints its path".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![]),
            run: run_worktree,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "A worktree of the project's repo on the branch, answered with its path: the one git already has the branch checked out in, the project folder or a worktree the person or an agent made included, else one wsp makes under its own folder, a new branch starting from the project folder's current commit, with .env files carried in from the project folder and, for each folder of the branch holding an ecosystem's lockfile where the project folder holds that ecosystem's folder there, or at the top for one that carries nothing (uv, Poetry, Cargo, Go), its relocatable dependency folders under it carried in, the folders tied to their path (a virtualenv, target) never copied, and its install run once in that folder, without writing a tracked file, unless what was carried records that it was installed from that very lockfile, then the project's after-worktree command (projects_set). On a Linux disk that shares no blocks a carried folder is a mount, so git clean -fdx there says Device or resource busy once for each and leaves it as it was. made says whether wsp made it, which is the only kind it ever removes. A running session cannot move into it: work there by path, or start a thread in it with run and cwd. git worktree add works too, without the carried files and the cleanup.".to_string(),
                input: vec![
                    input("project", InputKind::String, false, Some("the project, by the name or the id projects lists")),
                    input("branch", InputKind::String, false, Some("the branch, existing or new"))
                ],
                output: vec!["path", "branch", "made"],
                stream: vec![],
                call: call_worktree,
            },
        }),
        Verb::Cli(CliVerb {
            name: "worktree remove",
            usage: "wsp worktree remove <project> <branch> [--force] [--yes]".to_string(),
            about: "takes away a worktree wsp made for the branch, with git: refused while a thread is working in it, and over files no commit holds unless --force; the branch stays, and a worktree wsp did not make is never removed".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("force", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
            run: run_worktree_remove,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Takes away the worktree wsp made for the branch, with git: refused while a thread is working in it, and over files no commit holds unless force, which loses them. The branch itself stays, and a detached worktree's commit is kept under refs/rescue. A worktree the person or an agent made is never removed; threads that ran in a removed worktree go on in the project folder.".to_string(),
                input: vec![
                    input("project", InputKind::String, false, Some("the project, by the name or the id projects lists")),
                    input("branch", InputKind::String, false, Some("the branch the worktree holds")),
                    input("force", InputKind::Boolean, true, Some("remove it over files no commit holds, which go with it"))
                ],
                output: vec!["project", "branch", "removed"],
                stream: vec![],
                call: call_worktree_remove,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread read",
            usage: "wsp thread read <thread> [--last]".to_string(),
            about: "the thread's messages as the app lists them, oldest first: who each one is, when the runtime recorded it and the text, with every tool call folded to the one line the app's row reads; --last prints the final reply alone, the whole message its finished line carries. A tool's output and the agent's reasoning are no rows of it. Reading marks the thread read, so it stops reading Done here and in the app".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("last", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
            run: run_thread_read,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "The thread's messages as the app lists them, oldest first: each one's who (person for the message that opened or steered a turn, agent for the agent's own words, tool for one call of its folded to a line, turn for the outcome, duration and cost the turn ended with), at, the ms epoch the runtime recorded it, and its text. With last true, the final reply alone, the whole message the thread's finished line carries, and a row under it saying so when the thread has started another turn since, so a report is never read as the one being written. This is how you read a thread you did not open, and how you read the report behind a line that reached you; the transcript is the host's, so nothing on a machine is touched and a paused machine's thread reads the same as a running one's. A thread of many turns answers with all of them, so read one with last true when the report is what you are after. A call's output and the agent's reasoning are no rows of it. A thread whose rows the transcript's cap has dropped answers with none, which is an answer and not an error. Reading marks the thread read, as the app showing it does, so a finished thread stops reading Done in threads and in the app.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, Some("the thread's id, or a prefix of it that names one, as threads lists them")),
                    input("last", InputKind::Boolean, true, Some("true answers with the final reply alone, the whole message the thread's finished line carries, with a row under it where the thread has started another turn since; absent answers with every message"))
                ],
                output: vec!["threadId", "messages"],
                stream: vec![],
                call: call_thread_read,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread head",
            usage: "wsp thread head <thread>".to_string(),
            about: "the thread's facts and its newest events, what the app draws first on opening it: the title, the agent, model and access it runs on, where it stands and its folder, then as many of its newest events as fit in 64 KB, each tool result past 2 KB cut and marked with its whole length. Reading a head marks nothing".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![]),
            run: run_thread_head,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "The thread's head (by id, or a prefix of it): facts, the thread as threads lists it with the model, effort and context window its latest turn runs on and turnId while a turn runs; events, as many of the thread's newest transcript events as fit in 64 KB, oldest first, each tool result past 2 KB cut with cut set to its whole length; pos, the newest position the transcript has issued, which every event carries as its own pos; and total, how many events of the thread the transcript holds. It is the cheap look at a thread: read the whole conversation with thread_read. Nothing on a machine is touched and the thread is not marked read.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, Some("the thread's id, or a prefix of it that names one, as threads lists them"))
                ],
                output: vec!["facts", "events", "pos", "total", "trimmed"],
                stream: vec![],
                call: call_thread_head,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread rename",
            usage: "wsp thread rename <thread> \"<title>\"".to_string(),
            about: "names the thread inside the agent's own store, so the agent shows the same name".to_string(),
            page: Page::App,
            options: FlagTable(vec![]),
            run: run_thread_rename,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Names the thread (by id, or a prefix of it) in the agent's own store on the machine, the field the agent writes when a person renames the session inside it, so the thread reads by that name in wsp and in the agent. outcome renamed means the store took it; unsupported means the thread's agent keeps no name of a person's, which is an answer, not an error; no-session means the agent's store on the machine has no such session; failed means the store refused the write and error carries the machine's own line for it. Whether an agent keeps a name is on its row in harnesses.list, from the adapter on the machine.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, None),
                    input("title", InputKind::String, false, None)
                ],
                output: vec!["threadId", "title", "harness", "outcome", "error"],
                stream: vec![],
                call: call_thread_rename,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread forget",
            usage: "wsp thread forget <thread> [--yes]".to_string(),
            about: "drops a thread no turn ever ran on, the row a launch that never got going leaves behind; refused once a turn of it did work".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
            run: run_thread_forget,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Drops a thread (by id, or a prefix of it) no turn ever ran on: the row a launch that never got going leaves in threads and in the person's sidebar goes, and nothing is asked of the machine. A launch the agent refused outright, for want of a sign-in, counts as one that ran nothing and goes the same way. Refused in one line once a turn of the thread did work, since that work is written down on it and nowhere else; delete takes a thread that ran, with its turns.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, Some("the thread's id, or a prefix of it that names one, as threads lists them"))
                ],
                output: vec!["threadId", "workspaceId"],
                stream: vec![],
                call: call_thread_forget,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread settle",
            usage: "wsp thread settle <thread>... [--finished] [--processes stop|keep]".to_string(),
            about: "folds each thread and every thread under it into the sidebar's Settled, leaving a tree with a thread still working or asking in it; a thread settles itself and the threads it started, never its lead; refused while one still runs processes, on this computer or a box, unless --processes says to stop or keep them".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("finished", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("processes", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_thread_settle,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Settles each thread (by id, or a prefix of it) and every thread under it: they fold into the person's Settled in the sidebar, as the app's Settle does, and come back by themselves on their next turn. A thread with a thread working or asking anywhere in its tree is left whole, with why, as is one already settled; with finished, each thread named stays and every finished thread under it settles, a failed one staying for the person to read and one with work under it left with why. A thread settles itself and the threads under it, never its lead or a thread beside it. Settle each child once its report is acted on. The answer lists what settled and what was left, with why.".to_string(),
                input: vec![
                    input("threads", InputKind::StringList { min: Some(1) }, false, Some("the threads' ids, or prefixes that each name one, as threads lists them")),
                    input("finished", InputKind::Boolean, true, Some("settle the finished threads under each thread named, and not the thread itself")),
                    activity::processes_in()
                ],
                output: vec!["settled", "left", "ended"],
                stream: vec![],
                call: call_thread_settle,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread restore",
            usage: "wsp thread restore <thread>...".to_string(),
            about: "brings each thread back out of the sidebar's Settled with what the latest settle naming it moved".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![]),
            run: run_thread_restore,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Brings each thread (by id, or a prefix of it) back out of the person's Settled in the sidebar, with the threads under it that the latest settle naming it moved: after a settle with finished, the finished threads that settle moved. A thread folded by the quiet time or settled before stays. A thread restores itself and the threads under it, never its lead or a thread beside it. The answer lists what came back.".to_string(),
                input: vec![
                    input("threads", InputKind::StringList { min: Some(1) }, false, Some("the threads' ids, or prefixes that each name one, as threads lists them"))
                ],
                output: vec!["restored"],
                stream: vec![],
                call: call_thread_restore,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread allow",
            usage: "wsp thread allow <thread>".to_string(),
            about: "answers the prompt the thread is stopped on and lets the call run".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![]),
            run: run_thread_allow,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Answers the prompt the thread is stopped on and lets the call run, by thread id or a prefix of it. A thread stopped on a prompt reads Needs you in threads and runs nothing until somebody picks, so this is how a thread you did not open is unstuck; the prompt itself is on the thread's own rows, which thread_read prints. Refused in one line when the thread is waiting on no prompt and when the prompt it is stopped on carries no such answer, which is what a call that asks the person something rather than for consent does.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, Some("the thread's id, or a prefix of it that names one, as threads lists them"))
                ],
                output: vec!["threadId", "askId", "optionId"],
                stream: vec![],
                call: call_thread_allow,
            },
        }),
        Verb::Cli(CliVerb {
            name: "thread deny",
            usage: "wsp thread deny <thread> [--reason \"<words>\"]".to_string(),
            about: "answers the prompt the thread is stopped on and refuses the call".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("reason", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_thread_deny,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Answers the prompt the thread is stopped on and refuses the call, by thread id or a prefix of it. A thread stopped on a prompt reads Needs you in threads and runs nothing until somebody picks, so this is how a thread you did not open is unstuck; the prompt itself is on the thread's own rows, which thread_read prints. Refused in one line when the thread is waiting on no prompt and when the prompt it is stopped on carries no such answer, which is what a call that asks the person something rather than for consent does.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, Some("the thread's id, or a prefix of it that names one, as threads lists them")),
                    input("reason", InputKind::String, true, Some("what the agent should do instead, in your words; the agent reads it with the refusal, as it reads the reason a person types in the app"))
                ],
                output: vec!["threadId", "askId", "optionId"],
                stream: vec![],
                call: call_thread_deny,
            },
        }),
    ];
    verbs.extend(super::slate::slate_verbs());
    verbs.extend(vec![
        Verb::Cli(CliVerb {
            name: "send",
            usage: "wsp send <thread> [--model, --effort <value>] [--fast] [--file <path>] [--detach] \"<message>\"".to_string(),
            about: "a message to the thread, on a named model or effort, fast or not, with files; the thread keeps its own access and a running turn its own picks; --detach prints the id and returns".to_string(),
            page: Page::Front,
            options: FlagTable(vec![("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("fast", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("file", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("detach", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
            run: run_send,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Sends a message to an existing thread (by id, or a prefix of it) and returns the reply when it is complete; a person's message on the same thread lands in order with yours. With detach true it returns the thread id the moment the turn is started, without the reply, and the turn's end reaches whoever the thread's start named. A model or effort named here is the turn's; the thread runs on the agent and at the access its own turns ran at, which a message does not change; a turn that joins a running one keeps that one's. On a thread whose turn is not running the message starts a new turn (outcome `started`); when the turn is still running the message joins it (outcome `steered`) or waits for it and then runs (outcome `queued`), and the reply is that turn's. When the thread's turn has replied but its agent process is still running, the message waits for that process and runs as the thread's next turn (outcome `queued`), which the runtime says as `This thread replied, still working; the message runs as its next turn once that process exits`. A send is never refused for meeting a turn, and two sends keep the order they arrived in. A start on a computer already running as many threads as its threads at once waits for a slot and starts on its own when one frees (outcome `held`), with capped beside it, the computer and what runs there against its number, which the thread's row in threads carries while it waits; a thread that follows a run or a send to its end, not detached, lends that thread's next turn its own slot, one turn at a time, a turn already held ahead of the message included, and every other start, a child started detached included, waits for a slot of its own.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, None),
                    input("message", InputKind::String, false, None),
                    input("model", InputKind::String, true, Some("the model the turn runs on, by the agent's own slug (claude-sonnet-5); absent on a new thread means the project's, else the one set for that agent, else the catalog's default, on send the thread's own")),
                    input("effort", InputKind::String, true, Some("the reasoning effort, by the agent's own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default, high for claude; on send, the thread's own")),
                    input("fast", InputKind::Boolean, true, Some("true runs the turn in the agent's fast mode, on a model that offers one, and is refused naming the model otherwise; absent runs at the agent's usual speed")),
                    input("files", InputKind::StringList { min: None }, true, Some("paths on this computer, absolute or relative to the folder wsp runs in, of files to send with the message, at most 5. An image (PNG, JPEG, GIF or WebP, up to 10 MB) goes to the agent as an image; any other file, up to 10 MB, lands in the folder the thread works in, under .wsp-files where git lists none of it, and the message names its path. The host reads each file and sends its bytes, so the machine never reaches back for this computer’s files; an image to an agent that reads none is refused naming that agent.")),
                    input("detach", InputKind::Boolean, true, Some("true answers with the thread id the moment the turn is started, without the reply, and the turn's end reaches whoever notify named; for a turn that runs for minutes or an hour, so this call does not block for it. A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report."))
                ],
                output: vec!["threadId", "workspaceId", "harness", "text", "outcome", "capped", "afterCut", "notice"],
                stream: vec![],
                call: call_send,
            },
        }),
        Verb::Cli(CliVerb {
            name: "restart",
            usage: "wsp restart".to_string(),
            about: "stops the host and brings it back on the road it came up on, its service, the verb that started it or the app; running turns go on and the host that comes back re-opens them. A host wsp up holds in a terminal refuses".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![]),
            run: run_restart,
            reads_here: None,
            starts_no_host: None,
            any_release: Some("the restart is the fix the release refusal names for an older host here, which comes back on the release installed"),
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Stops the host and brings it back on the road it came up on: its service's manager, the verb that started it, or the app. Running turns go on across it and the host that comes back re-opens them, your own turn included, so a coordinator thread lands a host change with this and keeps working. Answers once that host serves, with running the ids of the threads running on it then. A run, send or wait this cut dials the host again and carries on. Refused for a host wsp up holds in a terminal, which only that terminal brings back.".to_string(),
                input: vec![],
                output: vec!["running"],
                stream: vec![],
                call: call_restart,
            },
        }),
        Verb::Cli(CliVerb {
            name: "stop",
            usage: "wsp stop <thread> [--subagent <id>]".to_string(),
            about: "stops the thread's running turn, as the app's stop does, or with --subagent one of its agent's own subagents alone; the computer stays up".to_string(),
            page: Page::Front,
            options: FlagTable(vec![("subagent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_stop,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Stops the thread's running turn (by id, or a prefix of it), as the app's stop button does; the computer stays up and the thread takes the next send. outcome accepted means the turn ended interrupted; not-running means it had already ended, which is an answer, not an error. A thread whose agents spawned threads of their own stops as one: under names each of those that was running and was stopped with it. Given the id of one of the agent's own subagents (off threads' subagents), that one is stopped alone and the turn runs on: accepted means the agent took the stop, refused and unsupported carry the reason in error. On a computer the person added, left says what the thread started there that the stop could not end, or that the computer is not answering, in which case the turn reads stopped now and ends there once that computer connects again; a stop on a message waiting for that end gives the message up and says so in left.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, None),
                    input("subagent", InputKind::String, true, Some("one of the agent's own subagents, by the id threads lists it under, to stop alone"))
                ],
                output: vec!["threadId", "task", "outcome", "under", "error", "left"],
                stream: vec![],
                call: call_stop,
            },
        }),
        Verb::CliOnly(CliOnlyVerb {
            name: "ssh",
            usage: "wsp ssh <workspace>".to_string(),
            about: "carries one ssh connection to the workspace's own ssh server, starting it there, over this computer's stdin and stdout: the ProxyCommand wsp's ssh config gives every wsp- alias, so `ssh wsp-<name>` and an editor's remote window land inside the workspace".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![]),
            run: run_ssh,
            reads_here: None,
            starts_no_host: Some("a proxy for one connection to a host already up: a host it started would serve a state no config named, find no such workspace and stay up after ssh gave up"),
            any_release: None,
            cloud: true,
            cloud_flags: vec![],
            cli_only: "a proxy for an ssh client on the computer the host runs on, piping raw bytes; an agent has no ssh client there to hand it to",
            host_side: None,
        }),
        Verb::Cli(CliVerb {
            name: "exec",
            usage: "wsp exec <thread> [--cwd <dir>] -- <command...>".to_string(),
            about: "runs the command where the thread works, each word as given, in --cwd or the thread's own folder".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("cwd", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_exec,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "Runs a command on the computer the thread runs on as argv (each word as given; use sh -c for a shell line), in the folder cwd names or the folder the thread works in, and returns its output lines, exit code and the folder it ran in. A non-zero exit is a result; the computer going away is an error.".to_string(),
                input: vec![
                    input("thread", InputKind::String, false, Some("the thread, by its id or a prefix of it that names one, as threads lists them")),
                    input("argv", InputKind::StringList { min: Some(1) }, false, None),
                    input("cwd", InputKind::String, true, Some("the folder the command runs in, absolute; absent means the folder the thread works in"))
                ],
                output: vec!["exitCode", "output", "cwd"],
                stream: vec!["output"],
                call: call_exec,
            },
        }),
        Verb::Cli(CliVerb {
            name: "folders",
            usage: "wsp folders [<folder>] [--hidden] [--repos] [--on <computer>]".to_string(),
            about: "the folders inside one folder on this computer or on a box you added with --on, or with --repos every git repo under the home folder, most recently used first, for naming one to record".to_string(),
            page: Page::App,
            options: FlagTable(vec![("hidden", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("repos", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_folders,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "The folders directly inside one folder on the person's own computer, or on a box they added, one level at a time, as the app's import dialog browses them: each folder's absolute path, whether git tracks it, and how many hidden ones the level holds. Browse this to name a folder for import instead of guessing a path. The roots are that computer's home folder and the folder of every project on it; a path outside those is refused, and folder absent lists the home folder.<!-- cloud --> A cloud account keeps no computer to browse and is refused.<!-- /cloud --> Folders only: no file is named and nothing is read.".to_string(),
                input: vec![
                    input("folder", InputKind::String, true, Some("the folder to list, absolute and inside the roots; absent lists the home folder")),
                    input("hidden", InputKind::Boolean, true, Some("true lists the hidden folders too, which are otherwise only counted")),
                    input("repos", InputKind::Boolean, true, Some("true answers every git repo under the home folder and the recorded projects instead of one level, each with its branch and when git last wrote to it, most recent first; folder is ignored")),
                    input("on", InputKind::String, true, Some("the computer whose folders to list, by the name computers lists; absent is the computer the app runs on"))
                ],
                output: vec!["dir", "roots", "folders", "hidden"],
                stream: vec![],
                call: call_folders,
            },
        }),
        Verb::Cli(CliVerb {
            name: "setup",
            usage: "wsp setup".to_string(),
            about: "the setup on this host as the app's Settings reads it: which keys are held (never their values)<!-- cloud --> and what a machine costs<!-- /cloud -->".to_string(),
            page: Page::App,
            options: FlagTable(vec![]),
            run: run_setup,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "The setup on this host, as the app's Settings reads it: which keys the host holds (their presence, never a value), this computer's home folder<!-- cloud -->, what a machine costs and the place a build would boot on, or why no build can start here<!-- /cloud -->. Read it to tell the person which provider key is missing.".to_string(),
                input: vec![],
                output: vec!["setup"],
                stream: vec![],
                call: call_setup,
            },
        }),
        Verb::Cli(CliVerb {
            name: "terminal config",
            usage: "wsp terminal config [--scheme light|dark]".to_string(),
            about: "the Ghostty config on this computer as the app's terminal pane applies it, read from ~/.config/ghostty and Application Support with its includes and theme resolved: the font and its fallbacks, the size, the colors, the cursor, the padding, the background opacity, and the blur, which is read but not applied; --scheme picks the side of a light:...,dark:... theme".to_string(),
            page: Page::App,
            options: FlagTable(vec![("scheme", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_terminal_config,
            reads_here: Some("the Ghostty config it reads is the person's own, in their home on this computer"),
            starts_no_host: None,
            any_release: None,
            cloud: false,
            cloud_flags: vec![],
            tool: Tool {
                description: "The person's Ghostty config on this computer as the app's terminal pane applies it: every config file Ghostty would load and the theme it names, read now, with only the keys the pane honours. files is empty when they have no Ghostty config, and each other key is absent when no file sets it, so the pane keeps its default there. backgroundBlur is read and not applied: the desktop window's own material is the blur, and a browser tab has none. Read this to say how their terminal looks or to check what a config change did; nothing is written.".to_string(),
                input: vec![
                    input("scheme", InputKind::Enum(vec!["light", "dark"]), true, Some("which side of a light:...,dark:... theme to resolve; dark when absent"))
                ],
                output: vec!["files", "fontFamily", "fontSize", "theme", "background", "foreground", "palette", "selectionBackground", "cursorColor", "cursorStyle", "cursorStyleBlink", "windowPaddingX", "windowPaddingY", "backgroundOpacity", "backgroundBlur"],
                stream: vec![],
                call: call_terminal_config,
            },
        }),
        Verb::Cli(CliVerb {
            name: "export",
            usage: "wsp export <workspace> <folder> [--from <path on the machine>] [--replace] [--agents <ids>]".to_string(),
            about: "brings a project folder and the agent sessions keyed to it home from the machine".to_string(),
            page: Page::Agent,
            options: FlagTable(vec![("from", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("replace", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("agents", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
            run: run_export,
            reads_here: None,
            starts_no_host: None,
            any_release: None,
            cloud: true,
            cloud_flags: vec![],
            tool: Tool {
                description: "Brings a project folder and the agent sessions keyed to it home from the workspace's machine to this computer: the folder lands at `folder` (absolute, must not exist unless replace), the sessions in the agents' homes here keyed to it. `from` is the folder's path on the machine, the same path as `folder` when absent. The result says per agent what moved, what landed as transcripts only, and how many indexed rollouts were skipped.".to_string(),
                input: vec![
                    input("workspace", InputKind::String, false, Some("the workspace's name, or its id when two share a name")),
                    input("folder", InputKind::String, false, Some("where the folder lands on this computer, absolute")),
                    input("from", InputKind::String, true, Some("the folder's path on the machine; defaults to folder")),
                    input("replace", InputKind::Boolean, true, Some("remove what is at folder first; without it an existing folder is refused")),
                    input("agents", InputKind::StringList { min: None }, true, Some("catalog ids of the agents whose sessions come home; absent means every agent with sessions for the folder"))
                ],
                output: vec!["dest", "files", "bytes", "excluded", "agents"],
                stream: vec![],
                call: call_export,
            },
        }),
    ]);
    verbs
}

/// What `run` asks of a start, read the same off the command line and the tool.
struct RunAsk {
    message: String,
    project: Option<String>,
    beside: Option<String>,
    fork: Option<String>,
    at: Option<AtWord>,
    branch: Option<String>,
    cwd: Option<String>,
    replaces: Option<String>,
    harness: Option<String>,
    picks: Picks,
    notify: Vec<String>,
    title: Option<String>,
    files: Vec<String>,
}

/// What `prepare_run` leaves for the turn: the stream's opener, the machine woken for it and the start's frame.
struct Prepared {
    opened: Option<Opened>,
    woken: Option<Woken>,
    opening: Frame,
}

/// `subStart`'s request (threads.ts:88-95): the sub-thread's ask with the picks, notify, title and files of the start.
struct SubReq {
    message: String,
    on: String,
    size: Option<String>,
    from: Option<String>,
    branch: Option<String>,
    project: Option<String>,
    harness: Option<String>,
    picks: Picks,
    notify: Option<Vec<String>>,
    title: Option<String>,
    files: Vec<String>,
    elsewhere: bool,
}

/// What `sub_start` answers (threads.ts:101): the start's frame, the sub-thread's answer, the line that names the thread
/// and the stop of the machine's listening.
struct SubRun {
    opening: Frame,
    sub: SubOut,
    opened: Opened,
    stop: Box<dyn Fn() + Send + Sync>,
}

fn json_of<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn frame(pairs: &[(&str, Value)]) -> Frame {
    pairs.iter().map(|(key, value)| ((*key).to_owned(), value.clone())).collect()
}

fn frame_str(args: &Frame, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn frame_bool(args: &Frame, key: &str) -> bool {
    args.get(key) == Some(&Value::Bool(true))
}

fn frame_bool_opt(args: &Frame, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

fn frame_list(args: &Frame, key: &str) -> Vec<String> {
    frame_list_opt(args, key).unwrap_or_default()
}

fn frame_list_opt(args: &Frame, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
}

fn frame_value(args: &Frame, key: &str) -> Option<Value> {
    args.get(key).cloned()
}

fn frame_picks(args: &Frame) -> Picks {
    Picks {
        model: frame_str(args, "model"),
        effort: frame_str(args, "effort"),
        access: frame_str(args, "access"),
        fast: frame_bool(args, "fast"),
    }
}

fn frame_at(args: &Frame) -> Option<AtWord> {
    match args.get("at") {
        Some(Value::Number(n)) => n.as_f64().map(AtWord::Count),
        _ => None,
    }
}

fn flag_value(ctx: &VerbContext, name: &str) -> Option<Value> {
    match ctx.flags.get(name)? {
        FlagValue::Str(text) => Some(json!(text)),
        FlagValue::Bool(flag) => Some(json!(flag)),
        FlagValue::List(items) => Some(json!(items)),
    }
}

fn picks_from(ctx: &VerbContext) -> Picks {
    pick_flags(&ctx.flags)
}

/// The folder a relative path is taken to be in (`path.resolve`), the caller's own where it has one.
fn here_cwd(cwd: Option<&str>) -> String {
    match cwd {
        Some(cwd) => cwd.to_owned(),
        None => std::env::current_dir().map(|dir| dir.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".to_owned()),
    }
}

/// `view_of` for a workspace the host answered as an `WorkspaceOut`: awake reads the same record as its view.
fn view_of(workspace: &WorkspaceOut) -> Result<WorkspaceView, HostError> {
    let value = serde_json::to_value(workspace).map_err(|e| HostError::new(e.to_string()))?;
    serde_json::from_value(value).map_err(|e| HostError::new(e.to_string()))
}

/// A thread's read as the tool and the line answer it: its id and its messages.
fn read_json(read: &ThreadRead) -> Value {
    json!({ "threadId": read.thread_id, "messages": json_of(&read.messages) })
}

fn local_offset(at: f64) -> i64 {
    DateZone::system().offset_seconds(at as i64)
}

fn spend_of(woken: Option<&Woken>) -> Option<String> {
    match woken {
        Some(woke) => turn_spend_word(Some(woke.workspace.kind.as_ref())),
        None => turn_spend_word(Some(Some(&WorkspaceKind::Local))),
    }
}

fn with_sub(view: TurnOut, sub: &SubOut) -> Value {
    let mut map = match json_of(&view) {
        Value::Object(map) => map,
        _ => Map::new(),
    };
    map.insert("sub".to_owned(), json_of(sub));
    Value::Object(map)
}

fn noticed_value(view: TurnOut, said: Option<&str>) -> Value {
    json_of(&noticed(view, said))
}

/// `() => hostBack(deps)`: the dial a turn makes again after the host let its socket go.
fn redial_of(deps: &VerbDeps) -> impl Fn() -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> + Send + Sync {
    let owned = deps.clone();
    move || {
        let owned = owned.clone();
        Box::pin(async move { host_back(&owned, None).await }) as BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>>
    }
}

/// `subStart` (threads.ts:88-102): the picks checked, the files read, the machine made under its listener, and the
/// opening that starts the thread in its folder there.
async fn sub_start(
    client: &dyn HostClient,
    env: &IndexMap<String, String>,
    req: &SubReq,
    say: Arc<dyn Fn(&str) + Send + Sync>,
) -> Result<SubRun, HostError> {
    checked_start(client, &req.message, req.harness.as_deref(), &req.picks, None, None).await?;
    files_from(&req.files, req.elsewhere)?;
    let ask = SubAsk {
        on: req.on.clone(),
        size: req.size.clone(),
        from: req.from.clone(),
        branch: req.branch.clone(),
        harness: req.harness.clone(),
        project: req.project.clone(),
    };
    let SubMade { workspace, sub, stop } = sub_for(client, &ask, &req.message, req.title.as_deref(), say).await?;
    let opening = opening_of(
        env,
        &OpeningAt::Workspace { id: workspace.id.clone() },
        &sub_opening(&sub.preamble, &req.message),
        &OpeningOpts {
            picks: req.picks.clone(),
            harness: Some(sub.harness.clone()),
            notify: req.notify.clone(),
            title: Some(workspace.name.clone()),
            files: req.files.clone(),
            elsewhere: req.elsewhere,
            cwd: Some(sub.folder.clone()),
            ..OpeningOpts::default()
        },
    )?;
    let on = sub.out.on.clone();
    let folder = sub.folder.clone();
    let opened: Opened = Arc::new(move |thread_id: &str, _folder: Option<&str>| sub_opened_line(thread_id, &on, &folder));
    Ok(SubRun { opening, sub: sub.out, opened, stop })
}

/// The start `run` makes from its words, after the refusals that come before any read (threads.ts:145-156): a fork, or
/// the thread beside, or the project's folder, checked, then the machine woken for it and its opening.
async fn prepare_run(
    client: &dyn HostClient,
    env: &IndexMap<String, String>,
    caller_cwd: Option<&str>,
    elsewhere: bool,
    ask: &RunAsk,
    resume: Option<ResumeAsk>,
    say: &(dyn Fn(&str) + Send + Sync),
) -> Result<Prepared, HostError> {
    if let Some(fork) = &ask.fork {
        let forked = thread_fork(
            client,
            fork,
            ask.at.as_ref(),
            ask.branch.as_deref(),
            &ask.message,
            ask.harness.as_deref(),
            &ask.picks,
            crate::verbs::cloud_of(env),
        )
        .await?;
        let notify = notify_of(client, &ask.notify).await?;
        let opening = opening_of(
            env,
            &OpeningAt::Forks(forked),
            &ask.message,
            &OpeningOpts {
                picks: ask.picks.clone(),
                harness: ask.harness.clone(),
                notify,
                title: ask.title.clone(),
                files: ask.files.clone(),
                elsewhere,
                ..OpeningOpts::default()
            },
        )?;
        return Ok(Prepared { opened: None, woken: None, opening });
    }
    let named = match &ask.beside {
        Some(beside) => RunTarget::Workspace {
            workspace: thread_at_in(client, beside, "wsp run --beside", false, cloud_of(env)).await?.1,
            opened: None,
        },
        None => {
            let wher = RunWhere { branch: ask.branch.clone(), cwd: ask.cwd.clone() };
            run_target(client, ask.project.as_deref(), caller_cwd, env, Some(elsewhere), &wher, crate::verbs::cloud_of(env)).await?
        }
    };
    let (workspace_id, fork_of) = match &named {
        RunTarget::Workspace { workspace, .. } => (Some(workspace.id.clone()), None),
        RunTarget::Fork(fork) => (None, Some(fork.fork.id.clone())),
        RunTarget::Folder(_) => (None, None),
    };
    checked_start(client, &ask.message, ask.harness.as_deref(), &ask.picks, workspace_id.as_deref(), fork_of.as_deref()).await?;
    if resume.is_some() && matches!(named, RunTarget::Fork(_)) {
        return Err(usage_refusal(RESUME_HERE_LINE, RESUME_HERE_FIX));
    }
    let notify = notify_of(client, &ask.notify).await?;
    let replaces = replaced_of(client, ask.replaces.as_deref()).await?;
    let (at, cwd) = match &named {
        RunTarget::Workspace { workspace, .. } => (OpeningAt::Workspace { id: workspace.id.clone() }, ask.cwd.clone()),
        RunTarget::Folder(folder) => (OpeningAt::Here(folder.here.clone()), None),
        RunTarget::Fork(fork) => (OpeningAt::Fork(fork.clone()), ask.cwd.clone()),
    };
    let read = opening_of(
        env,
        &at,
        &ask.message,
        &OpeningOpts {
            picks: ask.picks.clone(),
            harness: ask.harness.clone(),
            notify,
            title: ask.title.clone(),
            replaces,
            files: ask.files.clone(),
            elsewhere,
            cwd,
            resume,
        },
    )?;
    let (opened, target) = match named {
        RunTarget::Folder(folder) => (Some(folder.opened), None),
        RunTarget::Workspace { workspace, opened } => (opened, Some(workspace)),
        RunTarget::Fork(fork) => (None, Some(fork_for(client, &fork.fork, &ask.message, say).await?)),
    };
    let woken = match target {
        Some(workspace) => Some(awake(client, &view_of(&workspace)?, "send", say).await?),
        None => None,
    };
    let opening = match &woken {
        Some(woke) => {
            let mut read = read;
            read.insert("workspaceId".to_owned(), json!(woke.workspace.id));
            read
        }
        None => read,
    };
    Ok(Prepared { opened, woken, opening })
}

/// `run`'s refusals that come before any read, shared by the line and the tool (threads.ts:137-144); the beside one is
/// named with the usage on the line and with its own fix on the tool.
fn run_refusals(ask: &RunAsk, resume_copy: bool, resume: Option<&str>, replaced: bool, beside_fix: &str) -> Result<(), HostError> {
    if ask.beside.is_some() && (ask.project.is_some() || ask.branch.is_some()) {
        return Err(usage_refusal(BESIDE_ALONE_LINE, beside_fix));
    }
    if ask.at.is_some() && ask.fork.is_none() {
        return Err(usage_refusal(AT_WITHOUT_FORK_LINE, AT_WITHOUT_FORK_FIX));
    }
    if ask.fork.is_some() && (ask.project.is_some() || ask.beside.is_some() || ask.cwd.is_some() || replaced) {
        return Err(usage_refusal(FORK_BESIDE_LINE, FORK_BESIDE_FIX));
    }
    if ask.fork.is_some() && (resume.is_some() || resume_copy) {
        return Err(usage_refusal(FORK_RESUME_LINE, FORK_RESUME_FIX));
    }
    Ok(())
}

/// The answer a turn's end gives where it was followed to its end: the detached line and the turn's own view.
async fn nap_line(client: &dyn HostClient, woke: &Woken, started: &Mutex<Option<SharedTurn>>) -> Option<String> {
    let turn = started.lock().unwrap().clone().map(|turn| turn.lock().unwrap().clone());
    nap_after_dead_launch(client, woke, turn.as_ref()).await
}

mod rest;
mod run;

use self::rest::*;
use self::run::*;
