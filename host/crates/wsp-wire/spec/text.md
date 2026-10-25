# text

Source: `String.prototype.localeCompare` as node 22.23.4 (ICU 78.3) answers it, `JSON.stringify` on a whole value, and
rules 8.8 and 8.25 to 8.28 of `docs/rust-host-spec/08-text-numbers-time.md`. Rust: `src/text.rs`. Tests: `tests/text.rs`
(7, every expectation captured from node under `env -i`: `fixtures/text/collation.json`, `fixtures/text/json_stringify.json`).
The adapters and the views share both, so neither re-implements them.

## Behaviour

1. `collate(a, b)` is `a.localeCompare(b)` in the host's locale, read from the process environment once. The locale is
   the first of `LC_ALL`, `LC_MESSAGES` and `LANG` that is set, an empty one included (node's ICU reads the three itself and
   an empty `LC_ALL` is the root locale `und`, measured); `LC_COLLATE` is never read. `C`, `C.UTF-8`, `POSIX` and no
   variable at all give `en-US`; `sv_SE.UTF-8` is `sv-SE`, the codeset and `@modifier` dropped; a value that is no locale
   gives `en-US`. `locale_from_env` takes the lookup, so a test names the environment.
2. `collator(locale)` is ICU4X's (`icu_collator`, compiled data) at tertiary strength with nothing ignored, the options a
   call with none takes: case and accent are tie-breakers below the letters, punctuation and spaces sort before digits and
   digits before letters, `ß` after `ss`, `ä` after `a` in `en-US` and after `z` in `sv-SE`. A locale ICU4X has no rules for
   sorts as the root does, as ICU does.
3. `json_stringify(value)` is `JSON.stringify` of the JavaScript value `JSON.parse` makes of the value's own text: no white
   space; strings escaped as JavaScript escapes them (`"` and `\`, the controls as `\b \t \n \f \r` or `\u00xx` in lower case;
   U+2028, U+2029, DEL, `/` and the rest as themselves); numbers as `String(n)` through `JsNumber::js_string` (`0` for `-0`,
   `1e+21`, the double a value past 2^53 rounds to); each object's keys with the array-index keys first, ascending, and the
   rest in the order given, as a JavaScript object holds them. It writes no NaN or infinity: a JSON value has none.

## Edge cases

- `{"b":1,"2":2,"1":3,"07":4}` writes `{"1":3,"2":2,"b":1,"07":4}`; `4294967295` is no index, `4294967294` is.
- A lone surrogate cannot reach a Rust `String` (rule 8.5); the one place that matters is a harness line, which the
  adapters read as U+FFFD (the ruling on the claude spec pass).

## Decisions

1. `locale_from_env` treats a set but empty variable as the root locale, where rule 8.8 says an empty variable is unset.
   Node says otherwise (`LANG=` and `LC_ALL= LANG=sv_SE.UTF-8` both sort as the root), and the test pins it.
2. `serde_json` reads with `float_roundtrip` (`check.md`, decision 3), so a long number reads as JavaScript reads it
   before `json_stringify` writes it.

3. Han characters sort as ICU4X's root orders them: by code point within a block and block by block (core, extension A,
   B ...). Node's ICU 78.3 orders them by radical and stroke across blocks. Of 16,200 random Han pairs, the root
   order agreed on all but 32 of 6,000 within the core block (U+4E00 to U+9FFF), about half of the core-against-extension-A
   pairs and of the core-against-extension-B pairs, and 205 of 500 extension-A-against-compatibility pairs.
   `CollationType::Unihan` (`-u-co-unihan`) answers as the root does and `zh` is pinyin order, so neither closes it; the
   data to close it is the Unihan radical-stroke table, which `icu_collator`'s compiled data does not hold.
   `tests/text_known_gaps.rs` pins the Rust answer; `review_views.rs::han_characters_sort_by_radical_and_stroke_as_node_sorts_them`
   stays red until the data is there.
4. `locale_from_env` resolves ICU's legacy language codes as node does: `iw` is `he`, `in` is `id`, `ji` is `yi`, `mo` is
   `ro`, `tl` is `fil`, `cmn` is `zh`, `sh` is `sr-Latn`. Still unlike node, because ICU4X has no rules to sort by: `yi`
   (node puts Hebrew before Latin as it does for `he`; here `yi` sorts as the root), and the three-letter codes node
   canonicalizes (`heb`, `ces`, `hrv`, `yid`, `scc`, `scr`, `prs`), which no `LANG` names.

## Rules pinned → tests (`tests/text.rs`)

| Rule | Test |
|---|---|
| 1 locale | `locale_compare_sorts_as_node_does::the_locale_is_the_first_of_lc_all_lc_messages_and_lang_that_is_set` |
| 1, 2 order in each of 20 environments | `a_list_sorts_in_the_order_node_gave_it_in_each_locale` |
| 2 the order the host spec prints | `the_order_under_env_i_is_the_one_the_spec_prints` |
| 3 values node was asked | `json_stringify_writes_what_node_writes::on_every_value_node_was_asked` |
| 3 numbers, separators, index keys | `numbers_are_the_text_javascript_gives_them`, `a_line_separator_and_a_del_are_left_as_they_are_and_a_control_character_is_an_escape`, `index_keys_come_first_in_ascending_order_and_the_rest_in_the_order_given` |
