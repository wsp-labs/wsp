// SPDX-License-Identifier: AGPL-3.0-only
// The session fixture's extra threads, made only when BENCH_SESSION=<n> reaches fixture.sh: `session-1` to
// `session-<n>`, each a medium thread shaped like `real` (screenshots, pastes, tool output, subagents, code, tables)
// with its own seed and length, so a run that opens them one after another, as a person moves down the sidebar,
// meets a different thread each time. The default bench never makes them.

const NAME = /^session-(\d+)$/;

/** The plan of `session-<i>`, in the shape `real` takes; undefined for any other name. 12 to 40 turns. */
export function sessionPlan(name) {
  const i = Number(NAME.exec(name)?.[1] ?? 0);
  if (i < 1) return undefined;
  return { shape: "real", turns: 12 + ((i * 13) % 29), calls: 3, tasks: 2, changes: 1, pastes: 0.06, shots: 0.1, writeUps: 0.03, tables: 0.08, code: 0.3, seed: 100 + i };
}

/** The names BENCH_SESSION asks for: none when it is unset or empty. */
export function sessionNames(asked) {
  const n = Number(asked ?? 0);
  if (!Number.isInteger(n) || n < 0 || n > 200) throw new Error(`BENCH_SESSION=${asked} is not a count from 0 to 200`);
  return Array.from({ length: n }, (_, i) => `session-${i + 1}`);
}
