# slates_acts
Source: slates.ts:470-474 (the press state), 1290-1448 (`event`, `approve`, `revoke`, `cancel`), 1490-1493 (`image`)
at 65f6b89c7. Tests: slates.test.ts `takes five of the person's events a second`, `the person revokes an
approval`, `keeps a link's domain`, `a held run whose command the agent rewrote asks again`, `a second press on a
confirmed run`, `an image piece reads a file`, the press halves of `runs the setup chain` and round 4.

## What it does
What a person does on a window: a press, a submit or a change on a piece; an answer to an approval sheet (once,
always in this thread, not now), including a link's domain and the slate's permission to message the agent; a
revoke of a standing approval; a cancel; an image read.

## Behaviour
1. `event(p)` (:1290-1344), with `now` read once: request ids older than 600,000 ms are forgotten; a request id
   already held answers the first answer (a resend after a reconnect). The thread's bucket refills 5 a second up to
   5; below one token refuses conflict `V754 event-rate: too many presses; try again in a moment` (not remembered);
   else one token is taken and, at 500 held ids, the oldest dropped. On the chain: `need_record`; the piece is read
   off the host's own document: gone refuses usage `That part of the slate is gone (piece <p>, version <v>; the
   slate is at <r>).` / `Press it again once the slate has redrawn.`; a row action the piece lacks refuses usage
   `<p> has no row action <n>.` with the same fix; a scoped event's item is the host's own row of the piece's
   `items` at the index (the window's item only when the list does not resolve to an array). The shared start budget
   is released; one batch `by: person` with the event `{piece, kind, [item], [index], [rowAction]}`, the request id
   and `now` as the send time. Then off the chain: the sends delivered (a failure refuses `This thread cannot take a
   message now: <message>` with its kind or conflict); with no held cmd ask and tool runs started, up to 5,000 ms
   for the MCP table's sheet; the ask is the held one's, or the first tool ask of a run this press started. Answers
   `{outcome, said: SAID[kind][outcome], [turnId], [ask]}` when delivered, `{outcome: "held", said: "Needs your
   approval", ask}` when held, else `{outcome: "done", said: ""}`. A refused answer is forgotten so a press again
   is new.
2. `approve(p)` (:1346-1416), after `need_record`:
   - `send`: `refuse` drops `sendsAllowed`, else stamps it now; saved.
   - a `domain:` key: the rest must match `^[a-z0-9.-]+$` or refuses usage `<key> names no domain.` / `Approve a
     link as domain:example.com.`; `thread` keeps `{state:"allowed", at, cmd: "links to <domain>"}`, `refuse`
     deletes it, `once` keeps nothing; saved.
   - a key the MCP table owns: `refuse` keeps `{state:"refused", at, ...names}`, saves, then denies; else `thread`
     keeps it allowed, views kept, saved, then the table approves with the scope.
   - otherwise the declared `cmd` runs whose approval key is this one; none refuses usage `this slate declares no
     command with approval key <key>.` / `Read the slate again and approve what it asks now.` For `thread`, a run
     whose pin cannot take an Always refuses usage with that sentence and fix `Approve it with scope once.` On the
     chain: `refuse` keeps the refusal, saves, and denies each run; else `thread` keeps the approval, views kept, and
     each run reading held or running is started by the person: the runs table's answer, or a running record, is
     enough; a hold this process never saw (from before a restart) is started again, failing `NO_FOLDER` with no
     folder, and approved at once when it holds; `thread` re-arms timers; saved.
3. `revoke(p)` (:1418-1442), after `need_record`, on the chain: `send` with none allowed refuses usage `this thread's
   reactions were not allowed to message the agent.` / `Read its approvals again.`, else drops it; a key not held
   refuses usage `this thread holds no approval <key>.` with the same fix; else deletes it, cancels every declared
   run (cmd by run key, others by `code_key`) it covered in both tables, re-arms timers without them, saves.
4. `cancel(p)` (:1444-1448): `need_record`, then both tables cancel the run; not on the chain.
5. `image(p)` (:1490-1493): `need_record`, then `slate_image(src, {thread, have, folder, on: image_on, allowed: a
   domain approval reads allowed})`.

## Edge cases
- Five presses at once pass, the sixth refuses V754, a resend of the first answers its answer, and the value moved
  by exactly five.
- Revoke: the run reads cancelled, the approval is gone, the timer stays off until a write arms it again, and then it
  asks with the same key; a domain approval revokes; `nope` refuses `holds no approval nope`.
- A held run rewritten by the agent asks again for the new command with a new key; Run once runs the new one.
- A confirm run still running: a second press asks with the same key and leaves it running; refuse drops only the
  new start.
- `domain:../x y` refuses `names no domain`.

## Behaviour nobody wrote down
- Rule R2: `event`'s rate, request ids and refusals before the chain are synchronous and never queue.
- The op list: `slates.revoke` is in neither the thread nor the device op list (the person's alone); that is the
  serve layer's, tested there.

## Decisions
1. `event` answers a `Shared` future so a resend awaits the same answer.

## After the port
- None.
