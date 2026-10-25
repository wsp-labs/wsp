// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/threads.ts` at 65f6b89c7: the thread verbs, their command lines and their tools.
//! Spec: `spec/threads-a.md` (run to the answer verbs, threads.ts:1-448) and `spec/threads-b.md` (send to export,
//! threads.ts:449-723). The table's words are copied by script from the TS module (tests/fixtures/thread_verbs.json);
//! every run and call body is the implementer's.

use wsp_words::exit::HostError;

use super::activity;
use super::client::{
    BoxFuture, CallToolResult, CliOnlyVerb, CliVerb, FlagKind, FlagRow, FlagTable, Frame, InputKind, Page, Tool, ToolInput, Verb, VerbContext, VerbDeps,
};
use super::io;

fn input(name: &'static str, kind: InputKind, optional: bool, describe: Option<&str>) -> ToolInput {
    ToolInput { name, kind, optional, describe: describe.map(str::to_string) }
}

/// The first flag beside `--on` a sub-thread takes none of, as the line or the tool named it: the first pair whose
/// value is given and not `false` (`besideOn`, threads.ts:84).
pub fn beside_on(given: &[(&str, Option<serde_json::Value>)]) -> Option<String> {
    let _ = given;
    todo!("threads.ts:84")
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
                input: vec![
                    
                ],
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
                input: vec![
                    
                ],
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

/// `run`'s run (threads.ts:112).
fn run_run(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:112")
    })
}

/// `run`'s tool call (threads.ts:171).
fn call_run(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:171")
    })
}

/// `worktree`'s run (threads.ts:225).
fn run_worktree(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:225")
    })
}

/// `worktree`'s tool call (threads.ts:237).
fn call_worktree(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:237")
    })
}

/// `worktree remove`'s run (threads.ts:249).
fn run_worktree_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:249")
    })
}

/// `worktree remove`'s tool call (threads.ts:266).
fn call_worktree_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:266")
    })
}

/// `thread read`'s run (threads.ts:279).
fn run_thread_read(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:279")
    })
}

/// `thread read`'s tool call (threads.ts:296).
fn call_thread_read(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:296")
    })
}

/// `thread head`'s run (threads.ts:310).
fn run_thread_head(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:310")
    })
}

/// `thread head`'s tool call (threads.ts:323).
fn call_thread_head(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:323")
    })
}

/// `thread rename`'s run (threads.ts:336).
fn run_thread_rename(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:336")
    })
}

/// `thread rename`'s tool call (threads.ts:351).
fn call_thread_rename(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:351")
    })
}

/// `thread forget`'s run (threads.ts:366).
fn run_thread_forget(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:366")
    })
}

/// `thread forget`'s tool call (threads.ts:382).
fn call_thread_forget(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:382")
    })
}

/// `thread settle`'s run (threads.ts:396).
fn run_thread_settle(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:396")
    })
}

/// `thread settle`'s tool call (threads.ts:415).
fn call_thread_settle(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:415")
    })
}

/// `thread restore`'s run (threads.ts:429).
fn run_thread_restore(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:429")
    })
}

/// `thread restore`'s tool call (threads.ts:441).
fn call_thread_restore(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:441")
    })
}

/// `thread allow`'s run (threads.ts:see ANSWER_VERBS, threads.ts:62-68).
fn run_thread_allow(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:62-68")
    })
}

/// `thread allow`'s tool call (threads.ts:76-79).
fn call_thread_allow(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:76-79")
    })
}

/// `thread deny`'s run (threads.ts:see ANSWER_VERBS, threads.ts:62-68).
fn run_thread_deny(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:62-68")
    })
}

/// `thread deny`'s tool call (threads.ts:76-79).
fn call_thread_deny(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:76-79")
    })
}

/// `send`'s run (threads.ts:456).
fn run_send(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:456")
    })
}

/// `send`'s tool call (threads.ts:475).
fn call_send(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:475")
    })
}

/// `restart`'s run (threads.ts:495).
fn run_restart(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:495")
    })
}

/// `restart`'s tool call (threads.ts:506).
fn call_restart(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:506")
    })
}

/// `stop`'s run (threads.ts:518).
fn run_stop(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:518")
    })
}

/// `stop`'s tool call (threads.ts:529).
fn call_stop(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:529")
    })
}

/// `ssh`'s run (threads.ts:544).
fn run_ssh(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:544")
    })
}

/// `exec`'s run (threads.ts:562).
fn run_exec(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:562")
    })
}

/// `exec`'s tool call (threads.ts:583).
fn call_exec(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:583")
    })
}

/// `folders`'s run (threads.ts:604).
fn run_folders(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:604")
    })
}

/// `folders`'s tool call (threads.ts:622).
fn call_folders(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:622")
    })
}

/// `setup`'s run (threads.ts:634).
fn run_setup(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:634")
    })
}

/// `setup`'s tool call (threads.ts:645).
fn call_setup(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:645")
    })
}

/// `terminal config`'s run (threads.ts:656).
fn run_terminal_config(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:656")
    })
}

/// `terminal config`'s tool call (threads.ts:667).
fn call_terminal_config(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:667")
    })
}

/// `export`'s run (threads.ts:680).
fn run_export(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("threads.ts:680")
    })
}

/// `export`'s tool call (threads.ts:711).
fn call_export(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("threads.ts:711")
    })
}
