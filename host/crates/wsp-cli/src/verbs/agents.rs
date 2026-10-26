// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/agents.ts` at 65f6b89c7: the agents, skills, MCP servers and plugins on a computer or
//! where a thread runs, and an agent's defaults, setup, sign-in and key. Spec: `spec/agents-skills.md` and
//! `spec/agents-servers.md`. The run and call bodies are in `agents/run.rs` and `agents/call.rs`.

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

use wsp_catalog::catalog::agent_name;
use wsp_wire::types::agents_report::AgentsReport;
use wsp_wire::types::check::{issues_of, parse};
use wsp_wire::views::place_more::PlaceView;
use wsp_words::exit::{usage_refusal, HostError};
use wsp_words::place_name::{here_name, is_provider_place};
use wsp_words::projects::runs_in_folder;
use wsp_words::words::computer::THIS_COMPUTER;
use wsp_words::workspace_state::workspace_kind;

use super::agents_help::{
    agents_target, report_facts, ProjectAsked, ProjectValue, ServerCommand, ServerValues, AGENTS_READ_WORDS, SKILL_CHANGE_WORDS,
};
use super::client::*;
use super::turns_help::thread_at;
use super::workspaces_help::HOST_SIDE_VAULT;

mod call;
mod run;

use self::call::*;
use self::run::*;

/// `signInWorkspace` (agents.ts:36-43): the workspace a thread's sign-in runs on, refused before anything runs
/// where its machine is at a cloud provider.
pub async fn sign_in_workspace(client: &dyn HostClient, agent: &str, thread: &str) -> Result<String, HostError> {
    let (_, workspace) = thread_at(client, thread, "wsp agents signin", true).await?;
    let places = places_of(client).await?;
    // A machine whose record names no computer is at this host's own provider; one that names a provider is at that one.
    let place = workspace.place.as_ref().and_then(|id| places.iter().find(|p| &p.id == id));
    if !runs_in_folder(&workspace_kind(workspace.kind.as_ref())) && (workspace.place.is_none() || place.is_some_and(is_provider_place)) {
        let here = match here_name(&places) {
            named if named.is_empty() => THIS_COMPUTER.to_owned(),
            named => named,
        };
        return Err(usage_refusal(&brokered_sign_in_refusal(&agent_name(agent)), &brokered_sign_in_fix(&here)));
    }
    Ok(workspace.id)
}

/// `brokeredSignInRefusal` (agents-signin.ts:346).
fn brokered_sign_in_refusal(agent: &str) -> String {
    format!("{agent} does not sign in on a cloud machine.")
}

/// `brokeredSignInFix` (agents-signin.ts:347).
fn brokered_sign_in_fix(here: &str) -> String {
    format!("Sign in on {here} instead: cloud threads use it through wsp.")
}

/// The entries of `verbs/agents.ts`, in the TS table's order (its order is `tools/list`'s).
pub fn agent_verbs() -> Vec<Verb> {
    vec![
    // verbs/agents.ts:47
    Verb::Cli(CliVerb {
        name: "agents",
        usage: "wsp agents [<thread>] [--on <computer>]".to_string(),
        about: "the coding agents on this computer, a box you added or where a thread runs: each one's version and the newest out, whether it is signed in there, and whether it carries the wsp tools".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_agents,
        tool: Tool {
            description: format!("{}{}", "The coding agents the catalog knows, as they stand on one computer or where a thread runs: whether each is on that login's PATH and where, the version its command answers, the newest its vendor publishes as this host last read it (asked of npm, GitHub or the vendor from this host alone, kept a day, never with Newest agent versions off in Settings > Privacy or WSP_UPDATE_CHECK=0) and the version wsp's install pins, its sign-in there (signed in, signed in with a token or key from this host's vault, not signed in, or unknown) and how it stands in the status command's own words (signInDetail), the kind of that login (signInKind: api-key, subscription or oauth) and the plan it names (signInPlan), how a person signs it in, whether its threads there get the wsp tools (one of its MCP config files names the wsp server, or on any computer but the one the app runs on, its launch hands the server over), the vendor's own command that brings it up to the newest where it is older (update, which wsp shows and never runs), and on a computer how the person set it to run there (setup: on or off, the program, the config folder, the launch words and the names of its variables, never a value). ", AGENTS_READ_WORDS),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["target", "home", "user", "readAt", "stale", "pauseMode", "refused", "projects", "reach", "agents"],
            stream: vec![],
            call: call_agents,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:69
    Verb::Cli(CliVerb {
        name: "skills",
        usage: "wsp skills [<thread>] [--on <computer>]".to_string(),
        about: "the skills on this computer, a box you added or where a thread runs, each by name with every folder it lives in and which agent loads it from there".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_skills,
        tool: Tool {
            description: format!("{}{}", "Every skill on one computer or where a thread runs, one row per folder name: its description off its SKILL.md, every folder it lives in with the agent whose own folder that is (none for the shared ~/.agents/skills) and where a folder links to, and whether it is the person's own, a project's inside a thread, or a plugin's. ", AGENTS_READ_WORDS),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["target", "home", "user", "readAt", "stale", "pauseMode", "refused", "projects", "reach", "skills"],
            stream: vec![],
            call: call_skills,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:91
    Verb::Cli(CliVerb {
        name: "skills search",
        usage: "wsp skills search <query> [--limit <n>]".to_string(),
        about: "searches skills.sh for skills by their words, each with how often it was installed and the id wsp skills add takes".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("limit", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_skills_search,
        tool: Tool {
            description: "Skills on skills.sh whose words match the query, most installed first as skills.sh ranks them: each one's name, the repo it comes from, how many times it was installed, and the id skills_add takes. The host asks skills.sh; an empty query is refused.".to_string(),
            input: vec![
                ToolInput { name: "query", kind: InputKind::String, optional: false, describe: Some("the words to search skills.sh for".to_string()) },
                ToolInput { name: "limit", kind: InputKind::Integer { min: Some(1) }, optional: true, describe: Some("how many to answer, 20 without it".to_string()) },
            ],
            output: vec!["skills"],
            stream: vec![],
            call: call_skills_search,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:116
    Verb::Cli(CliVerb {
        name: "skills show",
        usage: "wsp skills show <skill> [<thread>] [--on <computer>] [--project [<name>]]".to_string(),
        about: "prints a skill's SKILL.md: one on skills.sh by its <owner>/<repo>/<skill> before it is installed, or one already on this computer, a box you added or where a thread runs by its name".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") })]),
        run: run_skills_show,
        tool: Tool {
            description: "A skill's SKILL.md as text, its first 64 KB and the whole file's size: a skill on skills.sh by its <owner>/<repo>/<skill>, read by the host with nothing installed, or a skill already on one computer or where a thread runs by its name. Nothing in it runs.".to_string(),
            input: vec![
                ToolInput { name: "skill", kind: InputKind::String, optional: false, describe: Some("an <owner>/<repo>/<skill> off skills_search, or the name of a skill skills lists".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("the project's skill of that name rather than the one that is not a project's: true with a thread, the thread's own project, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["text", "size"],
            stream: vec![],
            call: call_skills_show,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:139
    Verb::Cli(CliVerb {
        name: "skills add",
        usage: "wsp skills add <skill> [<thread>] [--on <computer>] [--agent <id>]... [--project [<name>]]".to_string(),
        about: "installs a skill off skills.sh by its <owner>/<repo>/<skill> into the shared skills folder, with a link or a copy for each agent named that does not read that folder; every file is checked first and lands as a plain file that runs nothing".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("agent", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") })]),
        run: run_skills_add,
        tool: Tool {
            description: "Installs one skill off skills.sh on one computer or where a thread runs, as the login it was added with: its files land once in ~/.agents/skills/<name> (the project's .agents/skills with project, from a thread), and each agent named that does not read that folder gets a link to it or a copy in its own skills folder; with no agent named, every agent whose own folder's home is there gets it. The download is checked whole before anything lands: every path plain and inside the skill, at most 200 files, 1 MB each and 5 MB in all, a SKILL.md at its root; every file lands 0644 and nothing in the skill runs. A skill already there is refused rather than written over.".to_string(),
            input: vec![
                ToolInput { name: "skill", kind: InputKind::String, optional: false, describe: Some("the skill's <owner>/<repo>/<skill>, as skills_search answers it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "agent", kind: InputKind::StringList { min: None }, optional: true, describe: Some("the catalog ids of the agents to put it in; every agent whose folder is there without it".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("put it in a project rather than the home: true for the thread's own, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["path", "agents"],
            stream: vec![],
            call: call_skills_add,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:170
    Verb::Cli(CliVerb {
        name: "skills remove",
        usage: "wsp skills remove <name> [<thread>] [--on <computer>] [--project [<name>]] [--yes]".to_string(),
        about: "removes a skill by its name: every folder it lives in and every link to it, where a link's own folder elsewhere stays".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") }), ("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_skills_remove,
        tool: Tool {
            description: format!("{}{}", "Removes one skill on one computer or where a thread runs by its name, as the login it was added with: every folder skills lists for it and every link to it; a folder a link points to outside the skills folders stays. ", SKILL_CHANGE_WORDS),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the skill's name, as skills lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("the project's skill of that name rather than the one that is not a project's: true with a thread, the thread's own project, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["removed"],
            stream: vec![],
            call: call_skills_remove,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:196
    Verb::Cli(CliVerb {
        name: "skills disable",
        usage: "wsp skills disable <name> [<thread>] [--on <computer>]".to_string(),
        about: "turns a skill off by its name, its SKILL.md renamed SKILL.md.off where it lives, so no agent loads it until it is turned on".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_skills_disable,
        tool: Tool {
            description: format!("{}{}", "Turns one skill off on one computer or where a thread runs by its name, as the login it was added with: its SKILL.md is renamed SKILL.md.off in each folder it really lives in, which every link to it follows, and no agent config is edited. A project's skill lives in the repo and is refused. ", SKILL_CHANGE_WORDS),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the skill's name, as skills lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["paths"],
            stream: vec![],
            call: call_skills_disable,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:196
    Verb::Cli(CliVerb {
        name: "skills enable",
        usage: "wsp skills enable <name> [<thread>] [--on <computer>]".to_string(),
        about: "turns a skill that was turned off on again, its SKILL.md.off renamed back".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_skills_enable,
        tool: Tool {
            description: format!("{}{}", "Turns one skill on again on one computer or where a thread runs by its name, as the login it was added with: its SKILL.md is renamed back from SKILL.md.off in each folder it really lives in, which every link to it follows, and no agent config is edited. A project's skill lives in the repo and is refused. ", SKILL_CHANGE_WORDS),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the skill's name, as skills lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["paths"],
            stream: vec![],
            call: call_skills_enable,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:220
    Verb::Cli(CliVerb {
        name: "servers",
        usage: "wsp servers [<thread>] [--on <computer>]".to_string(),
        about: "the MCP servers the agents on this computer, a box you added or where a thread runs are set up with: how each is reached, the file it is defined in and its sign-in as its config says it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_servers,
        tool: Tool {
            description: format!("{}{}", "Every MCP server each agent's own config file defines on one computer or where a thread runs, and a thread's project files: the agent, the file, how it is reached (its command with every value hidden, or its url's host), the names of the variables it sets or reads and never their values, whether the file switches it off, whether wsp's recipe put it there on a box, and its sign-in as the config alone says it: open for a command or a fixed header, unknown for a remote server until something connects. On a machine at a provider, which holds no sign-in or key of the person's, a server that needs one carries off with the sentence saying why, and no server there carries a sign-in line. On any computer but the one the app runs on it also lists the wsp server each thread's launch there hands an agent that takes servers on its launch, marked launch, with no file, and never turned off or removed. ", AGENTS_READ_WORDS),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["target", "home", "user", "readAt", "stale", "pauseMode", "refused", "projects", "reach", "servers"],
            stream: vec![],
            call: call_servers,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:242
    Verb::CliOnly(CliOnlyVerb {
        name: "agents signin",
        usage: "wsp agents signin <agent> [<thread>]".to_string(),
        about: "signs an agent in on this computer or where a thread runs, its own sign-in run there and shown in this terminal; a box you added takes wsp add <computer> --sign-in <agent>".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_agents_signin,
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
        cli_only: "runs the agent's own sign-in in a terminal a person types into, which is where the ones that ask them to pick a provider are answered",
        host_side: None,
    }),
    // verbs/agents.ts:260
    Verb::CliOnly(CliOnlyVerb {
        name: "agents key",
        usage: "wsp agents key <agent> [--api-key]".to_string(),
        about: "puts an agent's token or API key into this host's vault, typed where nothing echoes it; Claude Code's token is the one claude setup-token prints, Codex signs in on this computer with its own login, which machines at a provider use through wsp, and --api-key takes the agent's API key instead".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("api-key", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_agents_key,
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
        cli_only: "takes a token typed at the host's own terminal into its vault, which is the person's to hand over",
        host_side: Some(HOST_SIDE_VAULT),
    }),
    // verbs/agents.ts:286
    Verb::Cli(CliVerb {
        name: "agents addtools",
        usage: "wsp agents addtools <agent>".to_string(),
        about: "writes the wsp server into an agent's own config on this computer, the entry wsp mcp install writes, with the wsp skill beside it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_agents_addtools,
        tool: Tool {
            description: "Writes the wsp server into one agent's own MCP config on the computer the app runs on, the same entry wsp mcp install writes, with the wsp skill beside it, and answers the file. The wsp tools go into an agent's config on this computer; a thread on any other computer is handed them with every turn.".to_string(),
            input: vec![
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent, as agents lists it".to_string()) },
            ],
            output: vec!["file"],
            stream: vec![],
            call: call_agents_addtools,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:309
    Verb::Cli(CliVerb {
        name: "agents default",
        usage: "wsp agents default <agent>".to_string(),
        about: "the agent a new thread runs when neither the line nor its project names one; the catalog's first without it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![]),
        run: run_agents_default,
        tool: Tool {
            description: "Sets the agent a new thread runs when neither its start nor its project names one, on every computer and in the app alike; the catalog's first agent without it. A project's own agent, set with projects_set, wins over it, and an agent turned off on a computer is passed over there. An agent this host runs no thread of is refused naming the ones it runs.".to_string(),
            input: vec![
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent, one of claude, codex, opencode, cursor".to_string()) },
            ],
            output: vec!["defaultAgent"],
            stream: vec![],
            call: call_agents_default,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:329
    Verb::Cli(CliVerb {
        name: "agents set",
        usage: "wsp agents set <agent> [--model <slug>] [--effort <word>] [--access <word>] [--hide <model>]... [--show <model>]... [--order <model,model>] [--add-model <id>]... [--drop-model <id>]... [--reset <field>]...".to_string(),
        about: "an agent's defaults on every computer: the model, effort and access a new thread on it starts on, and which models its picker lists".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("model", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("effort", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("access", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("hide", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("show", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("order", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("add-model", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("drop-model", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("reset", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None })]),
        run: run_agents_set,
        tool: Tool {
            description: "Sets one agent's defaults, the same on every computer and in the app: the model and effort a new thread on it starts on, its access, and its model picker: models hidden from it (a start still takes one by name), the order it lists models in, and model ids the binary does not list that the person runs anyway, which a start then takes. A project's own model, effort and access, set with projects_set, win over these, and what a start names wins over both. An access word the agent maps to none of its modes is refused naming the ones it takes. reset puts a field back on the agent's own: model, effort, access or models.".to_string(),
            input: vec![
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent, as agents lists it".to_string()) },
                ToolInput { name: "model", kind: InputKind::String, optional: true, describe: Some("the model a new thread on it starts on, by the agent's own slug".to_string()) },
                ToolInput { name: "effort", kind: InputKind::String, optional: true, describe: Some("the effort a new thread on it starts at, by the agent's own word, where its model takes that one".to_string()) },
                ToolInput { name: "access", kind: InputKind::String, optional: true, describe: Some("how far the agent may go without asking, in wsp's words: ask (asks about each action that needs permission), auto-edit (edits files without asking), full (every action without asking) or plan (reads and proposes, changes nothing); each is mapped to the agent's own mode and refused where the agent has none, and a harness's own spelling is refused. Absent means the project's access, else the one set for that agent, else full".to_string()) },
                ToolInput { name: "hide", kind: InputKind::StringList { min: None }, optional: true, describe: Some("models to take off its picker, by slug".to_string()) },
                ToolInput { name: "show", kind: InputKind::StringList { min: None }, optional: true, describe: Some("hidden models to put back on its picker, by slug".to_string()) },
                ToolInput { name: "order", kind: InputKind::StringList { min: None }, optional: true, describe: Some("the models its picker lists first, in this order, by slug; the rest follow in the agent's own order".to_string()) },
                ToolInput { name: "add_model", kind: InputKind::StringList { min: None }, optional: true, describe: Some("model ids the binary does not list that a start may name and the picker shows".to_string()) },
                ToolInput { name: "drop_model", kind: InputKind::StringList { min: None }, optional: true, describe: Some("model ids added before, taken back off".to_string()) },
                ToolInput { name: "reset", kind: InputKind::Json, optional: true, describe: Some("fields to put back on the agent's own: model, effort, access, models".to_string()) },
            ],
            output: vec!["agent", "defaults"],
            stream: vec![],
            call: call_agents_set,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:384
    Verb::Cli(CliVerb {
        name: "agents setup",
        usage: "wsp agents setup <agent> [--on <computer>] [--enable | --disable] [--program <path>] [--config <folder>] [--arg <word>]... [--env <NAME>]... [--unset-env <NAME>]... [--reset <field>]...".to_string(),
        about: "how an agent runs on one computer: on or off there, the program run in its place, its config folder, words added to every launch and variables every launch carries, each value typed where nothing echoes it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("enable", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("disable", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None }), ("program", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("config", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("arg", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("env", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("unset-env", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("reset", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None })]),
        run: run_agents_setup,
        tool: Tool {
            description: "Sets how one agent runs on one computer, this computer without on: whether it is offered there at all (off, the app's lists drop it there and a start naming it is refused naming the computer), the program run in its place, the folder it keeps its config, sessions and sign-in in (an agent with no variable for one is refused, and a folder that is not under that computer's home once its links are followed, the home itself, or wsp's own folder; a login kept under the old folder does not follow, so sign it in again there), words added to every turn's launch, and variables taken off its launch. A variable that decides how the process starts or what it loads (PATH, HOME, LD_ and DYLD_ ones, NODE_OPTIONS and the like) is never set, nor one of wsp's own. A variable's value is never taken here: the person types it at wsp agents setup --env, where nothing echoes it. Answers the agent's row as that computer's read now gives it, its variables by name alone. reset puts back the agent's own program, config or args.".to_string(),
            input: vec![
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent, as agents lists it".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer it runs on, by the name computers lists; absent is the computer the app runs on".to_string()) },
                ToolInput { name: "enabled", kind: InputKind::Boolean, optional: true, describe: Some("false turns the agent off on that computer, true back on".to_string()) },
                ToolInput { name: "program", kind: InputKind::String, optional: true, describe: Some("the program run in the agent's place there, a path or a word on that computer's PATH".to_string()) },
                ToolInput { name: "config", kind: InputKind::String, optional: true, describe: Some("the folder on that computer the agent keeps its config, sessions and sign-in in, absolute".to_string()) },
                ToolInput { name: "args", kind: InputKind::StringList { min: None }, optional: true, describe: Some("words added to every turn's launch there, each passed as one word; they replace any set before".to_string()) },
                ToolInput { name: "unset_env", kind: InputKind::StringList { min: None }, optional: true, describe: Some("variables to take off its launch there, by name".to_string()) },
                ToolInput { name: "reset", kind: InputKind::Json, optional: true, describe: Some("fields to put back on the agent's own: program, config, args".to_string()) },
            ],
            output: vec!["agent"],
            stream: vec![],
            call: call_agents_setup,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:446
    Verb::CliOnly(CliOnlyVerb {
        name: "servers signin",
        usage: "wsp servers signin <name> --agent <id> [<thread>] [--on <computer>]".to_string(),
        about: "signs one MCP server in by its agent's own command for it, run where the server is set up and shown in this terminal".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_servers_signin,
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
        cli_only: "runs the harness's own sign-in for the server in a terminal a person types into, where the page's answer is pasted",
        host_side: None,
    }),
    // verbs/agents.ts:466
    Verb::Cli(CliVerb {
        name: "servers tools",
        usage: "wsp servers tools <name> --agent <id> [<thread>] [--on <computer>] [--project <name>] [--refresh]".to_string(),
        about: "starts one MCP server once where it is set up and lists its tools with their descriptions, and says whether it needs a sign-in".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") }), ("refresh", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_servers_tools,
        tool: Tool {
            description: "One MCP server's tools with their descriptions, and its sign-in as that one connect found it (open, signed in, needs a sign-in, failed, or unknown), with why nothing came back where nothing did. Starts that one server once on that computer or where the thread runs, as the login it was added with and with the command and variables its agent's config gives it, or asks its address once from there, and stops it within 20 seconds; the answer is the server's state, the one the app shows, and stands three minutes unless refreshed or a sign-in there ends; an edited entry is asked again. A server behind a sign-in its agent holds brings no list, since no login file is read: Claude Code is asked for its word on it, and for any other agent it answers unknown, naming that agent as the one holding the sign-in. A napping machine is not woken.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the server's name, as servers lists it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose config names it, as servers lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "project", kind: InputKind::String, optional: true, describe: Some("the project on that computer whose server it is, by the name projects lists, with on; a thread finds its own project's servers".to_string()) },
                ToolInput { name: "refresh", kind: InputKind::Boolean, optional: true, describe: Some("start it again even where an answer from the last three minutes stands".to_string()) },
            ],
            output: vec!["auth", "tools", "holder", "refused", "readAt"],
            stream: vec![],
            call: call_servers_tools,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:500
    Verb::Cli(CliVerb {
        name: "servers add",
        usage: "wsp servers add <name> [<thread>] [--on <computer>] --agent <id> (--command \"<line>\" | --url <address> [--header <name>=<VARIABLE>]...) [--env <NAME>]... [--project [<name>]]".to_string(),
        about: "writes one MCP server into an agent's own config: a command with its arguments and variables, or an address with its headers, each value read off this terminal's environment: the file names a variable and the value goes to the vault, an argument or the address naming a variable as ${NAME} keeps that name there, and an agent that reads no variable there refuses it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("command", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("env", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("url", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("header", FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") })]),
        run: run_servers_add,
        tool: Tool {
            description: "Writes one MCP server into one agent's own config on one computer or where a thread runs, the project's file with project from a thread: a command with its arguments and the variables it is given, or an address with its headers. Every value is read by name off the environment the wsp tools run with and never goes into an answer: the file names a variable and the value goes to the vault, which hands it to each turn there: on a computer the person added only to Claude Code and Codex, in that turn's launch and never its environment, and not to a Codex server whose own config reads its token through its bearer variable; elsewhere in the turn's environment. A header travels as WSP_MCP_<SERVER>_<HEADER>, and a Codex copy reads an Authorization: Bearer header whole as WSP_MCP_<SERVER>_AUTHORIZATION_BEARER, which the vault fills from the token it keeps alone under WSP_MCP_<SERVER>_AUTHORIZATION. An argument or the address may name one of those variables as ${NAME}, an address's variables being only the ones it names: the name stays in the agent's own syntax, and an agent that reads no variable there is refused. A name already in the file is refused rather than written over. Written as the login the computer was added with, into that agent's own file, which keeps its mode; a file that is a link out of the home, or out of the project for a project's file, is not written through, and a file the agent wrote meanwhile is left as it was. A napping machine is not woken. The report there reads again at once.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("what to call the server in the agent's config".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose config takes it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "command", kind: InputKind::String, optional: true, describe: Some("the line the server runs, the program and its arguments as a shell would split them, nothing expanded; or url".to_string()) },
                ToolInput { name: "env", kind: InputKind::StringList { min: None }, optional: true, describe: Some("variables the server is given, or the address names as ${NAME}, each by its name, its value read off the same name in the environment the wsp tools run with; an argument or the address may name one as ${NAME}".to_string()) },
                ToolInput { name: "url", kind: InputKind::String, optional: true, describe: Some("the server's https address; or command".to_string()) },
                ToolInput { name: "header", kind: InputKind::StringList { min: None }, optional: true, describe: Some("headers sent to the address, each <name>=<VARIABLE>, its value read off that variable in the environment the wsp tools run with".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("put it in a project's file rather than the agent's own: true for the thread's own project, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["file"],
            stream: vec![],
            call: call_servers_add,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:544
    Verb::Cli(CliVerb {
        name: "servers remove",
        usage: "wsp servers remove <name> [<thread>] [--on <computer>] --agent <id> [--scope <user|home|local|project>] [--project [<name>]] [--yes]".to_string(),
        about: "takes one MCP server's entry out of an agent's own config, every other line of the file as it was, and, once no agent's config on this computer lists that server, frees the vault's values kept for it that no other server holds".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("scope", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") }), ("yes", FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None })]),
        run: run_servers_remove,
        tool: Tool {
            description: "Takes one MCP server's entry out of one agent's own config on one computer or where a thread runs, in the scope servers lists it under, every other server and line of the file as it was, and, once no agent's config on this computer lists that server, frees the vault's values kept for it that no other server holds. Written as the login the computer was added with, into that agent's own file, which keeps its mode; a file that is a link out of the home, or out of the project for a project's file, is not written through, and a file the agent wrote meanwhile is left as it was. A napping machine is not woken. The report there reads again at once.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the server's name, as servers lists it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose config names it, as servers lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "scope", kind: InputKind::Enum(vec!["user", "home", "local", "project"]), optional: true, describe: Some("the scope servers lists it under: user, home, local or project; user without it".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("the project's server of that name: true with a thread, the thread's own project, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["file"],
            stream: vec![],
            call: call_servers_remove,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:575
    Verb::Cli(CliVerb {
        name: "servers disable",
        usage: "wsp servers disable <name> [<thread>] [--on <computer>] --agent <id> [--scope <user|home|local|project>] [--project [<name>]]".to_string(),
        about: "turns one MCP server off by the switch its agent reads, so the agent leaves it out until it is turned on".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("scope", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") })]),
        run: run_servers_disable,
        tool: Tool {
            description: "Turns one MCP server off in one agent's own config on one computer or where a thread runs, by the switch that agent reads (Codex's enabled line, OpenCode's enabled field, Gemini CLI's mcp.excluded); Claude Code keeps no such switch per server and is refused. Written as the login the computer was added with, into that agent's own file, which keeps its mode; a file that is a link out of the home, or out of the project for a project's file, is not written through, and a file the agent wrote meanwhile is left as it was. A napping machine is not woken. The report there reads again at once.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the server's name, as servers lists it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose config names it, as servers lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "scope", kind: InputKind::Enum(vec!["user", "home", "local", "project"]), optional: true, describe: Some("the scope servers lists it under: user, home, local or project; user without it".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("the project's server of that name: true with a thread, the thread's own project, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["file"],
            stream: vec![],
            call: call_servers_disable,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:575
    Verb::Cli(CliVerb {
        name: "servers enable",
        usage: "wsp servers enable <name> [<thread>] [--on <computer>] --agent <id> [--scope <user|home|local|project>] [--project [<name>]]".to_string(),
        about: "turns an MCP server that was turned off on again".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("scope", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("project", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: Some("on") })]),
        run: run_servers_enable,
        tool: Tool {
            description: "Turns one MCP server on again in one agent's own config on one computer or where a thread runs, by the switch that agent reads (Codex's enabled line, OpenCode's enabled field, Gemini CLI's mcp.excluded); Claude Code keeps no such switch per server and is refused. Written as the login the computer was added with, into that agent's own file, which keeps its mode; a file that is a link out of the home, or out of the project for a project's file, is not written through, and a file the agent wrote meanwhile is left as it was. A napping machine is not woken. The report there reads again at once.".to_string(),
            input: vec![
                ToolInput { name: "name", kind: InputKind::String, optional: false, describe: Some("the server's name, as servers lists it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose config names it, as servers lists it".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
                ToolInput { name: "scope", kind: InputKind::Enum(vec!["user", "home", "local", "project"]), optional: true, describe: Some("the scope servers lists it under: user, home, local or project; user without it".to_string()) },
                ToolInput { name: "project", kind: InputKind::Json, optional: true, describe: Some("the project's server of that name: true with a thread, the thread's own project, or the project's name, as projects lists it, with on".to_string()) },
            ],
            output: vec!["file"],
            stream: vec![],
            call: call_servers_enable,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:605
    Verb::Cli(CliVerb {
        name: "plugins",
        usage: "wsp plugins [<thread>] [--on <computer>]".to_string(),
        about: "each agent's plugins on this computer, a box you added or where a thread runs: on or off, missing where the agent names one it cannot load, its marketplace and version, and what it brings".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_plugins,
        tool: Tool {
            description: format!("{}{}", "Every plugin of each agent on one computer or where a thread runs, one row per agent and plugin in each scope: its id (name@marketplace), the agent, the scope it is installed for (user, or a project's or a project's local with that project), whether the agent's own settings turn it on there (Claude Code: local over project over user settings, then the plugin's default; Codex: what its app server answers), missing where the agent names it and loads nothing of it (Claude Code's folder for it is not there, or Codex lists no such plugin), its folder, its version, its description, where its marketplace comes from, and what it brings: skills, commands and subagents by the names a turn gives them, the events its hooks run on, its tool servers, language servers and apps. Claude Code's are read off its files, never by claude plugin list, which fetches; Codex's are asked of its app server. ", AGENTS_READ_WORDS),
            input: vec![
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["target", "home", "user", "readAt", "stale", "pauseMode", "refused", "projects", "reach", "plugins"],
            stream: vec![],
            call: call_plugins,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:628
    Verb::Cli(CliVerb {
        name: "plugins disable",
        usage: "wsp plugins disable <id> --agent <id> [<thread>] [--on <computer>]".to_string(),
        about: "turns one agent's plugin off for the login, by the agent's own road, so its next turn loads nothing of it".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_plugins_disable,
        tool: Tool {
            description: "Turns one agent's plugin off on one computer or where a thread runs, by its id and agent. Written for the login the computer was added with, where that agent's own command reads it: Claude Code's user settings, its enabledPlugins key written as claude plugin enable or disable writes it but with nothing downloaded first, Codex's config.toml through its app server's config write, which refuses a file changed since wsp read it. A plugin plugins does not list, a missing one, and one a project's settings switch are refused. A turn running now keeps what it loaded; the next one follows. A napping machine is not woken. The report there reads again at once. Answers its row after.".to_string(),
            input: vec![
                ToolInput { name: "plugin", kind: InputKind::String, optional: false, describe: Some("the plugin's id, name@marketplace, as plugins lists it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose plugin it is, as plugins lists it; one id can name a plugin of two agents".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["plugin"],
            stream: vec![],
            call: call_plugins_disable,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    // verbs/agents.ts:628
    Verb::Cli(CliVerb {
        name: "plugins enable",
        usage: "wsp plugins enable <id> --agent <id> [<thread>] [--on <computer>]".to_string(),
        about: "turns one agent's plugin that was turned off on again".to_string(),
        page: Page::Agent,
        options: FlagTable(vec![("agent", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None }), ("on", FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None })]),
        run: run_plugins_enable,
        tool: Tool {
            description: "Turns one agent's plugin on again on one computer or where a thread runs, by its id and agent. Written for the login the computer was added with, where that agent's own command reads it: Claude Code's user settings, its enabledPlugins key written as claude plugin enable or disable writes it but with nothing downloaded first, Codex's config.toml through its app server's config write, which refuses a file changed since wsp read it. A plugin plugins does not list, a missing one, and one a project's settings switch are refused. A turn running now keeps what it loaded; the next one follows. A napping machine is not woken. The report there reads again at once. Answers its row after.".to_string(),
            input: vec![
                ToolInput { name: "plugin", kind: InputKind::String, optional: false, describe: Some("the plugin's id, name@marketplace, as plugins lists it".to_string()) },
                ToolInput { name: "agent", kind: InputKind::String, optional: false, describe: Some("the catalog id of the agent whose plugin it is, as plugins lists it; one id can name a plugin of two agents".to_string()) },
                ToolInput { name: "thread", kind: InputKind::String, optional: true, describe: Some("a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer".to_string()) },
                ToolInput { name: "on", kind: InputKind::String, optional: true, describe: Some("the computer to read, by the name computers lists; absent with no thread is the computer the app runs on".to_string()) },
            ],
            output: vec!["plugin"],
            stream: vec![],
            call: call_plugins_enable,
        },
        reads_here: None,
        starts_no_host: None,
        any_release: None,
        cloud: false,
        cloud_flags: vec![],
    }),
    ]
}

async fn places_of(client: &dyn HostClient) -> Result<Vec<PlaceView>, HostError> {
    let reply = client.request("places.list", Frame::new()).await?;
    field(&reply, "places.list", "places")
}

fn json_of<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("a view writes as JSON")
}

/// A reply's field read into the type a verb takes, or the other-version refusal.
fn field<T: DeserializeOwned>(reply: &Frame, op: &str, key: &str) -> Result<T, HostError> {
    reply.get(key).cloned().and_then(|value| serde_json::from_value(value).ok()).ok_or_else(|| HostError::new(other_version(op)))
}

fn arg_str<'a>(args: &'a Frame, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

fn arg_list(args: &Frame, key: &str) -> Option<Vec<String>> {
    args.get(key).and_then(Value::as_array).map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
}

fn arg_bool(args: &Frame, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

/// A tool's `project` input: `true`, or a project's name.
fn arg_project(args: &Frame, key: &str) -> Option<ProjectValue> {
    match args.get(key)? {
        Value::Bool(asked) => Some(ProjectValue::Flag(*asked)),
        Value::String(name) => Some(ProjectValue::Name(name.clone())),
        _ => None,
    }
}

/// A line's `--project`: the bare flag is the empty name, which `project_asked` reads as asked alone.
fn flag_project(flags: &Flags, name: &str) -> Option<ProjectValue> {
    flag(flags, name).map(|value| ProjectValue::Name(value.to_owned()))
}

/// `{...reportFacts(r), <key>: list}`: the facts first, the one list after them.
fn facts_and(report: &AgentsReport, key: &str, list: Value) -> Value {
    let mut value = json_of(&report_facts(report));
    if let Value::Object(fields) = &mut value {
        fields.insert(key.to_owned(), list);
    }
    value
}

/// The body of a `servers.add` (agents.ts:514, 537): agent, name, the command's words, url, the values, then project.
fn add_body(agent: &str, name: &str, command: ServerCommand, url: Option<&str>, values: ServerValues, project: &ProjectAsked) -> Frame {
    let mut body = Frame::new();
    body.insert("agent".to_owned(), json!(agent));
    body.insert("name".to_owned(), json!(name));
    if let Value::Object(fields) = json_of(&command) {
        body.extend(fields);
    }
    if let Some(url) = url {
        body.insert("url".to_owned(), json!(url));
    }
    if let Value::Object(fields) = json_of(&values) {
        body.extend(fields);
    }
    if project.project {
        body.insert("project".to_owned(), json!(true));
    }
    body
}

/// The body with the scope's keys added after the ones it has.
fn with_scope(mut body: Frame, scope: &impl Serialize) -> Frame {
    if let Value::Object(fields) = json_of(scope) {
        body.extend(fields);
    }
    body
}

/// The body of a server's write that names one server: agent, then name.
fn server_body(agent: &str, name: &str) -> Frame {
    let mut body = Frame::new();
    body.insert("agent".to_owned(), json!(agent));
    body.insert("name".to_owned(), json!(name));
    body
}

/// `agentsReport` (agents-help.ts:113-116); a report this build cannot read is the host's refusal (client.ts:1086).
async fn agents_read(client: &dyn HostClient, read: ReadAim<'_>, cloud_on: bool) -> Result<AgentsReport, HostError> {
    let target = agents_target(client, read.thread, read.on, read.usage, read.line, None, cloud_on).await?;
    let mut params = Frame::new();
    params.insert("target".to_owned(), json!(target));
    let reply = client.request("agents.read", params).await?;
    let report = reply.get("report").cloned().unwrap_or(Value::Null);
    match parse::<AgentsReport>(&report) {
        Ok(report) => Ok(report),
        Err(_) => {
            let issues: Vec<Value> = issues_of::<AgentsReport>(&report)
                .iter()
                .map(|issue| json!({ "code": issue.code, "path": issue.path, "message": issue.message }))
                .collect();
            let text = serde_json::to_string(&issues).expect("issues write as JSON");
            Err(host_schema_refusal(&HostError::new(text), read.schema_usage)
                .unwrap_or_else(|| HostError::new(other_version("agents.read"))))
        }
    }
}

/// What a report read names: the thread or computer, and the usages its refusals carry.
struct ReadAim<'a> {
    thread: Option<&'a str>,
    on: Option<&'a str>,
    usage: &'a str,
    line: &'a str,
    schema_usage: &'a str,
}
