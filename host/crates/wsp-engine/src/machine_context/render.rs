// SPDX-License-Identifier: AGPL-3.0-only
//! machine-context.ts:304-547 at a4033dc34. Spec: spec/context_render.md.
use std::sync::Arc;

use wsp_catalog::catalog::agent_name;
use wsp_catalog::context::{ContextHookInput, ContextHooks, ContextOutcomeKind, GuestFile, GuestRoots, CONTEXT_MARKER, MODE, SKILL_NAME};
use wsp_catalog::road_modules::BREW_PREFIX;
use wsp_catalog::signin_rows::{SECRETS_FILE_FISH, SECRETS_FILE_SH};
use wsp_wire::types::run_block::RUN_BLOCK_WORDS;
use wsp_wire::views::golden_image::on_npm_bin;
use wsp_words::js::{js_trim, utf16};
use wsp_words::words::thread::{background_work_words, ANOTHER_AGENT_WORDS, TURN_END_WORDS};
use wsp_words::words::units::fmt_bytes;

use super::probe::{
    context_agents, context_path, facts_path, guest_roots, merge_facts, parse_probe, probe_command, skill_path, AliasesUnread, BuildFacts,
    ContextAgent, ContextProbe, ProbeShells, ALIAS_BOUND_S, BROWSER_SHIM_PATH,
};
use crate::exec_detached::put::INLINE_EXEC_MS;
use crate::fetch::Fetch;
use crate::golden_seam::{ImportResult, TOOLS_DISK_FLOOR, TOOLS_PATH};
use crate::machine::{ExecOptions, Machine};
use crate::preview::DAEMON_PORT;
use crate::vault::{import_into, tar_of, TarEntry, VaultOptions};

/// machine-context.ts:31.
pub const SKILL_DESCRIPTION: &str = r#"How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed."#;

/// machine-context.ts:321.
pub const SERVERS_OFF_LINE: &str = r#"- A server that needs the person's own sign-in or key does not run on a cloud machine, which holds no sign-in or key of theirs. When a tool from one is missing, say it is not available on cloud machines, and do not suggest signing it in here (no /mcp, no mcp login command)."#;

/// A workspace by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceName {
    pub name: String,
}

/// `Pick<GoldenVersion, "version" | "createdAt" | "setupSha" | "npmBin">`.
#[derive(Debug, Clone, PartialEq)]
pub struct GoldenPick {
    pub version: u32,
    pub created_at: String,
    pub setup_sha: String,
    pub npm_bin: Option<String>,
}

/// machine-context.ts:306.
#[derive(Debug, Clone)]
pub struct ContextInput {
    pub workspace: Option<WorkspaceName>,
    pub golden: Option<GoldenPick>,
    pub probe: ContextProbe,
    pub facts: BuildFacts,
    pub agent: Option<ContextAgent>,
    pub at_provider: bool,
}

const CD_FACT: &str = "- Every agent session starts in the thread's folder; terminal panes open in the home folder.";
const CD_HOWTO: &str = "- Work in a folder: cd <dir> && <cmd> on one line, or absolute paths.";
const ALIASES_NOT_READ_SHARED: &str = "- Aliases: not read here. This computer's home is shared with every workspace on it, so wsp opens no login shell on it.";

/// machine-context.ts:39 `turnFact`.
fn turn_fact(agent: Option<ContextAgent>) -> String {
    let road = agent.and_then(|a| a.background_road.as_deref());
    format!(
        "- {TURN_END_WORDS}; a command you run in the background holds the turn open until it finishes, up to the turn limit set for this computer if it has one, and its result reaches you there. {}. Nothing else wakes you, so a command whose output you want in the same breath runs in the foreground and you wait for it. Only a server you mean to keep serving is detached with setsid nohup, and that one holds nothing open.",
        background_work_words(road)
    )
}

/// machine-context.ts:325. The agent's cd fact where its module has one, else the shared one.
fn cd_fact(agent: Option<ContextAgent>) -> String {
    agent.and_then(|a| a.context).and_then(|c| c.cd()).map_or_else(|| CD_FACT.to_string(), |cd| cd.fact)
}

/// machine-context.ts:327.
fn cd_howto(agent: Option<ContextAgent>) -> String {
    agent.and_then(|a| a.context).and_then(|c| c.cd()).map_or_else(|| CD_HOWTO.to_string(), |cd| cd.howto)
}

/// machine-context.ts:330. The line that says why no alias list was read.
fn aliases_unread_line(late: bool) -> String {
    if late {
        format!("- Aliases: not read. The login shell had not finished starting after {ALIAS_BOUND_S} seconds, so wsp stopped it.")
    } else {
        ALIASES_NOT_READ_SHARED.to_string()
    }
}

fn missing_list<'a>(items: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    items.map(|(label, note)| format!("{label} ({note})")).collect::<Vec<_>>().join("; ")
}

fn containers_line(probe: &ContextProbe) -> Option<String> {
    let containers: Vec<&str> = ["docker", "podman"].into_iter().filter(|b| probe.has.contains(*b)).collect();
    let none = if containers.is_empty() { ", and Docker and Podman are not installed" } else { "" };
    if !probe.overlay {
        Some(format!("- Containers do not run here: the kernel has no overlayfs{none}. Install services natively."))
    } else if containers.is_empty() {
        Some("- Docker and Podman are not installed.".to_string())
    } else {
        None
    }
}

fn disk_line(probe: &ContextProbe, short: bool) -> String {
    let floor = fmt_bytes(TOOLS_DISK_FLOOR as f64);
    let tail = if short { format!("wsp keeps {floor} free.") } else { format!("wsp keeps {floor} free and skips tool installs that would go under it.") };
    match &probe.disk {
        Some(d) => format!(
            "- Disk: {} root disk, {} free when this file was written. {tail}",
            fmt_bytes(d.size_bytes),
            fmt_bytes(d.free_bytes)
        ),
        None => format!("- Disk: size unknown when this file was written. {tail}"),
    }
}

/// machine-context.ts:341. The document, sentence case and dry: what is true of this machine, what is true of this
/// workspace, and how the things that go wrong here are done right.
pub fn render_machine_context(input: &ContextInput) -> String {
    let probe = &input.probe;
    let facts = &input.facts;
    let fish = probe.has.contains("fish");
    let shim = probe.has.contains("wsp-open");
    let secrets_file = if fish { format!("{SECRETS_FILE_SH} and {SECRETS_FILE_FISH}") } else { SECRETS_FILE_SH.to_string() };
    let mut machine: Vec<String> = Vec::new();
    let kernel = probe.kernel.as_ref().map(|k| format!(" {k}")).unwrap_or_default();
    machine.push(format!("- Linux{kernel}, user root, home /root. macOS apps, casks and Mac App Store apps are not here."));
    if !probe.versions.is_empty() {
        let versions: Vec<String> = probe.versions.iter().map(|v| format!("{} {}", v.name, v.version)).collect();
        machine.push(format!("- On every wsp machine: {}.", versions.join(", ")));
    }
    if probe.has.contains("brew") {
        machine.push(format!("- Homebrew is at {BREW_PREFIX}."));
    }
    if probe.has.contains("golden-path") {
        let npm_bin = input.golden.as_ref().and_then(|g| g.npm_bin.as_deref());
        machine.push(format!("- Login shells get their PATH from /etc/profile.d/wsp-golden.sh: {}", on_npm_bin(npm_bin, TOOLS_PATH)));
    }
    machine.extend(containers_line(probe));
    machine.push(format!("- wsp-daemon listens on 0.0.0.0:{DAEMON_PORT} with its own token. Do not stop it and do not bind port {DAEMON_PORT}."));
    machine.push("- wsp on this machine's PATH drives the person's host as this thread; wsp --help agent lists its verbs.".to_string());
    machine.push(turn_fact(input.agent));
    machine.push(format!("- {ANOTHER_AGENT_WORDS}."));
    machine.push(format!("- {RUN_BLOCK_WORDS}."));
    machine.push(cd_fact(input.agent));
    machine.push("- A server listening on a port shows in the app as a server the person can open. A loopback-only bind (127.0.0.1) is unreachable through the preview edge; bind 0.0.0.0. Ports below 1024 are not forwarded.".to_string());
    if shim {
        machine.push(format!("- Sign-ins go through wsp: BROWSER is {} and xdg-open is the same shim. The page opens on the person's computer and the callback port is tunnelled back here.", BROWSER_SHIM_PATH));
    }
    if input.at_provider {
        machine.push(SERVERS_OFF_LINE.to_string());
    }
    machine.push(format!("- Secrets are exported by {secrets_file}. They are never printed, logged or committed, and that file is never read or copied. .env files, .netrc and private keys were never copied from the person's computer."));

    let mut workspace: Vec<String> = Vec::new();
    workspace.push(match &input.workspace {
        Some(w) => format!("- Workspace: {}.", w.name),
        None => "- This machine is a golden builder, not a workspace yet.".to_string(),
    });
    workspace.push(match &input.golden {
        Some(g) => format!(
            "- Golden: v{}, sealed {}, setup {}.",
            g.version,
            utf16::cut(&g.created_at, 10),
            utf16::cut(&g.setup_sha, 12)
        ),
        None if input.workspace.is_some() => "- Golden: version not recorded.".to_string(),
        None => "- Golden: not sealed yet.".to_string(),
    });
    workspace.push(disk_line(probe, false));
    let secrets = if probe.secrets.is_empty() { "none".to_string() } else { probe.secrets.join(", ") };
    workspace.push(format!("- Secrets set in {SECRETS_FILE_SH}: {secrets}. Use them by name ($NAME); a missing one is asked for through wsp, not typed here."));
    let tools = if facts.tools.is_empty() { "none".to_string() } else { missing_list(facts.tools.iter().map(|m| (m.label.as_str(), m.note.as_str()))) };
    workspace.push(format!("- Tools that did not install: {tools}."));
    let agents = if facts.agents.is_empty() { "none".to_string() } else { missing_list(facts.agents.iter().map(|m| (m.label.as_str(), m.note.as_str()))) };
    workspace.push(format!("- Agents that did not install: {agents}."));
    let files = if facts.files.is_empty() {
        "none".to_string()
    } else {
        facts.files.iter().map(|f| format!("{} ({})", f.path, f.note)).collect::<Vec<_>>().join("; ")
    };
    workspace.push(format!("- Files left on the person's computer: {files}."));
    workspace.push(match probe.aliases_unread {
        None => {
            let aliases = if probe.aliases.is_empty() {
                "none".to_string()
            } else {
                probe.aliases.iter().map(|a| format!("{} runs {}", a.name, a.word)).collect::<Vec<_>>().join("; ")
            };
            format!("- Aliases whose command is not here: {aliases}.")
        }
        Some(AliasesUnread::Late) => aliases_unread_line(true),
        Some(AliasesUnread::Shared) => aliases_unread_line(false),
    });

    let tmux = probe.has.contains("tmux");
    let mut howto: Vec<String> = Vec::new();
    let tmux_hint = if tmux { ", or tmux new -d -s <name> '<cmd>'" } else { "" };
    howto.push(format!("- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &{tmux_hint}. Read the log file, not the tool output. Bind 0.0.0.0 if the person should open it."));
    let shim_hint = if shim { " For a page that is not a local server, run wsp-open <url>; it appears in the app for them to open." } else { "" };
    howto.push(format!("- Hand the person a URL: print it as http://localhost:<port>; wsp forwards the port and the link opens on their computer.{shim_hint}"));
    if shim {
        howto.push("- Ask for a sign-in: run the tool's own login command (gh auth login, gcloud auth login). It opens through $BROWSER, the person signs in on their computer, and the callback comes back here. Do not paste tokens into the terminal and do not ask for a password.".to_string());
    }
    howto.push(cd_howto(input.agent));
    howto.push(format!("- Use a secret: $NAME is set from {SECRETS_FILE_SH}. Never echo, log or commit a value, and never cat that file."));

    let mut lines: Vec<String> = vec![
        CONTEXT_MARKER.to_string(),
        String::new(),
        "# This machine".to_string(),
        String::new(),
        "This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer: the files, tools and agents they ticked. It is not that computer.".to_string(),
        String::new(),
    ];
    lines.extend(machine);
    lines.extend(["".to_string(), "# This workspace".to_string(), String::new()]);
    lines.extend(workspace);
    lines.extend(["".to_string(), "# How to".to_string(), String::new()]);
    lines.extend(howto);
    lines.push(String::new());
    lines.join("\n")
}

/// machine-context.ts:409. The always-loaded file: the facts an agent must not get wrong, and one line pointing at the
/// skill for the rest.
pub fn render_short_context(input: &ContextInput) -> String {
    let probe = &input.probe;
    let tmux = probe.has.contains("tmux");
    let tmux_hint = if tmux { ", or tmux new -d -s <name> '<cmd>'" } else { "" };
    let mut lines: Vec<String> = vec![
        CONTEXT_MARKER.to_string(),
        String::new(),
        format!("This is a Linux machine in the cloud that wsp set up from the recipe of the person's computer; it is not that computer. The {SKILL_NAME} skill has the full picture: what did not install, the secret names, the aliases whose commands are missing, and how the common things are done here."),
        String::new(),
        format!("- Keep a server alive: setsid nohup <cmd> > /tmp/<name>.log 2>&1 < /dev/null &{tmux_hint}."),
        turn_fact(input.agent),
        format!("- {ANOTHER_AGENT_WORDS}."),
        format!("- {RUN_BLOCK_WORDS}."),
        cd_fact(input.agent),
        "- A server listening on a port shows in the app for the person to open; bind 0.0.0.0, not 127.0.0.1. A printed http://localhost:<port> link has its port forwarded to their computer. Ports below 1024 are not forwarded.".to_string(),
    ];
    lines.extend(containers_line(probe));
    lines.push(disk_line(probe, true));
    lines.push(format!("- Secrets are exported by {SECRETS_FILE_SH}. Use them by name ($NAME); never print, log or commit a value, and never read that file."));
    if probe.has.contains("wsp-open") {
        lines.push("- Sign-ins go through wsp: run the tool's own login command and the page opens on the person's computer. Do not paste tokens into the terminal and do not ask for a password.".to_string());
    }
    if input.at_provider {
        lines.push(SERVERS_OFF_LINE.to_string());
    }
    lines.push(String::new());
    lines.join("\n")
}

/// machine-context.ts:440. The skill every agent lists by name and description and loads on demand.
pub fn render_skill(doc: &str) -> String {
    format!("---\nname: {SKILL_NAME}\ndescription: {SKILL_DESCRIPTION}\n---\n\n{doc}")
}

/// machine-context.ts:449. `None` roots are the guest's.
pub fn agent_files(agent: ContextAgent, short: &str, skill: &str, probe: &ContextProbe, roots: Option<&GuestRoots>) -> ContextHooks {
    let roots = roots.cloned().unwrap_or_else(guest_roots);
    let module = agent.context.expect("a context agent carries its context module");
    module.hooks(&ContextHookInput {
        short: short.to_string(),
        skill: skill.to_string(),
        source: context_path(Some(&roots)),
        roots: roots.clone(),
        conflict: probe.conflicts.contains(&agent.id),
        fish: probe.has.contains("fish"),
    })
}

/// machine-context.ts:455.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextResult {
    pub agent: String,
    pub outcome: ContextOutcomeKind,
    pub path: Option<String>,
    pub note: Option<String>,
    pub skill: String,
}

/// machine-context.ts:469.
pub fn context_line(r: &ContextResult) -> String {
    let path = r.path.as_deref().unwrap_or_default();
    let note = r.note.as_deref().unwrap_or_default();
    let head = match r.outcome {
        ContextOutcomeKind::Written => format!("context.{}: {path}", r.agent),
        ContextOutcomeKind::Fallback => format!("context.{}: fallback {path}, {note}", r.agent),
        ContextOutcomeKind::NotLoaded => {
            format!("context.{}: not loaded, {note}; only the {SKILL_NAME} skill's description is in the prompt", r.agent)
        }
    };
    format!("{head}; skill {}", r.skill)
}

/// machine-context.ts:474.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOutcome {
    pub context: Vec<ContextResult>,
    /// One line for a stage detail.
    pub summary: String,
    /// Set when the probe or the write did not run; nothing was changed on the guest then.
    pub failure: Option<String>,
}

/// machine-context.ts:482.
#[derive(Clone, Default)]
pub struct ApplyContextOptions {
    pub workspace: Option<WorkspaceName>,
    pub golden: Option<GoldenPick>,
    pub result: Option<ImportResult>,
    pub roots: Option<GuestRoots>,
    /// The upload's transport; tests hand in one.
    pub fetch: Option<Arc<dyn Fetch>>,
    /// The PATH the probe runs on; `None` is TOOLS_PATH.
    pub path: Option<String>,
    pub shells: Option<ProbeShells>,
    pub at_provider: bool,
}

/// machine-context.ts:498.
pub fn summarize(results: &[ContextResult], agents: &[String], bytes: u64) -> String {
    let size = fmt_bytes(bytes as f64);
    if agents.is_empty() {
        return format!("{size} written; no agent on the machine");
    }
    let written: Vec<String> = results.iter().filter(|r| r.outcome == ContextOutcomeKind::Written).map(|r| agent_name(&r.agent)).collect();
    let mut parts = vec![if written.is_empty() { format!("{size} written") } else { format!("{size} written for {}", written.join(", ")) }];
    for r in results {
        let name = agent_name(&r.agent);
        let note = r.note.as_deref().unwrap_or_default();
        match r.outcome {
            ContextOutcomeKind::Fallback => parts.push(format!("{name} by its fallback ({note})")),
            ContextOutcomeKind::NotLoaded => parts.push(format!("{name} not loaded ({note}), skill only")),
            ContextOutcomeKind::Written => {}
        }
    }
    parts.retain(|p| !p.is_empty());
    parts.join("; ")
}

fn failed(failure: String) -> ContextOutcome {
    ContextOutcome { context: Vec::new(), summary: format!("not written ({failure})"), failure: Some(failure) }
}

/// machine-context.ts:514. Never fails: a guest that does not answer is the outcome's `failure`.
pub async fn apply_machine_context(machine: &dyn Machine, opts: ApplyContextOptions) -> ContextOutcome {
    let roots = opts.roots.clone().unwrap_or_else(guest_roots);
    let path = opts.path.clone().unwrap_or_else(|| TOOLS_PATH.to_string());
    let cmd = probe_command(Some(&roots), Some(&path), Some(opts.shells.unwrap_or_default()));
    let probed = match machine.exec(&cmd, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..Default::default() }).await {
        Ok(probed) => probed,
        Err(e) => return failed(format!("probe failed: {e}")),
    };
    let Some(probe) = parse_probe(&probed.stdout) else {
        return failed(format!("probe answered exit {} without its markers", probed.exit_code.0 .0));
    };
    let prior = probe.facts.clone().unwrap_or_default();
    let facts = merge_facts(Some(&prior), opts.result.as_ref());
    let input = ContextInput {
        workspace: opts.workspace.clone(),
        golden: opts.golden.clone(),
        probe: probe.clone(),
        facts: facts.clone(),
        agent: None,
        at_provider: opts.at_provider,
    };
    let mut files = vec![
        GuestFile { path: context_path(Some(&roots)), mode: MODE, content: render_short_context(&input) },
        GuestFile { path: skill_path(Some(&roots)), mode: MODE, content: render_skill(&render_machine_context(&input)) },
        GuestFile { path: facts_path(&roots), mode: MODE, content: format!("{}\n", serde_json::to_string_pretty(&facts).unwrap_or_default()) },
    ];
    let mut context: Vec<ContextResult> = Vec::new();
    for agent in context_agents() {
        if !probe.agents.contains(&agent.id) {
            continue;
        }
        let for_agent = ContextInput { agent: Some(agent), ..input.clone() };
        let out = agent_files(
            agent,
            &render_short_context(&for_agent),
            &render_skill(&render_machine_context(&for_agent)),
            &probe,
            Some(&roots),
        );
        files.extend(out.files.iter().cloned());
        context.push(ContextResult { agent: agent.id.clone(), outcome: out.outcome, path: out.path, note: out.note, skill: out.skill });
    }
    let entries: Vec<TarEntry> =
        files.iter().map(|f| TarEntry::File { path: f.path.clone(), mode: f.mode, content: f.content.clone().into_bytes() }).collect();
    let tar = match tar_of(&entries) {
        Ok(tar) => tar,
        Err(e) => return failed(format!("write failed: {}", squeeze(&e.to_string()))),
    };
    let vault = VaultOptions { overlay: true, fetch: opts.fetch.clone(), ..Default::default() };
    if let Err(e) = import_into(machine, &tar, "/", &vault).await {
        return failed(format!("write failed: {}", squeeze(&e.to_string())));
    }
    let summary = summarize(&context, &probe.agents, tar.len() as u64);
    ContextOutcome { context, summary, failure: None }
}

/// The write's failure words as one stage line: trimmed, and each run of newlines is "; ".
fn squeeze(message: &str) -> String {
    js_trim(message).split('\n').filter(|piece| !piece.is_empty()).collect::<Vec<_>>().join("; ")
}
