// SPDX-License-Identifier: AGPL-3.0-only
//! `AppProcList` as zod reads `z.array(AppProc).max(200)` (zod 3.25.76): the length issue first, then each row's.

use serde_json::{json, Value};
use wsp_wire::types::check::issues_of;
use wsp_wire::views::app_procs::AppProcList;

fn words(rows: Value) -> Vec<(String, Value, String)> {
    issues_of::<AppProcList>(&rows).into_iter().map(|e| (e.code, Value::Array(e.path), e.message)).collect()
}

#[test]
fn a_row_given_as_a_list_is_refused_after_the_length_issue() {
    let mut rows = vec![json!([1, "main"])];
    rows.extend((0..200).map(|_| json!({ "pid": 1, "kind": "main" })));
    assert_eq!(
        words(Value::Array(rows)),
        vec![
            ("too_big".into(), json!([]), "Array must contain at most 200 element(s)".into()),
            ("invalid_type".into(), json!([0]), "Expected object, received array".into()),
        ]
    );
}

#[test]
fn a_list_of_good_rows_has_no_issue() {
    assert!(words(json!([{ "pid": 1, "kind": "main", "origins": ["a"], "contents": [0] }])).is_empty());
}
