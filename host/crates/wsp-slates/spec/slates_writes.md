# slates_writes
Source: slates.ts:464-468 (the write budget), 1017-1093 (`rehearse`, `writeDocument`, `undo`), 1118-1231 (`write`,
`state`) at 65f6b89c7. Tests: slates.test.ts cases on writing, patching, checking, rehearsing, refusing, the agent's
and the person's `state`, secrets, the paired computer, warnings and dead runs.

## What it does
`slate_write`: a whole document or a patch, checked or stored, the undo, and a check that rehearses values and a
press on a copy. `slate_state`: the live values a window or the agent sets, a secret the person types, and the runs
the agent may start. Each goes on the thread's chain.

## Behaviour
1. `write(p, caller)` (:1118-1167). Before the chain: the target (a write); `values` or `press` without `check`
   refuses usage `values and press rehearse a slate, which only a check does.` / `Add check, or set the live values
   with slate_state.`; both `text` and `document`, or neither without a rehearsal, refuses usage code A601 `A601
   step-arg: a slate write takes exactly one of text or document, and this one has <none|both>.` / `Send text
   alone.`; whole means a document, or text whose start (after JS whitespace) is `<slate`. On the chain, with the
   record or a fresh one:
   - neither given: no document refuses `Z802 no-slate: this thread has no slate to rehearse; send the <slate> with
     check`; else rehearse the live document.
   - whole: parse the text or validate the document; errors are the parser's then the MCP table's refusals
     (`refused_on`); any, or no document, refuses `invalid`. A rehearsal rehearses it over `values_for`; a check
     answers the sketch of it over fresh values (no sources read), version unchanged, its warnings, no problems;
     else `check_version`, the agent's write spends the budget (V751 below), and `write_document` with cause
     `write` and every piece id.
   - a patch: version 0 refuses `Z802 no-slate: this thread has no slate to patch; write a whole <slate> first`;
     parse errors refuse invalid; a lone `undo` op answers the current sketch for a check, else `check_version` then
     `undo`. Otherwise apply; errors (with the MCP refusals) refuse invalid; values are the record's for a clear, else
     `values_for` over the patched values; a rehearsal or a check answer as above over the patched document; else
     `check_version`, the budget, and `write_document` with cause `clear` or `write` and the ids the ops name, once
     each, in order.
2. The agent's budget (:464-468): a refusal reads `V751 write-rate: wait <s> s; a slate that rewrites itself
   constantly is a bug`, kind conflict. The person's writes spend none.
3. `write_document(r, next, values, by, cause, pieces, warnings)` (:1052-1080): values over 262,144 bytes refuse
   invalid `S500 values-too-big: the values are <n> bytes; a slate holds at most 262144`; declarations dropped; a
   record past version 0 keeps the one-step `previous`; document and values set, `empty` `cleared` for a clear;
   version and revision + 1; held; timers armed; with a document, held asks re-held when their key moved, and each
   timed run reading idle that would be held is started now `by: timer` (so it reads held and asks); saved;
   announced; `watch_pr` when the document names `pr.checks`; every value pushed; answers `{version, text (sketch),
   warnings, problems, waiting}`.
4. `undo(r, by)` (:1082-1093): no `previous` refuses conflict `V752 nothing-to-undo: there is no earlier slate to go
   back to; the undo keeps one step`; else the current becomes `previous`, the old one is put back (`become`),
   version and revision + 1, saved, announced `undo`, all pushed; answers the sketch with no warnings or problems.
5. `rehearse(r, doc, values, p, warnings)` (:1021-1050): each value path (a bare name is `$name`); a path into a run
   seeds a copy of that run's record as `{state:"done", runs:1, ...was, [state "done", endedAt now when it had none]}`
   and sets the field there; the rest are the batch's writes. A press of a piece the document lacks refuses invalid
   `D203 piece-missing: there is no piece <p> to press` with the nearest id as its fix. The batch runs over the copy
   as the person with the press event; the answer's text is the sketch after it, then a line each `would start
   $<run> (<why>)`, `would cancel $<run>`, `would <do> <JSON text>`, `would <do> in the window`; the version is the
   record's; nothing stored, started or sent.
6. `state(p, caller)` (:1169-1231): the target (a write); a paired computer naming `start` refuses usage `a paired
   computer writes a slate's values and starts no run.` / `Press it on the computer the slate runs on.` On the
   chain: `need_record`; no document refuses `Z802 no-slate: this slate is empty; write one with slate_write`;
   `check_version`; neither values nor starts refuses usage `slate state sets values or starts runs, and this one
   names neither.` / `Give values, start, or both.` Each start (a leading `$` dropped): undeclared refuses invalid
   `K702 run-name: $<run> is not a run` with `$<nearest>`; approved always starts (`agent` or `person`); held says
   `$<run>: it already waits for the person to allow it on the slate, and starts once they do`; a `confirm` says
   `$<run>: it has confirm, so it asks the person every start and only a press or a <when> starts it`; else
   `$<run>: you start only a run the person allowed "Always in this thread"; a press, a <when> or every= starts it
   and the slate asks them`. Each value: not a value path refuses S501 `<written> is not a value path; a value path
   reads $name, with .field and [index] below it`; a derived or a run refuses A607 `$<n> is a <derived value|run>
   and cannot be written` with fix `write the value it reads`; undeclared refuses S501 `$<n> is not declared` with
   the nearest; a secret takes only the person's typed string at its root (S520 `$<n> is a secret: only the person
   types it, into its input` / `$<n> takes the typed text`), empty clears it, and the batch gets the handle. The
   agent spends the write budget; a call with values and no starts releases the shared start budget. One batch for
   the values (no reactions from a paired computer), `announce(state)` for the agent's, one batch for the starts;
   sends delivered without waiting (a failure logs `a slate send in thread <word> was not delivered: <message>`).
   Answers `{version, text (sketch, then "not started:" and each indented), problems, waiting, [notStarted]}`.

## Edge cases
- A whole write as the agent: version 1, text starting `slate v1`; a timed run not allowed reads held, `waiting:
  ["$tick"]`, no problem, and the text names it as waiting.
- Twice the same declaration keeps its timer and approval (no ask); a changed `every`, `always`, `timeout` or
  `once` asks again with a new key.
- A rehearsal never moves version, revision, values or asks; `check` with `values: {"$market.json": {open: true},
  "$market.exit": 0}` sketches `Open` and stores nothing.
- A refused write lists every error on one numbered line; one error is said alone.
- A file naming a secret is refused invalid with an S520 error.
- A write at the version read passes while a run moves values (versions move only with documents).

## Behaviour nobody wrote down
- Rule R2: everything after the target runs on the chain; the refusals before it never queue.

## Decisions
1. Not ported here, kept with `wsp-threads`: `a start into a folder that is not there is refused before the agent
   is launched` and the model, effort and window a press's or a notify's turn starts on (both test the session
   start). The slate half of the press (`outcome: started`, the prompt) is in `slates_acts`.

## After the port
- None.
