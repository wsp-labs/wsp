# internal-records: collections, log lines, records and the readers that take them back

TS: `packages/runtime/src/types/internal.ts:44-228, 556-863, 1068-1259` (about 560 lines) at `65f6b89c7`. Rust:
`src/types/internal.rs`. Tests: `tests/internal_records.rs` (106), against `tests/fixtures/core_types/internal.json`.

## Collections and keys (internal.ts:44-158, 841-845)

The store's collection names are fixed strings (`workspaces`, `projects`, `seed-choices`, `goldens`, `golden-recipes`,
`project-goldens`, `images`, `image-vaults`, `transcripts`, `transcript-heads`, `transcript-index`, `attachments`,
`attachment-keys`, `sessions`, `notify-owed`, `held-starts`, `sub-starts`, `workspace-names`, `dropped-machines`,
`unrecorded-machines`, `preferences`, `reads`, `agent-lists`, `builders`, `owner`, `creates`), with `PREFERENCES_ID`
"default" and `READS_ID` "since". Chapter 02's fresh schema decides where each lives; these names are what the
runtime passes to the store. Pinned by `constants_collection_names_match_the_ts`.

- `copy_key(place, name)` is `place/name`. `copy_key_parts(key)` splits at the **first** `/`: no slash is no place;
  `/x` is place `""`, not none; `x/` is name `""`; `a/b/c` is place `a`, name `b/c` (`copy_key_parts_*`).
- `recipe_key(name, v)` and `vault_key` are `name@v<v>` (`recipe_key_*`, `vault_key_*`).

## Timings and caps

`OWED_RETRY_MS` 30 000 doubling to `OWED_RETRY_MAX_MS` 600 000 for `OWED_FOR_MS` 3 600 000; `DROPPED_WATCH_MS` 7 days;
`UNRECORDED_DELETE_MS` 24 h; `DAEMON_TOKEN_MISS_TTL_MS` 60 000; `KEPT_CLOSE_WAIT_MS` 2 000; `KEY_PURPOSE_MAX` 238;
`LOST_CREATE_MS` 120 000; `HELD_TTL_MS` 15 min; `HEARTBEAT_MS` 5 min; `SESSION_FACTS` cwd, permissionMode, model,
effort; `OPTIONAL_BACKEND_CALLS` the ten names in the TS order. Pinned by `constants_*`.

## Log lines and reasons (internal.ts:160-208, 293)

Exact sentences, each pinned by its own test. Ids are cut with JavaScript's `slice(0, 8)`, which counts UTF-16 units:
`"😀😀😀😀😀"` keeps four emoji (`no_title_log_line_cuts_the_session_id_at_eight_utf16_units_so_five_emoji_keep_four`).
Which ids are cut: the session id in `no_title_log_line` and `no_name_write_log_line`, the thread id in
`no_made_title_log_line`, `no_checkpoint_log_line`, `lost_session_log_line` (whose session id stays whole) and
`stop_log_line` (its task too). `label_of` is the record's name, or `thread <first 8 of the id>` where a thread named
the verb. `swept_runs_log_line` says `1 harness run` for one and `<n> harness runs` otherwise, 0 included, and joins
the runs with `, ` (empty after `: ` for none). `server_values_left_line` and `server_values_cut_line` say the caps with
`fmt_bytes` (`32 MB`, `2 MB`); the cut line joins files with ` and `. `restart_cut_line(ms)` is `cut by a host restart
after <fmt_duration(ms, clock)>`: `0m 00s`, 999 ms as `0m 01s` (rounded), `1m 01s`, `1h 00m 05s`.

## Stored records

Each serde struct writes camelCase keys in the TS interface's order, an absent option left out, a `true`-only flag
(`building`, `owed`, `awaitsPrompt`, `cloudGone`) written only where set, whole numbers without `.0` (`JsNumber`).
`HeldStartRecord.notifyBy`/`notifyRoad` are kept as the JSON they were. Pinned by the `record_*` round trips, two per
record (every option present; every option absent), and the whole-number tests.

What survives a restart: everything in this section, read back from the store. What does not: `TurnLive.told_as`,
`TurnLive.failure` ("in memory alone", internal.ts:591, 600), `KeptProcess`, `LiveBuilder`, `LiveSession`,
`SessionEntry`'s handle, end, launch and calls. A running row's `reply`, `told`, `steered`, `waited`, `waitingSince`,
`run`, `from`, `asked`, `turnToken`, `scopeDeviceId` and `snapshot` do survive, on `IndexedSession`
(internal.ts:827-834); `toldLast` is an older host's, read and dropped by the area that reads it.

## Reading back (internal.ts:619-632, 741-762)

The store's JSON is read back field by field and never trusted whole: a line delivered after a restart must not start
under a caller nobody wrote.

- `read_steered(raw)`: None unless `raw` is a JSON object (null, an array and a string are None). Each entry, in the
  object's order: dropped unless `prompt` is a string and `startedBy` reads as a `SessionOrigin`; dropped where `by`
  is present and does not read as a `ThreadScope` (`{kind: "thread", threadId, workspaceId, rootThreadId}`), or `road`
  is present and does not read as a `WorkspaceOrigin`; a non-object entry (null) is dropped. A kept entry has `by` and
  `road` where they read, `requestId` only where it is a string, and `attachments` only where it reads as a list of
  `AttachmentRecord` **and is not empty**. `{}` reads as an empty map, not None. Pinned by the 13 `read_steered_*` tests.
- `read_asked(raw)`: None unless an object (not an array) with a string `prompt`; `effort`, `typed`, `requestId` kept
  only where strings, `owed` and `awaitsPrompt` only where exactly `true` (`read_asked_*`).
- `read_scope(raw)` and `read_road(raw)`: None for absent and for JSON null, the schema's value where it reads, None
  otherwise; `kind: "device"` is not a thread scope, `"paired"` is a road (`read_scope_*`, `read_road_*`).

## Launches (internal.ts:681-690)

`launches_as(kept, send)`: the same `fixed` string, and every pick the send names equal to the kept one's pick of that
name by JSON value, a pick the kept launch lacks reading as absent, which equals nothing the send names (JSON null
included). A kept pick the send does not name does not matter. Pinned by the `launches_as_*` tests.

## Refusals (internal.ts:943-964, 1070-1090, 879)

- `PrepareStoppedError::new(id, outcome)` and `message()`: `prepare stopped before a machine existed` with no id;
  `prepare stopped; builder <id> killed` with no outcome; `... left running with a seal still in it` with `Kept`
  (`kept` true); `... did not stop: <why>` with `Left(why)` (`left` set). Pinned by the four `prepare_stopped_*`.
- `DaemonRefusal { code, message }`; `is_no_host_cli`, `is_no_git_credential`, `is_on_default_branch` compare the code
  exactly; no code is none of them (`refusal_*`). The TS also answers false for any error that is not a
  `DaemonRefusal`; in Rust the type says so.
- `default_branch_refused(road, workspace, branch)` is wsp-words' `refusal(default_branch_refusal(workspace, branch),
  default_branch_fix(road, workspace), Usage)`: message, fix and kind pinned for fork, run and bring back.
- `is_cap_refusal(e)`: an `EngineError::Provider` of kind concurrency, nothing else (`is_cap_refusal_*`).

## Interface only

`KindModule` (internal.ts:1097-1214) is the per-kind table the machines area fills; its optional members answer None.
`SessionEntry`, `LiveSession`, `KeptProcess`, `LiveBuilder`, `TurnLive` and the small shapes after line 1215 are plain
holders with no behaviour here.
