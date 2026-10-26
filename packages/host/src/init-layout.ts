// SPDX-License-Identifier: AGPL-3.0-only
// The few rules the terminal's prompts and tables share: cut to a width with an
// ellipsis, pad cells into aligned columns, the bar down the left of the screen
// being answered, the help line, and the confirm and password prompts drawn
// with those same rules.
import type { Readable, Writable } from "node:stream";
import { WriteStream } from "node:tty";
import { stripVTControlCharacters, styleText } from "node:util";
import { ConfirmPrompt, PasswordPrompt, type State as PromptState } from "@clack/core";
import { S_BAR, S_RADIO_ACTIVE, S_RADIO_INACTIVE, S_STEP_ACTIVE, S_STEP_CANCEL, S_STEP_SUBMIT, unicode } from "@clack/prompts";

export const GUTTER = "  ";
/** The bar and its end on the screen being answered; finished screens keep clack's thin ones. Both one cell wide, so focus moving never shifts a column. */
export const S_BAR_FOCUS = unicode ? "┃" : "|";
export const S_BAR_FOCUS_END = unicode ? "┗" : "+";

const GRAPHEMES = new Intl.Segmenter();
/** Code points a terminal draws two cells wide: Hangul, CJK, kana, fullwidth forms and the emoji blocks. */
const WIDE = /^[\u1100-\u115F\u2E80-\u303E\u3041-\u33FF\u3400-\u4DBF\u4E00-\u9FFF\uA000-\uA4CF\uAC00-\uD7A3\uF900-\uFAFF\uFE30-\uFE4F\uFF00-\uFF60\uFFE0-\uFFE6\u{1F300}-\u{1F64F}\u{1F900}-\u{1F9FF}\u{20000}-\u{3FFFD}]/u;
const graphemeCells = (g: string): number => (WIDE.test(g) || /\p{Emoji_Presentation}|\uFE0F/u.test(g) ? 2 : /^[\p{Mn}\p{Me}\p{Cf}]/u.test(g) ? 0 : 1);

/** The cells a terminal gives text: a wide character or an emoji two, a combining mark none. */
export function cellWidth(text: string): number {
  let n = 0;
  for (const { segment } of GRAPHEMES.segment(text)) n += graphemeCells(segment);
  return n;
}

/** The text cut to fit width cells, ending in an ellipsis when anything was dropped; a character is kept or dropped
 * whole, never torn. */
export function ellipsize(text: string, width: number): string {
  if (width <= 0) return "";
  if (cellWidth(text) <= width) return text;
  let kept = "";
  let used = 0;
  for (const { segment } of GRAPHEMES.segment(text)) {
    const n = graphemeCells(segment);
    if (used + n > width - 1) break;
    kept += segment;
    used += n;
  }
  return `${kept}…`;
}

/** The text padded with spaces to width cells. */
export const padCells = (text: string, width: number, align: Align = "left"): string => {
  const pad = " ".repeat(Math.max(0, width - cellWidth(text)));
  return align === "right" ? `${pad}${text}` : `${text}${pad}`;
};

export type Align = "left" | "right";

/** The terminal width when the stream knows it, else clack's 80. Capped so a wide window does not spread the columns. */
export function widthOf(output: Writable | undefined, cap = 100): number {
  const columns = output !== undefined && "columns" in output && typeof output.columns === "number" ? output.columns : 80;
  return Math.min(columns, cap);
}

/** The line broken at word ends to fit the width, the rest indented (by two unless the caller says); a line that fits is left
 * as it is, a word longer than the width is cut on its own and the words after it go on. */
export function wrap(text: string, width: number, indent = "  "): string[] {
  if (text.length <= width) return [text];
  const lead = text.length - text.trimStart().length;
  let cut = text.lastIndexOf(" ", width);
  if (cut < lead) cut = text.indexOf(" ", lead);
  if (cut < 0) return [ellipsize(text, width)];
  return [ellipsize(text.slice(0, cut).trimEnd(), width), ...wrap(`${indent}${text.slice(cut + 1).trimStart()}`, width, indent)];
}

/** The cells a drawn row takes on screen: its text without the colour codes. */
export const cells = (row: string): number => stripVTControlCharacters(row).length;

/** Back to the first row and column of a block already on screen, with everything from there cleared: what a block
 * that redraws in place writes before its next frame. `drawn` is the cells each row of the last frame took, as
 * `cells` counts them. A terminal narrowed under the block reflows every row wider than it now is onto more rows,
 * so the count is taken at the width the terminal has at this moment and not at the one the frame was drawn to. */
export function rewind(drawn: readonly number[], columns: number): string {
  const rows = drawn.reduce((n, w) => n + Math.max(1, Math.ceil(w / columns)), 0);
  return rows > 0 ? `\x1b[${rows}A\x1b[G\x1b[J` : "";
}

/** Whether the stream is a terminal. */
export const isTTY = (output: Writable | undefined): boolean => output !== undefined && "isTTY" in output && output.isTTY === true;

/** Colour depth in bits for what this process draws: 1 (none) off a terminal and when the env says so (NO_COLOR, TERM=dumb), else Node's reading of TERM and COLORTERM; FORCE_COLOR wins, as it does for styleText. */
export function colourDepth(tty: boolean, env: NodeJS.ProcessEnv = process.env): number {
  if (!tty && env["FORCE_COLOR"] === undefined) return 1;
  return WriteStream.prototype.getColorDepth(env);
}

/** One 256-colour grey around text; written only where colourDepth says the terminal has 256 colours (8 bits) or more. */
export const grey = (n: number, s: string): string => `\x1b[38;5;${n}m${s}\x1b[39m`;
/** The greys the screens paint from, brightest first. From the middle of the ramp, so they read on dark and light
 * backgrounds alike; a key sits a step brighter than what it does, a heavy size a step brighter than the rest. */
export const GREY = { bright: 247, mid: 243, dim: 239 } as const;
const GAP = "   ";

export interface HelpKey {
  key: string;
  does: string;
}

/** Text that stands back from what it sits beside, at a colour depth: a grey from 256 colours up, dim at 16, plain
 * at none. */
export const muted = (s: string, depth: number): string => (depth <= 1 ? s : depth < 8 ? styleText("dim", s) : grey(GREY.mid, s));

/** The help line under a screen at a colour depth: keys in one grey and what they do in a dimmer one from 256 colours up, plain keys and dim words at 16, plain text at none; entries apart by three spaces. */
export function helpLine(keys: readonly HelpKey[], depth: number): string {
  if (depth <= 1) return keys.map(k => `${k.key} ${k.does}`).join(GAP);
  const key = depth < 8 ? (s: string) => s : (s: string) => grey(GREY.bright, s);
  return keys.map(k => `${key(k.key)} ${muted(k.does, depth)}`).join(muted(GAP, depth));
}

/** The columns a card's bar and its two spaces take before each line. clack's log.message writes lines as they are
 * (its note is what wraps, at the columns minus 6), so a line kept inside widthOf minus this is never wrapped again. */
export const CARD_FRAME = 3;

const dim = (s: string): string => styleText("dim", s);
const MASK = unicode ? "▪" : "*";
const CONFIRM_KEYS: readonly HelpKey[] = [{ key: "← →", does: "change" }, { key: "y n", does: "answer" }, { key: "enter", does: "choose" }, { key: "esc", does: "cancel" }];
/** The help line under a prompt that is typed into, secret or not. */
const ENTER_KEYS: readonly HelpKey[] = [{ key: "enter", does: "next" }, { key: "esc", does: "cancel" }];

export interface PromptOptions {
  /** The question: cyan on the step glyph while the prompt is being answered, plain once it is done. */
  message: string;
  /** Lines under the question, dim. In both, a newline separates lines and long ones wrap to the width. */
  hint?: string;
  input?: Readable;
  output?: Writable;
  /** Ends the prompt from outside, as a cancel: the question stopped meaning something before it was answered. It
   * hands the terminal back and settles, which is what a caller waiting on the prompt needs it to do. */
  signal?: AbortSignal;
}

/** The frame the two prompts share: the question, the hint, then the body rows down the bar; the screen being answered takes the thick bar, the cyan question and the help line, a finished one clack's thin bar. */
function promptFrame(state: PromptState, o: PromptOptions, body: readonly string[], keys: readonly HelpKey[]): string {
  const output = o.output ?? process.stdout;
  const width = widthOf(output) - CARD_FRAME;
  const open = state !== "submit" && state !== "cancel";
  const bar = dim(open ? S_BAR_FOCUS : S_BAR);
  const glyph = state === "submit" ? styleText("green", S_STEP_SUBMIT) : state === "cancel" ? styleText("red", S_STEP_CANCEL) : styleText("cyan", S_STEP_ACTIVE);
  const lines = o.message.split("\n").flatMap(m => wrap(m, width)).map((l, i) => `${i === 0 ? glyph : bar}  ${open ? styleText("cyan", l) : l}`);
  for (const h of o.hint === undefined ? [] : o.hint.split("\n")) for (const l of wrap(h, width)) lines.push(`${bar}  ${dim(l)}`);
  for (const b of body) lines.push(`${bar}  ${b}`);
  if (open) lines.push(`${dim(S_BAR_FOCUS_END)}  ${helpLine(keys, colourDepth(isTTY(output)))}`);
  return lines.join("\n");
}

const streamsOf = (o: PromptOptions) => ({ ...(o.input ? { input: o.input } : {}), ...(o.output ? { output: o.output } : {}), ...(o.signal ? { signal: o.signal } : {}) });

/** A yes or no question in the frame; the marker starts on No unless told otherwise, arrows move it, y and n answer, esc and ctrl-c cancel. */
export async function confirmPrompt(o: PromptOptions & { initialValue?: boolean }): Promise<boolean | symbol> {
  const yes = "Yes";
  const no = "No";
  const answer = (label: string, on: boolean): string => (on ? `${styleText("cyan", S_RADIO_ACTIVE)} ${label}` : `${dim(S_RADIO_INACTIVE)} ${dim(label)}`);
  const prompt = new ConfirmPrompt({
    active: yes,
    inactive: no,
    initialValue: o.initialValue ?? false,
    ...streamsOf(o),
    render() {
      const on = this.value === true;
      const picked = on ? yes : no;
      const body = this.state === "submit" ? [dim(picked)] : this.state === "cancel" ? [styleText(["strikethrough", "dim"], picked)] : [`${answer(yes, on)} ${dim("/")} ${answer(no, !on)}`];
      return promptFrame(this.state, o, body, CONFIRM_KEYS);
    },
  });
  // clack sends the escape key through as a cursor action too, which flips the marker before the cancel is drawn; this runs after clack's own listener and flips it back.
  prompt.on("cursor", action => {
    if (action === "cancel") prompt.value = prompt.value !== true;
  });
  // The prompt holds a boolean from construction on; the undefined in clack's type never comes back.
  return (await prompt.prompt()) as boolean | symbol;
}

/** A secret typed in the frame: every character drawn as the mask, the text itself never written to the output. */
export async function passwordPrompt(o: PromptOptions): Promise<string | symbol> {
  const prompt = new PasswordPrompt({
    mask: MASK,
    ...streamsOf(o),
    render() {
      const body = this.state === "submit" || this.state === "cancel" ? (this.masked === "" ? [] : [styleText(this.state === "submit" ? "dim" : ["strikethrough", "dim"], this.masked)]) : [this.userInputWithCursor];
      return promptFrame(this.state, o, body, ENTER_KEYS);
    },
  });
  // An untouched prompt settles to "" before it resolves; the undefined in clack's type never comes back.
  return (await prompt.prompt()) as string | symbol;
}
