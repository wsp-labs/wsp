# turns_picks: what a start carries

Port of `packages/host/src/verbs/turns-help.ts` lines 1-237 and 421-519 at 65f6b89c7 (`/root/cli-c-ts/turns-help.ts`).
Rust: `src/verbs/turns_help/picks.rs`. Probe: `tests/fixtures/cli_c/record/turns_picks.probe.ts` (views in `turns_views.ts`).
Recorded cases: `tests/fixtures/cli_c/turns_picks.json`, one test each in `tests/cli_c/turns_picks.rs`. Drivers:
`tests/cli_c/drivers_turns_picks.rs`. Hand tests: `tests/cli_c/turns_picks_rules.rs`.

## Scope

`Picks` and `picksOf` (138-148), `IMAGE_HEAD_BYTES` (151), `filesFrom` (159), `pickFlags` (184), `checkedStart` (197),
`projectDefaultsOf` (220), `preferencesOf` (225), `worktreeFor` (423), `openingOf` (434), `ThreadFork` and `threadFork`
(462, 469), `resumeAsked` (479), `replacedOf` (490), `notifyOf` (500), `messageTo` (516). The module's own `Turn` and the
rest of the file after 519 belong to `follow` and `stream`. The zod `ThreadDefaults` and `Preferences` are the wire types.

## Behaviour

- `picksOf` (141): writes `model`, `effort`, `access` (through `accessWordOf`), `fast` in that order; a pick that is
  absent is left out, an empty string is kept for model and effort, `fast` is kept only when true. An access that is no
  word of ask, auto-edit, full, plan (a harness's own spelling, an empty string, other capitals) is a usage refusal.
- `filesFrom` (159): with `elsewhere` and any path, refuses at once (guest rule). Otherwise stats every path in turn (a
  missing path and a folder are one refusal, the first one named, quoted as given), reads the first 12 bytes of each,
  tells an image by `imageTypeOf` (png, jpeg, gif, webp, by bytes, never by name), then asks `filesRefusal` over the
  weights (count over 5, then each file in order: empty, image over 10 MiB, other file over 10 MiB), and only then reads
  each file whole as base64. Each attachment is `mediaType`, `name`, `bytes`.
- `pickFlags` (184): `model`, `effort`, `access` where the flag is a string, then `fast` where it is the switch `true`.
- `checkedStart` (197): see the order below. A start on this computer (no workspace, no `forkOf`) checks the task and
  the agent's name only; the picks are the start's to read there.
- `projectDefaultsOf`, `preferencesOf`, `worktreeFor`: one op each, the answer read through its schema (schema order,
  defaults filled, unknown keys dropped).
- `openingOf` (434): the `sessions.start` fields in the order in "Order". `here` takes its folder from the target and
  never from the options; every other target takes the options'. A `resume` drops the folder before it is read.
- `threadFork` (469): reads `--at` as JavaScript's `Number()` does, then `threadAt`, then `checkedStart` against the thread's
  own agent (or the one named) and its workspace.
- `resumeAsked` (479), `replacedOf` (490), `notifyOf` (500), `messageTo` (516): as the doc comments in `picks.rs` say.

## Limits and caps

`FILES_MAX` 5 files (cases: 1, 4, 5 fit; 6 and 7 are refused with their count). `FILE_MAX_BYTES` and `IMAGE_MAX_BYTES`
10 MiB each, checked on the stat size (cases: 10 MiB - 1, 10 MiB fit; 10 MiB + 1, 10.5 MiB, 300 MiB are refused, for a
file and for an image). `IMAGE_HEAD_BYTES` 12 (a webp of 12 bytes is an image, of 11 is a file; a png of 8, a jpeg of 3, a
gif of 4 are images, one byte shorter are files). `--at` is a safe integer of 1 or more (9007199254740991 passes,
9007199254740992 is refused; 0, -1, 1.5, "", "-", "NaN", "Infinity", "1_0", "2nd" are refused; " 3 ", "+3", "1e2", "0x10",
"0b11", "0o7", "3.", "007" pass). Files near the cap are answered as `{ mediaType, name, length }` (14 MB of base64 does
not belong in a fixture): the probe's `summary` argument, mirrored by the driver.

## Null and empty inputs

No paths, no refs, no notify list, no resume id, no picks: each answers nothing and sends nothing. An empty prompt, harness,
title, branch, folder and model are sent as they are; an empty folder is refused (not absolute), an empty access is refused,
an empty `--at` is 0 and refused. An empty `notify` list is sent as an empty list. The empty ref names every thread, so
two or more threads: "n threads start with ; give more of the id". `fast: false`, `"fast"` as a word or a list are not fast.

## Order

`checkedStart`: empty task (no frame), `harnesses.list` (with `{workspaceId}` or none), `projects.defaults` only for a `forkOf`
with no agent named, a second `harnesses.list` (no params) only for a named agent the workspace's list lacks, then the picks:
access word, model, effort, fast, then the access mode. `openingOf` frame: target fields (`workspaceId`; `project` `branch`;
nothing; `fork` `branch`), `prompt`, `cwd`, `harness`, `notify`, `turnToken`, `title`, `replaces`, `attachments`, `resume`,
`model`, `effort`, `access`, `fast`. Refusals in `openingOf`: the folder, then the files, then the access word. `messageTo`:
`workspaceId`, `prompt`, `harness`, `thread`, `attachments`, picks; the files are read before the picks are checked.
`notifyOf`: one `sessions.list` per thread ref, in the order named, and the first bad ref stops the rest. `threadFork`: `--at`
(no frame), `sessions.list`, `workspaces.get`, `harnesses.list`.

## Invariants held

- Guest rule: a caller on a machine (`elsewhere`) never has this computer's disk read. `filesFrom`, `openingOf` and `messageTo`
  refuse a path before it is resolved, statted or opened.
- Host invariant 7 (cloud off, no line names a cloud) reaches this unit only through `threadFork` -> `threadAt`: a ref that
  names a machine is only a ref that names no thread.
- Frame field order (the wire text of a frame is part of the contract).

## What the TS leaves unclear

- A path with a trailing slash on a file (`a.txt/`): `statSync` with `throwIfNoEntry: false` still throws ENOTDIR as a raw Node
  error. Not recorded (the message cannot be reproduced); the Rust may refuse it as a file that is not there.
- The caps are read from the stat size and the file is read again afterwards; a file that grows between is not re-checked.
- `Number()` accepts more than a person types (hex, binary, octal, exponent, spaces): pinned as it is.
- `threadFork` passes `opensThread = true` to `startPicks` even for a thread that exists: it only checks, the answer is unused.
- `--at ""` is refused with the text "from 1, not." with no word after "not".
- `checkedStart` with both a workspace and a `forkOf` reads the project's defaults though the workspace names the table.

## Rule -> test

| Rule | Tests |
|---|---|
| picks order and absent picks | `picks_of__*`, `pick_flags__*` |
| access word refused the same at every door | `a_bad_access_word_is_one_sentence_at_every_door` |
| file caps, count, first thing wrong, weigh before read | `files_from__*`, `caps_are_checked_before_any_file_is_read_whole` |
| image head is twelve bytes, told by bytes | `image_head_is_twelve_bytes`, `files_from__a_webp_*`, `files_from__a_png_*` |
| guest rule, nothing touched | `guest_files_from_refuses_before_any_path_is_touched`, `guest_opening_of_and_message_to_refuse_before_any_path_is_touched`, `files_from__elsewhere_*`, `opening_of__elsewhere_*`, `message_to__elsewhere_*` |
| a pipe is no file | `a_pipe_is_no_file_and_is_not_opened` |
| invariant 7, cloud off | `cloud_off_a_fork_ref_never_names_a_machine`, `cloud_on_a_fork_ref_names_a_machine` |
| frame order, each target | `opening_of_field_order_for_a_workspace`, `..._for_a_folder_target`, `..._for_a_project_fork`, `..._for_a_thread_fork`, `..._with_a_resume`, `message_to_field_order` |
| turn token is `WSP_TURN` only | `opening_of_reads_only_the_turn_token` |
| checkedStart refusals, reads, orders | `checked_start__*` |
| `--at` as `Number()`, safe integer from 1 | `thread_fork__a_turn_count_*` |
| own refusals are usage refusals | `own_refusals_are_usage_refusals` |
| notify and replaces reads | `notify_of__*`, `replaced_of__*` |
| resume and copy | `resume_asked__*` |
| null and empty inputs | the cases named "none", "an empty ...", "nothing named" in every group |
| ported TS tests | `ported_ts_tests_are_recorded_cases` (resume-flags 3, verbs messageTo 3, parity notify 2, verbs-turns intents) |
