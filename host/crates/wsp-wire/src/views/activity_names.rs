// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/views/activity-names.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/views/activity-names.ts:89 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const ACTIVITY_KIND_WORDS: &[(&str, &str)] = &[
    ("agent", "Agent"),
    ("tool", "Tool server"),
    ("dev", "Dev server"),
    ("test", "Test run"),
    ("build", "Build"),
    ("slate", "Slate run"),
    ("terminal", "Terminal"),
    ("tab", "Browser tab"),
    ("app", "App"),
    ("wsp", "wsp"),
    ("other", "Other"),
];

/// TS: packages/protocol/src/views/activity-names.ts:104 at a3b870a6e.
/// A Set in the TS, in insertion order.
pub const UNTAGGED_KINDS: &[&str] = &["agent", "other", "wsp", "app"];

/// TS: packages/protocol/src/views/activity-names.ts:107 at a3b870a6e.
/// A Record in the TS: its keys in the TS's order.
pub const ACTIVITY_GROUP_WORDS: &[(&str, &str)] = &[
    ("wsp", "wsp"),
    ("wspNote", "The app and what runs it"),
    ("wspNoteBox", "What runs wsp here"),
    ("loose", "No thread"),
    ("looseNote", "Started with no thread wsp knows of"),
];

// ---- Hand-written below: the functions of the unit.

use crate::views::activity::{proc_bins, SCRIPT_RUNTIMES};
use wsp_frames::ProcEntry;

const TOOLS: &[&str] = &["cargo", "npm", "npx", "pnpm", "yarn", "uv", "uvx", "go", "bunx"];
const UV_VALUES: &[&str] = &[
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
const GENERIC_SCRIPTS: &[&str] = &["bin", "cli", "index", "main"];
const BUILD_DIRS: &[&str] = &["dist", "lib", "build", "out", "src", "bin", ".bin", "node_modules"];

/// The words after a tool that fetch a package and run it, which the package then names (`RUN_WORDS`).
fn run_words(comm: &str) -> &'static [&'static [&'static str]] {
    match comm {
        "uv" => &[&["tool", "uvx"], &["tool", "run"]],
        "npm" => &[&["exec"], &["x"]],
        "pnpm" => &[&["dlx"], &["exec"]],
        "yarn" => &[&["dlx"]],
        "npx" | "bunx" | "uvx" => &[&[]],
        _ => &[],
    }
}

/// The flags of a tool that take the next word as their value, which names nothing (`VALUE_FLAGS`).
fn value_flags(comm: &str) -> &'static [&'static str] {
    match comm {
        "uv" | "uvx" => UV_VALUES,
        "npm" => &["-p", "--package", "-c", "--call", "-w", "--workspace"],
        "npx" => &["-p", "--package", "-c", "--call"],
        "pnpm" => &["--package", "-c", "-C", "--dir", "--filter", "-F"],
        "yarn" | "bunx" => &["-p", "--package"],
        _ => &[],
    }
}

/// `NOT_A_NAME`: an address (`scheme://`, `git+`) or a version, which names no process.
fn not_a_name(word: &str) -> bool {
    let bytes = word.as_bytes();
    let scheme = word.find(':').is_some_and(|colon| {
        colon >= 1
            && bytes[0].is_ascii_alphabetic()
            && bytes[1..colon].iter().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
            && bytes[colon + 1..].starts_with(b"//")
    });
    let git = word.len() >= 4 && word[..4].eq_ignore_ascii_case("git+");
    let version = {
        let digits = word.strip_prefix(['v', 'V']).unwrap_or(word);
        digits.split('.').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
    };
    scheme || git || version
}

/// `pkg.replace(/(?<=.)@[^@/]*$/, "")`: a trailing `@version` goes, a leading `@scope` stays.
fn strip_version(pkg: &str) -> &str {
    match pkg.rfind('@') {
        Some(at) if at > 0 && !pkg[at..].contains('/') => &pkg[..at],
        _ => pkg,
    }
}

/// `.replace(/\.(m?js|cjs|ts)$/, "")`.
fn strip_script_ext(base: &str) -> &str {
    [".js", ".mjs", ".cjs", ".ts"].iter().find_map(|ext| base.strip_suffix(ext)).unwrap_or(base)
}

// views/activity-names.ts:39 at a3b870a6e
// export function procName(p: Pick<ProcEntry, "comm" | "cmdline">): string {
//   const argv = p.cmdline.split(" ").filter(w => w !== "");
//   const bin = (argv[0] ?? p.comm).split("/").at(-1) ?? p.comm;
//   const comm = p.comm !== "" ? p.comm : bin;
//   if (TOOLS.has(comm)) {
//     const valued = VALUE_FLAGS[comm];
//     const words = argv.slice(1).filter((w, i, all) => !w.startsWith("-") && !(i > 0 && valued?.has(all[i - 1]!) === true));
//     const path = RUN_WORDS[comm]?.find(run => run.every((w, i) => words[i] === w));
//     const pkg = path === undefined ? undefined : words[path.length];
//     if (pkg !== undefined && !NOT_A_NAME.test(pkg)) return pkg.replace(/(?<=.)@[^@/]*$/, "");
//     const sub = words[0];
//     return sub === undefined ? comm : `${comm} ${sub}`;
//   }
//   if (!SCRIPT_RUNTIMES.has(comm) && !SCRIPT_RUNTIMES.has(bin)) return comm;
//   const at = argv.findIndex((w, i) => i > 0 && !w.startsWith("-"));
//   if (at === -1) return comm;
//   const parts = argv[at]!.split("/");
//   const base = (parts.at(-1) ?? "").replace(/\.(m?js|cjs|ts)$/, "");
//   if (/\.py$/.test(base)) return `${comm} ${base}`;
//   if (!GENERIC_SCRIPTS.has(base)) return base;
//   // bin.js, cli.js and index.js say nothing: the package folder above them does, with the subcommand it was given.
//   const pkg = parts.slice(0, -1).reverse().find(d => d !== "" && !BUILD_DIRS.has(d));
//   const sub = argv.slice(at + 1).find(w => !w.startsWith("-"));
//   return [pkg ?? base, sub].filter(w => w !== undefined).join(" ");
// }
/// What a process is called: the package a runner runs, a tool with its subcommand, the script a runtime runs, or
/// its comm. Reads only `comm` and `cmdline`.
pub fn proc_name(p: &ProcEntry) -> String {
    let argv: Vec<&str> = p.cmdline.split(' ').filter(|w| !w.is_empty()).collect();
    let bin = argv.first().copied().unwrap_or(p.comm.as_str()).rsplit('/').next().unwrap_or("");
    let comm = if p.comm.is_empty() { bin } else { p.comm.as_str() };
    if TOOLS.contains(&comm) {
        let valued = value_flags(comm);
        let rest = argv.get(1..).unwrap_or(&[]);
        let words: Vec<&str> = rest
            .iter()
            .enumerate()
            .filter(|&(i, w)| !w.starts_with('-') && !(i > 0 && valued.contains(&rest[i - 1])))
            .map(|(_, w)| *w)
            .collect();
        let path = run_words(comm).iter().find(|run| run.iter().enumerate().all(|(i, w)| words.get(i) == Some(w)));
        if let Some(pkg) = path.and_then(|run| words.get(run.len()).copied()) {
            if !not_a_name(pkg) {
                return strip_version(pkg).to_string();
            }
        }
        return match words.first() {
            None => comm.to_string(),
            Some(sub) => format!("{comm} {sub}"),
        };
    }
    if !SCRIPT_RUNTIMES.contains(&comm) && !SCRIPT_RUNTIMES.contains(&bin) {
        return comm.to_string();
    }
    let Some(at) = argv.iter().enumerate().position(|(i, w)| i > 0 && !w.starts_with('-')) else {
        return comm.to_string();
    };
    let parts: Vec<&str> = argv[at].split('/').collect();
    let base = strip_script_ext(parts.last().copied().unwrap_or(""));
    if base.ends_with(".py") {
        return format!("{comm} {base}");
    }
    if !GENERIC_SCRIPTS.contains(&base) {
        return base.to_string();
    }
    let pkg = parts[..parts.len() - 1].iter().rev().find(|d| !d.is_empty() && !BUILD_DIRS.contains(d));
    let sub = argv[at + 1..].iter().find(|w| !w.starts_with('-'));
    let mut named = vec![pkg.copied().unwrap_or(base)];
    named.extend(sub.copied());
    named.join(" ")
}

/// One agent of the catalog as `agent_by_bin` reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentBin {
    pub id: String,
    pub bin: String,
}

// views/activity-names.ts:66 at a3b870a6e
// export function agentByBin(p: Pick<ProcEntry, "comm" | "cmdline">, agents: readonly { readonly id: string; readonly bin: string }[]): string | undefined {
//   const bins = procBins(p);
//   return agents.find(a => bins.includes(a.bin))?.id;
// }
/// The first agent, in the order given, whose binary is one of the names the process runs by (`proc_bins`).
pub fn agent_by_bin(p: &ProcEntry, agents: &[AgentBin]) -> Option<String> {
    let bins = proc_bins(p);
    agents.iter().find(|a| bins.contains(&a.bin)).map(|a| a.id.clone())
}

// views/activity-names.ts:72 at a3b870a6e
// export interface NameFacts {
//   /** The daemon's pid and the host's, where the reader knows them. */
//   readonly daemon?: number;
//   readonly host?: number;
//   /** Its agent's name, where it is a thread's agent. */
//   readonly agent?: string;
// }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NameFacts {
    /// The daemon's pid and the host's, where the reader knows them.
    pub daemon: Option<i64>,
    pub host: Option<i64>,
    /// Its agent's name, where it is a thread's agent.
    pub agent: Option<String>,
}

// views/activity-names.ts:81 at a3b870a6e
// export function activityName(p: Pick<ProcEntry, "pid" | "comm" | "cmdline">, facts: NameFacts): string {
//   if (facts.agent !== undefined) return facts.agent;
//   if (p.pid === facts.daemon) return "Daemon";
//   if (p.pid === facts.host) return "Host";
//   return procName(p);
// }
pub fn activity_name(p: &ProcEntry, facts: &NameFacts) -> String {
    if let Some(agent) = &facts.agent {
        return agent.clone();
    }
    if facts.daemon == Some(i64::from(p.pid)) {
        return "Daemon".to_string();
    }
    if facts.host == Some(i64::from(p.pid)) {
        return "Host".to_string();
    }
    proc_name(p)
}
