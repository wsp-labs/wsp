# slates_model
Source: slates.ts:81-292 (types, constants, small helpers, the `Slates` interface), 686-758 (`snapOf`,
`refreshSecrets`, `armTimers`, `become`, `valuesFor`, `dropDeclared`, `trimSnapshots`) at 65f6b89c7. Tests:
slates.test.ts `a check rehearses ... a changed command marks its last result stale`, `changing how often a command
runs ... asks again`, `a rewind restores the turn's snapshot`; the snapshot caps are new tests.

## What it does
The stored record of one thread's slate (schema 2), its snapshot shape, the deps the area takes, and the functions
that move a record between documents: which live values a new document keeps, what a dropped declaration leaves
behind, how a snapshot is put back, how timers are armed off a document, and how old snapshots are trimmed.

## Behaviour
1. `SlateRecord` (:100-122), keys in the order `fresh_record` writes them (:516): `threadId, workspaceId,
   rootThreadId, schema (2), version, revision, document, values, empty, turns, kept, comments, approvals, problems,
   shownOnce, updatedAt`, then `previous` and `sendsAllowed` when set. `version` moves only with the document;
   `revision` with every batch that moved a value and every version.
2. `SlateSnap` is `{document, values, [empty]}` (:82-86, 686). `turns` maps a turn id to `{at, version, revision}`
   in the order turns ended; `kept` maps `String(revision)` to a snapshot.
3. `Approval` is `{state, at, [run], [cmd], [scripts]}` (:89-96).
4. `problem(code, name, message, extra)` is `{code, name, message, ...extra}` (:226). `refused(p, kind)` and
   `usage(p)` are errors whose message is `<code> <name>: <message>`, with that kind and the code (:229-231).
5. `sources_named(text)`: of the host sources in their order (`thread, usage, cost, time, git, pr`), those the text
   names as `\b<name>\.` (:234). `own_path(p)` puts a `$` before a path that lacks one (:237).
6. `command_of(decl)`: the declaration less `every`, `always`, `timeout`, `stream` (:240-243). `size_of(v)` is the
   UTF-8 bytes of `JSON.stringify(v)` (:254).
7. `SAID` (:246-252): what each delivery outcome reads under a pressed piece, by the send's kind, word for word.
8. `values_for(r, next)` (:719-735): first each value `next` declares, in its order: a secret its handle; a name the
   old document also declared as a value and the record holds keeps its live value; else a copy of its `start`.
   Then each run `next` declares: a run the old document did not declare, or whose value is not a run record, reads
   `{state:"idle", runs:0}`; a done, failed or cancelled record whose `command_of` changed gets `stale: true` (added
   last); any other keeps its record.
9. `drop_declared(r, next)` (:738-745): each secret the old document declared that `next` does not declare as a
   secret is forgotten; each run it declared that `next` does not is cancelled in the runs table and the MCP runs.
10. `refresh_secrets(r)` writes each declared secret's handle as read now (:689-691).
11. `arm_timers(r, stopped)` (:693-698): each run with `every` not in `stopped`, in document order, as
    `{run, every, key, [always: true]}` where `key` is the run key of the approval declaration for a `cmd` run and
    `JSON.stringify(decl)` for the others; then the MCP servers: every non-`cmd` run's server, and those whose run has
    `every` and `always` as kept.
12. `become(r, snap)` (:702-715): every run of the thread stopped quietly in both tables; document and a copy of the
    values from the snapshot; each declared run whose value reads running becomes `cancelled`, `the thread was
    rewound`; secrets refreshed; `empty` set to the snapshot's (or `none`) when it has no document, dropped
    otherwise; timers armed. Nothing fires.
13. `trim_snapshots(r)` (:748-758): turns past the newest 100 dropped oldest first; every kept snapshot no turn points
    at dropped; then while more than one turn is left and the kept snapshots' JSON passes 8 MiB in UTF-8 bytes, the
    oldest turn and every snapshot no turn points at go.
14. Constants (:210-221): `SNAPSHOTS_KEPT` 100, `SNAPSHOT_BYTES` 8,388,608, `MESSAGE_CHARS` 20,000,
    `REQUEST_KEPT_MS` 600,000, `REQUESTS_KEPT` 500, `EVENTS_PER_SECOND` 5, `PRESS_SEND_MS` 2,000,
    `REACTION_SEND_MS` 60,000, `PROBLEMS_KEPT` 20, `PIECES_NAMED` 20; `SLATE_SEND_KEY` is `send`.

## Edge cases
- A value whose declaration moved from value to run (or back) starts fresh: idle, or its `start`.
- A command written again with only `timeout` changed keeps its result fresh; a changed `cmd` marks a done result
  stale and keeps `out` (`changed command marks its last result stale`).
- The 101st turn drops the first turn's pointer and the snapshot only it used; a turn that ends twice moves to the
  end of `turns` (:1502-1503).
- An 8 MiB cap with one turn left keeps that turn whatever its size.

## Behaviour nobody wrote down
- Rule 3.11: `turns` order is the order turns ended, and the trim drops in that order.
- Rule R7: a rewind's or a restart's secret is read off the runs table; neither fills nor empties a secret.

## Decisions
1. `SlateRecord`, `SlateSnap`, `Approval` are serde structs in the order of 1-3; a record read back from the store
   whose `empty` was deleted and set again had it last in the TS (an accepted difference, state.db only).
2. `SlatesDeps` is one trait (13.6): optional TS members are methods answering `Option` or a default.

## After the port
- None.
