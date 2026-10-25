// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/computers.ts` at 65f6b89c7: `wsp computers`, `computers set`, the recipes, `wsp usage`
//! and `usage reset`, and the `add` tool. Spec: `spec/computers.md`.

use wsp_wire::views::place_more::{PlaceSettingWord, PlaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_words::exit::HostError;

use super::client::*;

/// What a `--size` word asks of the host (computers.ts:36-42): the size to set, and the resets with `size` added
/// where the word names no size and the row takes none.
#[derive(Clone, Debug, PartialEq)]
pub struct SizeAsked {
    pub size: Option<WorkspaceSize>,
    pub reset: Vec<PlaceSettingWord>,
}

/// `sizeAsked` (computers.ts:36-42).
pub fn size_asked(place: &PlaceView, word: Option<&str>, reset: &[PlaceSettingWord]) -> Result<SizeAsked, HostError> {
    let _ = (place, word, reset);
    todo!("computers.ts:36-42")
}

/// The entries of `verbs/computers.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn computer_verbs() -> Vec<Verb> {
    vec![
    // verbs/computers.ts:46
    Verb::Cli(CliVerb {
        name: "computers",
        usage: "wsp computers".to_string(),
        about: "your computers: this Mac, each box you added<!-- cloud --> and each cloud account<!-- /cloud -->, with what each has, whether it is connected, how many machines it holds and what runs there against its cap<!-- cloud -->, and what each cloud spent today<!-- /cloud -->".to_string(),
        page: Page::Front,
        options: FlagTable(vec![]),
        run: run_computers,
        tool: Tool {
            description: "Every computer this host holds, which is the whole of where work can run: the computer the app runs on<!-- cloud -->, each box joined to it and each cloud account<!-- /cloud --><!-- no cloud --> and each box joined to it<!-- /no cloud -->. A row carries what that computer last reported (cores, memory, free disk, the engine it has for a project's own containers) and whether it is connected right now<!-- cloud -->; a cloud row carries its hourly rate<!-- /cloud -->. Every row carries its cap, threads at once on a computer<!-- cloud --> and machines at once and spend per day on a cloud<!-- /cloud --> (the number the person set, else one thread per 2 GB of memory up to twice its cores<!-- cloud -->, and 3 machines and $10 a day<!-- /cloud -->), and running, every thread running there now on a computer<!-- cloud --> or the machines holding a slot on a cloud<!-- /cloud -->; a row whose running meets its cap is full.<!-- cloud --> spend holds one row per cloud, what it has spent today (since midnight where the host runs) and this month and what it burns an hour now; a cloud whose spend today reaches its spend per day is at its limit and starts no new machine until midnight, while the machines already running there go on.<!-- /cloud --> A row whose copy of the image is building says which stage it is at, and one whose last build stopped says why. A project lives on a computer: on the computer the app runs on its threads run in the project's folder, and on a box<!-- cloud --> or a cloud<!-- /cloud --> they run on a machine forked from the image with the project inside. pending holds every add that has not reached Set up: how far it got, what was chosen for it, and why it stopped where one did.<!-- boat --> A thread's own token is answered Boat once it is added, where run with on starts a sub-thread, and from a folder the computers its projects are on, with no pending and no road.<!-- /boat -->".to_string(),
            input: vec![],
            output: vec!["computers", "spend", "pending"],
            stream: vec![],
            call: call_computers,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:66
    Verb::Cli(CliVerb {
        name: "computers set",
        usage: "wsp computers set <computer> [--name <new>] [--ssh <login>] [--threads <n>] [--machines <n>] [--spend <usd>] [--size <cpu>x<memGb>] [--delete-after <minutes>] [--nap <minutes>|off] [--turn-limit <hours>|off] [--max-machines <n>] [--max-depth <n>] [--recipe <name>|none] [--reset <setting>]...".to_string(),
        about: "what you set on one of your computers: its name, the ssh login it is reached by, how many threads run there at once, how long a quiet machine there runs before it naps, how long one turn there may run, the caps on agents starting agents, the recipe it follows<!-- boat -->, and on a cloud how many machines at once, how much it spends a day, the size a sub-thread's machine gets and how long a finished one stays<!-- /boat -->; --reset takes a setting back to its default".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("name", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("ssh", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("threads", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("machines", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("spend", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("size", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("delete-after", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("nap", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("turn-limit", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("max-machines", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("max-depth", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("recipe", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("reset", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None })]),
        run: run_computers_set,
        tool: Tool {
            description: "Sets what the person may set on one computer and answers its row as it now reads, the same row computers lists: name, what a computer the person added is called from now, in every listing and wherever a computer is named, its id, its projects and its machines staying as they are, refused for the computer the app runs on<!-- cloud -->, for a cloud<!-- /cloud --> and for a name another computer answers to; ssh, the login user@host the host reaches it by over ssh from now, for an update or a remove while its link is down and for the forward it dials back through, saved only once the computer that login reaches reads as this same one, its place file naming this computer and this host; threads, how many threads may run there at once, a new one waiting past it, except a turn a thread there follows to its end, not detached, which runs in that thread's slot (one per 2 GB of that computer's memory up to twice its cores until it is set); nap, the minutes a machine there with no window of its own runs with no turn and no work before it naps and stops costing anything, waking on the next message (20 until it is set, 0 never naps it), which every machine there counts again from now and which the computer the app runs on does not take, since its threads run in folders; turn_limit, the hours one turn there may run before it is stopped, a send carrying it on from where it stopped (none on a computer the person owns<!-- cloud --> and 6 on a cloud<!-- /cloud --> until it is set, 0 is none), read at each turn's start; max_machines and max_depth, how many machines and levels deep the agents there may start where the folder or the machine they run in holds no caps of its own (up to 3 machines and 2 levels deep, until it is set), read at every ask so one made before the change follows it<!-- boat -->; machines and spend on a cloud, how many machines may run there at once and the dollars a day it may spend before it starts no new machine (3 and $10 until they are set); size on a cloud, the size a sub-thread's machine gets when the thread starting it names none, as <cpu>x<memGb>, one of the sizes that cloud's row lists (the size its provider builds at until it is set); delete_after on a cloud, the minutes a finished sub-thread's machine stays before it is deleted, read at each finish, 0 deleting it once it is done (30 until it is set)<!-- /boat -->. A setting left out keeps what stands, and each word under reset takes that setting back to its default. The row carries cap, what runs there now, capDefault, what it reads by default, and settings, what the person set. A setting the computer's kind does not take, and a call that sets nothing, are refused in one line.".to_string(),
            input: vec![
                ToolInput { name: "computer", kind: InputKind::String, optional: false, describe: Some("the computer, by the name computers lists or its id".to_string()) },
                ToolInput { name: "name", kind: InputKind::String, optional: true, describe: Some("what to call a computer the person added from now".to_string()) },
                ToolInput { name: "ssh", kind: InputKind::String, optional: true, describe: Some("the login user@host the host reaches that computer by over ssh from now, saved once it reads as the same computer".to_string()) },
                ToolInput { name: "threads", kind: InputKind::Integer { min: Some(1) }, optional: true, describe: Some("how many threads may run on that computer at once".to_string()) },
                ToolInput { name: "machines", kind: InputKind::Integer { min: Some(1) }, optional: true, describe: Some("how many machines may run on that cloud at once".to_string()) },
                ToolInput { name: "spend", kind: InputKind::Number, optional: true, describe: Some("the dollars a day that cloud may spend before it starts no new machine".to_string()) },
                ToolInput { name: "size", kind: InputKind::String, optional: true, describe: Some("on a cloud, the size a sub-thread's machine gets when the thread starting it names none, as <cpu>x<memGb>".to_string()) },
                ToolInput { name: "delete_after", kind: InputKind::Integer { min: Some(0) }, optional: true, describe: Some("on a cloud, the minutes a finished sub-thread's machine stays before it is deleted; 0 deletes it once it is done".to_string()) },
                ToolInput { name: "nap", kind: InputKind::Integer { min: Some(0) }, optional: true, describe: Some("the minutes a quiet machine there runs before it naps; 0 never naps it".to_string()) },
                ToolInput { name: "turn_limit", kind: InputKind::Integer { min: Some(0) }, optional: true, describe: Some("the hours one turn there may run before it is stopped; 0 never stops one".to_string()) },
                ToolInput { name: "max_machines", kind: InputKind::Integer { min: Some(0) }, optional: true, describe: Some("how many machines may stand at once under one root thread there".to_string()) },
                ToolInput { name: "max_depth", kind: InputKind::Integer { min: Some(1) }, optional: true, describe: Some("how many levels deep the tree under a root thread there may go".to_string()) },
                ToolInput { name: "recipe", kind: InputKind::String, optional: true, describe: Some("the saved recipe it follows from now, as recipes lists it, or none: one it follows syncs to it, a change to the recipe reaching it with no step, and none keeps what it has".to_string()) },
                ToolInput { name: "reset", kind: InputKind::StringList { min: None }, optional: true, describe: Some("the settings to take back to their defaults, by the same words".to_string()) },
            ],
            output: vec!["computer"],
            stream: vec![],
            call: call_computers_set,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:141
    Verb::CommandTool(CommandToolVerb {
        name: "add",
        tool: Tool {
            description: "Adds a computer of the person's over ssh and sets it up from a saved recipe, as `wsp add <user@host> --recipe <name>` does: address is user@host or an alias from the ssh config, and the host logs in, checks it can run wsp there (root, or a login whose sudo runs it as root, systemd with cgroup v2, room on the disk), installs wsp, waits for it to dial back, then puts on the base tools and everything the recipe picks. With resume, address names a computer already added, or one that joined and waits on its picks, and sets it up again: from recipe where one is named, else from what it holds, running only what is missing, a sign-in that waited or ran out among it. This tool never waits on the person: it answers at the first sign-in waiting on them (the page and the code are in waiting, to hand over), at the end, or after ten minutes, and is called again with resume for the next of those; with later it does not stop at a sign-in and leaves it waiting. setup is the setup's steps as the computer's row holds them, each with its state and what it took; an install that fails is refused with the step it failed at. A login whose sudo asks for a password is refused with the line that says where the person types it, at a terminal or in the app, since no password travels through this tool. A computer this host has never dialled needs host_key, as the person read it off that computer, or the add is refused with the key it answered with. A project, a provider's key and the join code are not added here: projects_add records a project, and the other two belong at the host's own terminal.".to_string(),
            input: vec![
                ToolInput { name: "address", kind: InputKind::String, optional: false, describe: Some("user@host, an alias from the ssh config, or with resume a computer already added".to_string()) },
                ToolInput { name: "recipe", kind: InputKind::String, optional: true, describe: Some("the saved recipe to set it up from, as recipes lists it".to_string()) },
                ToolInput { name: "later", kind: InputKind::Boolean, optional: true, describe: Some("go on past a sign-in that waits on the person and leave it waiting".to_string()) },
                ToolInput { name: "resume", kind: InputKind::Boolean, optional: true, describe: Some("set up a computer already added, or one that joined and waits on its picks".to_string()) },
                ToolInput { name: "name", kind: InputKind::String, optional: true, describe: Some("what to call the computer here; what its address calls it without one".to_string()) },
                ToolInput { name: "ssh_port", kind: InputKind::Integer { min: Some(1) }, optional: true, describe: Some("the port ssh dials it on (default 22)".to_string()) },
                ToolInput { name: "ssh_key", kind: InputKind::String, optional: true, describe: Some("the key file ssh logs in with".to_string()) },
                ToolInput { name: "host_key", kind: InputKind::String, optional: true, describe: Some("the host key of a computer this one has never dialled, as read off that computer".to_string()) },
            ],
            output: vec!["computer", "setup", "waiting"],
            stream: vec!["setup", "waiting"],
            call: call_add,
        },
    }),
    // verbs/computers.ts:174
    Verb::Cli(CliVerb {
        name: "recipes",
        usage: "wsp recipes".to_string(),
        about: "your saved recipes: what each puts on a computer in one line, and the computers that follow it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_recipes,
        tool: Tool {
            description: "Every recipe this host keeps: a named pick of what goes on a computer of the person's (agents and how each signs in, MCP servers per agent, CLIs by the manager they came from, skills, plugins, folders to move over as projects, and the git, shell and GitHub configs), chosen from what the computer the app runs on has. Each carries summary, one line of what it holds, machines, the computers that follow it, since a recipe edit reaches them, and savedAt, when its file was last written. A recipe holds names and never a secret; a token reaches a computer only in the environment of a run there.".to_string(),
            input: vec![],
            output: vec!["recipes"],
            stream: vec![],
            call: call_recipes,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:197
    Verb::Cli(CliVerb {
        name: "recipes show",
        usage: "wsp recipes show <name>".to_string(),
        about: "one recipe whole: every row it holds by kind, the computers that follow it, and the hash it resolves to on this computer now".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_recipes_show,
        tool: Tool {
            description: "One recipe by its name: file is the recipe as saved, every row by kind (agents with signin vault, machine, token or key, mcp with the agents each server goes to, clis with via, the manager it came from, and needs where it builds with the C toolchain, skills with from, the folder it is read from here, plugins, folders with from, name, icon, hue and keep, configs git, shell and github, github with signin vault, machine or skip), machines the computers that follow it, and hash what it resolves to on the computer the app runs on now: the versions, the skill folders and the configs read there, which a computer that applied it is held against.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the recipe's name, as recipes lists it".to_string()) },
            ],
            output: vec!["recipe", "hash"],
            stream: vec![],
            call: call_recipes_show,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:221
    Verb::Cli(CliVerb {
        name: "recipes save",
        usage: "wsp recipes save <name> --from <computer>".to_string(),
        about: "saves what one of your computers was set up with as a recipe under that name, and that computer follows it from then on".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("from", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_recipes_save,
        tool: Tool {
            description: "Saves the picks one of the person's computers was set up with as a recipe under name, rewriting a recipe of that name whole, and that computer follows it from then on, so an edit to the recipe reaches it. Refused for a name with no letter or digit in it, for the computer the app runs on, and for a computer set up before picks were kept. The answer is the recipe as saved.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("what to call the recipe".to_string()) },
                ToolInput { name: "from", kind: InputKind::String, optional: false, describe: Some("the computer whose picks to save, by the name computers lists".to_string()) },
            ],
            output: vec!["recipe"],
            stream: vec![],
            call: call_recipes_save,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:246
    Verb::Cli(CliVerb {
        name: "recipes remove",
        usage: "wsp recipes remove <name> [--yes]".to_string(),
        about: "takes a recipe away; the computers that followed it keep what they have and follow none".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_recipes_remove,
        tool: Tool {
            description: "Takes a recipe's file away. The computers that followed it keep everything it put there and follow none from then on, so nothing reaches them from it again. The answer is the recipe as it stood, machines naming the computers it was taken off.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the recipe's name, as recipes lists it".to_string()) },
            ],
            output: vec!["recipe"],
            stream: vec![],
            call: call_recipes_remove,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:272
    Verb::Cli(CliVerb {
        name: "usage",
        usage: "wsp usage [--range day|week|month] [--by agent|account|computer|project|model|source]".to_string(),
        about: "what each agent account signed in on any of your computers may still use, and what was used over a day, a week or a month split one way, the work your agents logged outside wsp counted with wsp's threads as on the Usage page (--by source shows how much is outside); two answers, never added together".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("range", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("by", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_usage,
        tool: Tool {
            description: "Two answers that are never added together. accounts: every agent account signed in on any computer, the same one on three computers once, each with how much of its plan's windows is used and when each starts again, as that agent printed them in the last turn wsp ran on it; an agent that prints none reports no plan limit, a sign-in by API key pays per token with no plan window, and an account no turn has run on yet has no reading. used: the tokens the turns used over the range (today, the last seven days or the last thirty), split by agent, account, computer, project, model or source (a model once for each agent that ran it, and a row by agent, account or model naming its agent; every split counts the turns wsp's threads ran with the work the agents on this computer and on each joined one logged outside wsp, as each computer's own daemon read it, as the Usage page does, and by source the two stand as two rows), each row with the cost its agent reported, a list price off one table for every token and what the cache saved, input counting the cached and the written tokens, the turns wsp ran, and a series over the range with one line per row.".to_string(),
            input: vec![
                ToolInput { name: "range", kind: InputKind::Enum(vec!["day", "week", "month"]), optional: true, describe: Some("day (the default), week or month".to_string()) },
                ToolInput { name: "by", kind: InputKind::Enum(vec!["agent", "account", "computer", "project", "model", "source"]), optional: true, describe: Some("agent (the default), account, computer, project, model or source".to_string()) },
            ],
            output: vec!["accounts", "used"],
            stream: vec![],
            call: call_usage,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/computers.ts:294
    Verb::CliOnly(CliOnlyVerb {
        name: "usage reset",
        usage: "wsp usage reset <account> [--credit <id>] [--on <computer>] [--yes]".to_string(),
        about: "spends one of the resets a Codex account has banked, on a computer of yours that holds its login, after reading the account there: its five-hour and weekly windows start again now; asks first unless --yes".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("credit", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_usage_reset,
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
        cli_only: "spends a reset the person owns; the skill already says an agent does not read the person's accounts, and a thread's socket is refused every usage op",
        host_side: None,
    }),
    ]
}

/// The `computers` tool's call (`verbs/computers.ts:62-62`).
fn call_computers(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:62-62")
    })
}

/// `wsp computers` (`verbs/computers.ts:51-56`).
fn run_computers(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:51-56")
    })
}

/// The `computers_set` tool's call (`verbs/computers.ts:121-137`).
fn call_computers_set(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:121-137")
    })
}

/// `wsp computers set` (`verbs/computers.ts:71-100`).
fn run_computers_set(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:71-100")
    })
}

/// The `add` tool's call (`verbs/computers.ts:158-170`).
fn call_add(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:158-170")
    })
}

/// The `recipes` tool's call (`verbs/computers.ts:190-193`).
fn call_recipes(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:190-193")
    })
}

/// `wsp recipes` (`verbs/computers.ts:179-184`).
fn run_recipes(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:179-184")
    })
}

/// The `recipes_show` tool's call (`verbs/computers.ts:214-217`).
fn call_recipes_show(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:214-217")
    })
}

/// `wsp recipes show` (`verbs/computers.ts:202-208`).
fn run_recipes_show(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:202-208")
    })
}

/// The `recipes_save` tool's call (`verbs/computers.ts:239-242`).
fn call_recipes_save(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:239-242")
    })
}

/// `wsp recipes save` (`verbs/computers.ts:226-233`).
fn run_recipes_save(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:226-233")
    })
}

/// The `recipes_remove` tool's call (`verbs/computers.ts:265-268`).
fn call_recipes_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:265-268")
    })
}

/// `wsp recipes remove` (`verbs/computers.ts:251-260`).
fn run_recipes_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:251-260")
    })
}

/// The `usage` tool's call (`verbs/computers.ts:290-290`).
fn call_usage(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let _ = (args, deps);
        todo!("verbs/computers.ts:290-290")
    })
}

/// `wsp usage` (`verbs/computers.ts:277-284`).
fn run_usage(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:277-284")
    })
}

/// `wsp usage reset` (`verbs/computers.ts:300-312`).
fn run_usage_reset(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let _ = ctx;
        todo!("verbs/computers.ts:300-312")
    })
}
