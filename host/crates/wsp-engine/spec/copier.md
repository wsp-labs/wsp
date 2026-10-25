# copier (copier.ts:1-189)

The road to the daemon binary's own copy verb, run as a child of this host with argv (never a shell line), reading
back one JSON line; and the stand-in every test above the daemon drives. No state survives anything: a copier holds
only its binary path and runner.

## The verb copier (`verb_copier`, copier.ts:62)

`run: None` means `crate::child_exec::run_child` (adapt its `ChildOptions { timeout_ms, .. }`). The runner never
fails: a child that would not start is exit 127; one killed at its deadline is exit -1 with what it printed.

| Call | argv after the binary (copier.ts) | timeout | answer |
|---|---|---|---|
| `make(ask)` | `argv_for(ask)` (:43) | `COPY_TIMEOUT_MS` 600,000 | `CopyReport` parsed from stdout |
| `remove(from, to, road)` | `copy remove --from F --to T --road R` (:109) | 600,000 | `()` |
| `worktree(ask)` | `copy worktree --from F --home H --project P --branch B`, then `--module <json>` per module in order, then `--checkpoint C` only when given (:113) | 600,000 | `WorktreeReport` |
| `worktree_mount(o)` | `copy worktree-mount --from F --home H --path P` (:117) | `MOUNT_TIMEOUT_MS` 30,000 | `()` |
| `worktree_forget(o)` | `copy worktree-forget --from F --home H --project P` (:121) | 600,000 | `()` |
| `worktree_remove(o)` | `copy worktree-remove --from F --home H --path P`, then `--force` only when true (:125) | 600,000 | `WorktreeRemoval` |
| `room()` | none: `crate::local_backend::memory_here()` (:128) | | `Option<MemoryHere>` |

- `argv_for` (:43): `copy make --from <from> --to <to>`, `--base <base>` only where `base` is `Some`, one `--exclude
  <dir>` pair per entry of `exclude` in order (an empty list adds nothing), `--size-line-bytes <n>` in decimal
  (21474836480 stays whole), `--road <clonefile|worktree>` only where `road` is `Some`.
- A module's `--module` word is `JSON.stringify(m)`: serde_json of `CarryModule` in its field order `id, lockfiles,
  carry, never, installed`, `installed` absent when `None`, no spaces.
- `has_room()` is true for the verb copier; the room reading on Linux has `total_mb > 0`.

## Reading a run (copier.ts:63-106)

1. Exit not 0: the error is `refusal_line(binary, res)` (:63). Exit 1 is the verb's documented refusal: `said` is
   `stderr.trim()`, or `stdout.trim()` when stderr trims empty; the error is said's last line (split on `\n`); with
   both empty, `the copy failed and said nothing`. Any other exit (2 for a clap parse error, -1 for a kill, 127):
   when `said` is not empty it goes to the host's log on stderr as `<binary> copy: <said>`, and the error is
   `copy_verb_failed_line(exit)`, e.g. `the daemon's copy verb failed (exit 2); the host log has its output`. The
   usage text never reaches the error.
2. Exit 0 with a JSON answer (`make`, `worktree`, `worktree_remove`): `said = stdout.trim()`; parse it as JSON, then
   through the wire type and its zod `Check`. A parse failure, a wrong shape, an unknown enum value (road `rsync`), or
   empty stdout is the error `the <what> verb answered something this host does not read: <said's first 200 UTF-16
   units, or "nothing at all" when said is empty>`, where `<what>` is `copy`, `worktree` or `worktree removal`.
3. `remove`, `worktree_mount`, `worktree_forget` read nothing on exit 0.

## The stand-in (`fake_copier`, copier.ts:135)

Records every call in `log`, in call order: `asks`, `removed` as `(from, to, road)`, `worktrees`, `worktrees_mounted`
(the path), `worktrees_forgotten` (the project), `worktrees_removed` (the whole ask). Never fails.
- `make`: `script(ask)` when a script was handed; else road `ask.road` or `clonefile`, path `ask.to`, base 40 `0`s,
  branch `main`, fetched true, carried `deps-and-config`, excluded a copy of `ask.exclude`, bytes 1024, ms 1, no
  `skipped`, no `fell_back`.
- `worktree`: path `<home>/worktrees/<project>/<branch with each char outside [A-Za-z0-9._-] replaced by ->`, made
  with `create_dir_all`; answers that path, the branch, made true, carried `[]`, fresh true, modules `[]`, ms 1,
  `plain` and `overlaid` absent.
- `worktree_remove` answers `{ path: o.path }`, no `rescued`.
- `has_room()` is true exactly when a room closure was handed; `room()` answers it, else `None`.

## Rules against tests

| Rule | Tests (tests/copier.rs) |
|---|---|
| argv order, one word per exclusion, decimal size, the timeout | `is_run_with_one_word_per_exclusion_and_answers_the_report_it_printed` |
| base and road only when named | `names_the_base_and_the_road_on_the_line_only_where_the_caller_named_them` |
| exit 1: the last line of stderr | `throws_the_verbs_own_last_line_when_it_refused_so_a_person_reads_why` |
| any other exit: one sentence, no usage text; exit -1 with nothing said | `turns_any_other_exit_into_one_sentence_naming_the_verb` |
| an answer the wire type refuses | `refuses_a_line_this_host_does_not_read_rather_than_recording_half_a_copy` |
| module JSON words in field order; checkpoint flag only when given | `hands_the_worktree_verb_each_module_as_one_json_word_and_reads_back_the_modules`, `hands_the_worktree_verb_the_checkpoint_a_fork_starts_from_and_no_such_flag_without_one` |
| mount, forget, remove argv and refusals | `mounts_a_worktrees_overlays_again_by_its_path_and_throws_the_verbs_own_sentence`, `takes_a_projects_frozen_copies_away_by_its_id_and_throws_the_verbs_own_sentence`, `takes_a_copy_away_by_the_road_that_made_it` |
| room: real reading, stand-in only when handed | `is_the_kernels_reading_on_the_real_road_and_on_the_stand_in_only_what_a_test_hands_it` |
| the stand-in's records and default report | `records_what_it_was_asked_for_and_answers_a_report_of_the_road_it_was_told_to_take` |
| empty refusal, stdout fallback, exit 127 | copier_pins.rs: `a_refusal_that_said_nothing_has_its_own_sentence_and_stdout_stands_in_for_an_empty_stderr` |
| the bare line, quotes and spaces as one word each | copier_pins.rs: `a_copy_with_no_exclusion_base_or_road_is_the_bare_line` |
| stand-in folder name per UTF-16 unit, folder made, ask recorded | copier_pins.rs: `the_stand_ins_worktree_folder_replaces_every_char_outside_the_safe_set` |

Unclear in the TS: the warn on stderr has no test in either language; the Rust writes it with `eprintln!` (the
crate has no log facade yet).
