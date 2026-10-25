# views_place_setup_words
Source: every function and flat constant of `packages/protocol/src/views/place-setup-words.ts` (317 lines) at a3b870a6e; its one zod shape, `PlaceEstimate` (:218), is `wsp_wire::views::place_setup_words::PlaceEstimate`, and the tables it reads (`SETUP_STEP_WORDS`, `SETUP_STEP_CLASS`, `provisionCounts`) are wsp-wire's. Tests: `packages/protocol/test/places.test.ts` (9 of the 13 cases of "the setup on a computer you own", :441-563, reach this unit and are ported in `tests/place_setup.rs`, `places::the_setup_on_a_computer_you_own`; :452 and :459 parse zod shapes and :465 reads SETUP_STEP_CLASS, all wsp-wire's; :562 is project_path's, ported in the same module), `sign-in-ways.test.ts` (3 of 3, `sign_in_ways`), `place-state.test.ts:214-267` (6 of 6, `place_state::the_word_a_place_s_row_says`), plus `pinned` in `tests/view_wire_words.rs` (3 of its 4 tests reach this unit). `setup-fix.test.ts` (3 of 3, on `setupRowFix`) is ported in `tests/setup_fix.rs`.

## What it does
The words of a computer's setup: the one state word its row reads (Setup failed, Needs you, Pending, absent, Setting up, Behind, Ready, then the cap words), the TOOLS cell, the lines a terminal prints once a setup ends, the sign-in ways a row offers, the refusals around an add, and the lines the provision job writes as it packs, ships and lands the person's files.

## Behaviour
1. `place_syncing_line`, `place_provisioning_line` (the step's SETUP_STEP_WORDS in parentheses where a step is named), `place_no_picks_line` and `place_no_home_line` (:15-27) are fixed sentences around a name.
2. `important_failures(applied)` (:30-31): the rows, none without `applied`, that failed under a step whose class is important (folders, github), in row order; a row with no step never counts.
3. `setup_word(setup, applied)` (:35-45): "" without a setup. Running: "setting up <the first running step's words>", or "setting up" with no step running. Failed: "stopped: <said>", else "stopped: no reason recorded". Done: no failed row reads "<provision_count_word(provision_counts(rows))> ready"; else "<failed> of <rows> failed: <name_list(failed labels)>".
4. `sync_line` (:51-52): "putting on" while running, else "waiting to put on", then "the recipe's <plural(changes, change)>: <name_list>". `wait_line` (:55): expired reads "<label>'s sign-in ran out; a retry asks for a fresh code"; else "<label> waits on you to sign in", then " at <url>" and " with code <code>" each where set.
5. `place_word(place, absent, pending)` (:61-75), the first that holds: setup failed (Setup failed, its `said` as the sentence where set); a pending add that failed; the setup's first waiting sign-in (Needs you, `wait_line`); important failures (Needs you, "<label>: <note or 'failed'>" joined "; "); a pending add; an absent computer (its `away` and `sentence`); setup running (Setting up, `setup_word`); a sync (Behind, `sync_line`); `place_daemon_behind` (that word alone); else Ready. `pending_word` (:78-81): failed is Setup failed with its `said`, else Pending with `pending_step_words(step)` (:84-90).
6. `place_state_of(place, absent, spent_today_usd, pending)` (:104-113): blocked reads PLACE_BLOCKED_WORD with the blocked text; else `place_word`, which wins where it is Setup failed, Needs you or Pending, or where any absent computer is given; then `place_at_limit_line` gives At limit, then `place_full_line` gives Full, both with tone warning; else the `place_word` answer.
7. `setup_lines(name, setup, applied, command)` (:119-136): first "<name>: " and the tally, "nothing installed" or "<n> installed: <names>" (installed rows not marked earlier), then ", <n> already there" (rows present, or installed earlier) where any. Then every failed row as "  x <label>: <note or 'no reason recorded'>", then every skipped row as "  - <label>: <note or 'set aside'>", each followed by "    <fix>" and "    <command(row)>" where set; then "  ? <wait_line>" per waiting sign-in; then "<name>: <said>" where the setup said something. All failed rows come before all skipped rows.
8. The fixed sentences around an add, a sign-in row and a recipe's rows (:139-169, :200-203, :211-248, :316-317) fill in their names. `edited_there_line` uses `name_list(kept)` and the singular "was", "it stays", "it" for exactly one. `floor_failed_line` (:278) is a `name_list` of labels; `no_agent_line` (:280-281) joins "<label> (<note>)", or the label alone, with "; ".
9. Sign-in ways (:175-198): `sign_in_way_label` is "Make a token", "Use an API key", "Sign in on <box>". `sign_in_way_does`: token reads "<mint, else '<name>'s own command'> runs on <here> and asks your browser once"; key with `field` true reads "paste your <keyEnv or 'key'>; wsp saves it to ~/.wsp/.env without showing it", else "put <keyEnv or 'the key'> in ~/.wsp/.env on <here> first, and wsp reads it from there without showing it"; machine reads "<name>'s own sign-in runs there". `sign_in_ways_fix`: "<label>: <does>." per way in the order given, joined by one space; "" for no ways.
10. `says_no_key(note, key_env)` (:205): false without a note; else the note starts with "~/.wsp/.env on " and ends with " holds no <key_env>", case as written.
11. `setup_row_fix(row, box)` (:252-270): a note equal to NEEDS_GITHUB_LINE is "Sign GitHub in on <box>, then retry." whatever the step; then None with no step or the folders step; else the step's own fix, those of `floor`, `agents`, `clis` and `plugins` naming the box.
12. `setup_width(mem_mb)` (:275) is `max(1, min(4, floor(mem_mb / 2048)))`.
13. `provision_log_line(at, line)` (:286): `iso_seconds(at)` then a space and the line. `provision_packed_line(paths, p)` (:291-294): "packed on this computer: <plural(paths, path)>, ", then, only where `files` and `stood` are both set, "<plural(stood, file)> stands|stand there already, <plural(files, file)> packed, " ("stands" for exactly 1), then `fmt_bytes(bytes)`.
14. `provision_shipped_line(p)` (:298-301): one part reads "shipped: <fmt_bytes(total)>"; more read "shipped part <part> of <parts>: <fmt_bytes_of_total(bytes, total)>"; " in <plural(pieces, piece)>" follows where `pieces` is set. `provision_landed_line`, `provision_list_read_line` pluralise their count; `provision_servers_line` (:312-313) writes `written` bare and pluralises `servers`.

## Edge cases
- A failed row under the skills step never reads Needs you (test :474); a pending add still choosing wins over an absent computer (test :496).
- `edited_there_line("spoo", [])` is " were edited on spoo, so they stay and wsp no longer manages them" (pinned).
- `provision_servers_line(7, 3)` is "servers merged: 7 of 3 servers ..." (pinned).
- One part ignores `bytes`: `{ part: 1, parts: 1, bytes: 10, total: 10, pieces: 4 }` is "shipped: 10 B in 4 pieces" (pinned).
- `setup_width`: 2047 is 1, 6144 is 3, 65536 is 4 (pinned); NaN is NaN, since `Math.max` and `Math.min` pass NaN through.
- `says_no_key` is false for the right note under another key, and for the note with any text before it (pinned).
- `provision_log_line(1792920605000, "npm i -g codex")` is "2026-10-25T09:30:05Z npm i -g codex" (pinned).

## Behaviour nobody wrote down
- Rule 8.11: every count is `String(n)` through `plural` (`js::number_string`); `plural` is singular only for exactly 1.
- Rule 8.15: `provision_log_line` writes `toISOString` with the fraction of a second dropped.
- Rules 8.12 and 8.13: `fmt_bytes` rounds KB and MB with `js_round` and GB with `to_fixed(1)` (spec/units.md).
- Rust's `f64::max` and `f64::min` drop a NaN where JavaScript's keep it: `setup_width` keeps the TS answer.
- `name_list` quotes a label holding a comma as JSON (spec/base.md).

## Tapes
The STATE and TOOLS columns of `wsp computers` and the `computers` tool, and the app's computer rows; what `wsp add` and `wsp update` print once a setup ends; a sign-in row's fix and its buttons; the refusals of a second add, `wsp add --resume`, a setup during a sync and a workspace on a computer still being set up; the provision job's own log on that computer.

## Decisions
1. `Pick<...>` parameters take the whole `PlaceView` or `PlaceProvisionRow`; `absent: AbsentComputer | null` is `Option<&AbsentComputer>`; optional numbers are `Option<f64>`, other numbers f64.
2. SETUP_WORDS is `mod setup_words` of constants; PENDING_STEP_WORDS is `pending_step_words(step)`.
3. PlaceState is a struct with `tone: Option<PlaceTone>` (serialized "warning") and `sentence: Option<String>`, None left out when serialized.
4. `setup_lines`'s default `command` (always undefined) is `Option<&dyn Fn(&PlaceProvisionRow) -> Option<String>>`, None for no command.
5. `SignInWay` is an enum of this crate for the TS `Extract<RecipeSignIn, ...>`; `SignInWayFacts` is a struct; the anonymous `{ bytes, files?, stood? }` and `{ part, parts, bytes, total, pieces? }` are `Packed` and `Shipped`.
6. `edited_there_line` takes `&[impl AsRef<str>]`; SETUP_LOG_TAIL_BYTES is usize, SETUP_MB_PER_STEP i64, SIGN_IN_WAIT_MS f64.

## After the port
- `edited_there_line` with an empty list opens with a bare space and "were edited": nothing in the function refuses an empty list.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `place_setup.rs: places::the_setup_on_a_computer_you_own::says_in_one_sentence_why_a_workspace_cannot_be_made_there_yet_naming_the_step_it`, `places::the_setup_on_a_computer_you_own::says_a_computer_that_reported_no_home_folder_or_holds_no_picks_got_nothing_and_w`; `view_wire_words.rs: pinned::setup_lines_name_the_computer_and_what_to_do` |
| 2, 5 | `place_setup.rs: places::the_setup_on_a_computer_you_own::reads_one_word_by_one_precedence_setup_failed_needs_you_offline_setting_up_behin`, `places::the_setup_on_a_computer_you_own::reads_a_pending_add_as_pending_with_how_far_it_got_or_setup_failed_with_why`, `places::the_setup_on_a_computer_you_own::reads_a_joined_computer_still_choosing_as_pending_after_needs_you_and_before_off` |
| 3 | `place_setup.rs: places::the_setup_on_a_computer_you_own::says_on_the_tools_cell_what_is_under_way_where_it_stopped_or_what_stands` |
| 4 | `wait_line` through `place_setup.rs: places::the_setup_on_a_computer_you_own::reads_one_word_by_one_precedence_setup_failed_needs_you_offline_setting_up_behin`; `sync_line` is pinned by no test |
| 6 | `place_setup.rs: place_state::the_word_a_place_s_row_says::*` |
| 7 | `place_setup.rs: places::the_setup_on_a_computer_you_own::prints_what_installed_by_name_what_was_already_there_every_row_that_did_not_land`, `places::the_setup_on_a_computer_you_own::prints_under_a_row_that_did_not_land_what_to_do_about_it_then_the_command_line_s` |
| 8 | `view_wire_words.rs: pinned::setup_lines_name_the_computer_and_what_to_do`; `pending_held_line`, `floor_failed_line` and `no_agent_line` are pinned by no test |
| 9 | `place_setup.rs: sign_in_ways::a_failed_sign_in_row_s_line_and_its_buttons::*`; `view_wire_words.rs: pinned::the_sign_in_ways_a_computer_offers` |
| 10 | `view_wire_words.rs: pinned::setup_lines_name_the_computer_and_what_to_do` |
| 11 | `setup_fix.rs: the_fix_half_of_a_row_that_did_not_land::*` (setup-fix.test.ts, 3 cases) |
| 12 | `place_setup.rs: places::the_setup_on_a_computer_you_own::runs_one_step_after_the_base_tools_per_2_gb_of_the_computer_s_memory_at_least_on`; `view_wire_words.rs: pinned::a_provision_counts_steps_files_and_bytes` |
| 13, 14 | `view_wire_words.rs: pinned::a_provision_counts_steps_files_and_bytes` |
