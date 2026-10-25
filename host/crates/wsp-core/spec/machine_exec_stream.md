# machine_exec_stream
Source: packages/runtime/src/machine-exec.ts:227-264, 346-736 (430 lines) at 65f6b89c7. Rust:
`src/machine_exec/stream.rs`. Tests: the reader, input, limit, reap and let-go cases of
packages/runtime/test/machine-exec.test.ts (61 cases) and all of machine-exec-kept.test.ts (5). Rust:
`tests/machine_exec_stream.rs`, `tests/machine_exec_kept.rs`.

## What it does
`open` is the one reader both factory roads share: it polls the run's log over exec, yields its lines, ends the run
(TERM, the stop grace, KILL, files removed) however it ends, cuts it at the turn's limits, and carries the input
channel's writes, closes and held lines. The run belongs to the machine: a reader that lets go leaves it untouched.

## Behaviour
1. Per stream: a sentinel `__WSP_EOF_<12 hex>__` and a reap mark `__WSP_REAPED_<12 hex>__`, fresh random each open.
   `started_at` is the attach's `started_at`, else `now()`; the idle clock starts at `now()`; `offset` is `from` or 0
   (machine-exec.ts:347-393).
2. Laziness: the poll runs only while something iterates `lines` (a TS async generator). Nothing is read, cut, reaped
   or settled for a stream nobody iterates; `exited` settles only through the iteration (or the let-go of 13).
   Eager, at open: the launch's own posts (factory), the run question of 3, `seed_due`'s hand-over, signals.
3. The opening (machine-exec.ts:412-435): a launch failure with no posting, or whose posts never may have run, is the
   stream's failure. Else ask the machine `if [ -d 'base.d' ]; then echo WSP_RUN; else echo WSP_GONE; fi` until it
   answers: `WSP_RUN` opens (the launch counts as launched); `WSP_GONE` fails with `LaunchUnanswered` (message the
   launch's, kind `LaunchUnanswered`) unless the launch failure was `Unreached`, which stays; silence past
   `LINK_RETRY_WINDOW_MS` from the first ask (no bound with `asks_until_stopped`) fails `Unreached` with attempts =
   posts + asks and elapsed = now - first post; a kill stops the asking with the launch's own failure; a let-go parks
   it for ever. Between asks, `nap(poll_ms)`; a probe that throws or answers neither word is asked again.
4. The poll (machine-exec.ts:537-604), each round: let-go parks for ever; killed reaps and settles `None`; a waiting
   question touches the clock; `cut = turn_cut(...)` unless resting; a cut that is caught up with no exit seen reaps
   and ends with the cut. One exec of `poll_cmd(offset, work)` where `work = !resting && reads_work(idle, quiet)`:
   `E=$(cat 'b'.exit 2>/dev/null); tail -c +<offset+1> 'b'.log 2>/dev/null | head -c 262144 | base64 -w0; P=$(cat
   'b'.pid 2>/dev/null); ` + (work: `W=$(awk -v p="$P" '<TREE_WORK_AWK>' /proc/[0-9]*/stat 2>/dev/null); `) +
   `printf '\n<sentinel> %s %s %s\n' "$E" "$([ -n "$P" ] && kill -0 "$P" 2>/dev/null && echo up || echo down)" "$W"`.
5. A poll exec that throws: dark (below), `nap(poll_ms)`, then `hold(now - at)` on the clock (the dark stretch is no
   part of the quiet). An answer with no sentinel line: dark, `nap(poll_ms)`, no hold. Dark: `dark_since` is set once;
   with a cut, end with it when caught up or `now - dark_since >= LINK_RETRY_WINDOW_MS`. Any answer with the sentinel
   clears `dark_since`.
6. A sentinel answer is split on spaces: exit (default ""), live (default "up"), work (default ""). A work that parses
   (`parseInt`, safe integer) is read into the clock at `now()`. The chunk is the base64 of every line before the
   sentinel joined; `offset` moves by its length. A non-empty chunk touches the clock, keeps the polls hurried for
   3 s when within 30 s of a posting's first post, yields its lines, sets caught up when shorter than 262,144, notes
   `exit_seen = exit != ""`, and polls again with no nap.
7. An empty chunk with an exit: reap, settle `parseInt(exit)` (non-numeric settles `None`, decision 2), and when the
   code is 137 ask `killed_why(started_at)` once unless this side signalled; its words become the stream's last `Err`.
   Else caught up; a cut ends with it; `down` on two polls over the stream's life (never reset) reaps and settles
   `None`, then asks `killed_why`; else `nap(gap())`.
8. `gap()` is 200 ms (or `poll_ms` when smaller) while `now < hurried_until`, else 10 s when resting, else `poll_ms`
   (machine-exec.ts:372-378). A launch starts hurried for 30 s; a write, a hand-over and a signal to a resting run set
   it to now + 3 s and wake the nap in flight (`hurry`).
9. The reap (machine-exec.ts:456-510): one exec of `reap_script(b, read, end.is_none())` + `; true`, `read` being
   `S=<end or $({ wc -c < 'b'.log; } 2>/dev/null || echo 0)>; N=$((S - <offset>)); [ $N -gt 262144 ] && N=262144; [
   $N -gt 0 ] && tail -c +<offset+1> 'b'.log 2>/dev/null | head -c $N | base64 -w0; printf '\n%s %s\n' <mark>
   "$((S))"; [ $((S - <offset>)) -gt 262144 ] && exit 0; `. The group is signalled on the first reap only; the size
   the first reap read is the end every later read stops at; reads repeat while a chunk came and offset < end; when
   nothing came back, or offset < end, one more exec `rm -rf 'b'.*; true` removes the files; the tail no newline
   ended is yielded last. An exec failure reads as nothing came back.
10. `write(line)`: no input fails `this stream has no input channel`; settled fails `the stream has ended`; else wait
    for the opening (its failure is the write's), then for the line parked before this call (only that one), then
    append: through the one-at-a-time chain, `put_files(machine, [base.in += line\n], before: [`{ [ -e 'b'.exit ] || [
    ! -d 'b.d' ]; } && { echo WSP_GONE; exit 0; }`, release_held(b)], after: ["echo WSP_OK"])`. `WSP_GONE` answers
    `Gone` with the clock untouched; a non-zero exit or no `WSP_OK` fails `remote write failed on <id>: nothing came
    back saying WSP_OK, the word the guest prints once the message landed; <machine_answer>`; else touch, hurry,
    `Written`.
11. `close_input()`: once, only with input and unsettled; hurries a resting run; after the opening and every parked
    line, through the chain: `<release_held(b)>; P=$(cat 'b'.tail 2>/dev/null); [ -n "$P" ] && kill -TERM "$P"
    2>/dev/null; true`. Failures are swallowed.
12. `write_after(line, after)` (machine-exec.ts:680-715): queues behind the line parked before it; after the opening,
    parks `base.late.key` (12 hex) then `base.late` (`line\n`) in one `put_files` with `before: ["umask 077"]`; then
    awaits `after`. Refused: removes both when parked. Due and parked: through the chain, `until_reached` of
    `release_late(b)`, then touch and hurry. Due and not parked: `append(line)`. Every failure is swallowed; the queue
    moves on.
13. Let-go (`reading`): the open registers a drop; the drop parks the poll where it stands, wakes its nap, and every
    later nap returns at once with no timer; nothing is signalled, reaped or settled. Settling removes the drop.
    Dropping the `lines` stream before its end settles `exited` with `None` and reaps nothing (the TS `finally`).
14. `teardown` sends `kill -TERM`, `kill` marks killed and sends `kill -KILL`, each as `P=$(cat 'b'.pid
    2>/dev/null); [ -n "$P" ] && kill -<SIG> -- -$P 2>/dev/null; true` after the opening settles, marking the run
    signalled. `rest()` stops the limits and ends the leftovers (`LEFTOVERS_AWK` script, TERM, ten 200 ms checks, KILL)
    after the opening; `next_turn()` clears resting, restarts `started_at` and the clock, wakes the nap, answers
    `offset`. `seed_due`: once due and the opening succeeded, `until_reached` of `release_held(b)` through the chain;
    a refused seed or a hand-over that fails kills the stream.

## Edge cases
- A write after the command exited, before the poll saw it: one exec, `Gone`, input unchanged, idle clock untouched.
- A finished run re-opened past its wall keeps every line and its exit; one still going hands over what it printed,
  then the wall cut; on a dark machine the cut waits the reach window first.
- A stop of a run that ignores TERM reads what it printed after the last poll, past one chunk; a writer outside the
  group does not hold the stop past the size at the KILL.

## Behaviour nobody wrote down
- Invariant 1 (runtime AGENTS.md): a re-opened run is read from `from` or byte 0, and a reply that landed while no host
  read it is kept (`keeps_the_reply_of_a_run_that_finished_while_no_host_read_it_when_it_is_re_op`).
- Invariant 2: `next_turn` answers the offset the next turn's re-open reads from, so no line is read twice
  (`re_opens_a_turn_on_a_kept_process_from_where_that_turn_began_past_the_turns_b`).
- Trap #2243: a parked line goes in by its key, not its words; one hand-over at a time; a write waits only for lines
  parked before it.
- Trap #1620/#1665: a held seed waits on the machine (`base.held`), so a host restart hands it over at attach.

## Decisions
1. Private state sits behind a `std::sync::Mutex`, never held across an `.await` or a call out (rule 3.3).
2. `parseInt` of the exit text keeps JS semantics (leading spaces, sign, digits); NaN settles `None`, as
   `JSON.stringify(NaN)` writes the `exec.exit` frame's `null`.
3. `write_after` with no input answers `None` (the trait's road-holds-nothing), so the caller's own write gets the
   channel refusal; the TS throws.

## After the port
- `exited` never settles on a machine stream nobody iterates, a killed one included.

## Rules against tests
1-2 `a_reader_that_lets_go...` (2), `a_reader_let_go_of_while_its_poll_is_in_flight...`; 3 the five landed-then-dark
and dark-machine cases; 4-7 the poll, limit, dark and minute-guest cases [104-245, 480-645, 747-781, 1871]; 8 the three
hurry cases [344, 369, 420]; 9 the TERM and writer cases [1447, 1475, 1505]; 10-12 the write, close and held-line cases
[313-343, 465, 1594-1869]; 14 machine-exec-kept.test.ts (5); 7 `a_run_that_exits_137_is_asked_about_once...` [247].
