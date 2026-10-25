// SPDX-License-Identifier: AGPL-3.0-only
//! Inputs where `text::collate` answers unlike `String.prototype.localeCompare` of node 22.23.4 (ICU 78.3), other than
//! Han (which `review_views.rs::han_characters_sort_by_radical_and_stroke_as_node_sorts_them` holds). The TS calls
//! `a.localeCompare(b)` with no locale (`project-seed.ts:123`, `project-seed.ts:199`, `views/golden-image.ts:358`,
//! `slate/expr.ts:896`), so the oracle is the default locale node reads from the environment, not `new Intl.Collator(x)`.
//! Every expectation is the sign node printed under `env -i LANG=<value> node` for the same two strings.

use wsp_wire::text::{collator, locale_from_env};

/// What `a.localeCompare(b)` gave under `env -i LANG=<lang>`, for each case; None where this crate answers the same.
fn sorts(lang: &str, cases: &[(&str, &str, i32)]) -> Option<String> {
    let locale = locale_from_env(|name| (name == "LANG").then(|| lang.to_string()));
    let c = collator(&locale);
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|(a, b, node)| {
            let here = c.compare(a, b) as i32;
            (here != *node).then(|| format!("{a:?} vs {b:?}: node {node}, here {here}"))
        })
        .collect();
    (!wrong.is_empty()).then(|| format!("LANG={lang} (here {locale}): {}", wrong.join("; ")))
}

fn none_differ(found: Vec<Option<String>>) {
    let differ: Vec<String> = found.into_iter().flatten().collect();
    assert!(differ.is_empty(), "{} environment(s) sort unlike node:\n{}", differ.len(), differ.join("\n"));
}

/// text.rs:39-40 `current_language` maps `tl` to `fil` and `cmn` to `zh`; spec/text.md decision 4 says node does. Node does
/// only for `new Intl.Collator("tl")`; the default locale it reads from LANG keeps `tl-PH` and `cmn-CN`, which have no
/// rules, so both sort as the root (`LANG=tl_PH.UTF-8` gives `n ñ na ng nz o`, `LANG=cmn_CN.UTF-8` gives Han by radical
/// and stroke). Here `tl` takes the Filipino rules (ñ and ng after n) and `cmn` takes pinyin. `iw`, `in`, `mo` and `ji`
/// are aliased in node's default locale as well; `sh` has its rules through an alias in the data.
#[test]
fn tl_and_cmn_in_lang_keep_the_root_rules_as_node_keeps_them() {
    none_differ(vec![
        sorts("tl_PH.UTF-8", &[("ñ", "na", -1), ("na", "ñ", 1), ("ng", "nz", -1), ("nz", "ng", 1)]),
        sorts("cmn_CN.UTF-8", &[("阿", "一", 1), ("爱", "中", 1), ("一", "阿", -1)]),
    ]);
}

/// text.rs:26 maps `C` and `POSIX` to `en-US` but not `en_US_POSIX`, which parses to the variant `en-US-posix` whose
/// rules put every capital before every small letter. Node (V8 turns the ICU default `en_US_POSIX` into `en-US`):
/// `LANG=en_US_POSIX` gives `"a".localeCompare("B")` is -1, as `en_US.UTF-8` does.
#[test]
fn en_us_posix_in_lang_collates_as_en_us() {
    none_differ(vec![sorts("en_US_POSIX", &[("a", "B", -1), ("B", "a", 1), ("Z", "a", 1)])]);
}

/// The published `icu_collator_data` 2.4.0 holds no rules for `haw`, `kl`, `se`, `smn` and `bo` (the exporter's full
/// data has all five), so they sort as the root here and by their own rules in node. `LANG=haw_US.UTF-8`: vowels come
/// first; `kl_GL`: æ, ø and å come after z; `se_NO`: á and č are letters after a and c. spec/text.md decision 4 names
/// only `yi`.
#[test]
fn a_locale_node_has_rules_for_and_the_compiled_data_has_none_sorts_by_them() {
    none_differ(vec![
        sorts("haw_US.UTF-8", &[("e", "b", -1), ("b", "e", 1)]),
        sorts("kl_GL.UTF-8", &[("ø", "z", 1), ("z", "ø", -1), ("å", "z", 1), ("æ", "b", 1)]),
        sorts("se_NO.UTF-8", &[("á", "ab", 1), ("ab", "á", -1), ("č", "cz", 1), ("cz", "č", -1)]),
    ]);
}

/// ICU4X finds a locale's rules through its likely script (`sr-ME` is `sr-Latn-ME`, `pa-Arab-PK` has no rules); node's ICU
/// drops the region and keeps the language, so `sr_ME` sorts as `sr` (Cyrillic first), `az_Cyrl_AZ`, `uz_Cyrl_UZ` and
/// `pa_Arab_PK` as `az`, `uz` and `pa`.
#[test]
fn a_script_or_region_in_lang_reaches_the_rules_of_its_language_as_node_reaches_them() {
    none_differ(vec![
        sorts("sr_ME.UTF-8", &[("a", "б", 1), ("б", "a", -1), ("cz", "č", 1), ("č", "cz", -1)]),
        sorts("az_Cyrl_AZ.UTF-8", &[("x", "k", -1), ("k", "x", 1)]),
        sorts("uz_Cyrl_UZ.UTF-8", &[("ch", "cz", 1), ("cz", "ch", -1)]),
        sorts("pa_Arab_PK.UTF-8", &[("ਅ", "a", -1), ("a", "ਅ", 1)]),
    ]);
}

/// Lithuanian: a letter with a dot above followed by another accent. Node under `LANG=lt_LT.UTF-8`:
/// "\u{1e87}".localeCompare("\u{1e87}\u{303}") is -1 and "\u{1e81}".localeCompare("\u{1e87}\u{300}") is -1; ICU4X's `lt` rules give 1 and 0. Only
/// `ẇ` and `ż` style letters differ (19,044 of 4.1 million generated pairs); `i`, `j`, `į` and `ė` with accents agree.
#[test]
fn lithuanian_dot_above_before_another_accent_sorts_as_node_sorts_it() {
    none_differ(vec![sorts(
        "lt_LT.UTF-8",
        &[("\u{1e87}", "\u{1e87}\u{303}", -1), ("\u{1e87}\u{303}", "\u{1e87}", 1), ("\u{1e81}", "\u{1e87}\u{300}", -1)],
    )]);
}
