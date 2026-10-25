# pipe_pool
Source: packages/runtime/src/pipe.ts:1-392 (392 lines) at 65f6b89c7. Rust: `src/pipe/pool.rs`. Tests:
packages/runtime/test/pipe.test.ts's first three groups (16 cases) and 3 pinned beyond them. Rust:
`tests/pipe_pool.rs`, against a fake daemon (a local websocket server).

## What it does
The host's pool of pipes to each brokered workspace's daemon: extra websockets the daemon parks until a process on
the machine connects to one of its doors, then carries that one connection's bytes. The pool keeps 4 idle pipes per
workspace whose link is up, dials a replacement as each is taken, paces and caps its dials, and backs off on a daemon
that is behind, refusing its token, or unreachable.

## Behaviour
1. Constants (pipe.ts:16-30): `PIPE_IDLE` 4, `PIPE_IDLE_MAX_MS` 600,000, `PIPE_DIAL_MS` 15,000, `PIPE_BEHIND_AGAIN_MS`
   60,000, `PIPE_DIAL_RATE` 4, `TOKEN_REFUSED_MAX_MS` 1,800,000; `PIPE_POOL_MAX` (64) and `PIPE_DAEMON_VERSION` (159)
   are the wire's.
2. `token_refusals(first)` (pipe.ts:35-48): `refused(now)` counts one more when the host wrote within `2 *
   TOKEN_REFUSED_MAX_MS` before, else restarts at 1, forgets the write, and answers `{count, wait: min(first *
   2^(count-1), TOKEN_REFUSED_MAX_MS)}`; `wrote(now)` notes the write.
3. `token_refused_line(who, road, count, wait)` (pipe.ts:50-59): count 1 `<who>: its daemon refused the token <road>,
   so the host writes its own there again and dials in <w>`; count 2 the second sentence word for word; else `None`.
   `<w>` is `round(ms/1000) s` under 120,000 ms, else `round(ms/60000) min` (`js_round`).
4. `up(id)` adds a workspace once (a second `up` does nothing) and fills it; `down(id)` forgets it, cancels its retry
   and pace timers, ends every idle pipe (terminate) and every taken socket (terminate); `idle(id)` counts idle pipes
   whose socket opened; `close()` downs every workspace.
5. `fill` (pipe.ts:180-209), in order: nothing when stopped, asking, waiting on a retry or on the pace. Not `ready()`:
   wait the backoff of the next attempt. Not known to serve: ask the version. With 64 taken and not yet said, log
   `<id>: 64 pipes are in use, so the next connection waits for one to close` once. `short = min(4 - idle, 64 - taken
   - idle)`; none short: stop. Drop dial times older than 1000 ms; `allowed = min(short, rate - dials in the last
   second)`; the first time only, log `<id>: dialling <short> pipe(s), 4 kept idle` (`pipe` when 1); dial `allowed`;
   when `allowed < short` schedule a fill at `oldest dial + 1000 - now`.
6. `ask`: `version(id)` (a failure reads `None`); stopped: nothing; `None`: wait the backoff; at or above 159: serves,
   fill; below: behind with `<id>: its daemon (<v>) serves no pipes yet, so it is asked again each minute`.
   Behind: log once until a pipe is taken, mark not serving, end the idle pipes, retry after `behind_again_ms`.
7. A dial (pipe.ts:306-365): an idle entry at once; a `PIPE_DIAL_MS` timer fails it; `reach(id)`: `None` fails it;
   else a websocket to `daemon_ws_url(url)`; on open: send `{"id":0,"op":"auth","token":<token>,"pipe":true}` (that
   key order) and start the idle timer (`idle_max_ms`) that retires it (close 1000, fill, no log). Frames: an auth
   answer (`DaemonResponse` with id 0) of ok marks it authed, and when the attempt count is above 0 and every one of 4
   idle pipes is authed, resets attempts and logs `<id>: 4 pipes idle`; ok false is a refusal; any other text that is
   a `pipe.open` frame takes the pipe; anything else is behind with `<id>: its daemon answered a pipe as a link, so it
   is asked again each minute`. A close the daemon made with a refused-token code and reason is a refusal; any other
   close or failure of an untaken pipe fails it.
8. A failed dial: end it, and unless stopped: log `<id>: a pipe could not be dialled, so its others were closed and the
   pool is dialled again as the link comes back` when the attempt count is 0, end every idle pipe, wait the backoff
   of the next attempt (`backoff_ms(++attempt)`).
9. A refusal (pipe.ts:248-264): end every idle pipe, count it, log its line when there is one, `rewrite(id)`, cancel
   any retry and schedule one after the wait that notes the write and fills.
10. A take (pipe.ts:274-304): leave the idle set, cancel its idle timer, reset attempts and the behind note, add to
    taken (a close removes it, clears the full note under 64, and fills); wrap it as a byte stream before anything
    else runs; log `<id>: a pipe opened on the <door> door[ after <ms> ms held], dialling its replacement` or `...,
    its replacement waits for a pipe to close` (room = taken + idle < 64, counted after the take); with no handler
    log `<id>: nothing here serves pipes yet, so that connection was closed` and close it; a handler that fails logs
    `<id>: the pipe's handler failed (<message>)` and closes it; fill.
11. The byte stream carries binary frames both ways unchanged; either end closing ends the other; a reader that stops
    reading holds the daemon back (no unbounded buffer on the host side), and a writer is held back only by the
    socket, never by a window of the pool's own.

## Edge cases
- A daemon that grabs every pipe: 4 dials a second; at most 64 taken; 60 takes log a replacement, the 4 after them
  wait; three closes give exactly three more dials.
- A daemon that refuses as full (`1008 pipe pool full`) backs off with attempts 1, 2, 3, 4, 5.
- Refusal by close (4401) or by answer (`ok: false`) both rewrite once and dial again after exactly 60 s, not 1 ms
  before.

## Behaviour nobody wrote down
- A pipe goes to the workspace it was dialled for, never one the machine names (`PipeHandler`'s first argument).
- The whole pool of a workspace falls with one failed dial, since an idle pipe learns of a dead link only when taken.

## Decisions
1. The byte stream is `PipeDuplex`: `AsyncRead + AsyncWrite + Send + Unpin`, over the socket's binary frames.
2. Timers go through the runtime's `Clock` trait (rule 3.15); `unref` is dropped.

## After the port
- `fill` logs the 64-in-use line once per crossing, but `take` clears the note only on a close below 64.

## Rules against tests
2, 3 `token_refusals_doubles_after_the_host_s_own_write_up_to_30_min` (pinned), `token_refused_line_words` (pinned);
4-10 the 16 ported cases (names in the test file, one per TS `it`); 11 the four byte-carrying cases.
