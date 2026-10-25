// SPDX-License-Identifier: AGPL-3.0-only
//! machine-context.ts:1-302 at a4033dc34. Spec: spec/context_probe.md.
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use wsp_catalog::catalog::{catalog_agents, AgentEntry};
use wsp_catalog::context::{GuestRoots, SKILL_NAME};
use wsp_catalog::signin_rows::SECRETS_FILE_SH;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::{is_js_whitespace, js_trim, js_trim_end, to_number, utf16};

use crate::golden_seam::{df_kb_cmd, parse_versions, DfColumn, GoldenBaseTool, ImportResult, BASE_VERSION_LINES, TOOLS_PATH};
use crate::machine_facts::SHELL_READ;

/// machine-context.ts:26. A catalog agent whose `context` is set.
pub type ContextAgent = &'static AgentEntry;

/// machine-context.ts:28. The catalog's agents with a context module, in catalog order.
pub fn context_agents() -> Vec<ContextAgent> {
    catalog_agents().into_iter().filter(|a| a.context.is_some()).collect()
}

/// machine-context.ts:34. `{ etc: "/etc", home: "/root" }`.
pub fn guest_roots() -> GuestRoots {
    GuestRoots { etc: "/etc".to_string(), home: "/root".to_string() }
}

fn roots_or_default(roots: Option<&GuestRoots>) -> GuestRoots {
    roots.cloned().unwrap_or_else(guest_roots)
}

/// machine-context.ts:49. Mirrors OPEN_SHIM_PATH in wsp-wire.
pub const BROWSER_SHIM_PATH: &str = r#"/usr/local/bin/wsp-open"#;

/// machine-context.ts:43. `<etc>/wsp/machine-context.md`; `None` is [`guest_roots`].
pub fn context_path(roots: Option<&GuestRoots>) -> String {
    format!("{}/wsp/machine-context.md", roots_or_default(roots).etc)
}

/// machine-context.ts:44. `<etc>/wsp/skills/wsp-machine/SKILL.md`.
pub fn skill_path(roots: Option<&GuestRoots>) -> String {
    format!("{}/wsp/skills/{SKILL_NAME}/SKILL.md", roots_or_default(roots).etc)
}

/// machine-context.ts:45. `<etc>/wsp/machine-context.json`.
pub fn facts_path(roots: &GuestRoots) -> String {
    format!("{}/wsp/machine-context.json", roots.etc)
}

/// machine-context.ts:53. Serialized `id`, `label`, `note` in that order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingItem {
    pub id: String,
    /// The TS holds whatever object the guest's file carried, so an item can lack a label or a note: absent reads as
    /// empty, and an empty one is left out when written (a recorded item never has an empty one).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A file left on the person's computer; serialized `path`, `note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileNote {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// machine-context.ts:61. Serialized `tools`, `agents`, `files` in that order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildFacts {
    pub tools: Vec<MissingItem>,
    pub agents: Vec<MissingItem>,
    pub files: Vec<FileNote>,
}

/// One outcome row of a run, whichever list it came from: a tools row, a base row or an agent row.
struct Row {
    id: String,
    label: String,
    outcome: String,
    note: Option<String>,
}

/// machine-context.ts:72 `fold`: a kept item whose id a row names is replaced in place by that id's missing rows; the
/// missing rows whose id the list did not hold come after.
fn fold(list: &[MissingItem], rows: &[Row]) -> Vec<MissingItem> {
    let missing: Vec<MissingItem> = rows
        .iter()
        .filter(|r| r.outcome != "installed")
        .map(|r| MissingItem { id: r.id.clone(), label: r.label.clone(), note: r.note.clone().unwrap_or_else(|| r.outcome.clone()) })
        .collect();
    let mut out: Vec<MissingItem> = Vec::new();
    for m in list {
        if rows.iter().any(|r| r.id == m.id) {
            out.extend(missing.iter().filter(|r| r.id == m.id).cloned());
        } else {
            out.push(m.clone());
        }
    }
    out.extend(missing.iter().filter(|r| !list.iter().any(|m| m.id == r.id)).cloned());
    out
}

/// machine-context.ts:72. A run's outcomes folded over what the guest records.
pub fn merge_facts(prior: Option<&BuildFacts>, result: Option<&ImportResult>) -> BuildFacts {
    let mut facts = prior.cloned().unwrap_or_default();
    let Some(result) = result else {
        return facts;
    };
    let tool_rows: Vec<Row> = result
        .base
        .iter()
        .flatten()
        .chain(result.tools.iter())
        .map(|r| Row { id: r.id.clone(), label: r.label.clone(), outcome: r.outcome.clone(), note: r.note.clone() })
        .collect();
    facts.tools = fold(&facts.tools, &tool_rows);
    let agent_rows: Vec<Row> = result
        .agents
        .iter()
        .map(|a| Row { id: a.id.clone(), label: a.name.clone(), outcome: a.outcome.clone(), note: a.note.clone() })
        .collect();
    facts.agents = fold(&facts.agents, &agent_rows);
    for r in result.retired.iter().flatten() {
        facts.tools.retain(|m| m.id != r.id);
        facts.agents.retain(|m| m.id != r.id);
    }
    if let Some(files) = &result.files {
        facts.files.retain(|f| !files.skipped.iter().any(|s| s.path == f.path));
        facts.files.extend(files.skipped.iter().map(|s| FileNote { path: s.path.clone(), note: s.note.clone() }));
    }
    facts
}

/// The items a facts list holds as the TS holds them: an element that is an object is kept with the fields it has, as
/// `machine-context.ts:93` does not check its elements (spec/context_probe.md, open point 1).
fn items<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Vec<T> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| serde_json::from_value(x.clone()).ok()).collect())
        .unwrap_or_default()
}

/// machine-context.ts:93. Base64 of UTF-8 JSON; `None` where it does not decode or parse.
pub fn parse_facts(b64: &str) -> Option<BuildFacts> {
    let text = String::from_utf8_lossy(&crate::node_base64::node_base64_decode(b64)).into_owned();
    let v: Value = serde_json::from_str(&text).ok()?;
    if v.is_null() {
        return None;
    }
    Some(BuildFacts { tools: items(&v, "tools"), agents: items(&v, "agents"), files: items(&v, "files") })
}

/// machine-context.ts:122.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasesUnread {
    /// wsp opens no login shell here.
    Shared,
    /// The login shell had not answered by its bound.
    Late,
}

/// machine-context.ts:106. A disk read in bytes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Disk {
    pub size_bytes: f64,
    pub free_bytes: f64,
}

/// An alias the login shell defines whose command is not on the machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub name: String,
    pub word: String,
}

/// machine-context.ts:104. Sets keep the order lines first named them.
#[derive(Debug, Clone, PartialEq)]
pub struct ContextProbe {
    pub kernel: Option<String>,
    pub disk: Option<Disk>,
    pub overlay: bool,
    pub has: IndexSet<String>,
    pub versions: Vec<GoldenBaseTool>,
    pub agents: Vec<String>,
    pub secrets: Vec<String>,
    pub shell: String,
    pub aliases: Vec<Alias>,
    pub aliases_unread: Option<AliasesUnread>,
    pub conflicts: IndexSet<String>,
    pub facts: Option<BuildFacts>,
}

/// machine-context.ts:132.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProbeShells {
    #[default]
    Login,
    None,
}

/// machine-context.ts:139. Seconds the login shell listing the aliases may take.
pub const ALIAS_BOUND_S: u32 = 8;
/// The word the probe prints in place of its alias lines where it opened no shell to read them.
const ALIASES_UNREAD: &str = "unread";
/// The word it prints where the shell it opened was stopped at its bound.
const ALIASES_LATE: &str = "late";

/// machine-context.ts:161 `ALIAS_PROBES.zsh`.
pub const ALIAS_PROBE_ZSH: &str = r#"for k in ${(k)aliases}; do
  b=$aliases[$k]; w=
  for t in ${(z)b}; do
    case $t in [A-Za-z_]*=*|sudo|command|builtin|exec|env|nohup|noglob|nocorrect|time) continue;; esac
    w=${t#\\}; w=${w#\'}; w=${w#\"}; break
  done
  case $w in ''|*[!A-Za-z0-9_.+-]*|[!A-Za-z0-9_]*) continue;; esac
  case $b in *"$w()"*|*"$w ()"*|*"function $w"*) continue;; esac
  whence -w -- "$w" >/dev/null 2>&1 || echo "ALIAS $k $w"
done"#;

/// machine-context.ts:161 `ALIAS_PROBES.bash`.
pub const ALIAS_PROBE_BASH: &str = r#"set -f
alias | while IFS= read -r line; do
  case $line in "alias "*=*) ;; *) continue;; esac
  rest=${line#alias }; k=${rest%%=*}; b=${rest#*=}; b=${b#\'}; b=${b%\'}; w=
  for t in $b; do
    case $t in [A-Za-z_]*=*|sudo|command|builtin|exec|env|nohup|noglob|nocorrect|time) continue;; esac
    w=${t#\\}; w=${w#\'}; w=${w#\"}; break
  done
  case $w in ''|*[!A-Za-z0-9_.+-]*|[!A-Za-z0-9_]*) continue;; esac
  case $b in *"$w()"*|*"$w ()"*|*"function $w"*) continue;; esac
  type -t "$w" >/dev/null 2>&1 || echo "ALIAS $k $w"
done"#;

/// machine-context.ts:161 `ALIAS_PROBES.fish`.
pub const ALIAS_PROBE_FISH: &str = r#"for line in (alias)
  set -l rest (string replace -r '^alias ' '' -- $line)
  set -l name (string split -m1 ' ' -- $rest)[1]
  set -l def (string split -m1 ' ' -- $rest)[2]
  if string match -q '*=*' -- $name
    set def (string split -m1 '=' -- $rest)[2]
    set name (string split -m1 '=' -- $rest)[1]
  end
  set -l w
  for t in (string split ' ' -- (string unescape -- $def))
    if string match -qr '^[A-Za-z_][A-Za-z0-9_]*=' -- $t; continue; end
    if contains -- $t sudo command builtin exec env nohup noglob nocorrect time; continue; end
    set w (string trim -l -c \\ -- $t); break
  end
  string match -qr '^[A-Za-z0-9_][A-Za-z0-9_.+-]*$' -- $w; or continue
  type -q -- $w; or echo "ALIAS $name $w"
end"#;

/// machine-context.ts:161. The probe for a shell by name, `None` for any other name.
pub fn alias_probe(shell: &str) -> Option<&'static str> {
    match shell {
        "zsh" => Some(ALIAS_PROBE_ZSH),
        "bash" => Some(ALIAS_PROBE_BASH),
        "fish" => Some(ALIAS_PROBE_FISH),
        _ => None,
    }
}

/// machine-context.ts:197.
pub fn bounded_command(seconds: u32, cmd: &str) -> String {
    format!(
        "set -m; {cmd} & p=$!; ( sleep {seconds}; kill -KILL -- -$p ) >/dev/null 2>&1 & k=$!; set +m; wait $p; r=$?; kill -KILL -- -$p -$k 2>/dev/null; [ $r -ne 137 ]"
    )
}

/// machine-context.ts:206. `None` is `/proc/filesystems`.
pub fn overlay_probe(filesystems: Option<&str>) -> String {
    let fs = filesystems.unwrap_or("/proc/filesystems");
    format!(
        "if {{ grep -qw overlay {fs} || {{ modprobe overlay && grep -qw overlay {fs}; }}; }} >/dev/null 2>&1; then echo OVERLAY yes; else echo OVERLAY no; fi"
    )
}

/// machine-context.ts:202 `aliasScan`: the login shell listing its aliases under the bound, and the word that says so
/// where the bound stopped it.
fn alias_scan(sh: &str) -> String {
    let probe = alias_probe(sh).unwrap_or_default();
    let listing = bounded_command(ALIAS_BOUND_S, &format!("{sh} -lic {} </dev/null 2>/dev/null", shell_quote(probe)));
    format!("{listing} || echo \"ALIASES {ALIASES_LATE}\"")
}

/// machine-context.ts:212. `None`s are [`guest_roots`], golden-import's `TOOLS_PATH` and `ProbeShells::Login`.
pub fn probe_command(roots: Option<&GuestRoots>, path: Option<&str>, shells: Option<ProbeShells>) -> String {
    let roots = roots_or_default(roots);
    let (h, e) = (&roots.home, &roots.etc);
    let secrets = &SECRETS_FILE_SH["/etc".len()..];
    let mut lines = vec![
        format!("export PATH={}:$PATH", path.unwrap_or(TOOLS_PATH)),
        "echo WSP_CTX".to_string(),
        "echo \"KERNEL $(uname -r 2>/dev/null)\"".to_string(),
        format!("echo \"DISK $({{ {}; }} 2>/dev/null)\"", df_kb_cmd(&[DfColumn::Size, DfColumn::Free], Some(h))),
        overlay_probe(None),
        "for b in docker podman tmux fish brew; do if command -v \"$b\" >/dev/null 2>&1; then echo \"HAS $b\"; fi; done".to_string(),
        format!("if [ -f {e}/profile.d/wsp-golden.sh ]; then echo \"HAS golden-path\"; fi"),
        format!("if [ -x {BROWSER_SHIM_PATH} ]; then echo \"HAS wsp-open\"; fi"),
        BASE_VERSION_LINES.to_string(),
    ];
    for a in context_agents() {
        lines.push(format!("if command -v {} >/dev/null 2>&1; then echo \"AGENT {}\"; fi", shell_quote(&a.bin), a.id));
    }
    lines.push(format!("sed -n 's/^export \\([A-Za-z_][A-Za-z0-9_]*\\)=.*/SECRET \\1/p' {e}{secrets} 2>/dev/null"));
    lines.push(SHELL_READ.to_string());
    lines.push("echo \"SHELL $shell\"".to_string());
    match shells.unwrap_or_default() {
        ProbeShells::None => lines.push(format!("echo \"ALIASES {ALIASES_UNREAD}\"")),
        ProbeShells::Login => {
            lines.push("case $shell in".to_string());
            lines.push(format!("  zsh) {} ;;", alias_scan("zsh")));
            lines.push(format!("  fish) {} ;;", alias_scan("fish")));
            lines.push(format!("  *) {} ;;", alias_scan("bash")));
            lines.push("esac".to_string());
        }
    }
    for a in context_agents() {
        if let Some(cond) = a.context.and_then(|c| c.conflict(&roots)) {
            lines.push(format!("if {cond}; then echo \"CONFLICT {}\"; fi", a.id));
        }
    }
    let facts = facts_path(&roots);
    lines.push(format!("if [ -f {facts} ]; then echo \"FACTS $(base64 < {facts} | tr -d '\\n')\"; fi"));
    lines.push("echo WSP_CTX_END".to_string());
    lines.join("\n")
}

fn is_agent(s: &str) -> bool {
    context_agents().iter().any(|a| a.id == s)
}

/// `^[A-Za-z_][A-Za-z0-9_]*$`.
fn is_secret_name(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The DISK line's reading: `size free` in kB, both finite and size above 0.
fn disk_of(rest: &str) -> Option<Disk> {
    let words: Vec<f64> = rest.split(is_js_whitespace).filter(|w| !w.is_empty()).map(to_number).collect();
    let (size, free) = match words.as_slice() {
        [size, free, ..] => (*size, *free),
        _ => return None,
    };
    (size.is_finite() && free.is_finite() && size > 0.0).then_some(Disk { size_bytes: size * 1024.0, free_bytes: free * 1024.0 })
}

/// The probe's lines between its markers; nothing when the markers are missing.
pub fn parse_probe(stdout: &str) -> Option<ContextProbe> {
    let lines: Vec<&str> = stdout.split('\n').map(js_trim_end).collect();
    let start = lines.iter().position(|l| *l == "WSP_CTX")?;
    let end = lines.iter().position(|l| *l == "WSP_CTX_END")?;
    if end < start {
        return None;
    }
    let body = &lines[start + 1..end];
    let mut probe = ContextProbe {
        kernel: None,
        disk: None,
        overlay: false,
        has: IndexSet::new(),
        versions: parse_versions(&body.join("\n")),
        agents: Vec::new(),
        secrets: Vec::new(),
        shell: "bash".to_string(),
        aliases: Vec::new(),
        aliases_unread: None,
        conflicts: IndexSet::new(),
        facts: None,
    };
    for line in body {
        let (key, rest) = match line.find(' ') {
            Some(sp) => (&line[..sp], js_trim(&line[sp + 1..])),
            None => (*line, ""),
        };
        match key {
            "KERNEL" if !rest.is_empty() => probe.kernel = Some(rest.to_string()),
            "DISK" => probe.disk = disk_of(rest).or(probe.disk),
            "OVERLAY" => probe.overlay = rest == "yes",
            "HAS" if !rest.is_empty() => {
                probe.has.insert(rest.to_string());
            }
            "AGENT" if is_agent(rest) && !probe.agents.iter().any(|a| a == rest) => probe.agents.push(rest.to_string()),
            "SECRET" if is_secret_name(rest) && !probe.secrets.iter().any(|s| s == rest) => probe.secrets.push(rest.to_string()),
            "SHELL" if !rest.is_empty() => probe.shell = rest.to_string(),
            "ALIAS" => {
                if let Some(at) = rest.find(' ').filter(|&at| at > 0) {
                    probe.aliases.push(Alias { name: rest[..at].to_string(), word: js_trim(&rest[at + 1..]).to_string() });
                }
            }
            "ALIASES" if rest == ALIASES_UNREAD => probe.aliases_unread = Some(AliasesUnread::Shared),
            "ALIASES" if rest == ALIASES_LATE => probe.aliases_unread = Some(AliasesUnread::Late),
            "CONFLICT" if is_agent(rest) => {
                probe.conflicts.insert(rest.to_string());
            }
            "FACTS" => {
                if let Some(facts) = parse_facts(rest) {
                    probe.facts = Some(facts);
                }
            }
            _ => {}
        }
    }
    probe.aliases.sort_by(|a, b| utf16::cmp(&a.name, &b.name));
    Some(probe)
}
