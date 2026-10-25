// SPDX-License-Identifier: AGPL-3.0-only
//! Every agent's plugins on a computer, off the shelf each catalog agent names, read under the folder its store
//! variable names for a thread there, else under the home. Port of packages/collect/src/detect/plugins.ts at
//! a3b870a6e; the spec is spec/plugins.md.
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::pending::HostError;
use wsp_catalog::plugins::{FoundPlugin, PeekGroup, Peeked, PluginIo, PluginShelf, PluginsFound};
use wsp_wire::types::agents_report::{AgentsProject, PluginRow};

use crate::host::Host;

// plugins.ts:11
// /** One agent's plugins as its module found them, beside the rows the report carries. */
// export interface AgentPlugins {
//   agent: string;
//   shelf: PluginShelf;
//   found: PluginsFound;
// }
#[derive(Debug, Clone, PartialEq)]
pub struct AgentPlugins {
    pub agent: String,
    pub shelf: PluginShelf,
    pub found: PluginsFound,
}

// plugins.ts:17
// export interface PluginsRead {
//   plugins: PluginRow[];
//   refused: string[];
//   /** Each agent's own read, which a switch acts on and the skills reader takes plugin skills from. */
//   agents: AgentPlugins[];
// }
#[derive(Debug, Clone, PartialEq)]
pub struct PluginsRead {
    pub plugins: Vec<PluginRow>,
    pub refused: Vec<String>,
    /// Each agent's own read, which a switch acts on and the skills reader takes plugin skills from.
    pub agents: Vec<AgentPlugins>,
}

// plugins.ts:25
// /** How much of one file a peek reads. */
// const PEEK_FILE_BYTES = 256 * 1024;
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

// Private helper the port keeps (name in snake case):
// plugins.ts:54
// /** One peek of every group as one line on the computer, and what it printed read back by path; nothing where the
//  * answer did not come back whole. */
// async function peekOn(run: (line: string) => Promise<string | undefined>, groups: readonly PeekGroup[]): Promise<Peeked | undefined> {
//   const calls = groups.flatMap(g => {
//     const what = `${shellQuote((g.files ?? []).join("\n"))} ${shellQuote((g.dirs ?? []).map(d => `${d.depth} ${d.path}`).join("\n"))}`;
//     const named = (g.roots ?? []).length === 0 ? [] : [`peek ${what} ${g.roots!.map(shellQuote).join(" ")}`];
//     const from = g.rootsFrom === undefined ? [] : [`from ${shellQuote(g.rootsFrom.key)} ${g.rootsFrom.files.map(shellQuote).join(" ")} | while IFS= read -r r; do peek ${what} "$r"; done`];
//     return [...named, ...from];
//   });
//   const said = await run([PEEK, ...calls, String.raw`printf '\036END\n'`].join("\n"));
//   if (said === undefined || !said.trimEnd().endsWith("\x1eEND")) return undefined;
//   const texts = new Map<string, string>();
//   const lists = new Map<string, string[]>();
//   const looked = new Set<string>();
//   for (const record of said.split("\x1e").slice(1)) {
//     const cut = record.indexOf("\x1f");
//     if (cut < 0) continue;
//     const at = joinPath(record.slice(1, cut));
//     const body = record.slice(cut + 1);
//     if (record[0] === "f") texts.set(at, body);
//     else if (record[0] === "d") lists.set(at, body.split("\x1f").filter(n => n !== "" && n !== "\n"));
//     else if (record[0] === "r") looked.add(at);
//   }
//   return { text: path => texts.get(joinPath(path)), list: dir => lists.get(joinPath(dir)), looked: root => looked.has(joinPath(root)) };
// }

/// What `peekOn` reads back (plugins.ts:249): text and lists by `join_path` of their path, roots looked at.
pub struct PeekedRecords {
    pub texts: indexmap::IndexMap<String, String>,
    pub lists: indexmap::IndexMap<String, Vec<String>>,
    pub looked: indexmap::IndexSet<String>,
}

impl Peeked for PeekedRecords {
    fn text(&self, path: &str) -> Option<String> {
        todo!()
    }
    fn list(&self, dir: &str) -> Option<Vec<String>> {
        todo!()
    }
    fn looked(&self, root: &str) -> bool {
        todo!()
    }
}

/// The one line `peekOn` runs for these groups: PEEK, a call per group, then the end mark, joined by `\n`
/// (plugins.ts:229-235). Split out so a test can pin the line without a computer.
pub fn peek_line(groups: &[PeekGroup]) -> String {
    todo!()
}

/// What one peek printed, read back; `None` where the answer did not end with the end mark (plugins.ts:236-249).
pub fn peek_records(said: Option<&str>) -> Option<PeekedRecords> {
    todo!()
}

impl PluginIo for HostPluginIo<'_> {
    fn home(&self) -> &str {
        todo!()
    }
    fn store(&self) -> Option<&str> {
        todo!()
    }
    fn seed(&self) -> Option<&str> {
        todo!()
    }
    /// `sh -c <peek_line>` with a 30 s timeout.
    async fn peek(&self, groups: &[PeekGroup]) -> Option<Box<dyn Peeked>> {
        todo!()
    }
    /// `bash -c <harness_line(agent, line, stores)>` with a 30 s timeout.
    async fn run(&self, line: &str) -> Option<String> {
        todo!()
    }
    /// The TS object has no `edit`: a collect read never writes.
    fn can_edit(&self) -> bool {
        todo!()
    }
    async fn edit(&self, file: &str, base: &str, change: &dyn Fn(Option<&str>) -> Result<String, HostError>) -> Result<(), HostError> {
        todo!()
    }
}

// plugins.ts:79
// /** The module's view of the computer, for one agent: its files, and its lines run pointed at its store. */
// export function pluginIo(host: Host, agent: string): PluginIo {
//   const store = host.stores?.[agent];
//   const seed = host.seeds?.[agent];
//   return {
//     home: host.home,
//     ...(store !== undefined ? { store } : {}),
//     ...(seed !== undefined ? { seed } : {}),
//     peek: groups => peekOn(line => host.exec.run("sh", ["-c", line], { timeoutMs: 30_000 }), groups),
//     // bash: Codex's app server script reads its answers with read -u.
//     run: line => host.exec.run("bash", ["-c", harnessLine(agent, line, { stores: host.stores })], { timeoutMs: 30_000 }),
//   };
// }
pub fn plugin_io<'a>(host: &'a Host, agent: &str) -> HostPluginIo<'a> {
    todo!()
}

// plugins.ts:93
// /** A found plugin as the report's row, its paths `~`-relative. */
// export function pluginRow(host: Pick<Host, "home">, agent: string, p: FoundPlugin): PluginRow {
//   const { skills: _skills, project, path, setIn, ...rest } = p;
//   return {
//     agent,
//     ...rest,
//     ...(project !== undefined ? { project: { ...project, path: tilde(host.home, project.path) } } : {}),
//     ...(path !== undefined ? { path: tilde(host.home, path) } : {}),
//     ...(setIn !== undefined ? { setIn: tilde(host.home, setIn) } : {}),
//   };
// }
/// The row's keys: `agent`, then the found plugin's own in its order less `skills`, with `project`, `path` and `setIn`
/// `~`-relative under `home`.
pub fn plugin_row(home: &str, agent: &str, p: &FoundPlugin) -> PluginRow {
    todo!()
}

// plugins.ts:105
// /** Every agent's plugins, side by side: one agent's read that fails leaves the others' rows standing. */
// export async function detectPlugins(host: Host, o: { agents?: readonly AgentEntry[]; projects?: readonly AgentsProject[] } = {}): Promise<PluginsRead> {
//   const agents = (o.agents ?? CATALOG_AGENTS).filter(a => a.pluginShelf !== undefined);
//   const projects: PluginProject[] = (o.projects ?? []).map(p => ({ id: p.id, name: p.name, path: p.path }));
//   const reads = await Promise.all(
//     agents.map(async (a): Promise<AgentPlugins> => {
//       const shelf = a.pluginShelf!;
//       const found = await shelf.read(pluginIo(host, a.id), projects).catch((e: unknown) => ({ plugins: [], refused: [`plugins: ${a.name}'s plugins were not read: ${e instanceof Error ? e.message : String(e)}`] }));
//       return { agent: a.id, shelf, found };
//     }),
//   );
//   return {
//     plugins: reads.flatMap(r => r.found.plugins.map(p => pluginRow(host, r.agent, p))),
//     refused: reads.flatMap(r => r.found.refused),
//     agents: reads,
//   };
// }
/// `agents` `None` is the catalog's agents; only those with a plugin shelf are read.
pub async fn detect_plugins(host: &Host, agents: Option<&[&AgentEntry]>, projects: &[AgentsProject]) -> PluginsRead {
    todo!()
}
