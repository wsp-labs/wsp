// SPDX-License-Identifier: AGPL-3.0-only
//! The words the Computers pages say and the readings behind them, after `places.ts`, `format.ts`'s `WHERE_WORDS`,
//! `COMPUTER_PAGE_WORDS` and the protocol's place words they read: a row's name, its state cell, its sentences, the
//! Remove dialog's sentence, a cloud's plan, and the units every figure is said in.

use std::collections::HashMap;

use crate::ui::{catalog, MarkState};
use wsp_host_client::protocol::{
    PendingComputer, PendingComputerStep, PlaceHolds, PlaceSetup, PlaceSetupState, PlaceSetupStepsItemState, PlaceSetupStepsItemStep,
    PlaceSetupWaitingItemState, PlaceView, PlaceViewApplied, PlaceViewAppliedRowsItemKind, PlaceViewAppliedRowsItemOutcome,
    PlaceViewAppliedRowsItemStep, PlaceViewKind, PlaceViewSignInsValue, PlaceViewSync, PlaceViewSyncState, SessionView, WorkspaceOut,
};

use crate::screens::settings::pages::agents::catalog::AGENTS;
pub use crate::screens::settings::words::{built_when, capitalised, here_name, is_here, HERE};
use crate::screens::sidebar::menu::{self, values};
pub use crate::screens::terminal::state::Absent;
pub use crate::ui::fmt_bytes;

/// `NO_RECIPE`: what a computer that follows no recipe follows.
pub const NO_RECIPE: &str = "none";

/// `PROVIDER_KEY_WORDS`: each cloud this wsp can hold a key for, by the id its row keys it by, in the protocol's order.
pub struct ProviderWords {
    pub id: &'static str,
    pub name: &'static str,
    pub key_console: Option<&'static str>,
    /// A person meets this cloud through a lead's sub-threads alone.
    pub sub_threads: bool,
}

pub const PROVIDERS: [ProviderWords; 2] = [
    ProviderWords { id: "box", name: "Boat", key_console: Some("boat.dev/dashboard"), sub_threads: true },
    ProviderWords { id: "solari", name: "Solari", key_console: Some("console.getsolari.com"), sub_threads: false },
];

pub fn provider_words(id: &str) -> Option<&'static ProviderWords> {
    PROVIDERS.iter().find(|p| p.id == id)
}

/// `providerKeyName`: a cloud by the name its key rows give it; an id with no row reads as itself.
pub fn provider_key_name(id: &str) -> String {
    provider_words(id).map(|p| p.name.to_owned()).unwrap_or_else(|| id.to_owned())
}

/// `subThreadsOnly`.
pub fn sub_threads_only(id: &str) -> bool {
    provider_words(id).is_some_and(|p| p.sub_threads)
}

pub fn is_provider(place: &PlaceView) -> bool {
    matches!(place.kind, PlaceViewKind::Provider)
}

/// `placeName`: the name its owner gave the computer where it keeps one, a cloud by its key rows' name, every other
/// computer by the name it reported.
pub fn place_name(place: &PlaceView) -> String {
    if let Some(label) = &place.label {
        return label.clone();
    }
    if !is_provider(place) {
        return place.name.clone();
    }
    provider_key_name(&place.name)
}

/// `namesPlace`: the id the wire keys it by, or the name a person types; a cloud also by its key rows' name, in any case.
pub fn names_place(place: &PlaceView, word: &str) -> bool {
    if word == place.id || word == place.name {
        return true;
    }
    is_provider(place)
        && [place.id.clone(), place.name.clone(), provider_key_name(&place.name)].iter().any(|n| n.to_lowercase() == word.to_lowercase())
}

/// `plural`.
pub fn plural(n: i64, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// `nameList`: each name quoted where it carries a comma itself.
pub fn name_list(names: &[String]) -> String {
    names
        .iter()
        .map(|n| if n.contains(',') { serde_json::to_string(n).unwrap_or_else(|_| n.clone()) } else { n.clone() })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A JS number as `String(n)` writes it, for the figures the protocol rounds to one place.
pub fn js_number(n: f64) -> String {
    if n.fract() == 0. && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n}");
        s.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

fn mem_gb(mem_mb: f64) -> f64 {
    format!("{:.1}", mem_mb / 1024.).parse().unwrap_or(0.)
}

/// `fmtMemGb`.
pub fn fmt_mem_gb(mem_mb: f64) -> String {
    format!("{} GB", js_number(mem_gb(mem_mb)))
}

/// `fmtCpus` in a computer's own word, cores: "1 core", "10 cores".
pub fn fmt_cores(n: f64) -> String {
    format!("{} {}", js_number(n), if n == 1. { "core" } else { "cores" })
}

/// `fmtRate`.
pub fn fmt_rate(usd_per_hour: f64) -> String {
    let mils = format!("{usd_per_hour:.3}");
    let dollars = if mils.ends_with('0') { format!("{usd_per_hour:.2}") } else { mils };
    format!("${dollars}/hr")
}

/// `fmtCost`: dollars with cents and the en-US grouping.
pub fn fmt_cost(usd: f64) -> String {
    let fixed = format!("{:.2}", usd.abs());
    let (whole, cents) = fixed.split_once('.').unwrap_or((&fixed, "00"));
    let mut grouped = String::new();
    for (ix, ch) in whole.chars().enumerate() {
        if ix > 0 && (whole.len() - ix) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}${grouped}.{cents}", if usd < 0. { "-" } else { "" })
}

/// `sizeWord`: "2x4".
pub fn size_word(cpu: f64, mem_mb: f64) -> String {
    format!("{}x{}", js_number(cpu), js_number(mem_gb(mem_mb)))
}

/// `sizeFromWord`.
pub fn size_from_word(word: &str) -> Option<(i64, i64)> {
    let (cpu, mem) = word.trim().split_once('x')?;
    let cpu: i64 = cpu.parse().ok().filter(|_| cpu.chars().all(|c| c.is_ascii_digit()))?;
    if mem.is_empty() || !mem.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let mem_mb = (mem.parse::<f64>().ok()? * 1024.).round() as i64;
    (cpu > 0 && mem_mb > 0).then_some((cpu, mem_mb))
}

/// `placeIsOffline`.
pub fn is_offline(place: &PlaceView) -> bool {
    place.present == Some(false)
}

/// `absentOf`, the sidebar's one reading of a row.
pub fn absent_of(place: &PlaceView) -> Option<Absent> {
    menu::absent_of(&serde_json::to_value(place).ok()?)
}

/// `SETUP_WORDS`.
pub mod setup_words {
    pub const FAILED: &str = "Setup failed";
    pub const NEEDS_YOU: &str = "Needs you";
    pub const PENDING: &str = "Pending";
    pub const SETTING_UP: &str = "Setting up";
    pub const BEHIND: &str = "Behind";
    pub const READY: &str = "Ready";
}

/// `SETUP_STEP_WORDS`.
pub fn step_words(step: &PlaceSetupStepsItemStep) -> &'static str {
    use PlaceSetupStepsItemStep as S;
    match step {
        S::Floor => "the base tools",
        S::Agents => "the agents",
        S::Signins => "the agents' sign-ins",
        S::Clis => "the CLIs",
        S::Skills => "the skills",
        S::Mcp => "the agents' own files and MCP servers",
        S::Plugins => "the plugins",
        S::Configs => "git and the shell",
        S::Folders => "the folders, as projects",
        S::FolderServers => "the projects' own MCP servers",
        S::Github => "the GitHub sign-in",
        S::Context => "what the agents read about this computer",
        S::Unknown => "the next step",
    }
}

/// `provisionCountWord(provisionCounts(rows))`.
fn provision_count_word(applied: Option<&PlaceViewApplied>) -> String {
    let rows = applied.map(|a| a.rows.as_slice()).unwrap_or_default();
    let count = |kind: PlaceViewAppliedRowsItemKind| {
        rows.iter().filter(|r| r.kind.as_ref().map(|k| k.to_string()).unwrap_or_else(|| "tool".into()) == kind.to_string()).count() as i64
    };
    let said: Vec<String> = [
        (PlaceViewAppliedRowsItemKind::Tool, "tool"),
        (PlaceViewAppliedRowsItemKind::File, "file"),
        (PlaceViewAppliedRowsItemKind::Server, "MCP server"),
    ]
    .into_iter()
    .filter_map(|(kind, word)| {
        let n = count(kind);
        (n > 0).then(|| plural(n, word))
    })
    .collect();
    if said.is_empty() {
        plural(0, "tool")
    } else {
        said.join(", ")
    }
}

/// `setupWord`: the step under way, where it stopped, or what stands; empty where nothing set the computer up.
pub fn setup_word(setup: Option<&PlaceSetup>, applied: Option<&PlaceViewApplied>) -> String {
    let Some(setup) = setup else { return String::new() };
    let under = setup.steps.iter().find(|l| matches!(l.state, PlaceSetupStepsItemState::Running));
    match setup.state {
        PlaceSetupState::Running | PlaceSetupState::Unknown => match under {
            Some(under) => format!("setting up {}", step_words(&under.step)),
            None => "setting up".into(),
        },
        PlaceSetupState::Failed => format!("stopped: {}", setup.said.as_deref().unwrap_or("no reason recorded")),
        PlaceSetupState::Done => {
            let rows = applied.map(|a| a.rows.as_slice()).unwrap_or_default();
            let failed: Vec<String> =
                rows.iter().filter(|r| matches!(r.outcome, PlaceViewAppliedRowsItemOutcome::Failed)).map(|r| r.label.clone()).collect();
            if failed.is_empty() {
                format!("{} ready", provision_count_word(applied))
            } else {
                format!("{} of {} failed: {}", failed.len(), rows.len(), name_list(&failed))
            }
        }
    }
}

/// `syncLine`.
pub fn sync_line(sync: &PlaceViewSync) -> String {
    let verb = if matches!(sync.state, PlaceViewSyncState::Running) { "putting on" } else { "waiting to put on" };
    format!("{verb} the recipe's {}: {}", plural(sync.changes.len() as i64, "change"), name_list(&sync.changes))
}

/// `PlaceState`: a word and the sentence that says why.
#[derive(Clone, Debug, PartialEq)]
pub struct Said {
    pub word: String,
    pub sentence: Option<String>,
}

fn said(word: &str, sentence: Option<String>) -> Said {
    Said { word: word.to_owned(), sentence }
}

/// `pendingWord`.
pub fn pending_word(p: &PendingComputer) -> Said {
    if let Some(failed) = &p.failed {
        return said(setup_words::FAILED, Some(failed.said.clone()));
    }
    let step = match p.step {
        PendingComputerStep::Connect => "connecting over ssh",
        PendingComputerStep::Check => "checking it can run wsp",
        PendingComputerStep::Wsp => "installing wsp",
        PendingComputerStep::Floor => "joined; the base tools are going on while you choose",
        PendingComputerStep::Choosing => "joined; waiting on what goes on it",
        PendingComputerStep::Unknown => "setting up",
    };
    said(setup_words::PENDING, Some(step.to_owned()))
}

/// `placeWord`, for a row with no pending add: the setup's words in the order they win.
pub fn place_word(place: &PlaceView, absent: Option<&Absent>) -> Said {
    let setup = place.setup.as_ref();
    if let Some(setup) = setup.filter(|s| matches!(s.state, PlaceSetupState::Failed)) {
        return said(setup_words::FAILED, setup.said.clone());
    }
    if let Some(wait) = setup.and_then(|s| s.waiting.first()) {
        let line = if matches!(wait.state, PlaceSetupWaitingItemState::Expired) {
            format!("{}'s sign-in ran out; a retry asks for a fresh code", wait.label)
        } else {
            format!(
                "{} waits on you to sign in{}{}",
                wait.label,
                wait.url.as_ref().map(|u| format!(" at {u}")).unwrap_or_default(),
                wait.code.as_ref().map(|c| format!(" with code {c}")).unwrap_or_default()
            )
        };
        return said(setup_words::NEEDS_YOU, Some(line));
    }
    let important: Vec<String> = place
        .applied
        .as_ref()
        .map(|a| a.rows.as_slice())
        .unwrap_or_default()
        .iter()
        .filter(|r| {
            matches!(r.outcome, PlaceViewAppliedRowsItemOutcome::Failed)
                && matches!(r.step, Some(PlaceViewAppliedRowsItemStep::Folders | PlaceViewAppliedRowsItemStep::Github))
        })
        .map(|r| format!("{}: {}", r.label, r.note.as_deref().unwrap_or("failed")))
        .collect();
    if !important.is_empty() {
        return said(setup_words::NEEDS_YOU, Some(important.join("; ")));
    }
    if let Some(absent) = absent {
        return Said { word: absent.away.into(), sentence: Some(absent.sentence.clone()) };
    }
    if let Some(setup) = setup.filter(|s| matches!(s.state, PlaceSetupState::Running)) {
        return said(setup_words::SETTING_UP, Some(setup_word(Some(setup), None)));
    }
    if let Some(sync) = &place.sync {
        return said(setup_words::BEHIND, Some(sync_line(sync)));
    }
    match daemon_behind(place) {
        Some(behind) => said(&behind, None),
        None => said(setup_words::READY, None),
    }
}

/// `placeDaemonBehind`, as the host words it on the row.
pub fn daemon_behind(place: &PlaceView) -> Option<String> {
    place.behind.as_ref().map(|b| b.word.clone())
}

/// `PlaceStateCell`: one word with its sentence for the hover, or the one act the row offers.
#[derive(Clone, Debug, PartialEq)]
pub enum Cell {
    Word { word: String, why: Option<String>, mark: Option<MarkState> },
    Update { why: String },
    SignIn { why: String },
}

impl Cell {
    pub fn why(&self) -> Option<&str> {
        match self {
            Cell::Word { why, .. } => why.as_deref(),
            Cell::Update { why } | Cell::SignIn { why } => Some(why),
        }
    }

    /// `stateWords`: the word, or the act's sentence.
    pub fn words(&self) -> String {
        match self {
            Cell::Word { word, .. } => word.clone(),
            Cell::Update { why } | Cell::SignIn { why } => why.clone(),
        }
    }
}

/// `PLACE_STATE_WORDS`.
pub mod state_words {
    pub const READY: &str = "Ready";
    pub const BLOCKED: &str = "Blocked";
    pub const BEHIND: &str = "Behind";
    pub const SIGN_IN: &str = "Sign in";
}

/// Whether a catalog id is an agent's.
pub fn is_agent(id: &str) -> bool {
    AGENTS.iter().any(|a| a.id == id)
}

/// `catalogEntry(id)?.name ?? id`: a catalog row by the name it is listed under, where that is not its id; held to the
/// catalog by `the_copies_are_what_the_catalog_and_the_protocol_say_now`.
pub fn catalog_name(id: &str) -> String {
    let name = match id {
        "node" => "Node 22 with npm",
        "python" => "Python 3.12",
        "build-essential" => "C toolchain with cmake and ninja",
        "zip" => "zip and unzip",
        "gh" => "GitHub CLI",
        "docker" => "Docker engine and compose",
        "go" => "Go",
        "rust" => "Rust with cargo",
        "java" => "Java 21",
        "maven" => "Maven",
        "gradle" => "Gradle",
        "bun" => "Bun",
        "yarn" => "Yarn",
        "prettier" => "Prettier",
        "eslint" => "ESLint",
        "typescript" => "TypeScript",
        "wrangler" => "Cloudflare Wrangler",
        "gcloud" => "Google Cloud CLI",
        "aws" => "AWS CLI",
        "vercel" => "Vercel CLI",
        "netlify" => "Netlify CLI",
        "fly" => "flyctl",
        "supabase" => "Supabase CLI",
        "railway" => "Railway CLI",
        "doppler" => "Doppler CLI",
        "op" => "1Password CLI",
        "git-lfs" => "Git LFS",
        "ruby" => "Ruby 3.1 with bundler",
        "php" => "PHP 8.2 with Composer",
        "postgresql-client" => "PostgreSQL client",
        "redis-tools" => "Redis tools",
        "shellcheck" => "ShellCheck",
        "swift" => "Swift 6.3",
        "elixir" => "Elixir 1.14 with Erlang",
        "bazel" => "Bazel via bazelisk",
        "llvm" => "clang, clang-format, clang-tidy",
        "playwright" => "Chromium for Playwright",
        other => return catalog::name(other),
    };
    name.to_owned()
}

/// Whether a place is the computer the host runs on, as a project's landing reads it.
pub fn here_of(place: &PlaceView) -> bool {
    is_here(place)
}

/// `placeStateCell`: blocked first, then the setup's words that need the person, not answering, the setup running,
/// the recipe on its way, the daemon behind, an agent there that needs a sign-in, then Ready.
pub fn state_cell(place: &PlaceView, absent: Option<&Absent>, can_update: bool) -> Cell {
    if let Some(blocked) = &place.blocked {
        return Cell::Word { word: state_words::BLOCKED.into(), why: Some(blocked.clone()), mark: Some(MarkState::Failed) };
    }
    let setup = place_word(place, absent);
    let said = |mark: MarkState| Cell::Word { word: setup.word.clone(), why: setup.sentence.clone(), mark: Some(mark) };
    if setup.word == setup_words::FAILED {
        return said(MarkState::Failed);
    }
    if setup.word == setup_words::NEEDS_YOU {
        return said(MarkState::NeedsYou);
    }
    if let Some(absent) = absent {
        return Cell::Word { word: capitalised(absent.away), why: Some(absent.sentence.clone()), mark: Some(MarkState::Offline) };
    }
    if setup.word == setup_words::SETTING_UP {
        return said(MarkState::Working);
    }
    if let Some(sync) = &place.sync {
        let mark = matches!(sync.state, PlaceViewSyncState::Running).then_some(MarkState::Working);
        return Cell::Word { word: state_words::BEHIND.into(), why: Some(sync_line(sync)), mark };
    }
    if let Some(behind) = daemon_behind(place) {
        return if can_update {
            Cell::Update { why: behind }
        } else {
            Cell::Word { word: state_words::BEHIND.into(), why: Some(behind), mark: None }
        };
    }
    let mut unsigned: Vec<&String> =
        place.sign_ins.iter().filter(|(_, s)| matches!(s, PlaceViewSignInsValue::None)).map(|(a, _)| a).collect();
    unsigned.sort();
    if !unsigned.is_empty() {
        let names: Vec<String> = unsigned.into_iter().map(|a| catalog::name(a)).collect();
        return Cell::SignIn { why: format!("needs a sign-in: {}", names.join(", ")) };
    }
    Cell::Word { word: state_words::READY.into(), why: None, mark: Some(MarkState::Ready) }
}

/// `WHERE_WORDS.runs`: which wsp and daemon a computer runs, each part the host carries for it.
pub fn version_fact(place: &PlaceView, host_version: Option<&str>) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    if let Some(wsp) = host_version.filter(|_| is_here(place)) {
        parts.push(format!("wsp {wsp}"));
    }
    if let Some(daemon) = place.daemon_version {
        parts.push(format!("daemon {daemon}"));
    }
    (!parts.is_empty()).then_some(parts)
}

/// `shapeLine`: its system, cores and memory, as it last reported them.
pub fn shape_line(place: &PlaceView) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(os) = place.os.as_ref().filter(|os| !os.is_empty()) {
        parts.push(os.clone());
    }
    if let Some(shape) = &place.shape {
        parts.push(fmt_cores(shape.cpu));
        parts.push(fmt_mem_gb(shape.mem_mb));
    }
    parts.join(", ")
}

/// `WHERE_WORDS`.
pub mod where_words {
    pub fn not_read(said: &str) -> String {
        format!("Computers not read: {said}")
    }
    pub const UPDATE: &str = "Update";
    pub const DEFAULT: &str = "default";
    pub const REMOVE: &str = "Remove";
    pub fn remove_title(computer: &str) -> String {
        format!("Remove {computer}")
    }
    pub fn remove_description(computer: &str, here: &str) -> String {
        if here.is_empty() {
            String::new()
        } else {
            format!("wsp comes off {computer} and its threads' records leave {here}. Your files there stay.")
        }
    }
    pub const REMOVE_CLOUD_DESCRIPTION: &str = "Deletes every machine and image snapshot wsp made there, then forgets the key.";
    pub const REMOVING: &str = "Removing\u{2026}";
    pub fn remove_unmade(computer: &str) -> String {
        format!("{computer} was not removed: this host no longer holds it.")
    }
    pub const DONE: &str = "Done";
    pub fn key_refused_fix(cloud: &str) -> String {
        format!("Save a working key and remove {cloud} again, or forget it here.")
    }
    pub const FORGET: &str = "Forget";
    pub const FORGETTING: &str = "Forgetting\u{2026}";
    pub const REMOVE_ANYWAY: &str = "Remove anyway";
    pub const UNSAVED_FIX_CLOUD: &str = "Ask each sub-thread to push first; Remove anyway deletes that work with the machines.";
    pub const UNSAVED_FIX: &str = "Export a task's project or push its branch, and copy a project folder's work off the computer from the path named; Remove anyway takes it with the computer.";
    pub const READING_HOLDS: &str = "Reading what goes with it.";
    pub const CANCEL: &str = "Cancel";
    pub const DIAL_LINK: &str = "Try now";
    pub const DIAL_SSH: &str = "Try over ssh";
    pub const DIALLING: &str = "Dialling\u{2026}";
    pub const HEAD_COMPUTER: &str = "Computers";
    pub const HEAD_CORES: &str = "Cores";
    pub const HEAD_MEMORY: &str = "Memory";
    pub const HEAD_THREADS: &str = "Threads";
    pub const HEAD_CLOUD: &str = "Clouds";
    pub const HEAD_VERSION: &str = "Version";
    pub const HEAD_AGENTS: &str = "Agents";
    pub const HEAD_IMAGE: &str = "Image";
    pub const HEAD_PENDING: &str = "Pending";
    pub const HEAD_SETUP: &str = "Setup";
    pub fn image_sealed(version: u64, when: &str) -> String {
        format!("Image v{version}, sealed {when}")
    }
    pub const NO_IMAGE_YET: &str = "No image yet";
    pub fn running(n: usize) -> String {
        format!("{n} running")
    }
    pub const YOUR_IMAGE: &str = "Your image";
}

/// `ADD_COMPUTER_WORDS`, the parts these pages say.
pub mod add_words {
    pub const TITLE: &str = "Add a computer";
    pub const ADD_CLOUD: &str = "Add a cloud provider";
    pub const SUDO_FIX: &str = "Type it below; it goes to sudo there and is kept nowhere.";
    pub fn sign_ins_on(computer: &str) -> String {
        format!("Sign-ins on {computer}")
    }
}

/// `COMPUTER_PAGE_WORDS`.
pub mod page_words {
    pub fn behind_title(here: &str) -> String {
        format!("Runs an older wsp than {here}")
    }
    pub fn behind_update(name: &str, here: &str) -> String {
        format!("Threads still run there. Updating installs {here}'s version on {name} and restarts it; running threads carry on.")
    }
    pub const BEHIND_HERE_TITLE: &str = "Runs an older wsp than this app";
    pub fn behind_install(fix: &str) -> String {
        format!("Threads still run. Run {fix} in a terminal to bring it level.")
    }
    pub fn update(name: &str) -> String {
        format!("Update wsp on {name}")
    }
    pub const UPDATING: &str = "Updating";
    pub const LIMITS: &str = "Limits";
    pub const THREADS_AT_ONCE: &str = "Threads at once";
    pub fn threads_line(n: u64, name: &str, mem: &str) -> String {
        format!("New threads wait past this, except one a running thread waits on, which runs in that thread's place. The default is one thread for every 2 GB of memory, up to twice the cores, so {n} on {name}'s {mem}.")
    }
    pub const THREADS_LINE_BARE: &str =
        "New threads wait past this, except one a running thread waits on, which runs in that thread's place.";
    pub const NAP_TITLE: &str = "Nap a quiet workspace after";
    pub const NAP_LINE: &str = "A workspace with no running turn stops and wakes on the next message.";
    pub const NAP_NEVER: &str = "Never";
    pub const TURN_LIMIT_TITLE: &str = "Turn limit";
    pub const TURN_LIMIT_LINE: &str = "Stops a turn that runs this long. Send to the thread to continue where it stopped.";
    pub const TURN_LIMIT_OFF: &str = "Off";
    pub const MACHINES_TITLE: &str = "Machines at once";
    pub fn machines_line(name: &str, plan: Option<i64>) -> String {
        let plan = plan.map(|p| format!(" {name}'s plan allows {p}.")).unwrap_or_default();
        format!("How many machines may run on {name} at once. A sub-thread past it waits for one to free.{plan}")
    }
    pub const SPEND_TITLE: &str = "Spend per day";
    pub fn spend_line(name: &str) -> String {
        format!("What {name} may spend in a day before no new machine starts. Machines already running are left alone.")
    }
    pub const SIZE_TITLE: &str = "Machine size for a sub-thread";
    pub const SIZE_LINE: &str = "The size a sub-thread gets when the thread starting it names none.";
    pub fn size_choice(name: &str, cpu: f64, mem: &str, rate: Option<&str>) -> String {
        let name = super::capitalised(name);
        let rate = rate.map(|r| format!(", {r}")).unwrap_or_default();
        format!("{name}, {} vCPU, {mem}{rate}", super::js_number(cpu))
    }
    pub const DELETE_TITLE: &str = "Delete a finished sub-thread's machine after";
    pub const DELETE_LINE: &str = "Its transcript and the agent's session are kept, and a message after the delete starts a new machine.";
    pub const DELETE_AT_ONCE: &str = "At once";
    pub const THREADS_HERE: &str = "Threads here";
    pub const SPAWN_TITLE: &str = "Agents may start agents";
    pub fn spawn_line(depth: u64, machines: Option<u64>) -> String {
        let machines = machines.map(|m| format!("{m} {} and ", if m == 1 { "machine" } else { "machines" })).unwrap_or_default();
        let deep = if depth == 1 { "one level deep".to_owned() } else { format!("{depth} levels deep") };
        format!("A thread here may open threads of its own, up to {machines}{deep}.")
    }
    pub const LEVELS_TITLE: &str = "Levels deep";
    pub const LEVELS_LINE: &str = "1 lets the thread you start open threads, 2 lets those open their own.";
    pub const NAME_TITLE: &str = "Name";
    pub fn name_sheet(name: &str) -> String {
        format!("What {name} is called in every list. Its projects and threads stay on it.")
    }
    pub const SSH_TITLE: &str = "SSH login";
    pub const SSH_NONE: &str = "It joined with a code and has no login.";
    pub fn ssh_sheet(name: &str) -> String {
        format!("The login wsp uses to reach {name}, as user@host.")
    }
    pub fn ssh_checking(ssh: &str, name: &str) -> String {
        format!("Checking that {ssh} reaches {name}")
    }
    pub const CHANGE: &str = "Change";
    pub const CANCEL_WORD: &str = "Cancel";
    pub const SAVE: &str = "Save";
    pub const RECIPE_TITLE: &str = "Follows a recipe";
    pub fn recipe_line(name: &str) -> String {
        format!("Changes to it reach {name} on their own.")
    }
    pub fn agents_on(name: &str) -> String {
        format!("Agents, tool servers, skills and plugins on {name}")
    }
    pub const AGENTS_ON_LINE: &str = "Installed agents, their sign-ins, and the tools, skills and plugins they get.";
    pub const ACTIVITY_ROW: &str = "What runs here";
    pub fn activity_line(threads: usize) -> String {
        match threads {
            0 => "No thread running, wsp's own parts".into(),
            1 => "1 thread running, and wsp's own parts".into(),
            n => format!("{n} threads running, and wsp's own parts"),
        }
    }
    pub const SETUP_TITLE: &str = "Setup";
    pub const SETUP_DONE: &str = "Done";
    pub const SHOW_LOG: &str = "Show setup log";
    pub const HIDE_LOG: &str = "Hide setup log";
    pub fn setup_failed(names: &str) -> String {
        format!("{names} failed")
    }
}

/// The choices a span select offers: minutes or hours as milliseconds, `None` the word for none.
pub fn nap_choices() -> Vec<Option<i64>> {
    let mut out: Vec<Option<i64>> =
        [10, 20, 30, 60, 120, 180].iter().map(|m| m * 60_000).filter(|ms| *ms <= NAP_AFTER_MAX_MS).map(Some).collect();
    out.push(None);
    out
}

pub fn turn_limit_choices() -> Vec<Option<i64>> {
    let mut out: Vec<Option<i64>> =
        [1, 2, 4, 6, 8, 12, 24].iter().map(|h| h * 3_600_000).filter(|ms| *ms <= TURN_LIMIT_MAX_MS).map(Some).collect();
    out.push(None);
    out
}

pub fn delete_choices() -> Vec<Option<i64>> {
    std::iter::once(None).chain([10, 30, 60, 120, 180].iter().map(|m| Some(m * 60_000))).collect()
}

pub const NAP_AFTER_MAX_MS: i64 = 3 * 60 * 60_000;
pub const TURN_LIMIT_MAX_MS: i64 = 24 * 60 * 60_000;

/// `spanWord`: 20 minutes, 1 hour, or the word for none.
pub fn span_word(ms: Option<i64>, none: &str) -> String {
    let Some(ms) = ms else { return none.to_owned() };
    let minutes = (ms as f64 / 60_000.).round() as i64;
    if minutes % 60 != 0 {
        return format!("{minutes} {}", if minutes == 1 { "minute" } else { "minutes" });
    }
    let hours = minutes / 60;
    format!("{hours} {}", if hours == 1 { "hour" } else { "hours" })
}

/// `withValue`: a select's own choices, with a value set some other way standing among them in order.
pub fn with_value(choices: &[Option<i64>], now: Option<i64>) -> Vec<Option<i64>> {
    if choices.contains(&now) {
        return choices.to_vec();
    }
    let mut out: Vec<Option<i64>> = choices.iter().copied().filter(Option::is_some).collect();
    out.push(now);
    out.push(None);
    out.sort_by_key(|ms| ms.unwrap_or(i64::MAX));
    out
}

/// `cloudPlanLine`.
pub fn cloud_plan_line(place: &PlaceView, size_names: &[String]) -> Option<String> {
    let plan = place.plan.as_ref()?;
    let starts = place.starts.as_ref();
    let machines = plural(plan.machines_at_once, "machine");
    if plan.trial {
        let day = starts.map(|s| format!(", {} starts a day", s.day.limit)).unwrap_or_default();
        return Some(format!("Trial: {machines} at once{day}, {} sizes", size_names.join(" and ")));
    }
    let usd = plan.key.as_deref().and_then(|k| k.strip_prefix("box_")).filter(|d| !d.is_empty() && d.chars().all(|c| c.is_ascii_digit()));
    let head = usd.map(|d| format!("${d} plan")).unwrap_or_else(|| "Plan".into());
    let windows = starts
        .map(|s| format!(", {} starts a minute, {} an hour, {} a day", s.minute.limit, s.hour.limit, s.day.limit))
        .unwrap_or_default();
    Some(format!("{head}: {machines} at once{windows}"))
}

/// `cloudStartsTodayLine`.
pub fn starts_today_line(place: &PlaceView) -> Option<String> {
    place.starts.as_ref().map(|s| format!("Starts today: {} of {}", s.day.used, s.day.limit))
}

/// `subSizesSold`: the sizes the plan names, or every size before a limits read has answered.
pub fn sizes_sold(place: &PlaceView) -> Vec<&wsp_host_client::protocol::PlaceViewSizesItem> {
    place.sizes.iter().filter(|s| place.plan.as_ref().is_none_or(|plan| plan.sizes.contains(&size_word(s.cpu, s.mem_mb)))).collect()
}

/// `sizeNameOf`.
pub fn size_name(name: Option<&str>, cpu: f64, mem_mb: f64) -> String {
    name.map(str::to_owned).unwrap_or_else(|| size_word(cpu, mem_mb))
}

/// A cloud's cap: machines at once and spend per day, where the row carries one.
pub fn cloud_cap(place: &PlaceView) -> Option<(u64, f64)> {
    match place.cap.as_ref()? {
        wsp_host_client::protocol::PlaceViewCap::Variant1 { machines, spend_per_day_usd } => Some((machines.get(), *spend_per_day_usd)),
        wsp_host_client::protocol::PlaceViewCap::Variant0 { .. } | wsp_host_client::protocol::PlaceViewCap::Unknown(_) => None,
    }
}

/// A computer's threads at once, where the row carries one.
pub fn threads_cap(place: &PlaceView) -> Option<u64> {
    match place.cap.as_ref()? {
        wsp_host_client::protocol::PlaceViewCap::Variant0 { threads } => Some(threads.get()),
        wsp_host_client::protocol::PlaceViewCap::Variant1 { .. } | wsp_host_client::protocol::PlaceViewCap::Unknown(_) => None,
    }
}

pub fn threads_default(place: &PlaceView) -> Option<u64> {
    match place.cap_default.as_ref()? {
        wsp_host_client::protocol::PlaceViewCapDefault::Variant0 { threads } => Some(threads.get()),
        wsp_host_client::protocol::PlaceViewCapDefault::Variant1 { .. } | wsp_host_client::protocol::PlaceViewCapDefault::Unknown(_) => {
            None
        }
    }
}

/// `LIMIT_BAND_WORDS`.
pub mod limit_words {
    pub const MACHINES: &str = "Machines at once";
    pub const MINUTE: &str = "Starts this minute";
    pub const HOUR: &str = "Starts this hour";
    pub const DAY: &str = "Starts today";
    pub fn of(used: i64, limit: i64) -> String {
        format!("{used} of {limit}")
    }
    pub const COUNTED: &str = "Starts count over rolling windows; a create, a fork and a wake each count as one.";
    pub fn credit(hours: f64) -> String {
        if hours < 10. {
            format!("{hours:.1} h of credit left")
        } else {
            format!("{} h of credit left", hours.round())
        }
    }
    pub fn spend(today: f64, cap: f64) -> String {
        let cap = if cap.fract() == 0. { format!("${}", cap as i64) } else { super::fmt_cost(cap) };
        format!("{} of {cap} today", super::fmt_cost(today))
    }
}

/// `limitNote`: the credit left and the day's spend against the spend per day, where the row carries them.
pub fn limit_note(place: &PlaceView) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(hours) = place.credit_hours {
        parts.push(limit_words::credit(hours));
    }
    if let (Some((_, cap)), Some(today)) = (cloud_cap(place), place.spent_today_usd) {
        parts.push(limit_words::spend(today, cap));
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The columns of the limit band: a label, the used and the limit.
pub fn limit_columns(place: &PlaceView) -> Vec<(&'static str, &'static str, i64, i64)> {
    let mut out = Vec::new();
    if let Some(plan) = &place.plan {
        if let Some(running) = plan.running {
            out.push(("machines", limit_words::MACHINES, running, plan.machines_at_once));
        }
    }
    if let Some(s) = &place.starts {
        out.push(("minute", limit_words::MINUTE, s.minute.used, s.minute.limit));
        out.push(("hour", limit_words::HOUR, s.hour.used, s.hour.limit));
        out.push(("day", limit_words::DAY, s.day.used, s.day.limit));
    }
    out
}

/// `PLACE_LEAVE_LINE`.
pub const LEAVE_LINE: &str = "wsp leave";
/// `PLACES_WORDS.remove.leaveTakes`.
pub const LEAVE_TAKES: &str = "It takes off the daemon's service, wsp's own files under that login's home, the command beside them that opens sign-in pages in your browser, and, on a computer added over ssh, the copy of your image. Your work folder stays.";

/// `imageCopyLine`.
fn image_copy_line(size: Option<String>) -> String {
    format!("the copy of your image{}", size.map(|s| format!(" ({s})")).unwrap_or_default())
}

/// `threadWord`.
fn thread_word(n: i64) -> String {
    format!("{n} {}", if n == 1 { "thread" } else { "threads" })
}

/// One workspace a computer holds as the dialog names it: its state word and the threads it holds.
#[derive(Clone, Debug, Default)]
pub struct Held {
    pub state: String,
    pub threads: i64,
}

/// `workspaceWord(workspaceStateOf(workspace, null))`: the state word off the phase alone.
fn phase_word(phase: &str) -> &'static str {
    match phase {
        "released" => "Deleted",
        "gone" => "Gone",
        "pausing" => "Stopping",
        "napping" => "Stopped",
        "waking" => "Waking",
        "queued" => "Waiting",
        _ => "Running",
    }
}

/// `holdingsFor` for one row: each workspace `placeOf` stands there, by its state and the threads it holds.
pub fn held_on(place: &PlaceView, places: &[PlaceView], workspaces: &[WorkspaceOut], sessions: &[SessionView]) -> Vec<Held> {
    let listed = values(places);
    workspaces
        .iter()
        .filter(|w| serde_json::to_value(w).ok().and_then(|w| menu::place_of(&w, &listed)).as_deref() == Some(place.id.as_str()))
        .map(|w| Held {
            state: phase_word(&w.phase.to_string()).to_owned(),
            threads: sessions.iter().filter(|s| s.workspace_id == w.id).count() as i64,
        })
        .collect()
}

/// `PlaceHolding`: what stands on a computer right now, as the Remove dialog is given it.
#[derive(Clone, Debug, Default)]
pub struct Holding {
    pub workspaces: Vec<Held>,
    pub projects: Vec<String>,
    pub images: Option<Vec<(String, Option<f64>)>>,
}

fn listed(names: &[String]) -> String {
    match names.len() {
        0..=2 => names.join(" and "),
        n => format!("{} and {}", names[..n - 1].join(", "), names[n - 1]),
    }
}

fn named_images(images: &[(String, Option<f64>)]) -> String {
    listed(
        &images
            .iter()
            .map(|(id, size)| size.map(|s| format!("{id} ({})", fmt_bytes(s as u64))).unwrap_or_else(|| id.clone()))
            .collect::<Vec<_>>(),
    )
}

/// `providerImagesLine`.
fn provider_images_line(name: &str, images: &[(String, Option<f64>)]) -> String {
    let named = named_images(images);
    if images.len() == 1 {
        format!("Its image snapshot {named} is deleted at {name} before the key goes.")
    } else {
        format!("Its {} image snapshots, {named}, are deleted at {name} before the key goes.", images.len())
    }
}

/// `providerImagesLeftLine`.
fn provider_images_left_line(name: &str, images: &[(String, Option<f64>)]) -> String {
    let one = images.len() == 1;
    let head = if one {
        format!("Its image snapshot {} stays", named_images(images))
    } else {
        format!("Its {} image snapshots, {}, stay", images.len(), named_images(images))
    };
    format!(
        "{head} at {name} and {} every month; add the key again and remove {name} to delete {}.",
        if one { "bills" } else { "bill" },
        if one { "it" } else { "them" }
    )
}

/// `removeSentence`: what a remove takes, computed from what the computer holds.
pub fn remove_sentence(place: &PlaceView, holding: &Holding, here: &str, image_bytes: Option<f64>) -> String {
    if here.is_empty() {
        return String::new();
    }
    let count = holding.workspaces.len() as i64;
    let threads: i64 = holding.workspaces.iter().map(|w| w.threads).sum();
    let held = if count == 1 { "its task".to_owned() } else { format!("its {count} tasks") };
    let name = place_name(place);
    let offline = !is_provider(place) && is_offline(place);
    let ssh = place.road.as_ref().and_then(|r| r.ssh.clone());
    let mut lines: Vec<String> = Vec::new();
    if is_provider(place) && sub_threads_only(&place.id) {
        let machines = holding.workspaces.iter().filter(|w| w.state != "Deleted" && w.state != "Waiting").count();
        lines.push(if machines == 0 {
            format!("The key for {name} is forgotten on {here}.")
        } else {
            format!(
                "Its {} deleted at {name} and the key is forgotten on {here}.",
                if machines == 1 { "machine is".to_owned() } else { format!("{machines} machines are") }
            )
        });
        if let Some(images) = &holding.images {
            lines.push(provider_images_line(&name, images));
        }
        if threads > 0 {
            lines.push(format!(
                "{} with {}, and a message to one asks for {name} again.",
                if threads == 1 { "Its sub-thread stays".to_owned() } else { format!("The {threads} sub-threads stay") },
                if threads == 1 { "its transcript" } else { "their transcripts" }
            ));
        }
    } else if is_provider(place) {
        lines.push(if count == 0 {
            format!("The key for {name} is forgotten on {here}.")
        } else {
            format!(
                "Its {} deleted at {name} and the key is forgotten on {here}.",
                if count == 1 { "task is".to_owned() } else { format!("{count} tasks are") }
            )
        });
        if let Some(images) = &holding.images {
            lines.push(provider_images_line(&name, images));
        }
        if threads > 0 {
            lines.push(format!(
                "{} with {}.",
                if threads == 1 { "Its thread stays".to_owned() } else { format!("The {threads} threads stay") },
                if threads == 1 { "its transcript" } else { "their transcripts" }
            ));
        }
    } else {
        let copy = image_copy_line(image_bytes.map(|b| fmt_bytes(b as u64)));
        if offline && ssh.is_none() {
            lines.push(format!("{name} is offline, so nothing comes off it: run {LEAVE_LINE} on that computer once it is back."));
        } else if place.takes_runtime == Some(true) {
            lines.push(if count == 0 {
                format!("wsp and {copy} come off {name}, which is otherwise left as it is.")
            } else {
                format!("wsp, {held} and {copy} come off {name}, which is otherwise left as it is.")
            });
        } else {
            let comes = if count == 0 { "wsp comes".to_owned() } else { format!("wsp and {held} come") };
            lines.push(format!("{comes} off {name}, which keeps {copy} and is otherwise left as it is."));
        }
        if count > 0 {
            lines.push(format!(
                "{} and {} leave {here}.",
                if count == 1 { "The task's record" } else { "The tasks' records" },
                thread_word(threads)
            ));
        }
        let projects = &holding.projects;
        if !projects.is_empty() {
            let n = projects.len();
            lines.push(format!(
                "{} {} {} {here}.",
                if n == 1 { "Its project".to_owned() } else { format!("Its {n} projects") },
                projects.join(", "),
                if n == 1 { "leaves" } else { "leave" }
            ));
        }
    }
    if let Some(ssh) = ssh.filter(|_| offline) {
        lines.push(format!(
            "It is offline, so wsp logs in as {ssh} to take itself off; if that fails too, run {LEAVE_LINE} on that computer."
        ));
    }
    lines.join(" ")
}

/// `removeTitle`, `forgetTitle`, `removedTitle`.
pub fn remove_dialog_title(place: &PlaceView) -> String {
    format!("Remove {}?", place_name(place))
}

pub fn forget_title(place: &PlaceView) -> String {
    format!("Forget {}?", place_name(place))
}

pub fn removed_title(place: &PlaceView, forgot: bool) -> String {
    format!("{} {}", place_name(place), if forgot { "forgotten" } else { "removed" })
}

/// `providerForgetSentence`.
pub fn provider_forget_sentence(place: &PlaceView, holding: &Holding, here: &str) -> String {
    let name = place_name(place);
    let count = holding.workspaces.len();
    let leaves = if count == 0 {
        "The key leaves".to_owned()
    } else {
        format!("The key and {} leave", if count == 1 { "its task's record".to_owned() } else { format!("its {count} tasks' records") })
    };
    let images = holding.images.as_ref().map(|i| format!(" {}", provider_images_left_line(&name, i))).unwrap_or_default();
    format!("{leaves} {here}. Nothing at {name} is touched, so what wsp made there may bill until {name}'s own timer ends it.{images}")
}

fn held_part(held: &[(String, i64)], noun: &str) -> Option<String> {
    if held.is_empty() {
        return None;
    }
    let threads: i64 = held.iter().map(|(_, t)| t).sum();
    let names: Vec<String> = held.iter().map(|(n, _)| n.clone()).collect();
    let named = if held.len() == 1 { format!("{noun} {}", names[0]) } else { format!("{} {noun}s {}", held.len(), names.join(", ")) };
    Some(if threads == 0 { named } else { format!("{named} with {}", plural(threads, "thread")) })
}

/// `forgetSentence`: why a remove cannot, then the forget's own sentence.
pub fn forget_sentence(place: &PlaceView, holds: &PlaceHolds) -> String {
    let name = place_name(place);
    let away = format!("{}, and its forks and projects go over its link", crate::screens::terminal::words::absent_computer(&name).said);
    let forks: Vec<(String, i64)> = holds.forks.iter().map(|f| (f.name.clone(), f.threads)).collect();
    let projects: Vec<(String, i64)> = holds.projects.iter().map(|p| (p.name.clone(), p.threads)).collect();
    let goes: Vec<String> =
        [held_part(&forks, "fork"), held_part(&projects, "project")].into_iter().flatten().map(|p| format!("its {p}")).collect();
    let leaves = format!("{name} leaves this wsp{}", if goes.is_empty() { String::new() } else { format!(" with {}", goes.join(" and ")) });
    let tried = match place.road.as_ref().and_then(|r| r.ssh.as_deref()) {
        None => format!("{leaves}, and nothing is done on {name}"),
        Some(ssh) => format!("wsp tries to take itself off {name} over {ssh} first. Then {leaves}, and nothing more is done on {name}"),
    };
    format!("{away}. {tried}: whatever of wsp's stays there comes off with {LEAVE_LINE} run on that computer.")
}

/// `placeForgetsOnly`.
pub fn forgets_only(holds: &PlaceHolds) -> bool {
    holds.away == Some(true) && (!holds.forks.is_empty() || !holds.projects.is_empty())
}

/// `placeUnsavedRefusal(...).said`.
pub fn unsaved_said(place: &str, unsaved: &[String]) -> String {
    format!("{place} holds work no remote has, which a remove would lose: {}", unsaved.join("; "))
}

/// `subHeldOnRemoveLine`.
pub fn sub_held_line(held: &[String]) -> Option<String> {
    if held.is_empty() {
        return None;
    }
    let rest = if held.len() > 5 { format!("; and {} more", held.len() - 5) } else { String::new() };
    let first: Vec<&str> = held.iter().take(5).map(String::as_str).collect();
    Some(format!(
        "Work not pushed goes with {}: {}{rest}.",
        if held.len() == 1 { "its machine" } else { "their machines" },
        first.join("; ")
    ))
}

/// The keys a setup read: which clouds this host holds a key for (`keyHeld`), and which it could be given one for.
pub fn key_held(id: &str, keys: Option<&HashMap<String, bool>>) -> bool {
    keys.and_then(|k| k.get(id)).copied() == Some(true)
}

/// `cloudsOffered`: every cloud the host registered, held or not, and none before the host has said.
pub fn clouds_offered(keys: Option<&HashMap<String, bool>>) -> Vec<&'static str> {
    PROVIDERS.iter().filter(|p| keys.is_some_and(|k| k.contains_key(p.id))).map(|p| p.id).collect()
}

/// `PlaceDialRoad`: a frame on the link the computer holds, or the ssh login it was installed over.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DialRoad {
    Link,
    Ssh,
}

/// `placeDialRoad`.
pub fn dial_road(place: &PlaceView) -> Option<DialRoad> {
    if place.present == Some(true) {
        return Some(DialRoad::Link);
    }
    place.road.as_ref().and_then(|r| r.ssh.as_deref()).filter(|s| !s.is_empty()).map(|_| DialRoad::Ssh)
}

/// `absentRoad(...).refused`: what the last try said, where it did not answer.
pub fn dial_refused(place: &PlaceView) -> Option<String> {
    place.dialled.as_ref().filter(|d| !d.answered).and_then(|d| d.said.clone())
}

/// The clock now, as the web's `Date.now()`.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The tool names and the clouds' key words are copies of the catalog's and the protocol's tables.
    #[test]
    fn the_copies_are_what_the_catalog_and_the_protocol_say_now() {
        let read = crate::screens::settings::words::tests::catalog_read();
        for pair in read["tools"].as_array().unwrap() {
            let (id, name) = (pair[0].as_str().unwrap(), pair[1].as_str().unwrap());
            assert_eq!(catalog_name(id), name, "{id}");
        }
        let theirs = read["providerKeyWords"].as_object().unwrap();
        assert_eq!(PROVIDERS.iter().map(|p| p.id).collect::<Vec<_>>(), theirs.keys().map(String::as_str).collect::<Vec<_>>());
        for p in &PROVIDERS {
            let words = &theirs[p.id];
            assert_eq!(words["name"].as_str(), Some(p.name), "{}", p.id);
            assert_eq!(words["keyConsole"].as_str(), p.key_console, "{}", p.id);
            assert_eq!(words["subThreads"].as_bool().unwrap_or(false), p.sub_threads, "{}", p.id);
        }
    }

    fn place(extra: serde_json::Value) -> PlaceView {
        let mut base = json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false });
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        serde_json::from_value(base).unwrap()
    }

    #[test]
    fn a_place_reads_by_the_name_a_person_gave_it() {
        assert_eq!(place_name(&place(json!({ "label": "Studio" }))), "Studio");
        assert_eq!(place_name(&place(json!({ "kind": "provider", "name": "box", "id": "box" }))), "Boat");
        assert_eq!(place_name(&place(json!({}))), "box1");
    }

    #[test]
    fn units_read_as_the_protocol_writes_them() {
        assert_eq!(fmt_mem_gb(16384.), "16 GB");
        assert_eq!(fmt_mem_gb(15872.), "15.5 GB");
        assert_eq!(fmt_rate(0.05), "$0.05/hr");
        assert_eq!(fmt_rate(0.125), "$0.125/hr");
        assert_eq!(fmt_cost(1234.5), "$1,234.50");
        assert_eq!(size_word(2., 4096.), "2x4");
        assert_eq!(size_from_word("2x0.5"), Some((2, 512)));
        assert_eq!(span_word(Some(20 * 60_000), "Never"), "20 minutes");
        assert_eq!(span_word(Some(3_600_000), "Never"), "1 hour");
        assert_eq!(with_value(&nap_choices(), Some(45 * 60_000)).len(), nap_choices().len() + 1);
    }

    #[test]
    fn the_state_cell_reads_in_its_order() {
        let ready = place(json!({}));
        assert_eq!(state_cell(&ready, None, true), Cell::Word { word: "Ready".into(), why: None, mark: Some(MarkState::Ready) });
        let away = place(json!({ "present": false }));
        let cell = state_cell(&away, absent_of(&away).as_ref(), true);
        assert_eq!(
            cell,
            Cell::Word {
                word: "No answer".into(),
                why: Some("box1 is not answering; it connects on its own when it is on".into()),
                mark: Some(MarkState::Offline)
            }
        );
        let behind = place(json!({ "behind": { "word": "daemon 160, host 166", "fix": "wsp add box1 --update", "act": "update" } }));
        assert_eq!(state_cell(&behind, None, true), Cell::Update { why: "daemon 160, host 166".into() });
        let unsigned = place(json!({ "signIns": { "codex": "none" } }));
        assert_eq!(state_cell(&unsigned, None, true), Cell::SignIn { why: "needs a sign-in: Codex".into() });
        let blocked = place(json!({ "blocked": "a computer you joined can't run threads" }));
        assert!(matches!(state_cell(&blocked, None, true), Cell::Word { mark: Some(MarkState::Failed), .. }));
    }

    #[test]
    fn the_remove_sentence_says_what_leaves() {
        let p = place(json!({}));
        let none = Holding::default();
        assert_eq!(
            remove_sentence(&p, &none, "mac", None),
            "wsp comes off box1, which keeps the copy of your image and is otherwise left as it is."
        );
        let held = Holding { workspaces: vec![Held { state: "running".into(), threads: 2 }], projects: vec!["api".into()], images: None };
        assert_eq!(
            remove_sentence(&p, &held, "mac", Some(2. * 1024. * 1024. * 1024.)),
            "wsp and its task come off box1, which keeps the copy of your image (2 GB) and is otherwise left as it is. The task's record and 2 threads leave mac. Its project api leaves mac."
        );
        let cloud = place(json!({ "kind": "provider", "id": "solari", "name": "solari" }));
        assert_eq!(remove_sentence(&cloud, &none, "mac", None), "The key for Solari is forgotten on mac.");
        assert_eq!(remove_sentence(&p, &none, "", None), "");
    }

    #[test]
    fn a_clouds_sentence_counts_the_workspaces_standing_there() {
        let boat = place(json!({ "kind": "provider", "id": "box", "name": "box" }));
        let mac = place(json!({ "id": "here", "name": "mac", "default": true }));
        let places = vec![mac.clone(), boat.clone()];
        let workspace = |id: &str, extra: serde_json::Value| -> WorkspaceOut {
            let mut base = json!({ "id": id, "name": id, "machineId": id, "phase": "running", "golden": "g", "createdAt": "2026-10-01T00:00:00Z", "project": { "id": "p", "name": "p", "path": "/p", "computer": "here" } });
            for (k, v) in extra.as_object().unwrap() {
                base[k] = v.clone();
            }
            serde_json::from_value(base).unwrap()
        };
        let workspaces = vec![
            workspace("w1", json!({ "kind": "cloud", "provider": "box" })),
            workspace("w2", json!({ "kind": "cloud" })),
            workspace("w3", json!({ "kind": "cloud", "provider": "box", "phase": "released" })),
            workspace("w4", json!({ "kind": "local" })),
        ];
        let session = |id: &str, on: &str| -> SessionView {
            serde_json::from_value(json!({ "id": id, "workspaceId": on, "harness": "claude", "status": "completed" })).unwrap()
        };
        let sessions = vec![session("s1", "w1"), session("s2", "w2"), session("s3", "w3"), session("s4", "w4")];
        let holding = Holding { workspaces: held_on(&boat, &places, &workspaces, &sessions), ..Holding::default() };
        assert_eq!(
            remove_sentence(&boat, &holding, "mac", None),
            "Its 2 machines are deleted at Boat and the key is forgotten on mac. The 3 sub-threads stay with their transcripts, and a message to one asks for Boat again."
        );
        assert_eq!(held_on(&mac, &places, &workspaces, &sessions).len(), 1);
    }

    #[test]
    fn a_plan_reads_its_windows() {
        let cloud = place(json!({
            "kind": "provider", "id": "box", "name": "box",
            "plan": { "key": "box_20", "trial": false, "machinesAtOnce": 4, "sizes": ["2x4"] },
            "starts": { "minute": { "used": 0, "limit": 5 }, "hour": { "used": 1, "limit": 30 }, "day": { "used": 3, "limit": 100 } }
        }));
        assert_eq!(cloud_plan_line(&cloud, &[]).unwrap(), "$20 plan: 4 machines at once, 5 starts a minute, 30 an hour, 100 a day");
        assert_eq!(starts_today_line(&cloud).unwrap(), "Starts today: 3 of 100");
    }
}
