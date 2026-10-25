// SPDX-License-Identifier: AGPL-3.0-only
//! Every skill on a computer, off the folders each catalog agent loads skills from: one row per folder name with every
//! folder it lives in, whether that folder is a link, and the description off its SKILL.md's frontmatter. One command
//! reads every folder. Port of packages/collect/src/detect/skills.ts at a3b870a6e; the spec is spec/skills.md.
use std::sync::LazyLock;

use futures::future::join_all;
use indexmap::map::Entry;
use indexmap::{IndexMap, IndexSet};
use regex::Regex;
use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_catalog::plugins::in_store;
use wsp_wire::types::agents_report::{AgentsProject, SkillPath, SkillRow, SkillScope};
use wsp_words::js::{collate, js_trim, js_trim_end, utf16, JS_WS};

use crate::detect::plugins::AgentPlugins;
use crate::detect::tools::argv;
use crate::host::{tilde, Host, RunOptions};

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

static KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&KEY.replace(r"\s", &format!("[{JS_WS}]")).replace("(.*)", r"([^\n\r\x{2028}\x{2029}]*)")).expect("KEY compiles")
});
static BLOCK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[>|][-+]?$").expect("BLOCK compiles"));
static CONTINUED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"^[{JS_WS}]+[^{JS_WS}]")).expect("CONTINUED compiles"));
static SINGLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^'((?:[^']|'')*)'").expect("SINGLE compiles"));
static DOUBLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^"((?:[^"\\]|\\[^\n\r\x{2028}\x{2029}])*)""#).expect("DOUBLE compiles"));
static ESCAPED_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"\\(["\\])"#).expect("ESCAPED compiles"));
static COMMENT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"([{JS_WS}]+|^)#[^\n\r\x{{2028}}\x{{2029}}]*$")).expect("COMMENT compiles"));

/// A one-line YAML value as a string: a quoted one is what its quotes hold, a plain one ends where a comment starts.
fn scalar(v: &str) -> String {
    let t = js_trim(v);
    if let Some(m) = SINGLE_RE.captures(t) {
        return m[1].replace("''", "'");
    }
    if let Some(m) = DOUBLE_RE.captures(t) {
        return ESCAPED_RE.replace_all(&m[1], "$1").into_owned();
    }
    COMMENT_RE.replace(t, "").into_owned()
}

/// Node's `path.posix` normalize of the segments: `.` and `..` resolved, repeated slashes gone, and a leading `..`
/// kept only where `allow_above_root`.
fn normalize_string(path: &str, allow_above_root: bool) -> String {
    let p = path.as_bytes();
    let mut res: Vec<u8> = Vec::new();
    let mut last_segment_length: usize = 0;
    let mut last_slash: isize = -1;
    let mut dots: i32 = 0;
    let mut code: u8 = 0;
    for i in 0..=p.len() {
        if i < p.len() {
            code = p[i];
        } else if code == b'/' {
            break;
        } else {
            code = b'/';
        }
        let at = i as isize;
        if code == b'/' {
            if last_slash == at - 1 || dots == 1 {
            } else if dots == 2 {
                if res.len() < 2 || last_segment_length != 2 || !res.ends_with(b"..") {
                    if res.len() > 2 {
                        match res.iter().rposition(|&b| b == b'/') {
                            None => {
                                res.clear();
                                last_segment_length = 0;
                            }
                            Some(k) => {
                                res.truncate(k);
                                let last = res.iter().rposition(|&b| b == b'/').map_or(-1, |x| x as isize);
                                last_segment_length = (res.len() as isize - 1 - last) as usize;
                            }
                        }
                        last_slash = at;
                        dots = 0;
                        continue;
                    } else if !res.is_empty() {
                        res.clear();
                        last_segment_length = 0;
                        last_slash = at;
                        dots = 0;
                        continue;
                    }
                }
                if allow_above_root {
                    if res.is_empty() {
                        res.extend_from_slice(b"..");
                    } else {
                        res.extend_from_slice(b"/..");
                    }
                    last_segment_length = 2;
                }
            } else {
                let start = (last_slash + 1) as usize;
                if !res.is_empty() {
                    res.push(b'/');
                }
                res.extend_from_slice(&p[start..i]);
                last_segment_length = i - start;
            }
            last_slash = at;
            dots = 0;
        } else if code == b'.' && dots != -1 {
            dots += 1;
        } else {
            dots = -1;
        }
    }
    String::from_utf8_lossy(&res).into_owned()
}

/// Node's `path.posix.normalize`: a trailing slash is kept, an empty result is `/`, `./` or `.`.
fn posix_normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let absolute = path.starts_with('/');
    let trailing = path.ends_with('/');
    let mut out = normalize_string(path, !absolute);
    if out.is_empty() {
        return if absolute { "/" } else if trailing { "./" } else { "." }.to_string();
    }
    if trailing {
        out.push('/');
    }
    if absolute {
        format!("/{out}")
    } else {
        out
    }
}

/// Node's `path.posix.join` over the non-empty parts.
fn posix_join(parts: &[&str]) -> String {
    let joined: Vec<&str> = parts.iter().copied().filter(|p| !p.is_empty()).collect();
    if joined.is_empty() {
        return ".".to_string();
    }
    posix_normalize(&joined.join("/"))
}

/// Node's `path.posix.resolve(base, rel)` for an absolute `base`: no trailing slash, `/` for the root.
fn posix_resolve(base: &str, rel: &str) -> String {
    let joined = if rel.starts_with('/') { rel.to_string() } else { format!("{base}/{rel}") };
    format!("/{}", normalize_string(&joined, false))
}

fn posix_dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) => "/",
        Some(i) => &path[..i],
        None => ".",
    }
}

fn scope_word(scope: SkillScope) -> &'static str {
    match scope {
        SkillScope::User => "user",
        SkillScope::Project => "project",
        SkillScope::Plugin => "plugin",
    }
}

/// The project's skills folders that link out of the project, each with that link and where it points. `None` when
/// the answer does not come back whole.
async fn linked_out(host: &Host, project: &str, dirs: &[String]) -> Option<IndexMap<String, (String, String)>> {
    let mut args = argv(&["-c", LINKED_SCRIPT, "sh", project]);
    args.extend(dirs.iter().cloned());
    let said = host.exec.run("sh", &args, RunOptions { timeout_ms: Some(20_000), ..RunOptions::default() }).await?;
    if !js_trim_end(&said).ends_with(END) {
        return None;
    }
    let mut out = IndexMap::new();
    for line in said.split('\n') {
        let mut cols = line.split('\t');
        let dir = cols.next().unwrap_or("");
        let link = cols.next().unwrap_or("");
        let to = cols.next().unwrap_or("");
        if dirs.iter().any(|d| d == dir) {
            out.insert(dir.to_string(), (link.to_string(), to.to_string()));
        }
    }
    Some(out)
}

fn skipped_line(home: &str, dir: &str, at: Option<&(String, String)>) -> String {
    let folder = tilde(home, dir);
    match at {
        None => format!("skills: {folder} was not read, since whether it links out of the repo could not be told"),
        Some((link, to)) => {
            let through = if link == dir { String::new() } else { format!(" through {}", tilde(home, link)) };
            format!("skills: {folder} links out of the repo{through}, to {to}, so its skills are not read")
        }
    }
}

/// Adds each folder not yet listed; a folder already listed keeps its place and takes `agent` only when it is that
/// agent's own and has none.
fn add_roots(out: &mut IndexMap<String, SkillRootAt>, dirs: Vec<(String, bool)>, scope: SkillScope, agent: &str, project: Option<&AgentsProject>) {
    for (dir, own) in dirs {
        match out.entry(dir.clone()) {
            Entry::Vacant(v) => {
                v.insert(SkillRootAt {
                    dir,
                    scope,
                    agent: own.then(|| agent.to_string()),
                    project: project.cloned(),
                    skipped: None,
                    plugin: None,
                });
            }
            Entry::Occupied(mut o) => {
                let at = o.get_mut();
                if own && at.agent.is_none() {
                    at.agent = Some(agent.to_string());
                }
            }
        }
    }
}

/// A `SKILL.md` frontmatter's `name` and `description`, the first non-empty of each, read as the TS reads them.
pub fn skill_frontmatter(text: &str) -> SkillFront {
    let mut out = SkillFront::default();
    let mut names = 0u32;
    let lines: Vec<&str> = text.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let Some(m) = KEY_RE.captures(lines[i]) else {
            i += 1;
            continue;
        };
        let key = m.get(1).or_else(|| m.get(2)).or_else(|| m.get(3)).map_or("", |g| g.as_str());
        if key == "name" {
            names += 1;
        }
        let first = js_trim(m.get(4).map_or("", |g| g.as_str())).to_string();
        let block = BLOCK_RE.is_match(&first);
        let mut more: Vec<String> = Vec::new();
        while i + 1 < lines.len() && CONTINUED_RE.is_match(lines[i + 1]) {
            i += 1;
            more.push(js_trim(lines[i]).to_string());
        }
        let value = if block {
            more.join(" ")
        } else {
            std::iter::once(scalar(&first)).chain(more).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
        };
        let slot = if key == "name" { &mut out.name } else { &mut out.description };
        if !value.is_empty() && slot.is_none() {
            *slot = Some(value);
        }
        i += 1;
    }
    if names > 1 {
        out.names = Some(names);
    }
    out
}

/// Reads at most SKILL_HEAD_BYTES UTF-16 units of the text (rule 8.2).
pub fn skill_md_frontmatter(text: &str) -> SkillFront {
    let head = utf16::cut(text, SKILL_HEAD_BYTES);
    let pieces: Vec<&str> = head.split('\n').collect();
    let last = pieces.len().saturating_sub(1);
    let lines: Vec<&str> = pieces.iter().enumerate().map(|(i, l)| if i < last { l.strip_suffix('\r').unwrap_or(l) } else { l }).collect();
    if lines.first() != Some(&"---") {
        return SkillFront::default();
    }
    let end = lines.iter().skip(1).position(|l| *l == "---").map(|p| p + 1);
    let body = match end {
        Some(e) => &lines[1..e],
        None => &lines[1..],
    };
    skill_frontmatter(&body.join("\n"))
}

/// `agents` `None` is the catalog's agents. Each project's link check is one `sh` run; the projects run at once and
/// their roots land in project order.
pub async fn skill_roots(
    host: &Host,
    agents: Option<&[&AgentEntry]>,
    projects: &[AgentsProject],
    plugins: &[AgentPlugins],
) -> Vec<SkillRootAt> {
    let agents: Vec<&AgentEntry> = match agents {
        Some(a) => a.to_vec(),
        None => catalog_agents(),
    };
    let mut out: IndexMap<String, SkillRootAt> = IndexMap::new();
    for a in &agents {
        let store = host.stores.as_ref().and_then(|s| s.get(&a.id)).map(String::as_str);
        let dirs: Vec<(String, bool)> = a.skill_roots.user.iter().enumerate().map(|(i, r)| (in_store(&a.state_home, &r.dir, &host.home, store), i == 0)).collect();
        add_roots(&mut out, dirs, SkillScope::User, &a.id, None);
    }
    let checks = join_all(projects.iter().map(|project| {
        let agents = &agents;
        async move {
            let mut dirs: Vec<String> = Vec::new();
            for a in agents.iter() {
                for r in &a.skill_roots.project {
                    let d = posix_join(&[&project.path, &r.dir]);
                    if !dirs.contains(&d) {
                        dirs.push(d);
                    }
                }
            }
            let linked = linked_out(host, &project.path, &dirs).await;
            (dirs, linked)
        }
    }))
    .await;
    for (project, (dirs, linked)) in projects.iter().zip(checks) {
        for a in &agents {
            let roots: Vec<(String, bool)> = a.skill_roots.project.iter().enumerate().map(|(i, r)| (posix_join(&[&project.path, &r.dir]), i == 0)).collect();
            add_roots(&mut out, roots, SkillScope::Project, &a.id, Some(project));
        }
        for dir in &dirs {
            if linked.as_ref().is_none_or(|m| m.contains_key(dir)) {
                if let Some(at) = out.get_mut(dir) {
                    at.skipped = Some(skipped_line(&host.home, dir, linked.as_ref().and_then(|m| m.get(dir))));
                }
            }
        }
    }
    for p in plugins {
        for found in &p.found.plugins {
            let (Some(skills), None) = (&found.skills, &found.missing) else { continue };
            for dir in &skills.dirs {
                out.entry(dir.clone()).or_insert_with(|| SkillRootAt {
                    dir: dir.clone(),
                    scope: SkillScope::Plugin,
                    agent: Some(p.agent.clone()),
                    project: None,
                    skipped: None,
                    plugin: Some(SkillPlugin { id: found.id.clone(), prefix: skills.prefix.clone(), on: found.on }),
                });
            }
        }
    }
    out.into_values().collect()
}

pub async fn detect_skills(host: &Host, all: &[SkillRootAt]) -> SkillsRead {
    let outside: Vec<String> = all.iter().filter_map(|r| r.skipped.clone()).collect();
    let roots: Vec<&SkillRootAt> = all.iter().filter(|r| r.skipped.is_none()).collect();
    if roots.is_empty() {
        return SkillsRead { skills: Vec::new(), refused: outside };
    }
    let mut args = argv(&["-c", SCRIPT, "sh"]);
    args.extend(roots.iter().map(|r| r.dir.clone()));
    let Some(said) = host.exec.run("sh", &args, RunOptions { timeout_ms: Some(20_000), ..RunOptions::default() }).await else {
        let mut refused = vec!["skills: the folders could not be read".to_string()];
        refused.extend(outside);
        return SkillsRead { skills: Vec::new(), refused };
    };
    let mut refused = Vec::new();
    if !js_trim_end(&said).ends_with(END) {
        refused.push("skills: the answer was cut short, so the list is not whole".to_string());
    }
    refused.extend(outside);

    let roots_by_dir: IndexMap<&str, &SkillRootAt> = roots.iter().map(|r| (r.dir.as_str(), *r)).collect();
    let mut found: Vec<(&SkillRootAt, &str, &str, bool, &str)> = Vec::new();
    for record in said.split('\u{1e}').skip(1) {
        let f: Vec<&str> = record.split('\u{1f}').collect();
        if f.len() < 5 {
            continue;
        }
        let Some(root) = roots_by_dir.get(f[0]) else { continue };
        let dir = f[1];
        let Some(rest) = dir.strip_prefix(format!("{}/", root.dir).as_str()) else { continue };
        if rest.split('/').any(|seg| seg.starts_with('.')) {
            continue;
        }
        found.push((*root, dir, f[2], f[3] == "1", f[4]));
    }
    let dirs: IndexSet<(String, String)> = found.iter().map(|(root, dir, ..)| (root.dir.clone(), dir.to_string())).collect();

    let mut rows: IndexMap<String, SkillRow> = IndexMap::new();
    for (root, dir, link, off, head) in &found {
        let rel = &dir[root.dir.len() + 1..];
        let parts: Vec<&str> = rel.split('/').collect();
        let nested = (0..parts.len() - 1).any(|i| dirs.contains(&(root.dir.clone(), format!("{}/{}", root.dir, parts[..=i].join("/")))));
        if nested {
            continue;
        }
        let meta = skill_frontmatter(head);
        let plugin = root.plugin.as_ref();
        let base = dir.rsplit('/').next().unwrap_or(dir);
        let name = match plugin {
            None => base.to_string(),
            Some(p) => format!("{}:{base}", p.prefix),
        };
        let owner = root.project.as_ref().map(|p| p.id.as_str()).or(plugin.map(|p| p.id.as_str())).unwrap_or("");
        let key = format!("{}\0{owner}\0{name}", scope_word(root.scope));
        let path = SkillPath {
            path: tilde(&host.home, dir),
            agent: root.agent.clone(),
            link_to: (!link.is_empty()).then(|| tilde(&host.home, &posix_resolve(posix_dirname(dir), link))),
            off: (*off || plugin.is_some_and(|p| !p.on)).then(Default::default),
        };
        if let Some(row) = rows.get_mut(&key) {
            if !row.paths.iter().any(|p| p.path == path.path) {
                row.paths.push(path);
            }
        } else {
            let project = root.project.as_ref().map(|p| AgentsProject { id: p.id.clone(), name: p.name.clone(), path: tilde(&host.home, &p.path) });
            rows.insert(
                key,
                SkillRow {
                    name,
                    description: meta.description,
                    paths: vec![path],
                    scope: root.scope,
                    project,
                    plugin: plugin.map(|p| p.id.clone()),
                },
            );
        }
    }
    let mut skills: Vec<SkillRow> = rows.into_values().collect();
    skills.sort_by(|a, b| {
        collate(&a.name, &b.name)
            .then_with(|| collate(scope_word(a.scope), scope_word(b.scope)))
            .then_with(|| collate(a.project.as_ref().map_or("", |p| p.name.as_str()), b.project.as_ref().map_or("", |p| p.name.as_str())))
    });
    SkillsRead { skills, refused }
}
