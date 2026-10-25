# units
Source: `packages/protocol/src/words/units.ts` (397 lines) at the cut, 383708c78 (440 lines at a3b870a6e). Tests: `packages/protocol/test/format.test.ts` (23 cases on these names), `format-cases.test.ts` (bytes, durations and cost rows of `daemon/fixtures/contract/formats.json`), plus `pinned` in `tests/units.rs`.

## What it does
Every number a person reads with its unit: bytes (binary and decimal), memory and cpus, a machine size and its word, rates, durations, tokens, uptime, costs in dollars and rupees. Boat's start, balance, cancel and refusal lines, and the disk-expiry rule.

## Behaviour
1. `fmt_bytes(n)` / `fmt_disk_bytes(n)` (k = 1024 / 1000): `<n> B` under k (n as written, fractions kept); `js_round(n/k) KB` under k²; `js_round(n/k²) MB` under k³; else gb = n/k³ and `<js_round(gb) if to_fixed(gb,1) is integral else to_fixed(gb,1)> GB` (units.ts:24-39). So 1023.95 MB-ish rounds to "1024 MB", never "1 GB".
2. `compact_bytes(n)`: v = max(0, n), divide by 1024 while v >= 1000 up to T; unit "" writes `js_round(v)`; else v < 10 writes `to_fixed(v,1)`, else `js_round(v)` (units.ts:13-22).
3. `fmt_bytes_over(n)`: `fmt_bytes(n)` under 1 MiB and from 1 GiB; else `to_fixed(ceil(n/MiB*10)/10, 1) MB` (units.ts:43-46).
4. `fmt_mem_gb(mb)`: `Number(to_fixed(mb/1024, 1))` then ` GB`, so 1024 is "1 GB", 1638 "1.6 GB" (units.ts:80,88). `size_word`: `<cpu>x<that number>` (units.ts:196).
5. `fmt_cpus(n, word)`: `1 core` only for cores and exactly 1; vCPU at any count (units.ts:96). `fmt_size`: `<cpus>,\u{a0}<mem>` with every space then made U+00A0 (units.ts:103). `place_facts_line` joins the parts with ", " then turns every space into U+00A0 (units.ts:117).
6. `size_from_word(w)`: `^(\d+)x(\d+(?:\.\d+)?)$` on `js_trim(w)`, lower-case x only; memMb = `js_round(gb*1024)`; None unless cpu > 0 and memMb > 0 (units.ts:201-207).
7. `fmt_rate_dollars(r)`: mils = `to_fixed(r,3)`; `$` + (`to_fixed(r,2)` if mils ends in "0" else mils) (units.ts:234-237). `fmt_rate` appends `/hr`; `fmt_price` is "free" for 0 or absent (`charges_nothing`, -0 counts as 0).
8. `size_tone`: >= 1 GiB danger, >= 200 MiB warning, >= 50 MiB yellow, else muted. `disk_tone`: share = used/total (1 when total <= 0); >= 0.9 danger, >= 0.7 warning, else muted (units.ts:59-77).
9. `fmt_duration(ms, short)`: NaN, infinite or negative "0ms"; under 1 s `max(1, js_round(ms))ms`; under 10 s tenths = js_round(ms/100)/10, "10s" when that reaches 10, else `to_fixed(tenths,1)s`; under 60 s `js_round(ms/1000)s`; else total = js_round(ms/1000) seconds as `<minutes>m` or `<minutes>m <s>s`, hours folded into minutes. Clock: `<m>m <ss>s` or `<h>h <mm>m <ss>s`, 0 for non-finite or <= 0 (units.ts:325-345).
10. `fmt_tokens(n)`: count = js_round(n) for finite n > 0, else 0; under 1000 as is; else k under 999,500, M under 999,500,000, else B; value written as `Number(to_fixed(v, v<10 ? 2 : v<100 ? 1 : 0))` (units.ts:349-355).
11. `fmt_uptime`: floor minutes; `Nm`, `Nh Mm`, `Nd Hh`. `fmt_elapsed`: floor seconds; `Ns` under 60, else `fmt_duration(seconds*1000)` (units.ts:359-372).
12. `fmt_cost(usd)`: `$` + `locale_number(usd, EnUs, 2, 2)`, so a negative reads "$-12.50". `fmt_calls(n)`: `locale_number(n, EnUs, 0, 3)` + ` call`/` calls`. `fmt_inr(x)`: paise = `js_round(x*100) % 100 != 0`; sign, "₹", `locale_number(|x|, EnIn, p, p)` with p = 2 or 0 (units.ts:377-385).
13. `fmt_bytes_of(used, total, fmt)`: t = fmt(total); unit = t from its last space; used's text loses that unit when it ends with it (units.ts:392-397).
14. Boat lines: `starts_used_line` per window (today / this hour / this minute) with `(used of limit[ on plan])` and the machine's name or "the machine" for None or ""; `boat_request_limit_wait_line` waits `ceil(ms/1000)s`; `boat_refused_log_line` keeps the first line of `said` cut to 300 units, `none` for an absent code or id; `boat_cancelled_line` adds ` (said)` only for a non-empty said (units.ts:271-316).
15. `machine_cap_refusal`: holders = workspaces then `<builder> (builder)`; none: the no-slot sentence, `. The provider said: <said>` only for a non-empty said; one: `a machine slot is in use: x. Pause it ...`; two: `both machine slots are`; more: `machine slots are` and `Pause one` (units.ts:257-264).
16. `disk_expired(t, now)`: kept-until set and <= now, status not "running", asking None (an empty string is an asking) (units.ts:301-302).

## Edge cases
- Pinned per function in `tests/units.rs::pinned`: every byte tier edge, `compact_bytes(-5)` "0", `fmt_bytes_over(1 MiB + 1)` "1.1 MB", `fmt_rate_dollars(0.09000000000000001)` "$0.09", `0.0185` "$0.018", `fmt_tokens(999.5)` "1k", `1.5e15` "1500000B", `fmt_inr(0.995)` "₹1", `size_from_word("2x0.0001")` None, `"02x04"` 2x4, `disk_tone(5, 0)` danger, `fmt_cost(-0.004)` "$-0.00".

## Behaviour nobody wrote down
- Rules 8.11 to 8.14: every number via `number_string`, `js_round`, `to_fixed`, `locale_number`; the en-US and en-IN forms are the fixed formatters rule 8.14 lists (`units.ts:52,378,384`).

## Tapes
`places.list`, `workspaces.*` sizes and rates, `usage.*` costs: none of this unit alone.

## Decisions
1. `KIB` is `f64` (1024.0) since every use divides an `f64`.
2. `size_offer` is generic over `AsRef<WorkspaceSize>` and answers a reference into the slice.
3. `disk_tone` answers `SizeTone` (never Yellow) rather than a second enum.

## After the port
- `fmt_cost` of a negative prints "$-0.00" and "$-12.50", the sign after the dollar.
- `fmt_bytes` of a fraction under 1 KB prints it: "0.5 B".

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2, 3, 4, 13 | `fmt_bytes_and_fmt_mem_gb::*`, `bytes_s_reads_j`, `byte_formats_hold_every_tier_edge` |
| 5, 6 | `machine_size_words::*`, `a_computer_of_the_person_s_own_in_one_line::*`, `size_words_parse_and_print_as_the_ts_does` |
| 7 | `fmt_rate_reads_a_rate_a_provider_computed_...`, `rates_and_prices_read_the_rounded_thousandth` |
| 8 | `tones_switch_exactly_at_their_thresholds` |
| 9, 10, 11 | `fmt_duration_fmt_elapsed_and_fmt_cost::*`, `a_duration_of_s_ms_reads_j`, `durations_tokens_and_clocks_hold_their_edges` |
| 12 | `a_cost_of_s_reads_j`, `money_reads_in_the_locales_the_ts_names` |
| 14, 15 | `machine_cap_refusal::*`, `boat_lines_say_only_what_boat_answered` |
| 16 | `disk_expiry_holds_at_the_keep_instant` |
| box room | `the_room_left_on_a_box_as_the_doctor_reads_it_back::*` |
