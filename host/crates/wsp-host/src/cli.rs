// SPDX-License-Identifier: AGPL-3.0-only
//! The command dispatcher (`cli()` of `cli.ts`, thin): the cloud refusal, the verb table through `run_verb`, `mcp`,
//! `--version`, `up`, and the refusal of a word no command answers. The other command lines of `commands.ts` (`down`,
//! `status`, `host ...`, `login`, `hosts`, `init`, `add`, `remove`, `join`, `leave`, `doctor`) are `wsp-setup`'s.
use std::sync::Arc;

use serde_json::json;
use wsp_cli::host_lock::HostStarter;
use wsp_cli::parse_args::parse_args;
use wsp_cli::verbs::client::{failed, flag, flag_set, json_asked, take_common, CliIo, FlagKind, FlagRow, FlagTable, Page};
use wsp_cli::verbs::{cloud_line_of, cloud_of, find_verb, run_verb, verb_help, verb_usage, verbs_for, RunDeps};
use wsp_cli::version::VERSION;
use wsp_wire::wire::daemon_version::DAEMON_VERSION;
use wsp_words::exit::{cloud_off_refusal, json_line, usage_refusal, HostError};
use wsp_words::words::thread::{run_for_the_list, unknown_word_line};

use crate::start::SpawnStarter;
use crate::state::{state_path_from, Env};
use crate::up::{asked_from, takes_no_word, up, UpFlags};
use crate::wiring::person_home;

/// `MCP_COMMAND`.
pub const MCP_COMMAND: &str = "mcp";

/// The lines a word that opens lines but is none answers with: `wsp host` opens the four host lines.
const HOST_USAGES: [&str; 5] = [
    "wsp host pair",
    "wsp host devices [revoke <id>]",
    "wsp host devices revoke",
    "wsp host link [<url>] [--name <name>]",
    "wsp host unlink",
];

/// The commands of `commands.ts` that start or drive the host and that `wsp-setup` carries; none is in this build.
const SETUP_COMMANDS: [&str; 15] = [
    "down",
    "status",
    "host pair",
    "host devices",
    "host link",
    "host unlink",
    "login",
    "logout",
    "hosts",
    "init",
    "add",
    "remove",
    "join",
    "leave",
    "doctor",
];

fn flag_table(rows: &[(&'static str, FlagRow)]) -> FlagTable {
    FlagTable(rows.to_vec())
}

/// `SHARED_OPTIONS` (`cli/flags.ts`): the flags the shared parse reads.
fn shared_options() -> FlagTable {
    let short = |c: char| FlagRow { kind: FlagKind::Boolean, multiple: false, short: Some(c), value_with: None };
    flag_table(&[
        ("version", short('v')),
        ("help", short('h')),
        ("state", FlagRow::STRING),
        ("port", FlagRow::STRING),
        ("listen", FlagRow::STRING),
        ("advertise", FlagRow::STRING),
        ("provider", FlagRow::STRING),
        ("no-relay", FlagRow::BOOLEAN),
        ("yes", short('y')),
        ("force", FlagRow::BOOLEAN),
        ("forget", FlagRow::BOOLEAN),
        ("json", FlagRow::BOOLEAN),
        ("recipe", FlagRow::STRING),
        ("project", FlagRow::STRING),
        ("on", FlagRow::STRING),
        ("into", FlagRow::STRING),
        ("local", FlagRow::BOOLEAN),
        ("service", FlagRow::BOOLEAN),
        ("code", FlagRow::STRING),
        ("code-file", FlagRow::STRING),
        ("ssh-port", FlagRow::STRING),
        ("ssh-key", FlagRow::STRING),
        ("host-key", FlagRow::STRING),
        ("base", FlagRow::STRING),
        ("keep", FlagRow::STRINGS),
        ("cut", FlagRow::STRINGS),
        ("takes", FlagRow::STRINGS),
        ("no-memory", FlagRow::BOOLEAN),
        ("no-commits", FlagRow::BOOLEAN),
        ("remember", FlagRow::BOOLEAN),
        ("watch", FlagRow::BOOLEAN),
        ("update", FlagRow::BOOLEAN),
        ("later", FlagRow::BOOLEAN),
        ("resume", FlagRow::BOOLEAN),
        ("sign-in", FlagRow::STRING),
        ("name", FlagRow::STRING),
        ("host", FlagRow::STRING),
    ])
}

/// What the dispatcher is run with: the environment the verbs run in, and the words that start this same wsp.
pub struct Running {
    pub env: Env,
    pub wsp: Vec<String>,
    pub cwd: String,
}

impl Running {
    pub fn here() -> Running {
        let exe = std::env::current_exe().map(|p| p.display().to_string()).unwrap_or_else(|_| "wsp".to_string());
        let cwd = std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| "/".to_string());
        Running { env: std::env::vars().collect(), wsp: vec![exe], cwd }
    }
}

/// What a line comes to before anything is awaited: a verb, a command that needs the async runtime, or the tool server,
/// which runs on this thread with a runtime of its own.
pub enum Line {
    Done(i32),
    Verb(Vec<String>),
    Up(Vec<String>),
    Mcp(Vec<String>),
}

/// The part of `cli()` that reads the words: the flags every line shares come off the whole line first, then the
/// cloud refusal, then the verb, `mcp`, and the shared parse's own answers.
pub fn read_line(argv: &[String], io: &dyn CliIo, run: &Running) -> Line {
    let taken = take_common(argv);
    let (common, rest) = (taken.common, taken.rest);
    let cloud = cloud_of(&run.env);
    if let Some(line) = cloud_line_of(&rest, cloud, wsp_cli::cloud::CLOUD_BUILT) {
        return Line::Done(failed(io, json_asked(argv), &cloud_off_refusal(&line), ""));
    }
    let table = verbs_for(cloud);
    if let Some(verb) = find_verb(&table, &rest) {
        let words = verb.name().split(' ').count();
        let mut line: Vec<String> = rest[..words].to_vec();
        line.extend(common);
        line.extend(rest[words..].iter().cloned());
        return Line::Verb(line);
    }
    if rest.first().map(String::as_str) == Some(MCP_COMMAND) {
        let mut line = common;
        line.extend(rest[1..].iter().cloned());
        return Line::Mcp(line);
    }
    let mut shared = common;
    shared.extend(rest);
    let parsed = match parse_args(&shared, &shared_options()) {
        Ok(parsed) => parsed,
        Err(e) => return Line::Done(failed(io, json_asked(argv), &usage_refusal(&e.message, &run_for_the_list("wsp --help")), "")),
    };
    let json = flag_set(&parsed.values, "json");
    if flag_set(&parsed.values, "version") {
        io.log(&if json { json_line(&json!({"version": VERSION, "daemon": DAEMON_VERSION}), None) } else { format!("wsp {VERSION}") });
        return Line::Done(0);
    }
    let asked: Vec<String> = if parsed.positionals.first().map(String::as_str) == Some("help") {
        parsed.positionals[1..].to_vec()
    } else {
        parsed.positionals.clone()
    };
    let wants_help = flag_set(&parsed.values, "help") || parsed.positionals.first().map(String::as_str) == Some("help") || asked.is_empty();
    let word = asked.first().cloned();
    let known = word.as_deref().is_some_and(|w| w == "up" || SETUP_COMMANDS.iter().any(|c| c.split(' ').next() == Some(w)));
    if wants_help && !known {
        let Some(word) = word else {
            io.log(&verb_help(&table, Some(Page::Front)));
            return Line::Done(0);
        };
        if word == "host" {
            io.log(&HOST_USAGES.iter().map(|usage| format!("usage: {usage}")).collect::<Vec<_>>().join("\n"));
            return Line::Done(0);
        }
        if let Some(usage) = command_usage(&table, &word) {
            io.log(&usage);
            return Line::Done(0);
        }
        let pages = ["agent", "dev"].iter().map(|p| format!("wsp --help {p}")).collect::<Vec<_>>().join(", ");
        let refusal =
            usage_refusal(&format!("wsp --help takes a page, and got {word}."), &format!("The pages are {pages} and wsp host --help."));
        return Line::Done(failed(io, json, &refusal, ""));
    }
    let Some(word) = word else { return Line::Done(0) };
    if word == "up" && !wants_help {
        return Line::Up(shared);
    }
    if SETUP_COMMANDS.iter().any(|c| c.split(' ').next() == Some(word.as_str())) {
        let said = HostError::new(format!("wsp {word} is not in this build of the command line yet"));
        return Line::Done(failed(io, json, &said, ""));
    }
    let refusal = match command_usage(&table, &word) {
        None => usage_refusal(&unknown_word_line(&word), &run_for_the_list("wsp --help")),
        Some(usage) => usage_refusal(&format!("wsp {word} opens a line rather than being one."), &usage),
    };
    Line::Done(failed(io, json, &refusal, ""))
}

/// `commandUsage(word)`: the usage of the command a line stopped short of, whether it is a verb or the word the
/// plumbing folds under; none when no command owns the word.
fn command_usage(table: &[wsp_cli::verbs::client::Verb], word: &str) -> Option<String> {
    if word == "host" {
        return Some(HOST_USAGES.iter().map(|usage| format!("usage: {usage}")).collect::<Vec<_>>().join("\n"));
    }
    verb_usage(table, word)
}

/// A line that is a verb: run through the table with a host starter for the verbs that need one.
pub async fn run_verb_line(line: Vec<String>, io: Arc<dyn CliIo>, run: &Running) -> i32 {
    let cloud = cloud_of(&run.env);
    let table = verbs_for(cloud);
    let Some(verb) = find_verb(&table, &line) else { return 1 };
    let user = person_home();
    let env = run.env.clone();
    let cwd = run.cwd.clone();
    let note = io.clone();
    let choose = move |flag: Option<&str>| state_path_from(flag, &env, &cwd, &user, &|line| note.error(line));
    let starter: Arc<dyn HostStarter> =
        Arc::new(SpawnStarter { wsp: run.wsp.clone(), env: run.env.iter().map(|(k, v)| (k.clone(), v.clone())).collect() });
    let deps = RunDeps { cwd: Some(run.cwd.clone()), env: run.env.clone(), start: Some(starter), ..Default::default() };
    run_verb(&table, verb, &line, io, &choose, deps).await
}

/// `wsp up`: the flags of the shared parse that shape a serving host, then the host served until it is stopped.
pub async fn up_line(line: Vec<String>, io: Arc<dyn CliIo>, run: &Running) -> i32 {
    let json = json_asked(&line);
    let parsed = match parse_args(&line, &shared_options()) {
        Ok(parsed) => parsed,
        Err(e) => return failed(io.as_ref(), json, &usage_refusal(&e.message, &run_for_the_list("wsp --help")), ""),
    };
    if json {
        let refusal = usage_refusal(
            "Unknown option '--json' for wsp up: it answers in prose.",
            "That flag belongs to wsp init, wsp add, wsp remove, and to every verb.",
        );
        return failed(io.as_ref(), json, &refusal, "");
    }
    if let Some(refusal) = takes_no_word(&parsed.positionals[1..]) {
        return failed(io.as_ref(), json, &refusal, "");
    }
    let flags = UpFlags {
        port: flag(&parsed.values, "port").map(str::to_string),
        listen: flag(&parsed.values, "listen").map(str::to_string),
        advertise: flag(&parsed.values, "advertise").map(str::to_string),
        state: flag(&parsed.values, "state").map(str::to_string),
        no_relay: flag_set(&parsed.values, "no-relay"),
    };
    let say = io.clone();
    let asked = asked_from(&flags, &run.env, &run.cwd, &|line| say.error(line));
    match up(io.clone(), asked, run.env.clone(), run.wsp.clone()).await {
        Ok(code) => code,
        Err(e) => failed(io.as_ref(), json, &e, ""),
    }
}
