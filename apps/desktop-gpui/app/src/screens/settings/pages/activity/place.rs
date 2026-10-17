// SPDX-License-Identifier: AGPL-3.0-only
//! Activity's one reading of a computer's processes, after `views/activity.ts`'s `placeProcs` and
//! `views/activity-names.ts`: whose each process is (a thread's, wsp's own, or wsp's with no thread known), what kind
//! it is, what it is called, and which rows End reaches.

use std::collections::{HashMap, HashSet};

use super::wire::{AppKind, AppPart, Kind, OwnedBy, Pid, ProcEntry};

/// The catalog's agents (`CATALOG_AGENTS`): each id, the command it puts on PATH and its name.
pub const AGENTS: [(&str, &str, &str); 11] = [
    ("claude", "claude", "Claude Code"),
    ("codex", "codex", "Codex"),
    ("gemini", "gemini", "Gemini CLI"),
    ("opencode", "opencode", "OpenCode"),
    ("pi", "pi", "Pi"),
    ("hermes", "hermes", "Hermes Agent"),
    ("crush", "crush", "Crush"),
    ("qwen", "qwen", "Qwen Code"),
    ("goose", "goose", "Goose"),
    ("amp", "amp", "Amp"),
    ("cursor", "cursor-agent", "Cursor"),
];

/// `agentName`: the catalog's name for an agent, or its id.
pub fn agent_name(id: &str) -> String {
    AGENTS.iter().find(|a| a.0 == id).map(|a| a.2.to_owned()).unwrap_or_else(|| id.to_owned())
}

pub fn agent_bin(id: &str) -> Option<&'static str> {
    AGENTS.iter().find(|a| a.0 == id).map(|a| a.1)
}

/// The cgroup every box thread's own stands under (`THREAD_CGROUPS`).
pub const THREAD_CGROUPS: &str = "/wsp-threads";

/// `threadCgroup`: the cgroup a thread's processes stand in on a box.
pub fn thread_cgroup(thread_id: &str) -> String {
    let safe: String = thread_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' }).collect();
    format!("{THREAD_CGROUPS}/{safe}")
}

/// `inCgroup`: whether a row's cgroup is this one or one under it.
pub fn in_cgroup(at: Option<&str>, cgroup: &str) -> bool {
    at.is_some_and(|at| at == cgroup || at.strip_prefix(cgroup).is_some_and(|rest| rest.starts_with('/')))
}

const SCRIPT_RUNTIMES: [&str; 6] = ["node", "python", "python3", "bun", "deno", "ruby"];
const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "fish", "dash"];

fn base(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

/// `procBins`: the names a process runs by, its comm, argv[0]'s, and the script argv[0] runs where that is a runtime.
pub fn proc_bins(p: &ProcEntry) -> Vec<String> {
    let argv: Vec<&str> = p.cmdline.split(' ').filter(|w| !w.is_empty()).collect();
    let first = argv.first().map(|w| base(w));
    let script =
        first.filter(|f| SCRIPT_RUNTIMES.contains(f)).and_then(|_| argv.iter().skip(1).find(|w| !w.starts_with('-')).map(|w| base(w)));
    [Some(p.comm.as_str()), first, script].into_iter().flatten().filter(|b| !b.is_empty()).map(str::to_owned).collect()
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Whether `phrase` stands in `line` between word boundaries, as `\b(phrase)\b` reads it.
fn has_word(line: &str, phrase: &str) -> bool {
    let mut from = 0;
    while let Some(at) = line[from..].find(phrase) {
        let start = from + at;
        let end = start + phrase.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        let first = phrase.chars().next().is_some_and(is_word_char);
        let last = phrase.chars().next_back().is_some_and(is_word_char);
        let left = before.map_or(first, |b| is_word_char(b) != first);
        let right = after.map_or(last, |a| is_word_char(a) != last);
        if left && right {
            return true;
        }
        from = start + phrase.chars().next().map_or(1, char::len_utf8);
    }
    false
}

fn any_word(line: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|p| has_word(line, p))
}

fn test_line(line: &str) -> bool {
    any_word(
        line,
        &["vitest", "jest", "mocha", "pytest", "playwright test", "cargo test", "cargo nextest", "go test", "bun test", "node --test"],
    )
}

fn build_line(line: &str) -> bool {
    if any_word(line, &["rustc", "cargo build", "cargo check", "cc1", "cc1plus", "clang", "ld", "tsc", "go build", "swiftc"]) {
        return true;
    }
    // `webpack(?! serve)`: webpack the bundler, not its dev server.
    line.match_indices("webpack").any(|(start, word)| {
        let rest = &line[start + word.len()..];
        line[..start].chars().next_back().is_none_or(|c| !is_word_char(c))
            && rest.chars().next().is_none_or(|c| !is_word_char(c))
            && !rest.starts_with(" serve")
    })
}

fn dev_line(line: &str) -> bool {
    any_word(
        line,
        &[
            "vite",
            "next dev",
            "next-server",
            "webpack serve",
            "astro dev",
            "nuxt dev",
            "remix dev",
            "wrangler dev",
            "electron-vite",
            "http.server",
            "flask run",
            "uvicorn",
            "rails server",
        ],
    )
}

/// `SLATE_FOLDER`: a slate's runs read their files from `slates/<thread>/` or a box's `wsp-slates/<thread>/`.
pub fn slate_folder(line: &str) -> bool {
    let mut from = 0;
    while let Some(at) = line[from..].find("slates/") {
        let start = from + at;
        let lead = &line[..start];
        let lead = lead.strip_suffix("wsp-").unwrap_or(lead);
        let anchored = lead.is_empty() || lead.ends_with('/');
        let rest = &line[start + "slates/".len()..];
        let name = rest.find(|c: char| c == '/' || c.is_whitespace()).unwrap_or(rest.len());
        if anchored && name > 0 && rest[name..].starts_with('/') {
            return true;
        }
        from = start + 1;
    }
    false
}

/// A path's last part is twelve hex digits and the extension.
fn run_file(path: &str, ext: &str) -> bool {
    let Some(stem) = path.strip_suffix(ext) else { return false };
    let Some(slash) = stem.rfind('/') else { return false };
    let name = &stem[slash + 1..];
    name.len() == 12 && name.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)) && !path.chars().any(char::is_whitespace)
}

/// `isTurnPlumbing`: a turn's run script `bash <folder>/<12 hex>.sh`, or the pump `tail -n +1 -f <folder>/<12 hex>.in`.
pub fn is_turn_plumbing(command: &str) -> bool {
    command.strip_prefix("bash ").is_some_and(|rest| run_file(rest, ".sh"))
        || command.strip_prefix("tail -n +1 -f ").is_some_and(|rest| run_file(rest, ".in"))
}

/// From a root down to the shallowest process under it that is the agent, and on through an only child that is the
/// agent too, root first (`agentPath`).
fn agent_path(root: Pid, placed: &Placement, is_agent: &dyn Fn(&ProcEntry) -> bool) -> Option<Vec<Pid>> {
    let mut via: HashMap<Pid, Pid> = HashMap::new();
    let mut queue = std::collections::VecDeque::from([root]);
    while let Some(pid) = queue.pop_front() {
        if placed.by_pid.get(&pid).is_some_and(|p| is_agent(p)) {
            let mut path = vec![pid];
            let mut at = via.get(&pid).copied();
            while let Some(up) = at {
                path.insert(0, up);
                at = via.get(&up).copied();
            }
            let mut kids = placed.kids(pid);
            while kids.len() == 1 && is_agent(kids[0]) && !path.contains(&kids[0].pid) {
                path.push(kids[0].pid);
                kids = placed.kids(kids[0].pid);
            }
            return Some(path);
        }
        for kid in placed.kids(pid) {
            if via.contains_key(&kid.pid) || kid.pid == root {
                continue;
            }
            via.insert(kid.pid, pid);
            queue.push_back(kid.pid);
        }
    }
    None
}

/// A thread as the placement reads it (`PlaceThread`).
pub struct PlaceThread {
    pub id: String,
    pub roots: Vec<Pid>,
    pub cgroup: Option<String>,
    pub bin: Option<String>,
    pub agents: Vec<Pid>,
}

/// Whose a process is (`Whose`).
#[derive(Clone, Debug, PartialEq)]
pub enum Whose {
    Thread(String),
    Wsp,
    Loose,
}

/// A map that keeps the order its keys were first set in, as a JS Map does, which the group rule reads.
#[derive(Default)]
struct Ordered<V> {
    map: HashMap<Pid, V>,
    order: Vec<Pid>,
}

impl<V> Ordered<V> {
    fn set(&mut self, k: Pid, v: V) {
        if self.map.insert(k, v).is_none() {
            self.order.push(k);
        }
    }
    fn remove(&mut self, k: Pid) {
        if self.map.remove(&k).is_some() {
            self.order.retain(|o| *o != k);
        }
    }
    fn get(&self, k: Pid) -> Option<&V> {
        self.map.get(&k)
    }
    fn has(&self, k: Pid) -> bool {
        self.map.contains_key(&k)
    }
}

/// One computer's processes placed (`Placement`).
pub struct Placement<'a> {
    /// Every live process: a zombie holds no memory and takes no signal.
    pub procs: Vec<&'a ProcEntry>,
    by_pid: HashMap<Pid, &'a ProcEntry>,
    children: HashMap<Pid, Vec<&'a ProcEntry>>,
    pub whose: HashMap<Pid, Whose>,
    /// The process each thread's agent runs as, by its thread.
    agent: Ordered<String>,
    folds_into: Ordered<Pid>,
    kinds: HashMap<Pid, Kind>,
    daemon: Pid,
    host: Option<Pid>,
    app: HashMap<Pid, bool>,
}

fn started_by_wsp(p: &ProcEntry) -> bool {
    p.owner.as_ref().is_some_and(|o| matches!(o.by, OwnedBy::Root | OwnedBy::Fork | OwnedBy::Group | OwnedBy::Env | OwnedBy::Cgroup))
}

impl<'a> Placement<'a> {
    fn kids(&self, pid: Pid) -> &[&'a ProcEntry] {
        self.children.get(&pid).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn agent_of(&self, pid: Pid) -> Option<&str> {
        self.agent.get(pid).map(String::as_str)
    }

    pub fn folds_into(&self, pid: Pid) -> Option<Pid> {
        self.folds_into.get(pid).copied()
    }

    pub fn kind_of(&self, pid: Pid) -> Kind {
        self.kinds.get(&pid).copied().unwrap_or(Kind::Other)
    }

    fn wsp_part(&self, p: &ProcEntry) -> bool {
        p.pid == self.daemon || Some(p.pid) == self.host || self.app.contains_key(&p.pid) || matches!(p.kind, Some(Kind::App | Kind::Wsp))
    }

    /// wsp's own: the daemon, the host, and the app's parts other than its tabs.
    pub fn needed(&self, pid: Pid) -> bool {
        self.by_pid.get(&pid).is_some_and(|p| self.wsp_part(p) && self.app.get(&pid) != Some(&true))
    }

    fn thread_agent(&self, pid: Pid) -> bool {
        self.agent.has(pid) || self.folds_into.has(pid) || self.by_pid.get(&pid).is_some_and(|p| p.kind == Some(Kind::Agent))
    }

    /// Whether End reaches it: a process a thread or wsp started, never a thread's agent or what wsp needs.
    pub fn endable(&self, pid: Pid) -> bool {
        let (Some(p), Some(w)) = (self.by_pid.get(&pid), self.whose.get(&pid)) else { return false };
        !self.thread_agent(pid) && !self.needed(pid) && !self.app.contains_key(&pid) && (matches!(w, Whose::Thread(_)) || started_by_wsp(p))
    }
}

pub struct PlaceInput<'a> {
    pub procs: &'a [ProcEntry],
    pub daemon: Pid,
    pub host: Option<Pid>,
    pub threads: &'a [PlaceThread],
    pub listens: &'a dyn Fn(Pid) -> bool,
    /// The app's own processes: true for a browser tab or frame, false for any other part.
    pub app: HashMap<Pid, bool>,
}

/// `placeProcs`: the one rule that names whose each process is and what it is.
pub fn place_procs<'a>(input: PlaceInput<'a>) -> Placement<'a> {
    let procs: Vec<&ProcEntry> = input.procs.iter().filter(|p| !p.state.starts_with('Z')).collect();
    let by_pid: HashMap<Pid, &ProcEntry> = procs.iter().map(|p| (p.pid, *p)).collect();
    let mut children: HashMap<Pid, Vec<&ProcEntry>> = HashMap::new();
    for p in &procs {
        if p.ppid != p.pid && by_pid.contains_key(&p.ppid) {
            children.entry(p.ppid).or_default().push(p);
        }
    }
    let mut placed = Placement {
        procs,
        by_pid,
        children,
        whose: HashMap::new(),
        agent: Ordered::default(),
        folds_into: Ordered::default(),
        kinds: HashMap::new(),
        daemon: input.daemon,
        host: input.host,
        app: input.app,
    };
    let thread_ids: HashSet<&str> = input.threads.iter().map(|t| t.id.as_str()).collect();
    let bins: HashSet<&str> = AGENTS.iter().map(|a| a.1).collect();

    let mut root_of: HashMap<Pid, Pid> = HashMap::new();
    let mut stand_ins: Vec<(Pid, &PlaceThread)> = Vec::new();
    for t in input.threads {
        let recorded: HashSet<Pid> = t.agents.iter().copied().collect();
        let by_name = |p: &ProcEntry| proc_bins(p).iter().any(|b| t.bin.as_deref().map_or(bins.contains(b.as_str()), |bin| b == bin));
        let stamped: Vec<Pid> = placed
            .procs
            .iter()
            .filter(|p| p.kind == Some(Kind::Agent) && p.owner.as_ref().and_then(|o| o.thread.as_deref()) == Some(t.id.as_str()))
            .map(|p| p.pid)
            .collect();
        let mut named: Vec<Pid> = Vec::new();
        for r in t.roots.iter().chain(&stamped) {
            if !named.contains(r) && placed.by_pid.contains_key(r) {
                named.push(*r);
            }
        }
        stand_ins.extend(named.iter().map(|r| (*r, t)));
        let mut heads: Vec<Pid> = Vec::new();
        if let (true, Some(cgroup)) = (t.roots.is_empty(), t.cgroup.as_deref()) {
            let inside: HashSet<Pid> = placed.procs.iter().filter(|p| in_cgroup(p.cgroup.as_deref(), cgroup)).map(|p| p.pid).collect();
            heads = placed.procs.iter().filter(|p| inside.contains(&p.pid) && !inside.contains(&p.ppid)).map(|p| p.pid).collect();
        }
        let mut roots = named.clone();
        for h in heads {
            if !roots.contains(&h) {
                roots.push(h);
            }
        }
        for root in roots {
            let mut path = agent_path(root, &placed, &|p| recorded.contains(&p.pid));
            match &mut path {
                None => path = agent_path(root, &placed, &by_name),
                Some(path) => {
                    let mut kids = placed.kids(*path.last().unwrap()).to_vec();
                    while kids.len() == 1 && by_name(kids[0]) && !path.contains(&kids[0].pid) {
                        path.push(kids[0].pid);
                        kids = placed.kids(kids[0].pid).to_vec();
                    }
                }
            }
            let Some(path) = path else { continue };
            let at = *path.last().unwrap();
            placed.agent.set(at, t.id.clone());
            placed.folds_into.remove(at);
            root_of.insert(at, root);
            for pid in &path[..path.len() - 1] {
                if placed.agent.has(*pid) {
                    continue;
                }
                placed.folds_into.set(*pid, at);
                root_of.insert(*pid, root);
            }
        }
    }
    for (root, t) in stand_ins {
        if placed.agent.has(root) || placed.folds_into.has(root) {
            continue;
        }
        placed.agent.set(root, t.id.clone());
        root_of.insert(root, root);
    }
    // The pump and the script's shells stand in the group of the agent's run, under pid 1 once the script's shell is gone.
    let mut group_agent: HashMap<Pid, Pid> = HashMap::new();
    for pid in placed.agent.order.iter().chain(&placed.folds_into.order) {
        let at = if placed.agent.has(*pid) { *pid } else { *placed.folds_into.get(*pid).unwrap() };
        if let Some(pgid) = placed.by_pid.get(pid).and_then(|p| p.pgid) {
            group_agent.entry(pgid).or_insert(at);
        }
        if let Some(root) = root_of.get(pid) {
            group_agent.entry(*root).or_insert(at);
        }
    }
    let plumbing: Vec<(Pid, Pid)> = placed
        .procs
        .iter()
        .filter_map(|p| {
            let pgid = p.pgid?;
            if placed.agent.has(p.pid) || placed.folds_into.has(p.pid) || !is_turn_plumbing(&p.cmdline) {
                return None;
            }
            let at = *group_agent.get(&pgid)?;
            let stamped = p.owner.as_ref().and_then(|o| o.thread.as_deref());
            if stamped.is_some_and(|s| thread_ids.contains(s) && Some(s) != placed.agent.get(at).map(String::as_str)) {
                return None;
            }
            Some((p.pid, at))
        })
        .collect();
    for (pid, at) in plumbing {
        placed.folds_into.set(pid, at);
    }

    let mut nobody: HashSet<Pid> = HashSet::new();
    let pids: Vec<Pid> = placed.procs.iter().map(|p| p.pid).collect();
    for pid in &pids {
        place(*pid, &mut placed, &mut nobody, &mut HashSet::new(), input.threads, &thread_ids);
    }
    for pid in &pids {
        kind_of(*pid, &mut placed, input.listens, &bins);
    }
    placed
}

fn place(
    pid: Pid,
    placed: &mut Placement,
    nobody: &mut HashSet<Pid>,
    seen: &mut HashSet<Pid>,
    threads: &[PlaceThread],
    ids: &HashSet<&str>,
) -> Option<Whose> {
    if let Some(held) = placed.whose.get(&pid) {
        return Some(held.clone());
    }
    if nobody.contains(&pid) {
        return None;
    }
    let p = *placed.by_pid.get(&pid)?;
    let stamped = p.owner.as_ref().and_then(|o| o.thread.clone());
    let stretch = placed.agent.get(pid).or_else(|| placed.folds_into.get(pid).and_then(|at| placed.agent.get(*at))).cloned();
    let mut said = None;
    if let Some(stamped) = stamped.filter(|s| ids.contains(s.as_str())) {
        said = Some(Whose::Thread(stamped));
    } else if let Some(stretch) = stretch {
        said = Some(Whose::Thread(stretch));
    } else if placed.wsp_part(p) {
        said = Some(Whose::Wsp);
    } else if started_by_wsp(p) {
        said = Some(Whose::Loose);
    } else {
        let cg = threads.iter().find(|t| t.cgroup.as_deref().is_some_and(|c| in_cgroup(p.cgroup.as_deref(), c)));
        let slate = slate_folder(&p.cmdline).then(|| threads.iter().find(|t| p.cmdline.contains(&format!("slates/{}/", t.id))));
        if let Some(cg) = cg {
            said = Some(Whose::Thread(cg.id.clone()));
        } else if let Some(slate) = slate {
            said = Some(slate.map_or(Whose::Loose, |t| Whose::Thread(t.id.clone())));
        } else if p.pty.is_some() {
            said = Some(Whose::Loose);
        } else {
            let parent = (p.ppid != p.pid).then(|| placed.by_pid.get(&p.ppid).map(|q| q.pid)).flatten();
            if let Some(parent) = parent.filter(|q| !seen.contains(q)) {
                seen.insert(pid);
                said = place(parent, placed, nobody, seen, threads, ids);
            }
            if said.is_none() && p.owner.as_ref().is_some_and(|o| o.by == OwnedBy::Coalition) {
                said = Some(Whose::Loose);
            }
        }
    }
    match &said {
        Some(w) => {
            placed.whose.insert(pid, w.clone());
        }
        None => {
            nobody.insert(pid);
        }
    }
    said
}

fn kind_of(pid: Pid, placed: &mut Placement, listens: &dyn Fn(Pid) -> bool, bins: &HashSet<&str>) -> Kind {
    if let Some(held) = placed.kinds.get(&pid) {
        return *held;
    }
    let Some(p) = placed.by_pid.get(&pid).copied() else { return Kind::Other };
    placed.kinds.insert(pid, Kind::Other);
    let part = placed.app.get(&pid).copied();
    let parent = (p.ppid != p.pid).then(|| placed.by_pid.get(&p.ppid).copied()).flatten();
    let above = parent.map(|q| kind_of(q.pid, placed, listens, bins));
    let kind = if let Some(tab) = part {
        if tab {
            Kind::Tab
        } else {
            Kind::App
        }
    } else if placed.agent.has(pid) || placed.folds_into.has(pid) {
        Kind::Agent
    } else if (pid == placed.daemon || Some(pid) == placed.host) && p.kind.is_none() {
        Kind::Wsp
    } else if let Some(k) = p.kind.filter(|k| *k != Kind::Terminal || p.pty.is_some()) {
        k
    } else if p.pty.is_some() {
        Kind::Terminal
    } else if !matches!(placed.whose.get(&pid), Some(Whose::Thread(_))) && proc_bins(p).iter().any(|b| bins.contains(b.as_str())) {
        Kind::Agent
    } else if slate_folder(&p.cmdline) {
        Kind::Slate
    } else if test_line(&p.cmdline) {
        Kind::Test
    } else if build_line(&p.cmdline) {
        Kind::Build
    } else if listens(pid) || dev_line(&p.cmdline) {
        Kind::Dev
    } else if let Some(k @ (Kind::Dev | Kind::Test | Kind::Build)) = above {
        k
    } else if above == Some(Kind::Agent) && parent.is_some_and(|q| !placed.folds_into.has(q.pid)) && !SHELLS.contains(&p.comm.as_str()) {
        Kind::Tool
    } else {
        Kind::Other
    };
    placed.kinds.insert(pid, kind);
    kind
}

const TOOLS: [&str; 9] = ["cargo", "npm", "npx", "pnpm", "yarn", "uv", "uvx", "go", "bunx"];
const GENERIC_SCRIPTS: [&str; 4] = ["bin", "cli", "index", "main"];
const BUILD_DIRS: [&str; 8] = ["dist", "lib", "build", "out", "src", "bin", ".bin", "node_modules"];
const UV_VALUES: [&str; 14] = [
    "--from",
    "--with",
    "--with-editable",
    "--with-requirements",
    "--python",
    "-p",
    "--directory",
    "--project",
    "--index",
    "--index-url",
    "--extra-index-url",
    "--default-index",
    "--constraints",
    "--overrides",
];

/// The words after each tool that fetch a package and run it (`RUN_WORDS`).
fn run_words(tool: &str) -> &'static [&'static [&'static str]] {
    match tool {
        "uv" => &[&["tool", "uvx"], &["tool", "run"]],
        "npm" => &[&["exec"], &["x"]],
        "pnpm" => &[&["dlx"], &["exec"]],
        "yarn" => &[&["dlx"]],
        "npx" | "bunx" | "uvx" => &[&[]],
        _ => &[],
    }
}

/// The flags of each tool that take the next word as their value (`VALUE_FLAGS`).
fn value_flags(tool: &str) -> &'static [&'static str] {
    match tool {
        "uv" | "uvx" => &UV_VALUES,
        "npm" => &["-p", "--package", "-c", "--call", "-w", "--workspace"],
        "npx" => &["-p", "--package", "-c", "--call"],
        "pnpm" => &["--package", "-c", "-C", "--dir", "--filter", "-F"],
        "yarn" | "bunx" => &["-p", "--package"],
        _ => &[],
    }
}

/// `NOT_A_NAME`: an address or a version names no process.
fn not_a_name(word: &str) -> bool {
    let lower = word.to_lowercase();
    if lower.starts_with("git+") {
        return true;
    }
    if let Some(at) = lower.find("://") {
        let scheme = &lower[..at];
        let mut chars = scheme.chars();
        if chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c))
        {
            return true;
        }
    }
    let version = lower.strip_prefix('v').unwrap_or(&lower);
    !version.is_empty() && version.split('.').all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()))
}

/// `procName`: the script a runtime runs, the package a runner runs, a tool with its subcommand, or the command.
pub fn proc_name(p: &ProcEntry) -> String {
    let argv: Vec<&str> = p.cmdline.split(' ').filter(|w| !w.is_empty()).collect();
    let bin = base(argv.first().copied().unwrap_or(&p.comm)).to_owned();
    let comm = if p.comm.is_empty() { bin.clone() } else { p.comm.clone() };
    if TOOLS.contains(&comm.as_str()) {
        let valued = value_flags(&comm);
        let rest = &argv[argv.len().min(1)..];
        let words: Vec<&str> = rest
            .iter()
            .enumerate()
            .filter(|(i, w)| !w.starts_with('-') && !(*i > 0 && valued.contains(&rest[i - 1])))
            .map(|(_, w)| *w)
            .collect();
        let path = run_words(&comm).iter().find(|run| run.iter().enumerate().all(|(i, w)| words.get(i) == Some(w)));
        if let Some(pkg) = path.and_then(|run| words.get(run.len())) {
            if !not_a_name(pkg) {
                return match pkg[1..].rfind('@') {
                    Some(at) if !pkg[at + 2..].contains('/') => pkg[..at + 1].to_owned(),
                    _ => (*pkg).to_owned(),
                };
            }
        }
        return match words.first() {
            Some(sub) => format!("{comm} {sub}"),
            None => comm,
        };
    }
    if !SCRIPT_RUNTIMES.contains(&comm.as_str()) && !SCRIPT_RUNTIMES.contains(&bin.as_str()) {
        return comm;
    }
    let Some(at) = argv.iter().enumerate().position(|(i, w)| i > 0 && !w.starts_with('-')) else { return comm };
    let parts: Vec<&str> = argv[at].split('/').collect();
    let last = parts.last().copied().unwrap_or("");
    let stem = [".mjs", ".js", ".cjs", ".ts"].iter().find_map(|ext| last.strip_suffix(ext)).unwrap_or(last);
    if stem.ends_with(".py") {
        return format!("{comm} {stem}");
    }
    if !GENERIC_SCRIPTS.contains(&stem) {
        return stem.to_owned();
    }
    let pkg = parts[..parts.len() - 1].iter().rev().find(|d| !d.is_empty() && !BUILD_DIRS.contains(d));
    let sub = argv[at + 1..].iter().find(|w| !w.starts_with('-'));
    [Some(pkg.copied().unwrap_or(stem)), sub.copied()].into_iter().flatten().collect::<Vec<_>>().join(" ")
}

/// `agentByBin`: the agent a process runs by the binary its command names.
pub fn agent_by_bin(p: &ProcEntry) -> Option<&'static str> {
    let bins = proc_bins(p);
    AGENTS.iter().find(|a| bins.iter().any(|b| b == a.1)).map(|a| a.0)
}

/// `activityName`: its agent's name, the daemon, the host, else what `procName` reads off its command.
pub fn activity_name(p: &ProcEntry, daemon: Pid, host: Option<Pid>, agent: Option<&str>) -> String {
    if let Some(agent) = agent {
        return agent.to_owned();
    }
    if p.pid == daemon {
        return "Daemon".into();
    }
    if Some(p.pid) == host {
        return "Host".into();
    }
    proc_name(p)
}

/// `appName`: an app part by what the app calls it, wsp itself for its main process; a tab by the site it shows.
pub fn app_name(part: &AppPart) -> String {
    match part.kind {
        AppKind::Main => "wsp".into(),
        AppKind::Tab | AppKind::Frame => match part.origins.first() {
            None => part.kind.word().into(),
            Some(first) => {
                let site = first
                    .split_once("://")
                    .map(|(_, rest)| rest.split('/').next().unwrap_or(rest))
                    .filter(|h| !h.is_empty())
                    .unwrap_or(first);
                let more = part.origins.len() - 1;
                if more > 0 {
                    format!("{site} +{more}")
                } else {
                    site.to_owned()
                }
            }
        },
        kind => kind.word().into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(cmd: &str) -> ProcEntry {
        ProcEntry::new(1, 0, cmd.split(' ').next().unwrap(), cmd)
    }

    #[test]
    fn names_read_off_the_command_as_the_web_reads_them() {
        assert_eq!(proc_name(&run("cargo test -p x")), "cargo test");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "npx", "npx -y @playwright/mcp@0.0.41")), "@playwright/mcp");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "uv", "uv tool uvx mcp-search-console")), "mcp-search-console");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "node", "node /a/b/vite/bin/vite.js dev")), "vite");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "node", "node /x/wsp/dist/bin.js up")), "wsp up");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "python3", "python3 -m http.server")), "http.server");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "python3", "python3 /x/serve.py")), "python3 serve.py");
        assert_eq!(proc_name(&ProcEntry::new(1, 0, "sleep", "sleep 60")), "sleep");
    }

    #[test]
    fn the_lines_that_name_a_kind() {
        assert!(test_line("node node_modules/.bin/vitest run"));
        assert!(!test_line("node jester"));
        assert!(build_line("tsc -b"));
        assert!(build_line("webpack --mode production"));
        assert!(!build_line("webpack serve"));
        assert!(dev_line("python3 -m http.server 8000"));
        assert!(slate_folder("node /home/u/.wsp/slates/t1/run.js"));
        assert!(slate_folder("wsp-slates/t1/x"));
        assert!(!slate_folder("myslates/t1/x"));
        assert!(is_turn_plumbing("bash /tmp/runs/0123456789ab.sh"));
        assert!(is_turn_plumbing("tail -n +1 -f /tmp/runs/0123456789ab.in"));
        assert!(!is_turn_plumbing("bash /tmp/runs/0123456789ab.sh extra"));
    }

    #[test]
    fn a_thread_root_down_to_its_agent_is_the_agent() {
        let mut procs = vec![
            ProcEntry::new(10, 1, "bash", "bash /r/0123456789ab.sh"),
            ProcEntry::new(11, 10, "claude", "claude -p"),
            ProcEntry::new(12, 11, "node", "node /x/mcp/server.js"),
            ProcEntry::new(13, 12, "vite", "vite"),
            ProcEntry::new(20, 1, "wspd", "wspd"),
        ];
        procs[0].pgid = Some(10);
        let threads = [PlaceThread { id: "t1".into(), roots: vec![10], cgroup: None, bin: Some("claude".into()), agents: vec![] }];
        let placed =
            place_procs(PlaceInput { procs: &procs, daemon: 20, host: None, threads: &threads, listens: &|_| false, app: HashMap::new() });
        assert_eq!(placed.agent_of(11), Some("t1"));
        assert_eq!(placed.folds_into(10), Some(11));
        assert_eq!(placed.kind_of(12), Kind::Tool);
        assert_eq!(placed.kind_of(13), Kind::Dev);
        assert_eq!(placed.whose.get(&13), Some(&Whose::Thread("t1".into())));
        assert_eq!(placed.kind_of(20), Kind::Wsp);
        assert!(placed.needed(20));
        assert!(placed.endable(12));
        assert!(!placed.endable(11));
    }
}
