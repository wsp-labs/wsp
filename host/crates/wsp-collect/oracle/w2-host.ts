// SPDX-License-Identifier: AGPL-3.0-only
// The wave-2 oracles' shared pieces: a laptop described as JSON that the Rust tests rebuild with
// tests/common/oracle.rs, and one fixture file per unit under tests/fixtures/oracle/w2/.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Host, Platform } from "../../../../packages/collect/src/host.js";
import { fakeHost } from "../../../../packages/collect/test/fake-host.js";

export interface Laptop {
  platform?: Platform;
  files?: Record<string, number | string>;
  which?: string[];
  exec?: Record<string, string>;
  mtimes?: Record<string, number>;
  shell?: string;
  terminal?: string;
  xdgConfigHome?: string;
  claudeConfigDir?: string;
  codexHome?: string;
  stores?: Record<string, string>;
  seeds?: Record<string, string>;
}

/** The same mark as tests/common/fake_host.rs's LINES_FAIL: a text file's line that starts with it makes `lines` throw there. */
export const LINES_FAIL = "\u0000fail:";

/** fakeHost over the laptop, with the fields the TS fake has no option for set on the host, and LINES_FAIL honoured. */
export function host(l: Laptop): Host & { calls: string[] } {
  const h = fakeHost(l);
  const lines = h.fs.lines.bind(h.fs);
  const out = Object.assign(h, {
    ...(l.claudeConfigDir !== undefined ? { claudeConfigDir: l.claudeConfigDir } : {}),
    ...(l.codexHome !== undefined ? { codexHome: l.codexHome } : {}),
    ...(l.stores !== undefined ? { stores: l.stores } : {}),
    ...(l.seeds !== undefined ? { seeds: l.seeds } : {}),
  });
  out.fs = {
    ...h.fs,
    async *lines(path: string, holding?: string) {
      for await (const line of lines(path, holding)) {
        if (line.startsWith(LINES_FAIL)) throw new Error(line.slice(LINES_FAIL.length));
        yield line;
      }
    },
  };
  return out;
}

/** A value as JSON carries it: a Map is its entries, undefined is left out of objects and is null in arrays. */
export const plain = (v: unknown): unknown => JSON.parse(JSON.stringify(v, (_k, x) => (x instanceof Map ? [...x.entries()] : x instanceof Set ? [...x] : x)) ?? "null");

/** What a call threw, as `{ error: message }`, or what it answered, as `{ out }`. */
export async function settle(f: () => unknown): Promise<{ out: unknown } | { error: string }> {
  try {
    return { out: plain(await f()) };
  } catch (e) {
    return { error: e instanceof Error ? e.message : String(e) };
  }
}

const dir = fileURLToPath(new URL("../tests/fixtures/oracle/w2/", import.meta.url));
export function write(unit: string, value: unknown): void {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, `${unit}.json`), `${JSON.stringify(value, null, 1)}\n`);
}
