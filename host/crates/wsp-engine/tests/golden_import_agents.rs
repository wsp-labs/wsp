// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/golden-import.test.ts at a4033dc34, the agents' half: "agentInstallsFor" and "the
//! agents as steps of the one tools loop". Not ported: the case at line 1518 ("takes each agent off the way its line
//! put it on"), which tests agentUninstall, outside this port.
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use indexmap::IndexMap;
use wsp_catalog::catalog::{catalog_agents, catalog_entry, AgentEntry};
use wsp_catalog::roads::{
    node_install_script, InstallRoad, NodeRelease, PackageRoad, CLAUDE_CODE, CLAUDE_INSTALL, NODE_PATH_LINE, NODE_RELEASES,
};
use wsp_engine::golden_seam::{
    agent_installers, agent_installs_for, agent_installs_for_rows, agent_steps, node_floor_check, node_major_for, path_line,
    AgentInstaller, AgentsPlan, RecipeEntry, SkippedItem, ToolInstall, AGENT_NODE_STEP, CURRENT_LTS, TOOLS_PATH,
};
use wsp_engine::provision::{presence_tests, Versions};

// --- helpers ------------------------------------------------------------------------------------------------

/// golden-import.test.ts:65 `row`: a ticked row with the id as its label.
fn row(rung: &str, id: &str) -> RecipeEntry {
    RecipeEntry {
        rung: rung.to_string(),
        id: id.to_string(),
        label: id.to_string(),
        paths: Vec::new(),
        bytes: 0,
        default: "bring".to_string(),
        bring: Some(true),
        ..RecipeEntry::default()
    }
}

fn unticked(rung: &str, id: &str) -> RecipeEntry {
    RecipeEntry { bring: Some(false), ..row(rung, id) }
}

/// golden-import.test.ts:1611 `ticked`.
fn ticked(ids: &[&str]) -> Vec<RecipeEntry> {
    ids.iter().map(|id| row("agents", id)).collect()
}

/// `CATALOG_AGENTS`, owned.
fn agents() -> Vec<AgentEntry> {
    catalog_agents().into_iter().cloned().collect()
}

/// `AGENT_INSTALLERS`.
fn installers() -> IndexMap<String, AgentInstaller> {
    agent_installers(&agents())
}

fn ms(iso: &str) -> i64 {
    iso.parse::<jiff::Timestamp>().unwrap().as_millisecond()
}

/// `new Date()`.
fn now_ms() -> i64 {
    jiff::Timestamp::now().as_millisecond()
}

/// `NODE_RELEASES[major]`.
fn release(major: u32) -> NodeRelease {
    NODE_RELEASES.iter().find(|(m, _)| *m == major).expect("a pinned major").1
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// What `toMatchObject({ floor, version, agents })` reads of a plan's node step.
fn node_of(plan: &AgentsPlan) -> (u32, String, Vec<String>) {
    let n = plan.node.as_ref().expect("the plan asks for node");
    (n.floor, n.version.clone(), n.agents.clone())
}

/// Every spawn and every stub write takes this, so no fork holds a stub's write handle open while it is run.
static SHELL: Mutex<()> = Mutex::new(());

/// A stub program a test runs by name on a PATH. The box umask is 077, so the mode is set explicitly.
fn write_stub(path: &Path, script: &str) {
    let _g = SHELL.lock().unwrap_or_else(|e| e.into_inner());
    std::fs::write(path, script).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// The exit status of `script` under this computer's bash, `PATH` set where given; -1 where it has none.
fn bash_status(script: &str, path: Option<&str>) -> i32 {
    let _g = SHELL.lock().unwrap_or_else(|e| e.into_inner());
    let mut c = Command::new("bash");
    c.arg("-c").arg(script);
    if let Some(p) = path {
        c.env("PATH", p);
    }
    c.output().expect("bash runs").status.code().unwrap_or(-1)
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn starts_digit(s: &str) -> bool {
    s.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// The text after one or more leading digits.
fn after_digits(s: &str) -> Option<&str> {
    let rest = s.trim_start_matches(|c: char| c.is_ascii_digit());
    if rest.len() == s.len() {
        None
    } else {
        Some(rest)
    }
}

fn is_hex(s: &str, len: usize) -> bool {
    s.len() == len && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// `/\|\s*(ba)?sh\b/`, or with `any_ws` false `/\| *(ba)?sh\b/`.
fn pipes_into_shell(s: &str, any_ws: bool) -> bool {
    s.match_indices('|').any(|(i, _)| {
        let rest = s[i + 1..].trim_start_matches(|c: char| if any_ws { c.is_whitespace() } else { c == ' ' });
        ["sh", "bash"].iter().any(|w| rest.strip_prefix(w).is_some_and(|after| !after.chars().next().is_some_and(is_word)))
    })
}

/// `/@latest\b/`.
fn has_at_latest(s: &str) -> bool {
    s.match_indices("@latest").any(|(i, m)| !s[i + m.len()..].chars().next().is_some_and(is_word))
}

/// `/(prefix)v?\d/` for one literal prefix.
fn has_v_digit_after(s: &str, prefix: &str) -> bool {
    s.match_indices(prefix).any(|(i, m)| {
        let rest = &s[i + m.len()..];
        starts_digit(rest.strip_prefix('v').unwrap_or(rest))
    })
}

/// `/@\d|==\d|--branch v?\d|releases\/download\/v?\d|\/\d+\.\d+\.\d+\/|\bver='\d/`.
fn names_a_version(s: &str) -> bool {
    let after = |needle: &str| s.match_indices(needle).any(|(i, m)| starts_digit(&s[i + m.len()..]));
    let slash_semver = s.match_indices('/').any(|(i, _)| {
        after_digits(&s[i + 1..])
            .and_then(|r| r.strip_prefix('.'))
            .and_then(after_digits)
            .and_then(|r| r.strip_prefix('.'))
            .and_then(after_digits)
            .is_some_and(|r| r.starts_with('/'))
    });
    let ver_quote = s.match_indices("ver='").any(|(i, m)| {
        let boundary = !s[..i].chars().next_back().is_some_and(is_word);
        boundary && starts_digit(&s[i + m.len()..])
    });
    after("@")
        || after("==")
        || has_v_digit_after(s, "--branch ")
        || has_v_digit_after(s, "releases/download/")
        || slash_semver
        || ver_quote
}

/// `/git clone .*--branch v\d{4}\.\d+\.\d+ https:\/\/github\.com\/NousResearch\/hermes-agent\.git \/root\/\.hermes\/hermes-agent/`.
fn hermes_clone_pinned(install: &str) -> bool {
    const TAIL: &str = " https://github.com/NousResearch/hermes-agent.git /root/.hermes/hermes-agent";
    install.lines().any(|line| {
        line.match_indices("git clone ").any(|(i, _)| {
            let after_clone = &line[i..];
            after_clone.match_indices("--branch v").any(|(j, m)| {
                let rest = &after_clone[j + m.len()..];
                let year = rest.get(..4).is_some_and(|y| y.chars().all(|c| c.is_ascii_digit()));
                year && rest[4..]
                    .strip_prefix('.')
                    .and_then(after_digits)
                    .and_then(|r| r.strip_prefix('.'))
                    .and_then(after_digits)
                    .is_some_and(|r| r.starts_with(TAIL))
            })
        })
    })
}

/// `/rev-parse HEAD\)" = "[0-9a-f]{40}"/`.
fn rev_parse_pinned(install: &str) -> bool {
    const NEEDLE: &str = "rev-parse HEAD)\" = \"";
    install.match_indices(NEEDLE).any(|(i, m)| {
        let rest = &install[i + m.len()..];
        rest.get(..40).is_some_and(|sha| is_hex(sha, 40)) && rest[40..].starts_with('"')
    })
}

// --- "agentInstallsFor" ---------------------------------------------------------------------------------------

// TS: packages/engine/test/golden-import.test.ts:1498
// "every known agent has a pinned installer, its version check, and the documentation it was read from"
#[test]
fn every_known_agent_has_a_pinned_installer_its_version_check_and_its_documentation() {
    let table = installers();
    for (name, a) in &table {
        assert_ne!(a.name, "", "{name}");
        assert!(a.smoke.contains("--version"), "{name}: {}", a.smoke);
        assert!(!pipes_into_shell(&a.install, true), "{name}: {}", a.install);
        assert!(!has_at_latest(&a.install), "{name}: {}", a.install);
        // Every agent's line names the version it installs, the harness vendor's binary and a cask's own version line included.
        assert!(names_a_version(&a.install), "{name}: {}", a.install);
    }
    let mut keys: Vec<&str> = table.keys().map(String::as_str).collect();
    keys.sort();
    assert_eq!(keys, vec!["aider", "amp", "claude", "codex", "crush", "cursor", "gemini", "goose", "hermes", "opencode", "pi", "qwen"]);
    // The harness installs at the version its own text fixes, so a copy built from the pin gets that version.
    let claude = &table["claude"];
    assert_eq!(claude.name, "Claude Code");
    assert_eq!(claude.install, CLAUDE_INSTALL.as_str());
    assert_eq!(claude.smoke, "claude --version");
    assert_eq!(claude.road, "script");
    assert_eq!(claude.node, None);
    let pin = claude.pin.as_ref().expect("claude has a pin read");
    assert!(pin.read.as_deref().is_some_and(|r| r.contains("'claude' --version")), "{:?}", pin.read);
    assert!(pin.fixed);
    assert_eq!(pin.words, "by its own installer");
    // The road each line walks, which is what bounds a step that runs it on a computer somebody owns.
    let roads: BTreeMap<&str, &str> = table.iter().map(|(k, a)| (k.as_str(), a.road.as_str())).collect();
    let want_roads: BTreeMap<&str, &str> = [
        ("claude", "script"),
        ("codex", "npm"),
        ("gemini", "npm"),
        ("opencode", "npm"),
        ("aider", "uv"),
        ("pi", "npm"),
        ("hermes", "script"),
        ("crush", "release"),
        ("qwen", "npm"),
        ("goose", "release"),
        ("amp", "npm"),
        ("cursor", "vendor"),
    ]
    .into_iter()
    .collect();
    assert_eq!(roads, want_roads);
    let codex_pin = table["codex"].pin.as_ref().expect("codex has a pin read");
    assert!(codex_pin.read.as_deref().is_some_and(|r| r.contains("npm root -g")), "{:?}", codex_pin.read);
    assert!(codex_pin.fixed);
    assert_eq!(codex_pin.words, "as an npm global");
    let hermes_pin = table["hermes"].pin.as_ref().expect("hermes has a pin read");
    assert!(hermes_pin.read.as_deref().is_some_and(|r| r.contains("'hermes' --version")), "{:?}", hermes_pin.read);
    assert!(hermes_pin.fixed);
    assert_eq!(hermes_pin.words, "by its own installer");
    // Engines floors as the registry states them at the pinned versions.
    let nodes: BTreeMap<&str, Option<u32>> = table.iter().map(|(k, a)| (k.as_str(), a.node)).collect();
    let want_nodes: BTreeMap<&str, Option<u32>> = [
        ("claude", None),
        ("codex", Some(16)),
        ("gemini", Some(20)),
        ("opencode", None),
        ("aider", None),
        ("pi", Some(22)),
        ("hermes", None),
        ("crush", None),
        ("qwen", Some(22)),
        ("goose", None),
        ("amp", None),
        ("cursor", None),
    ]
    .into_iter()
    .collect();
    assert_eq!(nodes, want_nodes);
}

// TS: packages/engine/test/golden-import.test.ts:1526
// "installs only the ticked agents, in recipe order, each from the catalog's table"
#[test]
fn installs_only_the_ticked_agents_in_recipe_order_each_from_the_catalogs_table() {
    let a = agent_installs_for_rows(
        &[
            row("agents", "agents/claude"),
            row("agents", "agents/codex"),
            unticked("agents", "agents/gemini"),
            row("agents", "agents/unknown-thing"),
            row("shell", "shell/zshrc"),
        ],
        &agents(),
        now_ms(),
    );
    let got: Vec<[&str; 3]> = a.installs.iter().map(|i| [i.id.as_str(), i.name.as_str(), i.smoke.as_str()]).collect();
    assert_eq!(got, vec![["agents/claude", "Claude Code", "claude --version"], ["agents/codex", "Codex", "codex --version"]]);
    assert_eq!(a.installs[0].install, CLAUDE_INSTALL.as_str());
    assert!(a.installs[1].install.contains("npm install -g @openai/codex@"), "{}", a.installs[1].install);
    assert_eq!(a.skipped, vec![SkippedItem { id: "agents/unknown-thing".to_string(), note: "no installer known".to_string() }]);
}

// TS: packages/engine/test/golden-import.test.ts:1543
// "asks for Node once, at the lowest supported pinned major that meets every ticked agent's floor, else the current LTS, and only for the agents that run on it"
#[test]
fn asks_for_node_once_at_the_lowest_supported_pinned_major_meeting_every_ticked_floor() {
    let today = ms("2026-09-03T00:00:00Z");
    let catalog = agents();
    let plan_of = |ids: &[&str]| agent_installs_for_rows(&ticked(ids), &catalog, today);
    let v22 = || NODE_RELEASES.iter().find(|(m, _)| *m == 22).unwrap().1.version.to_string();
    // Node 20 left maintenance in April 2026: a Gemini-only recipe gets 22, not 20. OpenCode is an npm global with
    // no floor of its own, so it asks for node too, and the floor the two share is Gemini's.
    let gemini = plan_of(&["agents/gemini", "agents/opencode"]);
    assert_eq!(node_of(&gemini), (20, v22(), names(&["Gemini CLI", "OpenCode"])));
    let codex_only = plan_of(&["agents/codex"]);
    assert_eq!(node_of(&codex_only), (16, v22(), names(&["Codex"])));
    let both = plan_of(&["agents/gemini", "agents/pi"]);
    assert_eq!(node_of(&both), (22, v22(), names(&["Gemini CLI", "Pi"])));
    // An agent whose installer is an npm global brings node at the major the catalog's own row pins; nothing that
    // the floor no longer carries is assumed to be there.
    let opencode = plan_of(&["agents/opencode"]);
    assert_eq!(node_of(&opencode), (22, v22(), names(&["OpenCode"])));
    // Claude Code's installer is its vendor's own native one and Hermes takes a release: neither asks for node.
    assert_eq!(plan_of(&["agents/claude", "agents/hermes"]).node, None);
    assert_eq!(plan_of(&["agents/aider"]).node, None);
    assert_eq!(agent_installs_for_rows(&[unticked("agents", "agents/pi")], &catalog, today).node, None);
    // While 20 was still in maintenance it was the lowest satisfying major.
    assert_eq!(node_major_for(20, ms("2026-01-15T00:00:00Z")), Some(20));
    assert_eq!(node_major_for(20, today), Some(22));
    assert_eq!(node_major_for(16, today), Some(22));
    assert_eq!(node_major_for(22, ms("2028-01-01T00:00:00Z")), Some(CURRENT_LTS));
    assert_eq!(node_major_for(24, today), None);
}

// TS: packages/engine/test/golden-import.test.ts:1569
// "an agent whose floor no pinned major meets is set aside with a note rather than installed on a Node its engines refuse"
#[test]
fn an_agent_whose_floor_no_pinned_major_meets_is_set_aside_with_a_note() {
    let mut future = agents().into_iter().find(|a| a.id == "codex").expect("the catalog has codex");
    future.id = "future".to_string();
    future.name = "Future".to_string();
    future.install_road = InstallRoad::Npm {
        package: PackageRoad { package: "future".to_string(), version: Some("1.0.0".to_string()), pin: None },
        ignore_scripts: None,
    };
    future.node = Some(24);
    let mut all = agents();
    all.push(future);
    let a = agent_installs_for_rows(&[row("agents", "agents/future"), row("agents", "agents/codex")], &all, now_ms());
    assert_eq!(a.installs.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), vec!["agents/codex"]);
    assert_eq!(a.skipped, vec![SkippedItem { id: "agents/future".to_string(), note: "needs Node 24, none pinned".to_string() }]);
    let n = a.node.as_ref().expect("codex still asks for node");
    assert_eq!((n.floor, n.agents.clone()), (16, names(&["Codex"])));
}

// TS: packages/engine/test/golden-import.test.ts:1577
// "the Node script keeps a guest whose major is between the floor and the pin's, else installs the pinned, sha256-checked release into /usr/local"
#[test]
fn the_node_script_keeps_a_guest_between_the_floor_and_the_pin_else_installs_the_pinned_release() {
    let r20 = release(20);
    let script = node_install_script(20, &r20);
    assert!(script.contains("echo \"NODE_HAVE $node_have\""), "{script}");
    assert!(script.contains(r##"-ge 20 ] && [ "${node_major:-0}" -le 20 ]; then echo "NODE_KEPT $node_have"; exit 0; fi"##), "{script}");
    assert!(script.contains(&format!("https://nodejs.org/dist/v{}/", r20.version)), "{script}");
    assert!(script.contains(&format!("node-v{}-linux-x64.tar.gz sha={}", r20.version, r20.sha256.x86_64)), "{script}");
    assert!(script.contains(&format!("node-v{}-linux-arm64.tar.gz sha={}", r20.version, r20.sha256.aarch64)), "{script}");
    assert!(script.contains("sha256sum -c"), "{script}");
    assert!(script.contains("-C /usr/local --strip-components=1"), "{script}");
    // The install is proven by the node on the agents' PATH being the pinned one before it is reported.
    let installed = script.find(&format!("echo \"NODE_INSTALLED v{}\"", r20.version)).expect("the script reports the install");
    let check = script.find(&format!("test \"$(node --version)\" = \"v{}\"", r20.version)).expect("the script checks the version");
    let path = script.find("export PATH=\"/usr/local/bin:$PATH\"").expect("the script puts /usr/local/bin first");
    let strip = script.find("--strip-components=1").expect("the script extracts");
    assert!(path > strip, "path {path} after extract {strip}");
    assert!(check > path, "check {check} after path {path}");
    assert!(installed > check, "report {installed} after check {check}");
    assert!(!script.contains("apt") && !script.contains("nvm"), "{script}");
    assert!(!pipes_into_shell(&script, false), "{script}");
    for (_, r) in NODE_RELEASES.iter() {
        assert!(is_hex(r.sha256.x86_64, 64), "{}", r.sha256.x86_64);
        assert!(is_hex(r.sha256.aarch64, 64), "{}", r.sha256.aarch64);
    }
}

// TS: packages/engine/test/golden-import.test.ts:1600
// "the hermes and aider installers bring uv by a checksummed release binary and pin the source"
#[test]
fn the_hermes_and_aider_installers_bring_uv_by_a_checksummed_release_binary_and_pin_source() {
    let table = installers();
    let hermes = &table["hermes"].install;
    assert!(hermes.contains("sha256sum -c"), "{hermes}");
    assert!(hermes_clone_pinned(hermes), "{hermes}");
    assert!(rev_parse_pinned(hermes), "{hermes}");
    assert!(hermes.contains("uv venv --python 3.11 /root/.hermes/venvs/hermes"), "{hermes}");
    let aider = &table["aider"].install;
    const UV_LINE: &str = "uv tool install --force --python 3.12 --with pip aider-chat==";
    assert!(aider.match_indices(UV_LINE).any(|(i, m)| starts_digit(&aider[i + m.len()..])), "{aider}");
}

// --- "the agents as steps of the one tools loop" ---------------------------------------------------------------

fn step_of<'a>(steps: &'a [ToolInstall], id: &str) -> &'a ToolInstall {
    steps.iter().find(|t| t.id == id).unwrap_or_else(|| panic!("no step {id}"))
}

// TS: packages/engine/test/golden-import.test.ts:1613
// "puts the node step first and every agent whose road runs on node after it, each by its own road"
#[test]
fn puts_the_node_step_first_and_every_agent_whose_road_runs_on_node_after_it() {
    let steps = agent_steps(&agent_installs_for(&["agents/claude", "agents/codex"]), TOOLS_PATH, None);
    let got: Vec<(&str, &str, Option<&str>)> = steps.iter().map(|t| (t.id.as_str(), t.manager.as_str(), t.after.as_deref())).collect();
    assert_eq!(
        got,
        vec![
            (AGENT_NODE_STEP, "script", None),
            // The vendor's own installer brings whatever it needs, so it waits on nothing.
            ("agents/claude", "script", None),
            ("agents/codex", "npm", Some(AGENT_NODE_STEP)),
        ]
    );
    let codex = step_of(&steps, "agents/codex");
    let table = installers();
    let codex_installer = &table["codex"];
    // The agent's own version check is the step's check, its command is the step's bin, and the version the
    // catalog's road pins is what the step asks for, so a computer that answers at it installs nothing.
    assert_eq!(codex.check.as_deref(), Some(codex_installer.smoke.as_str()));
    assert_eq!(codex.bin.as_deref(), Some("codex"));
    let entry = catalog_entry("codex").expect("the catalog has codex");
    let want_asks = match entry.install_road() {
        InstallRoad::Npm { package, .. } => package.version.clone(),
        _ => None,
    };
    assert_eq!(codex.asks, want_asks);
    assert_eq!(codex.pin, codex_installer.pin);
    // The install line runs on the tools PATH with the Node the step put on ahead of it.
    assert!(codex.cmd.starts_with(&path_line(TOOLS_PATH, None)), "{}", codex.cmd);
    assert!(codex.cmd.contains(NODE_PATH_LINE), "{}", codex.cmd);
    assert!(codex.cmd.contains(&codex_installer.install), "{}", codex.cmd);
}

// TS: packages/engine/test/golden-import.test.ts:1635
// "names the node step after the release it installs, and reads the floor its script reads"
#[test]
fn names_the_node_step_after_the_release_it_installs_and_reads_the_floor_its_script_reads() {
    let plan = agent_installs_for(&["agents/codex"]);
    let steps = agent_steps(&plan, TOOLS_PATH, None);
    let node = step_of(&steps, AGENT_NODE_STEP);
    let planned = plan.node.as_ref().expect("codex asks for node");
    assert_eq!(node.label, format!("Node {}", planned.version));
    assert_eq!(node.cmd, planned.cmd);
    // The check is the floor the script itself keeps a node for, so a computer that already meets it runs nothing
    // and the row reads present on every run after the first.
    assert_eq!(node.check.as_deref(), Some(node_floor_check(planned.floor).as_str()));
    assert!(node.check.as_deref().is_some_and(|c| c.contains(&format!("-ge {}", planned.floor))), "{:?}", node.check);
    // A server that runs node waits on nothing once the agents step ended, read off this.
    assert_eq!(node.bin.as_deref(), Some("node"));
}

// TS: packages/engine/test/golden-import.test.ts:1648
// "reads that floor the way the install script reads it, under a real shell"
#[test]
fn reads_that_floor_the_way_the_install_script_reads_it_under_a_real_shell() {
    // Adapted: the TS reads the node this process runs on; a stub `node` answering v22.4.0 stands in so the case
    // reads the same on a computer with any node or none.
    let dir = tempfile::tempdir().unwrap();
    write_stub(&dir.path().join("node"), "#!/bin/sh\necho v22.4.0\n");
    let here: u32 = 22;
    let path = format!("{}:/usr/bin:/bin", dir.path().display());
    let run = |line: &str| bash_status(&format!("( {line} )"), Some(&path));
    assert_eq!(run(&node_floor_check(here)), 0);
    assert_ne!(run(&node_floor_check(here + 1)), 0);
    // A computer with no node at all reads as under every floor rather than as an error.
    assert_ne!(bash_status(&format!("( PATH=/nonexistent; {} )", node_floor_check(18)), None), 0);
}

// TS: packages/engine/test/golden-import.test.ts:1657
// "keeps an agent a computer has at the pin or newer, and installs over an older one, under a real shell"
#[test]
fn keeps_an_agent_a_computer_has_at_the_pin_or_newer_and_installs_over_an_older_one() {
    let dir = tempfile::tempdir().unwrap();
    let steps = agent_steps(&agent_installs_for(&["agents/claude"]), TOOLS_PATH, None);
    let step = step_of(&steps, "agents/claude");
    assert_eq!(step.asks.as_deref(), Some(CLAUDE_CODE.version));
    let parts: Vec<i64> = CLAUDE_CODE.version.split('.').map(|p| p.parse().unwrap()).collect();
    let (major, minor, patch) = (parts[0], parts[1], parts[2]);
    let tests = presence_tests(step, Versions::Asked).join(" && ");
    let present = |version: String| -> bool {
        write_stub(&dir.path().join("claude"), &format!("#!/bin/sh\necho '{version} (Claude Code)'\n"));
        bash_status(&format!("export PATH={}:/usr/bin:/bin\n{tests}", dir.path().display()), None) == 0
    };
    assert!(present(format!("{major}.{minor}.{}", patch + 16)));
    assert!(present(format!("{major}.{}.0", minor + 1)));
    assert!(present(CLAUDE_CODE.version.to_string()));
    assert!(!present(format!("{major}.{minor}.{}", patch - 10)));
    assert!(!present(format!("{major}.{}.{}", minor - 1, patch + 100)));
}

// TS: packages/engine/test/golden-import.test.ts:1674
// "carries no node step when nothing ticked runs on node, and nothing at all for no agent"
#[test]
fn carries_no_node_step_when_nothing_ticked_runs_on_node_and_nothing_for_no_agent() {
    let claude = agent_steps(&agent_installs_for(&["agents/claude"]), TOOLS_PATH, None);
    assert_eq!(claude.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["agents/claude"]);
    assert_eq!(agent_steps(&agent_installs_for(&[]), TOOLS_PATH, None), Vec::<ToolInstall>::new());
}
