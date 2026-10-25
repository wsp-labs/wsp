// SPDX-License-Identifier: AGPL-3.0-only
// One screen of wsp init as a list: a sentence saying what it decides,
// searchable rows under group headings that carry their own counts and toggle
// as a unit, rows that always come along shown as bullets the cursor skips, a
// window sized to the terminal, and a detail pane for the highlighted item. A
// row is answered by a tick, or by one word cycled with the arrows where the
// screen gives it choices. Built on @clack/core so the frame diffing, raw mode
// and cancel handling are clack's; the keys and the layout are ours.
import type { Key } from "node:readline";
import { emitKeypressEvents } from "node:readline";
import type { Readable, Writable } from "node:stream";
import { styleText } from "node:util";
import { Prompt, isCancel } from "@clack/core";
import { S_BAR, S_STEP_ACTIVE, S_STEP_CANCEL, S_STEP_SUBMIT } from "@clack/prompts";
import { GUTTER, S_BAR_FOCUS, S_BAR_FOCUS_END, accent, colourDepth, ellipsize, helpLine, isTTY, rowsOf, summarize, viewport, widthOf, wrap, type Align, type HelpKey } from "./init-layout.js";

/** The 16-colour names the Disk line may take, in order of weight. */
export type Tone = "yellow" | "yellowBright" | "red";
/** A column's text with the paint it takes once it has been padded, so a colour never changes a column's width. */
export interface Cell {
  text: string;
  paint?: (padded: string) => string;
}
/** A column: a bare string is dim. */
export type Column = string | Cell;

export interface SelectItem {
  id: string;
  label: string;
  /** The dim second column: a size, a state word. */
  hint?: Column;
  /** The column between the label and the hint: why the row is on. Absent on every item, the column is not drawn. */
  why?: Column;
  group?: string;
  /** Lines for the detail pane while this item is highlighted; the reason a row is locked belongs here. */
  detail: string[];
  /** on: always ticked, shown as a bullet the cursor skips. off: never ticked. */
  lock?: "on" | "off";
  /** The words this row is answered with instead of a tick, cycled with the arrows; the answer is the second column. */
  choices?: readonly Choice[];
}

/** One answer a row can take: the value the screen reads back, and the words the row shows. */
export interface Choice {
  value: string;
  label: string;
}

export type Entry =
  | { type: "locked"; items: SelectItem[] }
  | { type: "bullet"; item: SelectItem }
  | { type: "more"; count: number }
  | { type: "group"; group: string; items: SelectItem[]; folded: boolean }
  | { type: "item"; item: SelectItem };

export interface RungAnswer {
  ticks: Set<string>;
  /** The answer of every choice row, by id. */
  answers: Map<string, string>;
}
/** The widths a frame's three columns settled on. */
interface Columns {
  label: number;
  why: number;
  second: number;
}

/** The bar, a space, the focus marker, a space before every row. */
const EDGE = 4;
const dim = (s: string): string => styleText("dim", s);
const textOf = (c: Column | undefined): string => (c === undefined ? "" : typeof c === "string" ? c : c.text);
export function matches(item: SelectItem, query: string): boolean {
  const q = query.trim().toLowerCase();
  return q === "" || item.label.toLowerCase().includes(q) || item.id.toLowerCase().includes(q);
}

export const focusable = (e: Entry): boolean => e.type === "group" || e.type === "item";

/** The nearest focusable index from `at`, looking in `dir` first and back the other way when that side has none. */
export function settle(entries: readonly Entry[], at: number, dir: 1 | -1 = 1): number {
  const bounded = Math.min(Math.max(0, at), Math.max(0, entries.length - 1));
  for (let i = bounded; i >= 0 && i < entries.length; i += dir) if (focusable(entries[i]!)) return i;
  for (let i = bounded; i >= 0 && i < entries.length; i -= dir) if (focusable(entries[i]!)) return i;
  return 0;
}

/** The answer a choice row stands on: what the screen was given, else its first choice. */
export const answerOf = (i: SelectItem, answers: ReadonlyMap<string, string>): string | undefined =>
  i.choices === undefined ? undefined : (answers.get(i.id) ?? i.choices[0]?.value);

/** The row's next choice in `dir`; the group cycles as one, from the answer its rows share or from the first. */
function cycle(a: RungAnswer, items: readonly SelectItem[], dir: 1 | -1): void {
  const rows = items.filter(i => i.choices !== undefined);
  const values = [...new Set(rows.map(i => answerOf(i, a.answers)))];
  for (const i of rows) {
    const choices = i.choices ?? [];
    const at = values.length === 1 ? choices.findIndex(c => c.value === values[0]) : -1;
    const next = choices[(at + dir + choices.length * 2) % choices.length];
    if (next !== undefined) a.answers.set(i.id, next.value);
  }
}

/** Cuts a label to width from the middle, keeping its tail: the part of a path that differs. */
function middle(text: string, width: number): string {
  if (width <= 0) return "";
  if (text.length <= width) return text;
  if (width === 1) return "…";
  const head = Math.floor((width - 1) / 2);
  return `${text.slice(0, head)}…${text.slice(text.length - (width - 1 - head))}`;
}

function commonPrefix(texts: readonly string[]): number {
  const first = texts[0] ?? "";
  let n = 0;
  while (n < first.length && texts.every(t => t[n] === first[n])) n += 1;
  return n;
}

/** One keypress, by name ("c", "return"); ctrl-c and escape resolve as "cancel". */
export function readKey(input: Readable, output: Writable, accept: readonly string[]): Promise<string> {
  return new Promise(resolve => {
    emitKeypressEvents(input);
    const tty = input as Readable & { isTTY?: boolean; setRawMode?: (on: boolean) => void };
    if (tty.isTTY && tty.setRawMode) tty.setRawMode(true);
    const done = (value: string): void => {
      input.off("keypress", onKey);
      if (tty.isTTY && tty.setRawMode) tty.setRawMode(false);
      input.pause();
      resolve(value);
    };
    const onKey = (char: string | undefined, key: Key): void => {
      if (key.sequence === "\x03" || key.name === "escape") return done("cancel");
      const name = key.name ?? char;
      if (name !== undefined && accept.includes(name)) done(name);
    };
    input.on("keypress", onKey);
    input.resume();
  });
}
