// SPDX-License-Identifier: AGPL-3.0-only
// A command's output read summary first (#2468): the tally a test runner prints at its end, then the lines that say
// what failed, so the output piece and the sketch lead with those and fold the raw lines. One reading the web app, the
// GPUI app and the sketch share; a runner it does not know still folds, and leads with its last line.

/** What a run's lines come to: the runner's own counts, the lines that name a failure, and its last line. */
export interface SlateRunSummary {
  passed?: number;
  failed?: number;
  skipped?: number;
  /** Failure and error lines, trimmed, in order, at most SLATE_SUMMARY_LINES. */
  matters: string[];
  /** The last line that is not blank, for output no runner's tally reads. */
  last?: string;
}

export const SLATE_SUMMARY_LINES = 5;

type Count = "passed" | "failed" | "skipped";
const WORD: Readonly<Record<string, Count>> = {
  passed: "passed", passing: "passed",
  failed: "failed", failing: "failed", failures: "failed", error: "failed", errors: "failed",
  skipped: "skipped", pending: "skipped", ignored: "skipped", todo: "skipped",
};
const TALLY = /(\d[\d,]*) (passed|passing|failed|failing|failures|errors?|skipped|pending|ignored|todo)\b/gi;
/** cargo prints one per test binary, so a run of several crates sums them. */
const CARGO = /^\s*test result: \w+\. /;
/** Lines that name a failure in vitest, jest, mocha, pytest, cargo, go, tsc, eslint and plain shells. */
const MATTERS = /^\s*(?:FAIL\b|FAILED\b|ERROR\b|--- FAIL|[×✗✖❌]|\d+\) )|\.\.\. FAILED$|\berror(?:\[\w+\])?:|\bpanicked at\b|\bAssertionError\b|^Traceback \(most recent call last\)|\bnpm ERR!/;
const ANSI = new RegExp("\u001b\\[[0-9;]*m", "g");

function tally(line: string): Partial<Record<Count, number>> {
  const out: Partial<Record<Count, number>> = {};
  for (const m of line.matchAll(TALLY)) {
    const word = WORD[m[2]!.toLowerCase()]!;
    out[word] = (out[word] ?? 0) + Number(m[1]!.replace(/,/g, ""));
  }
  return out;
}

/** The summary of a run's lines. Adjacent tally lines merge, the later winning a count both say (vitest's Tests line
 * over its Test Files line, jest's Tests over its Test Suites); every cargo test result line sums. */
export function slateRunSummary(lines: readonly string[]): SlateRunSummary {
  const clean = lines.map(l => l.replace(ANSI, ""));
  const counts: Partial<Record<Count, number>> = {};
  const cargo = clean.filter(l => CARGO.test(l));
  if (cargo.length > 0) {
    for (const line of cargo) for (const [k, n] of Object.entries(tally(line)) as [Count, number][]) counts[k] = (counts[k] ?? 0) + n;
  } else {
    let end = clean.length - 1;
    while (end >= 0 && Object.keys(tally(clean[end]!)).length === 0) end--;
    let start = end;
    while (start > 0 && Object.keys(tally(clean[start - 1]!)).length > 0) start--;
    for (let i = start; i <= end && end >= 0; i++) Object.assign(counts, tally(clean[i]!));
  }
  const matters: string[] = [];
  for (const line of clean) {
    const said = line.trim();
    if (said === "" || !MATTERS.test(line) || matters.includes(said) || Object.keys(tally(line)).length > 0) continue;
    matters.push(said.length > 160 ? `${said.slice(0, 159)}…` : said);
    if (matters.length === SLATE_SUMMARY_LINES) break;
  }
  const last = [...clean].reverse().find(l => l.trim() !== "")?.trim();
  return { ...counts, matters, ...(last !== undefined ? { last } : {}) };
}

/** The counts as words, failed first and only when some did: "2 failed", "191 passed", "43 skipped". */
export function slateSummaryCounts(s: SlateRunSummary): { count: Count; words: string }[] {
  return (["failed", "passed", "skipped"] as const).filter(k => s[k] !== undefined && s[k]! > 0).map(k => ({ count: k, words: `${s[k]!.toLocaleString("en-US")} ${k}` }));
}
