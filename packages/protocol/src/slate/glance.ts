// SPDX-License-Identifier: AGPL-3.0-only
// Whether a slate reads at a glance: the person glances at it beside the chat, so a slate heavy with words,
// open sections, tables, logs and rows that all say the same is warned back to the agent in its sketch, against the
// bounds in SLATE_GLANCE. Read off what the person sees now: hidden pieces and folded sections' children count for
// nothing.
import { evaluateSlateExpression, resolveSlateProp, slatePropDependencies, slateTruthy, type SlateEvalContext } from "./expr.js";
import { SLATE_PIECES } from "./kit.js";
import { SLATE_GLANCE, SLATE_LIMITS } from "./limits.js";
import { SLATE_LOG_LINE as LOG_LINE } from "./output.js";
import { parseSlateOwnPath, slateEqual } from "./paths.js";
import { slateProblem } from "./problems.js";
import { isSlateBinding, type SlateDoc, type SlateJson, type SlatePiece, type SlateProblem, type SlatePropValue } from "./types.js";

/** Item and row fields whose words the person reads as a note or a cell. */
const CELL_FIELDS: ReadonlySet<string> = new Set(["note", "title", "name", "value", "label", "window"]);
/** A run's raw result, which a slate shows through json fields instead. */
const RAW_FIELDS: ReadonlySet<string> = new Set(["out", "err", "lines"]);
/** A table's ruler line, dashes with spaces, pipes or plus signs between. */
const RULER = /^[\s|+:]*-{3,}[\s|+:-]*$/;

/** Pieces read as words or by eye, off the kit's own entries; layout, controls and heads are neither. */
const readsAs = (type: string): string | undefined => SLATE_PIECES[type]?.reads;
const WORDS = { has: (type: string): boolean => readsAs(type) === "words" };
const SEEN = { has: (type: string): boolean => readsAs(type) === "eye" };
/** A piece whose value is prose, held to a paragraph's bound rather than a cell's. */
const PROSE = { has: (type: string): boolean => readsAs(type) === "words" && SLATE_PIECES[type]?.textProp === "value" };
/** A piece that folds its children away (a section), one that lays rows in columns (a table), one that shows a run's log. */
const folds = (type: string): boolean => SLATE_PIECES[type]?.collapsed !== undefined;
const columned = (type: string): boolean => Object.values(SLATE_PIECES[type]?.items ?? {}).some(i => i.prop === "columns");
const showsLog = (type: string): boolean => Object.values(SLATE_PIECES[type]?.props ?? {}).some(p => p.names === "run");

/** Words that name a state, the way a status column reads: a row's state is its first word, marks and figures aside,
 * so "✓ passed (42)" and "passed in 3s" both read passed. */
const STATE_WORDS: ReadonlySet<string> = new Set([
  "pass", "passed", "passing", "ok", "success", "succeeded", "green", "done", "complete", "completed", "fail", "failed", "failing", "error",
  "red", "running", "pending", "queued", "waiting", "skipped", "ready", "healthy", "unhealthy", "up", "down", "live", "merged", "open",
  "closed", "blocked", "todo", "idle", "active", "good", "bad", "clean",
]);
const stateWord = (v: SlateJson | undefined): string | undefined => {
  if (typeof v !== "string" && typeof v !== "boolean") return undefined;
  const first = String(v).toLowerCase().replace(/[^\p{L}\s]/gu, " ").trim().split(/\s+/)[0];
  return first !== undefined && STATE_WORDS.has(first) ? first : undefined;
};

const words = (text: string): number => text.split(/\s+/).filter(w => /[\p{L}\p{N}]/u.test(w)).length;

/** A text drawn as a table of characters: a ruler under its head, or three lines or more each split in columns by
 * runs of spaces; its column count read off its widest line. */
function asciiColumns(text: string): number | undefined {
  const lines = text.split("\n").filter(l => l.trim() !== "");
  if (lines.length < 2) return undefined;
  const cells = (l: string): number => l.trim().split(/\s{2,}|\s*\|\s*/).filter(c => c !== "").length;
  const ruled = lines.some(l => RULER.test(l));
  const gridded = lines.length >= 3 && lines.every(l => cells(l) >= 3);
  return ruled || gridded ? Math.max(...lines.filter(l => !RULER.test(l)).map(cells)) : undefined;
}

interface Said { piece: string; prop: string; value: string; row?: number }

/** The rows a check reads of a list: a table stops drawing at SLATE_LIMITS.rowsDrawn, so none reads past them; any
 * other list is read to SLATE_GLANCE.rowsRead, which keeps a 5,000-row checklist's sketch quick. */
const seenRows = (type: string, items: SlateJson | undefined): SlateJson[] =>
  !Array.isArray(items) ? [] : items.slice(0, columned(type) ? SLATE_LIMITS.rowsDrawn : SLATE_GLANCE.rowsRead);

/** Warnings for a slate the person cannot take in at a glance; `sketchWords` counts the words of its sketch's lines. */
export function slateGlanceProblems(doc: SlateDoc, read: SlateEvalContext, sketchWords: number): SlateProblem[] {
  const out: SlateProblem[] = [];
  const shown = (piece: SlatePiece): boolean => piece.when === undefined || slateTruthy(evaluateSlateExpression(piece.when, read));
  const prop = (piece: SlatePiece, name: string, ctx: SlateEvalContext = read): SlateJson | undefined => {
    const raw = piece.props?.[name];
    return raw === undefined ? undefined : resolveSlateProp(raw, ctx);
  };
  const visible: string[] = [];
  /** Pieces inside a folded section: not read at a glance, but a log there is still on the slate. */
  const folded: string[] = [];
  const sections: { id: string; words: number; seen: number }[] = [];
  const tuck = (id: string, seen: Set<string>): void => {
    const piece = doc.pieces[id];
    if (piece === undefined || seen.has(id) || !shown(piece)) return;
    seen.add(id);
    folded.push(id);
    for (const child of piece.children ?? []) tuck(child, seen);
  };
  const walk = (id: string, section: number | undefined, seen: Set<string>): void => {
    const piece = doc.pieces[id];
    if (piece === undefined || seen.has(id) || !shown(piece)) return;
    seen.add(id);
    visible.push(id);
    const at = section !== undefined && sections[section] !== undefined ? sections[section] : undefined;
    if (at !== undefined) {
      if (WORDS.has(piece.type)) at.words++;
      if (SEEN.has(piece.type)) at.seen++;
    }
    if (folds(piece.type)) {
      if (prop(piece, "open") === false) {
        for (const child of piece.children ?? []) tuck(child, seen);
        return;
      }
      sections.push({ id, words: 0, seen: 0 });
      for (const child of piece.children ?? []) walk(child, sections.length - 1, seen);
      return;
    }
    for (const child of piece.children ?? []) walk(child, section, seen);
  };
  walk(doc.root, undefined, new Set());

  if (sections.length > SLATE_GLANCE.sectionsOpen) {
    out.push(slateProblem("W030", `${sections.length} sections are open (${sections.map(s => s.id).join(", ")}); at most ${SLATE_GLANCE.sectionsOpen} stay open at a glance: replace a section rather than add one beside it, and fold detail with collapsible open={false}`, { fix: `<section ... collapsible open={false}>` }));
  }
  const tables = visible.filter(id => columned(doc.pieces[id]!.type));
  if (tables.length > SLATE_GLANCE.tablesOpen) {
    out.push(slateProblem("W031", `${tables.length} tables are open (${tables.join(", ")}); at most ${SLATE_GLANCE.tablesOpen} reads at a glance: show a count with the exceptions (<progress>) or fold the rest into a collapsed section`, { piece: tables[1]!, fix: "<progress>" }));
  }

  // Every word the person reads in a note, a cell or a line, with where it came from.
  const sayings = (ids: readonly string[]): Said[] => {
    const said: Said[] = [];
    for (const id of ids) {
      const piece = doc.pieces[id]!;
      const module = SLATE_PIECES[piece.type];
      if (module === undefined) continue;
      const items = module.repeating === true ? prop(piece, "items") : undefined;
      const rows = seenRows(piece.type, items);
      for (const [name, spec] of Object.entries(module.props)) {
        if (spec.binds === "item" && CELL_FIELDS.has(name)) {
          rows.forEach((item, index) => { const v = prop(piece, name, { ...read, item, index }); if (typeof v === "string") said.push({ piece: id, prop: name, value: v, row: index }); });
        } else if (spec.binds !== "item" && Object.values(module.items).every(it => it.prop !== name)) {
          const v = prop(piece, name);
          if (typeof v === "string" && (name === "value" || name === "note" || name === "label" || name === "body")) said.push({ piece: id, prop: name, value: v });
        }
      }
      for (const [tag, item] of Object.entries(module.items)) {
        const list = piece.props?.[item.prop];
        if (!Array.isArray(list)) continue;
        for (const entry of list as SlatePropValue[]) {
          if (entry === null || typeof entry !== "object" || Array.isArray(entry) || isSlateBinding(entry)) continue;
          for (const [field, raw] of Object.entries(entry as Record<string, SlatePropValue>)) {
            if (!CELL_FIELDS.has(field) || (tag === "col" && field === "title")) continue;
            if (item.row === true) rows.forEach((row, index) => { const v = resolveSlateProp(raw, { ...read, item: row, index }); if (typeof v === "string") said.push({ piece: id, prop: `${tag}.${field}`, value: v, row: index }); });
            else { const v = resolveSlateProp(raw, read); if (typeof v === "string") said.push({ piece: id, prop: `${tag}.${field}`, value: v }); }
          }
        }
      }
    }
    return said;
  };
  const said = sayings(visible);

  const long = new Map<string, { said: Said; count: number }>();
  for (const s of said) {
    const type = doc.pieces[s.piece]!.type;
    const prose = PROSE.has(type) && s.prop === "value";
    const bound = prose ? SLATE_GLANCE.textWords : SLATE_GLANCE.cellWords;
    if (words(s.value) <= bound || asciiColumns(s.value) !== undefined || LOG_LINE.test(s.value)) continue;
    const key = `${s.piece} ${s.prop}`;
    const had = long.get(key);
    if (had === undefined) long.set(key, { said: s, count: 1 });
    else had.count++;
  }
  // Named one by one up to a few, then counted, so the warnings do not become the wall they warn of.
  const named = [...long.values()];
  const more = named.splice(4);
  for (const { said: s, count } of named) {
    const prose = PROSE.has(doc.pieces[s.piece]!.type) && s.prop === "value";
    const where = s.row !== undefined ? ` on ${count} row${count === 1 ? "" : "s"}` : "";
    out.push(slateProblem("W033", prose
      ? `${s.prop} is a paragraph of ${words(s.value)} words; past ${SLATE_GLANCE.textWords} the words belong in the chat, and the slate shows the one line or figure they lead to`
      : `${s.prop} runs to ${words(s.value)} words${where}, like "${s.value.slice(0, 60)}${s.value.length > 60 ? "…" : ""}"; a note or a cell keeps to ${SLATE_GLANCE.cellWords} words, read at a glance, and the rest goes in the chat`, { piece: s.piece, prop: s.prop }));
  }

  if (more.length > 0) out.push(slateProblem("W033", `and ${more.length} more notes, cells or paragraphs past their bound: ${more.map(m => `${m.said.piece}.${m.said.prop}`).join(", ")}`));

  // Raw output is warned wherever it is drawn, in a folded section too: an <output> is its one home, summary first.
  const logged = new Set<string>();
  for (const id of [...visible, ...folded]) {
    const piece = doc.pieces[id]!;
    if (showsLog(piece.type)) continue;
    const raw = Object.values(piece.props ?? {}).flatMap(v => slatePropDependencies(v)).find(path => {
      const own = parseSlateOwnPath(path);
      const first = own?.segs[0];
      return own !== undefined && doc.runs[own.name] !== undefined && typeof first === "string" && RAW_FIELDS.has(first);
    });
    if (raw !== undefined) { logged.add(id); out.push(slateProblem("W034", `${id} shows ${raw}, a run's raw output; show the one figure or word it means through the run's json, and leave the log to an <output>, which leads with what passed and failed and folds the raw lines`, { piece: id, fix: `<output run={$${raw.slice(1).split(/[.[]/)[0]}} />` })); }
  }
  for (const s of [...said, ...sayings(folded)]) {
    if (logged.has(s.piece) || !LOG_LINE.test(s.value)) continue;
    logged.add(s.piece);
    out.push(slateProblem("W034", `${s.prop} reads like a process's raw output ("${s.value.split("\n").find(l => LOG_LINE.test(l))!.trim().slice(0, 50)}"); show what it means, a state or a figure, and leave the log to an <output> of the run that printed it`, { piece: s.piece, prop: s.prop }));
  }

  const ascii = new Set<string>();
  for (const s of said) {
    const cols = asciiColumns(s.value);
    if (cols === undefined || ascii.has(s.piece)) continue;
    ascii.add(s.piece);
    out.push(slateProblem("W035", `${s.prop} is a table drawn in characters, ${cols} columns wide, which wraps into noise in the panel; give its rows to a <table>, or its figures to <stats>`, { piece: s.piece, prop: s.prop, fix: "<table items={...}>" }));
    if (cols > SLATE_GLANCE.columns) out.push(slateProblem("W032", `${s.piece}'s table has ${cols} columns; at most ${SLATE_GLANCE.columns} read at a glance in the panel: keep the columns that answer the question, the rest go in the chat`, { piece: s.piece, prop: s.prop }));
  }
  for (const id of tables) {
    const cols = (doc.pieces[id]!.props?.["columns"] as SlatePropValue[] | undefined) ?? [];
    if (cols.length > SLATE_GLANCE.columns) out.push(slateProblem("W032", `${id} has ${cols.length} columns; at most ${SLATE_GLANCE.columns} read at a glance in the panel: keep the columns that answer the question, the rest go in the chat`, { piece: id, prop: "columns" }));
    const items = prop(doc.pieces[id]!, "items");
    const rows = seenRows(doc.pieces[id]!.type, items);
    const of = Array.isArray(items) && items.length > rows.length ? ` that draw, of ${items.length}` : "";
    if (rows.length < SLATE_GLANCE.sameRows) continue;
    for (const col of cols) {
      if (col === null || typeof col !== "object" || Array.isArray(col)) continue;
      const c = col as Record<string, SlatePropValue>;
      // What a field reads on every row, read row by row and given up at the first row that differs.
      const every = <T>(name: string, as: (v: SlateJson) => T | undefined): { v: T } | undefined => {
        if (c[name] === undefined) return undefined;
        let first: T | undefined;
        for (const [index, item] of rows.entries()) {
          const v = as(resolveSlateProp(c[name]!, { ...read, item, index }) ?? null);
          if (v === undefined || (index > 0 && !slateEqual(v as SlateJson, first as SlateJson))) return undefined;
          if (index === 0) first = v;
        }
        return { v: first as T };
      };
      const title = typeof c["title"] === "string" ? c["title"] : "a column";
      const value = every("value", v => v);
      const state = value === undefined ? every("value", stateWord) : undefined;
      // Muted ink names no state, so a column muted on every row is only a quieter one.
      const tone = value === undefined && state === undefined ? every("tone", v => (v === null || v === "default" || v === "muted" ? undefined : v)) : undefined;
      if (value !== undefined) out.push(slateProblem("W036", `${title} reads ${JSON.stringify(value.v)} on all ${rows.length} rows of ${id}${of}, which says nothing row by row; say it once above the table, or show only the rows that differ`, { piece: id, prop: "columns" }));
      else if (state !== undefined) out.push(slateProblem("W036", `${title} says ${state.v} on all ${rows.length} rows of ${id}${of}: a state the same on every row says nothing; say it once as a count, <progress> or <stats>, and list only the rows that differ`, { piece: id, prop: "columns", fix: "<progress>" }));
      // A tone on every row is most often a list of one state's rows already (the failures alone): the rows stay, the tone goes.
      else if (tone !== undefined) out.push(slateProblem("W036", `${title} is ${String(tone.v)} on all ${rows.length} rows of ${id}${of}: a tone the same on every row says nothing; drop the tone, and name the state once in the section's title or a count above`, { piece: id, prop: "columns" }));
    }
  }

  for (const id of visible) {
    const piece = doc.pieces[id]!;
    const module = SLATE_PIECES[piece.type];
    // Label and value pairs (facts, a stat row) that all name one state.
    for (const item of Object.values(module?.items ?? {})) {
      if (item.row === true || item.fields["label"] === undefined || item.fields["value"] === undefined) continue;
      const given = piece.props?.[item.prop];
      const pairs = (Array.isArray(given) ? given : []).filter((f): f is Record<string, SlatePropValue> => f !== null && typeof f === "object" && !Array.isArray(f) && !isSlateBinding(f));
      const states = pairs.map(f => (f["value"] === undefined ? undefined : stateWord(resolveSlateProp(f["value"], read))));
      if (pairs.length >= SLATE_GLANCE.sameRows && states[0] !== undefined && states.every(s => s === states[0])) out.push(slateProblem("W036", `all ${pairs.length} lines of ${id} say ${states[0]}: a state the same on every line says nothing; say it once as a count, <progress> or <stats>, and list only what differs`, { piece: id, prop: item.prop, fix: "<progress>" }));
    }
    if (module?.ticks === true && prop(piece, "editable") !== true) {
      const given = prop(piece, "items");
      // Every row, since the claim is that all are done, but `every` stops at the first row that is not.
      const rows = Array.isArray(given) ? given : [];
      const done = rows.length >= SLATE_GLANCE.sameRows && rows.every((item, index) => prop(piece, "done", { ...read, item, index }) === true) ? rows : [];
      if (done.length > 0) out.push(slateProblem("W036", `every row of ${id} is done, all ${done.length}; a tick on every row says nothing: say it once, "All ${done.length} done", or a <progress> that lists only the rows not done`, { piece: id, fix: "<progress>" }));
    }
  }

  if (sketchWords > SLATE_GLANCE.sketchWords) {
    out.push(slateProblem("W037", `the sketch runs to ${sketchWords} words, past the ${SLATE_GLANCE.sketchWords} a glance takes in; the slate is a companion, not a record: keep what the person checks while you work, fold or drop the rest`));
  }
  const heavy = (w: number, s: number): boolean => w > 3 && w > SLATE_GLANCE.textPerVisual * s;
  const allWords = visible.filter(id => WORDS.has(doc.pieces[id]!.type)).length;
  const allSeen = visible.filter(id => SEEN.has(doc.pieces[id]!.type)).length;
  if (heavy(allWords, allSeen)) {
    out.push(slateProblem("W038", `${allWords} pieces of words against ${allSeen} the person reads by eye; past ${SLATE_GLANCE.textPerVisual} to 1 a slate is a second reply: turn counts into <stats> or <progress>, questions into <decision>, and leave the prose in the chat`));
  }
  for (const s of sections) {
    if (heavy(s.words, s.seen)) out.push(slateProblem("W038", `section ${s.id} holds ${s.words} pieces of words and ${s.seen} to read by eye; show its point as a figure, a count or a chart, or fold it`, { piece: s.id }));
  }
  return out;
}
