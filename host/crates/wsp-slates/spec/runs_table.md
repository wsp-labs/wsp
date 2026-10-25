# runs_table
Source: slate-runs.ts:613-983 (`createSlateRuns`), 133-163 (`RunApprovals`, `SlateRunsDeps`), 210-242 (`SlateRuns`) at
65f6b89c7. Tests: slate-runs.test.ts (28 cases); the large-module pilot's 8 review tests (#2440, 6396255198).

## What it does
The process table of every thread's slate: one live entry per run with its record, a generation that drops the end
of a replaced instance, the last result kept through holds, the hold it waits in (approval, four running, the start
budgets), a FIFO queue for runs past four at once, timers per declared `every`, and the secrets. Every record change
goes to `deps.on_record` at once. It starts nothing by itself but through a timer's `on_timer`.

## Behaviour
1. `live(thread, run, last)` (:627-636) makes an entry once: record `{state:"idle", runs: last.runs ?? 0}`, gen 0,
   no starts, and `last_result(last)` as its kept result. A later `last` is ignored.
2. `write` (:637-643): sets the record; a record that is not running and holds a result (`last_result`) replaces the
   kept result, else the kept result stays (a hold never wipes it); then `on_record(thread, run, record)`.
3. `start(req)` (:883-904), in order: (a) `asleep` and `by: timer`: a running entry answers `noop`; else drops any
   hold and writes held `[result], state, why: <asleep>, runs` and answers `held` with no ask. (b) `on: "host"` with
   `host_refused` saying why: `fail`. (c) `beside` = the entry reads running; `beside` with `once` answers `noop`.
   (d) drops any hold. (e) spends one start against the person's own budget for `by: person`, else the shared one;
   past it, `hold(pressed | budget, beside)`. (f) a `confirm`, or no approval for `run_key(decl)`, is
   `hold(approval, beside)`. (g) else `launch`.
4. `hold(req, entry, for, beside)` (:704-713): keeps `{req, for}`; `busy` joins the queue once. Beside a running
   instance the record is left as it is (not written) and the answer is `held` with that record (and the ask for
   `approval`). Otherwise writes `{state:"held", why, runs}` with why `needs your approval`, `4 runs are already
   running`, the pressed or the budget sentence; answers `held`, with the ask only for `approval`.
5. `fail(req, entry, why)` (:715-719): writes `{state:"failed", why: streak(failed, why), exit:null, runs, endedAt}`.
6. `launch(req, entry)` (:733-834), in order: a running instance is stopped with no reason; four running in the
   thread is `hold(busy)`; `inputs()` is read; a secret in `args` fails `SLATE_SECRET_IN_ARGS`; `road(thread, on ??
   "thread")`; env is `login_env(thread)` here (every `WSP_` name dropped, `SLATE_DIR` from `dir(thread)`, whose
   failure fails `the slate's files were not written: <message>`) or `{SLATE_DIR: road.slate_dir, WSP_THREAD:
   thread}` on a road; each env input in order: a name not matching the env pattern fails `<name> is not a name an
   environment variable can have`; a secret's plaintext, a value's `as_text`, and a secret not set or a null value
   removes the name; args are `as_text` (null is empty); stdin a secret's plaintext or empty, or `as_text`; cwd the
   folder, or the folder resolved with `cwd` (`path.resolve` here, posix on a road: an absolute `cwd` wins, `..`
   folds); here a cwd that is not a folder fails `the folder <cwd> does not exist`. Then gen += 1, `runs` + 1,
   writes the running record (rule `runs_parts` 1) and starts the road.
7. While it runs (:768-774): each line scrubbed, the last 500 kept, `on_line` only for the current gen.
8. Its end (:775-821), dropped when the gen moved: out and err scrubbed then `run_output_tail`; `json` is the out
   parsed when its trimmed text is not empty and parses; exit 124 on timeout else the code; why the scrubbed spawn
   error, `timed out after <s> s`, none on exit 0, `ended by <signal or "a signal">` on no code, `exited with <n>`.
   A failure or no `then` finishes at once; else the `then` runs on the raw scrubbed out (here through `reshape`,
   on a road through `road.reshape` with only `SLATE_DIR` and `WSP_THREAD` and 60 s), and its answer finishes with
   `done` and its json, or `failed` with its why, exit and err. Finishing writes the finished record (`runs_parts`
   1), with `lines` only for a `stream` run, then runs the queue.
9. Process naming (:822-830): `own(thread, pid, "slate")` once the road answers a pid and the run has not ended; its
   release runs at the end. The kill is kept only when the gen is current and the record still reads running.
10. `stop(thread, run, entry, why)` (:836-856): gen += 1, kill, kill the reshape, drop the hold, leave the queue. A
    reason and a record that read running or held writes the cancelled record; otherwise nothing is written.
11. `approve(thread, run, scope)` (:905-912): only an entry held for approval; `always` allows the key first; then
    `launch`. Else none. `deny` (:913-923): with an instance still running or reshaping, only the new start is
    dropped (no record); else `stop` with `you said not to run it`.
12. `cancel` stops with `cancelled` then runs the queue (:929-934). `stop_all(quiet | why)` stops each run with no
    reason when quiet, else the why or `cancelled`, and does not run the queue (:935-939).
13. The queue (:721-731): while runs wait and fewer than four run, the first is taken; one no longer held as busy is
    skipped; the rest launch.
14. Timers (:858-879, 940-952): `timers(thread, list)` keeps each timer whose run, key, every and always are
    unchanged (its period and failures run on), drops the rest, adds the new ones, then ticks. A tick arms each
    timer that should run (always, or the slate is shown) and is not armed: it fires at once, then every
    `max(every, 10)` seconds; it disarms the ones that should not. A fire of an `always` timer with five or more
    failures in a row, within 300,000 ms of the last, is skipped. `streak` (:694-702): `done` clears the failures,
    `failed` adds one and stamps the time, and on an `always` timer at five or more adds `; <BACKED_OFF>` to the why
    (`it failed` when there was none).
15. `release(thread)` empties the shared start budget of each entry, never the person's (:954-956).
16. `held(thread)` lists each entry held for approval, in entry order, its ask built again (inputs read now) (:926).
    The ask (:673-690): `run, key, cmd, env, args, [stdin], folder, on, timeout, [confirm], [then], [files]`; a set
    secret shows `••••`, an unset one `•••• (not filled)`, null `(unset)`; stdin its first line only.
17. `drop(thread, keep)` (:965-974): without `keep` every run is stopped with no reason; with it the running ones are
    left running; timers cleared and the thread forgotten either way; then every approval the approvals list holds
    for the thread is revoked, and the thread's secrets forgotten, `keep` or not.
18. `close()` (:975-981): every run of every thread stopped with no reason, timers cleared, all forgotten (rule R8).
19. `reshape(thread, cmd, input, folder, timeout_s = 60)` (:651-659): `login_env` failing answers at once
    `{why: "the slate's files were not written: ...", exit: null, err: ""}`; the cwd is the folder when it exists,
    else the host process's own.

## Edge cases
- A hold never wipes the kept result: done, then a held start, then approved, gives a running record with the old
  out and `refreshing` (pilot review).
- The budget is spent before the approval check: a start held for approval still counts.
- A thirteenth reaction or timer start in a minute holds `budget`; the person's start still runs; `release` frees the
  shared budget; the person's twelve a minute are freed by nothing (tests `reactions and timers ...`, `the person's
  starts ...`).
- A fifth start waits held `4 runs are already running` and runs when one ends (test `a fifth run waits`).
- `stop_all(quiet)` writes nothing, and the killed run's end writes nothing either (gen moved).
- A held start beside a running confirm run: the record stays running with its runs; `deny` drops the new start and
  the first one ends `done` with `runs: 1` (slates.test.ts `a second press on a confirmed run`).
- The asleep tick keeps the last out and exit on its held record and lists no ask.
- `every` past 2,147,483 s fires every 1 ms, as node clamps (rule 8.18); it never panics.
- An `always` timer failing every 10 s: five starts by 40 s, none more until 300 s after the fifth, the sixth then;
  once one succeeds every tick starts again (test `an always timer that fails five ...`).
- Shown-only timers: none fire until shown; shown fires at once and each 10 s floor after; hidden again stops them;
  `always` ones fire regardless (test `tick only while shown`).
- On a road the env is exactly `{SLATE_DIR, WSP_THREAD}` for the command and for its `then` (test `on the thread's
  own machine ...`).

## Behaviour nobody wrote down
- Rule R1 (3.3): `on_record`, `on_line`, `on_timer`, `own`'s release and a road's `kill` and `start` are called with
  no borrow of the table held; `on_record` re-enters the slate area, which may call `start` or `cancel` again.
- Rule 3.15: `now` and every timer through the clock. Rule 8.18: the interval clamp.
- Invariant 2: one end per instance, enforced by the generation.

## Decisions
1. `RunApprovals` and `SlateRunsDeps` are traits; `SlateRuns` is a trait so the slate area's tests can swap it, as
   `deps.runs` lets the TS.
2. `on_line`'s stream is `Stream::Out | Stream::Err`.

## After the port
- `stop_all` never runs the queue; a restart while four run holds itself busy until another ends (pilot).
- A timer's `every` past node's limit fires every millisecond (8.18).
- `drop(keep)` leaves running processes untracked: their ends still call `on_record`.
