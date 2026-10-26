// SPDX-License-Identifier: AGPL-3.0-only
// A command's output read summary first: the tally a test runner prints at its end, then the lines that say
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
/** Lines a test runner or a language prints on its own, at a line's start where a person's words would not be: the
 * lines W034 reads as a process's output. */
const RUNNER_FAILURE = [
  "^\\s*FAIL +(?:\\|[^|]+\\| +)?\\S+\\.\\w+", "--- FAIL", "\\.\\.\\. FAILED$", "^\\s*error(?:\\[\\w+\\])?:",
  "^\\s*[\\w.]*(?:Error|Exception)(?:\\[\\w+\\])?:", "\\bpanicked at\\b", "^Traceback \\(most recent call last\\)", "\\bnpm ERR!", "\\berror TS\\d+:",
];
/** Lines that only name a failure inside a log already known to be one, and that an agent writes as a status too
 * (FAILED, ✗ 2 failed, 1) a step, Last error: ...): the summary reads them, W034 does not. */
const NAMED_FAILURE = ["^\\s*(?:FAIL\\b|FAILED\\b|ERROR\\b|[×✗✖❌]|\\d+\\) )", "\\berror(?:\\[\\w+\\])?:", "\\bAssertionError\\b"];
const MATTERS = new RegExp([...RUNNER_FAILURE, ...NAMED_FAILURE].join("|"), "m");
/** Any line a process prints rather than a person writes: a runner's failure line, an exit code, a tally, file verdict
 * or indented test mark with its time, a stack frame, a lowercase warning head, a terminal colour. The glance checks warn where one is
 * drawn outside an <output>. */
export const SLATE_LOG_LINE = new RegExp([
  ...RUNNER_FAILURE, "\\b[Ee]xit(?:ed)?(?: with)? code \\d", "\\btest result:", "\\b\\d+ passed; \\d+ failed", "^\\s*warning(?:\\[|:)",
  "^\\s*Test Files {2}", "^\\s*Tests:? {2,}\\d", "^=+ .*\\bin \\d+(?:\\.\\d+)?s =+$", "^\\s*PASS {1,2}\\S+\\.\\w+",
  "^\\s*--- PASS:", "^ok {2,}\\S+\\s+\\d", "^\\s+[✓✔×✗✖] .+ \\d+ ?ms$", "^\\s+at .+:\\d+:\\d+\\)?$", "\u001b\\[[0-9;]*m",
].join("|"), "m");
/** vitest names a failed test twice, by its mark in the tree and on its FAIL line after: the FAIL line, with the file,
 * is the one kept. */
const FAIL_NAMES = /^\s*FAIL\s+(?:\|[^|]+\|\s+)?\S+ > (.+)$/;
const MARK_NAMES = /^\s*[×✗✖]\s+(.+?)(?:\s+\d+(?:\.\d+)?m?s)?$/;
const LINE_CHARS = 160;
/** A line past LINE_CHARS characters, cut on a whole one: UTF-16 units would split an emoji. */
function cut(line: string): string {
  if (line.length <= LINE_CHARS) return line;
  const chars: string[] = [];
  for (const ch of line) {
    chars.push(ch);
    if (chars.length > LINE_CHARS) return `${chars.slice(0, LINE_CHARS - 1).join("")}…`;
  }
  return line;
}
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
  const failNamed = new Set(clean.map(l => FAIL_NAMES.exec(l)?.[1]).filter(n => n !== undefined));
  const matters: string[] = [];
  for (const line of clean) {
    const said = cut(line.trim());
    if (said === "" || !MATTERS.test(line) || matters.includes(said) || Object.keys(tally(line)).length > 0) continue;
    const mark = MARK_NAMES.exec(line)?.[1];
    if (mark !== undefined && failNamed.has(mark)) continue;
    matters.push(said);
    if (matters.length === SLATE_SUMMARY_LINES) break;
  }
  const end = [...clean].reverse().find(l => l.trim() !== "")?.trim();
  const last = end === undefined ? undefined : cut(end);
  return { ...counts, matters, ...(last !== undefined ? { last } : {}) };
}

/** The counts as words, failed first and only when some did: "2 failed", "191 passed", "43 skipped". */
export function slateSummaryCounts(s: SlateRunSummary): { count: Count; words: string }[] {
  return (["failed", "passed", "skipped"] as const).filter(k => s[k] !== undefined && s[k]! > 0).map(k => ({ count: k, words: `${s[k]!.toLocaleString("en-US")} ${k}` }));
}
