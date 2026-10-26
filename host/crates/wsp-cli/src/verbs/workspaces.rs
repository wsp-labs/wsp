// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/workspaces.ts` at 65f6b89c7: the git verbs on a thread's folder (commit, discard, fix,
//! merge in, merge, update), the GitHub starts (start, review, review post), the machine verbs (rename, fork,
//! pause, resize, wake, rebuild, forget), the image and `wsp delete`. Spec: `spec/workspaces-git.md` and
//! `spec/workspaces-life.md`.

mod git;
mod life;

use std::sync::Arc;

use self::git::*;
use self::life::*;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use wsp_wire::types::pull_request::{GitUpdateReply, MergeMethod};
use wsp_wire::types::tree::{MergeInResult, FIX_CHECK_OR_CHILD};
use wsp_wire::views::activity::ProcessesAsk;
use wsp_wire::views::harness::{ReviewPostResult, StartResult};
use wsp_wire::views::session::SessionOrigin;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::{WorkspaceOut, WorkspaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::daemon::{GitCommitReply, GitDiscardReply};
use wsp_words::changes::{committed_line, discarded_line, MadeCommit};
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::projects::{thread_delete_question, thread_deleted_line};
use wsp_words::pull_request::merged_line;
use wsp_words::start::{start_words, ReviewVerdict};
use wsp_words::sub_thread::sub_deleted_line;
use wsp_words::tree::merge_in_line;
use wsp_words::words::image::sealed_built_line;
use wsp_words::words::thread::{resize_word_line, resized_line, RESIZE_WORD_FIX};
use wsp_words::words::units::size_from_word;
use wsp_words::workspace_state::{
    delete_notice, gone_refusal, turn_spend_word, workspace_kind, workspace_state, WorkspaceState, WorkspaceStateInput,
};

use crate::cloud::cloud_on;
use crate::cloud_text::cloud_text;

use super::activity::{ended_lines, processes_asked};
use super::client::*;
use super::io::{as_json, as_text, confirmed};
use super::turns_help::{
    before_sending, checked_start, child_of, follow, follow_verb, notify_of, opening_of, pick_flags, refuse_machine_thread, sharing_with,
    sub_deleting, thread_at, thread_deleted, thread_here, turn_failure, turn_view, FollowOn, OpeningAt, OpeningOpts, Picks, SaidAbout,
    ThreadDeleted, ThreadDeletedAsk, ThreadRef, Turn,
};
use super::workspaces_help::{
    agents_asked, asked_to_fix, awake, build_image_at, committed, create_for, delete_question, delete_workspace, deleted_line, deleting,
    dropping, fix_line, folder_of, forget, forget_question, forgot_line, image_lines, image_view, made_worktree, merged_in, merged_pr,
    project_of, rebuild, rebuilt_line, renamed_workspace_line, review_lines, review_posted, shared_of, started_lines, state_line,
    thread_label, thread_of, update_line, with_shared, woke_line, workspace_of, ChildRef, CreateAsked, Dropping, MergeLead, ProjectPick,
    RenamedWorkspace, ReviewEdits, ReviewStartedAsk, StartedFromAsk, VERDICTS,
};

/// `NO_THREAD_FIX` (workspaces.ts:49): what a delete naming no thread says to do instead.
pub const NO_THREAD_FIX: &str = "Name a thread wsp threads lists.";

/// The entries of `verbs/workspaces.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn workspace_verbs() -> Vec<Verb> {
    vec![
    // verbs/workspaces.ts:53
    Verb::Cli(CliVerb {
        name: "rename",
        usage: "wsp rename <workspace> \"<name>\"".to_string(),
        about: "names the workspace on this computer; the name is unique here, so one another workspace holds is refused".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_rename,
        tool: Tool {
            description: "Names the workspace on this computer, the name the sidebar and every listing show and the one workspace takes. A name is unique here, since that is how a workspace is addressed, so a name another workspace holds and a blank one are refused in one line and nothing is renamed. Threads on the machine are addressed by id and run on through it, and the machine at the provider keeps the metadata name it was forked under until it is next forked or rebuilt.".to_string(),
            input: vec![
                ToolInput { name: "workspace", kind: InputKind::String, optional: false, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: None },
            ],
            output: vec!["was", "workspace"],
            stream: vec![],
            call: call_rename,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: true,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:78
    Verb::Cli(CliVerb {
        name: "fork",
        usage: "wsp fork <workspace> [--name <n>] [--size <cpu>x<memGb>] [--send \"<task>\" [run's flags]]".to_string(),
        about: "a new machine from the source's image version, on the source's branch with its commits pushed first, not a copy of its live disk; --size as new's, the person's alone".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("name", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("size", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("send", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("access", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("cwd", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("notify", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("max-machines", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("max-depth", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_fork,
        tool: Tool {
            description: "A new machine of the source's image version, a child of the source that starts on the source's branch with its commits pushed first (not a copy of its live disk); a thread's fork sits under that thread in its tree, and its size and agent caps are the person's to pick. A workspace in a project's folder is no machine to fork. With a task, its first thread is opened and the reply returned. When that first turn fails, the error still names the workspace, which exists: continue with run on it rather than forking again.".to_string(),
            input: vec![
                ToolInput { name: "workspace", kind: InputKind::String, optional: false, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
                ToolInput { name: "name", kind: InputKind::String, optional: true, describe: Some("defaults to <source>-fork".to_string()) },
                ToolInput { name: "size", kind: InputKind::String, optional: true, describe: Some("the machine size as <cpu>x<memGb>, like 2x4; absent takes the image's size. A size the provider does not offer is refused with the list it does, so read that list rather than guessing twice; a build wants the largest memory offered".to_string()) },
                ToolInput { name: "task", kind: InputKind::String, optional: true, describe: None },
                ToolInput { name: "agent", kind: InputKind::String, optional: true, describe: Some("the agent to run in the thread, one of claude, codex, opencode, cursor; absent means the host's default".to_string()) },
                ToolInput { name: "model", kind: InputKind::String, optional: true, describe: Some("the model the turn runs on, by the agent's own slug (claude-sonnet-5); absent on a new thread means the project's, else the one set for that agent, else the catalog's default, on send the thread's own".to_string()) },
                ToolInput { name: "effort", kind: InputKind::String, optional: true, describe: Some("the reasoning effort, by the agent's own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default, high for claude; on send, the thread's own".to_string()) },
                ToolInput { name: "access", kind: InputKind::String, optional: true, describe: Some("how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full".to_string()) },
                ToolInput { name: "cwd", kind: InputKind::String, optional: true, describe: Some("the folder on the machine the thread works in or the command runs in, absolute. Absent means the workspace's own project, which is where a thread there starts unless it says otherwise".to_string()) },
                ToolInput { name: "notify", kind: InputKind::StringList { min: Some(1) }, optional: true, describe: Some("who is told each time a turn of the new thread ends, each one a thread (by id, or a prefix of it) the line goes into as a message carrying that turn's whole reply, or me, which is the thread this call came out of when it came out of one and otherwise the person's app, where the line is its last reply line alone. Several targets each get the line once, which is how a builder's end reaches its orchestrator and a reviewer together; a target whose thread is gone by then falls back to the person, and the new thread's own id is refused. A wsp thread starts every child with --notify me and ends its turn, and each child's finished line wakes it with that child's whole report.".to_string()) },
                ToolInput { name: "max_machines", kind: InputKind::Integer { min: Some(0) }, optional: true, describe: Some("how many machines may stand at once under one root thread; defaults to 3".to_string()) },
                ToolInput { name: "max_depth", kind: InputKind::Integer { min: Some(1) }, optional: true, describe: Some("how many levels deep the tree under a root thread may go, 2 by default; 1 stops at the root's own children".to_string()) },
            ],
            output: vec!["workspace", "notice", "turn", "failure"],
            stream: vec!["workspace", "notice"],
            call: call_fork,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: true,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:140
    Verb::Cli(CliVerb {
        name: "commit",
        usage: "wsp commit <thread> [--message \"<message>\"] [--file <path>]...".to_string(),
        about: "commits every changed file in the folder the thread works in, other threads' changes there included, or the files named; without a message the thread's agent drafts one".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("message", FlagRow { kind: FlagKind::String, multiple: false, short: Some('m'), value_with: None }), ("file", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None })]),
        run: run_commit,
        tool: Tool {
            description: "Commits the files changed in the folder the thread works in against its last commit, untracked ones added, with the message given, running the folder's hooks as git does. Other threads working in the same folder share its changes, so a commit of every file takes theirs too and the answer names them in sharedWith; files names some by path from the checkout's top and leaves the rest uncommitted. Without a message the thread's own agent drafts one from the diff and the message the thread was opened with, on its own command line with no thread and no tool, and the commit is made with that. Refused in one line when a file named has no change, when git knows no author in the folder (with the command that sets one), when a hook said no (with its last line), and when another git in the folder holds the index after one wait. Nothing reaches a remote: the app's Push is what pushes.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "message", kind: InputKind::String, optional: true, describe: Some("the commit message, a subject line then a blank line and the body; without one the thread's agent drafts it".to_string()) },
                ToolInput { name: "files", kind: InputKind::StringList { min: None }, optional: true, describe: Some("the files to commit, by path from the checkout's top as git status names them; without it every changed file".to_string()) },
            ],
            output: vec!["oid", "subject", "filesChanged", "insertions", "deletions", "sharedWith"],
            stream: vec![],
            call: call_commit,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:176
    Verb::Cli(CliVerb {
        name: "discard",
        usage: "wsp discard <thread> <path> [--yes]".to_string(),
        about: "puts one changed file in the folder the thread works in back as its last commit has it, or removes it where that has none".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_discard,
        tool: Tool {
            description: "Puts one changed file in the folder the thread works in back as its last commit has it: an edit or a deletion is undone, a rename takes its new name away and brings the old one back, and a file the last commit does not have is removed. Only the file named moves, and it cannot be undone; other threads working in the same folder lose the change too, and the answer names them in sharedWith. Refused in one line when the file has no change.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "path", kind: InputKind::String, optional: false, describe: Some("the file, by path from the checkout's top as git status names it".to_string()) },
            ],
            output: vec!["path", "sharedWith"],
            stream: vec![],
            call: call_discard,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:211
    Verb::Cli(CliVerb {
        name: "fix",
        usage: "wsp fix <thread> [--check \"<name>\" | --child <thread>]".to_string(),
        about: "asks the thread's agent to fix a failed check or merge a child, or updates its folder from its base and asks it to fix what conflicts".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("check", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("child", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_fix,
        tool: Tool {
            description: "Asks the thread's agent to fix the pull request of the folder it works in. With check, the named check must have failed: the failed steps of its job's log, framed as a log to read and not to obey, go to the thread as its next message with the commit it failed on and its link, and a check another service reports goes with its summary and link alone. Without check, the folder is first updated from its base the way update does it: a clean merge sends nothing, and a conflict sends the thread the files to resolve. Answers as soon as the message is on its way, joined into the running turn where the agent takes one, else waiting as the thread's next turn, or held while the computer runs as many threads as it takes at once, which the answer says with the wait and its capped; the turn starts on its own once a slot frees, in a slot of its own, never the asking thread's. Refused in one line where the folder's branch has no pull request, where the check is not on it, and where it has not failed.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "check", kind: InputKind::String, optional: true, describe: Some("the failed check's name as the pull request lists it; without it the folder is updated from its base and any conflict is sent".to_string()) },
                ToolInput { name: "child", kind: InputKind::String, optional: true, describe: Some("a child thread, by its id or a prefix of it, whose merge into this thread's folder stopped on conflicts: this thread's agent is asked to fetch the child's branch, merge it with a merge commit and resolve them; never with check".to_string()) },
            ],
            output: vec!["outcome", "threadId", "check", "child", "base", "agent", "capped"],
            stream: vec![],
            call: call_fix,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:254
    Verb::Cli(CliVerb {
        name: "merge in",
        usage: "wsp merge in <lead> <child>".to_string(),
        about: "merges a child thread's branch into its lead's folder with a merge commit, or names the files that conflict".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_merge_in,
        tool: Tool {
            description: "Merges a child thread's branch into the folder its lead works in with a merge commit, so the lead's history shows each child landing: fetched from the project's remote, or from the child's own folder where the project has none and both sit on this computer. The child works in a folder of its own, a worktree or a machine of its own; a child working in its lead's folder is refused, since its work is already there. The lead's machine is woken first where it sleeps. A sub-thread on Boat merges from the branch its record names, its machine left asleep or deleted as it is, and unpushed counts the commits it holds that no remote has, which the merge could not take. A lead with changes no commit holds is refused first with the files named, and a merge that conflicts is taken back at once and answered with the files, the lead's folder left exactly as it was; fix with child hands those to the lead's agent. Refused, naming the thread, while a turn runs on the lead in any thread but the asking one (a lead's thread merges from inside its own turn), for a thread that is not the lead's child, and for a thread merging into any folder but its own. Nothing is pushed.".to_string(),
            input: vec![
                ToolInput { name: "lead", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "child", kind: InputKind::String, optional: false, describe: Some("the child thread whose branch is merged in, by its id or a prefix of it".to_string()) },
            ],
            output: vec!["lead", "child", "branch", "merged", "commits", "conflicts", "unpushed"],
            stream: vec![],
            call: call_merge_in,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:287
    Verb::Cli(CliVerb {
        name: "merge",
        usage: "wsp merge <thread> [--method merge|squash|rebase] [--when-checks-pass]".to_string(),
        about: "merges the pull request of the thread's branch, or merges it once its checks pass".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("method", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("when-checks-pass", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_merge,
        tool: Tool {
            description: "Merges the pull request of the branch the thread's folder is on as the person, by the method named or the repository's own default, and only while its head is still the commit the host last read, so a push since then fails it in the git host's own words. when_checks_pass arms it to merge once its checks pass instead, where the repository allows that. Answers with the number, the method, and whether it merged now or waits on its checks. Refused in one line where the branch has no open pull request, where the repository does not allow the method or does not merge by itself, and with the git host's own reason where it refused, branch protection included. A thread's own token is refused: merging is the person's act.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "method", kind: InputKind::Enum(vec!["merge", "squash", "rebase"]), optional: true, describe: Some("merge, squash or rebase; without it the repository's default".to_string()) },
                ToolInput { name: "when_checks_pass", kind: InputKind::Boolean, optional: true, describe: Some("merge once the checks pass rather than now, where the repository allows it".to_string()) },
            ],
            output: vec!["number", "method", "merged", "autoArmed"],
            stream: vec![],
            call: call_merge,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:322
    Verb::Cli(CliVerb {
        name: "start",
        usage: "wsp start <link> [--project <name>] [--agent <id>] [--model, --effort, --access <word>]".to_string(),
        about: "a thread off a GitHub issue or pull request link, the issue or the pull request its first message; an issue's thread runs in the project folder, a pull request's in a worktree on its branch".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("access", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_start,
        tool: Tool {
            description: "Opens a thread off a GitHub issue or pull request link, returning as soon as the thread is started. The link names the repository, and the project here whose remote is that repository is the one used (project names one where two computers hold it); a link no project matches is refused naming the repository and the add line. The issue's or pull request's title, description, comments and link are the thread's first message, and an issue's thread is asked to have its pull request close the issue. On the computer the app runs on an issue's thread runs in the project folder, and a pull request's head is fetched into the project's repo as a branch and its thread runs in a worktree on it, so the project folder's checkout never moves. A thread's own token is refused: starting work from a link is the person's act.".to_string(),
            input: vec![
                ToolInput { name: "link", kind: InputKind::String, optional: false, describe: Some("a GitHub issue or pull request link, https://github.com/<owner>/<repo>/issues/<n> or /pull/<n>".to_string()) },
                ToolInput { name: "project", kind: InputKind::String, optional: true, describe: Some("the project by name or id where two projects hold the repository; absent is this computer's".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: true, describe: Some("the agent to run in the thread, one of claude, codex, opencode, cursor; absent means the host's default".to_string()) },
                ToolInput { name: "model", kind: InputKind::String, optional: true, describe: Some("the model the turn runs on, by the agent's own slug (claude-sonnet-5); absent on a new thread means the project's, else the one set for that agent, else the catalog's default, on send the thread's own".to_string()) },
                ToolInput { name: "effort", kind: InputKind::String, optional: true, describe: Some("the reasoning effort, by the agent's own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default, high for claude; on send, the thread's own".to_string()) },
                ToolInput { name: "access", kind: InputKind::String, optional: true, describe: Some("how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full".to_string()) },
            ],
            output: vec!["workspace", "threadId", "sessionId"],
            stream: vec![],
            call: call_start,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:352
    Verb::Cli(CliVerb {
        name: "review",
        usage: "wsp review <link|thread> [--agent <id>] [--model, --effort <word>]".to_string(),
        about: "a reviewer thread on a pull request, read-only, whose review waits in wsp until you post it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_review,
        tool: Tool {
            description: "Starts a reviewer thread on a pull request, off its link or off the pull request of the branch a thread's folder is on, and returns as soon as the thread is started, its id first, which review_post takes. On the computer the app runs on the reviewer works in a worktree on the pull request's head<!-- cloud -->, and on a cloud in a fresh copy at that head<!-- /cloud -->, at its agent's read-only access (Codex unless another is named; an agent with no read-only access is refused naming the ones that have one), with the description, the diff against the base and the repository's own review rules in its first message. Its reply ends in a review the host keeps as a draft; nothing reaches the git host until review_post. A thread's own token is refused.".to_string(),
            input: vec![
                ToolInput { name: "target", kind: InputKind::String, optional: false, describe: Some("a GitHub pull request link, or the thread whose branch's pull request to review, by its id or a prefix of it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: true, describe: Some("the reviewing agent, codex or claude; absent is codex".to_string()) },
                ToolInput { name: "model", kind: InputKind::String, optional: true, describe: Some("the model the turn runs on, by the agent's own slug (claude-sonnet-5); absent on a new thread means the project's, else the one set for that agent, else the catalog's default, on send the thread's own".to_string()) },
                ToolInput { name: "effort", kind: InputKind::String, optional: true, describe: Some("the reasoning effort, by the agent's own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default, high for claude; on send, the thread's own".to_string()) },
            ],
            output: vec!["workspace", "threadId", "sessionId"],
            stream: vec![],
            call: call_review,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:385
    Verb::Cli(CliVerb {
        name: "review post",
        usage: "wsp review post <thread> [--verdict comment|approve|request-changes] [--summary \"<text>\"]".to_string(),
        about: "posts a reviewer thread's review on its pull request as you, its ticked comments on their lines".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("verdict", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("summary", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_review_post,
        tool: Tool {
            description: "Posts a reviewer thread's review on its pull request as the person, in one call: the verdict, the summary and every ticked comment on its line, pinned to the head the review was written against. verdict and summary edit the draft first. A comment on a line outside the diff goes into the summary, since the git host takes none there. Refused in one line where the thread has no review yet, and with the git host's own reason where it refused, approving one's own pull request included. A thread's own token is refused: posting under the person's name is the person's act.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "verdict", kind: InputKind::Enum(vec!["comment", "approve", "request_changes"]), optional: true, describe: Some("comment, approve or request_changes; absent is the draft's".to_string()) },
                ToolInput { name: "summary", kind: InputKind::String, optional: true, describe: Some("the review's summary; absent is the draft's".to_string()) },
            ],
            output: vec!["url", "number", "comments", "folded"],
            stream: vec![],
            call: call_review_post,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:420
    Verb::Cli(CliVerb {
        name: "update",
        usage: "wsp update <thread>".to_string(),
        about: "merges the latest commits of the base into the branch of the folder the thread works in, or names the files that conflict".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_update,
        tool: Tool {
            description: "Merges the latest commits of the base from the remote into the branch the thread's folder is on, with a merge commit, so a branch already pushed is never rewritten; other threads working in the same folder get them too, and the answer names them in sharedWith. A folder with changes no commit holds is refused first with the files named. A merge that conflicts is taken back at once and answered with the files that conflict, the folder left exactly as it was; fix sends those to the agent. Nothing is pushed.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
            ],
            output: vec!["base", "merged", "commits", "conflicts", "sharedWith"],
            stream: vec![],
            call: call_update,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:452
    Verb::Cli(CliVerb {
        name: "pause",
        usage: "wsp pause <workspace>".to_string(),
        about: "naps the workspace's machine".to_string(),
        page: Page::Front,
        options: FlagTable(vec![]),
        run: run_pause,
        tool: Tool {
            description: "Naps the workspace's machine; it wakes on the next thread or command.".to_string(),
            input: vec![
                ToolInput { name: "workspace", kind: InputKind::String, optional: false, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
            ],
            output: vec!["workspace"],
            stream: vec![],
            call: call_pause,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: true,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:474
    Verb::Cli(CliVerb {
        name: "resize",
        usage: "wsp resize <thread> --size <cpu>x<memGb>".to_string(),
        about: "moves the machine a thread runs on to another size its provider sells: a stop, then a resume at that size".to_string(),
        page: Page::App,
        options: FlagTable(vec![("size", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_resize,
        tool: Tool {
            description: "Moves the machine a thread runs on to another size its provider sells, as a stop and then a resume at that size, and returns its record once it runs again. On Boat small and default share a 75 GB root disk and a move to large grows it to 160 GB with the files kept; xlarge's was never measured. Refused while a turn runs on it, since the stop would end it, and on a provider that changes no machine's size; a machine already at that size comes back unchanged. A thread's own token is refused: the bigger size is the person's spend.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: false, describe: Some("the thread, by its id or a prefix of it that names one, as threads lists them".to_string()) },
                ToolInput { name: "size", kind: InputKind::String, optional: false, describe: Some("the size to move to, as <cpu>x<memGb>, like 4x8".to_string()) },
            ],
            output: vec!["workspace"],
            stream: vec![],
            call: call_resize,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:502
    Verb::Cli(CliVerb {
        name: "wake",
        usage: "wsp wake <workspace>".to_string(),
        about: "wakes the workspace's machine and prints the state it came up in".to_string(),
        page: Page::Front,
        options: FlagTable(vec![]),
        run: run_wake,
        tool: Tool {
            description: "Wakes the workspace's machine and returns its view once the runtime has answered; one already running comes back unchanged. run, send and exec do this themselves, so it is only needed to wake a machine ahead of them.".to_string(),
            input: vec![
                ToolInput { name: "workspace", kind: InputKind::String, optional: false, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
            ],
            output: vec!["workspace"],
            stream: vec![],
            call: call_wake,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: true,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:527
    Verb::Cli(CliVerb {
        name: "rebuild",
        usage: "wsp rebuild <workspace>".to_string(),
        about: "replaces a gone workspace's machine from its image and prints the state of the new one".to_string(),
        page: Page::App,
        options: FlagTable(vec![]),
        run: run_rebuild,
        tool: Tool {
            description: "Replaces the machine of a workspace the provider no longer has, forking it afresh from the image the workspace was made on and importing the vault its last nap left; the workspace keeps its id, its name and its threads, and the new machine's id and state come back once the runtime has them. Anything written on the old machine's disk since that nap is not there. This is the road out of the refusal every other verb gives a gone workspace, and it is refused in one line on a machine that still answers.".to_string(),
            input: vec![
                ToolInput { name: "workspace", kind: InputKind::String, optional: false, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
            ],
            output: vec!["workspace"],
            stream: vec![],
            call: call_rebuild,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: true,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:552
    Verb::Cli(CliVerb {
        name: "image",
        usage: "wsp image".to_string(),
        about: "the image this host owns: its version, its hash, whether it holds your sign-ins (an image built on Boat holds none), and the copy each place has built of it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_image,
        tool: Tool {
            description: "The image this host owns and the copy each place has built of it. The record is the recipe the seal was planned from, the sign-ins it holds and a hash over both; a copy built at that hash is current and any other is stale, whatever version the place's own manifest gave it. A record with no vault was read back off its own copy rather than written at a seal, so it judges none of them and every copy of it asks for the sign-ins again: cutting the next version holds them. An image built on Boat holds no sign-in: its machines use the sign-ins on this computer through wsp.".to_string(),
            input: vec![],
            output: vec!["image", "copies"],
            stream: vec![],
            call: call_image,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:575
    Verb::Cli(CliVerb {
        name: "image build",
        usage: "wsp image build <place> [--force]".to_string(),
        about: "builds this host's image at a place from the record, its sign-ins coming from the vault and no sign-in run again".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("force", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_image_build,
        tool: Tool {
            description: "Builds this host's image at a place from the record alone: a builder is forked there with the recipe the image was sealed from and every sign-in set to skip, the sign-ins the seal held are landed on it out of the vault, and the copy is sealed and recorded under that place at the record's hash. Nothing signs in again and no Keychain is read. A place that already holds a copy built from this record is answered with that copy and `built` false, so asking twice costs nothing, unless `force` asks for a new copy, which takes the daemon this host ships now; a place whose copy is building is answered with that build, never a second one. A joined computer builds its copy at the end of its setup; every other copy, and every copy a newer version left behind, is built only when this line asks or when a fork there finds no current copy, and that fork says so before the build starts, since a build bills where it runs. Refused in one line for a place this host does not hold, for a place that takes no copy at all, and for a record sealed without the recipe it was built from. A record holding no sign-ins is refused too, since every copy of it would ask for them again; `force` builds it anyway.".to_string(),
            input: vec![
                ToolInput { name: "place", kind: InputKind::String, optional: false, describe: Some("the place to build the copy at, by the name wsp computers lists".to_string()) },
                ToolInput { name: "force", kind: InputKind::Boolean, optional: true, describe: Some("build a new copy even where the place already holds this version; a record holding no sign-ins is built too, and its copy asks for every one of them again".to_string()) },
            ],
            output: vec!["copy", "built"],
            stream: vec![],
            call: call_image_build,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:599
    Verb::Cli(CliVerb {
        name: "forget",
        usage: "wsp forget <workspace> [--yes]".to_string(),
        about: "drops a gone workspace and its threads from this computer; refused while its machine exists".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_forget,
        tool: Tool {
            description: "Drops a workspace whose machine the provider no longer has: its record and its threads leave this computer and the person's sidebar. The provider is read until twelve reads in a row agree the machine is gone, and nothing is asked of the machine. Refused in one line while the machine still exists (pause it, or delete it at the provider, first).".to_string(),
            input: vec![
                ToolInput { name: "workspace", kind: InputKind::String, optional: false, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
            ],
            output: vec!["workspaceId", "name", "threads"],
            stream: vec![],
            call: call_forget,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: true,
        cloud_flags: vec![],
    }),
    // verbs/workspaces.ts:629
    Verb::Cli(CliVerb {
        name: "delete",
        usage: "wsp delete <thread><!-- cloud -->|<workspace><!-- /cloud --> [--processes stop|keep] [--yes]".to_string(),
        about: "takes a thread away, its turns and checkpoints with it, and the worktree wsp made for it once no other thread is in it, never the project folder, refused while one still runs processes, on this computer or a box, unless --processes says to stop or keep them<!-- cloud -->; a workspace goes as before, its machine deleted at the provider and its record and threads dropped from this computer<!-- /cloud -->".to_string(),
        page: Page::Front,
        options: FlagTable(vec![("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("processes", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_delete,
        tool: Tool {
            description: "Takes a thread away, named by thread: its turns and checkpoints go, and where it is the last thread in a worktree wsp made, that worktree goes with it, refused over files no commit holds; the project folder is never touched. A sub-thread goes with its machine on Boat and its kept session, answering machine deleted; the question names work its machine holds that no remote has, and a thread's own token is refused it while such work stands.<!-- cloud --> Named by workspace, a workspace goes as before: its machine is deleted at the provider and its record and threads dropped from this computer and the person's sidebar, and everything on that machine's disk that was not exported or pushed goes with it.<!-- /cloud --> Called without confirm it deletes nothing and answers with what would go, which is the line to put to the person. A thread that still runs processes beside its agent (a dev server, a watcher), on this computer or a box, is refused, naming them, until processes says stop, which ends each by identity before the thread goes (on a box, everything in the thread's cgroup there), or keep, which leaves them running.".to_string(),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("the thread, by its id or a prefix of it that names one".to_string()) },
                ToolInput { name: "workspace", kind: InputKind::String, optional: true, describe: Some("the workspace's name, or its id when two share a name".to_string()) },
                super::activity::processes_in(),
                ToolInput { name: "confirm", kind: InputKind::Boolean, optional: true, describe: Some("true deletes it; absent or false answers with what would go and deletes nothing, so a person can be asked first".to_string()) },
            ],
            output: vec!["threadId", "workspaceId", "worktree", "name", "machineId", "threads", "ended", "machine"],
            stream: vec![],
            call: call_delete,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec!["workspace"],
    }),
    ]
}

fn frame_str(args: &Frame, key: &str) -> Option<String> {
    args.get(key).and_then(Value::as_str).map(str::to_owned)
}

fn frame_list(args: &Frame, key: &str) -> Vec<String> {
    args.get(key)
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn frame_true(args: &Frame, key: &str) -> bool {
    args.get(key) == Some(&Value::Bool(true))
}

fn frame_picks(args: &Frame) -> Picks {
    Picks {
        model: frame_str(args, "model"),
        effort: frame_str(args, "effort"),
        access: frame_str(args, "access"),
        fast: frame_true(args, "fast"),
    }
}

fn json_of<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

fn object_of<T: Serialize>(value: &T) -> Map<String, Value> {
    match json_of(value) {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// `{...x, ...sharedOf(shared)}`: the answer's fields with `sharedWith` where other threads share the folder.
fn with_shared_field(mut map: Map<String, Value>, shared: &[String]) -> Value {
    if let Some(ids) = shared_of(shared) {
        map.insert("sharedWith".to_owned(), json!(ids));
    }
    Value::Object(map)
}

fn parsed<T: DeserializeOwned>(reply: Frame, op: &str) -> Result<T, HostError> {
    serde_json::from_value(Value::Object(reply)).map_err(|_| HostError::new(other_version(op)))
}

fn view_of(out: &WorkspaceOut) -> Result<WorkspaceView, HostError> {
    serde_json::from_value(json_of(out)).map_err(|_| HostError::new(other_version("workspaces")))
}

fn processes_ask(word: Option<&str>) -> ProcessesAsk {
    match word {
        Some("stop") => ProcessesAsk::Stop,
        Some("keep") => ProcessesAsk::Keep,
        _ => ProcessesAsk::Ask,
    }
}

fn redial_of(deps: &VerbDeps) -> impl Fn() -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> + Send + Sync {
    let owned = deps.clone();
    move || {
        let owned = owned.clone();
        Box::pin(async move { host_back(&owned, None).await }) as BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>>
    }
}

fn home() -> Option<String> {
    std::env::var("HOME").ok().filter(|home| !home.is_empty())
}

fn stderr_of(ctx: &VerbContext) -> impl Fn(&str) + Send + Sync {
    let io = ctx.io.clone();
    move |line: &str| io.error(line)
}

fn quiet_tell(_line: &str) {}

fn is_link(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

fn verdict_of(word: &str) -> Option<ReviewVerdict> {
    VERDICTS.iter().find(|(name, _)| *name == word).map(|(_, verdict)| verdict.clone())
}

fn merge_method_of(word: &str) -> Option<MergeMethod> {
    serde_json::from_value(Value::String(word.to_owned())).ok()
}

fn project_pick(project: &ProjectView) -> ProjectPick {
    ProjectPick { id: project.id.clone(), name: project.name.clone(), computer: project.computer.clone() }
}

fn made_commit(made: &GitCommitReply) -> MadeCommit {
    MadeCommit { oid: made.oid.clone(), subject: made.subject.clone(), files_changed: made.files_changed.as_f64() as u64 }
}

/// `{threadId, ...gone}`: a thread's delete as the answer writes it, its `on` left out on a sub-thread's.
fn deleted_value(thread_id: &str, gone: &ThreadDeleted, with_on: bool) -> Value {
    let mut fields = object_of(gone);
    if !with_on {
        fields.remove("on");
    }
    let mut map = Map::new();
    map.insert("threadId".to_owned(), json!(thread_id));
    map.extend(fields);
    Value::Object(map)
}

fn workspace_gone_value(d: &Dropping) -> Value {
    json!({ "workspaceId": d.workspace.id, "name": d.workspace.name, "machineId": d.workspace.machine_id, "threads": d.threads })
}

fn kept_result(text: String, structured: Value) -> CallToolResult {
    CallToolResult { is_error: Some(true), ..as_text(&text, structured) }
}

/// The second line of a question, read as `question.split("\n")[1]` reads it.
fn second_line(question: &str) -> &str {
    question.split('\n').nth(1).unwrap_or("undefined")
}

fn ended_lines_of(gone: &ThreadDeleted) -> Vec<String> {
    gone.ended.iter().flatten().map(ended_lines).collect()
}
