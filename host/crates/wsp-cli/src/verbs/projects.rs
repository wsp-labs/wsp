// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/projects.ts` at 65f6b89c7: `wsp projects` and its subcommands, `wsp threads` and
//! `wsp threads wait`, and the tool-only `projects_add`. Spec: `spec/projects.md`.

use wsp_wire::types::thread_defaults::ThreadDefaults;
use wsp_wire::views::workspace::ProjectSeed;
use wsp_words::exit::HostError;

use super::client::*;

/// `projectSetLines` (projects.ts:24-29): what a new thread on the project starts on, then the after-worktree
/// command and the trackers where the answer carries them.
pub fn project_set_lines(
    project_name: &str,
    defaults: &ThreadDefaults,
    after_worktree: Option<&str>,
    trackers: Option<&[String]>,
) -> Vec<String> {
    let _ = (project_name, defaults, after_worktree, trackers);
    todo!("projects.ts:24-29")
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
    let _ = (client, project, ask);
    todo!("projects.ts:32-41")
}

/// `projectSeedText` (projects.ts:44-47): the head, a blank line, the menu indented two spaces and a blank line
/// where it has rows, then the foot.
pub fn project_seed_text(seed: &ProjectSeed) -> String {
    let _ = seed;
    todo!("projects.ts:44-47")
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
                ToolInput { name: "reset", kind: InputKind::StringList { min: None }, optional: true, describe: Some("fields to put back on the layer below: agent, model, effort, access, after-worktree to take the command away, or trackers to empty the list".to_string()) },
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
        let _ = (args, deps);
        todo!("verbs/projects.ts:74-77")
    })
}

/// `wsp projects` (`verbs/projects.ts:56-68`).
fn run_projects(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/projects.ts:56-68")
    })
}

/// The `projects_add` tool's call (`verbs/projects.ts:95-109`).
fn call_projects_add(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/projects.ts:95-109")
    })
}

/// The `projects_remove` tool's call (`verbs/projects.ts:141-146`).
fn call_projects_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/projects.ts:141-146")
    })
}

/// `wsp projects remove` (`verbs/projects.ts:118-133`).
fn run_projects_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/projects.ts:118-133")
    })
}

/// The `projects_set` tool's call (`verbs/projects.ts:184-187`).
fn call_projects_set(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/projects.ts:184-187")
    })
}

/// `wsp projects set` (`verbs/projects.ts:155-169`).
fn run_projects_set(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/projects.ts:155-169")
    })
}

/// The `projects_seed` tool's call (`verbs/projects.ts:214-214`).
fn call_projects_seed(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/projects.ts:214-214")
    })
}

/// `wsp projects seed` (`verbs/projects.ts:196-202`).
fn run_projects_seed(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/projects.ts:196-202")
    })
}

/// The `threads` tool's call (`verbs/projects.ts:236-236`).
fn call_threads(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/projects.ts:236-236")
    })
}

/// `wsp threads` (`verbs/projects.ts:223-230`).
fn run_threads(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/projects.ts:223-230")
    })
}

/// The `threads_wait` tool's call (`verbs/projects.ts:267-271`).
fn call_threads_wait(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/projects.ts:267-271")
    })
}

/// `wsp threads wait` (`verbs/projects.ts:246-259`).
fn run_threads_wait(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/projects.ts:246-259")
    })
}
