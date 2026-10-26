// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the wsp-cli tests pin, for packages/host/src/verbs.ts, verbs/client.ts, verbs/io.ts and
// verbs/threads.ts at 65f6b89c7: one JSON file per subject under tests/fixtures/, `_cloud` on the files written with
// the cloud on. Run from the repository root, once each way:
//   pnpm exec vitest run --config host/crates/wsp-cli/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
//   ORACLE_CLOUD=1 pnpm exec vitest run --config host/crates/wsp-cli/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { it } from "vitest";
import type { z } from "zod";
import { NAP_AFTER_MAX_MS, TURN_LIMIT_MAX_MS, DELETE_AFTER_MAX_MS, verbFailure, type ProjectPlan, type HostFolderListing, type ThreadView, type TurnResult } from "@wsp/protocol";
import * as V from "../../../../packages/host/src/verbs.js";
import { THREAD_VERBS } from "../../../../packages/host/src/verbs/threads.js";
import { CLOUD_ON } from "../../../../packages/host/src/cloud.js";
import type { CliIO } from "../../../../packages/host/src/cli.js";
import type { Turn, Ended } from "../../../../packages/host/src/verbs/turns-help.js";

const OUT = fileURLToPath(new URL("../tests/fixtures/", import.meta.url));
const SUFFIX = CLOUD_ON ? "_cloud" : "";
const write = (name: string, value: unknown): void => writeFileSync(`${OUT}${name}${SUFFIX}.json`, `${JSON.stringify(value, null, 1)}\n`);

/** An answer or the failure object every door prints for it, plus the fix the refusal carried. */
function caught<T>(f: () => T): { ok: T } | { err: ReturnType<typeof verbFailure> & { kind: string | null; fix: string | null } } {
  try {
    return { ok: f() };
  } catch (e) {
    const k = (e as { kind?: unknown }).kind;
    const fix = (e as { fix?: unknown }).fix;
    return { err: { ...verbFailure(e), kind: typeof k === "string" ? k : null, fix: typeof fix === "string" ? fix : null } };
  }
}

/** One zod input field as the Rust ToolInput carries it. */
function field(name: string, s: z.ZodTypeAny): unknown {
  let optional = false;
  let describe: string | undefined = s.description;
  let t: z.ZodTypeAny = s;
  for (;;) {
    const n = t._def.typeName as string;
    if (n === "ZodOptional") {
      optional = true;
      t = t._def.innerType;
      describe ??= t.description;
      continue;
    }
    if (n === "ZodDefault") {
      t = t._def.innerType;
      describe ??= t.description;
      continue;
    }
    break;
  }
  const n = t._def.typeName as string;
  const d = t._def as Record<string, unknown>;
  let kind: unknown;
  if (n === "ZodString") kind = { type: "string" };
  else if (n === "ZodBoolean") kind = { type: "boolean" };
  else if (n === "ZodNumber") {
    const checks = d["checks"] as { kind: string; value?: number; inclusive?: boolean }[];
    const min = checks.find(c => c.kind === "min");
    kind = checks.some(c => c.kind === "int") ? { type: "integer", min: min === undefined ? null : min.inclusive === false ? min.value! + 1 : min.value } : { type: "number" };
  } else if (n === "ZodArray" && (d["type"] as z.ZodTypeAny)._def.typeName === "ZodString") kind = { type: "stringList", min: (d["minLength"] as { value: number } | null)?.value ?? null };
  else if (n === "ZodEnum") kind = { type: "enum", options: d["values"] };
  else if (n === "ZodLiteral") kind = { type: "literal", value: d["value"] };
  else kind = { type: "json" };
  return { name, kind, optional, describe: describe ?? null };
}

/** One table entry, every field the Rust Verb carries, its strings as the module wrote them. */
function entry(v: V.Verb): unknown {
  const r = v as unknown as Record<string, unknown>;
  const tool = (r["tool"] as V.Tool | undefined) ?? undefined;
  return {
    name: v.name,
    shape: "command" in v ? "commandTool" : "toolOnly" in v ? "toolOnly" : "cliOnly" in v ? "cliOnly" : "cli",
    usage: r["usage"] ?? null,
    about: r["about"] ?? null,
    page: r["page"] ?? null,
    options: r["options"] === undefined ? null : Object.entries(r["options"] as V.FlagTable).map(([name, row]) => ({ name, type: row.type, multiple: row.multiple === true, short: row.short ?? null, valueWith: row.valueWith ?? null })),
    readsHere: r["readsHere"] ?? null,
    startsNoHost: r["startsNoHost"] ?? null,
    anyRelease: r["anyRelease"] ?? null,
    cloud: r["cloud"] === true,
    cloudFlags: r["cloudFlags"] ?? [],
    cliOnly: r["cliOnly"] ?? null,
    hostSide: r["hostSide"] ?? null,
    toolOnly: r["toolOnly"] ?? null,
    tool: tool === undefined ? null : { description: tool.description, input: Object.entries(tool.input).map(([n, s]) => field(n, s as z.ZodTypeAny)), output: Object.keys(tool.output), stream: tool.stream ?? [] },
  };
}

/** A terminal that records what a line printed. */
function recordingIO(tty = false, answer = "no"): CliIO & { lines: string[]; errors: string[]; asked: string[] } {
  const lines: string[] = [];
  const errors: string[] = [];
  const asked: string[] = [];
  return {
    lines,
    errors,
    asked,
    log: l => void lines.push(l),
    error: l => void errors.push(l),
    stream: () => {},
    isTTY: tty,
    ask: async q => {
      asked.push(q);
      return answer;
    },
    askSecret: async () => "",
  };
}

it("the verb table", () => {
  write("thread_verbs", THREAD_VERBS.map(entry));
  write("all_verbs", V.ALL_VERBS.map(entry));
  write("verbs", V.VERBS.map(entry));
});

it("the help a verb prints", () => {
  const pages: Record<string, string> = {};
  for (const v of V.CLI_VERBS) pages[v.name] = V.verbPage(v, "hostSide" in v && v.hostSide !== undefined ? V.COMMON_FLAG_WORDS.hostSide : V.COMMON_FLAG_WORDS.host);
  const help: Record<string, string> = { all: V.verbHelp() };
  for (const p of ["front", "agent", "host", "dev", "app"] as const) help[p] = V.verbHelp(p);
  const firstWords = [...new Set(V.CLI_VERBS.map(v => v.name.split(" ")[0]!)), "nope", ""];
  const usages = Object.fromEntries(firstWords.map(w => [w, V.verbUsage(w) ?? null]));
  const says: Record<string, string | null> = {};
  for (const v of V.CLI_VERBS) for (const f of V.ownFlagsOf(v)) says[`${v.name} ${f}`] = V.flagSays(v.name, f) ?? null;
  const usageLines = [
    ["wsp run [<project>] [--on boat [--size <cpu>x<memGb>] [--from <ref>]] [--beside <thread>]", "    ", "  "],
    ["wsp a\nwsp b [--c <d e f>]", "    ", "usage: "],
    ["", "  ", "  "],
    ["wsp x [--" + "y".repeat(90) + "]", "    ", "  "],
  ].map(([u, i, l]) => ({ usage: u, indent: i, lead: l, lines: V.usageLines(u!, i!, l) }));
  const helpPages = [
    { usage: "wsp x", about: [], rows: [] },
    { usage: "wsp x [--long <v>]", about: ["  does a thing"], rows: [["--long", "a long flag ".repeat(12)], ["--j", "short"]] },
  ].map(c => ({ ...c, page: V.helpPage(c.usage, c.about, c.rows as [string, string][]) }));
  write("pages", { pages, help, usages, says, usageLines, helpPages, flagWords: V.FLAG_WORDS });
});

it("finding a verb and the shared flags", () => {
  const lines = [
    [],
    ["threads"],
    ["thread", "read", "x"],
    ["thread"],
    ["thread", "nope"],
    ["--json", "threads"],
    ["projects", "add"],
    ["projects", "remove", "x"],
    ["nope"],
    ["run", "say hi"],
    ["agents", "set", "claude"],
    ["exec", "t", "--", "ls"],
    ["Threads"],
    ["image", "build"],
    ["fork", "x"],
  ];
  const found = lines.map(argv => ({ argv, verb: V.findVerb(argv)?.name ?? null, cloudLine: V.cloudLineOf(argv) ?? null }));
  const commons = [
    ["threads", "--json"],
    ["--json", "threads"],
    ["--host", "mini", "threads"],
    ["threads", "--host"],
    ["threads", "--host=mini", "--state", "/s.json"],
    ["-h", "run"],
    ["run", "--", "--json", "x"],
    ["exec", "t", "--state", "--", "ls"],
    ["--hostx", "threads"],
    ["--json=1", "threads"],
    ["-hx"],
    ["--state", "a", "--state", "b"],
  ].map(argv => ({ argv, ...V.takeCommon(argv), jsonAsked: V.jsonAsked(argv) }));
  const table = { project: { type: "string", valueWith: "on" }, on: { type: "string" }, agent: { type: "string", multiple: true } } as V.FlagTable;
  const optional = [
    ["skills", "--project"],
    ["skills", "--project", "x"],
    ["skills", "--on", "mini", "--project", "x"],
    ["skills", "--on=mini", "--project"],
    ["skills", "--on", "mini", "--project", "--agent", "a"],
    ["skills", "--project", "--on", "mini"],
    ["skills", "--on", "mini", "--", "--project", "x"],
    ["skills", "--project=x"],
  ].map(argv => ({ argv, out: V.optionalValues(argv, table) }));
  write("lookup", { found, commons, optional, toolNames: ["thread read", "projects", "a b c"].map(w => [w, V.toolName(w)]) });
});

it("node's parser", () => {
  const options = { host: { type: "string" }, state: { type: "string" }, json: { type: "boolean" }, help: { type: "boolean", short: "h" }, file: { type: "string", multiple: true }, n: { type: "string", short: "m" }, yes: { type: "boolean" } } as const;
  const cases = [
    [],
    ["a", "b"],
    ["--host", "x"],
    ["--host=x"],
    ["--host="],
    ["--host"],
    ["--host", "-x"],
    ["--host", "--json"],
    ["--json=1"],
    ["--help=1"],
    ["-h"],
    ["-hh"],
    ["-hx"],
    ["-mfoo"],
    ["-m"],
    ["-m", "-x"],
    ["-hm", "v"],
    ["--file", "a", "--file", "b"],
    ["--file=a=b"],
    ["--", "--json", "x"],
    ["a", "--", "--"],
    ["-"],
    ["a", "--json", "b"],
    ["--json", "--json"],
    ["--Host", "x"],
    ["---x"],
    ["--nope=1"],
    ["--host", "a", "--host", "b"],
    ["--yes", "--host", "a", "--json"],
    ["--=x"],
    ["--", "-h"],
    ["-"],
    ["x", "-"],
  ];
  const parsed = cases.map(args => {
    try {
      const { values, positionals } = parseArgs({ args, options, allowPositionals: true, strict: true });
      return { args, values: Object.entries(values), positionals };
    } catch (e) {
      return { args, code: (e as { code: string }).code, message: (e as Error).message };
    }
  });
  write("parse_args", { options: Object.entries(options), parsed });
});

it("a verb's run before it dials", async () => {
  const lines = [
    ["threads", "--nope"],
    ["threads", "--nope", "--json"],
    ["run", "--ssh", "x", "hi"],
    ["run", "--recipe", "x"],
    ["exec", "t", "--cwd", "packages", "--", "ls"],
    ["exec", "t", "--cwd", "packages", "--json", "--", "ls"],
    ["exec", "--cwd", "/root", "--", "ls"],
    ["threads", "--host"],
    ["thread", "read", "--last=1", "x"],
    ["exec", "--help"],
    ["exec", "-h"],
    ["thread", "read", "x", "--help", "--nope"],
    ["run"],
    ["run", "a", "b", "c"],
    ["thread", "read"],
    ["thread", "read", "a", "b"],
    ["worktree", "x"],
    ["thread", "settle"],
    ["thread", "restore"],
    ["restart", "x"],
    ["stop"],
    ["send", "t"],
    ["setup", "x"],
    ["terminal", "config", "x"],
    ["terminal", "config", "--scheme", "blue"],
    ["folders", "a", "b"],
    ["thread", "allow"],
    ["thread", "deny", "a", "b"],
    ["exec", "t"],
    ["thread", "rename", "t"],
    ["thread", "forget"],
    ["thread", "head"],
    ["worktree", "remove", "p"],
    ["thread", "settle", "t", "--processes", "maybe"],
    ["run", "--size", "2x4", "hi"],
    ["run", "--from", "main", "hi"],
    ["run", "--at", "2", "hi"],
  ];
  const ran = [];
  for (const argv of lines) {
    const verb = V.findVerb(argv);
    if (verb === undefined) {
      ran.push({ argv, verb: null });
      continue;
    }
    const io = recordingIO();
    let dialled = 0;
    const code = await V.runVerb(verb, argv, io, flag => flag ?? "/nowhere/state.json", {
      env: {},
      dial: async () => {
        dialled++;
        throw Object.assign(new Error("dialled"), { kind: "usage" });
      },
    });
    ran.push({ argv, verb: verb.name, code, lines: io.lines, errors: io.errors, dialled });
  }
  write("run_verb", ran);
});

it("the client's helpers", async () => {
  const words = ["", " ", "off", "0", "1", "20", "1.5", "-1", "1e1", "0x10", "010", "+5", " 5", "60", "61", "1440", "1441", "24", "25", "720", "721", "Infinity", "NaN", "abc"];
  const asked = {
    nap: words.map(w => ({ word: w, out: caught(() => V.napAsked(w, "mini")) })),
    turnLimit: words.map(w => ({ word: w, out: caught(() => V.turnLimitAsked(w, "mini")) })),
    deleteAfter: words.map(w => ({ word: w, out: caught(() => V.deleteAfterAsked(w, "mini")) })),
    dollars: [...words, "10.25", "1e309", "-0", "0.0"].map(w => ({ word: w, out: caught(() => V.dollarsAsked(w, "mini")) })),
    limits: { napMinutes: NAP_AFTER_MAX_MS / 60_000, turnHours: TURN_LIMIT_MAX_MS / 3_600_000, deleteMinutes: DELETE_AFTER_MAX_MS / 60_000 },
  };
  const oneOf = [
    ["by", ["agent", "model"], undefined, undefined],
    ["by", ["agent", "model"], "model", undefined],
    ["by", ["agent", "model"], "Model", undefined],
    ["by", ["agent", "model"], "", "mini"],
    ["by", ["agent", "model"], 'say "hi"', undefined],
  ].map(([n, w, v, on]) => ({ name: n, words: w, value: v ?? null, on: on ?? null, out: caught(() => V.oneOf(n as string, w as string[], v as string | undefined, on as string | undefined) ?? null) }));
  const tables = [
    [],
    [["A"]],
    [["A", "B"], ["long cell", "x"]],
    [["A", "B", "C"], ["", "", ""], ["x", "", "z"]],
    [["NAME", "LAST"], ["é", "x"], ["日本", "y"], ["😀", "z"]],
    [["A", "B"], ["a"], ["bb", "c", "extra"]],
    [["A ", "B  "], ["x", "y  "]],
  ].map(rows => ({ rows, lines: V.table(rows) }));
  const releases = [
    ["0.3.8", "0.3.7", { state: "/s/state.json" }, undefined],
    ["0.3.8", "0.3.7", { state: "/s/state.json" }, "app"],
    ["0.3.8", "0.3.7", { state: "/s/state.json" }, "up"],
    ["0.3.8", "0.3.7", { state: "/s/state.json" }, "service"],
    ["0.3.8", "0.3.7", { where: "mini", here: false }, undefined],
    ["0.3.8", "0.3.7", { where: "http://127.0.0.1:7777", here: true }, "up"],
    ["0.3.8", "0.3.10", { state: "/s/state.json" }, undefined],
    ["0.3.8", "0.4.0", { where: "mini", here: false }, "app"],
  ].map(([mine, theirs, host, road]) => {
    const e = V.releaseRefusal(mine as string, theirs as string, host as V.ReleaseHost, road as string | undefined) as Error & { kind?: string; fix?: string };
    return { mine, theirs, host, road: road ?? null, message: e.message, kind: e.kind ?? null, fix: e.fix ?? null };
  });
  const usage = new Error("the request did not match: [{\"path\":[\"thread\"],\"message\":\"Required\"}]");
  const failures = [
    new Error("plain"),
    Object.assign(new Error("wsp exec: names itself"), { kind: "usage" }),
    Object.assign(new Error("no thread nope"), { kind: "not-found" }),
    Object.assign(new Error("refused token"), { kind: "auth" }),
    Object.assign(new Error("slate"), { kind: "usage", errors: [{ code: "V1" }], warnings: [], left: [{ id: "x" }] }),
    Object.assign(new Error("lists not arrays"), { errors: "x" }),
  ];
  const failed = failures.flatMap(e =>
    [false, true].map(json => {
      const io = recordingIO();
      const code = V.failed(io, json, e, "wsp exec: ");
      return { message: e.message, json, code, errors: io.errors, lines: io.lines };
    }),
  );
  const toolFailures = [...failures, usage].flatMap(e => [undefined, "wsp exec <thread> -- <command...>"].map(u => ({ message: e.message, usage: u ?? null, out: V.toolFailure(e, u) })));
  const misc = {
    flagFor: [["--nap", undefined], ["--nap", "mini"]].map(([f, on]) => [f, on ?? null, V.flagFor(f!, on)]),
    absolutePath: ["/a", "a", "", "./a", "~/a"].map(p => ({ path: p, out: caught(() => V.absolutePath("--cwd is a path on the machine", p)) })),
    absoluteFolder: [undefined, "/x", "x"].map(p => ({ path: p ?? null, out: caught(() => V.absoluteFolder(p) ?? null) })),
    accessWordOf: ["ask", "auto-edit", "full", "plan", "bypassPermissions", "", "Full"].map(w => ({ word: w, out: caught(() => V.accessWordOf(w)) })),
    under: [["/a/b", "/a"], ["/a", "/a"], ["/ab", "/a"], ["/a/b", "/a/"], ["/a", "/a/"], ["a", ""], ["/x", "/"]].map(([p, f]) => [p, f, V.under(p!, f!)]),
    releaseHereFix: [undefined, "app", "up", "service", "verb"].map(r => [r ?? null, V.releaseHereFix("0.3.8", r)]),
    otherVersion: V.otherVersion("sessions.start"),
    printable: ["a\tb\nc", "\x1b[31mred\x1b[0m", "\x7f\x85\x9f\xa0é", "\r\x0b\x0c\x00"].map(s => [s, V.printable(s)]),
    cell: ["a\tb\nc", "\x1b[31mred", "ok", " x"].map(s => [s, V.cell(s)]),
    pick: [V.pick({ a: 1, b: undefined, c: null, d: "", e: false })],
    opened: [V.openedThreadLine("t-1", undefined), V.openedThreadLine("t-1", (id, f) => `${id} in ${f}`, "/x"), V.openedThreadSaid("t-1", undefined), V.openedThreadSaid("t-1", (id, f) => `${id} in ${f ?? "-"}`)],
    schemaRefusal: [usage, new Error("plain")].map(e => ({ message: e.message, out: caught(() => { const r = V.hostSchemaRefusal(e, "wsp exec <thread>"); if (r) throw r; return null; }) })),
    settingResets: V.SETTING_RESETS,
    common: Object.entries(V.COMMON),
    pickOptions: Object.entries(V.PICK_OPTIONS),
    sendOptions: Object.entries(V.SEND_OPTIONS),
    flag: [V.flag({ a: "x", b: true, c: ["y"] }, "a") ?? null, V.flag({ a: "x", b: true, c: ["y"] }, "b") ?? null, V.flag({}, "a") ?? null],
    flagList: [V.flagList({ c: ["y", "z"], a: "x" }, "c"), V.flagList({ c: ["y"], a: "x" }, "a"), V.flagList({}, "c")],
    hostPlatform: V.hostPlatform(),
  };
  const out: string[] = [];
  const fmt = (json: boolean): void => {
    const io = recordingIO();
    const o = V.formatter({ ...io, stream: t => void out.push(`stream:${t}`) }, json);
    o.emit({ a: 1, "\u0085": "\u009f" }, "the line");
    o.emit({ b: 2 });
    o.stream("text");
    out.push(...io.lines.map(l => `log:${l}`));
  };
  fmt(false);
  fmt(true);
  write("client", { asked, oneOf, tables, releases, failed, toolFailures, misc, formatter: out });
});

it("the io helpers", async () => {
  const plan: ProjectPlan = {
    source: "/root/alpha",
    repo: true,
    files: 1203,
    bytes: 52428800,
    secrets: [
      { path: ".env", bytes: 120, signals: ["name", "keys"] },
      { path: "deploy/key.pem", bytes: 1704, signals: ["pem", "mode"] },
      { path: ".git/config", bytes: 300, signals: ["url"], rewrite: { urls: ["https://github.com/a/b"], drop: [] } },
    ],
    excluded: ["node_modules", "target"],
    skipped: [{ path: "big.iso", note: "over the size a file may be" }],
    agents: [
      { agent: "claude", name: "Claude Code", sessions: 3, bytes: 4096, carry: "moves" },
      { agent: "codex", name: "Codex", sessions: 0, bytes: 0, carry: "transcript-only", error: "its store is locked" },
      { agent: "opencode", name: "OpenCode", sessions: 1, bytes: 10, carry: "transcript-only" },
    ],
  };
  const empty: ProjectPlan = { source: "/root/empty", repo: false, files: 0, bytes: 0, secrets: [], excluded: [], skipped: [], agents: [] };
  const sets = (s: ReadonlySet<string>): string[] => [...s].sort();
  const chosen = [
    [[], []],
    [["deploy/key.pem"], []],
    [[], [".env"]],
    [[".env"], [".env"]],
    [["nope"], []],
    [[], ["nope"]],
  ].map(([keep, cut]) => ({ keep, cut, out: caught(() => sets(V.secretsChosen(plan, keep!, cut!))) }));
  const chosenEmpty = caught(() => sets(V.secretsChosen(empty, ["x"], [])));
  const agents = [undefined, ["claude"], ["codex"], ["nope"], []].map(named => ({ named: named ?? null, out: caught(() => sets(V.agentsChosen(plan, named))) }));
  const agentsEmpty = caught(() => sets(V.agentsChosen(empty, ["claude"])));
  const planLines = {
    full: V.planLines(plan, V.secretsChosen(plan, [], []), V.agentsChosen(plan, undefined)),
    ticked: V.planLines(plan, new Set([".env", "deploy/key.pem", ".git/config"]), new Set()),
    empty: V.planLines(empty, new Set(), new Set()),
  };
  const listing: HostFolderListing = { dir: "/root", roots: ["/root", "/srv/app"], folders: [{ path: "/root/alpha", repo: true, branch: "main" }, { path: "/root/beta", repo: true }, { path: "/root/notes", repo: false }], hidden: 4 };
  const emptyListing: HostFolderListing = { dir: "/root/empty", roots: ["/root"], folders: [], hidden: 0 };
  const flags = {
    timeout: [undefined, "1", "0.5", "0", "-1", "", " ", "abc", "1e3", "Infinity"].map(v => ({ value: v ?? null, out: caught(() => V.timeoutFlag(v) ?? null) })),
    agents: [undefined, "claude", "claude, codex", ",", " , ", "claude,,codex", "nope", "claude,nope", "Claude"].map(v => ({ value: v ?? null, out: caught(() => V.agentsFlag(v) ?? null) })),
    scheme: [undefined, "light", "dark", "Dark", ""].map(v => ({ value: v ?? null, out: caught(() => V.schemeFlag(v) ?? null) })),
    projectFolders: [[], ["/a", "/b"], ["/a", "b", "c"]].map(f => ({ folders: f, out: caught(() => V.projectFolders(f)) })),
  };
  const replaced = [Object.assign(new Error("/root/x is already there"), { kind: "exists" }), new Error("other"), Object.assign(new Error("gone"), { kind: "not-found" })].map(e => {
    const r = V.withReplaceHint(e) as Error & { kind?: string };
    return { message: e.message, kind: (e as { kind?: string }).kind ?? null, out: { message: r.message, kind: r.kind ?? null, same: r === e } };
  });
  const confirms = [];
  for (const [yes, tty, answer] of [[true, false, "no"], [false, false, "yes"], [false, true, "yes"], [false, true, "no"], [false, true, ""]] as const) {
    const io = recordingIO(tty, answer);
    let out: unknown;
    try {
      out = { ok: await V.confirmedAt(io, yes, "Remove the worktree for b in p?\nThe branch stays.", "b") };
    } catch (e) {
      out = { err: verbFailure(e) };
    }
    confirms.push({ yes, tty, answer, out, asked: io.asked, errors: io.errors });
  }
  const session = { id: "s-1", workspaceId: "ws-1", harness: "claude", status: "running", threadId: "t-1", startedBy: "agent", cwd: "/root/alpha" } as unknown as Turn["session"];
  const result: TurnResult = { status: "completed", text: "all done\nlast line", durationMs: 1200, costUsd: 0.25 } as TurnResult;
  const turns: Turn[] = [
    { session, threadId: "t-1", outcome: "started", result },
    { session, threadId: "t-1", outcome: "started" },
    { session, threadId: "t-1", outcome: "steered", result: { ...result, text: undefined } as TurnResult, afterCut: true },
    { session: { ...session, capped: { place: "mini", running: 2, atOnce: 2 } } as unknown as Turn["session"], threadId: "t-1", outcome: "held" },
    { session, threadId: "t-1", outcome: "queued", result: { status: "failed", error: "the agent stopped" } as TurnResult },
    { session, threadId: "t-1", outcome: "started", result: { status: "failed", error: "sign in first", refusal: "sign-in" } as TurnResult },
  ];
  const turnOuts = turns.map(t => ({ turn: t, out: caught(() => V.turnOut(t)), view: V.turnView(t), detached: V.detachedOut(t), detachedOpened: V.detachedOut(t, (id, f) => `opened ${id} in ${f ?? "-"}`, undefined, "the disk was gone") }));
  const texts = [
    { threadId: "t", workspaceId: "w", harness: "claude", outcome: "started", text: "hi" },
    { threadId: "t", workspaceId: "w", harness: "claude", outcome: "started", afterCut: true, text: "hi" },
    { threadId: "t", workspaceId: "w", harness: "claude", outcome: "started", notice: "new disk", afterCut: true },
    { threadId: "t", workspaceId: "w", harness: "claude", outcome: "started" },
  ].map(o => ({ out: o, text: V.turnText(o as never), noticed: V.noticed(o, "said"), kept: V.noticed(o, undefined) }));
  const named = [{ id: "t-1", threadId: "t-1", workspaceId: "ws-1", harness: "claude", status: "running" }, { id: "s-2", workspaceId: "ws-1", harness: "codex", status: "running" }] as unknown as ThreadView[];
  const ended: Ended = { threadId: "t-1", result };
  const waits = [
    { waited: { ended }, length: "tail" },
    { waited: { ended }, length: "whole" },
    { waited: { ended: { threadId: "t-1", result: { status: "failed", error: "boom" } as TurnResult } }, length: "tail" },
    { waited: { ended: { ...ended, sub: { branch: "wsp/x", sha: "abc1234", commits: 2 } as never } }, length: "tail" },
    { waited: { timedOutMs: 30_000 }, length: "tail" },
  ].map(w => ({ ...w, out: V.waitAnswer(named, w.waited as never, w.length as never) }));
  write("io", {
    plan,
    empty,
    chosen,
    chosenEmpty,
    agents,
    agentsEmpty,
    planLines,
    listing,
    emptyListing,
    folderLines: { full: V.folderLines(listing), empty: V.folderLines(emptyListing) },
    flags,
    replaced,
    confirms,
    turnOuts,
    texts,
    waits,
    asJson: V.asJson({ a: 1, b: { c: ["x", "\u0085"] } }),
    asText: V.asText("line", { a: 1 }),
    sendMeets: V.SEND_MEETS,
    capMeets: V.CAP_MEETS,
    planOnly: V.PLAN_ONLY,
    importNow: V.IMPORT_NOW,
    projectsFlag: [V.projectsFlag({ project: ["a", "/b", "../c"] }), V.projectsFlag({ project: "a" }), V.projectsFlag({})],
    reasonFlag: Object.entries(V.REASON_FLAG),
    pickInputs: Object.entries(V.PICK_INPUTS).map(([n, s]) => field(n, s as z.ZodTypeAny)),
    sendInputs: Object.entries(V.SEND_INPUTS).map(([n, s]) => field(n, s as z.ZodTypeAny)),
    shared: Object.entries({ WorkspaceIn: V.WorkspaceIn, ThreadIn: V.ThreadIn, AgentIn: V.AgentIn, NotifyIn: V.NotifyIn, RunProjectIn: V.RunProjectIn, BesideIn: V.BesideIn, BranchIn: V.BranchIn, OnIn: V.OnIn, SubSizeIn: V.SubSizeIn, SubFromIn: V.SubFromIn, RunCwdIn: V.RunCwdIn, ExecCwdIn: V.ExecCwdIn, CwdIn: V.CwdIn, DetachIn: V.DetachIn, ResumeIn: V.ResumeIn, CopyIn: V.CopyIn, TitleIn: V.TitleIn, ForkIn: V.ForkIn, AtIn: V.AtIn, ReplacesIn: V.ReplacesIn, FilesIn: V.FilesIn, FastIn: V.FastIn, ConfirmIn: V.ConfirmIn, SizeIn: V.SizeIn, MaxMachinesIn: V.MaxMachinesIn, MaxDepthIn: V.MaxDepthIn, PROJECT_FOLDERS: V.PROJECT_FOLDERS, WEIGH_BY_FOLDERS: V.WEIGH_BY_FOLDERS }).map(([n, s]) => field(n, s as z.ZodTypeAny)),
  });
});
