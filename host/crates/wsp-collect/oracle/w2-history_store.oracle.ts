// SPDX-License-Identifier: AGPL-3.0-only
// history_store: buckets, the one-holder rule, usage, the cache file and read_histories.
// A cache case names its cache path with "<dir>" for a temp folder the Rust side substitutes; "before" and "after" are
// the file's exact text (null for no file), "dirs_after" says whether named folders exist after the run.
//   pnpm exec vitest run --config host/crates/wsp-collect/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 host/crates/wsp-collect/oracle/w2-history_store.oracle.ts
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { it } from "vitest";
import { CATALOG_AGENTS, MIB } from "../../../../packages/catalog/src/index.js";
import { fileHistoryCache, readHistories } from "../../../../packages/collect/src/history/index.js";
import { inFolders, installKey, installedName, isHeavy, meetsUsedFloor, oneHolder, reduceCalls, usageOf, type HistoryBucket } from "../../../../packages/collect/src/history/tally.js";
import { host, plain, settle, write, type Laptop } from "./w2-host.js";

type Case = Record<string, unknown>;
const HOME = "/Users/dev";

const bucket = (session: string, o: Record<string, unknown> = {}): HistoryBucket => ({ session, calls: 1, commands: {}, installs: {}, ...o }) as HistoryBucket;
const usage = (buckets: HistoryBucket[], folders?: string[]) => usageOf(buckets, folders);
const sh = (session: string, line: string, o: Record<string, unknown> = {}) => ({ session, kind: "shell", line, ...o });
const ot = (session: string, name: string, o: Record<string, unknown> = {}) => ({ session, kind: "other", name, ...o });
async function* gen(items: unknown[]): AsyncGenerator<never> {
  for (const i of items) yield i as never;
}

const asst = (session: string, command: string, extra: Record<string, unknown> = {}): string => JSON.stringify({ type: "assistant", cwd: "/p", sessionId: session, message: { role: "assistant", content: [{ type: "tool_use", id: "t", name: "Bash", input: { command } }] }, ...extra });
const FAIL = "\u0000fail:EIO";

/** Runs one cache scenario over a real temp folder; the path is recorded with `<dir>` for the folder. */
async function cacheCase(name: string, rel: string, before: string | null, ops: { op: "get" | "set" | "save"; args?: unknown[] }[], dirs: string[] = []): Promise<Case> {
  const dir = mkdtempSync(join(tmpdir(), "w2-history-"));
  const path = join(dir, rel);
  if (before !== null) {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, before);
  }
  const c = fileHistoryCache(path);
  const outs: unknown[] = [];
  for (const o of ops) {
    if (o.op === "get") outs.push(plain((c.get as (...a: unknown[]) => unknown)(...(o.args ?? []))));
    else if (o.op === "set") {
      (c.set as (...a: unknown[]) => unknown)(...(o.args ?? []));
      outs.push(null);
    } else {
      c.save();
      outs.push(null);
    }
  }
  return {
    name,
    cache: { path: `<dir>/${rel}`, before, after: existsSync(path) ? readFileSync(path, "utf8") : null, dirs_after: Object.fromEntries(dirs.map(d => [`<dir>/${d}`, existsSync(join(dir, d))])) },
    ops,
    outs,
  };
}

/** Runs read_histories over a laptop (and an optional cache file) and records everything it told. */
async function historyCase(name: string, laptop: Laptop, agents: string[], opts: { folders?: string[] } = {}, cache?: { rel: string; before: string | null }, withCalls = true): Promise<Case> {
  const h = host(laptop);
  let path: string | undefined;
  const dir = mkdtempSync(join(tmpdir(), "w2-history-"));
  if (cache !== undefined) {
    path = join(dir, cache.rel);
    if (cache.before !== null) {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, cache.before);
    }
  }
  const told: string[] = [];
  const progress: string[] = [];
  const picked = agents.map(id => CATALOG_AGENTS.find(a => a.id === id)!);
  const r = await settle(() => readHistories(h, picked, { ...(opts.folders !== undefined ? { folders: opts.folders } : {}), ...(path !== undefined ? { cache: fileHistoryCache(path) } : {}), onAgent: a => told.push(`${a.agent} ${a.state}`), onProgress: p => progress.push(`${p.agent} ${p.read}/${p.files}`) }));
  return {
    name,
    laptop,
    args: [agents, opts],
    ...r,
    told,
    progress,
    ...(withCalls ? { calls: h.calls } : {}),
    ...(cache !== undefined ? { cache: { path: `<dir>/${cache.rel}`, before: cache.before, after: path !== undefined && existsSync(path) ? readFileSync(path, "utf8") : null, dirs_after: { [`<dir>/${dirname(cache.rel)}`]: existsSync(dirname(path!)) } } } : {}),
  };
}

it("history_store", async () => {
  const f: Record<string, unknown[]> = {};

  // ---- reduce_calls
  const reduce = async (name: string, calls: unknown[]): Promise<Case> => ({ name, args: [calls], out: plain(await reduceCalls(gen(calls))) });
  f["reduce_calls"] = [
    await reduce("at zero and negative", [sh("s", "ls", { at: 0 }), sh("s", "ls", { at: -3 }), ot("t", "x", { at: -3 }), ot("t", "x", { at: -9 })]),
    await reduce("at undefined then defined", [ot("s", "x"), ot("s", "x", { at: 4 }), ot("s", "x"), ot("s", "x", { at: 4 })]),
    await reduce("folder empty vs missing share a key", [ot("a", "x"), ot("a", "x", { folder: "" }), ot("b", "x", { folder: "" }), ot("b", "x"), ot("c", "x", { folder: "/f" }), ot("c", "x")]),
    await reduce("same session two folders", [sh("s", "gh", { folder: "/a" }), sh("s", "gh", { folder: "/b" }), sh("s", "gh", { folder: "/a" })]),
    await reduce("shell lines yield counts, empty ones count a call", [sh("s", ""), sh("s", "   "), sh("s", "ls | ls && brew install a a b; npm i -g a"), sh("s", "cd /x"), sh("s", "git --version")]),
    await reduce("[JS key order: integer-like names first] integer-like names", [sh("s", "z; 7; a; 10; 007; 2")]),
    await reduce("no calls", []),
    await reduce("a NUL in a session id keeps two buckets", [ot("a\u0000b", "x"), ot("a", "x", { folder: "b" })]),
    await reduce("install keys in first-seen order", [sh("s", "npm i -g b a; brew install c; npm i -g b")]),
    await reduce("first call order of buckets", [ot("s1", "Read", { folder: "/a", at: 5 }), ot("s2", "Read"), sh("s1", "ls", { folder: "/b" }), sh("s1", "gh api", { folder: "/a", at: 3 }), ot("s1", "Edit", { folder: "/a", at: 9 })]),
  ];

  // ---- one_holder
  const h = (holder: string, ...buckets: HistoryBucket[]) => ({ holder, buckets });
  const oneHolderCase = (name: string, held: ReturnType<typeof h>[]): Case => ({ name, args: [held], out: plain(oneHolder(held)) });
  f["one_holder"] = [
    oneHolderCase("a repeated holder merges its newest", [h("A", bucket("s", { at: 5 })), h("B", bucket("s", { at: 10 }), bucket("s", { at: 1, calls: 7 })), h("A", bucket("s", { at: 20, calls: 2 }))]),
    oneHolderCase("two folder buckets of the kept holder both stay", [h("A", bucket("s", { at: 5, folder: "/x" }), bucket("s", { at: 5, folder: "/y" })), h("B", bucket("s", { at: 5 }))]),
    oneHolderCase("a time of zero beats none", [h("A", bucket("s", { calls: 3 })), h("B", bucket("s", { at: 0, calls: 4 }))]),
    oneHolderCase("negative times", [h("A", bucket("s", { at: -5, calls: 3 })), h("B", bucket("s", { at: -6, calls: 4 })), h("C", bucket("s", { at: -5, calls: 5 }))]),
    oneHolderCase("a later holder wins, then a tie keeps it", [h("A", bucket("s", { at: 1, calls: 3 })), h("B", bucket("s", { at: 2, calls: 4 })), h("C", bucket("s", { at: 2, calls: 5 }))]),
    oneHolderCase("a holder with no buckets", [h("A"), h("B", bucket("s", { at: 1 }))]),
    oneHolderCase("output order is held order then bucket order", [h("B", bucket("t", { at: 1 }), bucket("s", { at: 1, calls: 2 })), h("A", bucket("s", { at: 1, calls: 3 }), bucket("u", { at: 1 }))]),
    oneHolderCase("nothing held", []),
    oneHolderCase("both with no time: the first holder", [h("A", bucket("s", { calls: 3 })), h("B", bucket("s", { calls: 4 }))]),
  ];

  // ---- usage_of
  const usageCase = (name: string, buckets: HistoryBucket[], folders?: string[]): Case => ({ name, args: folders === undefined ? [buckets] : [buckets, folders], out: plain(usage(buckets, folders)) });
  const nine = { "～": 1, "\u{1F600}": 1, Z: 1, a: 1, "é": 1, e: 1, B: 1, b: 1, "10": 1, "9": 1 };
  const mixed = [bucket("s1", { folder: "/a/b", commands: { gh: 1 } }), bucket("s2", { folder: "/a", commands: { gh: 1 } }), bucket("s3", { folder: "", commands: { gh: 1 } }), bucket("s4", { commands: { gh: 1 } }), bucket("s5", { folder: "/", commands: { gh: 1 } }), bucket("s6", { folder: "/ab", commands: { gh: 1 } }), bucket("s7", { folder: "/a/", commands: { gh: 1 } })];
  f["usage_of"] = [
    usageCase("code unit order of names", [bucket("s1", { commands: nine })]),
    usageCase("sessions desc, calls desc, name asc", [bucket("s1", { commands: { b: 3, a: 3, c: 1 } }), bucket("s2", { commands: { b: 1, c: 5, d: 9 } }), bucket("s3", { commands: { c: 1 } })]),
    usageCase("a tool sums its commands and its installs", [bucket("s1", { calls: 5, commands: { jq: 2, brew: 1 }, installs: { "brew jq": 1, "apt jq": 2 } })]),
    usageCase("two names of one tool add up, npx has no tool", [bucket("s1", { commands: { npm: 2, npx: 1, node: 1, pnpm: 1 }, installs: { "npm agent-browser": 1, "go gh": 1, "cargo ripgrep": 1 } }), bucket("s2", { commands: { npm: 1 } })]),
    usageCase("installs alone, a key with spaces or none", [bucket("s1", { installs: { "brew gh": 1, weird: 1, "a b c": 1 } })]),
    usageCase("a session in two folders counts once", [bucket("s1", { folder: "/a", commands: { gh: 1 } }), bucket("s1", { folder: "/b", commands: { gh: 2 } }), bucket("s2", { commands: { gh: 1 } })]),
    usageCase("calls and no names", [bucket("s1", { calls: 7 })]),
    usageCase("no buckets", []),
    ...[[], ["/"], ["/a"], ["/a/"], ["/a/b/"], [""], ["/a", "/ab"], ["/a/b", "/a"], ["a"], ["/A"]].map(fl => usageCase(`folders ${JSON.stringify(fl)}`, mixed, fl)),
  ];

  f["in_folders"] = [...mixed.map(b => ({ name: `folder ${JSON.stringify(b.folder ?? null)} against /a`, args: [b.folder === undefined ? {} : { folder: b.folder }, ["/a"]], out: inFolders(b, ["/a"]) })), { name: "no folders at all", args: [{ folder: "/a" }, []], out: inFolders({ folder: "/a" }, []) }];

  f["installed_name"] = ["a b c", "nospace", "", " x", "npm @scope/x", "npm x"].map(k => ({ name: JSON.stringify(k), args: [k], out: installedName(k) }));
  f["install_key"] = [{ via: "npm", name: "x" }, { via: "brew", name: "a b" }].map(i => ({ name: JSON.stringify(i), args: [i], out: installKey(i) }));

  const floors: [number, number, number | undefined][] = [[2, 5, undefined], [1, 99, undefined], [2, 4, undefined], [2, 5, 0], [3, 20, 300 * MIB], [2, 5, 300 * MIB], [2, 5, 300 * MIB + 1], [3, 20, 300 * MIB + 1], [3, 19, 300 * MIB + 1], [2, 20, 300 * MIB + 1], [3, 5, 300 * MIB + 1], [0, 0, undefined], [100, 100, 1e12]];
  f["meets_used_floor"] = floors.map(([sessions, calls, size]) => ({ name: `${sessions} sessions, ${calls} calls, size ${size}`, args: size === undefined ? [{ sessions, calls }] : [{ sessions, calls }, size], out: meetsUsedFloor({ sessions, calls }, size) }));
  f["is_heavy"] = [undefined, 0, 300 * MIB, 300 * MIB + 1, -1].map(size => ({ name: `size ${size}`, args: size === undefined ? [] : [size], out: isHeavy(size) }));

  // ---- the cache file
  const GOOD = { stamp: "1:1", buckets: [{ session: "s", folder: "/f", at: 5, calls: 2, commands: { gh: 2 }, installs: { "brew jq": 1 } }] };
  const file = (v: unknown): string => JSON.stringify(v);
  const asked = [{ op: "get" as const, args: ["/x", "1:1"] }, { op: "get" as const, args: ["/y", "1:1"] }, { op: "save" as const }];
  const withBucket = (b: Record<string, unknown>) => file({ version: 1, parse: 2, files: { "/x": { stamp: "1:1", buckets: [{ ...GOOD.buckets[0], ...b }] } } });
  const shapes: [string, string][] = [
    ["good", file({ version: 1, parse: 2, files: { "/x": GOOD } })],
    ["an extra top-level key", file({ version: 1, parse: 2, files: { "/x": GOOD }, extra: 1 })],
    ["an extra key on the entry", file({ version: 1, parse: 2, files: { "/x": { ...GOOD, extra: 1 } } })],
    ["an extra key on the bucket", withBucket({ extra: 1 })],
    ["a null folder refuses the whole file", withBucket({ folder: null })],
    ["a null at refuses the whole file", withBucket({ at: null })],
    ["no folder and no at, zero calls", file({ version: 1, parse: 2, files: { "/x": { stamp: "1:1", buckets: [{ session: "s", calls: 0, commands: {}, installs: {} }] } } })],
    ["a fractional negative at", withBucket({ at: -1.5 })],
    ["fractional calls", withBucket({ calls: 1.5 })],
    ["a command count of zero", withBucket({ commands: { gh: 0 } })],
    ["a command count as a string", withBucket({ commands: { gh: "2" } })],
    ["installs as an array", withBucket({ installs: [] })],
    ["a numeric stamp", file({ version: 1, parse: 2, files: { "/x": { stamp: 1, buckets: [] } } })],
    ["buckets missing", file({ version: 1, parse: 2, files: { "/x": { stamp: "1:1" } } })],
    ["one bad entry of two refuses both", file({ version: 1, parse: 2, files: { "/x": GOOD, "/y": { stamp: "1:1", buckets: "no" } } })],
    ["files as an array", file({ version: 1, parse: 2, files: [] })],
    ["files null", file({ version: 1, parse: 2, files: null })],
    ["files missing", file({ version: 1, parse: 2 })],
    ["version as a string", file({ version: "1", parse: 2, files: {} })],
    ["parse missing", file({ version: 1, files: { "/x": GOOD } })],
    ["parse 1", file({ version: 1, parse: 1, files: { "/x": GOOD } })],
    ["session as a number", withBucket({ session: 5 })],
    ["empty session and empty stamp", file({ version: 1, parse: 2, files: { "/x": { stamp: "", buckets: [{ ...GOOD.buckets[0], session: "" }] } } })],
    ["a BOM before the JSON", `﻿${file({ version: 1, parse: 2, files: { "/x": GOOD } })}`],
    ["text after the JSON", `${file({ version: 1, parse: 2, files: { "/x": GOOD } })} x`],
    ["blank lines after the JSON", `${file({ version: 1, parse: 2, files: { "/x": GOOD } })}\n\n`],
    ["duplicate keys: the last wins", '{"version":1,"parse":2,"files":{"/x":{"stamp":"9:9","buckets":[]},"/x":{"stamp":"1:1","buckets":[]}}}'],
    ["version and parse written 1.0 and 2.0", '{"version":1.0,"parse":2.0,"files":{"/x":{"stamp":"1:1","buckets":[]}}}'],
    ["calls written 1e3", '{"version":1,"parse":2,"files":{"/x":{"stamp":"1:1","buckets":[{"session":"s","calls":1e3,"commands":{},"installs":{}}]}}}'],
  ];
  f["file_history_cache"] = [];
  for (const [name, text] of shapes) f["file_history_cache"]!.push(await cacheCase(`shape: ${name}`, "c.json", text, asked));
  f["file_history_cache"]!.push(await cacheCase("a second stamp on a duplicate-key file", "c.json", shapes.find(s => s[0].startsWith("duplicate"))![1], [{ op: "get", args: ["/x", "9:9"] }, { op: "get", args: ["/x", "1:1"] }]));

  f["file_history_cache"]!.push(
    await cacheCase("save with nothing asked and no file: no file, no folder", "deep/c.json", null, [{ op: "save" }], ["deep"]),
    await cacheCase("save with nothing asked over a garbage file leaves it", "c.json", "garbage", [{ op: "save" }]),
    await cacheCase("a set over a garbage file replaces it whole", "deep/er/c.json", "garbage", [{ op: "set", args: ["/f", "1:2", []] }, { op: "save" }]),
    await cacheCase("a set makes the missing folder", "new/folder/c.json", null, [{ op: "set", args: ["/f", "1:2", []] }, { op: "save" }], ["new/folder"]),
  );
  const t1 = await cacheCase("set twice replaces in place", "c.json", null, [{ op: "set", args: ["/b", "1:1", [bucket("s", { at: 3 })]] }, { op: "set", args: ["/a", "1:1", []] }, { op: "set", args: ["/b", "2:2", [bucket("t")]] }, { op: "save" }]);
  const T1 = (t1["cache"] as { after: string }).after;
  f["file_history_cache"]!.push(
    t1,
    await cacheCase("hits and a miss, nothing changed: no write", "c.json", T1, [{ op: "get", args: ["/a", "1:1"] }, { op: "get", args: ["/b", "9:9"] }, { op: "get", args: ["/b", "2:2"] }, { op: "save" }]),
    await cacheCase("all hits and a miss for an unknown path: no write", "c.json", T1, [{ op: "get", args: ["/a", "1:1"] }, { op: "get", args: ["/b", "2:2"] }, { op: "get", args: ["/c", "1:1"] }, { op: "save" }]),
    await cacheCase("a hit then a miss on one path keeps the hit, drops the rest", "c.json", T1, [{ op: "get", args: ["/a", "1:1"] }, { op: "get", args: ["/a", "0:0"] }, { op: "save" }]),
    await cacheCase("save with nothing asked of a filled cache", "c.json", T1, [{ op: "save" }]),
    await cacheCase("a miss and a set of the same path", "c.json", T1, [{ op: "get", args: ["/a", "9:9"] }, { op: "set", args: ["/a", "9:9", [bucket("n")]] }, { op: "save" }]),
    await cacheCase("[JS key order: integer-like command names first] key order of a bucket", "c.json", null, [{ op: "set", args: ["/o", "1:1", [{ session: "s", at: 7, calls: 1, commands: { b: 1, "3": 1, a: 1, "1": 1 }, installs: {} }]] }, { op: "save" }]),
    await cacheCase("a re-read bucket comes back in zod order (at before calls)", "c.json", file({ version: 1, parse: 2, files: { "/o": { stamp: "1:1", buckets: [{ session: "s", calls: 1, commands: { b: 1 }, installs: {}, at: 7 }] } } }), [{ op: "get", args: ["/o", "1:1"] }, { op: "set", args: ["/z", "1:1", []] }, { op: "save" }]),
    await cacheCase("escapes in text", "c.json", null, [{ op: "set", args: ["/x", "1:1", [bucket("s", { commands: { "a\u0000b": 1, "\u007f": 2 }, folder: "/f \n\t\"é\u{1F600}" })]] }, { op: "save" }]),
  );

  // ---- read_histories
  const S = (name: string) => `~/.claude/projects/-p/${name}.jsonl`;
  const two: Laptop = { files: { [S("s1")]: [asst("s1", "gh pr view"), asst("s1", "ls")].join("\n"), [S("s2")]: asst("s2", "pnpm test && npm install -g agent-browser") } };
  const first = await historyCase("a first run with a cache", two, ["claude"], {}, { rel: "c.json", before: null });
  const A = (first["cache"] as { after: string }).after;
  const codexFiles: Record<string, string> = { ...two.files!, "~/.codex/sessions/2026/a/rollout-t.jsonl": JSON.stringify({ type: "response_item", payload: { type: "function_call", name: "shell", arguments: JSON.stringify({ command: "ls" }) } }) };
  const both = await historyCase("a first run over every catalog agent", { files: codexFiles }, CATALOG_AGENTS.map(a => a.id), {}, { rel: "c.json", before: null });
  const B = (both["cache"] as { after: string }).after;
  const asstFolder = (session: string, cmd: string, cwd: string) => asst(session, cmd, { cwd });
  const seven: Laptop = { files: Object.fromEntries([...Array.from({ length: 6 }, (_, i) => [S(`s${i}`), asst(`s${i}`, `gh x${i}`)]), [S("s6"), asstFolder("s6", "gh x6", "/other")]]) };
  f["read_histories"] = [
    first,
    await historyCase("a second run over an unchanged history opens no file", two, ["claude"], {}, { rel: "c.json", before: A }),
    await historyCase("a later mtime re-reads one file", { ...two, mtimes: { [S("s2")]: 1001 } }, ["claude"], {}, { rel: "c.json", before: A }),
    await historyCase("a grown file re-reads one file", { files: { ...two.files!, [S("s2")]: `${two.files![S("s2")]}\n${asst("s2", "cargo build")}` } }, ["claude"], {}, { rel: "c.json", before: A }),
    await historyCase("a file that is gone leaves the cache", { files: { [S("s1")]: two.files![S("s1")]! } }, ["claude"], {}, { rel: "c.json", before: A }),
    await historyCase("a half-written cache is read as none", two, ["claude"], {}, { rel: "c.json", before: "{ half a file" }),
    await historyCase("another parse version is read as none", two, ["claude"], {}, { rel: "c.json", before: A.replace('"parse":2', '"parse":3') }),
    both,
    await historyCase("a run over only codex drops claude's entries", { files: codexFiles }, ["codex"], {}, { rel: "c.json", before: B }),
    await historyCase("a run over only a no-reader agent empties the cache", { files: codexFiles }, ["gemini"], {}, { rel: "c.json", before: B }),
    await historyCase("no agents empties the cache", { files: codexFiles }, [], {}, { rel: "c.json", before: B }),
    await historyCase("nothing to read, no cache file, folder not made", {}, CATALOG_AGENTS.map(a => a.id), {}, { rel: "deep/c.json", before: null }),
    await historyCase("nothing to read, a garbage cache file is left", {}, CATALOG_AGENTS.map(a => a.id), {}, { rel: "c.json", before: "garbage" }),
    await historyCase("every catalog agent: claude read, codex empty, hermes unreadable, the rest no reader", { files: { [S("s1")]: asst("s1", "gh auth status"), "~/.hermes/state.db": 4096 } }, CATALOG_AGENTS.map(a => a.id)),
    await historyCase("a file that cannot be read among three: the others finish and are cached", { files: { [S("s0")]: asst("s0", "gh a"), [S("s1")]: `${FAIL}`, [S("s2")]: asst("s2", "gh c") } }, ["claude"], {}, { rel: "c.json", before: null }),
    await historyCase("a file that cannot be read alone", { files: { [S("s0")]: FAIL } }, ["claude"], {}, { rel: "c.json", before: null }),
    await historyCase("a file that cannot be read, after a failing part way", { files: { [S("s0")]: `${asst("s0", "gh a")}\n${FAIL}` } }, ["claude"]),
    await historyCase("folders that match nothing say empty", seven, ["claude"], { folders: ["/nowhere"] }, undefined, false),
    await historyCase("folders /p", seven, ["claude"], { folders: ["/p"] }, undefined, false),
    await historyCase("an empty folders list counts nothing", seven, ["claude"], { folders: [] }, undefined, false),
    await historyCase("all seven files, no weighing", seven, ["claude"], {}, undefined, false),
    await historyCase("an empty transcript", { files: { [S("e")]: "" } }, ["claude"]),
    await historyCase("a transcript with one empty command", { files: { [S("e")]: asst("e", "") } }, ["claude"]),
    await historyCase("a cleared copy under the same holder double counts", { files: { [S("s1")]: asst("s1", "gh a", { cwd: "/x" }), "~/.claude/projects/-p/_cleared_sessions/s1.jsonl": asst("s1", "gh b", { cwd: "/x" }) } }, ["claude"]),
    await historyCase("a folder named memory is a session, a root file is not", { files: { "~/.claude/projects/-p/memory/notes.jsonl": asst("m", "gh a", { cwd: "/x" }), "~/.claude/projects/stray.jsonl": asst("t", "gh b", { cwd: "/x" }) } }, ["claude"]),
    await historyCase("two holders, no times: the first holder", { files: { "~/.claude/projects/-p/s1.jsonl": asst("s1", "gh a", { cwd: "/x" }), "~/.claude/projects/-q/s1.jsonl": asst("s1", "git b", { cwd: "/x" }) } }, ["claude"]),
    await historyCase("two holders, the later call wins", { files: { "~/.claude/projects/-p/s1.jsonl": asst("s1", "gh a", { cwd: "/x", timestamp: "2026-09-06T10:00:00Z" }), "~/.claude/projects/-q/s1.jsonl": asst("s1", "git b", { cwd: "/x", timestamp: "2026-09-06T10:00:01Z" }) } }, ["claude"]),
    await historyCase("two holders, the same time: the first holder", { files: { "~/.claude/projects/-p/s1.jsonl": asst("s1", "gh a", { cwd: "/x", timestamp: "2026-09-06T10:00:00Z" }), "~/.claude/projects/-q/s1.jsonl": asst("s1", "git b", { cwd: "/x", timestamp: "2026-09-06T10:00:00Z" }) } }, ["claude"]),
    await historyCase("two holders, one with no time", { files: { "~/.claude/projects/-p/s1.jsonl": asst("s1", "gh a", { cwd: "/x", timestamp: "2026-09-06T10:00:00Z" }), "~/.claude/projects/-q/s1.jsonl": asst("s1", "git b", { cwd: "/x" }) } }, ["claude"]),
  ];

  write("history_store", f);
});
