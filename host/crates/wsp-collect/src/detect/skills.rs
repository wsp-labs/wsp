// SPDX-License-Identifier: AGPL-3.0-only
//! Every skill on a computer, off the folders each catalog agent loads skills from: one row per folder name with every
//! folder it lives in, whether that folder is a link, and the description off its SKILL.md's frontmatter. One command
//! reads every folder. Port of packages/collect/src/detect/skills.ts at a3b870a6e; the spec is spec/skills.md.
use wsp_catalog::catalog::AgentEntry;
use wsp_wire::types::agents_report::{AgentsProject, SkillRow, SkillScope};

use crate::detect::plugins::AgentPlugins;
use crate::host::Host;

/// The plugin a plugin's folder comes with (skills.ts:24).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillPlugin {
    pub id: String,
    pub prefix: String,
    pub on: bool,
}

// skills.ts:14
// /** One folder to read skills from, absolute: whose own folder it is, where it is one agent's, and what kind. */
// export interface SkillRootAt {
//   dir: string;
//   scope: SkillScope;
//   agent?: string;
//   /** The project a project folder is in, its path absolute. */
//   project?: AgentsProject;
//   /** Why nothing under this folder is read, as the list's refusal line says it: a project's folder that links out of
//    * the repo, or one the read could not tell. */
//   skipped?: string;
//   /** The plugin a plugin's folder comes with: its id, the name its skills are announced under, and whether it is on. */
//   plugin?: { id: string; prefix: string; on: boolean };
// }
#[derive(Debug, Clone, PartialEq)]
pub struct SkillRootAt {
    pub dir: String,
    pub scope: SkillScope,
    pub agent: Option<String>,
    /// The project a project folder is in, its path absolute.
    pub project: Option<AgentsProject>,
    /// Why nothing under this folder is read, as the list's refusal line says it.
    pub skipped: Option<String>,
    pub plugin: Option<SkillPlugin>,
}

// skills.ts:27
// export interface SkillsRead {
//   skills: SkillRow[];
//   refused: string[];
// }
#[derive(Debug, Clone, PartialEq)]
pub struct SkillsRead {
    pub skills: Vec<SkillRow>,
    pub refused: Vec<String>,
}

// skills.ts:117
// /** What a SKILL.md's frontmatter says; `names` counts the name lines when there is more than one. */
// export interface SkillFront {
//   name?: string;
//   description?: string;
//   names?: number;
// }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillFront {
    pub name: Option<String>,
    pub description: Option<String>,
    /// The count of name lines, only when more than one.
    pub names: Option<u32>,
}

/// The constants below are generated from skills.ts by evaluating its own definitions; byte for byte the TS text.
pub const SKILL_HEAD_BYTES: usize = 4096;
pub const END: &str = "\u{1e}END";
pub const FRONTMATTERS: &str = concat!(
    "function q(s,  o, i) { o = \"\"; while ((i = index(s, \"\\047\")) > 0) { o = o substr(s, 1, i - 1) \"\\047\\\\\\047\\047\"; s = substr(s, i + 1) } return \"\\047\" o s \"\\047\" }\n",
    "/^r/ { T[++t] = substr($0, 2); next }\n",
    "{\n",
    "  k = substr($0, 2, 1); o = substr($0, 3, 1); f = substr($0, 4)\n",
    "  if (f in S) next\n",
    "  S[f] = 1\n",
    "  d = f; if (match(f, \"/[^/]*$\")) d = substr(f, 1, RSTART - 1)\n",
    "  for (i = 1; i <= t; i++) {\n",
    "    if (index(f, T[i] \"/\") != 1) continue\n",
    "    rel = substr(f, length(T[i]) + 2); up = gsub(\"/\", \"/\", rel)\n",
    "    if (up < 1 || up > 2) continue\n",
    "    n++; R[n] = T[i]; K[n] = k; O[n] = o; F[n] = f; D[n] = d\n",
    "  }\n",
    "  if (k == \"1\" && !(d in L)) { L[d] = \"\"; cmd = cmd \" \" q(d) }\n",
    "}\n",
    "END {\n",
    "  if (cmd != \"\") {\n",
    "    cmd = \"QUOTING_STYLE=literal ls -ld --\" cmd \" 2>/dev/null\"\n",
    "    while ((cmd | getline line) > 0) {\n",
    "      # Assumes an absolute name, a target with no newline, and no link named another link plus \" -> \" and a target start.\n",
    "      i = index(line, \" /\")\n",
    "      if (i == 0) continue\n",
    "      rest = substr(line, i + 1); best = \"\"\n",
    "      for (d in L) if (length(d) > length(best) && substr(rest, 1, length(d) + 4) == d \" -> \") best = d\n",
    "      if (best != \"\") L[best] = substr(rest, length(best) + 5)\n",
    "    }\n",
    "    close(cmd)\n",
    "  }\n",
    "  for (j = 1; j <= n; j++) {\n",
    "    printf \"\\036%s\\037%s\\037%s\\037%s\\037\", R[j], D[j], (K[j] == \"1\" ? L[D[j]] : \"\"), (O[j] == \"1\" ? \"1\" : \"\")\n",
    "    f = F[j]; k = 0; b = 0\n",
    "    while (f != \"\" && b < cap && (getline line < f) > 0) {\n",
    "      k++\n",
    "      if (b + length(line) > cap) line = substr(line, 1, cap - b)\n",
    "      b += length(line) + 1\n",
    "      sub(/\\r$/, \"\", line)\n",
    "      if (k == 1) { if (line != \"---\") break; continue }\n",
    "      if (line == \"---\") break\n",
    "      print line\n",
    "    }\n",
    "    if (f != \"\") close(f)\n",
    "  }\n",
    "}",
);
pub const SCRIPT: &str = concat!(
    "{\n",
    "  printf 'r%s\\n' \"$@\"\n",
    "  find -L \"$@\" -mindepth 2 -maxdepth 3 \\( -name SKILL.md -o -name SKILL.md.off \\) -type f 2>/dev/null | while IFS= read -r f; do\n",
    "    [ -f \"$f\" ] || continue\n",
    "    d=${f%/*}\n",
    "    o=0; case $f in *.off) [ -f \"$d/SKILL.md\" ] && continue; o=1;; esac\n",
    "    l=0; [ -L \"$d\" ] && l=1\n",
    "    printf 'f%s%s%s\\n' \"$l\" \"$o\" \"$f\"\n",
    "  done\n",
    "} | LC_ALL=C awk -v cap=4096 'function q(s,  o, i) { o = \"\"; while ((i = index(s, \"\\047\")) > 0) { o = o substr(s, 1, i - 1) \"\\047\\\\\\047\\047\"; s = substr(s, i + 1) } return \"\\047\" o s \"\\047\" }\n",
    "/^r/ { T[++t] = substr($0, 2); next }\n",
    "{\n",
    "  k = substr($0, 2, 1); o = substr($0, 3, 1); f = substr($0, 4)\n",
    "  if (f in S) next\n",
    "  S[f] = 1\n",
    "  d = f; if (match(f, \"/[^/]*$\")) d = substr(f, 1, RSTART - 1)\n",
    "  for (i = 1; i <= t; i++) {\n",
    "    if (index(f, T[i] \"/\") != 1) continue\n",
    "    rel = substr(f, length(T[i]) + 2); up = gsub(\"/\", \"/\", rel)\n",
    "    if (up < 1 || up > 2) continue\n",
    "    n++; R[n] = T[i]; K[n] = k; O[n] = o; F[n] = f; D[n] = d\n",
    "  }\n",
    "  if (k == \"1\" && !(d in L)) { L[d] = \"\"; cmd = cmd \" \" q(d) }\n",
    "}\n",
    "END {\n",
    "  if (cmd != \"\") {\n",
    "    cmd = \"QUOTING_STYLE=literal ls -ld --\" cmd \" 2>/dev/null\"\n",
    "    while ((cmd | getline line) > 0) {\n",
    "      # Assumes an absolute name, a target with no newline, and no link named another link plus \" -> \" and a target start.\n",
    "      i = index(line, \" /\")\n",
    "      if (i == 0) continue\n",
    "      rest = substr(line, i + 1); best = \"\"\n",
    "      for (d in L) if (length(d) > length(best) && substr(rest, 1, length(d) + 4) == d \" -> \") best = d\n",
    "      if (best != \"\") L[best] = substr(rest, length(best) + 5)\n",
    "    }\n",
    "    close(cmd)\n",
    "  }\n",
    "  for (j = 1; j <= n; j++) {\n",
    "    printf \"\\036%s\\037%s\\037%s\\037%s\\037\", R[j], D[j], (K[j] == \"1\" ? L[D[j]] : \"\"), (O[j] == \"1\" ? \"1\" : \"\")\n",
    "    f = F[j]; k = 0; b = 0\n",
    "    while (f != \"\" && b < cap && (getline line < f) > 0) {\n",
    "      k++\n",
    "      if (b + length(line) > cap) line = substr(line, 1, cap - b)\n",
    "      b += length(line) + 1\n",
    "      sub(/\\r$/, \"\", line)\n",
    "      if (k == 1) { if (line != \"---\") break; continue }\n",
    "      if (line == \"---\") break\n",
    "      print line\n",
    "    }\n",
    "    if (f != \"\") close(f)\n",
    "  }\n",
    "}' && printf '\\036END\\n'",
);
pub const SKILL_LINK_ABOVE: &str = concat!(
    "linked() {\n",
    "  p=${1%/*}\n",
    "  while [ -n \"$p\" ] && [ \"$p\" != \"$2\" ] && [ \"$p\" != / ]; do\n",
    "    if [ -L \"$p\" ]; then\n",
    "      if [ -n \"$3\" ] && r=$(realpath \"$p\" 2>/dev/null); then case $r/ in \"$3\"/*) p=${p%/*}; continue;; esac; fi\n",
    "      printf '%s\\t%s\\n' \"$p\" \"$(readlink \"$p\")\"\n",
    "      return 0\n",
    "    fi\n",
    "    p=${p%/*}\n",
    "  done\n",
    "  return 1\n",
    "}",
);
pub const LINKED_SCRIPT: &str = concat!(
    "linked() {\n",
    "  p=${1%/*}\n",
    "  while [ -n \"$p\" ] && [ \"$p\" != \"$2\" ] && [ \"$p\" != / ]; do\n",
    "    if [ -L \"$p\" ]; then\n",
    "      if [ -n \"$3\" ] && r=$(realpath \"$p\" 2>/dev/null); then case $r/ in \"$3\"/*) p=${p%/*}; continue;; esac; fi\n",
    "      printf '%s\\t%s\\n' \"$p\" \"$(readlink \"$p\")\"\n",
    "      return 0\n",
    "    fi\n",
    "    p=${p%/*}\n",
    "  done\n",
    "  return 1\n",
    "}\n",
    "p=$1; shift\n",
    "[ -d \"$p\" ] || { printf '\\036END\\n'; exit 0; }\n",
    "b=$(realpath \"$p\") || exit 1\n",
    "for r in \"$@\"; do\n",
    "  l=$(linked \"$r/.\" \"$p\" \"$b\") && printf '%s\\t%s\\n' \"$r\" \"$l\"\n",
    "done\n",
    "printf '\\036END\\n'",
);

/// A top-level `name` or `description` key, as a JavaScript regex source (skills.ts:114).
pub const KEY: &str = r#"^(?:(name|description)|"(name|description)"|'(name|description)')\s*:(?:\s+(.*))?$"#;

// Private helpers the port keeps (names in snake case):
// skills.ts:104
// /** A one-line YAML value as a string: a quoted one is what its quotes hold, a plain one ends where a comment starts. */
// const scalar = (v: string): string => {
//   const t = v.trim();
//   const single = /^'((?:[^']|'')*)'/.exec(t);
//   if (single !== null) return single[1]!.replaceAll("''", "'");
//   const double = /^"((?:[^"\\]|\\.)*)"/.exec(t);
//   if (double !== null) return double[1]!.replace(/\\(["\\])/g, "$1");
//   return t.replace(/(^|\s+)#.*$/, "");
// };
// skills.ts:189
// /** The project's skills folders that sit under a link out of the project, each with that link and where it points: a
//  * repo that links one out would hand its skills acts the person's own folders. Nothing when the answer does not come
//  * back whole, which leaves every one of them unread. */
// async function linkedOut(host: Host, project: string, dirs: readonly string[]): Promise<Map<string, { link: string; to: string }> | undefined> {
//   const said = await host.exec.run("sh", ["-c", LINKED_SCRIPT, "sh", project, ...dirs], { timeoutMs: 20_000 });
//   if (said === undefined || !said.trimEnd().endsWith(END)) return undefined;
//   const out = new Map<string, { link: string; to: string }>();
//   for (const line of said.split("\n")) {
//     const [dir = "", link = "", to = ""] = line.split("\t");
//     if (dirs.includes(dir)) out.set(dir, { link, to });
//   }
//   return out;
// }
// skills.ts:202
// /** What the list says of a project's skills folder it does not read: where it links out of the repo, or that the
//  * read could not tell. */
// function skippedLine(host: Host, dir: string, at: { link: string; to: string } | undefined): string {
//   const folder = tilde(host.home, dir);
//   if (at === undefined) return `skills: ${folder} was not read, since whether it links out of the repo could not be told`;
//   return `skills: ${folder} links out of the repo${at.link === dir ? "" : ` through ${tilde(host.home, at.link)}`}, to ${at.to}, so its skills are not read`;
// }
// skills.ts:236
// /** The folders each plugin read says its skills sit in, each with its plugin: none of a plugin whose folder is gone. */
// function pluginSkillRoots(plugins: readonly AgentPlugins[]): SkillRootAt[] {
//   return plugins.flatMap(({ agent, found }) =>
//     found.plugins.flatMap(p => (p.skills === undefined || p.missing !== undefined ? [] : p.skills.dirs.map(dir => ({ dir, scope: "plugin" as const, agent, plugin: { id: p.id, prefix: p.skills!.prefix, on: p.on } })))),
//   );
// }

// skills.ts:125
// /** `name` and `description` off a SKILL.md's frontmatter lines, the first of each: a plain value, a quoted one, or a
//  * folded or literal block, read as one line. */
// export function skillFrontmatter(text: string): SkillFront {
//   const out: SkillFront = {};
//   let names = 0;
//   const lines = text.split("\n");
//   for (let i = 0; i < lines.length; i++) {
//     const m = KEY.exec(lines[i]!);
//     if (m === null) continue;
//     const key = (m[1] ?? m[2] ?? m[3]) as "name" | "description";
//     if (key === "name") names++;
//     let value = (m[4] ?? "").trim();
//     const block = /^[>|][-+]?$/.test(value);
//     const more: string[] = [];
//     while (i + 1 < lines.length && /^\s+\S/.test(lines[i + 1]!)) more.push(lines[++i]!.trim());
//     value = block ? more.join(" ") : [scalar(value), ...more].filter(w => w !== "").join(" ");
//     if (value !== "" && out[key] === undefined) out[key] = value;
//   }
//   if (names > 1) out.names = names;
//   return out;
// }
pub fn skill_frontmatter(text: &str) -> SkillFront {
    todo!()
}

// skills.ts:147
// /** `name` and `description` off a whole SKILL.md: the lines between its opening `---` and the next, as the reader's
//  * script takes them. */
// export function skillMdFrontmatter(text: string): SkillFront {
//   const lines = text.slice(0, SKILL_HEAD_BYTES).split(/\r?\n/);
//   if (lines[0] !== "---") return {};
//   const end = lines.indexOf("---", 1);
//   return skillFrontmatter(lines.slice(1, end === -1 ? undefined : end).join("\n"));
// }
/// Reads at most SKILL_HEAD_BYTES UTF-16 units of the text (rule 8.2).
pub fn skill_md_frontmatter(text: &str) -> SkillFront {
    todo!()
}

// skills.ts:211
// /** The folders every catalog agent loads skills from on that computer, absolute, each once, an agent's own under the
//  * folder its store variable names there: an agent's own folder carries that agent, a folder several read carries none.
//  * Each project adds the folders the agents read inside it, and each plugin read the folders its skills sit in. */
// export async function skillRoots(host: Host, o: { agents?: readonly AgentEntry[]; projects?: readonly AgentsProject[]; plugins?: readonly AgentPlugins[] } = {}): Promise<SkillRootAt[]> {
//   const agents = o.agents ?? CATALOG_AGENTS;
//   const out = new Map<string, SkillRootAt>();
//   const add = (dirs: { dir: string; own: boolean }[], scope: SkillScope, agent: string, project?: AgentsProject): void => {
//     for (const { dir, own } of dirs) {
//       const at = out.get(dir);
//       if (at === undefined) out.set(dir, { dir, scope, ...(own ? { agent } : {}), ...(project !== undefined ? { project } : {}) });
//       else if (own && at.agent === undefined) out.set(dir, { ...at, agent });
//     }
//   };
//   for (const a of agents) add(a.skillRoots.user.map((r, i) => ({ dir: inStore(a, r.dir, { home: host.home, store: host.stores?.[a.id] }), own: i === 0 })), "user", a.id);
//   await Promise.all(
//     (o.projects ?? []).map(async project => {
//       const at = (r: { dir: string }): string => posix.join(project.path, r.dir);
//       const dirs = [...new Set(agents.flatMap(a => a.skillRoots.project.map(at)))];
//       const linked = await linkedOut(host, project.path, dirs);
//       for (const a of agents) add(a.skillRoots.project.map((r, i) => ({ dir: at(r), own: i === 0 })), "project", a.id, project);
//       for (const dir of dirs) if (linked === undefined || linked.has(dir)) out.set(dir, { ...out.get(dir)!, skipped: skippedLine(host, dir, linked?.get(dir)) });
//     }),
//   );
//   for (const root of pluginSkillRoots(o.plugins ?? [])) if (!out.has(root.dir)) out.set(root.dir, root);
//   return [...out.values()];
// }
/// `agents` `None` is the catalog's agents. Each project's link check is one `sh` run; the projects run at once and
/// their roots land in project order.
pub async fn skill_roots(host: &Host, agents: Option<&[&AgentEntry]>, projects: &[AgentsProject], plugins: &[AgentPlugins]) -> Vec<SkillRootAt> {
    todo!()
}

// skills.ts:245
// /** Every skill under the roots, one row per folder name and kind with every folder it lives in. A folder whose name starts
//  * with a dot, one without a SKILL.md, and a skill inside another skill's folder are not skills. An answer cut short
//  * is a refusal naming it, never a list that silently stops. */
// export async function detectSkills(host: Host, all: readonly SkillRootAt[]): Promise<SkillsRead> {
//   const outside = all.flatMap(r => (r.skipped === undefined ? [] : [r.skipped]));
//   const roots = all.filter(r => r.skipped === undefined);
//   if (roots.length === 0) return { skills: [], refused: outside };
//   const said = await host.exec.run("sh", ["-c", SCRIPT, "sh", ...roots.map(r => r.dir)], { timeoutMs: 20_000 });
//   if (said === undefined) return { skills: [], refused: ["skills: the folders could not be read", ...outside] };
//   const refused = [...(said.trimEnd().endsWith(END) ? [] : ["skills: the answer was cut short, so the list is not whole"]), ...outside];
//   const rootOf = new Map(roots.map(r => [r.dir, r]));
//   const found: { root: SkillRootAt; dir: string; link: string; off: boolean; head: string }[] = [];
//   for (const record of said.split("\x1e").slice(1)) {
//     const [rootDir, dir, link, off, head] = record.split("\x1f");
//     const root = rootDir === undefined ? undefined : rootOf.get(rootDir);
//     if (root === undefined || dir === undefined || head === undefined) continue;
//     const rel = dir.slice(root.dir.length + 1);
//     if (!dir.startsWith(`${root.dir}/`) || rel.split("/").some(seg => seg.startsWith("."))) continue;
//     found.push({ root, dir, link: link ?? "", off: off === "1", head });
//   }
//   const dirs = new Set(found.map(f => `${f.root.dir}\0${f.dir}`));
//   const rows = new Map<string, SkillRow>();
//   for (const f of found) {
//     // A SKILL.md under a folder that is itself a skill belongs to that skill (its examples, its templates).
//     const parts = f.dir.slice(f.root.dir.length + 1).split("/");
//     if (parts.slice(1).some((_, i) => dirs.has(`${f.root.dir}\0${f.root.dir}/${parts.slice(0, i + 1).join("/")}`))) continue;
//     const meta = skillFrontmatter(f.head);
//     // Keyed by folder, which is what an agent loads a skill by; the frontmatter's name is the file's own claim. A
//     // plugin's skill goes by the name the agent announces it under, its plugin's first.
//     const plugin = f.root.plugin;
//     const name = plugin === undefined ? posix.basename(f.dir) : `${plugin.prefix}:${posix.basename(f.dir)}`;
//     const key = `${f.root.scope}\0${f.root.project?.id ?? plugin?.id ?? ""}\0${name}`;
//     const path: SkillPath = {
//       path: tilde(host.home, f.dir),
//       ...(f.root.agent !== undefined ? { agent: f.root.agent } : {}),
//       ...(f.link !== "" ? { linkTo: tilde(host.home, posix.resolve(posix.dirname(f.dir), f.link)) } : {}),
//       ...(f.off || plugin?.on === false ? { off: true as const } : {}),
//     };
//     const row = rows.get(key);
//     const project = f.root.project === undefined ? {} : { project: { ...f.root.project, path: tilde(host.home, f.root.project.path) } };
//     if (row === undefined) rows.set(key, { name, ...(meta.description !== undefined ? { description: meta.description } : {}), paths: [path], scope: f.root.scope, ...project, ...(plugin !== undefined ? { plugin: plugin.id } : {}) });
//     else if (!row.paths.some(p => p.path === path.path)) row.paths.push(path);
//   }
//   return { skills: [...rows.values()].sort((a, b) => a.name.localeCompare(b.name) || a.scope.localeCompare(b.scope) || (a.project?.name ?? "").localeCompare(b.project?.name ?? "")), refused };
// }
pub async fn detect_skills(host: &Host, all: &[SkillRootAt]) -> SkillsRead {
    todo!()
}
