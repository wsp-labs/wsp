// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/provider-marks.test.ts.
use std::collections::HashSet;

use regex::Regex;
use wsp_catalog::marks::{provider_mark, provider_marks};

// PROVIDER_KEY_WORDS' keys (protocol/src/place-word.ts:25): Boat's row in every build, Solari's in none that is public.
#[cfg(not(feature = "public-build"))]
const PROVIDER_KEY_WORDS: [&str; 2] = ["box", "solari"];
#[cfg(feature = "public-build")]
const PROVIDER_KEY_WORDS: [&str; 1] = ["box"];

mod provider_marks {
    use super::*;

    #[test]
    fn every_provider_a_person_can_add_has_its_mark_one_module_each_found_by_the_provid() {
        let found: Vec<Option<String>> = PROVIDER_KEY_WORDS.iter().map(|id| provider_mark(id).map(|m| m.provider.clone())).collect();
        let expected: Vec<Option<String>> = PROVIDER_KEY_WORDS.iter().map(|id| Some(id.to_string())).collect();
        assert_eq!(found, expected);
        let providers: HashSet<&str> = provider_marks().iter().map(|m| m.provider.as_str()).collect();
        assert_eq!(providers.len(), provider_marks().len());
        assert_eq!(provider_mark("not-a-provider"), None);
    }

    #[test]
    fn solari_s_mark_is_an_outline_at_the_glyphs_2px_stroke_so_it_weighs_what_the_compu() {
        let outline = Regex::new(r#"^<svg viewBox="0 0 24 24"><path fill="none" stroke="currentColor" stroke-width="2" [^>]*/></svg>$"#).unwrap();
        let svg = &provider_mark("solari").unwrap().mark.svg;
        assert!(outline.is_match(svg), "{svg}");
    }

    #[test]
    fn a_provider_s_mark_says_which_of_its_own_pages_it_was_read_off_and_when() {
        let source = Regex::new(r"^https://[^ ]+ \(read \d{4}-\d{2}-\d{2}(, [^)]+)?\)$").unwrap();
        assert!(!provider_marks().is_empty());
        for m in provider_marks() {
            assert!(source.is_match(&m.mark.source), "{}", m.id);
        }
    }
}
