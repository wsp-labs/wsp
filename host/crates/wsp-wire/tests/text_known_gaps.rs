// SPDX-License-Identifier: AGPL-3.0-only
//! Where `text::collate` answers unlike node 22.23.4 (ICU 78.3) and the compiled ICU4X 2.4 data cannot be made to
//! answer alike. See spec/text.md, Decision 3.

use std::cmp::Ordering;
use wsp_wire::text::{collator, locale_from_env};

fn lang(value: &str) -> String {
    locale_from_env(|name| (name == "LANG").then(|| value.to_string()))
}

#[test]
fn a_legacy_language_code_resolves_as_node_resolves_it() {
    let resolved: Vec<String> = ["iw", "iw_IL", "in_ID", "ji_US", "mo_MD", "sh_RS", "tl_PH", "cmn_CN"].map(lang).into();
    assert_eq!(resolved, ["he", "he-IL", "id-ID", "yi-US", "ro-MD", "sr-Latn-RS", "fil-PH", "zh-CN"]);
}

#[test]
fn han_characters_sort_by_code_point_within_a_block_and_by_block_where_node_sorts_by_radical_and_stroke() {
    let root = collator("en-US");
    assert_eq!(root.compare("中", "\u{4e3d}"), Ordering::Less);
    assert_eq!(root.compare("\u{3400}", "\u{4e00}"), Ordering::Less);
    assert_eq!(root.compare("\u{4e00}", "\u{4e8c}"), Ordering::Less);
}
