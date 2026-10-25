// SPDX-License-Identifier: AGPL-3.0-only
//! machine-context.ts:1-302 at a4033dc34. Spec: spec/context_probe.md.
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::context::GuestRoots;

use crate::golden_seam::{GoldenBaseTool, ImportResult};

/// machine-context.ts:26. A catalog agent whose `context` is set.
pub type ContextAgent = &'static AgentEntry;

/// machine-context.ts:28. The catalog's agents with a context module, in catalog order.
pub fn context_agents() -> Vec<ContextAgent> {
    todo!()
}

/// machine-context.ts:34. `{ etc: "/etc", home: "/root" }`.
pub fn guest_roots() -> GuestRoots {
    todo!()
}

/// machine-context.ts:49. Mirrors OPEN_SHIM_PATH in wsp-wire.
pub const BROWSER_SHIM_PATH: &str = r#"/usr/local/bin/wsp-open"#;

/// machine-context.ts:43. `<etc>/wsp/machine-context.md`; `None` is [`guest_roots`].
pub fn context_path(roots: Option<&GuestRoots>) -> String {
    todo!()
}

/// machine-context.ts:44. `<etc>/wsp/skills/wsp-machine/SKILL.md`.
pub fn skill_path(roots: Option<&GuestRoots>) -> String {
    todo!()
}

/// machine-context.ts:45. `<etc>/wsp/machine-context.json`.
pub fn facts_path(roots: &GuestRoots) -> String {
    todo!()
}

/// machine-context.ts:53. Serialized `id`, `label`, `note` in that order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingItem {
    pub id: String,
    pub label: String,
    pub note: String,
}

/// A file left on the person's computer; serialized `path`, `note`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileNote {
    pub path: String,
    pub note: String,
}

/// machine-context.ts:61. Serialized `tools`, `agents`, `files` in that order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildFacts {
    pub tools: Vec<MissingItem>,
    pub agents: Vec<MissingItem>,
    pub files: Vec<FileNote>,
}

/// machine-context.ts:72. A run's outcomes folded over what the guest records.
pub fn merge_facts(prior: Option<&BuildFacts>, result: Option<&ImportResult>) -> BuildFacts {
    todo!()
}

/// machine-context.ts:93. Base64 of UTF-8 JSON; `None` where it does not decode or parse.
pub fn parse_facts(b64: &str) -> Option<BuildFacts> {
    todo!()
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
    todo!()
}

/// machine-context.ts:197.
pub fn bounded_command(seconds: u32, cmd: &str) -> String {
    todo!()
}

/// machine-context.ts:206. `None` is `/proc/filesystems`.
pub fn overlay_probe(filesystems: Option<&str>) -> String {
    todo!()
}

/// machine-context.ts:212. `None`s are [`guest_roots`], golden-import's `TOOLS_PATH` and `ProbeShells::Login`.
pub fn probe_command(roots: Option<&GuestRoots>, path: Option<&str>, shells: Option<ProbeShells>) -> String {
    todo!()
}

/// machine-context.ts:247. `None` without both markers in order.
pub fn parse_probe(stdout: &str) -> Option<ContextProbe> {
    todo!()
}
