// SPDX-License-Identifier: AGPL-3.0-only
// What Activity calls a process and its kind, which Settings > Activity, the
// Processes pane and `wsp activity` all print: one rule, so the page and the
// command line never name one process two ways.
import type { ProcEntry, ProcKind } from "../wire/daemon.js";
import { procBins, SCRIPT_RUNTIMES } from "./activity.js";

/** Named with the subcommand they run: cargo test, pnpm dev. */
const TOOLS = new Set(["cargo", "npm", "npx", "pnpm", "yarn", "uv", "uvx", "go", "bunx"]);
/** The words after each tool that fetch a package and run it, which the package then names: `uv tool uvx
 * mcp-search-console`. A runner of its own (npx, bunx, uvx) needs none. */
const RUN_WORDS: Readonly<Record<string, readonly (readonly string[])[]>> = {
  uv: [["tool", "uvx"], ["tool", "run"]],
  npm: [["exec"], ["x"]],
  pnpm: [["dlx"], ["exec"]],
  yarn: [["dlx"]],
  npx: [[]],
  bunx: [[]],
  uvx: [[]],
};
const UV_VALUES = ["--from", "--with", "--with-editable", "--with-requirements", "--python", "-p", "--directory", "--project", "--index", "--index-url", "--extra-index-url", "--default-index", "--constraints", "--overrides"];
/** The flags of each tool that take the next word as their value, which names nothing. */
const VALUE_FLAGS: Readonly<Record<string, ReadonlySet<string>>> = {
  uv: new Set(UV_VALUES),
  uvx: new Set(UV_VALUES),
  npm: new Set(["-p", "--package", "-c", "--call", "-w", "--workspace"]),
  npx: new Set(["-p", "--package", "-c", "--call"]),
  pnpm: new Set(["--package", "-c", "-C", "--dir", "--filter", "-F"]),
  yarn: new Set(["-p", "--package"]),
  bunx: new Set(["-p", "--package"]),
};
/** A word that names no process: an address or a version. */
const NOT_A_NAME = /^([a-z][a-z0-9+.-]*:\/\/|git\+|v?\d+(\.\d+)*$)/i;
const GENERIC_SCRIPTS = new Set(["bin", "cli", "index", "main"]);
const BUILD_DIRS = new Set(["dist", "lib", "build", "out", "src", "bin", ".bin", "node_modules"]);

/** What a process is called: the script a runtime runs, the package a runner runs, a tool with its subcommand, or the
 * command. */
export function procName(p: Pick<ProcEntry, "comm" | "cmdline">): string {
  const argv = p.cmdline.split(" ").filter(w => w !== "");
  const bin = (argv[0] ?? p.comm).split("/").at(-1) ?? p.comm;
  const comm = p.comm !== "" ? p.comm : bin;
  if (TOOLS.has(comm)) {
    const valued = VALUE_FLAGS[comm];
    const words = argv.slice(1).filter((w, i, all) => !w.startsWith("-") && !(i > 0 && valued?.has(all[i - 1]!) === true));
    const path = RUN_WORDS[comm]?.find(run => run.every((w, i) => words[i] === w));
    const pkg = path === undefined ? undefined : words[path.length];
    if (pkg !== undefined && !NOT_A_NAME.test(pkg)) return pkg.replace(/(?<=.)@[^@/]*$/, "");
    const sub = words[0];
    return sub === undefined ? comm : `${comm} ${sub}`;
  }
  if (!SCRIPT_RUNTIMES.has(comm) && !SCRIPT_RUNTIMES.has(bin)) return comm;
  const at = argv.findIndex((w, i) => i > 0 && !w.startsWith("-"));
  if (at === -1) return comm;
  const parts = argv[at]!.split("/");
  const base = (parts.at(-1) ?? "").replace(/\.(m?js|cjs|ts)$/, "");
  if (/\.py$/.test(base)) return `${comm} ${base}`;
  if (!GENERIC_SCRIPTS.has(base)) return base;
  // bin.js, cli.js and index.js say nothing: the package folder above them does, with the subcommand it was given.
  const pkg = parts.slice(0, -1).reverse().find(d => d !== "" && !BUILD_DIRS.has(d));
  const sub = argv.slice(at + 1).find(w => !w.startsWith("-"));
  return [pkg ?? base, sub].filter(w => w !== undefined).join(" ");
}

/** The agent a process runs by the binary its command names, of the agents given (the catalog's), or none. */
export function agentByBin(p: Pick<ProcEntry, "comm" | "cmdline">, agents: readonly { readonly id: string; readonly bin: string }[]): string | undefined {
  const bins = procBins(p);
  return agents.find(a => bins.includes(a.bin))?.id;
}

/** What the row reading a process knows of it beside its command line. */
export interface NameFacts {
  /** The daemon's pid and the host's, where the reader knows them. */
  readonly daemon?: number;
  readonly host?: number;
  /** Its agent's name, where it is a thread's agent. */
  readonly agent?: string;
}

/** What Activity calls a process: its agent's name, the daemon, the host, else what `procName` reads off its command. */
export function activityName(p: Pick<ProcEntry, "pid" | "comm" | "cmdline">, facts: NameFacts): string {
  if (facts.agent !== undefined) return facts.agent;
  if (p.pid === facts.daemon) return "Daemon";
  if (p.pid === facts.host) return "Host";
  return procName(p);
}

/** Each kind as a row's tag says it. */
export const ACTIVITY_KIND_WORDS: Readonly<Record<ProcKind, string>> = {
  agent: "Agent",
  tool: "Tool server",
  dev: "Dev server",
  test: "Test run",
  build: "Build",
  slate: "Slate run",
  terminal: "Terminal",
  tab: "Browser tab",
  app: "App",
  wsp: "wsp",
  other: "Other",
};

/** Kinds a row carries no tag for: its name says it already, or the tag would say nothing. */
export const UNTAGGED_KINDS: ReadonlySet<ProcKind> = new Set(["agent", "other", "wsp", "app"]);

/** The heads of the groups that are no thread, and the line under each. */
export const ACTIVITY_GROUP_WORDS = {
  wsp: "wsp",
  wspNote: "The app and what runs it",
  wspNoteBox: "What runs wsp here",
  loose: "No thread",
  looseNote: "Started with no thread wsp knows of",
} as const;
