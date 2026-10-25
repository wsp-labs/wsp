# slates_reads
Source: slates.ts:549-589 (`viewsFor`, `contextOf`, `scrub`, `sketched`), 592-685 (`boundPaths`, `problemsOf`,
`waitingOf`, `changedSince`, `asksOf`, `viewOf`), 1233-1279 (`read`, `catalog`), 1482-1488 (`resolve`) at 65f6b89c7.
Tests: slates.test.ts `a read carries each run's json`, `a read puts the paths it was asked for in its text`, `a
thread with no slate yet reads as an empty slate`, `a read asked for a run or a value without its $`, `a read
answers the JSX-like form by default`, `a read names each approval by the run`, `a timed run the person has not
allowed reads held`, `a plotted value that is not a number`.

## What it does
Everything the area answers without changing a record: how a path is resolved against the record and the host's
sources, the sketch with its standing problems and waiting runs, the sheet's asks, the view a window draws, and the
agent's `slate_read`, `slate_catalog` and a window's `resolve`.

## Behaviour
1. `views_for(r, extra)` (:549-553): the sources named by the document's JSON and the extra paths (`sources_named`),
   always with `time` first, viewed once through `view_sources`.
2. `context_of(r, views, row)` (:556-581): `now`, the row's `item` and `index` when given, and `resolve(path)`: a
   path without `$` reads the views; a `$` path that does not parse reads nothing; a derived name is evaluated once
   per context and cached, a name already being evaluated (a cycle) reads nothing, then the path's segments step
   into it; any other `$` path reads the values.
3. `sketched(r, views, check, asked, warnings)` (:585-588): the module's sketch with the context, the version, the
   warnings, and either `check: true` or the problems (`asked` then `problems_of`) and `waiting_of`; a text not
   starting `slate v<n>` gets `slate` replaced by `slate v<version>` at its start; the whole text scrubbed.
4. `bound_paths(doc)` (:592-609): the dependencies of every binding in every piece's props (and of every `${...}`
   in a lone `{format}`), of each piece's `when`, and of each derived expression, in that order, once each, less
   those starting `item` or `index`.
5. `problems_of(r, views)` (:613-620): the record's problems; `Z804 rewound-before: the thread was rewound to before
   this slate existed; write one with slate_write` when the document is null and `empty` is `rewound-before`; then
   for each bound path without `$` that resolves to nothing, `R900 data-missing: <path> has no value yet`.
6. `waiting_of(r)` (:623-627): `$name` of each declared run whose record reads held with no why or the approval why.
7. `changed_since(r, run, key, pin)` (:631-641): over the record's other allowed approvals of the same run: unread
   pinned files answer `<paths> could not be read on <computer>, so it asks again`; else the files whose hash
   differs from the approval's, in the order of the approval's then the new ones, answer `<paths> changed since you
   allowed it, so it asks again`; else none.
8. `asks_of(r)` (:643-667): each run the runs table holds for approval, as `{key, run, kind:"cmd", cmd, env, args,
   [stdin], computer (or "this computer"), folder, timeoutS, [confirm], [then], [files], [noAlways: true], why}`;
   `noAlways` when the pin left anything unpinned; `why` the pin's refusal, else `changed_since`, else the record's
   why, else `needs your approval`. Then the MCP table's held asks.
9. `view_of(r)` (:669-684): `{threadId, workspaceId, version, revision, document, values, [empty (or "none") when
   no document], comments, approvals (with `send: {state:"allowed", at}` added when sends are allowed), asks,
   problems, shownOnce, canUndo (a previous exists), updatedAt}`.
10. `read(p, caller)` (:1233-1261): the target (not a write); no record reads as a fresh one, never a refusal. Paths:
    `*` with a document means every bound path, else the asked list less `*`. Views include those paths. Misreads
    of each path; the sketch (or only `slate v<n>` with `sketch: false`) carries them. The text is the sketch, then
    `read:` and `  <path> = <JSON of the scrubbed value>` per path (cut at 4,000 units with `... (N characters)`),
    then a blank line and the printed document unless `text: false` or no document. The answer: `version, text,
    [document] (only with document: true), values (each path), state (each value not a run, as $name), derived
    (each, as $name), runs (each, `run_shown`, a summary when paths were named), problems (misreads then problems_of),
    waiting, comments, approvals` where each approval is keyed `$<run>` when a declared run carries it now, else its
    key, and reads its state. Every string scrubbed.
11. `catalog(p, caller)` (:1263-1279): a sub-thread refuses Z803; the module's catalog text; when a name was given
    and the text says `<name> is not in the catalog`, and the call names a thread (a refused target answers the
    text), the MCP table's catalog of that server, or the text plus `\n<name> as an MCP server of this thread's
    agent: <message>` when that fails.
12. `resolve(p)` (:1482-1488): no record answers each path null; else each path evaluated, scrubbed.

## Edge cases
- A thread with no slate reads `{version: 0, text: "slate v0, empty: write one with slate_write", problems: []}`.
- `values: ["$ticks", "$tick.state"], text: false` gives `read:\n  $ticks = 0\n  $tick.state = "held"` and no
  `<slate`.
- `values: ["tick", "ticks", "spot.json", "time.now", "$ticks", "len(git.changed)"]` gives exactly three X401s
  (run, value, no source) and the text carries `X401 slate: tick is not a source`.
- A run with `json` reads without `out`; a 9,001-unit out is cut with the `… (9001 characters; values
  ["$raw.out"] reads it whole)` tail; a read naming paths gives run summaries without `json`.
- A read of an approval made by "Always" reads `{$tick: "allowed"}`.
- `document: true` adds the JSON document; by default the text carries the printed `<slate title=...>`.

## Behaviour nobody wrote down
- Rule 8.21: two cut forms (U+2026 in `run_shown`, `...` here). Rule R5: 4,000 units.
- Every answer string passes through the thread's scrub (secrets never leave).

## Decisions
1. The source views are `IndexMap<String, Option<Value>>` (a source with no view is `None`).

## After the port
- None.
