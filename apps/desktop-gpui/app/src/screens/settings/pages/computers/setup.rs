// SPDX-License-Identifier: AGPL-3.0-only
//! A computer's setup as rows, after `add/setup.ts`'s `setupRows` and `AddComputerDialog.tsx`'s `SetupList`: Install
//! wsp, then each step in the order the person chose them, the items of a step that did not land under it and the
//! sign-ins under Agents, each row with what it asks: Retry where a row did not land, Skip where the host can set an
//! item aside, Copy the keys on a server that waited for a yes, a sign-in that waits with its code and its tab, and
//! Sign in on the computer where a sign-in was skipped or failed, followed through the Agents page's store. A step
//! that ran opens on a click to its last lines of output (places.setupLog), and one that failed stands open.

use std::collections::HashMap;
use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClipboardItem, Entity, FontWeight, ScrollHandle, SharedString, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::settings::SetupLog;
use wsp_host_client::protocol::{PlaceView, PlacesSetupLogRequest, PlacesSetupRequest, PlacesSkipRequest};
use wsp_host_client::Failure;

use super::remove::call_whole;
use super::words::plural;
use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::Ctx;
use crate::screens::settings::pages::agents::model::AgentRow;
use crate::screens::settings::pages::agents::store::{Flow, Start, Store};
use crate::screens::settings::pages::agents::{flow, sign_in_start, Rows};
use crate::screens::settings::words::{fmt_duration, here_name, HERE};
use crate::screens::settings::Acts;
use crate::ui::catalog;
use crate::ui::{motion, scroll_view, size, step_mark, text, ActivePalette, Button, CardItem, Glyph, MarkState, StatusMark, Step};

/// `ADD_COMPUTER_WORDS` the rows say.
pub mod words {
    use crate::screens::settings::words::{capitalised, list_words};

    pub const INSTALL: &str = "Install wsp";
    pub const INSTALLED: &str = "Installed and connected.";
    pub const SKIPPED: &str = "Skipped.";
    pub const TOKEN_COPIED: &str = "Token copied.";
    pub const SIGNED_IN_WITH_TOKEN: &str = "Signed in with a token.";
    pub const NOT_SIGNED_IN_YET: &str = "Not signed in yet.";
    pub const TAB_OPENED: &str = "A tab opened in your browser.";
    pub const RETRY: &str = "Retry";
    pub const SKIP: &str = "Skip";
    pub const SKIP_FOR_NOW: &str = "Skip for now";
    pub const COPY_KEYS: &str = "Copy the keys";
    pub const SIGN_IN_LATER: &str = "You can sign in later in Settings.";
    pub const NO_OUTPUT: &str = "No output yet.";
    pub const RAN_OUT: &str = "The sign-in page ran out.";
    pub const RAN_OUT_FIX: &str = "Retry opens a fresh one.";
    pub const OPEN_TAB: &str = "Open the tab again";
    pub const COPY: &str = "Copy";
    pub const CANCEL: &str = "Cancel";
    pub fn signed_in_on(name: &str) -> String {
        if name.is_empty() {
            "Signed in there.".to_owned()
        } else {
            format!("Signed in on {name}.")
        }
    }
    pub fn at_its_terminal(name: &str) -> String {
        format!("Signs in at its own terminal on {name}.")
    }
    pub fn waits_for_install(name: &str) -> String {
        format!("Waits for {name} to install.")
    }
    pub fn sign_in_on(name: &str) -> String {
        format!("Sign in on {name}")
    }
    pub fn still_setting_up(name: &str) -> String {
        format!("{name} is still being set up; press again once it is done.")
    }
    /// `placeNoPicksLine`.
    pub fn no_picks(name: &str) -> String {
        format!("{name} has nothing picked to go on it: choose in the app's Add a computer, or run wsp add {name} --resume --recipe <name>")
    }
    /// `keysNotCopied`, over `keysKeptLine` where the computer follows a recipe.
    pub fn keys_not_copied(keys: &[String], recipe: Option<&str>) -> String {
        let needs = if keys.is_empty() { "a key".to_owned() } else { list_words(keys) };
        match recipe {
            None => format!("Not copied: it needs {needs}."),
            Some(recipe) => capitalised(&format!(
                "not copied: it needs {needs}; to send {}, pick {COPY_KEYS} under MCP servers on the recipe {recipe} in Settings > Recipes.",
                if keys.len() == 1 { "it" } else { "them" }
            )),
        }
    }
    /// The protocol's `SIGNED_IN_THERE`, `AT_ITS_TERMINAL` and `waitsForInstallLine`, as a row's note carries them.
    pub const SIGNED_IN_THERE: &str = "signed in on that computer";
    pub const AT_ITS_TERMINAL: &str = "signs in at its own terminal on that computer";
    pub fn waits_for_install_line(name: &str) -> String {
        format!("waits for {name} to install")
    }
}

/// A row's state as the marks draw it: a sign-in the person set aside reads skipped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Waiting,
    Working,
    Done,
    Skipped,
    NeedsYou,
    Failed,
    /// A line a step passed with, said and not stopped on. No setup row says it today; `StepRow` draws it for any list.
    #[allow(dead_code)]
    Warned,
}

/// A sign-in that waits on the person.
#[derive(Clone, Debug, PartialEq)]
pub struct Wait {
    pub url: Option<String>,
    pub code: Option<String>,
    pub expired: bool,
}

/// `StepLine`: one row of the list.
#[derive(Clone, Debug, PartialEq)]
pub struct StepLine {
    pub id: String,
    pub name: String,
    pub state: StepState,
    pub note: Option<String>,
    pub ms: Option<f64>,
    pub said: Option<String>,
    pub fix: Option<String>,
    pub wait: Option<Wait>,
    pub sub: bool,
    /// The catalog id whose sign-in the row offers on the computer.
    pub sign_in: Option<String>,
    pub copy_keys: bool,
}

impl StepLine {
    fn new(id: impl Into<String>, name: impl Into<String>, state: StepState) -> StepLine {
        StepLine {
            id: id.into(),
            name: name.into(),
            state,
            note: None,
            ms: None,
            said: None,
            fix: None,
            wait: None,
            sub: false,
            sign_in: None,
            copy_keys: false,
        }
    }
}

const GITHUB_ROW: &str = "github";
const MCP_ID_PREFIX: &str = "agents/mcp/";

/// The steps as every list draws them, the noun each counts what it put on by, and the verb a running one is said in.
const SETUP_ROWS: [(&str, &str, Option<&str>, Option<&str>); 9] = [
    ("floor", "Base packages", Some("package"), None),
    ("agents", "Agents", Some("agent"), Some("Installing")),
    ("mcp", "MCP servers", None, Some("Copying")),
    ("clis", "CLIs", Some("CLI"), Some("Installing")),
    ("skills", "Skills", Some("skill"), Some("Copying")),
    ("plugins", "Plugins", Some("plugin"), Some("Installing")),
    ("github", "GitHub", None, None),
    ("folders", "Projects", Some("project"), Some("Importing")),
    ("configs", "Other config", Some("file"), Some("Copying")),
];

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// `signInOfRow`: an agent's sign-in row, or gh for the GitHub row.
fn sign_in_of_row(id: &str) -> Option<String> {
    id.strip_prefix("signins/").map(str::to_owned).or_else(|| (id == GITHUB_ROW).then(|| "gh".to_owned()))
}

/// `agentOfRow`.
fn agent_of_row(id: &str) -> Option<&str> {
    if id.starts_with(MCP_ID_PREFIX) {
        return None;
    }
    id.strip_prefix("agents/")
}

fn outcome(row: &Value) -> &str {
    str_of(row, "outcome").unwrap_or_default()
}

fn landed(row: &Value) -> bool {
    matches!(outcome(row), "installed" | "present")
}

/// `signInOffer`: a row the person skipped or that failed offers the sign-in it stands for.
fn sign_in_offer(row: Option<&Value>) -> Option<String> {
    row.filter(|r| matches!(outcome(r), "skipped" | "failed")).and_then(|r| str_of(r, "id")).and_then(sign_in_of_row)
}

/// What each step's picks name, by key and the name a person reads.
fn picked(step: &str, picks: &Value) -> Vec<(String, String)> {
    let keys = |table: &str| -> Vec<(String, Value)> {
        picks.get(table).and_then(Value::as_object).map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default()
    };
    match step {
        "agents" => keys("agents").into_iter().map(|(k, _)| (k.clone(), catalog::name(&k))).collect(),
        "mcp" | "clis" | "skills" => keys(step).into_iter().map(|(k, _)| (k.clone(), k)).collect(),
        "plugins" => keys("plugins").into_iter().map(|(k, _)| (k.clone(), k.split('@').next().unwrap_or_default().to_owned())).collect(),
        "folders" => keys("folders").into_iter().map(|(k, v)| (k.clone(), str_of(&v, "name").map(str::to_owned).unwrap_or(k))).collect(),
        "configs" => ["git", "shell"]
            .into_iter()
            .filter(|id| picks.pointer(&format!("/configs/{id}")).is_some())
            .map(|id| (id.to_owned(), id.to_owned()))
            .collect(),
        _ => Vec::new(),
    }
}

/// `doingLine`: what a running step is putting on, its picks less the rows already there.
fn doing_line(step: &str, verb: &str, picks: &Value, rows: &[&Value]) -> Option<String> {
    let there = |key: &str, name: &str| {
        rows.iter().any(|r| {
            let (id, label) = (str_of(r, "id").unwrap_or_default(), str_of(r, "label").unwrap_or_default());
            landed(r) && (id.ends_with(&format!("/{key}")) || label == name || label.starts_with(&format!("{name} ")))
        })
    };
    let names: Vec<String> = picked(step, picks).into_iter().filter(|(k, n)| !there(k, n)).map(|(_, n)| n).collect();
    if names.is_empty() {
        return None;
    }
    let counted = match step {
        "skills" => Some("skills"),
        "plugins" => Some("plugins"),
        _ => None,
    };
    let said = match (counted, names.len()) {
        (Some(noun), n) if n > 3 => format!("{n} {noun}"),
        (_, n) if n <= 3 => names.join(", "),
        (_, n) => format!("{} and {} more", names[..2].join(", "), n - 2),
    };
    Some(format!("{verb} {said}."))
}

/// `countLine`: what a step put there, counted, never named; MCP counts its servers apart from the agents' files.
fn count_line(step: &str, noun: Option<&str>, rows: &[&Value]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    if step != "mcp" {
        return Some(plural(rows.len() as i64, noun.unwrap_or("item")));
    }
    let servers = rows.iter().filter(|r| str_of(r, "id").is_some_and(|id| id.starts_with(MCP_ID_PREFIX))).count();
    let parts: Vec<String> = [
        (servers > 0).then(|| plural(servers as i64, "MCP server")),
        (servers < rows.len()).then(|| plural((rows.len() - servers) as i64, "agent file")),
    ]
    .into_iter()
    .flatten()
    .collect();
    Some(parts.join(", "))
}

/// `githubEnded`: the GitHub row by the way it signed in.
fn github_ended(rows: &[&Value], picks: Option<&Value>, name: &str) -> Option<String> {
    let row = rows.iter().find(|r| str_of(r, "id") == Some(GITHUB_ROW))?;
    if outcome(row) == "skipped" {
        return Some(words::SKIPPED.to_owned());
    }
    if !landed(row) {
        return None;
    }
    let machine = picks.is_some_and(|p| p.pointer("/configs/github").is_some_and(|g| str_of(g, "signin") == Some("machine")));
    Some(if str_of(row, "note") == Some(words::SIGNED_IN_THERE) || machine {
        words::signed_in_on(name)
    } else {
        words::TOKEN_COPIED.to_owned()
    })
}

/// `setupRows`: Install wsp, then each step with the items of it that did not land under it, the sign-ins under
/// Agents. `name` is the computer's, for the words that name it.
pub fn setup_rows(place: &Value, name: &str) -> Vec<StepLine> {
    let setup = place.get("setup");
    let picks = place.get("picks");
    let recipe = str_of(place, "recipe").filter(|r| *r != "none").and_then(|_| picks.and_then(|p| str_of(p, "name")));
    let rows: Vec<&Value> = place.pointer("/applied/rows").and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let steps: Vec<&Value> = setup.and_then(|s| s.get("steps")).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let setup_failed = setup.and_then(|s| str_of(s, "state")) == Some("failed");
    let mut install = StepLine::new("wsp", words::INSTALL, StepState::Done);
    install.note = Some(words::INSTALLED.to_owned());
    let mut out = vec![install];
    for (step, title, noun, verb) in SETUP_ROWS {
        let line = steps.iter().find(|s| str_of(s, "step") == Some(step)).copied();
        let line_state = line.and_then(|l| str_of(l, "state"));
        let mine: Vec<&Value> = rows
            .iter()
            .filter(|r| str_of(r, "step") == Some(step) || (step == "mcp" && str_of(r, "step") == Some("folderServers")))
            .copied()
            .collect();
        let landed_rows: Vec<&Value> = mine.iter().filter(|r| landed(r)).copied().collect();
        let stopped = setup_failed && line_state == Some("failed");
        let missed = mine.is_empty() || mine.iter().any(|r| outcome(r) == "failed");
        let github = if step == "github" { mine.iter().find(|r| str_of(r, "id") == Some(GITHUB_ROW)).copied() } else { None };
        let ran = match line_state {
            None => StepState::Waiting,
            Some("running") => StepState::Working,
            Some(s) if stopped || (s == "failed" && landed_rows.is_empty() && missed) => StepState::Failed,
            Some(_) => StepState::Done,
        };
        let state = match github {
            Some(g) if ran != StepState::Working => match outcome(g) {
                "skipped" => StepState::Skipped,
                "failed" => StepState::Failed,
                _ => ran,
            },
            _ => ran,
        };
        let note = if stopped {
            None
        } else if line_state == Some("running") {
            match (verb, picks) {
                (Some(verb), Some(picks)) => doing_line(step, verb, picks, &mine),
                _ => None,
            }
        } else if step == "github" {
            github_ended(&mine, picks, name)
                .or_else(|| count_line(step, noun, &landed_rows))
                .or_else(|| line.and_then(|l| str_of(l, "note")).map(str::to_owned))
        } else {
            count_line(step, noun, &landed_rows).or_else(|| line.and_then(|l| str_of(l, "note")).map(str::to_owned))
        };
        let mut row = StepLine::new(step, title, state);
        row.note = note;
        row.ms = line.and_then(|l| l.get("ms")).and_then(Value::as_f64);
        if stopped {
            row.said = setup.and_then(|s| str_of(s, "said")).map(str::to_owned);
        }
        if let Some(g) = github.filter(|g| state == StepState::Failed && outcome(g) == "failed" && !stopped) {
            row.said = str_of(g, "note").map(str::to_owned);
            row.fix = str_of(g, "fix").map(str::to_owned);
        }
        if step == "github" && ran != StepState::Working {
            row.sign_in = sign_in_offer(github);
        }
        out.push(row);
        if step == "agents" {
            out.extend(sign_in_rows(setup, &rows, name, place.get("signIns")));
        }
        if stopped {
            continue;
        }
        for r in mine.iter().filter(|r| outcome(r) == "failed" && Some(**r) != github) {
            let mut item = StepLine::new(str_of(r, "id").unwrap_or_default(), str_of(r, "label").unwrap_or_default(), StepState::Failed);
            item.sub = true;
            item.said = str_of(r, "note").map(str::to_owned);
            item.fix = str_of(r, "fix").map(str::to_owned);
            out.push(item);
        }
        for r in mine.iter().filter(|r| outcome(r) == "skipped") {
            let Some(keys) = r.get("keys").and_then(Value::as_array) else { continue };
            let keys: Vec<String> = keys.iter().filter_map(Value::as_str).map(str::to_owned).collect();
            let mut item = StepLine::new(str_of(r, "id").unwrap_or_default(), str_of(r, "label").unwrap_or_default(), StepState::Skipped);
            item.sub = true;
            item.note = Some(words::keys_not_copied(&keys, recipe));
            item.copy_keys = true;
            out.push(item);
        }
    }
    out
}

/// `signInRows`: the sign-ins under Agents, one that waits off its wait, any other off its row.
fn sign_in_rows(setup: Option<&Value>, rows: &[&Value], name: &str, sign_ins: Option<&Value>) -> Vec<StepLine> {
    let waits: Vec<&Value> = setup.and_then(|s| s.get("waiting")).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default();
    let installing = |agent: Option<&str>| {
        agent.is_some_and(|agent| {
            rows.iter().find(|r| str_of(r, "id").and_then(agent_of_row) == Some(agent)).is_some_and(|r| outcome(r) == "failed")
        })
    };
    let mut out: Vec<StepLine> = Vec::new();
    for r in rows.iter().filter(|r| str_of(r, "step") == Some("signins")) {
        let id = str_of(r, "id").unwrap_or_default();
        if waits.iter().any(|w| str_of(w, "row") == Some(id)) {
            continue;
        }
        let label = str_of(r, "label").unwrap_or_default();
        let agent = sign_in_of_row(id);
        let mut line = StepLine::new(id, format!("{label} sign-in"), StepState::Done);
        line.sub = true;
        let o = outcome(r);
        if matches!(o, "failed" | "skipped") && installing(agent.as_deref()) {
            line.state = StepState::Waiting;
            line.note = Some(words::waits_for_install(label));
            out.push(line);
            continue;
        }
        line.ms = r.get("ms").and_then(Value::as_f64);
        line.sign_in = sign_in_offer(Some(r));
        let note = str_of(r, "note");
        match o {
            "failed" => {
                line.state = StepState::Failed;
                line.said = note.map(str::to_owned);
                line.fix = str_of(r, "fix").map(str::to_owned);
            }
            "skipped" => {
                line.state = StepState::Skipped;
                line.note = Some(if note == Some(words::AT_ITS_TERMINAL) {
                    words::at_its_terminal(name)
                } else if note == Some(words::waits_for_install_line(label).as_str()) {
                    words::NOT_SIGNED_IN_YET.to_owned()
                } else {
                    words::SKIPPED.to_owned()
                });
            }
            _ => {
                let state = agent.as_deref().and_then(|a| sign_ins.and_then(|s| str_of(s, a)));
                line.note = Some(match state {
                    Some("vault-key") => words::SIGNED_IN_WITH_TOKEN.to_owned(),
                    Some("none") => words::NOT_SIGNED_IN_YET.to_owned(),
                    Some("signed-in") => words::signed_in_on(name),
                    _ if note == Some(words::SIGNED_IN_THERE) => words::signed_in_on(name),
                    _ => words::TOKEN_COPIED.to_owned(),
                });
            }
        }
        out.push(line);
    }
    for w in waits {
        let mut line = StepLine::new(
            str_of(w, "row").unwrap_or_default(),
            format!("{} sign-in", str_of(w, "label").unwrap_or_default()),
            StepState::NeedsYou,
        );
        line.sub = true;
        let url = str_of(w, "url").map(str::to_owned);
        let expired = str_of(w, "state") == Some("expired");
        if url.is_some() && !expired {
            line.note = Some(words::TAB_OPENED.to_owned());
        }
        line.wait = Some(Wait { url, code: str_of(w, "code").map(str::to_owned), expired });
        out.push(line);
    }
    out
}

/// `SignInWay`: a way an agent that mints its token here signs in on a computer you own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Way {
    Token,
    Key,
    Machine,
}

impl Way {
    fn of(word: &str) -> Option<Way> {
        [Way::Token, Way::Key, Way::Machine].into_iter().find(|w| w.word() == word)
    }

    pub fn word(self) -> &'static str {
        match self {
            Way::Token => "token",
            Way::Key => "key",
            Way::Machine => "machine",
        }
    }

    /// `signInWayLabel`: its name, the same on its button and at the head of its sentence in the row's fix.
    pub fn label(self, computer: &str) -> String {
        match self {
            Way::Token => "Make a token".to_owned(),
            Way::Key => "Use an API key".to_owned(),
            Way::Machine => format!("Sign in on {computer}"),
        }
    }
}

/// `waysOf`: each way of a row's agent, the one the computer was set up with first.
pub fn ways_of(agent: &str, picked: Option<&str>) -> Vec<Way> {
    let mut ways: Vec<Way> = catalog::sign_in_ways(agent).into_iter().filter_map(Way::of).collect();
    if let Some(at) = ways.iter().position(|w| Some(w.word()) == picked) {
        let first = ways.remove(at);
        ways.insert(0, first);
    }
    ways
}

/// `signInWaysFix` with the key typed into the field under the row: a sentence per way, each opening with its button's
/// name and saying what pressing it does.
pub fn ways_fix(ways: &[Way], agent: &str, here: &str, computer: &str) -> String {
    let name = catalog::name(agent);
    let mint = catalog::sign_in(agent).and_then(|s| s.mint);
    let key_env = catalog::key_env(agent);
    ways.iter()
        .map(|way| {
            let does = match way {
                Way::Token => format!(
                    "{} runs on {here} and asks your browser once",
                    mint.map_or_else(|| format!("{name}'s own command"), str::to_owned)
                ),
                Way::Key => format!("paste your {}; wsp saves it to ~/.wsp/.env without showing it", key_env.unwrap_or("key")),
                Way::Machine => format!("{name}'s own sign-in runs there"),
            };
            format!("{}: {does}.", way.label(computer))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `rowsLeft`: the rows that `asks` counts, an item by its own name and a step only where no item under it is named.
pub fn rows_left(rows: &[StepLine], asks: impl Fn(&StepLine) -> bool) -> Vec<&StepLine> {
    let mut named: Vec<&str> = Vec::new();
    let mut step: Option<&StepLine> = None;
    for row in rows {
        if !row.sub {
            step = Some(row);
        } else if asks(row) {
            if let Some(step) = step {
                named.push(&step.id);
            }
        }
    }
    rows.iter().filter(|r| asks(r) && (r.sub || !named.contains(&r.id.as_str()))).collect()
}

/// `rowsLeft` with its default: what waits on the person.
pub fn waiting_on_you(row: &StepLine) -> bool {
    matches!(row.state, StepState::NeedsYou | StepState::Failed)
}

/// `SETUP_STEP_WORDS`, by which a log line names its step.
fn step_words(step: &str) -> Option<&'static str> {
    Some(match step {
        "floor" => "the base tools",
        "agents" => "the agents",
        "signins" => "the agents' sign-ins",
        "clis" => "the CLIs",
        "skills" => "the skills",
        "mcp" => "the agents' own files and MCP servers",
        "plugins" => "the plugins",
        "configs" => "git and the shell",
        "folders" => "the folders, as projects",
        "folderServers" => "the projects' own MCP servers",
        "github" => "the GitHub sign-in",
        "context" => "what the agents read about this computer",
        _ => return None,
    })
}

/// `opensLog`: a step of the setup, not an item under one, that has started.
pub fn opens_log(row: &StepLine) -> bool {
    !row.sub && row.state != StepState::Waiting && step_words(&row.id).is_some()
}

const LOG_LINES: usize = 12;
const LOG_CHARS: usize = 300;

/// `stepLogs`: the last lines each step wrote, off the end of the setup log, oldest first; the line that marks a step
/// running or done is passed over.
pub fn step_logs(lines: &[String]) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for line in lines {
        let Some((stamp, rest)) = line.split_once(' ') else { continue };
        if stamp.is_empty() || stamp.contains(char::is_whitespace) {
            continue;
        }
        let Some(rest) = rest.strip_prefix('[') else { continue };
        let Some((step, said)) = rest.split_once("] ") else { continue };
        if step.is_empty() || !step.chars().all(|c| c.is_ascii_lowercase()) {
            continue;
        }
        let Some(own) = step_words(step) else { continue };
        let said = said.trim();
        if said.is_empty() || said.starts_with(&format!("{own}: ")) {
            continue;
        }
        let held = out.entry(step.to_owned()).or_default();
        held.push(if said.chars().count() > LOG_CHARS {
            format!("{}\u{2026}", said.chars().take(LOG_CHARS - 1).collect::<String>())
        } else {
            said.to_owned()
        });
        if held.len() > LOG_LINES {
            held.remove(0);
        }
    }
    out
}

/// How often an open step's output is read while it runs.
const OUTPUT_EVERY: Duration = Duration::from_secs(3);

/// What the list holds between frames: the refusal under the row whose act asked, whether an act is out, which rows
/// the person turned, and the step logs with the road that reads them.
#[derive(Default)]
struct List {
    refused: Option<(String, String)>,
    busy: bool,
    turned: HashMap<String, bool>,
    logs: Option<HashMap<String, Vec<String>>>,
    reading: Option<(bool, Task<()>)>,
    seen: (Option<String>, Option<String>),
    /// Each open log's scroll and the text it last pinned to its end.
    scrolls: HashMap<String, (ScrollHandle, String)>,
    /// The row whose device code was just copied, its glyph a check for a moment.
    copied: Option<String>,
}

/// How long a copied code's glyph stays a check (`COPIED_MS`).
const COPIED: Duration = Duration::from_millis(1_400);

/// Reads the setup log once, and again every few seconds while `live`, until the list stops wanting it.
fn read_logs(acts: &Acts, place: String, live: bool, list: &Entity<List>, cx: &mut App) -> Task<()> {
    let Some((host, runtime)) = acts.host(cx) else { return Task::ready(()) };
    let me = list.downgrade();
    cx.spawn(async move |cx| loop {
        let (tx, rx) = futures::channel::oneshot::channel();
        let (host, place2) = (host.clone(), place.clone());
        runtime.spawn(async move {
            _ = tx.send(host.call(req::<PlacesSetupLogRequest>(json!({ "placeId": place2 }))).await);
        });
        if let Ok(Ok(SetupLog { lines })) = rx.await {
            let next = step_logs(&lines);
            let kept = me.update(cx, |l, cx| {
                if l.logs.as_ref() != Some(&next) {
                    l.logs = Some(next);
                    cx.notify();
                }
            });
            if kept.is_err() {
                return;
            }
        }
        if !live {
            return;
        }
        cx.background_executor().timer(OUTPUT_EVERY).await;
    })
}

/// One act on the host that answers the computer's row: the refusal lands under its row, a conflict says the setup
/// still runs, and the places are read again.
fn ask<O>(acts: &Acts, list: &Entity<List>, row: String, name: String, op: O, said_of: fn(&O::Reply) -> Option<String>, cx: &mut App)
where
    O: wsp_host_client::Op + Send + 'static,
    O::Reply: Send + 'static,
{
    list.update(cx, |l, _| l.busy = true);
    let (acts2, list) = (acts.clone(), list.clone());
    call_whole(acts, op, cx, move |answer: Result<O::Reply, Failure>, cx| {
        let refused = match answer {
            Ok(reply) => said_of(&reply),
            Err(failure) if failure.kind.as_deref() == Some("conflict") => Some(words::still_setting_up(&name)),
            Err(failure) => Some(failure.message),
        };
        list.update(cx, |l, _| {
            l.busy = false;
            l.refused = refused.map(|said| (row, said));
        });
        acts2.reread(Reread::Places, cx);
    });
}

fn setup_said(reply: &wsp_host_client::ops::dialogs::PlaceSetUp) -> Option<String> {
    reply.said.clone()
}

fn nothing_said(_: &wsp_host_client::ops::settings::Place) -> Option<String> {
    None
}

/// The row's own frame: its mark, its name over its note, its time and the chevron of a row that opens on its output.
fn line(row: &StepLine, toggle: Option<bool>, any_toggle: bool, window: &mut Window, cx: &mut App) -> gpui_kit::Div {
    let p = cx.palette().clone();
    let quiet = row.state == StepState::Waiting;
    let mark: AnyElement = match row.state {
        StepState::Waiting => step_mark(Step::NotStarted, cx).into_any_element(),
        StepState::Working => step_mark(Step::Running, cx).into_any_element(),
        StepState::Skipped => step_mark(Step::Skipped, cx).into_any_element(),
        StepState::Done => step_mark(Step::Done, cx).into_any_element(),
        StepState::Failed => step_mark(Step::Failed, cx).into_any_element(),
        StepState::Warned => div()
            .debug_selector(|| "step-warned".into())
            .child(Icon::new(Glyph::TriangleAlert).with_size(px(14.)).text_color(p.warning))
            .into_any_element(),
        StepState::NeedsYou => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(16.))
            .child(StatusMark::state(format!("step-{}-mark", row.id), MarkState::NeedsYou))
            .into_any_element(),
    };
    let time = row.ms.map(fmt_duration).unwrap_or_default();
    let chevron = match toggle {
        Some(open) => Some(
            Icon::new(Glyph::ChevronRight)
                .with_size(px(14.))
                .flex_none()
                .text_color(p.muted_foreground)
                .rotate(gpui_kit::radians(turn(&row.id, open, window, cx).to_radians()))
                .into_any_element(),
        ),
        None if any_toggle => Some(div().flex_none().size(px(14.)).into_any_element()),
        None => None,
    };
    h_flex()
        .w_full()
        .gap_x_3()
        .py(size::ROW_PAD_Y)
        .px(size::CARD_INSET)
        .min_h(size::LINE_FLOOR)
        .when(row.sub, |el| el.pl(size::CARD_INSET + px(28.)))
        .child(div().flex_none().w(px(16.)).flex().items_center().justify_center().child(mark))
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .min_w_0()
                        .text_size(text::SETTING_TITLE.0)
                        .line_height(text::SETTING_TITLE.1)
                        .font_weight(if quiet { FontWeight::NORMAL } else { FontWeight::MEDIUM })
                        .text_color(if quiet { p.muted_foreground } else { p.foreground })
                        .child(row.name.clone()),
                )
                .children(
                    row.note
                        .clone()
                        .map(|note| div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(note)),
                ),
        )
        .child(
            h_flex()
                .flex_none()
                .gap_2()
                .child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(text::FACT.0)
                        .text_color(p.muted_foreground)
                        .child(time),
                )
                .children(chevron),
        )
}

/// The chevron's turn in degrees, a quarter while its row is open, eased over `duration-150`.
pub fn turn(row: &str, open: bool, window: &mut Window, cx: &mut App) -> f32 {
    motion::ease_f32(SharedString::from(format!("setup-{row}-chevron")), "turn", if open { 90. } else { 0. }, motion::TURN, window, cx)
}

/// `StepLog`: a step's last lines in the mono, wrapping, scrolling inside its 192 px and pinned to its newest line
/// whenever the lines change.
fn step_log(row: &str, lines: Option<&Vec<String>>, list: &Entity<List>, window: &mut Window, cx: &mut App) -> AnyElement {
    let p = cx.palette().clone();
    let Some(lines) = lines.filter(|l| !l.is_empty()) else {
        return div()
            .text_size(text::NOTE.0)
            .line_height(text::NOTE.1)
            .text_color(p.muted_foreground)
            .child(words::NO_OUTPUT)
            .into_any_element();
    };
    let text = lines.join("\n");
    let scroll = list.update(cx, |l, _| {
        let (handle, pinned) = l.scrolls.entry(row.to_owned()).or_insert_with(|| (ScrollHandle::new(), String::new()));
        if *pinned != text {
            *pinned = text.clone();
            handle.scroll_to_bottom();
        }
        handle.clone()
    });
    let last = lines.len() - 1;
    let log = div()
        .debug_selector(|| "step-log".into())
        .max_h(px(192.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(11.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .children(lines.iter().enumerate().map(|(ix, line)| {
            let line = div().child(line.clone());
            if ix == last {
                line.debug_selector(|| "step-log-last".into())
            } else {
                line
            }
        }));
    scroll_view(SharedString::from(format!("step-log-{row}")), &scroll, log, window, cx).into_any_element()
}

/// `DeviceCode`: the code in the large mono and Copy beside it, which turns only its glyph once it copied.
fn device_code(row: &str, code: &str, list: &Entity<List>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let copied = list.read(cx).copied.as_deref() == Some(row);
    let (list, row_id, value) = (list.clone(), row.to_owned(), code.to_owned());
    h_flex()
        .gap_x_3()
        .child(
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(20.))
                .line_height(px(28.))
                .text_color(p.foreground)
                .child(code.to_owned()),
        )
        .child(
            div().debug_selector(|| "copy-code".into()).child(
                Button::outline(SharedString::from(format!("setup-{row}-copy-code")), words::COPY)
                    .glyph(if copied { Glyph::Check } else { Glyph::Copy })
                    .xs()
                    .on_click(move |_, window, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(value.clone()));
                        list.update(cx, |l, cx| {
                            l.copied = Some(row_id.clone());
                            cx.notify();
                        });
                        let (list, row_id) = (list.clone(), row_id.clone());
                        window
                            .spawn(cx, async move |cx| {
                                cx.background_executor().timer(COPIED).await;
                                list.update(cx, |l, cx| {
                                    if l.copied.as_deref() == Some(row_id.as_str()) {
                                        l.copied = None;
                                        cx.notify();
                                    }
                                });
                            })
                            .detach();
                    }),
            ),
        )
        .into_any_element()
}

/// `wayAct`: one way's act on a row whose agent has several. The key, where the computer was set up to take one, is
/// asked for in a field under the row; elsewhere it runs the row again on the key. The token is made on the computer
/// the host runs on, the login runs on the computer itself; each ends what the other's sign-ins hold on that row, and
/// one that runs is Cancel.
#[allow(clippy::too_many_arguments)]
fn way_act(
    way: Way,
    agent: &str,
    picked: Option<&str>,
    row: &str,
    place: &str,
    computer: &str,
    busy: bool,
    store: &Entity<Store>,
    list: &Entity<List>,
    ctx: &Ctx,
    value: &Value,
    cx: &App,
) -> AnyElement {
    let k = SharedString::from(format!(
        "setup-{row}-{}",
        if way == Way::Machine { "sign-in".to_owned() } else { format!("sign-in-{}", way.word()) }
    ));
    let (store, row, place, agent) = (store.clone(), row.to_owned(), place.to_owned(), agent.to_owned());
    let tagged = |button: Button| div().debug_selector(move || format!("way-{}", way.word())).child(button).into_any_element();
    if way == Way::Key && picked != Some("key") {
        let (acts, list, name, picks) = (ctx.acts.clone(), list.clone(), computer.to_owned(), value.get("picks").cloned());
        return tagged(Button::outline(k, way.label(computer)).glyph(Glyph::KeyRound).xs().held(busy).on_click(
            move |_, _, cx| match &picks {
                None => list.update(cx, |l, _| l.refused = Some((row.clone(), words::no_picks(&name)))),
                Some(picks) => {
                    let mut choices = picks.clone();
                    choices["agents"][&agent]["signin"] = json!("key");
                    let op = req::<PlacesSetupRequest>(json!({ "ref": place, "choices": choices, "row": row }));
                    ask(&acts, &list, row.clone(), name.clone(), op, setup_said, cx)
                }
            },
        ));
    }
    let (at, other, start, glyph) = match way {
        Way::Key => (place.clone(), HERE.to_owned(), Start::Vault { agent: agent.clone(), mint: None, word: "key" }, Glyph::KeyRound),
        Way::Token => (
            HERE.to_owned(),
            place.clone(),
            Start::Run { agent: agent.clone(), server: None, finish: None, pastes: false, cloud: false },
            Glyph::LogIn,
        ),
        Way::Machine => {
            let pastes = catalog::there_pastes(&agent);
            (
                place.clone(),
                HERE.to_owned(),
                Start::Run { agent: agent.clone(), server: None, finish: None, pastes, cloud: false },
                Glyph::LogIn,
            )
        }
    };
    if store.read(cx).flows.get(&(at.clone(), row.clone())).is_some_and(Flow::runs) {
        let k = SharedString::from(format!("{k}-cancel"));
        return tagged(
            Button::outline(k, words::CANCEL)
                .glyph(Glyph::X)
                .xs()
                .on_click(move |_, _, cx| store.update(cx, |s, cx| s.cancel(&at, &row, cx))),
        );
    }
    tagged(Button::outline(k, way.label(computer)).glyph(glyph).xs().on_click(move |_, _, cx| {
        let start = start.clone();
        store.update(cx, |s, cx| {
            s.cancel(&other, &row, cx);
            s.start(&at, &row, start, cx)
        })
    }))
}

/// `SetupList`: the setup's rows, each with what it asks.
pub fn list(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> AnyElement {
    let name = super::words::place_name(place);
    let value = serde_json::to_value(place).unwrap_or(Value::Null);
    let rows = setup_rows(&value, &name);
    let id = place.id.clone();
    let list = window.use_keyed_state(SharedString::from(format!("computer-{id}-setup-list")), cx, |_, _| List::default());
    // A refusal says how the setup or a recipe's sync stood when it was pressed, so it goes once either moves on.
    let seen = (
        value.pointer("/setup/state").and_then(Value::as_str).map(str::to_owned),
        value.pointer("/sync/state").and_then(Value::as_str).map(str::to_owned),
    );
    if list.read(cx).seen != seen {
        list.update(cx, |l, _| {
            l.seen = seen;
            l.refused = None;
        });
    }
    let is_open = |l: &List, row: &StepLine| opens_log(row) && l.turned.get(&row.id).copied().unwrap_or(row.state == StepState::Failed);
    let (want, live) = {
        let l = list.read(cx);
        let open: Vec<&StepLine> = rows.iter().filter(|r| is_open(l, r)).collect();
        (!open.is_empty(), open.iter().any(|r| r.state == StepState::Working))
    };
    let reading = list.read(cx).reading.as_ref().map(|(l, _)| *l);
    if !want {
        if reading.is_some() {
            list.update(cx, |l, _| l.reading = None);
        }
    } else if reading != Some(live) {
        let task = read_logs(&ctx.acts, id.clone(), live, &list, cx);
        list.update(cx, |l, _| l.reading = Some((live, task)));
    }
    let store = rows.iter().any(|r| r.sign_in.is_some()).then(|| {
        let store = Store::of(&ctx.agents, &ctx.acts, cx);
        store.update(cx, |s, cx| s.read(&id, cx));
        store
    });
    let own_picks = place.recipe.as_deref().is_none_or(|r| r == "none");
    let any_toggle = rows.iter().any(opens_log);
    let p = cx.palette().clone();
    let mut items: Vec<CardItem> = Vec::new();
    for row in rows {
        let (busy, refused, open, logs) = {
            let l = list.read(cx);
            (l.busy, l.refused.clone().filter(|(r, _)| *r == row.id).map(|(_, said)| said), is_open(l, &row), l.logs.clone())
        };
        let key = |part: &str| SharedString::from(format!("setup-{}-{part}", row.id));
        let retry = {
            let (acts, list, row_id, name, id) = (ctx.acts.clone(), list.clone(), row.id.clone(), name.clone(), id.clone());
            move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                ask(
                    &acts,
                    &list,
                    row_id.clone(),
                    name.clone(),
                    req::<PlacesSetupRequest>(json!({ "ref": id, "row": row_id })),
                    setup_said,
                    cx,
                )
            }
        };
        let skip_as = |word: &'static str, bare: bool| {
            let (acts, list, row_id, name, id) = (ctx.acts.clone(), list.clone(), row.id.clone(), name.clone(), id.clone());
            let button = if bare { Button::text(key("skip"), word) } else { Button::ghost(key("skip"), word) };
            button.xs().held(busy).on_click(move |_, _, cx| {
                ask(
                    &acts,
                    &list,
                    row_id.clone(),
                    name.clone(),
                    req::<PlacesSkipRequest>(json!({ "placeId": id, "row": row_id })),
                    nothing_said,
                    cx,
                )
            })
        };
        let skip = |word: &'static str| skip_as(word, false);
        let bare_skip = |word: &'static str| skip_as(word, true);
        // The agent's own sign-in on that computer, off its row in the computer's agents report, through the store.
        let sign_in: Option<(AnyElement, Option<AnyElement>)> = match (&store, &row.sign_in) {
            (Some(store), Some(agent)) => {
                let s = store.read(cx);
                let report_row: Option<AgentRow> =
                    s.reads.get(&id).and_then(|r| r.report.as_ref()).and_then(|r| r.agents.iter().find(|a| a.id == *agent).cloned());
                let start = report_row.as_ref().and_then(|r| sign_in_start(r, &Rows::of(place, s)));
                let flow_now: Option<Flow> = s.flows.get(&(id.clone(), row.id.clone())).cloned();
                let button = if flow_now.as_ref().is_some_and(|f| f.runs() && !matches!(f, Flow::Copy { .. })) {
                    let (store, id, row_id) = (store.clone(), id.clone(), row.id.clone());
                    Some(
                        Button::outline(key("cancel"), words::CANCEL)
                            .glyph(Glyph::X)
                            .xs()
                            .on_click(move |_, _, cx| store.update(cx, |s, cx| s.cancel(&id, &row_id, cx)))
                            .into_any_element(),
                    )
                } else {
                    start.map(|start| {
                        let (store, id, row_id) = (store.clone(), id.clone(), row.id.clone());
                        Button::outline(key("sign-in"), words::sign_in_on(&name))
                            .glyph(Glyph::LogIn)
                            .xs()
                            .on_click(move |_, _, cx| {
                                let start = start.clone();
                                store.update(cx, |s, cx| s.start(&id, &row_id, start, cx))
                            })
                            .into_any_element()
                    })
                };
                let drawn = flow_now.map(|f| flow::view(store, &id, &row.id, &f, true, window, cx));
                button.map(|b| (b, drawn))
            }
            _ => None,
        };
        let (sign_in_button, flow_view) = match sign_in {
            Some((b, f)) => (Some(b), f),
            None => (None, None),
        };
        let picked =
            row.sign_in.as_deref().and_then(|agent| value.pointer(&format!("/picks/agents/{agent}/signin"))).and_then(Value::as_str);
        let ways = match (&store, &row.sign_in) {
            (Some(_), Some(agent)) => ways_of(agent, picked),
            _ => Vec::new(),
        };
        let way_flow = store.as_ref().filter(|_| !ways.is_empty()).and_then(|store| {
            let s = store.read(cx);
            let mine = s.flows.get(&(id.clone(), row.id.clone())).cloned().map(|f| (id.clone(), f));
            mine.or_else(|| s.flows.get(&(HERE.to_owned(), row.id.clone())).cloned().map(|f| (HERE.to_owned(), f)))
        });
        // With the key's field open under it, the row's fix says to paste the key there.
        let keyed = picked == Some("key") && way_flow.as_ref().is_some_and(|(at, f)| *at == id && matches!(f, Flow::Vault { .. }));
        let fix = if keyed && ways.contains(&Way::Key) {
            Some(ways_fix(&ways, row.sign_in.as_deref().unwrap_or_default(), &here_name(&ctx.data.places), &name))
        } else {
            row.fix.clone()
        };
        let flow_view = match (&store, way_flow) {
            (Some(store), Some((at, f))) => Some(flow::view(store, &at, &row.id, &f, true, window, cx)),
            _ if !ways.is_empty() => None,
            _ => flow_view,
        };
        let mut acts: Vec<AnyElement> = Vec::new();
        let by_way = !ways.is_empty() && row.wait.is_none() && matches!(row.state, StepState::Failed | StepState::Skipped);
        if let (true, Some(store), Some(agent)) = (by_way, &store, row.sign_in.clone()) {
            for way in &ways {
                acts.push(way_act(*way, &agent, picked, &row.id, &id, &name, busy, store, &list, ctx, &value, cx));
            }
            if row.state == StepState::Failed && row.sub {
                acts.push(skip(words::SKIP).into_any_element());
            }
        } else {
            match row.state {
                StepState::Skipped if row.copy_keys && own_picks => {
                    let (acts2, list2, row_id, name2, id2) = (ctx.acts.clone(), list.clone(), row.id.clone(), name.clone(), id.clone());
                    let picks = value.get("picks").cloned();
                    acts.push(
                        Button::outline(key("copy-keys"), words::COPY_KEYS)
                            .xs()
                            .held(busy)
                            .on_click(move |_, _, cx| match &picks {
                                None => list2.update(cx, |l, _| l.refused = Some((row_id.clone(), words::no_picks(&name2)))),
                                Some(picks) => {
                                    let mut choices = picks.clone();
                                    choices["copyKeys"] = json!(true);
                                    let op = req::<PlacesSetupRequest>(json!({ "ref": id2, "choices": choices, "row": row_id }));
                                    ask(&acts2, &list2, row_id.clone(), name2.clone(), op, setup_said, cx)
                                }
                            })
                            .into_any_element(),
                    );
                }
                StepState::Skipped => acts.extend(sign_in_button),
                StepState::Failed if row.wait.is_none() => {
                    acts.push(sign_in_button.unwrap_or_else(|| {
                        Button::outline(key("retry"), words::RETRY).xs().held(busy).on_click(retry.clone()).into_any_element()
                    }));
                    if row.sub {
                        acts.push(skip(words::SKIP).into_any_element());
                    }
                }
                _ => {}
            }
        }
        let mut under: Vec<AnyElement> = Vec::new();
        if let Some(wait) = &row.wait {
            if wait.expired {
                under.push(
                    div()
                        .text_size(text::NOTE.0)
                        .line_height(px(18.))
                        .text_color(p.error_foreground)
                        .child(format!("{} ", words::RAN_OUT))
                        .child(div().text_color(p.foreground).child(words::RAN_OUT_FIX))
                        .flex()
                        .flex_wrap()
                        .into_any_element(),
                );
                under.push(
                    h_flex()
                        .flex_wrap()
                        .gap_2()
                        .child(Button::outline(key("retry"), words::RETRY).xs().held(busy).on_click(retry.clone()))
                        .child(skip(words::SKIP_FOR_NOW))
                        .into_any_element(),
                );
            } else {
                let url = wait.url.clone();
                under.push(
                    h_flex()
                        .min_h(px(40.))
                        .flex_wrap()
                        .gap_x_3()
                        .gap_y_2()
                        .children(wait.code.as_deref().map(|code| device_code(&row.id, code, &list, cx)))
                        .children(url.map(|url| {
                            Button::outline(key("open-tab"), words::OPEN_TAB)
                                .glyph(Glyph::ExternalLink)
                                .xs()
                                .on_click(move |_, _, cx| crate::ui::open_link(&url, cx))
                        }))
                        .child(bare_skip(words::SKIP_FOR_NOW))
                        .into_any_element(),
                );
                under.push(
                    div()
                        .text_size(text::NOTE.0)
                        .line_height(text::NOTE.1)
                        .text_color(p.muted_foreground)
                        .child(words::SIGN_IN_LATER)
                        .into_any_element(),
                );
            }
        } else if let Some(view) = flow_view {
            under.push(view);
        } else if open && logs.is_some() {
            under.push(step_log(&row.id, logs.as_ref().and_then(|l| l.get(&row.id)), &list, window, cx));
        }
        if let Some(said) = refused {
            under.push(crate::ui::Refusal::new().said(said).into_any_element());
        }
        let said = row.said.clone().filter(|s| !s.is_empty()).map(|said| {
            let mut words =
                div().flex().flex_wrap().text_size(text::NOTE.0).line_height(px(18.)).text_color(p.error_foreground).child(said);
            if let Some(fix) = fix.clone() {
                words = words.child(div().text_color(p.foreground).child(format!("\u{a0}{fix}")));
            }
            words.debug_selector(|| "step-refusal".into()).into_any_element()
        });
        let toggle = opens_log(&row).then_some(open);
        let head = line(&row, toggle, any_toggle, window, cx);
        let head: AnyElement = match toggle {
            Some(_) => {
                let (list, row_id) = (list.clone(), row.id.clone());
                let accent = p.accent;
                div()
                    .id(key("toggle"))
                    .cursor_pointer()
                    .hover(move |s| s.bg(accent.opacity(0.6)))
                    .on_click(move |_, _, cx| {
                        list.update(cx, |l, cx| {
                            let was = l.turned.get(&row_id).copied();
                            let now = !was.unwrap_or(open);
                            l.turned.insert(row_id.clone(), now);
                            cx.notify();
                        })
                    })
                    .child(head)
                    .into_any_element()
            }
            None => head.into_any_element(),
        };
        let shows_under = said.is_some() || !under.is_empty() || !acts.is_empty();
        let body = shows_under.then(|| {
            v_flex()
                .gap_3()
                .pb_4()
                .pr(size::CARD_INSET)
                .pl(size::CARD_INSET + px(if row.sub { 56. } else { 28. }))
                .children(said)
                .children(under)
                .when(!acts.is_empty(), |el| el.child(h_flex().flex_wrap().gap_2().children(acts)))
        });
        let row_id = row.id.clone();
        items.push(CardItem::placed(move |corners| {
            corners.apply(v_flex().id(SharedString::from(format!("step-row-{row_id}")))).child(head).children(body).into_any_element()
        }));
    }
    crate::ui::Card::new(SharedString::from(format!("setup-{id}"))).items(items).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows_of(place: Value) -> Vec<StepLine> {
        setup_rows(&place, "box1")
    }

    #[test]
    fn a_step_whose_line_failed_but_landed_items_reads_done() {
        let place = json!({
            "setup": { "state": "done", "addId": "a", "startedAt": "x", "waiting": [], "steps": [
                { "step": "floor", "state": "done", "ms": 1200 },
                { "step": "clis", "state": "failed" },
            ] },
            "applied": { "hash": "h", "at": "x", "rows": [
                { "id": "tools/npm/jq", "label": "jq", "outcome": "installed", "step": "clis" },
                { "id": "tools/npm/rg", "label": "ripgrep", "outcome": "failed", "step": "clis", "note": "no network" },
            ] },
        });
        let rows = rows_of(place);
        let clis = rows.iter().find(|r| r.id == "clis").unwrap();
        assert_eq!(clis.state, StepState::Done);
        assert_eq!(clis.note.as_deref(), Some("1 CLI"));
        let item = rows.iter().find(|r| r.id == "tools/npm/rg").unwrap();
        assert!(item.sub && item.state == StepState::Failed && item.said.as_deref() == Some("no network"));
        assert_eq!(rows.iter().find(|r| r.id == "floor").unwrap().ms, Some(1200.));
        assert_eq!(rows.iter().find(|r| r.id == "agents").unwrap().state, StepState::Waiting);
        let left: Vec<&str> = rows_left(&rows, waiting_on_you).iter().map(|r| r.name.as_str()).collect();
        assert_eq!(left, ["ripgrep"]);
    }

    #[test]
    fn sign_ins_stand_under_agents_and_a_project_server_counts_under_mcp() {
        let place = json!({
            "signIns": { "claude": "signed-in" },
            "setup": { "state": "running", "addId": "a", "startedAt": "x", "steps": [
                { "step": "agents", "state": "done" }, { "step": "mcp", "state": "done" }, { "step": "context", "state": "done" },
            ], "waiting": [{ "row": "signins/codex", "label": "Codex", "expiresAt": "x", "state": "waiting", "code": "AB-12", "url": "https://x" }] },
            "applied": { "hash": "h", "at": "x", "rows": [
                { "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" },
                { "id": "signins/claude", "label": "Claude Code", "outcome": "installed", "step": "signins" },
                { "id": "signins/opencode", "label": "OpenCode", "outcome": "skipped", "step": "signins" },
                { "id": "agents/mcp/docs", "label": "docs", "outcome": "installed", "step": "folderServers" },
                { "id": "agents/mcp/x", "label": "x", "outcome": "skipped", "step": "mcp", "keys": ["API_KEY"] },
            ] },
        });
        let rows = rows_of(place);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        let at = |id: &str| ids.iter().position(|i| *i == id).unwrap();
        assert!(at("agents") < at("signins/claude") && at("signins/claude") < at("signins/codex") && at("signins/codex") < at("mcp"));
        assert!(!ids.contains(&"context"));
        assert_eq!(rows[at("signins/claude")].note.as_deref(), Some("Signed in on box1."));
        assert_eq!(rows[at("signins/opencode")].sign_in.as_deref(), Some("opencode"));
        assert_eq!(rows[at("signins/codex")].state, StepState::NeedsYou);
        assert_eq!(rows[at("mcp")].note.as_deref(), Some("1 MCP server"));
        let keys = &rows[at("agents/mcp/x")];
        assert!(keys.copy_keys && keys.note.as_deref() == Some("Not copied: it needs API_KEY."));
    }

    #[test]
    fn a_step_log_keeps_each_steps_last_lines() {
        let lines: Vec<String> = ["t [clis] installing jq", "t [clis] the CLIs: done", "t [nope] x", "garbage", "t [floor] apt ok"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let logs = step_logs(&lines);
        assert_eq!(logs.get("clis"), Some(&vec!["installing jq".to_owned()]));
        assert_eq!(logs.get("floor"), Some(&vec!["apt ok".to_owned()]));
        assert_eq!(logs.len(), 2);
    }

    #[test]
    fn the_sign_in_ways_are_the_catalogs() {
        let read = crate::screens::settings::words::tests::catalog_read();
        for pair in read["signInWays"].as_array().unwrap() {
            let agent = pair[0].as_str().unwrap();
            let ours: Vec<&str> = ways_of(agent, None).iter().map(|w| w.word()).collect();
            assert_eq!(json!(ours), pair[1], "{agent}'s ways");
            if !ours.is_empty() {
                assert_eq!(json!(catalog::key_env(agent)), pair[2], "{agent}'s key variable");
            }
        }
        let words = &read["signInWayWords"];
        for pair in words["labels"].as_array().unwrap() {
            let way = Way::of(pair[0].as_str().unwrap()).unwrap();
            assert_eq!(json!(way.label("box1")), pair[1]);
        }
        assert_eq!(json!(ways_fix(&[Way::Token, Way::Key, Way::Machine], "claude", "mac", "box1")), words["fix"]);
    }

    #[test]
    fn the_way_a_computer_was_set_up_with_comes_first() {
        assert_eq!(ways_of("claude", None), [Way::Token, Way::Key, Way::Machine]);
        assert_eq!(ways_of("claude", Some("machine")), [Way::Machine, Way::Token, Way::Key]);
        assert_eq!(ways_of("claude", Some("key")), [Way::Key, Way::Token, Way::Machine]);
        assert!(ways_of("codex", Some("token")).is_empty(), "an agent that mints no token has one sign-in");
    }

    /// Settings standing on one computer's page off `place`, with no host behind it.
    fn computer_page(place: Value, cx: &mut gpui_kit::TestAppContext) -> &mut gpui_kit::VisualTestContext {
        use crate::screens::settings::model::{At, Data};
        use crate::screens::settings::SettingsScreen;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let preferences = json!({ "theme": "light", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
            "projectLook": {}, "projectOrder": [], "labs": false });
        let id = place["id"].as_str().unwrap().to_owned();
        let data = Data {
            preferences: serde_json::from_value(preferences).ok(),
            places: vec![serde_json::from_value(place).unwrap()],
            places_read: true,
            ..Data::default()
        };
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| SettingsScreen::with_data(data, At::Computer(id), runtime, window, cx));
            gpui_kit::component::Root::new(view, window, cx)
        });
        cx.simulate_resize(gpui_kit::size(gpui_kit::px(1280.), gpui_kit::px(1600.)));
        cx.run_until_parked();
        cx
    }

    fn box1(setup: Value, rows: Value, picks: Value) -> Value {
        json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "setup": setup,
            "applied": { "hash": "h", "at": "x", "rows": rows }, "picks": picks })
    }

    #[gpui_kit::test]
    fn a_waiting_sign_in_copies_its_code(cx: &mut gpui_kit::TestAppContext) {
        let setup = json!({ "state": "running", "addId": "a", "startedAt": "x", "steps": [{ "step": "agents", "state": "running" }],
            "waiting": [{ "row": "signins/codex", "label": "Codex", "expiresAt": "x", "state": "waiting", "code": "AB-12", "url": "https://x" }] });
        let cx = computer_page(box1(setup, json!([]), json!({ "name": "box1", "agents": {} })), cx);
        let copy = cx.debug_bounds("copy-code").expect("Copy stands beside the code").center();
        cx.simulate_click(copy, gpui_kit::Modifiers::none());
        cx.run_until_parked();
        assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("AB-12"));
    }

    #[gpui_kit::test]
    fn a_failed_sign_in_offers_each_way_the_one_it_was_set_up_with_first(cx: &mut gpui_kit::TestAppContext) {
        let setup = json!({ "state": "failed", "addId": "a", "startedAt": "x", "waiting": [],
            "steps": [{ "step": "agents", "state": "done" }] });
        let rows = json!([
            { "id": "agents/claude", "label": "Claude Code", "outcome": "installed", "step": "agents" },
            { "id": "signins/claude", "label": "Claude Code", "outcome": "failed", "step": "signins", "note": "no token" },
        ]);
        let picks = json!({ "name": "box1", "agents": { "claude": { "signin": "machine" } } });
        let cx = computer_page(box1(setup, rows, picks), cx);
        let mut x = |way: &'static str| cx.debug_bounds(way).unwrap_or_else(|| panic!("the act {way}")).origin.x;
        let (machine, token, key) = (x("way-machine"), x("way-token"), x("way-key"));
        assert!(machine < token && token < key, "machine, then token, then key: {machine:?} {token:?} {key:?}");
    }

    #[gpui_kit::test]
    fn a_warned_step_draws_its_triangle(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        struct One;
        impl Render for One {
            fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                line(&StepLine::new("x", "Copied with a warning", StepState::Warned), None, false, window, cx)
            }
        }
        let (_, cx) = cx.add_window_view(|_, _| One);
        cx.run_until_parked();
        assert!(cx.debug_bounds("step-warned").is_some());
    }

    #[gpui_kit::test]
    fn the_chevron_turns_a_quarter_over_150_ms(cx: &mut gpui_kit::TestAppContext) {
        use std::cell::Cell;
        use std::rc::Rc;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        struct Turning(bool, Rc<Cell<f32>>);
        impl Render for Turning {
            fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                self.1.set(turn("floor", self.0, window, cx));
                div()
            }
        }
        let seen = Rc::new(Cell::new(-1.));
        let (view, cx) = cx.add_window_view(|_, _| Turning(false, seen.clone()));
        cx.run_until_parked();
        assert_eq!(seen.get(), 0.);
        let at = |open: bool, ms: u64, cx: &mut gpui_kit::VisualTestContext| {
            cx.executor().advance_clock(Duration::from_millis(ms));
            view.update(cx, |v, cx| {
                v.0 = open;
                cx.notify();
            });
            cx.run_until_parked();
            seen.get()
        };
        let started = at(true, 0, cx);
        let midway = at(true, 75, cx);
        let ended = at(true, 100, cx);
        assert!(started < 1. && midway > 1. && midway < 89., "it eases: {started} then {midway}");
        assert_eq!(ended, 90.);
    }

    #[gpui_kit::test]
    fn a_step_log_scrolls_and_stands_on_its_newest_line(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        struct Log(Entity<List>, Vec<String>);
        impl Render for Log {
            fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
                div().w(px(400.)).child(step_log("floor", Some(&self.1), &self.0, window, cx))
            }
        }
        let lines: Vec<String> = (0..12).map(|n| format!("line {n} {}", "wrapping words ".repeat(8))).collect();
        let (_, cx) = cx.add_window_view(|_, cx| Log(cx.new(|_| List::default()), lines));
        cx.simulate_resize(gpui_kit::size(px(600.), px(800.)));
        cx.run_until_parked();
        cx.run_until_parked();
        let block = cx.debug_bounds("step-log").expect("the log");
        let last = cx.debug_bounds("step-log-last").expect("its newest line");
        assert_eq!(block.size.height, px(192.), "the log stands at its cap");
        assert!(
            last.bottom() <= block.bottom() + px(0.5) && last.top() >= block.top(),
            "the newest line is in view: {last:?} in {block:?}"
        );
        let bar = cx.debug_bounds("scrollbar").expect("the overflowing log draws its bar");
        assert_eq!((block.right() - bar.right(), bar.size.width), (px(1.), px(6.)), "the shared bar at the log's right edge");
        assert_eq!((bar.top() - block.top(), block.bottom() - bar.bottom()), (px(4.), px(4.)));
    }
}
