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

use std::sync::Arc;

use indexmap::IndexMap;
use wsp_words::exit::HostError;

use crate::host_lock::HostStarter;
use crate::parse_args::ParseArgsError;
use crate::signin_relay::RelayTerminal;
use crate::watch::WatchSignals;
use client::{CliIo, DialFn, LineVerb, Opener, Page, Verb};

pub use crate::verbs::agents_help::set::AGENTS_ON_WORDS;

/// `TerminalScheme.options` in order (`protocol`, `TerminalScheme`).
pub const TERMINAL_SCHEMES: [&str; 2] = ["light", "dark"];

/// `ALL_VERBS` (verbs.ts:366-374): computers, agents, projects, workspaces, threads, conversations, activity, each
/// group in its module's order.
pub fn all_verbs() -> Vec<Verb> {
    todo!("verbs.ts:366-374")
}

/// `withoutCloudFlags(v)` (verbs.ts:379-387): a `Cli` or `CliOnly` entry with its `cloud_flags` gone from its
/// options, from its usage (each ` [--<flag>...]` group, the first match only) and from its tool's input (an input
/// whose name with `_` as `-` is a dropped flag). Every other entry as it was.
pub fn without_cloud_flags(verb: &Verb) -> Verb {
    let _ = verb;
    todo!("verbs.ts:379-387")
}

/// `inCloudWords(v)` (verbs.ts:391-394): usage, about and the tool's description through `cloud_text(_, cloud_on)`.
pub fn in_cloud_words(verb: &Verb, cloud_on: bool) -> Verb {
    let _ = (verb, cloud_on);
    todo!("verbs.ts:391-394")
}

/// `VERBS` (verbs.ts:399) for a given cloud switch: every verb with the cloud on; with it off every verb but the
/// cloud's own, each without its cloud flags; then every one in its cloud words.
pub fn verbs_for(cloud_on: bool) -> Vec<Verb> {
    let _ = cloud_on;
    todo!("verbs.ts:399")
}

/// `VERBS` of this process: `verbs_for(crate::cloud::cloud_on())`, built once.
pub fn verbs() -> &'static [Verb] {
    todo!("verbs.ts:399")
}

/// `cloudLineOf(argv)` (verbs.ts:404-412), with the two switches handed in.
pub fn cloud_line_of(argv: &[String], cloud_on: bool, cloud_built: bool) -> Option<String> {
    let _ = (argv, cloud_on, cloud_built);
    todo!("verbs.ts:404-412")
}

/// `CLI_VERBS` (verbs.ts:415): the `Cli` and `CliOnly` entries of the table, in its order.
pub fn cli_verbs(table: &[Verb]) -> Vec<LineVerb<'_>> {
    let _ = table;
    todo!("verbs.ts:415")
}

/// `findVerb(argv)` (verbs.ts:418-423): the line verb whose words open argv, the longest name in characters first;
/// among names of one length, the table's order.
pub fn find_verb<'a>(table: &'a [Verb], argv: &[String]) -> Option<LineVerb<'a>> {
    let _ = (table, argv);
    todo!("verbs.ts:418-423")
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
    let _ = (verb, name, cloud_on);
    todo!("verbs.ts:608-611")
}

/// `aboutLines(verb, indent)` (verbs.ts:614): `wrap(indent + about, HELP_WIDTH, indent)`.
pub fn about_lines(about: &str, indent: &str) -> Vec<String> {
    let _ = (about, indent);
    todo!("verbs.ts:614")
}

/// `usageLines(usage, indent, lead)` (verbs.ts:619-632): each line of the usage wrapped on its own, a space inside
/// brackets held as U+00A0 while wrapping and given back after, the first line opened with `lead` ("  " by default).
pub fn usage_lines(usage: &str, indent: &str, lead: &str) -> Vec<String> {
    let _ = (usage, indent, lead);
    todo!("verbs.ts:619-632")
}

/// `verbHelp(page)` (verbs.ts:635-639): each line verb on the page (every one where None), its usage lines at
/// indent four spaces, its about at six, the verbs joined by one newline.
pub fn verb_help(table: &[Verb], page: Option<Page>) -> String {
    let _ = (table, page);
    todo!("verbs.ts:635-639")
}

/// `ownFlagsOf(verb)` (verbs.ts:642): the verb's own flags, in declaration order, none of COMMON's.
pub fn own_flags_of(verb: LineVerb<'_>) -> Vec<&'static str> {
    let _ = verb;
    todo!("verbs.ts:642")
}

/// `COMMON_FLAG_WORDS` (verbs.ts:647-652).
pub mod common_flag_words {
    pub const JSON: &str = "print the raw protocol values, one JSON object per line, with everything else on stderr";
    pub const STATE: &str = "the state file the host serves";
    pub const HOST: &str = "run the line against a host on your account, by the name wsp hosts lists it under; WSP_HOST names one for a whole shell";
    pub const HOST_SIDE: &str = "read to say this line runs at its own host's terminal; it dials no other";
}

/// `helpPage(usage, about, rows)` (verbs.ts:656-663): the usage at `usage: ` with a seven-space indent, the about
/// lines, then, where there are rows, a blank line and each row as `  <word padded to the widest + 4><says>`
/// wrapped at HELP_WIDTH under an indent of that width plus two.
pub fn help_page(usage: &str, about: &[String], rows: &[(String, String)]) -> String {
    let _ = (usage, about, rows);
    todo!("verbs.ts:656-663")
}

/// `verbPage(verb, host)` (verbs.ts:668-675): the verb's own flags one row each (`flag_says` or ""), then --json,
/// --state and --host with `host` as its words.
pub fn verb_page(verb: LineVerb<'_>, host: &str, cloud_on: bool) -> String {
    let _ = (verb, host, cloud_on);
    todo!("verbs.ts:668-675")
}

/// `verbUsage(word)` (verbs.ts:678-681): `usage: <usage>` of every line verb whose first word is `word`, one per
/// line; None where none is.
pub fn verb_usage(table: &[Verb], word: &str) -> Option<String> {
    let _ = (table, word);
    todo!("verbs.ts:678-681")
}

/// `parseRefusal(verb, e)` (verbs.ts:685-692): an unknown `--<flag>` that other line verbs read is
/// `foreign_flag_line("--<flag>", ["wsp <verb>"...], "wsp <this>")`; anything else the parser's own message; both
/// usage refusals with the fix `usage: <usage>`.
pub fn parse_refusal(table: &[Verb], verb: LineVerb<'_>, e: &ParseArgsError) -> HostError {
    let _ = (table, verb, e);
    todo!("verbs.ts:685-692")
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
    let _ = (table, verb, argv, io, state_path_of, deps);
    todo!("verbs.ts:694-764")
}
