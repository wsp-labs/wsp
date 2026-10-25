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

use wsp_frames::ProcEntry;

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
    todo!()
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
    todo!()
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
    todo!()
}
