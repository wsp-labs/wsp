# text

Source: `String.prototype.localeCompare` as node 22.23.4 (ICU 78.3) answers it, `JSON.stringify` on a whole value, and
rules 8.8 and 8.25 to 8.28 of `docs/rust-host-spec/08-text-numbers-time.md`. Rust: `src/text.rs`. Tests: `tests/text.rs`
(7, every expectation captured from node under `env -i`: `fixtures/text/collation.json`, `fixtures/text/json_stringify.json`;
`tests/rereview_collation.rs` holds the locale cases of the default locale node reads from `LANG`).
The adapters and the views share both, so neither re-implements them.

## Behaviour

1. `collate(a, b)` is `a.localeCompare(b)` in the host's locale, read from the process environment once. The locale is
   the first of `LC_ALL`, `LC_MESSAGES` and `LANG` that is set, an empty one included (node's ICU reads the three itself and
   an empty `LC_ALL` is the root locale `und`, measured); `LC_COLLATE` is never read. `C`, `C.UTF-8`, `POSIX` and no
   variable at all give `en-US`; `en_US_POSIX` is `en-US`; `sv_SE.UTF-8` is `sv-SE`, the codeset and `@modifier` dropped; a
   value that is no locale gives `en-US`. `locale_from_env` takes the lookup, so a test names the environment.
2. `collator(locale)` is ICU4X's (`icu_collator`) over data baked into the crate (`src/text/collation_data/`, decision 3) at
   tertiary strength with nothing ignored, the options a call with none takes: case and accent are tie-breakers below the
   letters, punctuation and spaces sort before digits and digits before letters, `ß` after `ss`, `ä` after `a` in `en-US`
   and after `z` in `sv-SE`. The rules are those of the first of language-script-region, language-script, language-region
   and language that has some, by the locale's own subtags (`sr-ME` sorts as `sr`, `pa-Arab-PK` as `pa`, as ICU opens it);
   a locale with none sorts as the root does, as ICU does. Three languages take their script from the region as node does:
   `zh-US` and `zh-TW` are Traditional (stroke order), `kk-CN` is Arabic; `nb` and `nn` are `no`.
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

3. The compiled data of `icu_collator` (`icu_collator_data`, Implicit Han order) cannot match node: it orders Han characters by
   code point within a block and block by block, where node's ICU 78.3 orders them by radical and stroke across blocks (1.1
   million adjacent pairs of node's own sort of every scalar value differ in 529 places in `en-US`, 17,127 in `zh` and 4,470
   in `ko`), and it holds no rules for `haw`, `kl`, `se`, `smn` and `bo`. The baked data does match: it is the full collation
   data of all locales exported from ICU 78.3 (CLDR 48) with `CollationRootHan::Unihan`, the radical-stroke order node's ICU
   uses, by `icu_provider_source` 2.4.0 into `icu_provider_export`'s baked exporter (`SourceDataProvider::new_custom()` with
   `with_icuexport` on the ICU 78.3 export, `with_cldr` on CLDR 48.0.0 JSON and `with_collation_root_han(Unihan)`; markers
   `icu::collator::provider::MARKERS` and the two `NormalizerNfd` ones; every locale; no deduplication; not pretty). ICU 79
   differs from node's 78.3 in 208 places, so 78.3 it is. It is 4.5 MB of source in `.rs.data` files that `mod.rs` includes,
   which neither rustfmt nor the file size law (`.rs` files only) reads; icu_collator, icu_normalizer and the rest of the data
   crates are pinned to 2.4 because the data holds structs of that release. In 1,112,063 adjacent scalar value pairs (and
   840,000 random comparisons mixing Han, accented Latin, Cyrillic, Greek, Hangul, kana, digits and punctuation in `en-US`,
   `zh`, `ko`, `sv`, `lt`, `tl` and `sr-Latn`) none differs from node.
4. `locale_from_env` resolves ICU's legacy language codes as node does: `iw` is `he`, `in` is `id`, `ji` is `yi`, `mo` is
   `ro`, `sh` is `sr-Latn`. `tl` and `cmn` are not aliased, node keeps `tl-PH` and `cmn-CN` and both sort as the root
   (`LANG=tl_PH.UTF-8` gives `"ñ".localeCompare("na")` is -1; the alias applies to `new Intl.Collator("tl")` only). The
   three-letter codes node canonicalizes (`heb`, `ces`, `hrv`, `yid`, `scc`, `scr`, `prs`) are not resolved, and no `LANG`
   names them.
5. ICU does not normalize what it collates, ICU4X decomposes it first. Lithuanian ignores a dot above (U+0307) before a
   grave, an acute or a tilde; ICU4X ignores it in a precomposed letter too (`ẇ` followed by U+0303), where ICU leaves it,
   so `lt` collation puts U+034F, which collates as nothing, after a precomposed letter that ends in a dot above when one of
   those marks follows (`apart_from_marks`). Without it 19,044 of 4.1 million generated pairs differ; with it one of 200,000
   pairs of Lithuanian letters and marks does (a precomposed `ṩ` followed by U+0328 and then U+0303).

## Rules pinned → tests (`tests/text.rs`)

| Rule | Test |
|---|---|
| 1 locale | `locale_compare_sorts_as_node_does::the_locale_is_the_first_of_lc_all_lc_messages_and_lang_that_is_set` |
| 1, 2 order in each of 20 environments | `a_list_sorts_in_the_order_node_gave_it_in_each_locale` |
| 2 the order the host spec prints | `the_order_under_env_i_is_the_one_the_spec_prints` |
| 3 values node was asked | `json_stringify_writes_what_node_writes::on_every_value_node_was_asked` |
| 3 numbers, separators, index keys | `numbers_are_the_text_javascript_gives_them`, `a_line_separator_and_a_del_are_left_as_they_are_and_a_control_character_is_an_escape`, `index_keys_come_first_in_ascending_order_and_the_rest_in_the_order_given` |
