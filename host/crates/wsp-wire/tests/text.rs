// SPDX-License-Identifier: AGPL-3.0-only
//! `wsp_wire::text`: `localeCompare` as node's ICU sorts a list in the host's locale, and `JSON.stringify` on a whole
//! value, each against what node 22 answered for the same inputs (`fixtures/text/*.json`, captured under `env -i`).

use serde_json::Value;
use wsp_wire::text::{collator, json_stringify, locale_from_env};

fn fixture(name: &str) -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/text").join(name);
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

mod json_stringify_writes_what_node_writes {
    use super::*;

    #[test]
    fn on_every_value_node_was_asked() {
        let fixture = fixture("json_stringify.json");
        for case in fixture["cases"].as_array().unwrap() {
            let (text, want) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
            let value: Value = serde_json::from_str(text).unwrap();
            assert_eq!(json_stringify(&value), want, "{text}");
        }
    }

    #[test]
    fn numbers_are_the_text_javascript_gives_them() {
        let value: Value = serde_json::from_str("[-0,0,1e21,1e-7,9007199254740993,9007199254740994,1.5,100,1E2]").unwrap();
        assert_eq!(json_stringify(&value), "[0,0,1e+21,1e-7,9007199254740992,9007199254740994,1.5,100,100]");
    }

    #[test]
    fn a_line_separator_and_a_del_are_left_as_they_are_and_a_control_character_is_an_escape() {
        let value: Value = serde_json::from_str(r#"" \u007f\u001f\u0000""#).unwrap();
        assert_eq!(json_stringify(&value), "\"\u{2028}\u{7f}\\u001f\\u0000\"");
    }

    #[test]
    fn index_keys_come_first_in_ascending_order_and_the_rest_in_the_order_given() {
        let value: Value = serde_json::from_str(r#"{"b":1,"10":2,"2":3,"a":4,"01":5}"#).unwrap();
        assert_eq!(json_stringify(&value), r#"{"2":3,"10":2,"b":1,"a":4,"01":5}"#);
    }
}

mod locale_compare_sorts_as_node_does {
    use super::*;

    #[test]
    fn the_locale_is_the_first_of_lc_all_lc_messages_and_lang_that_is_set() {
        let said = |pairs: &'static [(&'static str, &'static str)]| {
            locale_from_env(move |name| pairs.iter().find(|(k, _)| *k == name).map(|(_, v)| v.to_string()))
        };
        assert_eq!(said(&[]), "en-US");
        assert_eq!(said(&[("LANG", "sv_SE.UTF-8")]), "sv-SE");
        assert_eq!(said(&[("LC_ALL", "sv_SE.UTF-8"), ("LANG", "en_US.UTF-8")]), "sv-SE");
        assert_eq!(said(&[("LC_COLLATE", "sv_SE.UTF-8")]), "en-US");
        assert_eq!(said(&[("LC_MESSAGES", "sv_SE.UTF-8"), ("LANG", "de_DE.UTF-8")]), "sv-SE");
        assert_eq!(said(&[("LANG", "C.UTF-8")]), "en-US");
        assert_eq!(said(&[("LANG", "POSIX")]), "en-US");
        assert_eq!(said(&[("LANG", "")]), "und");
        assert_eq!(said(&[("LC_ALL", ""), ("LANG", "sv_SE.UTF-8")]), "und");
        assert_eq!(said(&[("LANG", "sv_SE@euro")]), "sv-SE");
    }

    #[test]
    fn a_list_sorts_in_the_order_node_gave_it_in_each_locale() {
        let fixture = fixture("collation.json");
        let words: Vec<String> = fixture["words"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_string()).collect();
        let mut wrong = Vec::new();
        for case in fixture["cases"].as_array().unwrap() {
            let env = case["env"].as_object().unwrap();
            let locale = locale_from_env(|name| env.get(name).map(|v| v.as_str().unwrap().to_string()));
            let node_locale = case["nodeLocale"].as_str().unwrap();
            if !node_locale.contains("-x-") && node_locale != "und" {
                assert_eq!(locale, node_locale, "{env:?}");
            }
            let collator = collator(&locale);
            let mut sorted = words.clone();
            sorted.sort_by(|a, b| collator.compare(a, b));
            let want: Vec<&str> = case["sorted"].as_array().unwrap().iter().map(|w| w.as_str().unwrap()).collect();
            if sorted != want {
                wrong.push(format!("{env:?} ({locale}): {sorted:?} against {want:?}"));
            }
        }
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    }

    #[test]
    fn the_order_under_env_i_is_the_one_the_spec_prints() {
        let collator = collator("en-US");
        let mut words = ["b", "A", "a", "B", "_x", "1", "a-b", "ab", "a_b", "Z", "é", "e", "f"];
        words.sort_by(|a, b| collator.compare(a, b));
        assert_eq!(words, ["_x", "1", "a", "A", "a_b", "a-b", "ab", "b", "B", "e", "é", "f", "Z"]);
        let mut accents = ["z", "å", "ä", "a"];
        accents.sort_by(|a, b| collator.compare(a, b));
        assert_eq!(accents, ["a", "å", "ä", "z"]);
        let sweden = super::collator("sv-SE");
        accents.sort_by(|a, b| sweden.compare(a, b));
        assert_eq!(accents, ["a", "z", "å", "ä"]);
    }
}
