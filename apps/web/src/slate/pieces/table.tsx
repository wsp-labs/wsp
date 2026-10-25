// SPDX-License-Identifier: AGPL-3.0-only
// Rows from a list in the settings list grammar: a header over the card whose first cell names the list, the rows in
// the card on one column template shared with the tables beside it, the first text column the row's name taking the
// room, figures 12 px mono at the end, words 13 px muted. With more than two text columns a row folds to its title
// over a note of the rest. One action that every row takes makes the row open, ending in a chevron. Nothing is cut:
// long text wraps and the row grows. Rows are keyed by the piece's `key`, else their index.
import { SLATE_LIMITS, type SlateJson, type SlatePropValue } from "@wsp/protocol/slate";
import type { SlateStep } from "../model.js";
import { Button } from "../../components/ui/button.js";
import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";
import { cn } from "../../lib/utils.js";
import type { SlateEngine } from "../engine.js";
import type { PieceViewProps, PieceView } from "../SlateView.js";
import { truthy } from "../actions.js";
import { placeOf } from "./runs.js";
import { CARD_SURFACE } from "../../settings/rows.js";
import { isFigure, isSentence, NOTE, present, rowKeys, str, TONE_INK, toneOf } from "./look.js";
import { Outcome } from "./outcome.js";
import { usePress } from "./press.js";
import { pressTitle } from "./button.js";
import { useWidth } from "./chart.js";

type Template = Record<string, SlatePropValue>;
const records = (value: SlatePropValue | undefined): Template[] =>
  Array.isArray(value) ? value.filter((entry): entry is Template => entry !== null && typeof entry === "object" && !Array.isArray(entry)) : [];

type Row = { item: SlateJson; index: number };

/** The columns whose when holds, eight at most. */
const shownColumns = (slate: SlateEngine, raw: SlatePropValue | undefined): Template[] => present(slate, raw, records(raw)).slice(0, 8);

/** A figure as a cell shows it: a number, or text like 524 MB, 12%, $4.20 or 3 days. */

function RowAction({ id, template, at, row, slate, raise }: Pick<PieceViewProps, "id" | "slate" | "raise"> & { template: Template; at: number; row: Row }) {
  const on = template["on"] as unknown as { press?: SlateStep | SlateStep[] } | undefined;
  const { busy, said, refused, press } = usePress(() => raise("press", { row, rowAction: at, ...(on?.press !== undefined ? { actions: on.press } : {}) }));
  const label = str(slate.resolve(template["label"], row)) ?? "";
  const first = str(row.item !== null && typeof row.item === "object" && !Array.isArray(row.item) ? Object.values(row.item)[0] : row.item);
  return (
    <span className="inline-flex flex-col items-end gap-1">
      <Button data-slate-row-action={`${id}:${at}`} variant="outline" size="xs" disabled={busy} title={pressTitle(on?.press)} aria-label={first === undefined ? label : `${label}, ${first}`} onClick={press}>
        {label}
      </Button>
      <Outcome said={said} refused={refused} />
    </span>
  );
}

/** The widest a column stays on one line, in characters of the mono face; anything longer wraps. */
const TIGHT_CH = 26;

/** Each column's tight width in characters, or undefined for a column that wraps: a mono column with no sentence in
 * it, or one whose every cell is a figure, as long as its widest cell fits on one line. */
function tightWidths(slate: SlateEngine, id: string): (number | undefined)[] {
  const piece = slate.piece(id);
  if (piece === undefined) return [];
  const items = slate.resolve(piece.props?.["items"]);
  const list = Array.isArray(items) ? items : [];
  const shown = list.slice(0, drawnRows(slate.resolve(piece.props?.["rows"]), list.length));
  return shownColumns(slate, piece.props?.["columns"]).map(column => {
    const cells = shown.map((item, index) => slate.resolve(column["value"], { item, index }));
    const mono = column["mono"] === true && !cells.some(isSentence);
    if (!mono && (cells.length === 0 || !cells.every(isFigure))) return undefined;
    const widest = Math.max(str(slate.resolve(column["title"]))?.length ?? 0, ...cells.map(cell => str(cell)?.length ?? 0));
    return widest > TIGHT_CH ? undefined : widest;
  });
}

/** The widths this table's tight columns take so the tables beside it with as many columns line up: each column
 * the widest any of them needs. */
function sharedWidths(slate: SlateEngine, id: string, own: (number | undefined)[]): (number | undefined)[] {
  const fellows = slate.tablesBeside(id).filter(other => other !== id).map(other => tightWidths(slate, other)).filter(widths => widths.length === own.length);
  if (fellows.length === 0) return own;
  return own.map((width, at) => (width === undefined ? undefined : Math.max(width, ...fellows.map(widths => widths[at] ?? 0))));
}

/** The column that takes a table's room, the one of the longest words or machine text that is not all figures (else the
 * first), and its longest word in characters, the widest any table beside it with as many columns needs. The column
 * never narrows past that word, so a word wraps whole rather than break mid-word. */
function fillOf(slate: SlateEngine, id: string, beside = true): { at: number; word: number } {
  const piece = slate.piece(id);
  const items = slate.resolve(piece?.props?.["items"]);
  const list = Array.isArray(items) ? items : [];
  const shown = list.slice(0, drawnRows(slate.resolve(piece?.props?.["rows"]), list.length));
  const columns = shownColumns(slate, piece?.props?.["columns"]);
  const cells = columns.map(column => shown.map((item, index) => slate.resolve(column["value"], { item, index })));
  const longest = (at: number) => Math.max(str(slate.resolve(columns[at]?.["title"]))?.length ?? 0, ...cells[at]!.map(cell => str(cell)?.length ?? 0));
  const words = columns.map((_, at) => at).filter(at => !(cells[at]!.length > 0 && cells[at]!.every(isFigure)));
  const at = words.reduce<number | undefined>((best, at) => (best === undefined || longest(at) > longest(best) ? at : best), undefined) ?? 0;
  const word = Math.max(0, ...(cells[at] ?? []).flatMap(cell => (str(cell) ?? "").split(/\s+/).map(w => w.length)));
  const fellows = beside ? slate.tablesBeside(id).filter(other => other !== id && shownColumns(slate, slate.piece(other)?.props?.["columns"]).length === columns.length) : [];
  return { at, word: Math.max(word, ...fellows.map(other => fillOf(slate, other, false).word)) };
}

/** How many of a table's rows draw: what its rows prop asks for, never past SLATE_LIMITS.rowsDrawn; past it the table
 * says how many more there are rather than draw them all on every push. */
const drawnRows = (asked: SlateJson | undefined, length: number): number => Math.min(typeof asked === "number" ? Math.max(0, Math.floor(asked)) : length, SLATE_LIMITS.rowsDrawn);


const NAME = "text-sm leading-5 text-foreground";
const WORD = "text-[13px] leading-5 text-muted-foreground";
const MONO = "font-mono text-xs leading-5 tabular-nums";
/** One character of the 12 px mono face, the ch a tight column's width counts in. */
const MONO_CH = 7.2;
const INSET = "px-(--settings-inset,20px)";
const ROW = "min-h-11 py-3 transition-colors duration-150";
/** The chevron's button reaches over the whole row it stands in, so the row presses without its cells in a button. */
const OPENS = "grid cursor-pointer place-items-center outline-none after:absolute after:inset-0 focus-visible:after:ring-2 focus-visible:after:ring-ring focus-visible:after:ring-inset disabled:cursor-default";

/** A row that opens: the whole row presses the one action every row takes, through the chevron's button in its last
 * cell, which `children` places. */
function OpenRow({ id, template, row, raise, className, label, children }: Pick<PieceViewProps, "id" | "raise"> & { template: Template; row: Row; className: string; label: string; children: (opener: ReactNode) => ReactNode }) {
  const on = template["on"] as unknown as { press?: SlateStep | SlateStep[] } | undefined;
  const { busy, said, refused, press } = usePress(() => raise("press", { row, rowAction: 0, ...(on?.press !== undefined ? { actions: on.press } : {}) }));
  const opener = (
    <span role="cell" className="self-center">
      <button type="button" data-slate-row-open={`${id}:${row.index}`} disabled={busy} title={pressTitle(on?.press)} aria-label={label} onClick={press} className={OPENS}>
        <ChevronRight aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
      </button>
    </span>
  );
  return (
    <div role="row" className="col-span-full grid grid-cols-subgrid">
      <div className={cn(className, "relative hover:bg-accent/60")}>{children(opener)}</div>
      {said === undefined && refused === undefined ? null : <span className={cn("col-span-full pb-2", INSET)}><Outcome said={said} refused={refused} /></span>}
    </div>
  );
}

export const table: PieceView = {
  type: "table",
  card: false,
  aligns: true,
  rowScoped: ["columns", "rowActions", "key"],
  component: function TablePiece({ id, piece, props, slate, raise }) {
    const [ref, width] = useWidth();
    const items = Array.isArray(props["items"]) ? props["items"] : [];
    const columns = shownColumns(slate, piece.props?.["columns"]);
    const actions = records(piece.props?.["rowActions"]).slice(0, 3);
    const shown = items.slice(0, drawnRows(props["rows"], items.length));
    const keyProp = piece.props?.["key"];
    const keys = rowKeys(shown.map((item, index) => (keyProp === undefined ? undefined : slate.resolve(keyProp, { item, index }))));
    const place = placeOf(slate, id);
    const inset = place === "inside" ? "" : INSET;
    const card = place === "inside" ? "" : CARD_SURFACE;
    const widths = sharedWidths(slate, id, tightWidths(slate, id));
    const text = widths.flatMap((width, at) => (width === undefined ? [at] : []));
    const cellsOf = (at: number) => shown.map((item, index) => slate.resolve(columns[at]?.["value"], { item, index }));
    const figures = columns.map((_, at) => shown.length > 0 && cellsOf(at).every(isFigure));
    // The row's name: a mono first column of names (a path, an id) in its own face, so no column after it draws larger;
    // else the column most of whose cells are sentences, else the first text column. A cell led by a digit is a figure,
    // a date or a time, never a name.
    const monoFirst = columns[0]?.["mono"] === true && !figures[0] && !cellsOf(0).some(cell => isSentence(cell) || /^\d/.test(str(cell) ?? ""));
    const sentences = text.find(at => cellsOf(at).filter(isSentence).length * 2 > shown.length);
    const title = monoFirst ? 0 : (sentences ?? text[0] ?? 0);
    const titleMono = columns[title]?.["mono"] === true && !cellsOf(title).some(isSentence);
    const opens = actions.length === 1 && actions[0]!["when"] === undefined;
    const end = (column: Template, at: number) => at !== title && (column["align"] === "end" || (figures[at] === true && column["align"] === undefined));
    const header = (at: number) => str(slate.resolve(columns[at]?.["title"]));
    const tone = (column: Template, row: Row) => {
      const named = slate.resolve(column["tone"], row);
      return named === undefined ? undefined : TONE_INK[toneOf(named, slate, id)];
    };
    const actionCell = (row: Row, opener: ReactNode) =>
      opens ? (
        opener
      ) : actions.length === 0 ? null : (
        <span role="cell" className="inline-flex justify-end gap-1 self-center">
          {actions.map((template, at) =>
            template["when"] !== undefined && !truthy(slate.resolve({ bind: String(template["when"]) }, row)) ? null : <RowAction key={at} id={id} template={template} at={at} row={row} slate={slate} raise={raise} />,
          )}
        </span>
      );
    const below = (
      <>
        {items.length === 0 ? <p className={cn("flex min-h-11 items-center py-3", WORD, inset, card)}>{str(props["empty"]) ?? "Nothing here"}</p> : null}
        {items.length > shown.length ? <p className={NOTE}>and {items.length - shown.length} more</p> : null}
      </>
    );
    // The note says the words first, then the figures and dates.
    const noteOrder = [...text, ...columns.map((_, at) => at).filter(at => !text.includes(at))];
    // A table with no rows has nothing to fold: it stands as a grid, its header naming every column.
    // The tight columns at their widths and the filling column's longest word, with 16 px between columns and the inset
    // at each edge: where the table is narrower than that it folds, rather than break a word or spill out of its card.
    const { at: fillAt, word: fillWordCh } = fillOf(slate, id);
    const needed = (widths.reduce<number>((sum, w, at) => sum + (w === undefined || at === fillAt ? 0 : w), 0) + fillWordCh * 1.2) * MONO_CH + 16 * (columns.length - 1) + (opens || actions.length > 0 ? 56 : 0) + (place === "inside" ? 0 : 40);
    const cramped = width > 0 && needed > width;
    if ((text.length > 2 || cramped) && shown.length > 0) {
      // Folded: the name over a note of every other cell, figures and dates in mono, 12 px apart; a section's head names
      // the list, so the column's own header stands only where there is none.
      return (
        <div ref={ref} role="table" aria-label={header(title)} data-slate-folded className="flex min-w-0 flex-col gap-2.5">
            <div className={cn("grid min-w-0 grid-cols-[minmax(0,1fr)_auto] [&>*+*]:border-t [&>*+*]:border-border/50", card)}>
              {/* The header names the list inside its card, its first row, as a grid's does. */}
              {header(title) === undefined || slate.parent(id)?.type === "section" ? null : (
                <div role="row" data-slate-head className={cn("col-span-full flex min-h-9 items-center py-2 text-xs leading-4 text-muted-foreground", inset)}>
                  <span role="columnheader">{header(title)}</span>
                </div>
              )}
              {shown.map((item, index) => {
                const row = { item, index };
                const name = str(slate.resolve(columns[title]?.["value"], row)) ?? "";
                const body = (opener: ReactNode) => (
                  <>
                    <span role="cell" className="flex min-w-0 flex-col gap-1">
                      <span className={cn(titleMono ? cn(MONO, "text-foreground break-all") : cn(NAME, "break-words"), tone(columns[title]!, row))}>{name}</span>
                      <span className="flex min-w-0 flex-wrap items-baseline gap-x-3 gap-y-1">
                        {noteOrder.map(at => {
                          const column = columns[at]!;
                          if (at === title) return null;
                          const value = slate.resolve(column["value"], row);
                          const shownValue = str(value);
                          if (shownValue === undefined || shownValue === "") return null;
                          const mono = (column["mono"] === true && !isSentence(value)) || isFigure(value);
                          return (
                            <span key={at} className={cn("min-w-0 break-words", mono ? cn(MONO, "leading-4 text-muted-foreground") : "text-xs leading-4 text-muted-foreground", tone(column, row))}>
                              {shownValue}
                            </span>
                          );
                        })}
                      </span>
                    </span>
                    {actionCell(row, opener)}
                  </>
                );
                const className = cn("col-span-full grid grid-cols-subgrid items-start gap-x-4", ROW, inset);
                return opens ? (
                  <OpenRow key={keys[index]} id={id} template={actions[0]!} row={row} raise={raise} className={className} label={`${str(slate.resolve(actions[0]!["label"], row)) ?? ""}, ${name}`}>
                    {body}
                  </OpenRow>
                ) : (
                  <div key={keys[index]} role="row" className={className}>
                    {body(null)}
                  </div>
                );
              })}
            </div>
          {below}
        </div>
      );
    }
    // A grid: the header over the card and every row on one template, the header cells on the rows' own tracks. The
    // card's border and the rows' inset are two edge tracks, so no row pads its subgrid and no track overflows into the
    // next. A figure or machine-text column is as wide as its widest value (its header's included, the tables beside
    // it at the least), so every column stands 16 px off the next.
    // The column of the longest words or machine text takes the room left, so the last column ends on the right inset
    // however wide the panel; its least is its longest word, in the 12 px mono ch, a little narrower than a 14 px name's.
    const fill = fillAt;
    const fillWord = Math.ceil(fillWordCh * 1.2);
    const tight = (at: number) => widths[at] !== undefined && at !== fill;
    const edges = place !== "inside";
    const edge = "calc(var(--settings-inset,20px) - 15px)";
    const tracks = [
      ...(edges ? [edge] : []),
      // With no rows to size them by, the columns share the card evenly, so the header spans it rather than bunching left.
      ...columns.map((_, at) => (shown.length === 0 ? "minmax(0,1fr)" : at === fill ? `minmax(min(${fillWord}ch,50%),1fr)` : widths[at] !== undefined ? `minmax(${widths[at]}ch,max-content)` : "fit-content(40%)")),
      ...(opens || actions.length > 0 ? ["auto"] : []),
      ...(edges ? [edge] : []),
    ];
    const rim = edges ? <span aria-hidden /> : null;
    // The header names the columns inside the card, its first row, so it reads as the card's own and never floats over it.
    const head = (space: string) => (
      <div role="row" data-slate-head className={cn("col-span-full grid grid-cols-subgrid items-center", space)}>
        {rim}
        {columns.map((column, at) => (
          <span key={at} role="columnheader" data-ch={widths[at]} className={cn("font-sans text-xs leading-4 text-muted-foreground", end(column, at) && "text-right", tight(at) && "whitespace-nowrap")}>
            {header(at)}
          </span>
        ))}
        {opens || actions.length > 0 ? <span aria-hidden /> : null}
        {rim}
      </div>
    );
    return (
      <div ref={ref} role="table" className={cn("grid min-w-0 gap-x-4", MONO)} style={{ gridTemplateColumns: tracks.join(" ") }}>
        {columns.every((_, at) => (header(at) ?? "") === "") || shown.length > 0 ? null : head("min-h-7 pb-[5px]")}
        {shown.length === 0 ? null : (
          <div className={cn("col-span-full grid grid-cols-subgrid [&>*+*]:border-t [&>*+*]:border-border/50", card)}>
            {columns.every((_, at) => (header(at) ?? "") === "") ? null : head("min-h-9 py-2")}
            {shown.map((item, index) => {
              const row = { item, index };
              const body = (opener: ReactNode) => (
                <>
                  {rim}
                  {columns.map((column, at) => {
                    const value = slate.resolve(column["value"], row);
                    const mono = (column["mono"] === true && !isSentence(value)) || isFigure(value);
                    return (
                      <span
                        key={at}
                        role="cell"
                        className={cn(
                          "min-w-0",
                          mono ? cn(MONO, "text-foreground", !tight(at) && "break-all") : cn(at === title ? NAME : WORD, "break-words font-sans"),
                          tight(at) && "whitespace-nowrap",
                          tone(column, row),
                          end(column, at) && "text-right",
                        )}
                      >
                        {str(value)}
                      </span>
                    );
                  })}
                  {actionCell(row, opener)}
                  {rim}
                </>
              );
              const className = cn("col-span-full grid grid-cols-subgrid items-baseline", ROW);
              return opens ? (
                <OpenRow key={keys[index]} id={id} template={actions[0]!} row={row} raise={raise} className={className} label={`${str(slate.resolve(actions[0]!["label"], row)) ?? ""}, ${str(slate.resolve(columns[title]?.["value"], row)) ?? ""}`}>
                  {body}
                </OpenRow>
              ) : (
                <div key={keys[index]} role="row" className={className}>
                  {body(null)}
                </div>
              );
            })}
          </div>
        )}
        <div className="col-span-full font-sans empty:hidden">{below}</div>
      </div>
    );
  },
};
