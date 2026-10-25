# machine_facts (machine-facts.ts, 118 lines)

What a machine that already existed says about itself, through `key value` lines one shell prints. Rust:
`src/machine_facts.rs`. Pure; nothing kept.

- `read_lists(stdout)` (:20): split on `\n` only. A line whose first space is at index 0 or that has none is skipped
  (`" b x"`, `"nospace"`, `""`). The key is up to the first space; the value is the rest, `trim()`med as JavaScript
  trims (so a trailing `\r` and runs of spaces go). Keys in first-seen order, values in line order.
- `read_values` (:15): each key's **last** value, keys in first-seen order. A key whose line printed nothing is
  present with `""`; every reader below takes `""` as no answer.
- The seven `*_READ` lines are the TS strings byte for byte (oracle `reads`); `OS_READ` is three printfs so one that
  fails leaves the other two.
- `arch_of`, `system_of` (:73, :79): the value, `None` for absent or `""`.
- `mem_mb_of` (:85): `kb = Number(memkb)` under JavaScript rules; `Some(round(kb / 1024))` where `kb` is finite and
  above 0, so `"0x10"` is `Some(0)`, `"1e3"` is `Some(1)`, `" 2048 "` is `Some(2)`, `"-5"`, `"0"`, `""`, `"Infinity"`,
  `"abc"` and an absent key are `None`. `Math.round` rounds half up.
- `os_name_of` (:93): `macOS <mac>` where mac is non-empty, else `pretty` where non-empty, else `kernel` where
  non-empty, else `None`.
- `uptime_ms_of(values, now)` (:106): where `uptime` is present and not `""` and `Number(uptime)` is finite, that
  figure times 1000, rounded, **negative included** (`"-3"` is `-3000`, `"  "` is 0). Otherwise the first
  `sec = (\d+)` in `boot`: `max(0, now - sec * 1000)`. Otherwise `None` (`"abc"` falls through to `boot`).
- `read_os_name(exec)` (:115): runs `OS_READ.join("\n")` once. An exec that fails or exits non-zero is `None`;
  otherwise `os_name_of(read_values(stdout))`.

| Rule | Tests |
|---|---|
| line split, skipped lines, trim, key order, last value | `machine_facts::reads_one_answer_per_line_and_a_line_that_printed_nothing_is_an_answer_the_machine_does_not_have`, `machine_facts_oracle::read_values_keeps_each_keys_last_value_in_first_seen_order_and_read_lists_keeps_them_all`, `machine_facts_oracle::a_line_with_a_leading_space_or_no_space_is_no_answer_and_a_carriage_return_is_trimmed` |
| read lines byte for byte | `machine_facts_oracle::every_read_line_is_the_ts_string_byte_for_byte` |
| os name order | `machine_facts::names_the_system_the_way_its_maker_does`, `machine_facts_oracle::os_name_prefers_the_mac_version_then_the_distribution_then_the_kernel` |
| uptime: /proc first, boot otherwise, negatives kept, clamp at 0 | `machine_facts::takes_the_length_proc_keeps_and_works_one_out_against_the_clock_from_a_boot_time`, `machine_facts_oracle::uptime_ms_of_takes_procs_figure_first_and_the_boot_moment_otherwise` |
| memory under Number() | `machine_facts::reads_the_memory_line_in_whole_mb_and_nothing_where_the_line_printed_no_figure`, `machine_facts_oracle::mem_mb_of_reads_the_figure_as_javascripts_number_does` |
| empty arch and system are none | `machine_facts_oracle::arch_and_system_read_an_empty_answer_as_none` |
| one exec, failure is none | `machine_facts::one_command_asks_for_the_name_and_a_machine_that_could_not_run_it_leaves_the_caller_without_one` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `machine_facts`), held by
`tests/machine_facts_pins.rs`.

- F-1 `uptime_ms_of`: `Math.round(Number(uptime) x 1000)`, which rounds a half toward +infinity (`-0.0025` is -2,
  `-0.0005` is 0, `0.0005` is 1; `wsp_words::js` has the helper, `f64::round` is wrong here); `Number` reads `1e3`,
  `0x10` and padded figures; a non-finite uptime falls to boot. Boot is `/sec = (\d+)/` unanchored with ASCII digits,
  so `{ usec = 5, sec = 7 }` reads 5 and `sec = ٣` reads nothing; `now - boot x 1000` in f64, at least 0, so a
  20-digit boot is 0. An empty or non-numeric uptime falls to boot (`uptime_rounds_and_reads_boot_as_the_ts`).
- F-2 `mem_mb_of` reads memkb with JS `Number`: `0b11`, `0o17`, `.5` and `+2048` are numbers (`+2048` is 2 MB, the
  rest round to 0 and are `Some(0)`); `1_0`, `1,000`, `5kB` and `1e400` are `None`; 512 kB is 1 MB, 511 is 0
  (`memory_reads_every_number_form_js_reads`). 1e30 kB, which the TS answers as 9.77e26 MB, is past a `u64`: an
  accepted difference.
- F-3 `read_values`: the last value of a key wins even when empty; the value is JS-trimmed, the key is not, so a BOM
  before the first key stays in it; a line with no space (a tab only) is skipped
  (`the_last_value_wins_and_keys_are_not_trimmed`). The TS builds a plain object, whose integer-like keys iterate
  first; nothing here iterates it, so first-seen order is kept.
