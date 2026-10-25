// SPDX-License-Identifier: AGPL-3.0-only
// Both plugin shelves' whole read of the TS tests' recorded fixtures, written as tests/fixtures/plugins.json: every row
// the TS reads, so the Rust read is held to all of them, not to a count. The fakes are plugins.test.ts's own.
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { CLAUDE_PLUGIN_SHELF, CODEX_PLUGIN_SHELF, joinPath, plainValues, type PeekGroup, type Peeked, type PluginIo } from "../../../../packages/catalog/src/index.js";

const OUT = fileURLToPath(new URL("../tests/fixtures/", import.meta.url));
const fixture = <T,>(name: string): T => JSON.parse(readFileSync(new URL(`../../../../packages/catalog/test/fixtures/plugins/${name}`, import.meta.url), "utf8")) as T;
const CLAUDE = fixture<{ files: Record<string, string> }>("claude-2.1.296.json");
const CODEX = fixture<{ pluginList: unknown; configRead: unknown; pluginRead: Record<string, { plugin: unknown }>; writeStale: unknown; writeOk: unknown }>("codex-0.162.1.json");

/** A computer holding exactly these files, every folder above one there too, answering only what a peek asks;
 * `runs` records each line run and `peeks` each peek. */
function filesIo(files: Record<string, string>, o: { store?: string; run?: (line: string) => string | undefined } = {}): PluginIo & { runs: string[]; peeks: number } {
  const paths = Object.keys(files);
  const io = {
    home: "/x",
    ...(o.store !== undefined ? { store: o.store } : {}),
    runs: [] as string[],
    peeks: 0,
    peek: async (groups: readonly PeekGroup[]): Promise<Peeked> => {
      io.peeks++;
      const texts = new Map<string, string>();
      const lists = new Map<string, string[]>();
      const looked = new Set<string>();
      for (const g of groups) {
        const key = g.rootsFrom?.key ?? "";
        const from = [...new Set((g.rootsFrom?.files ?? []).flatMap(f => plainValues(files[f] ?? "", key)))];
        for (const r of [...(g.roots ?? []), ...from]) {
          looked.add(joinPath(r));
          for (const f of g.files ?? []) if (files[joinPath(r, f)] !== undefined) texts.set(joinPath(r, f), files[joinPath(r, f)]!);
          for (const d of g.dirs ?? []) {
            const dir = joinPath(r, d.path);
            const under = paths.filter(x => x.startsWith(`${dir}/`)).map(x => x.slice(dir.length + 1).split("/"));
            if (under.length > 0) lists.set(dir, [...new Set(under.flatMap(segs => segs.slice(0, d.depth).map((_, i) => segs.slice(0, i + 1).join("/"))))]);
          }
        }
      }
      return { text: p => texts.get(joinPath(p)), list: d => lists.get(joinPath(d)), looked: r => looked.has(joinPath(r)) };
    },
    run: async (line: string) => {
      io.runs.push(line);
      return o.run?.(line);
    },
  };
  return io;
}

function codexIo(o: { stale?: boolean; dead?: boolean; none?: boolean; old?: boolean; uninstalled?: string } = {}): PluginIo & { runs: string[]; sent: { method: string; params: Record<string, unknown> }[] } {
  const sent: { method: string; params: Record<string, unknown> }[] = [];
  const io = filesIo({}, {
    run: line => {
      if (o.none === true) return "__WSP_NO_CODEX__\n";
      if (o.dead === true) return undefined;
      const lines = [...line.matchAll(/'(\{"jsonrpc"[^']*\})'/g)].map(m => JSON.parse(m[1]!) as { id: number; method: string; params: Record<string, unknown> });
      return lines
        .map(req => {
          sent.push({ method: req.method, params: req.params });
          if (req.method === "initialize") return { id: req.id, result: { codexHome: "/x/codex" } };
          if (req.method === "plugin/list" && o.uninstalled !== undefined) {
            const list = JSON.parse(JSON.stringify(CODEX.pluginList)) as { marketplaces: { plugins: { id: string; installed: boolean; enabled: boolean }[] }[] };
            for (const p of list.marketplaces.flatMap(m => m.plugins)) if (p.id === o.uninstalled) Object.assign(p, { installed: false, enabled: false });
            return { id: req.id, result: list };
          }
          if (req.method === "plugin/list") return o.old === true ? { id: req.id, error: { code: -32600, message: "Invalid request: unknown variant `plugin/list`, expected one of `initialize`, `thread/start`" } } : { id: req.id, result: CODEX.pluginList };
          if (req.method === "config/read") return { id: req.id, result: CODEX.configRead };
          if (req.method === "plugin/read") return { id: req.id, result: Object.values(CODEX.pluginRead).find(r => (r.plugin as { summary: { name: string } }).summary.name === req.params.pluginName) };
          if (req.method === "config/value/write") return o.stale === true ? { id: req.id, error: CODEX.writeStale } : { id: req.id, result: CODEX.writeOk };
          return { id: req.id, error: { message: "unknown" } };
        })
        .map(a => JSON.stringify(a))
        .join("\n");
    },
  });
  return Object.assign(io, { sent });
}


it("writes the plugin reads", async () => {
  const lab = [{ id: "p1", name: "lab", path: "/x/lab" }];
  const out = {
    claudeStore: await CLAUDE_PLUGIN_SHELF.read(filesIo(CLAUDE.files, { store: "/x/claude" }), []),
    claudeStoreLab: await CLAUDE_PLUGIN_SHELF.read(filesIo(CLAUDE.files, { store: "/x/claude" }), lab),
    claudeNoStore: await CLAUDE_PLUGIN_SHELF.read(filesIo(CLAUDE.files), []),
    codex: await CODEX_PLUGIN_SHELF.read(codexIo(), []),
    codexUninstalled: await CODEX_PLUGIN_SHELF.read(codexIo({ uninstalled: "brag@brag" }), []),
  };
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, "plugins.json"), `${JSON.stringify(out, null, 2)}\n`);
});
