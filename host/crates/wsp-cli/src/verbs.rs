// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs.ts` at 65f6b89c7: the verb table, the lookup, the help pages a verb prints and the
//! run from a parsed line to an exit code. Spec: `spec/verbs-table.md`.

pub mod activity;
pub mod activity_table;
pub mod agents;
pub mod agents_help;
pub mod client;
pub mod computers;
pub mod conversations;
pub mod io;
pub mod projects;
pub mod slate;
pub mod threads;
pub mod turns_help;
pub mod workspaces;
pub mod workspaces_help;

use std::sync::{Arc, Mutex, OnceLock};

use async_trait::async_trait;
use indexmap::IndexMap;
use regex::Regex;
use wsp_wire::types::env::CLOUD_ENV;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::words::thread::foreign_flag_line;

use crate::cloud_text::cloud_text;
use crate::host_lock::HostStarter;
use crate::hosts::{aim_name, aimed_host, host_side_only_fix, host_side_only_line, state_ignored_line, HostAim, HostPick};
use crate::init_layout::wrap;
use crate::parse_args::{parse_args, ParseArgsCode, ParseArgsError};
use crate::signin_relay::RelayTerminal;
use crate::watch::WatchSignals;
use client::{
    absolute_folder, common, dial_host, failed, flag, flag_set, host_schema_refusal, json_asked, optional_values, Again, CliIo, DialFn,
    DialOpts, Dialer, FlagTable, HostClient, LineVerb, Opener, Out, Page, Tool, Verb, VerbContext, VerbDeps,
};

pub use crate::verbs::agents_help::set::AGENTS_ON_WORDS;

/// `TerminalScheme.options` in order (`protocol`, `TerminalScheme`).
pub const TERMINAL_SCHEMES: [&str; 2] = ["light", "dark"];

/// `ALL_VERBS` (verbs.ts:366-374): computers, agents, projects, workspaces, threads, conversations, activity, each
/// group in its module's order.
pub fn all_verbs() -> Vec<Verb> {
    [
        computers::computer_verbs(),
        agents::agent_verbs(),
        projects::project_verbs(),
        workspaces::workspace_verbs(),
        threads::thread_verbs(),
        conversations::conversation_verbs(),
        activity::activity_verbs(),
    ]
    .concat()
}

fn cloud_flags_of(verb: &Verb) -> Vec<&'static str> {
    match verb {
        Verb::Cli(verb) => verb.cloud_flags.clone(),
        Verb::CliOnly(verb) => verb.cloud_flags.clone(),
        Verb::ToolOnly(_) | Verb::CommandTool(_) => Vec::new(),
    }
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn pad_end(text: &str, width: usize) -> String {
    format!("{text}{}", " ".repeat(width.saturating_sub(utf16_len(text))))
}

/// The line `opens` tests: its words are the first of argv, in order.
fn opens(name: &str, argv: &[String]) -> bool {
    name.split(' ').enumerate().all(|(at, word)| argv.get(at).map(String::as_str) == Some(word))
}

/// `withoutCloudFlags(v)` (verbs.ts:379-387): a `Cli` or `CliOnly` entry with its `cloud_flags` gone from its
/// options, from its usage (each ` [--<flag>...]` group, the first match only) and from its tool's input (an input
/// whose name with `_` as `-` is a dropped flag). Every other entry as it was.
pub fn without_cloud_flags(verb: &Verb) -> Verb {
    let dropped = cloud_flags_of(verb);
    if dropped.is_empty() {
        return verb.clone();
    }
    let is_dropped = |name: &str| dropped.iter().any(|flag| *flag == name);
    let usage_without = |usage: &str| {
        dropped.iter().fold(usage.to_owned(), |line, name| {
            let group = Regex::new(&format!(r" \[--{}\b[^\]]*\]", regex::escape(name))).expect("a flag name makes a pattern");
            group.replacen(&line, 1, "").into_owned()
        })
    };
    match verb {
        Verb::Cli(verb) => {
            let mut cut = verb.clone();
            cut.options.0.retain(|(name, _)| !is_dropped(name));
            cut.usage = usage_without(&verb.usage);
            cut.tool.input.retain(|input| !is_dropped(&input.name.replace('_', "-")));
            Verb::Cli(cut)
        }
        Verb::CliOnly(verb) => {
            let mut cut = verb.clone();
            cut.options.0.retain(|(name, _)| !is_dropped(name));
            cut.usage = usage_without(&verb.usage);
            Verb::CliOnly(cut)
        }
        other => other.clone(),
    }
}

fn worded_tool(tool: &Tool, cloud_on: bool) -> Tool {
    let mut worded = tool.clone();
    worded.description = cloud_text(&tool.description, cloud_on);
    for input in &mut worded.input {
        input.describe = input.describe.as_deref().map(|text| cloud_text(text, cloud_on));
    }
    worded
}

/// `inCloudWords(v)` (verbs.ts:391-394): usage, about and the tool's description through `cloud_text(_, cloud_on)`.
pub fn in_cloud_words(verb: &Verb, cloud_on: bool) -> Verb {
    match verb {
        Verb::Cli(verb) => {
            let mut worded = verb.clone();
            worded.usage = cloud_text(&verb.usage, cloud_on);
            worded.about = cloud_text(&verb.about, cloud_on);
            worded.tool = worded_tool(&verb.tool, cloud_on);
            Verb::Cli(worded)
        }
        Verb::CliOnly(verb) => {
            let mut worded = verb.clone();
            worded.usage = cloud_text(&verb.usage, cloud_on);
            worded.about = cloud_text(&verb.about, cloud_on);
            Verb::CliOnly(worded)
        }
        Verb::ToolOnly(verb) => {
            let mut worded = verb.clone();
            worded.tool = worded_tool(&verb.tool, cloud_on);
            Verb::ToolOnly(worded)
        }
        Verb::CommandTool(verb) => {
            let mut worded = verb.clone();
            worded.tool = worded_tool(&verb.tool, cloud_on);
            Verb::CommandTool(worded)
        }
    }
}

/// `VERBS` (verbs.ts:399) for a given cloud switch: every verb with the cloud on; with it off every verb but the
/// cloud's own, each without its cloud flags; then every one in its cloud words.
pub fn verbs_for(cloud_on: bool) -> Vec<Verb> {
    let kept: Vec<Verb> =
        if cloud_on { all_verbs() } else { all_verbs().iter().filter(|verb| !verb.is_cloud()).map(without_cloud_flags).collect() };
    kept.iter().map(|verb| in_cloud_words(verb, cloud_on)).collect()
}

/// `VERBS` of this process: `verbs_for(crate::cloud::cloud_on())`, built once.
pub fn verbs() -> &'static [Verb] {
    static VERBS: OnceLock<Vec<Verb>> = OnceLock::new();
    VERBS.get_or_init(|| verbs_for(crate::cloud::cloud_on())).as_slice()
}

/// `cloudLineOf(argv)` (verbs.ts:404-412), with the two switches handed in.
pub fn cloud_line_of(argv: &[String], cloud_on: bool, cloud_built: bool) -> Option<String> {
    if cloud_on || !cloud_built {
        return None;
    }
    let mut all = all_verbs();
    all.sort_by_key(|verb| std::cmp::Reverse(utf16_len(verb.name())));
    let verb = all.iter().find(|verb| opens(verb.name(), argv))?;
    if verb.is_cloud() {
        return Some(format!("wsp {}", verb.name()));
    }
    let typed = cloud_flags_of(verb)
        .into_iter()
        .find(|name| argv.iter().any(|w| w == &format!("--{name}") || w.starts_with(&format!("--{name}="))))?;
    Some(format!("wsp {} --{typed}", verb.name()))
}

/// `CLI_VERBS` (verbs.ts:415): the `Cli` and `CliOnly` entries of the table, in its order.
pub fn cli_verbs(table: &[Verb]) -> Vec<LineVerb<'_>> {
    table.iter().filter_map(Verb::line).collect()
}

/// `findVerb(argv)` (verbs.ts:418-423): the line verb whose words open argv, the longest name in characters first;
/// among names of one length, the table's order.
pub fn find_verb<'a>(table: &'a [Verb], argv: &[String]) -> Option<LineVerb<'a>> {
    let mut lines = cli_verbs(table);
    lines.sort_by_key(|verb| std::cmp::Reverse(verb.name().chars().count()));
    lines.into_iter().find(|verb| opens(verb.name(), argv))
}

/// `HELP_WIDTH` (verbs.ts:426).
pub const HELP_WIDTH: usize = 80;

/// `FLAG_WORDS` (verbs.ts:440-604), in the TS object's order, cloud spans not yet cut. Copied by script.
pub fn flag_words() -> Vec<(&'static str, String)> {
    use wsp_catalog::git_hosts::THREAD_AGENTS;
    vec![
        ("agent", format!("which agent runs the thread, by its catalog id ({}); the project's own default without it", THREAD_AGENTS.join(", "))),
        ("agents", "the agents whose sessions for that folder travel with it, by catalog id, comma separated; every one that has them without it".to_string()),
        ("access", "how far the agent may go without asking: ask, auto-edit, full or plan, refused where the agent has no such mode; without it, the project's, else the agent's default, else full".to_string()),
        ("cwd", "the folder on the machine to work in; the project's folder without it".to_string()),
        ("exec cwd", "the folder to run the command in, absolute; the folder the thread works in without it".to_string()),
        ("detach", "print the thread's id and return, leaving the reply to the thread's finished line".to_string()),
        ("conversations agent", "one agent's conversations alone, by its catalog id; every agent's without it".to_string()),
        ("resume", "a conversation the agent kept in the project's folder outside wsp, by the id wsp conversations lists: the thread opens on it, in the folder it ran in, with its newest messages above the first prompt".to_string()),
        ("copy", "with --resume, continue on a copy of the conversation the agent makes, the original left as it was: what a conversation open in another app needs".to_string()),
        ("thread deny reason", "what the agent should do instead, in your words; it reads them with the refusal, as it reads the reason typed in the app".to_string()),
        ("thread settle finished", "settle the finished threads under each thread named, and not the thread itself; a failed one stays for you to read".to_string()),
        ("thread settle processes", "stop ends every process a thread to settle still runs beside its agent, by identity, before it settles; keep leaves them running; refused without it while any run".to_string()),
        ("delete processes", "stop ends every process the thread still runs beside its agent, by identity, before it goes; keep leaves them running; refused without it while any run".to_string()),
        ("activity computer", "the computer to read: this one, which is the default, or a box wsp computers lists".to_string()),
        ("activity wide", "each process's pid and whole command line, the run script and input pump of a turn on rows of their own, nothing cut to the terminal's width".to_string()),
        ("activity end computer", "the computer the process runs on, which must be this one; a box's go with its thread's settle and --processes stop".to_string()),
        ("activity end started", "the process's start as wsp activity --json lists it, startedAt in epoch milliseconds; a pid that started otherwise is refused".to_string()),
        ("stop subagent", "stop one of the agent's own subagents alone, by its id in the SUBAGENT column of wsp threads; the turn and its other subagents run on".to_string()),
        ("effort", "how hard the agent thinks, by its own word (low, medium, high, xhigh, max); absent on a new thread means the agent's default; on send, the thread's own".to_string()),
        ("run beside", "a thread whose folder the new thread starts in, by its id or a prefix, on the computer it runs on; never with a project or --branch".to_string()),
        ("run branch", "a branch other than the one the project's folder has checked out: the thread runs in the worktree holding it, made under wsp's folder from the folder's current commit for a new branch; with --on, the branch the sub-thread makes and pushes; wsp/<slug> without it".to_string()),
        ("run on", "boat, to start the thread as a sub-thread of the thread asking on a machine of its own on Boat, forked from your image; it fetches your pushed branch, or the ref --from names, and pushes its own".to_string()),
        ("run from", "with --on, a branch, tag or commit the remote already holds, origin/main or a commit id, that the sub-thread starts from in place of your branch, which then need not be pushed".to_string()),
        ("run size", "with --on, the machine's size as <cpu>x<memGb>: 2x4, 4x8, 8x16 or 16x32, the ones the plan sells; Boat's own size setting without it".to_string()),
        ("run cwd", "a folder inside the project or one of its worktrees, absolute, to start the thread in; the project's folder without it".to_string()),
        ("run fork", "a thread to fork, by its id or a prefix: the new thread carries its conversation through one finished turn, on its agent and in its folder, or with --branch in a new worktree with that turn's files; never with a project, --beside, --cwd or --replaces".to_string()),
        ("run at", "with --fork, the finished turn the fork carries through, counted from 1 as wsp thread read lists them; the latest without it".to_string()),
        ("worktree remove force", "remove it over files no commit holds, which go with it".to_string()),
        ("projects remove force", "remove it even where its checkout on a computer of yours holds work no remote has, which goes with it".to_string()),
        ("export from", "the folder on the machine to bring home; the project registered for the folder you named without it".to_string()),
        ("recipes save from", "the computer whose picks the recipe is saved from, by the name wsp computers shows; it follows the recipe from then on".to_string()),
        ("computers set ssh", "the login user@host the host reaches a computer you added by over ssh from now, saved only once the computer it reaches reads as that same one".to_string()),
        ("computers set name", "what to call a computer you added from now, which every listing then shows; its id, its projects and its machines stay as they are".to_string()),
        ("computers set recipe", "the saved recipe it follows from now, by the name wsp recipes shows, or none to keep what it has and follow nothing".to_string()),
        ("force", "build again even where the place already holds this version".to_string()),
        ("hidden", "list the folders whose names start with a dot too".to_string()),
        ("usage range", "the days what was used is read over: day (today, the default), week (the last seven) or month (the last thirty); the accounts' limits are the same whatever it says".to_string()),
        ("usage by", "how what was used is split: agent (the default), account, computer, project or model".to_string()),
        ("folders on", "the computer whose folders to list, by the name wsp computers shows; a box you added answers from its own disk, and this computer is listed without it".to_string()),
        ("agents on", AGENTS_ON_WORDS.to_string()),
        ("usage reset on", "the computer to spend it on, by the name wsp computers shows, one of the account's own that holds its login; the first of those that is connected without it".to_string()),
        ("usage reset credit", "the reset to spend, by the id Codex lists it under; whichever Codex picks without it".to_string()),
        ("skills on", AGENTS_ON_WORDS.to_string()),
        ("skills show on", AGENTS_ON_WORDS.to_string()),
        ("skills add on", AGENTS_ON_WORDS.to_string()),
        ("skills remove on", AGENTS_ON_WORDS.to_string()),
        ("skills disable on", AGENTS_ON_WORDS.to_string()),
        ("skills enable on", AGENTS_ON_WORDS.to_string()),
        ("skills search limit", "how many skills to answer, from 1 to 50; 20 without it".to_string()),
        ("skills show project", "the project's skill of that name rather than the one that is not a project's: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("skills remove project", "the project's skill of that name rather than the one that is not a project's: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("skills add agent", "an agent to put the skill in, by its catalog id; repeats, and every agent whose folder is there without it".to_string()),
        ("skills add project", "put it in a project rather than the home: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("servers on", AGENTS_ON_WORDS.to_string()),
        ("servers tools on", AGENTS_ON_WORDS.to_string()),
        ("servers signin on", AGENTS_ON_WORDS.to_string()),
        ("servers signin agent", "the agent whose config names the server, by its catalog id as wsp servers shows it".to_string()),
        ("servers tools agent", "the agent whose config names the server, by its catalog id as wsp servers shows it".to_string()),
        ("servers tools refresh", "start the server again even where an answer from the last three minutes stands".to_string()),
        ("agents key api-key", "the value typed is the agent's API key, not the token its own command mints; for a computer picked to use the key".to_string()),
        ("servers tools project", "the project on the computer --on names whose server it is, by name; a thread finds its own project's servers".to_string()),
        ("servers add on", AGENTS_ON_WORDS.to_string()),
        ("servers add agent", "the agent whose config takes the server, by its catalog id".to_string()),
        ("servers add command", "the line the server runs, its program and arguments in one quoted value, split as a shell splits it and nothing expanded; or --url".to_string()),
        ("servers add env", "a variable the server is given, or one the address names as ${NAME}, by its name, its value read off the same name in this terminal's environment; an argument or the address may name it as ${NAME}; repeats".to_string()),
        ("servers add url", "the server's https address; or --command".to_string()),
        ("servers add header", "<name>=<VARIABLE>, a header sent to the address with its value read off that variable in this terminal's environment; repeats".to_string()),
        ("servers add project", "put it in a project's file rather than the agent's own: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("servers remove on", AGENTS_ON_WORDS.to_string()),
        ("servers remove agent", "the agent whose config names the server, by its catalog id as wsp servers shows it".to_string()),
        ("servers remove scope", "user, home, local or project, as wsp servers shows it; user without it".to_string()),
        ("servers remove project", "the project scope: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("servers disable on", AGENTS_ON_WORDS.to_string()),
        ("servers disable agent", "the agent whose config names the server, by its catalog id as wsp servers shows it".to_string()),
        ("servers disable scope", "user, home, local or project, as wsp servers shows it; user without it".to_string()),
        ("servers disable project", "the project scope: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("servers enable on", AGENTS_ON_WORDS.to_string()),
        ("servers enable agent", "the agent whose config names the server, by its catalog id as wsp servers shows it".to_string()),
        ("servers enable scope", "user, home, local or project, as wsp servers shows it; user without it".to_string()),
        ("servers enable project", "the project scope: alone with a thread, the thread's own project, or the project's name with --on".to_string()),
        ("plugins on", AGENTS_ON_WORDS.to_string()),
        ("plugins disable on", AGENTS_ON_WORDS.to_string()),
        ("plugins disable agent", "the agent whose plugin it is, by its catalog id as wsp plugins shows it".to_string()),
        ("plugins enable on", AGENTS_ON_WORDS.to_string()),
        ("plugins enable agent", "the agent whose plugin it is, by its catalog id as wsp plugins shows it".to_string()),
        ("repos", "every git repo under the home folder instead of one level, most recently used first".to_string()),
        ("fast", "run the turn in the agent's fast mode, on a model that offers one; refused naming the model otherwise".to_string()),
        ("file", "a file on this computer to send with the message: an image goes as an image, any other file lands in the thread's folder and the message names its path; repeats".to_string()),
        ("last", "the final reply alone, the whole message the thread's finished line carries".to_string()),
        ("slate write check", "validate and sketch the slate, storing nothing".to_string()),
        ("slate write set", "with --check, a value to rehearse against, like '$i=2' or 'picked=1'; repeats".to_string()),
        ("slate write press", "with --check, a piece to rehearse a press on, after the --set values".to_string()),
        ("slate write row", "the row index of the --press piece inside a list, from 0".to_string()),
        ("slate write action", "which row action of a --press table to rehearse, from 0".to_string()),
        ("if-version", "the version a read printed; refused with V750 when the document moved past it".to_string()),
        ("slate state start", "a run to start now that the person said \"Always in this thread\" to; any other answers held; repeats".to_string()),
        ("slate read values", "a path to resolve now, like '$check.exit' or usage.week.percent, or * for every bound one; repeats".to_string()),
        ("slate read no-text", "leave out the slate in the JSX-like form a patch is written against".to_string()),
        ("slate read document", "also print the stored JSON document".to_string()),
        ("slate read no-sketch", "leave the sketch out".to_string()),
        ("threads", "how many threads may run on that computer at once; a new one waits past it".to_string()),
        ("machines", "how many machines may run on that cloud at once".to_string()),
        ("spend", "the dollars a day that cloud may spend before it starts no new machine".to_string()),
        ("computers set size", "on a cloud, the size a sub-thread's machine gets when the thread starting it names none, as <cpu>x<memGb>; 2x4 until it is set".to_string()),
        ("delete-after", "on a cloud, the minutes a finished sub-thread's machine stays before it is deleted, 0 to 1440; 30 until it is set".to_string()),
        ("nap", "the minutes a quiet machine there runs before it naps, or off".to_string()),
        ("turn-limit", "the hours one turn there may run before it is stopped, or off; off on a computer you own<!-- cloud --> and 6 on a cloud<!-- /cloud --> until it is set".to_string()),
        ("reset", "a setting to take back to its default, by its flag's word; repeats".to_string()),
        ("max-depth", "how many levels of threads may stand under the root thread; defaults to 2".to_string()),
        ("max-machines", "how many machines may stand at once under one root thread; defaults to 3".to_string()),
        ("model", "the model the turn runs on, by the agent's own slug (claude-sonnet-5); the thread's own without it".to_string()),
        ("name", "what to call the new machine; <source>-fork without it".to_string()),
        ("notify", "where each turn's end is sent, a thread's id or me; repeats".to_string()),
        ("replace", "overwrite what is already at the destination".to_string()),
        ("replaces", "a stopped or failed thread this one restarts, by id or a prefix; the host settles it once this one starts".to_string()),
        ("scheme", format!("which side of a light:...,dark:... theme to read; {}", TERMINAL_SCHEMES.join(" or "))),
        ("send", "a message for the new machine's first thread, with run's own flags after it".to_string()),
        ("size", "the machine size as <cpu>x<memGb>, like 2x4; a size the provider does not offer is refused naming the ones it does".to_string()),
        ("threads wait tail", "print the reply's last line alone, the line a notify sends, rather than the whole reply".to_string()),
        ("timeout", "how long to wait before answering that they are still running".to_string()),
        ("title", "what to call the thread; the agent names it from its first message without one".to_string()),
        ("commit message", "the commit message, a subject line, a blank line, then the body, -m for short; the thread's agent drafts it without one".to_string()),
        ("commit file", "a file to commit, by path from the checkout's top, once per file; every changed file without one".to_string()),
        ("fix check", "the failed check to send, by its name on the pull request; without it the folder is updated from its base and a conflict is sent".to_string()),
        ("fix child", "a child thread whose merge into this thread's folder stopped on conflicts, by its id or a prefix; this thread's agent is asked to merge it and resolve them, and nothing is updated".to_string()),
        ("merge method", "merge, squash or rebase; the repository's own default without one".to_string()),
        ("merge when-checks-pass", "merge once the checks pass rather than now, where the repository allows it".to_string()),
        ("start project", "the project by name or id where two computers hold the link's repository; this computer's without it".to_string()),
        ("review agent", "the reviewing agent, codex or claude, each at its read-only access; codex without it".to_string()),
        ("review post verdict", "comment, approve or request-changes; the draft's own, the reviewer's word unless you changed it, without it".to_string()),
        ("review post summary", "the review's summary, written over the draft's".to_string()),
        ("tree", "indent the threads an agent opened under the one that opened them".to_string()),
        ("watch", "draw the table again every second where it stands, until Ctrl-C; it needs a terminal to redraw on".to_string()),
        ("yes", "go ahead without being asked".to_string()),
        ("agents set model", "the model a new thread on it starts on, by the agent's own slug".to_string()),
        ("agents set effort", "the effort a new thread on it starts at, by the agent's own word".to_string()),
        ("agents set access", "how far a new thread on it may go without asking: ask, auto-edit, full or plan, refused where the agent has no such mode".to_string()),
        ("agents set hide", "a model to take off its picker; a start still takes it by name; repeats".to_string()),
        ("agents set show", "a hidden model to put back on its picker; repeats".to_string()),
        ("agents set order", "the models its picker lists first, comma separated; the rest follow in the agent's own order".to_string()),
        ("agents set add-model", "a model id the binary does not list, which a start may then name; repeats".to_string()),
        ("agents set drop-model", "a model id added before, taken back off; repeats".to_string()),
        ("agents set reset", "put a field back on the agent's own: model, effort, access or models; repeats".to_string()),
        ("agents setup on", "the computer, by the name wsp computers shows; this computer without it".to_string()),
        ("agents setup enable", "offer the agent there again".to_string()),
        ("agents setup disable", "take the agent off that computer: the app's lists drop it there and a start naming it is refused".to_string()),
        ("agents setup program", "the program run in the agent's place there, a path or a word on its PATH".to_string()),
        ("agents setup config", "the folder there the agent keeps its config, sessions and sign-in in, absolute and under that computer's home; sign it in again there".to_string()),
        ("agents setup arg", "a word added to every turn's launch there; repeats, and replaces any set before".to_string()),
        ("agents setup env", "a variable every launch there carries, by name, never one that decides how the process starts (PATH, LD_*, NODE_OPTIONS and the like); its value is asked for where nothing echoes it; repeats".to_string()),
        ("agents setup unset-env", "a variable to take off its launch there, by name; repeats".to_string()),
        ("agents setup reset", "put a field back on the agent's own: program, config or args; repeats".to_string()),
        ("projects set agent", format!("the agent a new thread on it runs ({})", THREAD_AGENTS.join(", "))),
        ("projects set model", "the model a new thread on it starts on, by the agent's own slug".to_string()),
        ("projects set effort", "the effort a new thread on it starts at, by the agent's own word".to_string()),
        ("projects set access", "how far a new thread on it may go without asking: ask, auto-edit, full or plan".to_string()),
        ("projects set after-worktree", "a shell line a new worktree of the project runs once at its top, after each ecosystem's own install".to_string()),
        ("projects set tracker", "a repo, owner/name, besides the project's own that gh on a sub-thread's machine may comment on; repeats, and the ones named are the whole list".to_string()),
        ("projects set reset", "put a field back on the layer below: agent, model, effort or access, take the after-worktree command away with after-worktree, or empty the trackers with trackers; repeats".to_string()),
        ("projects seed keep", "one more path off the menu that goes to a sub-thread's machine; given once per path".to_string()),
        ("projects seed cut", "one path off the menu that stays on this computer; given once per path".to_string()),
        ("projects seed none", "send no git-ignored file to a sub-thread's machine".to_string()),
        ("projects seed yes", "save the picks; without it the menu is printed and nothing is saved".to_string()),
    ]
}

/// `flagSays(verb, name)` (verbs.ts:608-611): the `"<verb> <name>"` row, else the `name` row, through
/// `cloud_text(_, cloud_on)`; None where neither exists.
pub fn flag_says(verb: &str, name: &str, cloud_on: bool) -> Option<String> {
    let words = flag_words();
    let joined = format!("{verb} {name}");
    let said = words.iter().find(|(key, _)| *key == joined.as_str()).or_else(|| words.iter().find(|(key, _)| *key == name))?;
    Some(cloud_text(&said.1, cloud_on))
}

/// `aboutLines(verb, indent)` (verbs.ts:614): `wrap(indent + about, HELP_WIDTH, indent)`.
pub fn about_lines(about: &str, indent: &str) -> Vec<String> {
    wrap(&format!("{indent}{about}"), HELP_WIDTH, indent)
}

/// `usageLines(usage, indent, lead)` (verbs.ts:619-632): each line of the usage wrapped on its own, a space inside
/// brackets held as U+00A0 while wrapping and given back after, the first line opened with `lead` ("  " by default).
pub fn usage_lines(usage: &str, indent: &str, lead: &str) -> Vec<String> {
    usage
        .split('\n')
        .enumerate()
        .flat_map(|(at, line)| {
            let mut depth = 0i64;
            let grouped: String = line
                .chars()
                .map(|c| {
                    match c {
                        '[' => depth += 1,
                        ']' => depth -= 1,
                        _ => {}
                    }
                    if c == ' ' && depth > 0 {
                        '\u{a0}'
                    } else {
                        c
                    }
                })
                .collect();
            let first = if at == 0 { lead } else { "" };
            wrap(&format!("{first}{grouped}"), HELP_WIDTH, indent).into_iter().map(|l| l.replace('\u{a0}', " ")).collect::<Vec<_>>()
        })
        .collect()
}

/// `verbHelp(page)` (verbs.ts:635-639): each line verb on the page (every one where None), its usage lines at
/// indent four spaces, its about at six, the verbs joined by one newline.
pub fn verb_help(table: &[Verb], page: Option<Page>) -> String {
    cli_verbs(table)
        .into_iter()
        .filter(|verb| page.is_none_or(|wanted| verb.page() == wanted))
        .map(|verb| {
            let mut lines = usage_lines(verb.usage(), "    ", "  ");
            lines.extend(about_lines(verb.about(), "      "));
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `ownFlagsOf(verb)` (verbs.ts:642): the verb's own flags, in declaration order, none of COMMON's.
pub fn own_flags_of(verb: LineVerb<'_>) -> Vec<&'static str> {
    verb.options().0.iter().map(|(name, _)| *name).filter(|name| !["host", "state", "json", "help"].contains(name)).collect()
}

/// `COMMON_FLAG_WORDS` (verbs.ts:647-652).
pub mod common_flag_words {
    pub const JSON: &str = "print the raw protocol values, one JSON object per line, with everything else on stderr";
    pub const STATE: &str = "the state file the host serves";
    pub const HOST: &str =
        "run the line against a host on your account, by the name wsp hosts lists it under; WSP_HOST names one for a whole shell";
    pub const HOST_SIDE: &str = "read to say this line runs at its own host's terminal; it dials no other";
}

/// `helpPage(usage, about, rows)` (verbs.ts:656-663): the usage at `usage: ` with a seven-space indent, the about
/// lines, then, where there are rows, a blank line and each row as `  <word padded to the widest + 4><says>`
/// wrapped at HELP_WIDTH under an indent of that width plus two.
pub fn help_page(usage: &str, about: &[String], rows: &[(String, String)]) -> String {
    let width = rows.iter().map(|(word, _)| utf16_len(word)).max().unwrap_or(0) + 4;
    let mut lines = usage_lines(usage, "       ", "usage: ");
    lines.extend(about.iter().cloned());
    if !rows.is_empty() {
        lines.push(String::new());
        for (word, says) in rows {
            lines.extend(wrap(&format!("  {}{says}", pad_end(word, width)), HELP_WIDTH, &" ".repeat(width + 2)));
        }
    }
    lines.join("\n")
}

/// `verbPage(verb, host)` (verbs.ts:668-675): the verb's own flags one row each (`flag_says` or ""), then --json,
/// --state and --host with `host` as its words.
pub fn verb_page(verb: LineVerb<'_>, host: &str, cloud_on: bool) -> String {
    let mut rows: Vec<(String, String)> = own_flags_of(verb)
        .into_iter()
        .map(|name| (format!("--{name}"), flag_says(verb.name(), name, cloud_on).unwrap_or_default()))
        .collect();
    rows.push(("--json".to_owned(), common_flag_words::JSON.to_owned()));
    rows.push(("--state".to_owned(), common_flag_words::STATE.to_owned()));
    rows.push(("--host".to_owned(), host.to_owned()));
    help_page(verb.usage(), &about_lines(verb.about(), "  "), &rows)
}

/// `verbUsage(word)` (verbs.ts:678-681): `usage: <usage>` of every line verb whose first word is `word`, one per
/// line; None where none is.
pub fn verb_usage(table: &[Verb], word: &str) -> Option<String> {
    let usages: Vec<String> = cli_verbs(table)
        .into_iter()
        .filter(|verb| verb.name().split(' ').next() == Some(word))
        .map(|verb| format!("usage: {}", verb.usage()))
        .collect();
    if usages.is_empty() {
        None
    } else {
        Some(usages.join("\n"))
    }
}

/// `parseRefusal(verb, e)` (verbs.ts:685-692): an unknown `--<flag>` that other line verbs read is
/// `foreign_flag_line("--<flag>", ["wsp <verb>"...], "wsp <this>")`; anything else the parser's own message; both
/// usage refusals with the fix `usage: <usage>`.
pub fn parse_refusal(table: &[Verb], verb: LineVerb<'_>, e: &ParseArgsError) -> HostError {
    let usage = format!("usage: {}", verb.usage());
    let unknown = Regex::new(r"^Unknown option '--([^']+)'").expect("a fixed pattern");
    let named = match e.code {
        ParseArgsCode::UnknownOption => unknown.captures(&e.message).map(|found| found[1].to_owned()),
        ParseArgsCode::InvalidOptionValue => None,
    };
    let Some(named) = named else {
        return usage_refusal(&e.message, &usage);
    };
    let readers: Vec<String> = cli_verbs(table)
        .into_iter()
        .filter(|other| other.name() != verb.name() && other.options().has(&named))
        .map(|other| format!("wsp {}", other.name()))
        .collect();
    if readers.is_empty() {
        return usage_refusal(&e.message, &usage);
    }
    usage_refusal(&foreign_flag_line(&format!("--{named}"), &readers, &format!("wsp {}", verb.name())), &usage)
}

/// What `runVerb` takes beside the line (verbs.ts:694, `Pick<VerbDeps, ...>`).
#[derive(Clone, Default)]
pub struct RunDeps {
    pub cwd: Option<String>,
    pub env: IndexMap<String, String>,
    pub start: Option<Arc<dyn HostStarter>>,
    pub signals: Option<Arc<dyn WatchSignals>>,
    pub dial: Option<Arc<dyn DialFn>>,
    pub elsewhere: bool,
    pub terminal: Option<Arc<tokio::sync::Mutex<RelayTerminal>>>,
    pub open: Option<Arc<dyn Opener>>,
}

/// `runVerb(verb, argv, io, statePathOf, deps)` (verbs.ts:694-764). `argv` opens with the verb's words; `table` is
/// what the parse refusal reads for the other verbs that take a flag.
pub async fn run_verb(
    table: &[Verb],
    verb: LineVerb<'_>,
    argv: &[String],
    io: Arc<dyn CliIo>,
    state_path_of: &(dyn Fn(Option<&str>) -> String + Send + Sync),
    deps: RunDeps,
) -> i32 {
    let name = verb.name();
    let cloud = table.iter().any(Verb::is_cloud);
    let json = json_asked(argv);
    let own_words = name.split(' ').count();
    let rest = argv.get(own_words..).unwrap_or_default();
    let parsed = match parse_args(&optional_values(rest, verb.options()), &with_common(verb.options())) {
        Ok(parsed) => parsed,
        Err(e) => return failed(io.as_ref(), json, &parse_refusal(table, verb, &e), ""),
    };
    let flags = parsed.values;
    let args = parsed.positionals;
    if let Err(e) = absolute_folder(flag(&flags, "cwd")) {
        return failed(io.as_ref(), json, &usage_refusal(&e.message, &format!("usage: {}", verb.usage())), "");
    }
    let host_side = verb.host_side();
    if flag_set(&flags, "help") {
        let host = if host_side.is_some() { common_flag_words::HOST_SIDE } else { common_flag_words::HOST };
        io.log(&verb_page(verb, host, cloud));
        return 0;
    }
    let state_path = state_path_of(flag(&flags, "state"));
    let pick = HostPick { host: flag(&flags, "host").map(str::to_owned), env: Some(deps.env.clone()), home: None };
    let aim = match aimed_host(&state_path, &pick) {
        Ok(aim) => aim,
        Err(e) => return failed(io.as_ref(), flag_set(&flags, "json"), &e, &format!("wsp {name}: ")),
    };
    let elsewhere = !matches!(aim, HostAim::Here);
    if let (Some(side), true) = (host_side, elsewhere) {
        let refusal = usage_refusal(&host_side_only_line(name, &aim_name(&aim)), &host_side_only_fix(Some(side)));
        return failed(io.as_ref(), flag_set(&flags, "json"), &refusal, "");
    }
    let state_note = if elsewhere && flag(&flags, "state").is_some() { Some(state_ignored_line(&aim_name(&aim))) } else { None };
    let dialer = Arc::new(LineDialer {
        state_path: state_path.clone(),
        aim: aim.clone(),
        io: io.clone(),
        any_release: verb.any_release().is_some(),
        start: if verb.starts_no_host().is_none() { deps.start.clone() } else { None },
        dial: deps.dial.clone(),
        state_note,
        noted: Mutex::new(false),
        client: Mutex::new(None),
    });
    let mut env = deps.env.clone();
    if cloud {
        env.insert(CLOUD_ENV.to_owned(), "1".to_owned());
    } else {
        env.shift_remove(CLOUD_ENV);
    }
    let ctx = VerbContext {
        deps: VerbDeps {
            state_path,
            cwd: deps.cwd.clone(),
            env,
            dialer: dialer.clone(),
            host_wait_ms: None,
            start: deps.start.clone(),
            signals: deps.signals.clone(),
            dial: None,
            elsewhere: deps.elsewhere,
            terminal: deps.terminal.clone(),
            open: deps.open.clone(),
        },
        args,
        flags: flags.clone(),
        io: io.clone(),
        out: Out::formatter(io.clone(), flag_set(&flags, "json")),
        aim,
        usage: verb.usage().to_owned(),
    };
    let run = verb.run();
    let code = match run(&ctx).await {
        Ok(code) => code,
        Err(e) => {
            let refused = host_schema_refusal(&e, verb.usage());
            failed(io.as_ref(), flag_set(&flags, "json"), refused.as_ref().unwrap_or(&e), &format!("wsp {name}: "))
        }
    };
    let open = dialer.client.lock().unwrap().take();
    if let Some(client) = open {
        client.close();
    }
    code
}

/// The flags every line takes, then the verb's own: `{ ...COMMON, ...verb.options }`, a verb's flag that COMMON also
/// names taking COMMON's place in the order.
fn with_common(own: &FlagTable) -> FlagTable {
    let mut merged = common();
    for (name, row) in &own.0 {
        match merged.0.iter_mut().find(|(known, _)| known == name) {
            Some(slot) => slot.1 = row.clone(),
            None => merged.0.push((*name, row.clone())),
        }
    }
    merged
}

/// The socket a line dials once and hands back to every later plain call, and dials again only past a stop
/// (`client` of `runVerb`, verbs.ts:746-755).
struct LineDialer {
    state_path: String,
    aim: HostAim,
    io: Arc<dyn CliIo>,
    any_release: bool,
    start: Option<Arc<dyn HostStarter>>,
    dial: Option<Arc<dyn DialFn>>,
    state_note: Option<String>,
    noted: Mutex<bool>,
    client: Mutex<Option<Arc<dyn HostClient>>>,
}

#[async_trait]
impl Dialer for LineDialer {
    async fn client(&self, again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        if let Some(note) = &self.state_note {
            let mut noted = self.noted.lock().unwrap();
            if !*noted {
                *noted = true;
                self.io.error(note);
            }
        }
        if again.is_none() {
            let held = self.client.lock().unwrap().clone();
            if let Some(client) = held {
                return Ok(client);
            }
        }
        let io = self.io.clone();
        let opts = DialOpts {
            aim: Some(self.aim.clone()),
            say: Some(Arc::new(move |line: &str| io.error(line))),
            any_release: self.any_release,
            deadline_ms: again.map(|again| again.within_ms),
            start: if again.is_none() { self.start.clone() } else { None },
            ..DialOpts::default()
        };
        let client = match &self.dial {
            Some(dial) => dial.dial(&self.state_path, opts).await?,
            None => dial_host(&self.state_path, opts).await?,
        };
        *self.client.lock().unwrap() = Some(client.clone());
        Ok(client)
    }
}
