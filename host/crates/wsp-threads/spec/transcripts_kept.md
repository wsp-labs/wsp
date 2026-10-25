# transcripts_kept: sent images, the sessions document, daemon tokens

TS: `T/transcripts.ts:17-92, 513-545, 590-657, 716-735` at `65f6b89c7`. Rust: `src/threads/transcripts/kept.rs`,
methods on `TranscriptsArea`. Area state: `kept_writes: IndexMap<ws, Joined>` (one chain per workspace, the caller
gets the swallowed promise), `machine_tokens`, `daemon_tokens: IndexMap<machine, {has_daemon, at}>`,
`writing: IndexMap<machine, Shared<..>>` (a join), `viewed_marks: IndexMap<ws, IndexMap<path, blob id>>`,
`accrued: IndexMap<ws, f64>`. Collections: `attachment-keys` (record `{threads: {<thread>: [{key, mediaType}]}}`),
`attachments` (blobs), `sessions`. `DAEMON_TOKEN_MISS_TTL_MS` 60,000. `SESSION_INDEX_CAP` 200.

## Kept images

K1. The chain (`onKept`, `:24-37`): each step runs after the workspace's previous one. It reads `attachment-keys/<ws>`
(`{threads: {}}` when absent), runs the step; a step answering `None` wrote its own document; else a document with no
threads is deleted and any other put. A failure anywhere warns `the images <ws>'s messages carried were not kept as
asked: <message>`; the answered future never fails.

K2. `keep_sent_images(ws, thread, request, attachments)` (`:39-53`): the images among the attachments (by
`isImage(mediaType)`) whose `attachmentKey(thread, request, index)` exists, `index` being the attachment's place in
the whole list. None answers at once with no chain turn. Else one turn: the thread's entries less those keys, then the
new ones `{key, mediaType}` in order, put as the document (other threads kept) **before** the bytes; then each blob
`attachments/<key>` from the base64 bytes, in order. A request id or thread that is not a plain id keeps nothing.

K3. `drop_kept_keys(ws, keys)` (`:55-65`): a turn that writes nothing when no thread's list holds any of the keys;
else deletes every named key's blob (in the order given), drops those entries, drops threads left empty, and the
document is put (or deleted when empty, K1).

K4. `drop_sent_images(ws, threads)` (`:86-92`), `threads` a list or all: the named threads that hold entries (all
of them for all); none writes nothing; else every blob of theirs is deleted and the threads leave the document.

K5. `image_keys_of(e)` (`:67-73`): a `session.start` or `session.steer` with a thread: the keys of its image
attachments in attachment order; any other event, none. `trimming(ws)` collects them from every dropped event and,
when done, drops them with `drop_kept_keys` (detached) only when there are any.

## The sessions document

K6. `cap_sessions(ws)` (`:590-609`): with more than 200 rows on the workspace, the excess goes: of the rows not
running and not their thread's newest (`threadKeyOf`: thread id, else row id; the later of equal `startedAt` is the
newest), oldest `startedAt` first (stable). Each goes from `sessions`; its thread's record, when there is one, is set
to `{...record, ended: <the row's status>}` keeping its map place. A running row and a thread's newest row never go,
so the count can stay over 200.

K7. `persist_sessions(ws)` (`:617-657`): a workspace not live writes nothing and answers at once. Synchronously:
every row of the workspace that is settled and did work (`threadRan([view])`) sets `worked: true` on its thread's
record where that record reads `worked: false` (absent stays absent); then `cap_sessions`; then the document
`{workspaceId, sessions, threads, viewed?}` in that key order (rule 2.36): `sessions` is the workspace's rows in map
order, less rows with a `launch` in flight, each `{...view, turnId, notify?, notifyBy?, notifyRoad?, <while
running: reply?, told?, steered?, waited?, waitingSince?, run?, from?, asked?, turnToken?, scopeDeviceId?>,
snapshot?}`; `threads` the records of this workspace in map order keyed by thread with `workspaceId` left out;
`viewed` only when the workspace's marks are not empty. The put goes on `ctx.index_flushes()`'s chain for the
workspace (so an earlier snapshot never lands after a later one); a failed put is swallowed with no log.

## Daemon tokens

K8. `token_for_machine(id)` (`:521-527`): made once per machine and kept for the host's life: `daemonTokenFor(seed,
id)` when the host was given a seed (`opts.daemonToken`, whose shape is checked when the area is built:
`assertTokenShape` refusing fails the build), else 24 random bytes from `ctx.ids()` as 48 lowercase hex.

K9. `daemon_token_of(machine, path)` (`:532-544`): a cached answer stands when the machine had a daemon, or when it
had none less than 60 s ago (clock); it answers the token or `None`. Else the rotation in flight for the machine, or
a new `rotateDaemonToken(machine, token, daemonTokenPathOf(machine, path))`, shared by every caller until it settles;
its answer is cached `{has_daemon, at: now}` and answers the token or `None`. A failed rotation fails each caller and
caches nothing.

## Reads for other areas

K10. On every `workspace.cost` event, `accrued[ws] = accruedUsd` (listener at construction, `:718-720`).

K11. `ran_on(ws, thread, session)` (`:723-729`): `recordedPicks` over the thread's rows on that workspace sorted by
`startedAt`; with a session, a model the rows do not say is read with `ctx.resumed_fact(ws, session, "model")` and
an effort with `"effort"`, laid under the rows' picks (`{...modelPicks(model), effort?, ...recorded}`).

K12. `box_of(thread)` (`:731-735`): the workspace of the thread's latest row, else of its record; `None` when that
workspace is not live or is local.

## Tests (`tests/transcripts_kept.rs`)

| Rule | Test |
|---|---|
| K1, K2 | `keep_sent_images_writes_the_entry_before_the_bytes`, `keep_sent_images_keeps_none_without_a_plain_request`, `two_sends_at_once_keep_both_threads_entries`, `a_failed_image_write_warns_and_the_chain_goes_on` |
| K3, K5 | `drop_kept_keys_deletes_blobs_and_empties_the_document`, `drop_kept_keys_naming_nothing_kept_writes_nothing` |
| K4 | `drop_sent_images_takes_named_threads_or_all` |
| K6 | `cap_sessions_drops_the_oldest_finished_rows_past_two_hundred`, `cap_sessions_keeps_each_threads_newest_and_running_rows`, `a_capped_row_marks_its_thread_ended` |
| K7 | `persist_sessions_writes_the_document_in_the_ts_key_order`, `persist_sessions_keeps_live_fields_only_while_running`, `persist_sessions_leaves_out_unlaunched_rows_and_other_workspaces`, `persist_sessions_of_a_workspace_not_live_writes_nothing`, `persist_sessions_marks_a_settled_working_thread_worked`, `persist_sessions_lands_snapshots_in_call_order` |
| K8 | `token_for_machine_is_made_once_per_machine_from_the_seed_or_random` |
| K9 | `daemon_token_of_shares_one_rotation_and_caches_a_miss_for_a_minute` |
| K10, K11, K12 | `accrued_follows_the_cost_events`, `ran_on_reads_the_rows_then_the_sessions_facts`, `box_of_is_none_for_a_local_workspace` |
