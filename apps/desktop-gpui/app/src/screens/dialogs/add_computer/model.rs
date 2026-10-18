// SPDX-License-Identifier: AGPL-3.0-only
//! Add a computer as the dialog reads it (`addFlow.ts`, `setup.ts`, `adds.ts`): the steps a person walks, the
//! install's job as the checks list, a computer's setup as its rows, a setup frame folded onto the setup, and a
//! step's last lines of output. Each record is the host's JSON as the protocol shapes it.

use serde_json::{json, Map, Value};

use super::words;
use crate::ui::Step;

/// The steps a person walks, then the running steps and the ready page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AddStep {
    Where,
    Checks,
    StartFrom,
    Agents,
    Mcp,
    Clis,
    Skills,
    Plugins,
    Github,
    Projects,
    Other,
    Summary,
    Running,
    Ready,
}

/// `ADD_STEPS`, in the order they run.
pub const ADD_STEPS: [AddStep; 12] = [
    AddStep::Where,
    AddStep::Checks,
    AddStep::StartFrom,
    AddStep::Agents,
    AddStep::Mcp,
    AddStep::Clis,
    AddStep::Skills,
    AddStep::Plugins,
    AddStep::Github,
    AddStep::Projects,
    AddStep::Other,
    AddStep::Summary,
];

impl AddStep {
    /// A step the picks are made on, whose foot says they are saved.
    pub fn picks(self) -> bool {
        !matches!(self, AddStep::Where | AddStep::Checks | AddStep::Running | AddStep::Ready)
    }
}

/// `stepsFor`: the steps this add walks, Start from only where a recipe is saved.
pub fn steps_for(recipes: usize) -> Vec<AddStep> {
    ADD_STEPS.into_iter().filter(|s| *s != AddStep::StartFrom || recipes > 0).collect()
}

/// `firstPick`: Start from where a recipe is saved, else Agents.
pub fn first_pick(recipes: usize) -> AddStep {
    if recipes > 0 {
        AddStep::StartFrom
    } else {
        AddStep::Agents
    }
}

/// One row of a list of steps (`StepLine`).
#[derive(Clone, Debug, PartialEq)]
pub struct StepLine {
    pub id: String,
    pub name: String,
    pub state: Step,
    pub note: Option<String>,
    pub ms: Option<u64>,
    pub said: Option<String>,
    pub fix: Option<String>,
    /// A sign-in's wait (`PlaceWait`).
    pub wait: Option<Value>,
    pub sub: bool,
    /// When a running step began, ms since the epoch.
    pub since: Option<f64>,
    /// The catalog id whose sign-in the row offers on the computer.
    pub sign_in: Option<String>,
    pub copy_keys: bool,
}

impl StepLine {
    fn new(id: impl Into<String>, name: impl Into<String>, state: Step) -> Self {
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
            since: None,
            sign_in: None,
            copy_keys: false,
        }
    }
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn arr<'a>(v: Option<&'a Value>, key: &str) -> &'a [Value] {
    v.and_then(|v| v.get(key)).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

const INSTALL: &str = "Install wsp";

/// `CHECK_ROWS`: the install as the checks list draws it, each row off the job's steps it spans.
const CHECK_ROWS: [(&str, &str, &[&str]); 7] = [
    ("connect", "Connected", &["connect"]),
    ("chip", "Chip", &["chip"]),
    ("root", "Root", &["root"]),
    ("system", "System", &["system"]),
    ("disk", "Disk", &["disk"]),
    ("reach", "Connects back", &["reach"]),
    ("wsp", INSTALL, &["wsp", "service", "join"]),
];

/// `addFix`: the host's own fix, else the sudo fix or the login fix by the refusal's kind.
pub fn add_fix(job: &Value) -> Option<String> {
    match str_of(job, "kind") {
        Some(words::SUDO_KIND) => Some(words::SUDO_FIX.into()),
        kind => {
            str_of(job, "fix").map(str::to_owned).or_else(|| (kind == Some(words::LOGIN_REFUSED_KIND)).then(|| words::REFUSED_FIX.into()))
        }
    }
}

/// `checkRows`: each row off the steps it spans; the row an install stopped at carries the refusal: the one with a
/// failed step, else the one running, else the first not done.
pub fn check_rows(job: Option<&Value>) -> Vec<StepLine> {
    let steps = arr(job, "steps");
    let state = job.and_then(|j| str_of(j, "state"));
    let mine = |names: &[&str]| -> Vec<&Value> { steps.iter().filter(|s| str_of(s, "step").is_some_and(|n| names.contains(&n))).collect() };
    let done = |id: &str, names: &[&str]| {
        if id == "wsp" {
            state == Some("done")
        } else {
            names.iter().all(|n| steps.iter().any(|s| str_of(s, "step") == Some(n) && str_of(s, "state") == Some("done")))
        }
    };
    let failed_at = (state == Some("failed"))
        .then(|| {
            let with = |st: &str| CHECK_ROWS.iter().find(|(_, _, names)| mine(names).iter().any(|s| str_of(s, "state") == Some(st)));
            with("failed").or_else(|| with("running")).or_else(|| CHECK_ROWS.iter().find(|(id, _, names)| !done(id, names))).map(|r| r.0)
        })
        .flatten();
    CHECK_ROWS
        .iter()
        .map(|(id, name, names)| {
            let mine = mine(names);
            let st = if Some(*id) == failed_at {
                Step::Failed
            } else if done(id, names) {
                Step::Done
            } else if !mine.is_empty() && state == Some("running") {
                Step::Running
            } else {
                Step::NotStarted
            };
            let mut line = StepLine::new(*id, *name, st);
            let timed: Vec<u64> = mine.iter().filter_map(|s| s.get("ms").and_then(Value::as_u64)).collect();
            if st == Step::Done && !timed.is_empty() {
                line.ms = Some(timed.iter().sum());
            }
            if st == Step::Failed {
                line.said = Some(job.and_then(|j| str_of(j, "said")).unwrap_or_default().to_owned());
                line.fix = job.and_then(add_fix);
            } else {
                line.note =
                    mine.iter().rev().find(|s| str_of(s, "state") != Some("failed")).and_then(|s| str_of(s, "note")).map(str::to_owned);
            }
            line
        })
        .collect()
}

/// `askedHostKey`: the key a computer never dialled answered with, on an add refused for it.
pub fn asked_host_key(job: Option<&Value>) -> Option<String> {
    let job = job?;
    (str_of(job, "state") == Some("failed") && str_of(job, "kind") == Some(words::HOST_KEY_KIND))
        .then(|| str_of(job, "hostKey").map(str::to_owned))
        .flatten()
}

/// `askedSudo`: the add stopped for the password the login's sudo asks for.
pub fn asked_sudo(job: Option<&Value>) -> bool {
    job.is_some_and(|j| str_of(j, "state") == Some("failed") && str_of(j, "kind") == Some(words::SUDO_KIND))
}

/// `withPlaceStage`: one step of an install onto its job.
pub fn with_place_stage(job: &Value, e: &Value) -> Value {
    let mut job = job.clone();
    let mut line = Map::new();
    for key in ["step", "state", "note", "ms"] {
        if let Some(v) = e.get(key).filter(|v| !v.is_null()) {
            line.insert(key.into(), v.clone());
        }
    }
    let step = str_of(e, "step").unwrap_or_default().to_owned();
    let state = str_of(e, "state").unwrap_or_default().to_owned();
    let steps = job.get_mut("steps").and_then(Value::as_array_mut);
    if let Some(steps) = steps {
        match steps.iter_mut().find(|s| str_of(s, "step") == Some(step.as_str())) {
            Some(held) => *held = Value::Object(line),
            None => steps.push(Value::Object(line)),
        }
    }
    if step == "join" && state == "done" {
        job["state"] = json!("done");
        if let Some(place) = e.get("placeId").filter(|v| !v.is_null()) {
            job["placeId"] = place.clone();
        }
    } else if state == "failed" {
        job["state"] = json!("failed");
        if job.get("said").is_none_or(Value::is_null) {
            if let Some(note) = e.get("note").filter(|v| !v.is_null()) {
                job["said"] = note.clone();
            }
        }
    }
    job
}

/// `foldSetup`: a setup frame onto the setup the record held. Frames for another run leave it as it was.
pub fn fold_setup(setup: &Value, e: &Value) -> Value {
    if str_of(setup, "addId") != str_of(e, "addId") {
        return setup.clone();
    }
    let mut next = setup.clone();
    let steps = |next: &mut Value| -> Vec<Value> { next.get("steps").and_then(Value::as_array).cloned().unwrap_or_default() };
    if let Some(line) = e.get("line").filter(|v| !v.is_null()) {
        let mut list = steps(&mut next);
        match list.iter_mut().find(|s| s.get("step") == line.get("step")) {
            Some(held) => *held = line.clone(),
            None => list.push(line.clone()),
        }
        next["steps"] = Value::Array(list);
    }
    for step in arr(Some(e), "running") {
        let mut list = steps(&mut next);
        match list.iter_mut().find(|s| s.get("step") == Some(step)) {
            Some(held) if str_of(held, "state") == Some("running") => {}
            Some(held) => *held = json!({ "step": step, "state": "running" }),
            None => list.push(json!({ "step": step, "state": "running" })),
        }
        next["steps"] = Value::Array(list);
    }
    let mut waiting: Vec<Value> = arr(Some(&next), "waiting").to_vec();
    if let Some(wait) = e.get("wait").filter(|v| !v.is_null()) {
        waiting.retain(|w| w.get("row") != wait.get("row"));
        waiting.push(wait.clone());
    }
    if let Some(landed) = str_of(e, "landed") {
        waiting.retain(|w| str_of(w, "row") != Some(landed));
    }
    next["waiting"] = Value::Array(waiting);
    if let Some(end) = str_of(e, "end") {
        next["state"] = json!(if end == "failed" { "failed" } else { "done" });
        if end == "failed" {
            if let Some(said) = e.get("said").filter(|v| !v.is_null()) {
                next["said"] = said.clone();
            }
        }
    }
    next
}

/// `runningSince`: when the host started a running step, ms since the epoch.
pub fn running_since(setup: Option<&Value>, step: &str) -> Option<f64> {
    let at = arr(setup, "steps").iter().find(|s| str_of(s, "step") == Some(step)).and_then(|s| str_of(s, "startedAt"))?;
    chrono::DateTime::parse_from_rfc3339(at).ok().map(|t| t.timestamp_millis() as f64)
}

/// `githubPick`: the GitHub choice, a missing row a skip.
pub fn github_pick(picks: &Value) -> &str {
    match picks.pointer("/configs/github") {
        None | Some(Value::Null) => "skip",
        Some(row) => str_of(row, "signin").unwrap_or("vault"),
    }
}

/// `signInOfRow`: the catalog id whose login a row stands for.
pub fn sign_in_of_row(id: &str) -> Option<String> {
    match id.strip_prefix("signins/") {
        Some(agent) => Some(agent.to_owned()),
        None if id == "github" => Some("gh".into()),
        None => None,
    }
}

/// `signInWaysOf`: the ways an agent that mints its token on the computer running the host signs in on one you own,
/// the way its picks name first; none for any other agent, whose row offers its own sign-in there.
pub fn sign_in_ways(agent: &str, picks: Option<&Value>) -> Vec<&'static str> {
    let ways = crate::ui::catalog::sign_in_ways(agent);
    let first = picks.and_then(|p| p.pointer(&format!("/agents/{agent}/signin"))).and_then(Value::as_str);
    let mut out: Vec<&'static str> = ways.iter().copied().filter(|w| Some(*w) == first).collect();
    out.extend(ways.iter().copied().filter(|w| Some(*w) != first));
    out
}

/// `agentOfRow`.
fn agent_of_row(id: &str) -> Option<&str> {
    if id.starts_with("agents/mcp/") {
        return None;
    }
    id.strip_prefix("agents/")
}

/// What a running step is putting on, off the picks: the verb, the noun a step of many like rows is counted by, and
/// each picked row by its key and its name.
type Doing = (&'static str, Option<&'static str>, Vec<(String, String)>);

fn doing(step: &str, picks: &Value) -> Option<Doing> {
    let keys =
        |kind: &str| -> Vec<String> { picks.get(kind).and_then(Value::as_object).map(|t| t.keys().cloned().collect()).unwrap_or_default() };
    let keyed = |kind: &str| keys(kind).into_iter().map(|k| (k.clone(), k)).collect::<Vec<_>>();
    Some(match step {
        "agents" => ("Installing", None, keys("agents").into_iter().map(|k| (k.clone(), crate::ui::catalog::name(&k))).collect()),
        "mcp" => ("Copying", None, keyed("mcp")),
        "clis" => ("Installing", None, keyed("clis")),
        "skills" => ("Copying", Some("skills"), keyed("skills")),
        "plugins" => (
            "Installing",
            Some("plugins"),
            keys("plugins").into_iter().map(|k| (k.clone(), k.split('@').next().unwrap_or_default().to_owned())).collect(),
        ),
        "folders" => (
            "Importing",
            None,
            picks
                .get("folders")
                .and_then(Value::as_object)
                .map(|t| t.iter().map(|(k, f)| (k.clone(), str_of(f, "name").unwrap_or(k).to_owned())).collect())
                .unwrap_or_default(),
        ),
        "configs" => (
            "Copying",
            None,
            ["git", "shell"]
                .into_iter()
                .filter(|id| picks.pointer(&format!("/configs/{id}")).is_some_and(|v| !v.is_null()))
                .map(|id| (id.to_owned(), id.to_owned()))
                .collect(),
        ),
        _ => return None,
    })
}

fn landed(r: &Value) -> bool {
    matches!(str_of(r, "outcome"), Some("installed" | "present"))
}

/// `doingLine`: the picks less the rows already there, named, or counted past three where the step counts them.
fn doing_line(step: &str, picks: &Value, rows: &[&Value]) -> Option<String> {
    let (verb, counted, picked) = doing(step, picks)?;
    let there = |key: &str, name: &str| {
        rows.iter().any(|r| {
            let label = str_of(r, "label").unwrap_or_default();
            landed(r)
                && (str_of(r, "id").unwrap_or_default().ends_with(&format!("/{key}"))
                    || label == name
                    || label.starts_with(&format!("{name} ")))
        })
    };
    let names: Vec<String> = picked.into_iter().filter(|(k, n)| !there(k, n)).map(|(_, n)| n).collect();
    if names.is_empty() {
        return None;
    }
    let said = match counted {
        Some(noun) if names.len() > 3 => format!("{} {noun}", names.len()),
        _ if names.len() <= 3 => names.join(", "),
        _ => format!("{} and {} more", names[..2].join(", "), names.len() - 2),
    };
    Some(format!("{verb} {said}."))
}

/// `countLine`: what a step put there, counted, never named.
fn count_line(step: &str, rows: &[&Value]) -> Option<String> {
    if rows.is_empty() {
        return None;
    }
    if step != "mcp" {
        let noun = match step {
            "floor" => "package",
            "agents" => "agent",
            "clis" => "CLI",
            "skills" => "skill",
            "plugins" => "plugin",
            "folders" => "project",
            "configs" => "file",
            _ => "item",
        };
        return Some(words::plural(rows.len(), noun));
    }
    let servers = rows.iter().filter(|r| str_of(r, "id").unwrap_or_default().starts_with("agents/mcp/")).count();
    let parts: Vec<String> = [
        (servers > 0).then(|| words::plural(servers, "MCP server")),
        (servers < rows.len()).then(|| words::plural(rows.len() - servers, "agent file")),
    ]
    .into_iter()
    .flatten()
    .collect();
    Some(parts.join(", "))
}

/// `SETUP_ROWS`: the steps as the person chose them, by the name each goes by.
const SETUP_ROWS: [(&str, &str); 9] = [
    ("floor", "Base packages"),
    ("agents", "Agents"),
    ("mcp", "MCP servers"),
    ("clis", "CLIs"),
    ("skills", "Skills"),
    ("plugins", "Plugins"),
    ("github", "GitHub"),
    ("folders", "Projects"),
    ("configs", "Other config"),
];

/// `setupRows`: Install wsp, then each step with the items of it that did not land under it, the sign-ins under
/// Agents. `name` is the computer's, for the words that name it.
pub fn setup_rows(place: &Value, name: &str) -> Vec<StepLine> {
    let setup = place.get("setup").filter(|v| !v.is_null());
    let picks = place.get("picks").filter(|v| !v.is_null());
    let recipe = str_of(place, "recipe").filter(|r| *r != "none").and(picks.and_then(|p| str_of(p, "name")));
    let rows = arr(place.get("applied"), "rows");
    let mut out = vec![StepLine { note: Some(words::INSTALLED.into()), ..StepLine::new("wsp", INSTALL, Step::Done) }];
    for (step, title) in SETUP_ROWS {
        let line = arr(setup, "steps").iter().find(|s| str_of(s, "step") == Some(step));
        let mine: Vec<&Value> = rows
            .iter()
            .filter(|r| str_of(r, "step") == Some(step) || (step == "mcp" && str_of(r, "step") == Some("folderServers")))
            .collect();
        let landed_rows: Vec<&Value> = mine.iter().copied().filter(|r| landed(r)).collect();
        let line_state = line.and_then(|l| str_of(l, "state"));
        let stopped = setup.and_then(|s| str_of(s, "state")) == Some("failed") && line_state == Some("failed");
        let missed = mine.is_empty() || mine.iter().any(|r| str_of(r, "outcome") == Some("failed"));
        let github = (step == "github").then(|| mine.iter().copied().find(|r| str_of(r, "id") == Some("github"))).flatten();
        let ran = match line_state {
            None => Step::NotStarted,
            Some("running") => Step::Running,
            Some(st) if stopped || (st == "failed" && landed_rows.is_empty() && missed) => Step::Failed,
            Some(_) => Step::Done,
        };
        let state = match github {
            Some(g) if ran != Step::Running => match str_of(g, "outcome") {
                Some("skipped") => Step::Skipped,
                Some("failed") => Step::Failed,
                _ => ran,
            },
            _ => ran,
        };
        let note = if stopped {
            None
        } else if line_state == Some("running") {
            picks.and_then(|p| doing_line(step, p, &mine))
        } else {
            let ended = (step == "github").then(|| github_ended(github, picks, name)).flatten();
            ended.or_else(|| count_line(step, &landed_rows)).or_else(|| line.and_then(|l| str_of(l, "note")).map(str::to_owned))
        };
        let mut row = StepLine { note, ms: line.and_then(|l| l.get("ms")).and_then(Value::as_u64), ..StepLine::new(step, title, state) };
        if stopped {
            row.said = setup.and_then(|s| str_of(s, "said")).map(str::to_owned);
        }
        if let Some(g) = github.filter(|g| state == Step::Failed && str_of(g, "outcome") == Some("failed") && !stopped) {
            row.said = str_of(g, "note").map(str::to_owned);
            row.fix = str_of(g, "fix").map(str::to_owned);
        }
        if step == "github" && ran != Step::Running {
            row.sign_in = sign_in_offer(github);
        }
        out.push(row);
        if step == "agents" {
            out.extend(sign_in_rows(setup, rows, name, place.get("signIns")));
        }
        if !stopped {
            for r in &mine {
                let id = str_of(r, "id").unwrap_or_default();
                let label = str_of(r, "label").unwrap_or_default();
                if str_of(r, "outcome") == Some("failed") && Some(*r) != github {
                    out.push(StepLine {
                        sub: true,
                        said: str_of(r, "note").map(str::to_owned),
                        fix: str_of(r, "fix").map(str::to_owned),
                        ..StepLine::new(id, label, Step::Failed)
                    });
                }
            }
            for r in &mine {
                let keys = r.get("keys").and_then(Value::as_array);
                if str_of(r, "outcome") == Some("skipped") {
                    if let Some(keys) = keys {
                        let keys: Vec<String> = keys.iter().filter_map(Value::as_str).map(str::to_owned).collect();
                        out.push(StepLine {
                            sub: true,
                            copy_keys: true,
                            note: Some(words::keys_not_copied(&keys, recipe)),
                            ..StepLine::new(str_of(r, "id").unwrap_or_default(), str_of(r, "label").unwrap_or_default(), Step::Skipped)
                        });
                    }
                }
            }
        }
    }
    out
}

/// `githubEnded`: the GitHub row by the way it signed in.
fn github_ended(row: Option<&Value>, picks: Option<&Value>, name: &str) -> Option<String> {
    let row = row?;
    match str_of(row, "outcome") {
        Some("skipped") => Some(words::SKIPPED.into()),
        Some("installed" | "present") => {
            let there = str_of(row, "note") == Some("signed in on that computer") || picks.is_some_and(|p| github_pick(p) == "machine");
            Some(if there { words::signed_in_on(name) } else { words::TOKEN_COPIED.into() })
        }
        _ => None,
    }
}

/// `signInOffer`: the sign-in a row offers on the computer, on a row the person skipped or that failed.
fn sign_in_offer(row: Option<&Value>) -> Option<String> {
    let row = row?;
    matches!(str_of(row, "outcome"), Some("skipped" | "failed")).then(|| sign_in_of_row(str_of(row, "id")?)).flatten()
}

/// `signInRows`: the sign-ins under Agents, each by its agent; one that waits on the person off its wait.
fn sign_in_rows(setup: Option<&Value>, rows: &[Value], name: &str, sign_ins: Option<&Value>) -> Vec<StepLine> {
    let waits = arr(setup, "waiting");
    let installing = |agent: Option<&str>| {
        agent.is_some_and(|a| {
            rows.iter().find(|r| agent_of_row(str_of(r, "id").unwrap_or_default()) == Some(a)).and_then(|r| str_of(r, "outcome"))
                == Some("failed")
        })
    };
    let mut out: Vec<StepLine> = rows
        .iter()
        .filter(|r| str_of(r, "step") == Some("signins") && !waits.iter().any(|w| w.get("row") == r.get("id")))
        .map(|r| {
            let id = str_of(r, "id").unwrap_or_default();
            let label = str_of(r, "label").unwrap_or_default();
            let row_name = format!("{label} sign-in");
            let outcome = str_of(r, "outcome").unwrap_or_default();
            let agent = sign_in_of_row(id);
            if matches!(outcome, "failed" | "skipped") && installing(agent.as_deref()) {
                return StepLine {
                    sub: true,
                    note: Some(words::waits_for_install(label)),
                    ..StepLine::new(id, row_name, Step::NotStarted)
                };
            }
            let base = StepLine {
                sub: true,
                ms: r.get("ms").and_then(Value::as_u64),
                sign_in: sign_in_offer(Some(r)),
                ..StepLine::new(id, row_name, Step::Done)
            };
            match outcome {
                "failed" => StepLine {
                    state: Step::Failed,
                    said: str_of(r, "note").map(str::to_owned),
                    fix: str_of(r, "fix").map(str::to_owned),
                    ..base
                },
                "skipped" => {
                    let note = match str_of(r, "note") {
                        Some("signs in at its own terminal on that computer") => words::at_its_terminal(name),
                        Some(n) if n == format!("waits for {label} to install") => words::NOT_SIGNED_IN_YET.into(),
                        _ => words::SKIPPED.into(),
                    };
                    StepLine { state: Step::Skipped, note: Some(note), ..base }
                }
                _ => {
                    let state = agent.as_deref().and_then(|a| sign_ins.and_then(|s| s.get(a))).and_then(Value::as_str);
                    let note = match state {
                        Some("vault-key") => words::SIGNED_IN_WITH_TOKEN.into(),
                        Some("none") => words::NOT_SIGNED_IN_YET.into(),
                        Some("signed-in") => words::signed_in_on(name),
                        _ if str_of(r, "note") == Some("signed in on that computer") => words::signed_in_on(name),
                        _ => words::TOKEN_COPIED.into(),
                    };
                    StepLine { note: Some(note), ..base }
                }
            }
        })
        .collect();
    for w in waits {
        let waiting = str_of(w, "state") == Some("waiting");
        out.push(StepLine {
            sub: true,
            wait: Some(w.clone()),
            note: (w.get("url").is_some_and(|u| !u.is_null()) && waiting).then(|| words::TAB_OPENED.into()),
            ..StepLine::new(
                str_of(w, "row").unwrap_or_default(),
                format!("{} sign-in", str_of(w, "label").unwrap_or_default()),
                Step::NeedsYou,
            )
        });
    }
    out
}

/// `setupCount`: the steps done of all, not the items under them; a step set aside is through.
pub fn setup_count(rows: &[StepLine]) -> (usize, usize) {
    let steps: Vec<&StepLine> = rows.iter().filter(|r| !r.sub).collect();
    (steps.iter().filter(|r| matches!(r.state, Step::Done | Step::Skipped)).count(), steps.len())
}

/// How a setup stands as a header reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standing {
    Running,
    NeedsYou,
    Failed,
    Ready,
}

/// `setupStanding`, off the host's word for the computer: Setup failed, then Needs you where a sign-in waits or a
/// folder or the GitHub sign-in failed, else ready.
pub fn standing(place: &Value) -> Standing {
    let Some(setup) = place.get("setup").filter(|v| !v.is_null()) else { return Standing::Running };
    match str_of(setup, "state") {
        Some("running") | None => Standing::Running,
        Some("failed") => Standing::Failed,
        _ if !arr(Some(setup), "waiting").is_empty() => Standing::NeedsYou,
        _ if arr(place.get("applied"), "rows")
            .iter()
            .any(|r| str_of(r, "outcome") == Some("failed") && matches!(str_of(r, "step"), Some("folders" | "github"))) =>
        {
            Standing::NeedsYou
        }
        _ => Standing::Ready,
    }
}

/// `opensLog`: a step of the setup, not an item under one, that has started.
pub fn opens_log(row: &StepLine) -> bool {
    !row.sub && row.state != Step::NotStarted && words::setup_step_words(&row.id).is_some()
}

/// How many of a step's last lines its row opens on, and how much of one line it keeps.
pub const LOG_LINES: usize = 12;
pub const LOG_CHARS: usize = 300;

/// `stepLogs`: the last lines each step wrote, off the end of the setup log, oldest first; the line marking a step
/// running or done is passed over.
pub fn step_logs(lines: &[String]) -> std::collections::HashMap<String, Vec<String>> {
    let mut out: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    for line in lines {
        let Some((_, rest)) = line.split_once(' ') else { continue };
        let Some(rest) = rest.strip_prefix('[') else { continue };
        let Some((step, said)) = rest.split_once("] ") else { continue };
        if !step.chars().all(|c| c.is_ascii_lowercase()) {
            continue;
        }
        let Some(words) = words::setup_step_words(step) else { continue };
        let said = said.trim();
        if said.is_empty() || said.starts_with(&format!("{words}: ")) {
            continue;
        }
        let held = out.entry(step.to_owned()).or_default();
        held.push(if said.chars().count() > LOG_CHARS {
            format!("{}…", said.chars().take(LOG_CHARS - 1).collect::<String>())
        } else {
            said.to_owned()
        });
        if held.len() > LOG_LINES {
            held.remove(0);
        }
    }
    out
}

/// `tooBig`: the picks with the room kept free need more than the computer has free, as last weighed.
pub fn too_big(estimate: Option<&Value>) -> bool {
    let Some(e) = estimate else { return false };
    let n = |k: &str| e.get(k).and_then(Value::as_u64);
    matches!((n("freeBytes"), n("neededBytes"), n("keptBytes")), (Some(free), Some(need), Some(kept)) if free < need + kept)
}

/// Whether picks hold nothing at all.
pub fn empty(picks: Option<&Value>) -> bool {
    picks.is_none_or(|p| words::recipe_counts(p).iter().all(|(_, n)| *n == 0))
}

/// The preferences patch that keeps a saved recipe's icon with the person's other looks (`recipeLook`), merged by slug.
pub fn recipe_look(slug: &str, icon: &str) -> Value {
    json!({ "patch": { "recipeLook": { slug: { "icon": icon } } } })
}

/// A refusal as the host said it (`refusalParts`): the sentence, and what to do about it where the host said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub said: String,
    pub fix: Option<String>,
}

impl Said {
    pub fn plain(said: impl Into<String>) -> Self {
        Said { said: said.into(), fix: None }
    }
}

impl From<&wsp_host_client::Failure> for Said {
    fn from(f: &wsp_host_client::Failure) -> Self {
        Said { said: f.said().to_owned(), fix: f.fix.as_deref().map(str::to_owned) }
    }
}

/// `placeName`: its label, else its name.
pub fn place_name(place: &Value) -> String {
    crate::screens::sidebar::looks::place_name(place)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(steps: Value, state: &str) -> Value {
        json!({ "addId": "a1", "address": "box", "startedAt": "2026-10-18T00:00:00Z", "state": state, "steps": steps })
    }

    #[test]
    fn a_saved_recipes_icon_is_kept_under_its_slug() {
        assert_eq!(recipe_look("lab-boxes", "rocket"), json!({ "patch": { "recipeLook": { "lab-boxes": { "icon": "rocket" } } } }));
    }

    #[test]
    fn the_checks_stand_where_the_install_stopped() {
        let rows = check_rows(Some(&job(
            json!([{ "step": "connect", "state": "done", "ms": 300 }, { "step": "chip", "state": "running" }]),
            "running",
        )));
        assert_eq!(rows.iter().map(|r| r.state).collect::<Vec<_>>()[..3], [Step::Done, Step::Running, Step::NotStarted]);
        assert_eq!(rows[0].ms, Some(300));
        let mut failed = job(json!([{ "step": "connect", "state": "failed", "note": "no route" }]), "failed");
        failed["said"] = json!("ssh: connect to host 10.255.255.1 port 22: no route");
        failed["kind"] = json!("login");
        let rows = check_rows(Some(&failed));
        assert_eq!(rows[0].state, Step::Failed);
        assert_eq!(rows[0].fix.as_deref(), Some(words::REFUSED_FIX));
        assert!(rows[1..].iter().all(|r| r.state == Step::NotStarted));
    }

    #[test]
    fn a_stage_moves_its_job_and_a_join_ends_it() {
        let j = job(json!([]), "running");
        let j = with_place_stage(&j, &json!({ "type": "place.stage", "addId": "a1", "step": "connect", "state": "done", "ms": 12 }));
        assert_eq!(j["steps"][0]["step"], "connect");
        let j = with_place_stage(&j, &json!({ "type": "place.stage", "addId": "a1", "step": "join", "state": "done", "placeId": "p1" }));
        assert_eq!((j["state"].as_str(), j["placeId"].as_str()), (Some("done"), Some("p1")));
    }

    #[test]
    fn a_setup_frame_folds_onto_its_run_only() {
        let setup = json!({ "state": "running", "addId": "s1", "startedAt": "x", "steps": [], "waiting": [] });
        let e = json!({ "addId": "s1", "running": ["floor"], "wait": { "row": "signins/claude", "label": "Claude Code", "expiresAt": "x", "state": "waiting" } });
        let next = fold_setup(&setup, &e);
        assert_eq!(next["steps"][0]["state"], "running");
        assert_eq!(next["waiting"].as_array().unwrap().len(), 1);
        let landed = fold_setup(&next, &json!({ "addId": "s1", "landed": "signins/claude", "end": "ready" }));
        assert_eq!((landed["waiting"].as_array().unwrap().len(), landed["state"].as_str()), (0, Some("done")));
        assert_eq!(fold_setup(&setup, &json!({ "addId": "other", "end": "failed" })), setup);
    }

    #[test]
    fn a_step_log_keeps_its_last_twelve_lines_cut_at_300() {
        let mut lines: Vec<String> = (0..20).map(|i| format!("2026-10-18T00:00:00Z [clis] line {i}")).collect();
        lines.push("2026-10-18T00:00:00Z [clis] the CLIs: done".into());
        lines.push(format!("2026-10-18T00:00:00Z [floor] {}", "x".repeat(400)));
        let logs = step_logs(&lines);
        assert_eq!(logs["clis"].len(), 12);
        assert_eq!(logs["clis"][0], "line 8");
        assert_eq!(logs["floor"][0].chars().count(), 300);
    }

    #[test]
    fn setup_rows_count_steps_and_name_what_waits() {
        let place = json!({
            "id": "p1", "name": "box",
            "setup": { "state": "running", "addId": "s1", "startedAt": "x", "steps": [
                { "step": "floor", "state": "done", "ms": 4000 }, { "step": "agents", "state": "running" }
            ], "waiting": [{ "row": "signins/claude", "label": "Claude Code", "url": "https://x", "code": "AB-CD", "expiresAt": "x", "state": "waiting" }] },
            "picks": { "name": "box", "agents": { "claude": {}, "codex": {} } }
        });
        let rows = setup_rows(&place, "box");
        assert_eq!(rows[1].ms, Some(4000));
        assert_eq!(rows[2].state, Step::Running);
        assert!(rows[2].note.as_deref().unwrap().starts_with("Installing "));
        let wait = rows.iter().find(|r| r.id == "signins/claude").unwrap();
        assert_eq!((wait.state, wait.note.as_deref()), (Step::NeedsYou, Some(words::TAB_OPENED)));
        assert_eq!(setup_count(&rows), (2, 10));
    }
}
