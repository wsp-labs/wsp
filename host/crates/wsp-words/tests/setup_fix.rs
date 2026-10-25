// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/protocol/test/setup-fix.test.ts whole (3 cases): the fix half of a setup row that did not land.

use regex::Regex;
use serde_json::json;
use wsp_wire::views::place::{PlaceProvisionRow, SETUP_STEP_CLASS};
use wsp_words::views::place_setup_words::{setup_row_fix, NEEDS_GITHUB_LINE};

/// The TS `{ step?, note }`; the row's other required fields are filler no rule reads.
fn row(step: Option<&str>, note: &str) -> PlaceProvisionRow {
    let mut v = json!({ "id": "r1", "label": "row", "outcome": "failed", "note": note });
    if let Some(s) = step {
        v["step"] = json!(s);
    }
    serde_json::from_value(v).unwrap()
}

mod the_fix_half_of_a_row_that_did_not_land {
    use super::*;

    #[test]
    fn names_what_to_do_for_a_row_of_every_step_and_the_reason_s_own_fix_where_the_reaso() {
        let sentence = Regex::new(r"^[A-Z].*\.$").unwrap();
        let banned = Regex::new(r"(?i)\bdaemons?\b|\bwsp [a-z]+ |this Mac|\u{2014}|\u{b7}").unwrap();
        for (step, _) in SETUP_STEP_CLASS.iter().filter(|(s, _)| *s != "folders") {
            let fix = setup_row_fix(&row(Some(step), "exit 1"), "studio").unwrap_or_else(|| panic!("{step}: no fix"));
            assert!(sentence.is_match(&fix), "{step}: {fix}");
            assert!(!banned.is_match(&fix), "{step}: {fix}");
        }
        assert_eq!(setup_row_fix(&row(Some("folders"), NEEDS_GITHUB_LINE), "studio").as_deref(), Some("Sign GitHub in on studio, then retry."));
        let agents = setup_row_fix(&row(Some("agents"), "exit 1"), "studio").unwrap();
        assert!(agents.contains("studio"), "{agents}");
    }

    #[test]
    fn says_nothing_beside_a_folder_s_own_line_but_where_github_is_what_it_needs() {
        assert_eq!(setup_row_fix(&row(Some("folders"), "that source is already a project on studio, mine"), "studio"), None);
    }

    #[test]
    fn says_nothing_for_a_row_with_no_step_to_read_it_by() {
        assert_eq!(setup_row_fix(&row(None, "exit 1"), "studio"), None);
    }
}
