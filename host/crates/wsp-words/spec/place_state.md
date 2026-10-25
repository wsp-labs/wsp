# place_state
Source: the functions of `packages/protocol/src/place-state.ts` (379 lines) at a3b870a6e; its flat constants (`THREAD_MEM_MB`, `CLOUD_CAP_DEFAULT`, `DELETE_AFTER_MS`, `CAP_RAISE_ACT`, ...) are `wsp_wire::types::place_state`. Tests: `packages/protocol/test/place-state.test.ts` (19 of 25 cases ported; the 6 of `describe("the word a place's row says")`, :175-271, need `placeStateOf` of `views/place-setup-words.ts`, which is not in this crate yet), plus `pinned` in `tests/place_state.rs`.

## What it does
What a place's cap is and what it counts: which place a workspace runs on, the settings each kind of place takes and how a row says them, the room left under the cap, and the lines a thread waiting for a slot is told.

## Behaviour
1. `workspace_place`: the record's `place` (place-state.ts:16). `workspace_place_id`: the place its record names, else this computer for a local workspace, else the row of the provider its record was stamped with, else the first provider row for a fork with no stamp; None where the list holds no such row (:29-42).
2. `threads_at_once(size)`: `max(1, min(2 * floor(cpu), js_round(memMb / THREAD_MEM_MB)))` (:44).
3. `nap_ms_of(minutes)` and `turn_limit_ms_of(hours)`: 0 is None (never naps, no limit), else minutes * 60 000 or hours * 3 600 000 (:60, :76). `setting_for(own, placed, fallback)`: own where present, else placed, else fallback; a present None (the TS null) is a value and stops the chain (:80-82).
4. `phase_holds_slot(w)`: the phase's state is not paused, gone or released, and the workspace is not queued with a queue entry (:87-90).
5. `place_setting_named` and `place_setting_dropped`: a setting with parts (the spawn switches) is named where any part is set; dropping it keeps the other parts and drops `spawn` where none is left (:181-195).
6. `place_cap_of`, `place_turn_limit`: per kind's rule in PLACE_CAPS; the turn limit is the settings' `turnLimitMs`, else the kind's default (off on a computer, six hours on a cloud) (:241-249).
7. `place_set_refusal(place, set, reset)`, in this order: nothing named and nothing reset; a word the kind does not take; a word both set and reset; a size the row does not offer (`refusalLine(sizeRefusal(..), SIZE_PICK_FIX)`); else None. An added computer (not this one) also lists "a new name" and "a new ssh login" in the first refusal (:253-270).
8. `place_takes`: the kind's keys; `nap` only where the row takes forks; `size` only where it offers sizes (:277-281). `place_settings_line`: per taken setting, "<value> (the default)" when unset, else "<value> (<default short> by default)" (:287-297).
9. `running_on`: 0 for an unknown place id; else the kind's count of the workspaces on that place and their threads (:301-309). `place_room`: None without a cap; room is `max(0, atOnce - running)` (:311-317). `place_full_line`: "full: N of M <noun>s running" when room is used up, else "no room on <name>" when forks report no room (:321-325).
10. Cap lines (:329-365): the running line, the wait line and bar words by `why` (slot, minute, threads), the started and stopped lines, `deleted_before_start_line`, `cap_restarted_line`, and `cap_raise_act` ("Raise machines at once" for a slot, None for a minute, else CAP_RAISE_ACT).
11. `place_spend_limit` and `place_at_limit_line`: a cloud's `spendPerDayUsd` only; the line only when spend today is at or over it (:371-379).

## Edge cases
- `nap_ms_of(-1)` reads -60000; `turn_limit_ms_of(1.5)` reads 5400000 (pinned).
- `setting_for(Some(None), ..)` reads None: a nap set to off stays off.
- `running_on` for an id not in the list is 0, not a panic.

## Behaviour nobody wrote down
- Rule 8.12: `threads_at_once` rounds with `js_round`.
- `dollars(usd)` writes a whole dollar amount as `$10` and anything else through `fmt_cost`.

## Tapes
`places.list` rows (cap, running, settings line), `session.capWait` and `session.uncapped` events, `wsp places set` refusals.

## Decisions
1. `place_set_refusal` takes `reset: &[PlaceSettingWord]` (TS default `[]`); `place_cap_of` takes `Option<&PlaceCapSet>` (TS default `{}`).
2. `setting_for<T: Clone>` is generic; a nullable setting is `T = Option<_>`, so absent (outer None) and null (`Some(None)`) stay apart.
3. `running_on` answers `usize`.

## After the port
- `nap_ms_of` and `turn_limit_ms_of` take a negative number and answer a negative window; the callers' schemas refuse one first.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `threads_at_once::*` |
| 3 | `one_rule_for_each_setting::*`, `pinned::minutes_and_hours_turn_into_windows_zero_is_off`, `pinned::a_setting_falls_through_only_when_absent` |
| 4, 9 | `what_runs_on_a_place::*`, `a_held_spawns_words_and_the_slots_a_cloud_counts::a_queued_spawn_waiting_holds_no_slot_until_its_machine_is_being_made_and_a_relea` |
| 5, 7 | `threads_at_once::refuses_a_number_the_rows_kind_does_not_take_set_or_reset_a_set_with_nothing_in_` |
| 6 | `the_turn_limit::*` |
| 8 | `what_a_places_settings_read_as_in_a_line::*`, `the_turn_limit::reads_in_the_settings_line_at_what_it_runs_at_with_the_default_beside_one_the_pe` |
| 10 | `a_held_spawns_words_and_the_slots_a_cloud_counts::*`, `pinned::held_turn_lines` |
| 11 | not pinned: its TS test sits in the skipped describe (needs `placeStateOf`) |
