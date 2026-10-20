// SPDX-License-Identifier: AGPL-3.0-only
// The transcripts the bench's threads hold, the same on every run. Each turn is the person's short message, the
// agent's tool calls with their output where it made any, and the agent's final reply: markdown prose, a quarter of
// them with a fenced code block under it. Both apps fold a finished turn's calls behind one "Worked for" line and show
// the message and the reply, so a thread of n turns shows 2n messages. `real` is shaped like a person's long thread
// instead (real.mjs): screenshots, long pastes, long tool output, subagents, code in many languages, tables.
import { realTurns, rng } from "./real.mjs";

/** The host keeps 5,000 events of a thread and drops the oldest past that (TRANSCRIPT_CAP); a turn is 5 events and a
 * tool call 2 more, so `long` is the most turns that fit whole with some calls among them: 900 * 5 + 200 * 2 = 4,900. */
export const THREADS = {
  long: { turns: 900, calls: 200, seed: 1 },
  "short-1": { turns: 10, calls: 15, seed: 2 },
  "short-2": { turns: 10, calls: 15, seed: 3 },
  "short-3": { turns: 10, calls: 15, seed: 4 },
  /** Shares of turns: `pastes` and `shots` of person messages, `writeUps`, `tables` and `code` of replies; `calls` is
   * the mean tool calls a turn, `tasks` the turns that also launch a subagent, `changes` the turns that change 100 to
   * 140 files of the folder. */
  real: { shape: "real", turns: 220, calls: 3, tasks: 16, changes: 6, pastes: 0.06, shots: 0.08, writeUps: 0.02, tables: 0.08, code: 0.3, seed: 5 },
};
export const EVENTS_PER_TURN = 5;
export const EVENTS_PER_CALL = 2;

/** The share of final replies that carry a fenced code block under their prose. */
export const CODE_SHARE = 0.25;

const IDENTS = ["HostClient", "threadMessages", "LegendList", "scrollTop", "useChatThread", "session.delta", "toolUseId", "HISTORY_PAGE_EVENTS", "Throwaway", "state.json", "host.lock", "openHost", "Window::refresh", "uniform_list", "px_per_s", "ThreadView", "sessions.history", "messageId", "Live::Event", "flattenContent"];
const FILES = ["apps/web/src/components/chat/timeline/list.tsx", "packages/protocol/src/thread-read.ts", "apps/desktop/src/host-lifecycle.ts", "apps/desktop-gpui/host-client/src/lib.rs", "packages/host/src/verbs/turns-help.ts", "packages/adapter-claude/src/adapter.ts", "apps/desktop-gpui/app/src/main.rs", "packages/runtime/src/session.ts"];
const OPENERS = ["The list keeps", "I checked how", "Reading the adapter,", "The host answers", "So the scroller", "Next I traced", "That leaves", "The test shows", "Looking at the log,", "The page fold"];
const MIDDLES = ["every row it has measured", "the transcript position it was held through", "one page of 200 events at a time", "the newest window before older ones", "the offset from elapsed time", "a fresh state file in a temp folder", "the tool call's whole input", "each message id as its own row"];
const ENDS = ["so nothing jumps on resize.", "which is why the count matched.", "and the second read is a no-op.", "before the first frame is painted.", "so a stale page reloads once.", "and the old value is dropped."];
const LIST_HEADS = ["Three places read it:", "What changed:", "The callers:", "Left to check:", "Where it goes wrong:"];
const LANGS = ["rust", "ts", "sh"];
const PROMPTS = ["Look at why the transcript jumps when a long tool output lands and fix it.", "Can you make the sidebar keep its scroll when a thread moves to the top?", "Add a test that the paging stops at the first event.", "The switch between threads feels slow, find where the time goes.", "Wire the new row type through the adapter and the read.", "Tidy the scroller code you just touched, keep behaviour the same.", "Why does the host send the head twice on reconnect?", "Make the code blocks wrap instead of scrolling sideways."];

const pick = (r, list) => list[Math.floor(r() * list.length)];
const between = (r, lo, hi) => lo + Math.floor(r() * (hi - lo + 1));

function sentence(r) {
  return `${pick(r, OPENERS)} \`${pick(r, IDENTS)}\` holds ${pick(r, MIDDLES)}, ${pick(r, ENDS)}`;
}

/** Markdown prose of 2 to 8 lines: sentences, sometimes a list, inline code throughout. */
function prose(r) {
  const n = between(r, 2, 8);
  const lines = [];
  while (lines.length < n) {
    if (lines.length > 0 && n - lines.length >= 3 && r() < 0.35) {
      const items = between(r, 2, Math.min(4, n - lines.length - 1));
      lines.push(pick(r, LIST_HEADS));
      for (let i = 0; i < items; i++) lines.push(`- \`${pick(r, IDENTS)}\` in \`${pick(r, FILES)}\` ${pick(r, ENDS)}`);
    } else {
      lines.push(sentence(r));
    }
  }
  return lines.join("\n");
}

function codeLine(r, lang, i) {
  const id = pick(r, IDENTS).replace(/[^A-Za-z_]/g, "_").toLowerCase();
  if (lang !== "sh" && i % 7 === 6) return "}";
  if (lang === "rust") return i % 7 === 0 ? `fn ${id}_${i}(cx: &mut Context<Self>) -> usize {` : `    let ${id} = self.rows.get(${i}).map(|row| row.height).unwrap_or(${between(r, 12, 90)});`;
  if (lang === "ts") return i % 7 === 0 ? `export function ${id}${i}(rows: readonly Row[]): number {` : `  const ${id} = rows[${i}]?.height ?? ${between(r, 12, 90)};`;
  return `${pick(r, ["grep -n", "rg", "sed -n '1,40p'", "wc -l"])} ${pick(r, FILES)}`;
}

/** One fenced block of 5 to 40 lines. */
function code(r) {
  const lang = pick(r, LANGS);
  const n = between(r, 5, 40);
  return `\`\`\`${lang}\n${Array.from({ length: n }, (_, i) => codeLine(r, lang, i)).join("\n")}\n\`\`\``;
}

/** One tool call and its output of 1 to 30 lines. */
function tool(r) {
  const n = between(r, 1, 30);
  const file = pick(r, FILES);
  const kind = pick(r, ["Bash", "Read", "Grep", "Edit"]);
  const out = Array.from({ length: n }, (_, i) => (kind === "Bash" ? `${file}:${between(r, 1, 900)}: ${sentence(r)}` : `${String(i + 1).padStart(4)}\t${codeLine(r, "ts", i)}`)).join("\n");
  const input = kind === "Bash" ? { command: `rg -n ${pick(r, IDENTS)} ${file}`, description: "Find the uses" } : kind === "Read" ? { file_path: file } : kind === "Grep" ? { pattern: pick(r, IDENTS), path: file } : { file_path: file, old_string: codeLine(r, "ts", 1), new_string: codeLine(r, "ts", 2) };
  return { name: kind, input, output: kind === "Edit" ? `The file ${file} has been updated.` : out };
}

/** A thread's turns, `{ prompt, tools: [{ name, input, output }], reply }`: exactly CODE_SHARE of the replies carry
 * code, and the thread's calls fall on turns at random. */
export function turnsOf(name) {
  const plan = THREADS[name];
  if (plan === undefined) throw new Error(`no bench thread ${name}`);
  if (plan.shape === "real") return realTurns(plan);
  const r = rng(plan.seed);
  const coded = Array.from({ length: plan.turns }, (_, i) => i < Math.round(plan.turns * CODE_SHARE));
  for (let i = coded.length - 1; i > 0; i--) {
    const j = Math.floor(r() * (i + 1));
    [coded[i], coded[j]] = [coded[j], coded[i]];
  }
  const calls = Array(plan.turns).fill(0);
  for (let i = 0; i < plan.calls; i++) calls[Math.floor(r() * plan.turns)]++;
  return coded.map((withCode, t) => ({
    prompt: `${pick(r, PROMPTS)} (${name}, turn ${t + 1})`,
    tools: Array.from({ length: calls[t] }, () => tool(r)),
    reply: withCode ? `${prose(r)}\n\n${code(r)}` : prose(r),
  }));
}

/** The first line of the thread's last reply, which the app shows as soon as the thread is opened at its end. */
export function lastWords(name) {
  return turnsOf(name).at(-1).reply.split("\n")[0];
}

export function firstPrompt(name) {
  return turnsOf(name)[0].prompt;
}

/** The events a thread's turns record on the host: each turn's 5, each call's 2, a thinking block's 1, and a
 * subagent's launch, start, lines, calls, notification and answer. */
export function eventsOf(turns) {
  const callEvents = c => (c.children === undefined ? EVENTS_PER_CALL : 4 + c.children.reduce((n, k) => n + (k.text !== undefined ? 1 : EVENTS_PER_CALL), 0));
  return turns.reduce((n, t) => n + EVENTS_PER_TURN + (t.thinking !== undefined ? 1 : 0) + t.tools.reduce((m, c) => m + callEvents(c), 0), 0);
}

/** The transcript's lines as markdown source with every turn folded: each person message's and each reply's. Wrapping
 * draws more lines than this on a narrow window, and tool output opened by hand adds its own. */
export function renderedLines(turns) {
  return turns.reduce((n, t) => n + t.prompt.split("\n").length + t.reply.split("\n").length, 0);
}
