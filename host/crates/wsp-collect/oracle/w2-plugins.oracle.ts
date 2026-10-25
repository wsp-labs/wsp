// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/plugins.md: plugins.ts on the edge inputs of the wave-2 audit (group d). `agents` args are catalog agent ids.
// Not recorded: a shelf whose read throws (the refusal text per thrown value is in the audit reply), the projects a shelf is
// handed, and the real Claude fixture read.
import { it } from "vitest";
import { CATALOG_AGENTS, type FoundPlugin, type PeekGroup } from "../../../../packages/catalog/src/index.js";
import { detectPlugins, pluginIo, pluginRow } from "../../../../packages/collect/src/detect/plugins.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const agentsOf = (ids: string[] | null) => (ids === null ? undefined : ids.map(id => CATALOG_AGENTS.find(a => a.id === id)!));
const lineOf = (calls: string[], cmd: string): string => calls.find(c => c.startsWith(`run ${cmd} -c `))!.slice(`run ${cmd} -c `.length).replace(/ \(30000 ms\)$/, "");

async function peekLine(name: string, groups: PeekGroup[]): Promise<Case> {
  const h = host({});
  await pluginIo(h, "claude").peek(groups);
  return { name, args: [groups], out: lineOf(h.calls, "sh"), calls: h.calls };
}

type Probes = { f?: string[]; d?: string[]; r?: string[] };
async function records(name: string, said: string | undefined, probes: Probes = {}): Promise<Case> {
  const probe = host({});
  const groups: PeekGroup[] = [{ roots: ["/x"] }];
  await pluginIo(probe, "claude").peek(groups);
  const laptop: Laptop = said === undefined ? {} : { exec: { [`sh -c ${lineOf(probe.calls, "sh")}`]: said } };
  const h = host(laptop);
  const p = await pluginIo(h, "claude").peek(groups);
  const out = p === undefined ? null : { f: Object.fromEntries((probes.f ?? []).map(k => [k, p.text(k) ?? null])), d: Object.fromEntries((probes.d ?? []).map(k => [k, p.list(k) ?? null])), r: Object.fromEntries((probes.r ?? []).map(k => [k, p.looked(k)])) };
  return { name, args: [said ?? null], probes, out: plain(out) };
}

async function io(name: string, agent: string, laptop: Laptop): Promise<Case> {
  const h = host(laptop);
  const p = pluginIo(h, agent);
  await p.run("echo hi");
  return { name, laptop, args: [agent, "echo hi"], out: { home: p.home, store: p.store ?? null, seed: p.seed ?? null, line: lineOf(h.calls, "bash") }, calls: h.calls };
}

const detect = async (name: string, laptop: Laptop, agents: string[] | null, projects: { id: string; name: string; path: string }[] = []): Promise<Case> => {
  const h = host(laptop);
  const r = await settle(async () => {
    const read = await detectPlugins(h, { agents: agentsOf(agents), projects });
    return { plugins: read.plugins, refused: read.refused, agents: read.agents.map(a => ({ agent: a.agent, found: a.found })) };
  });
  return { name, laptop, args: [agents, projects], ...r, calls: h.calls };
};

const FOUND = (id: string, extra: Record<string, unknown> = {}): FoundPlugin => ({ id, name: id, marketplace: "m", scope: "user", on: true, brings: { skills: [], commands: [], subagents: [], hooks: [], servers: [], lsp: [], apps: [] }, ...extra }) as unknown as FoundPlugin;

it("plugins fixtures", async () => {
  const fixture: Record<string, Case[]> = {};
  const E = "\x1eEND\n";

  fixture["peek_line"] = [
    await peekLine("rootsFrom with no files: two spaces after the key", [{ rootsFrom: { key: "installPath", files: [] }, dirs: [{ path: ".", depth: 1 }] }]),
    await peekLine("an empty roots list adds no peek call", [{ roots: [], rootsFrom: { key: "k", files: ["/i.json"] }, files: ["a"] }]),
    await peekLine("roots and rootsFrom: roots first", [{ roots: ["/r"], rootsFrom: { key: "k", files: ["/i.json"] }, files: ["a"], dirs: [{ path: "x y", depth: 3 }] }]),
    await peekLine("roots only", [{ roots: ["/r"] }]),
    await peekLine("an empty root", [{ roots: [""] }]),
    await peekLine("a key and a file with quotes and spaces", [{ rootsFrom: { key: "it's", files: ["/a b"] } }]),
    await peekLine("three groups keep their order", [{ roots: ["/r1"], files: ["a"] }, { roots: ["/r2"], files: ["b"] }, { rootsFrom: { key: "k", files: ["/f"] } }]),
    await peekLine("dirs at depth 0 and 10 joined by a newline", [{ roots: ["/r"], dirs: [{ path: "a", depth: 0 }, { path: "b", depth: 10 }] }]),
    await peekLine("a group with neither adds nothing", [{ files: ["a"] }]),
    await peekLine("no groups", []),
  ];

  fixture["peek_records"] = [
    await records("an f body is not split on U+001F", `\x1ef/a\x1fx\x1fy${E}`, { f: ["/a"] }),
    await records("an f body keeps its newline", `\x1ef/a\x1fline\n${E}`, { f: ["/a"] }),
    await records("an empty f body is present", `\x1ef/a\x1f${E}`, { f: ["/a"] }),
    await records("a later d record replaces the earlier", `\x1ed/r/skills\x1fa\x1fb\x1ed/r/skills\x1fc\x1f${E}`, { d: ["/r/skills"] }),
    await records("d names holding a newline", `\x1ed/r/s\x1fa\x1f\n\x1fb\nc\x1f\n\x1f\x1f${E}`, { d: ["/r/s"] }),
    await records("an empty d list is present", `\x1ed/r/s\x1f${E}`, { d: ["/r/s"] }),
    await records("an unknown kind is ignored", `\x1ex/a\x1fstuff\x1ef/b\x1fok${E}`, { f: ["/a", "/b"], d: ["/a"], r: ["/a"] }),
    await records("an r record marks its root, any spelling", `\x1er/x\x1f\x1ef/x/a\x1ft${E}`, { f: ["/x/a"], r: ["/x", "/x/", "//x", "/y", "/x/a"] }),
    await records("text before the first record is dropped", `motd\n\x1ef/a\x1fx${E}`, { f: ["/a"] }),
    await records("blanks after the end mark", "\x1ef/a\x1fx\x1eEND  \n\n", { f: ["/a"] }),
    await records("NBSP after the end mark", "\x1ef/a\x1fx\x1eEND ", { f: ["/a"] }),
    await records("an end mark in the middle", "\x1ef/a\x1fx\x1eEND\n\x1ef/b\x1fy\x1eEND\n", { f: ["/a", "/b"] }),
    await records("no end mark", "\x1ef/a\x1fx", { f: ["/a"] }),
    await records("the end mark alone, no newline", "\x1eEND", { f: ["/a"] }),
    await records("an empty path", `\x1ef\x1fv${E}`, { f: ["", "/"] }),
    await records("record paths are joined", `\x1ef/a//b/../c/\x1fv${E}`, { f: ["/a/c", "/a//c/", "/a/b/../c"] }),
    await records("a record separator inside a body cuts it", `\x1ef/a\x1fx\x1ey${E}`, { f: ["/a"] }),
    await records("f and d of one path are separate", `\x1ef/a\x1ft\x1ed/a\x1fn${E}`, { f: ["/a"], d: ["/a"] }),
    await records("kind letters with no path", `\x1ef\x1fx\x1ed\x1fy${E}`, { f: ["/"], d: ["/"] }),
    await records("lookups are joined too", `\x1ef/a/b\x1ft\x1ed/a/c\x1fz${E}`, { f: ["a/b", "/a/./b", "/a/x/../b"], d: ["/a/c/", "a/c"] }),
    await records("two end marks", `\x1ef/a\x1fx\x1eEND\nmore${E}`, { f: ["/a"] }),
    await records("nothing came back", undefined, { f: ["/a"] }),
    await records("an empty answer", "", { f: ["/a"] }),
    await records("the end mark with a newline only", E, { f: ["/a"] }),
  ];

  fixture["plugin_io"] = [
    await io("no stores, no seeds", "claude", {}),
    await io("claude with a store", "claude", { stores: { claude: "/Users/dev/s", codex: "" } }),
    await io("codex when only claude has a store", "codex", { stores: { claude: "/Users/dev/s" }, seeds: { claude: "/seed:/seed2" } }),
    await io("codex with a store", "codex", { stores: { claude: "/Users/dev/s", codex: "/Users/dev/c" } }),
    await io("empty store and seed strings are present", "claude", { stores: { claude: "" }, seeds: { claude: "" } }),
    await io("a seed does not change the line", "claude", { stores: { claude: "/Users/dev/s" }, seeds: { claude: "/seed" } }),
  ];

  fixture["plugin_row"] = [
    ["a path equal to home and a lookalike prefix", FOUND("b@m", { path: "/Users/dev", setIn: "/Users/developer/y", project: { id: "p", name: "P", path: "/elsewhere" } })],
    ["a trailing slash and a dot segment", FOUND("c@m", { path: "/Users/dev/", setIn: "/Users/dev/./x" })],
    ["a project path with a trailing slash, an undefined version, skills dropped", FOUND("a@m", { skills: { dirs: ["/s"], prefix: "a" }, path: "/Users/dev/.claude/x", setIn: "/Users/dev", project: { id: "p", name: "P", path: "/Users/dev/p/" }, version: undefined, source: "src" })],
    ["a missing plugin", FOUND("d@m", { missing: "gone" })],
  ].map(([n, p]) => ({ name: n, args: ["/Users/dev", "claude", plain(p)], out: plain(pluginRow({ home: "/Users/dev" }, "claude", p as FoundPlugin)) }));

  fixture["detect_plugins"] = [
    await detect("no agents with a shelf in the list", {}, []),
    await detect("claude with no answer", {}, ["claude"]),
    await detect("claude under a store", { stores: { claude: "/Users/dev/s" } }, ["claude"]),
    await detect("claude with a project", { stores: { claude: "/Users/dev/s" } }, ["claude"], [{ id: "p", name: "P", path: "/Users/dev/p" }]),
    await detect("codex with no answer", {}, ["codex"]),
    await detect("codex under a store", { stores: { codex: "/Users/dev/c" } }, ["codex"]),
    await detect("every catalog agent with a shelf", {}, null),
    await detect("an agent without a shelf is left out", {}, ["gemini", "claude"]),
  ];

  write("plugins", fixture);
});
