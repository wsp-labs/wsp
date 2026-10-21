// SPDX-License-Identifier: AGPL-3.0-only
// CI's --shard=<n>/<count>, split by time. Vitest's own split deals the files out by a hash of their paths, which gave
// one of four parts 830 s of tests and another 486 s. Each file weighs what scripts/test-times.json (written by
// scripts/test-times.mjs) says its tests took in CI, plus the cost of loading it, a file the table lacks weighs the
// table's median, and the heaviest file goes first to the lightest part. Every runner resolves the same files and
// deals them the same way.
import { readFileSync } from "node:fs";
import { relative } from "node:path";
import { BaseSequencer, type WorkspaceSpec } from "vitest/node";

const TIMES: Record<string, number> = JSON.parse(readFileSync(new URL("./scripts/test-times.json", import.meta.url), "utf8"));
const LOAD_MS = 500;

/** The keys in part <index> (from 1) of <count>. */
export function deal(keys: string[], index: number, count: number, times: Record<string, number> = TIMES): string[] {
  const known = Object.values(times).sort((a, b) => a - b);
  const median = known.length === 0 ? 0 : known[Math.floor(known.length / 2)]!;
  const weighed = keys.map(key => ({ key, ms: (times[key] ?? median) + LOAD_MS })).sort((a, b) => b.ms - a.ms || (a.key < b.key ? -1 : a.key > b.key ? 1 : 0));
  const loads = new Array<number>(count).fill(0);
  const mine: string[] = [];
  for (const { key, ms } of weighed) {
    const lightest = loads.indexOf(Math.min(...loads));
    loads[lightest]! += ms;
    if (lightest === index - 1) mine.push(key);
  }
  return mine;
}

export class TimedSequencer extends BaseSequencer {
  override async shard(files: WorkspaceSpec[]): Promise<WorkspaceSpec[]> {
    const { index, count } = this.ctx.config.shard!;
    const byKey = new Map(files.map(spec => [`${spec.project.name}:${relative(this.ctx.config.root, spec.moduleId)}`, spec]));
    return deal([...byKey.keys()], index, count).map(key => byKey.get(key)!);
  }
}
