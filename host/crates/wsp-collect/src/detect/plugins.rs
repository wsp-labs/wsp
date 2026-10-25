// SPDX-License-Identifier: AGPL-3.0-only
//! Every agent's plugins on a computer, off the shelf each catalog agent names, read under the folder its store
//! variable names for a thread there, else under the home. Port of packages/collect/src/detect/plugins.ts at
//! a3b870a6e; the spec is spec/plugins.md.
use futures::future::join_all;
use indexmap::{IndexMap, IndexSet};
use wsp_catalog::catalog::{catalog_agents, harness_line, AgentEntry};
use wsp_catalog::plugins::{join_path, FoundPlugin, PeekGroup, Peeked, PluginIo, PluginProject, PluginShelf, PluginsFound};
use wsp_wire::types::agents_report::{AgentsProject, PluginRow};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::exit::HostError;
use wsp_words::js::js_trim_end;

use crate::detect::tools::argv;
use crate::host::{tilde, Host, RunOptions};

#[derive(Debug, Clone, PartialEq)]
pub struct AgentPlugins {
    pub agent: String,
    pub shelf: PluginShelf,
    pub found: PluginsFound,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginsRead {
    pub plugins: Vec<PluginRow>,
    pub refused: Vec<String>,
    /// Each agent's own read, which a switch acts on and the skills reader takes plugin skills from.
    pub agents: Vec<AgentPlugins>,
}

pub const PEEK_FILE_BYTES: usize = 256 * 1024;

/// The `peek` and `from` shell functions every peek line opens with, generated from plugins.ts:204-224.
pub const PEEK: &str = concat!(
    "peek() {\n",
    "  files=$1; dirs=$2; shift 2\n",
    "  for r; do\n",
    "    printf '\\036r%s\\037' \"$r\"\n",
    "    printf '%s\\n' \"$files\" | while IFS= read -r f; do [ -n \"$f\" ] && [ -f \"$r/$f\" ] && { printf '\\036f%s/%s\\037' \"$r\" \"$f\"; head -c 262144 \"$r/$f\"; }; done\n",
    "    printf '%s\\n' \"$dirs\" | while IFS=' ' read -r k d; do\n",
    "      [ -n \"$d\" ] || continue\n",
    "      if [ \"$d\" = . ]; then p=$r; else p=$r/$d; fi\n",
    "      [ -d \"$p\" ] || continue\n",
    "      printf '\\036d%s\\037' \"$p\"\n",
    "      find -L \"$p\" -mindepth 1 -maxdepth \"$k\" 2>/dev/null | while IFS= read -r x; do printf '%s\\037' \"${x#\"$p\"/}\"; done\n",
    "    done\n",
    "  done\n",
    "}\n",
    "from() {\n",
    "  k=$1; shift\n",
    "  for f; do [ -f \"$f\" ] && LC_ALL=C grep -o \"\\\"$k\\\"[[:space:]]*:[[:space:]]*\\\"[^\\\"\\\\]*\\\"\" \"$f\"; done | sed 's/^\"[^\"]*\"[[:space:]]*:[[:space:]]*\"//; s/\"$//' | sort -u\n",
    "}",
);

/// The module's view of the computer for one agent (the object `pluginIo` returns, plugins.ts:253).
pub struct HostPluginIo<'a> {
    pub host: &'a Host,
    pub agent: String,
}

/// What `peekOn` reads back (plugins.ts:249): text and lists by `join_path` of their path, roots looked at.
pub struct PeekedRecords {
    pub texts: indexmap::IndexMap<String, String>,
    pub lists: indexmap::IndexMap<String, Vec<String>>,
    pub looked: indexmap::IndexSet<String>,
}

impl Peeked for PeekedRecords {
    fn text(&self, path: &str) -> Option<String> {
        self.texts.get(&join_path(&[path])).cloned()
    }
    fn list(&self, dir: &str) -> Option<Vec<String>> {
        self.lists.get(&join_path(&[dir])).cloned()
    }
    fn looked(&self, root: &str) -> bool {
        self.looked.contains(&join_path(&[root]))
    }
}

/// The one line `peekOn` runs for these groups: PEEK, a call per group, then the end mark, joined by `\n`
/// (plugins.ts:229-235). Split out so a test can pin the line without a computer.
pub fn peek_line(groups: &[PeekGroup]) -> String {
    let mut lines = vec![PEEK.to_string()];
    for g in groups {
        let files = g.files.clone().unwrap_or_default();
        let dirs: Vec<String> = g.dirs.iter().flatten().map(|d| format!("{} {}", d.depth, d.path)).collect();
        let what = format!("{} {}", shell_quote(&files.join("\n")), shell_quote(&dirs.join("\n")));
        if let Some(roots) = g.roots.as_ref().filter(|r| !r.is_empty()) {
            let quoted: Vec<String> = roots.iter().map(|r| shell_quote(r)).collect();
            lines.push(format!("peek {what} {}", quoted.join(" ")));
        }
        if let Some(from) = &g.roots_from {
            let quoted: Vec<String> = from.files.iter().map(|f| shell_quote(f)).collect();
            lines.push(format!("from {} {} | while IFS= read -r r; do peek {what} \"$r\"; done", shell_quote(&from.key), quoted.join(" ")));
        }
    }
    lines.push(r"printf '\036END\n'".to_string());
    lines.join("\n")
}

/// What one peek printed, read back; `None` where the answer did not end with the end mark (plugins.ts:236-249).
pub fn peek_records(said: Option<&str>) -> Option<PeekedRecords> {
    let said = said?;
    if !js_trim_end(said).ends_with("\u{1e}END") {
        return None;
    }
    let mut out = PeekedRecords { texts: IndexMap::new(), lists: IndexMap::new(), looked: IndexSet::new() };
    for record in said.split('\u{1e}').skip(1) {
        let Some(cut) = record.find('\u{1f}') else { continue };
        let kind = record.chars().next();
        let kind_len = kind.map_or(0, char::len_utf8);
        let at = join_path(&[record.get(kind_len..cut).unwrap_or("")]);
        let body = &record[cut + 1..];
        match kind {
            Some('f') => {
                out.texts.insert(at, body.to_string());
            }
            Some('d') => {
                out.lists.insert(at, body.split('\u{1f}').filter(|n| !n.is_empty() && *n != "\n").map(str::to_string).collect());
            }
            Some('r') => {
                out.looked.insert(at);
            }
            _ => {}
        }
    }
    Some(out)
}

impl PluginIo for HostPluginIo<'_> {
    fn home(&self) -> &str {
        &self.host.home
    }
    fn store(&self) -> Option<&str> {
        self.host.stores.as_ref()?.get(&self.agent).map(String::as_str)
    }
    fn seed(&self) -> Option<&str> {
        self.host.seeds.as_ref()?.get(&self.agent).map(String::as_str)
    }
    /// `sh -c <peek_line>` with a 30 s timeout.
    async fn peek(&self, groups: &[PeekGroup]) -> Option<Box<dyn Peeked>> {
        let line = peek_line(groups);
        let opts = RunOptions { timeout_ms: Some(30_000), ..RunOptions::default() };
        let said = self.host.exec.run("sh", &argv(&["-c", &line]), opts).await;
        peek_records(said.as_deref()).map(|p| Box::new(p) as Box<dyn Peeked>)
    }
    /// `bash -c <harness_line(agent, line, stores)>` with a 30 s timeout.
    async fn run(&self, line: &str) -> Option<String> {
        let wrapped = harness_line(&self.agent, line, self.host.stores.as_ref(), None);
        let opts = RunOptions { timeout_ms: Some(30_000), ..RunOptions::default() };
        self.host.exec.run("bash", &argv(&["-c", &wrapped]), opts).await
    }
    /// The TS object has no `edit`: a collect read never writes.
    fn can_edit(&self) -> bool {
        false
    }
    async fn edit(&self, _file: &str, _base: &str, _change: &dyn Fn(Option<&str>) -> Result<String, HostError>) -> Result<(), HostError> {
        Err(HostError::new("a plugin read never edits a file"))
    }
}

/// The module's view of the computer, for one agent: its files, and its lines run pointed at its store.
pub fn plugin_io<'a>(host: &'a Host, agent: &str) -> HostPluginIo<'a> {
    HostPluginIo { host, agent: agent.to_string() }
}

/// The row's keys: `agent`, then the found plugin's own in its order less `skills`, with `project`, `path` and `setIn`
/// `~`-relative under `home`.
pub fn plugin_row(home: &str, agent: &str, p: &FoundPlugin) -> PluginRow {
    PluginRow {
        agent: agent.to_string(),
        id: p.id.clone(),
        name: p.name.clone(),
        marketplace: p.marketplace.clone(),
        version: p.version.clone(),
        scope: p.scope,
        on: p.on,
        missing: p.missing,
        description: p.description.clone(),
        source: p.source.clone(),
        brings: p.brings.clone(),
        project: p.project.as_ref().map(|pr| AgentsProject { id: pr.id.clone(), name: pr.name.clone(), path: tilde(home, &pr.path) }),
        path: p.path.as_deref().map(|x| tilde(home, x)),
        set_in: p.set_in.as_deref().map(|x| tilde(home, x)),
    }
}

/// `agents` `None` is the catalog's agents; only those with a plugin shelf are read.
pub async fn detect_plugins(host: &Host, agents: Option<&[&AgentEntry]>, projects: &[AgentsProject]) -> PluginsRead {
    let agents: Vec<&AgentEntry> = match agents {
        Some(a) => a.to_vec(),
        None => catalog_agents(),
    };
    let shelves: Vec<(&AgentEntry, PluginShelf)> = agents.into_iter().filter_map(|a| a.plugin_shelf.map(|s| (a, s))).collect();
    let projects: Vec<PluginProject> =
        projects.iter().map(|p| PluginProject { id: p.id.clone(), name: p.name.clone(), path: p.path.clone() }).collect();
    let projects = &projects;
    let reads: Vec<AgentPlugins> = join_all(shelves.into_iter().map(|(a, shelf)| async move {
        let found = shelf.read(&plugin_io(host, &a.id), projects).await;
        AgentPlugins { agent: a.id.clone(), shelf, found }
    }))
    .await;
    PluginsRead {
        plugins: reads.iter().flat_map(|r| r.found.plugins.iter().map(|p| plugin_row(&host.home, &r.agent, p))).collect(),
        refused: reads.iter().flat_map(|r| r.found.refused.iter().cloned()).collect(),
        agents: reads,
    }
}
