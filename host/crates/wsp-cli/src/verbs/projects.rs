// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/projects.ts` at 65f6b89c7: `wsp projects` and its subcommands, `wsp threads` and
//! `wsp threads wait`, and the tool-only `projects_add`. Spec: `spec/projects.md`.

use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::workspace::{ProjectSeed, ProjectView};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::words::computer::Platform;
use wsp_words::words::turn::NotifyLength;

use super::agents_help::{
    after_worktree_line, defaults_cell, draw_rows, new_threads_head_line, project_defaults_set, thread_defaults_lines, trackers_line,
    ProjectSetAsk, Rows,
};
use super::client::*;
use super::io::{as_json, as_text, confirmed, timeout_flag, wait_answer};
use super::turns_help::{project_defaults_of, wait_through, Waited};
use super::workspaces_help::{
    disk_gone_lines, place_names, project_listed, project_row_line, projects_listed, shown, thread_lines, thread_rows, thread_tree,
    threads, threads_of, workspaces, NO_PROJECT_YET, THREAD_HEAD,
};

/// `projectSetLines` (projects.ts:24-29): what a new thread on the project starts on, then the after-worktree
/// command and the trackers where the answer carries them.
pub fn project_set_lines(
    project_name: &str,
    defaults: &ThreadDefaults,
    after_worktree: Option<&str>,
    trackers: Option<&[String]>,
) -> Vec<String> {
    let mut lines = vec![new_threads_head_line(project_name)];
    lines.extend(thread_defaults_lines(defaults));
    if let Some(command) = after_worktree {
        lines.push(after_worktree_line(command));
    }
    if let Some(list) = trackers {
        lines.push(trackers_line(list));
    }
    lines
}

/// The picks a seed line asks for (projects.ts:32).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SeedAsk {
    pub keep: Vec<String>,
    pub cut: Vec<String>,
    pub none: bool,
    pub save: bool,
}

/// `projectSeed` (projects.ts:32-41): `projects.seed` with `project`, then `keep`, `cut`, `none: true` and
/// `save: true` only where given, in that order, the reply parsed as `ProjectSeed`.
pub async fn project_seed(client: &dyn HostClient, project: &str, ask: &SeedAsk) -> Result<ProjectSeed, HostError> {
    let mut params = Frame::new();
    params.insert("project".to_owned(), Value::from(project));
    if !ask.keep.is_empty() {
        params.insert("keep".to_owned(), json!(ask.keep));
    }
    if !ask.cut.is_empty() {
        params.insert("cut".to_owned(), json!(ask.cut));
    }
    if ask.none {
        params.insert("none".to_owned(), Value::Bool(true));
    }
    if ask.save {
        params.insert("save".to_owned(), Value::Bool(true));
    }
    let reply = client.request("projects.seed", params).await?;
    crate::zod_read::zod_read(&Value::Object(reply))
}

/// `projectSeedText` (projects.ts:44-47): the head, a blank line, the menu indented two spaces and a blank line
/// where it has rows, then the foot.
pub fn project_seed_text(seed: &ProjectSeed) -> String {
    let lines = wsp_words::project_seed::project_seed_lines(seed);
    let mut out = vec![lines.head, String::new()];
    if !lines.rows.is_empty() {
        out.extend(table(&lines.rows).into_iter().map(|line| format!("  {line}")));
        out.push(String::new());
    }
    out.extend(lines.foot);
    out.join("\n")
}

/// The host platform word as the sentences name a computer by (`hostPlatform`).
fn platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Darwin
    } else {
        Platform::Linux
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

/// The home folder `os.homedir()` names, from `HOME`.
fn home() -> Option<String> {
    crate::hosts::env_var("HOME").filter(|home| !home.is_empty())
}

fn json_of<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a view writes as JSON")
}

/// A string key of a row the host wrote, empty where it is not there.
fn row_text<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn arg_str<'a>(args: &'a Frame, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

/// A list argument as its strings, where the frame carries one.
fn arg_list(args: &Frame, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
}

/// The reply's field read into the type a verb takes; a field the host left out or shaped otherwise is the other-version
/// refusal.
fn field<T: serde::de::DeserializeOwned>(reply: &Frame, op: &str, key: &str) -> Result<T, HostError> {
    reply.get(key).cloned().and_then(|value| serde_json::from_value(value).ok()).ok_or_else(|| HostError::new(other_version(op)))
}

/// The `threads` rows as the listing draws them: the table of the thread head and each thread's line, then the disk lines.
async fn thread_rows_frame(client: &dyn HostClient, within: Option<&str>, tree: bool) -> Result<Rows, HostError> {
    let rows = thread_rows(client, within).await?;
    let (now, home) = (now_ms(), home());
    let mut lines: Vec<Vec<String>> = vec![THREAD_HEAD.iter().map(|head| (*head).to_owned()).collect()];
    if tree {
        for entry in thread_tree(&rows) {
            lines.extend(thread_lines(entry.row, &"  ".repeat(entry.depth), home.as_deref(), now));
        }
    } else {
        for row in &rows {
            lines.extend(thread_lines(row, "", home.as_deref(), now));
        }
    }
    let mut out = table(&lines);
    out.extend(disk_gone_lines(&rows));
    Ok(Rows { value: json!({ "threads": json_of(&rows) }), rows: out })
}

/// The entries of `verbs/projects.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn project_verbs() -> Vec<Verb> {
    vec![
    // verbs/projects.ts:51
    Verb::Cli(CliVerb {
        name: "projects",
        usage: "wsp projects".to_string(),
        about: "your projects, each on its computer: where its code comes from, where its folder sits, the branch a machine of it starts on and how many threads it has".to_string(),
        page: Page::Front,
        options: FlagTable(vec![]),
        run: run_projects,
        tool: Tool {
            description: "Every project this host holds: its name, the computer it lives on, where its code comes from (a folder on the computer the app runs on, or a repo a computer clones), where its folder sits, the branch a machine of it starts on, and its repo's top folder where it is a folder here inside a git repo. The name is what run takes. A project is recorded with add and is one source on one computer; on the computer the app runs on its threads run in its folder, or in a worktree of its repo for another branch. defaults holds, by project id, the agent, model, effort and access a new thread there starts on when its start names none, each with where it came from: the project's own (projects_set), the person's default (agents_set, agents_default) or the agent's own.".to_string(),
            input: vec![],
            output: vec!["projects", "defaults"],
            stream: vec![],
            call: call_projects,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/projects.ts:81
    Verb::ToolOnly(ToolOnlyVerb {
        name: "projects add",
        tool: Tool {
            description: "Records a project: one source on one computer. A folder on the computer the app runs on is the project as it stands, a git repo or not, a folder inside a repo being a project of that repo; its threads run in it. A repo is cloned into a folder of its own name inside the folder named with into on the computer the app runs on, made where it does not exist yet, or into that folder itself where its name is the repo's or the repo's with a number, and the clone's folder is then a folder project there, or by the computer named with on, whose machines then hold a checkout of it. A repo with neither into nor on, a clone's folder that holds files, and a source already recorded on that computer are each refused in one line, and a clone that fails says git's own last line. The answer is the project, whose name is what run takes.".to_string(),
            input: vec![
                ToolInput { name: "source", kind: InputKind::String, optional: false, describe: Some("a folder on the computer the app runs on, or a repo's url".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer that clones the repo, by the name computers lists; a folder, and a repo cloned with into, take none".to_string()) },
                ToolInput { name: "into", kind: InputKind::String, optional: true, describe: Some("an absolute path on the computer the app runs on to clone the repo in: into a folder of the repo's name inside it, made where it does not exist yet, or into the path itself where its name is the repo's or the repo's with a number; the project is then the clone's folder".to_string()) },
                ToolInput { name: "name", kind: InputKind::String, optional: true, describe: Some("what to call the project here; the folder's or the repo's own last word without it".to_string()) },
                ToolInput { name: "base", kind: InputKind::String, optional: true, describe: Some("the branch a machine of the project starts on; the remote's own default branch at the clone without it".to_string()) },
            ],
            output: vec!["project", "notice"],
            stream: vec![],
            call: call_projects_add,
        },
        tool_only: "the command line records a project with wsp add, the same word that joins a computer and takes a provider's key; those two belong at the terminal the host runs at, so the tool door carries the project half alone",
        reads_here: None,
    }),
    // verbs/projects.ts:113
    Verb::Cli(CliVerb {
        name: "projects remove",
        usage: "wsp projects remove <project> [--yes] [--force]".to_string(),
        about: "takes a project out of this wsp; its folder is left where it is, on this computer or on a computer you joined, a project with a machine standing on it is refused naming them, and one whose checkout on a computer of yours holds work no remote has is stopped naming it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("force", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_projects_remove,
        tool: Tool {
            description: "Takes a project's record out of this wsp. Its folder stays exactly where it is, on this computer or on a computer you joined, the memory its agent keeps on that computer stays, and no repo is ever asked for anything. Refused in one line while a machine of it stands, naming them; delete those first. Refused in one line too while its checkout on a computer of yours holds work no remote has, naming it by its path there, unless force.".to_string(),
            input: vec![
                ToolInput { name: "project", kind: InputKind::String, optional: false, describe: Some("the project's name, or its id when two share a name".to_string()) },
                ToolInput { name: "force", kind: InputKind::Boolean, optional: true, describe: Some("remove it even where its checkout on a computer of yours holds work no remote has".to_string()) },
            ],
            output: vec!["project", "said"],
            stream: vec![],
            call: call_projects_remove,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/projects.ts:150
    Verb::Cli(CliVerb {
        name: "projects set",
        usage: "wsp projects set <project> [--agent <id>] [--model <slug>] [--effort <word>] [--access <word>] [--after-worktree <command>] [--tracker <owner/name>]... [--reset <field>]...".to_string(),
        about: "what a new thread on one project starts on, over the agent's own defaults: its agent, model, effort and access; the shell line a new worktree of it runs once, after each ecosystem's own install, for what none of them knows; and the repos besides its own that gh on a sub-thread's machine may comment on, the whole list each time".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("access", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("after-worktree", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("tracker", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("reset", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None })]),
        run: run_projects_set,
        tool: Tool {
            description: "Sets what a new thread on one project starts on, over each agent's own defaults and under what a start names: its agent, model, effort and access. Nothing about a computer is a project's to set (threads at once, an agent's program, config folder, launch words, variables, or whether it is on); agents_setup sets those per computer. A model or effort kept for the project's agent drops to the agent's own on a thread that runs another agent, and an access word the project's agent maps to none of its modes is refused naming the ones it takes. after_worktree is a shell line a new worktree of the project runs once at its top, after the locked install of each ecosystem whose lockfile it holds, each run in the folder holding that lockfile (uv sync --locked, cargo fetch --locked, go list -mod=readonly -deps -test ./..., npm install --no-save, pnpm install --frozen-lockfile where the node_modules carried in were installed from another lockfile, and the like), for what none of them knows. trackers is the whole list of repos besides the project's own, each owner/name, that gh on a sub-thread's machine may comment on, issues and pull requests both; every other gh write there stays refused. Answers what a new thread there now starts on, each value with where it came from, the after-worktree command and the trackers. reset puts a field back on the layer below: agent, model, effort or access, after-worktree takes the command away and trackers empties the list.".to_string(),
            input: vec![
                ToolInput { name: "project", kind: InputKind::String, optional: false, describe: Some("the project's name, or its id when two share a name".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: true, describe: Some("the agent a new thread on it runs, one of claude, codex, opencode, cursor".to_string()) },
                ToolInput { name: "model", kind: InputKind::String, optional: true, describe: Some("the model a new thread on it starts on, by the agent's own slug".to_string()) },
                ToolInput { name: "effort", kind: InputKind::String, optional: true, describe: Some("the effort a new thread on it starts at, by the agent's own word".to_string()) },
                ToolInput { name: "access", kind: InputKind::String, optional: true, describe: Some("how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full".to_string()) },
                ToolInput { name: "after_worktree", kind: InputKind::String, optional: true, describe: Some("a shell line a new worktree of the project runs once at its top, after each ecosystem's own install".to_string()) },
                ToolInput { name: "trackers", kind: InputKind::StringList { min: None }, optional: true, describe: Some("the whole list of repos, each owner/name, besides the project's own that gh on a sub-thread's machine may comment on".to_string()) },
                ToolInput { name: "reset", kind: InputKind::Json, optional: true, describe: Some("fields to put back on the layer below: agent, model, effort, access, after-worktree to take the command away, or trackers to empty the list".to_string()) },
            ],
            output: vec!["project", "defaults", "afterWorktree", "trackers"],
            stream: vec![],
            call: call_projects_set,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/projects.ts:191
    Verb::Cli(CliVerb {
        name: "projects seed",
        usage: "wsp projects seed <project> [--keep <path>]... [--cut <path>]... [--none] [--yes]".to_string(),
        about: "what git ignores in a project's folder on this computer, and which of it goes to a sub-thread's machine: the menu with each path's size and why, the picks marked; --yes saves the picks, which every later sub-thread of the project takes".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("keep", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("cut", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("none", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_projects_seed,
        tool: Tool {
            description: "What git ignores in a project's folder on this computer, and which of it goes to a sub-thread's machine: every path with its size, its kind (config, rebuilt, data, never for a login, unknown), the words its row ends in and whether it is picked. The picks start from the ones saved for the folder, else from nothing, since nothing git ignores is sent until the person saves picks; none empties them, keep adds paths off the menu and cut takes them away, each relative to the folder. A path git tracks or that is not there, the repository's own .git and a login are refused by name; a saved path no longer in the folder stays saved and is named in picked.missing. confirm saves the picks, which every later sub-thread of the project takes, as the same record wsp add --remember keeps; without it the menu is answered and nothing is saved. A thread's own token reads the menu and is refused the save, with the line to hand the person. A project whose folder is on another computer is refused.".to_string(),
            input: vec![
                ToolInput { name: "project", kind: InputKind::String, optional: false, describe: Some("the project, by name or id; its folder must be on this computer".to_string()) },
                ToolInput { name: "keep", kind: InputKind::StringList { min: None }, optional: true, describe: Some("paths off the menu to send, each relative to the project's folder".to_string()) },
                ToolInput { name: "cut", kind: InputKind::StringList { min: None }, optional: true, describe: Some("paths off the menu to keep on this computer".to_string()) },
                ToolInput { name: "none", kind: InputKind::Boolean, optional: true, describe: Some("send nothing git ignores".to_string()) },
                ToolInput { name: "confirm", kind: InputKind::Boolean, optional: true, describe: Some("true saves the picks; without it the menu is answered and nothing is saved".to_string()) },
            ],
            output: vec!["project", "folder", "rows", "picked", "saved"],
            stream: vec![],
            call: call_projects_seed,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/projects.ts:218
    Verb::Cli(CliVerb {
        name: "threads",
        usage: "wsp threads [<project>] [--tree] [--watch]".to_string(),
        about: "who is working, where and on which computer: every thread as the sidebar lists it, with its project, the folder it works in, that folder's branch, the agent, the state and who opened it, and under each thread the agent's own subagents with their SUBAGENT id; --tree indents the threads an agent spawned under the one that spawned them, and --watch draws the same table again every second where it stands".to_string(),
        page: Page::Front,
        options: FlagTable(vec![("tree", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("watch", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_threads,
        tool: Tool {
            description: "Every thread as the sidebar lists it: its project, the folder it works in (the project folder, or a worktree of the project's repo), that folder's branch as git reads it now, the computer it runs on, the agent inside, its state, who opened it (person, cli or agent) and its title. claudeSessionId is the agent's own id for the thread's session, absent until the agent announced one; for a Claude Code thread on this computer, cd into folder and run claude --resume with it to go on with the thread in a terminal, once the thread is not running; a thread on another computer goes on in a terminal there, its line also setting the CLAUDE_CONFIG_DIR its turns run with, as the app's Continue in terminal copies it. capped is on a thread waiting for a slot on its computer: the computer and what runs there against its threads at once; it starts on its own when one frees. Optionally within one project. A thread an agent inside another thread opened carries parentThreadId and rootThreadId, which is the tree stop ends as one. lastLine is the last line of the latest turn's reply and failure why that turn failed. subagents lists the agent's own subagents of every turn (id, title, state running, done, failed or stopped, model, asked: the start of what it was asked, lastLine or failure once it ends); stop takes one of them alone by its id.".to_string(),
            input: vec![
                ToolInput { name: "project", kind: InputKind::String, optional: true, describe: Some("one project's threads, by the name or the id projects lists".to_string()) },
            ],
            output: vec!["threads"],
            stream: vec![],
            call: call_threads,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/projects.ts:240
    Verb::Cli(CliVerb {
        name: "threads wait",
        usage: "wsp threads wait <thread>... [--timeout <s>] [--tail]".to_string(),
        about: "blocks until one of the threads leaves running and prints its finished line, with the reply whole under it; --tail prints the reply's last line alone, which is what a notify sends, and --timeout gives up after so many seconds and says so on stderr".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("timeout", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("tail", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_threads_wait,
        tool: Tool {
            description: "Blocks until one of the named threads leaves running and answers with that thread's end: its id, status (completed, interrupted or failed), how long it worked, what it cost and the last line of its reply, and on a sub-thread result, what its machine was read to hold, the text being the one line a notify sends. One thread per call: a caller that started three builders calls this three times, dropping each returned id from the list, since a thread already over comes back at once and would come back again. With timeout, the seconds to wait before answering with nothing and timedOut true, so other work fits between calls; keep it under your own tool call limit and call again. This blocks, so it is for a shell script and not for your own conversation. A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report. A caller that is not a wsp thread cannot be woken at all, so it takes the reply of one turn as the call returns, and hands work of more than one turn to a single coordinator thread in the project's folder on this computer. Never poll threads for a state change.".to_string(),
            input: vec![
                ToolInput { name: "threads", kind: InputKind::StringList { min: Some(1) }, optional: false, describe: Some("thread ids, or prefixes that each pick one".to_string()) },
                ToolInput { name: "timeout", kind: InputKind::Number, optional: true, describe: Some("seconds to wait; absent waits until one of the threads finishes".to_string()) },
            ],
            output: vec!["finished", "timedOut"],
            stream: vec![],
            call: call_threads_wait,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    ]
}

/// The `projects` tool's call (`verbs/projects.ts:74-77`).
fn call_projects(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = args;
        let client = deps.client(None).await?;
        let projects = projects_listed(&*client).await?;
        let defaults = project_defaults_of(&*client).await?;
        Ok(as_json(json!({ "projects": projects, "defaults": json_of(&defaults) })))
    })
}

/// `wsp projects` (`verbs/projects.ts:56-68`).
fn run_projects(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if !ctx.args.is_empty() {
            return Err(usage_refusal("wsp projects takes no positional arguments.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let projects = projects_listed(&*client).await?;
        let none = projects.is_empty();
        let held = if none { Vec::new() } else { workspaces(&*client).await? };
        let running = if none { Vec::new() } else { threads(&*client, None).await? };
        let named: IndexMap<String, String> = if none { IndexMap::new() } else { place_names(&*client).await.unwrap_or_default() };
        let defaults: IndexMap<String, ThreadDefaults> = if none { IndexMap::new() } else { project_defaults_of(&*client).await? };
        let line = if none {
            NO_PROJECT_YET.to_owned()
        } else {
            let mut rows =
                vec![["PROJECT", "ID", "COMPUTER", "SOURCE", "PATH", "BASE", "THREADS", "NEW THREADS"].map(str::to_owned).to_vec()];
            for p in &projects {
                let mut cells = project_row_line(p, &held, &running, &named);
                cells.push(defaults_cell(defaults.get(row_text(p, "id"))));
                rows.push(cells);
            }
            table(&rows).join("\n")
        };
        ctx.out.emit(&json!({ "projects": projects, "defaults": json_of(&defaults) }), Some(&line));
        Ok(0)
    })
}

/// The `projects_add` tool's call (`verbs/projects.ts:95-109`).
fn call_projects_add(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let mut params = Frame::new();
        params.insert("source".to_owned(), Value::from(arg_str(&args, "source").unwrap_or_default()));
        for key in ["on", "name", "base", "into"] {
            if let Some(value) = arg_str(&args, key) {
                params.insert(key.to_owned(), Value::from(value));
            }
        }
        let reply = client.request("projects.add", params).await?;
        let project: ProjectView = field(&reply, "projects.add", "project")?;
        let shown_project = reply.get("project").cloned().unwrap_or(Value::Null);
        let notice = reply.get("notice").and_then(Value::as_str).map(str::to_owned);
        let named = place_names(&*client).await.unwrap_or_default();
        let said = wsp_words::projects::added_project_line(&project, Some(&named), &platform());
        let text = match &notice {
            Some(notice) => format!("{said}\n{notice}"),
            None => said,
        };
        let structured = match &notice {
            Some(notice) => json!({ "project": shown_project, "notice": notice }),
            None => json!({ "project": shown_project }),
        };
        Ok(as_text(&text, structured))
    })
}

/// A null project dies at the read of its id, as the TS does; a row without an id leaves the key out of the frame.
fn project_id_of(project: &Value) -> Result<Option<Value>, HostError> {
    if project.is_null() {
        return Err(HostError::new("Cannot read properties of null (reading 'id')"));
    }
    Ok(project.get("id").cloned())
}

/// A reply without `said` has no line and no `said` key.
fn said_of(reply: &Frame) -> Result<Option<String>, HostError> {
    reply
        .get("said")
        .map(|value| serde_json::from_value(value.clone()).map_err(|_| HostError::new(other_version("projects.remove"))))
        .transpose()
}

/// The `projects_remove` tool's call (`verbs/projects.ts:141-146`).
fn call_projects_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let project = project_listed(&*client, arg_str(&args, "project").unwrap_or_default()).await?;
        let mut params = pick(vec![("projectId", project_id_of(&project)?)]);
        if args.get("force") == Some(&Value::Bool(true)) {
            params.insert("force".to_owned(), Value::Bool(true));
        }
        let reply = client.request("projects.remove", params).await?;
        let said = said_of(&reply)?;
        let structured = pick(vec![("project", Some(project)), ("said", said.as_ref().map(|s| json!(s)))]);
        Ok(as_text(said.as_deref().unwrap_or_default(), Value::Object(structured)))
    })
}

/// `wsp projects remove` (`verbs/projects.ts:118-133`).
fn run_projects_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.as_str(),
            _ => return Err(usage_refusal("wsp projects remove takes one project.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let project = project_listed(&*client, reference).await?;
        let project_id = project_id_of(&project)?;
        let project_name = shown(project.get("name"));
        let force = flag_set(&ctx.flags, "force");
        let mut check = pick(vec![("projectId", project_id.clone()), ("check", Some(Value::Bool(true)))]);
        if force {
            check.insert("force".to_owned(), Value::Bool(true));
        }
        let answer = client.request("projects.remove", check).await?;
        let losing = match answer.get("unsaved").and_then(Value::as_str) {
            Some(unsaved) => format!("\nWith it goes work no remote has: {unsaved}."),
            None => String::new(),
        };
        let question = format!(
            "Remove {}?\nIts record leaves this wsp, with the folder wsp made for it on the computer holding it; a folder of yours stays where it is.{losing}",
            project_name
        );
        if !confirmed(ctx, &question, &project_name).await? {
            return Ok(1);
        }
        let mut remove = pick(vec![("projectId", project_id)]);
        if force {
            remove.insert("force".to_owned(), Value::Bool(true));
        }
        let reply = client.request("projects.remove", remove).await?;
        let said = said_of(&reply)?;
        let answer = pick(vec![("project", Some(project)), ("said", said.as_ref().map(|s| json!(s)))]);
        ctx.out.emit(&Value::Object(answer), said.as_deref());
        Ok(0)
    })
}

/// The `projects_set` tool's call (`verbs/projects.ts:184-187`).
fn call_projects_set(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let project = arg_str(&args, "project").unwrap_or_default();
        let ask = ProjectSetAsk {
            agent: arg_str(&args, "agent").map(str::to_owned),
            model: arg_str(&args, "model").map(str::to_owned),
            effort: arg_str(&args, "effort").map(str::to_owned),
            access: arg_str(&args, "access").map(str::to_owned),
            after_worktree: arg_str(&args, "after_worktree").map(str::to_owned),
            trackers: arg_list(&args, "trackers"),
            reset: arg_list(&args, "reset"),
        };
        let set = project_defaults_set(&*deps.client(None).await?, project, &ask).await?;
        let text = project_set_lines(&set.project_name, &set.defaults, set.after_worktree.as_deref(), set.trackers.as_deref()).join("\n");
        Ok(as_text(&text, json_of(&set)))
    })
}

/// `wsp projects set` (`verbs/projects.ts:155-169`).
fn run_projects_set(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.as_str(),
            _ => return Err(usage_refusal("wsp projects set takes one project.", &usage_is(ctx))),
        };
        let flags = &ctx.flags;
        let ask = ProjectSetAsk {
            agent: flag(flags, "agent").map(str::to_owned),
            model: flag(flags, "model").map(str::to_owned),
            effort: flag(flags, "effort").map(str::to_owned),
            access: flag(flags, "access").map(str::to_owned),
            after_worktree: flag(flags, "after-worktree").map(str::to_owned),
            trackers: Some(flag_list(flags, "tracker")),
            reset: Some(flag_list(flags, "reset")),
        };
        let set = project_defaults_set(&*ctx.client(None).await?, reference, &ask).await?;
        let line = project_set_lines(&set.project_name, &set.defaults, set.after_worktree.as_deref(), set.trackers.as_deref()).join("\n");
        ctx.out.emit(&json_of(&set), Some(&line));
        Ok(0)
    })
}

/// The `projects_seed` tool's call (`verbs/projects.ts:214-214`).
fn call_projects_seed(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let ask = SeedAsk {
            keep: arg_list(&args, "keep").unwrap_or_default(),
            cut: arg_list(&args, "cut").unwrap_or_default(),
            none: args.get("none") == Some(&Value::Bool(true)),
            save: args.get("confirm") == Some(&Value::Bool(true)),
        };
        let seed = project_seed(&*deps.client(None).await?, arg_str(&args, "project").unwrap_or_default(), &ask).await?;
        Ok(as_json(json_of(&seed)))
    })
}

/// `wsp projects seed` (`verbs/projects.ts:196-202`).
fn run_projects_seed(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.as_str(),
            _ => return Err(usage_refusal("wsp projects seed takes one project.", &usage_is(ctx))),
        };
        let ask = SeedAsk {
            keep: flag_list(&ctx.flags, "keep"),
            cut: flag_list(&ctx.flags, "cut"),
            none: flag_set(&ctx.flags, "none"),
            save: flag_set(&ctx.flags, "yes"),
        };
        let seed = project_seed(&*ctx.client(None).await?, reference, &ask).await?;
        ctx.out.emit(&json_of(&seed), Some(&project_seed_text(&seed)));
        Ok(0)
    })
}

/// The `threads` tool's call (`verbs/projects.ts:236-236`).
fn call_threads(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let within = arg_str(&args, "project");
        let rows = thread_rows(&*deps.client(None).await?, within).await?;
        Ok(as_json(json!({ "threads": json_of(&rows) })))
    })
}

/// `wsp threads` (`verbs/projects.ts:223-230`).
fn run_threads(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            let said = "wsp threads takes at most one project; wsp threads wait is its one subcommand, and wsp thread read <thread> prints what one said.";
            return Err(usage_refusal(said, &usage_is(ctx)));
        }
        let within: Option<String> = ctx.args.first().cloned();
        let tree = flag_set(&ctx.flags, "tree");
        let frame = move |client: Arc<dyn HostClient>| -> BoxFuture<'static, Result<Rows, HostError>> {
            let within = within.clone();
            Box::pin(async move { thread_rows_frame(&*client, within.as_deref(), tree).await })
        };
        draw_rows(ctx, "wsp threads", &frame).await
    })
}

/// A timeout answers the milliseconds it was given, not the whole ones the wait slept (io.ts:315): `fmtDuration` rounds them.
fn timed_out_as_given(waited: Waited, given_ms: Option<f64>) -> Waited {
    match (waited, given_ms) {
        (Waited::TimedOutMs(_), Some(ms)) => Waited::TimedOutMs(ms),
        (waited, _) => waited,
    }
}

/// The `threads_wait` tool's call (`verbs/projects.ts:267-271`).
fn call_threads_wait(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let refs = arg_list(&args, "threads").unwrap_or_default();
        let named = threads_of(&*deps.client(None).await?, &refs).await?;
        let given_ms = args.get("timeout").and_then(Value::as_f64).map(|seconds| seconds * 1_000.0);
        let waited = wait_through(deps, &named, given_ms.map(|ms| ms as u64), None).await?;
        let waited = timed_out_as_given(waited, given_ms);
        let (value, line) = wait_answer(&named, &waited, NotifyLength::Tail);
        Ok(as_text(&line, json_of(&value)))
    })
}

/// `wsp threads wait` (`verbs/projects.ts:246-259`).
fn run_threads_wait(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            return Err(usage_refusal("wsp threads wait takes one thread or more.", &usage_is(ctx)));
        }
        let given_ms = timeout_flag(flag(&ctx.flags, "timeout"))?;
        let client = ctx.client(None).await?;
        let named = threads_of(&*client, &ctx.args).await?;
        let io = ctx.io.clone();
        let redialed = move || io.error(HOST_RESTARTING_LINE);
        let waited = wait_through(&ctx.deps, &named, given_ms.map(|ms| ms as u64), Some(&redialed)).await?;
        let waited = timed_out_as_given(waited, given_ms);
        let length = if flag_set(&ctx.flags, "tail") { NotifyLength::Tail } else { NotifyLength::Whole };
        let (value, line) = wait_answer(&named, &waited, length);
        if value.timed_out == Some(true) {
            ctx.out.emit(&json_of(&value), None);
            ctx.io.error(&line);
        } else {
            ctx.out.emit(&json_of(&value), Some(&line));
        }
        Ok(0)
    })
}
