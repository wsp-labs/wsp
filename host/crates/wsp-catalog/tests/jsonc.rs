// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/jsonc.test.ts.
use serde_json::{json, Map, Value};
use std::time::Instant;
use wsp_catalog::jsonc::read_jsonc;

mod reading_json_with_comments {
    use super::*;

    #[test]
    fn drops_comments_and_trailing_commas_outside_strings_and_keeps_everything_inside_t() {
        let text = [
            "{",
            "  // a, b }",
            r#"  "a": "x // not a comment, /* nor this */","#,
            r#"  "b": ["\"q\"", 1, /* c */ ],"#,
            r#"  "c": { "d": 2, // e"#,
            "  },",
            "}",
        ]
        .join("\n");
        assert_eq!(
            read_jsonc(&text).unwrap(),
            json!({ "a": "x // not a comment, /* nor this */", "b": ["\"q\"", 1], "c": { "d": 2 } })
        );
        assert_eq!(read_jsonc(r#"{ "a": [1, 2] }"#).unwrap(), json!({ "a": [1, 2] }));
        assert!(read_jsonc(r#"{ "a": 1 // no close"#).is_err());
        assert!(read_jsonc(r#"{ "a": , }"#).is_err());
    }

    #[test]
    fn reads_a_config_of_tens_of_megabytes_in_one_pass() {
        let mut projects = Map::new();
        for i in 0..5000 {
            let history: Vec<Value> = (0..10).map(|j| json!({ "display": format!("{}{}", "x".repeat(150), j) })).collect();
            projects.insert(format!("/p/{i}"), json!({ "history": history }));
        }
        let text = serde_json::to_string_pretty(&json!({ "mcpServers": {}, "projects": projects })).unwrap();
        assert!(text.len() > 10_000_000);
        let started = Instant::now();
        let value = read_jsonc(&text).unwrap();
        assert_eq!(value["projects"].as_object().unwrap().len(), 5000);
        assert!(started.elapsed().as_millis() < 5000);
    }
}
