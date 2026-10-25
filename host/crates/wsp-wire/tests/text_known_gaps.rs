// SPDX-License-Identifier: AGPL-3.0-only
//! The locale a legacy language code in `LANG` resolves to, which node 22.23.4 (ICU 78.3) settles before it collates. No
//! gap is left in `text::collate` against node: the baked Unihan data closed the Han one (spec/text.md, decision 3).

use wsp_wire::text::locale_from_env;

fn lang(value: &str) -> String {
    locale_from_env(|name| (name == "LANG").then(|| value.to_string()))
}

/// Under `env -i LANG=<value> node`, `new Intl.Collator().resolvedOptions().locale` is `he-IL` for `iw_IL`, `id-ID` for
/// `in_ID`, `yi-US`, `ro-MD`, `sr-Latn-RS` for `sh_RS`, and for `tl_PH` and `cmn_CN` it stays `tl-PH` and `cmn-CN`: neither
/// language has collation rules, so both sort as the root (`"ñ".localeCompare("na")` is -1 under `LANG=tl_PH.UTF-8`).
#[test]
fn a_legacy_language_code_resolves_as_node_resolves_it() {
    let resolved: Vec<String> = ["iw", "iw_IL", "in_ID", "ji_US", "mo_MD", "sh_RS", "tl_PH", "cmn_CN"].map(lang).into();
    assert_eq!(resolved, ["he", "he-IL", "id-ID", "yi-US", "ro-MD", "sr-Latn-RS", "tl-PH", "cmn-CN"]);
}
