// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/ssh-config.test.ts.
// The ssh config as the machine can parse it. OpenSSH there stops at the first
// keyword it does not know, and the person's computer runs a build that knows
// more of them, so the copy opens with IgnoreUnknown and keeps everything else.
use wsp_collect::ssh_config::with_ignore_unknown as ignore_unknown;

const PRELUDE: &str = "# wsp: ssh on the machine does not know every option yours does, and one unknown option would stop it reading the file.\nIgnoreUnknown *\n";

mod with_ignore_unknown {
    use super::*;

    #[test]
    fn puts_ignoreunknown_above_everything_the_file_holds_and_changes_nothing_else() {
        let text = [
            "# written on the Mac",
            "Include ~/.ssh/conf.d/*.conf",
            "",
            "Host *",
            "  AddKeysToAgent yes",
            "  UseKeychain yes",
            "  IdentityFile ~/.ssh/id_ed25519",
            "",
            "Match host bastion",
            "  User root",
            "",
        ]
        .join("\n");
        assert_eq!(ignore_unknown(&text), format!("{PRELUDE}{text}"));
    }

    #[test]
    fn says_who_added_the_line_since_the_person_reads_this_file_on_the_machine() {
        let out = ignore_unknown("Host x\n");
        assert!(out.split('\n').next().unwrap().starts_with("# wsp: "));
    }

    #[test]
    fn leaves_a_config_that_has_already_been_through_here_alone_so_a_re_import_does_not() {
        let once = ignore_unknown("Host x\n  User me\n");
        assert_eq!(ignore_unknown(&once), once);
    }

    #[test]
    fn prefaces_a_config_that_carries_an_ignoreunknown_of_its_own_rather_than_trusting() {
        let text = "Host *\n  IgnoreUnknown UseKeychain\n  UseKeychain yes\n";
        assert_eq!(ignore_unknown(text), format!("{PRELUDE}{text}"));
    }

    #[test]
    fn prefaces_an_empty_config() {
        assert_eq!(ignore_unknown(""), PRELUDE);
    }
}

/// Every case of tests/fixtures/oracle/w2/small.json (oracle/w2-small.oracle.ts): spec small's wave-2 rules.
mod w2_oracle {
    use serde_json::{json, Value};
    use wsp_collect::cache_names::{CACHE_DIRS, FINDER_METADATA};
    use wsp_collect::host::{expand, tilde};
    use wsp_collect::ssh_config::{with_ignore_unknown, PRELUDE, SSH_CONFIG_PATH, SSH_IGNORE_UNKNOWN_DETAIL};

    fn fixture() -> Value {
        let path = format!("{}/tests/fixtures/oracle/w2/small.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn s(v: &Value) -> &str {
        v.as_str().unwrap()
    }

    #[test]
    fn expand_tilde_and_the_prelude_answer_what_the_ts_answered() {
        let f = fixture();
        for case in f["expand"].as_array().unwrap() {
            assert_eq!(json!(expand(s(&case["args"][0]), s(&case["args"][1]))), case["out"], "{}", case["name"]);
        }
        for case in f["tilde"].as_array().unwrap() {
            assert_eq!(json!(tilde(s(&case["args"][0]), s(&case["args"][1]))), case["out"], "{}", case["name"]);
        }
        for case in f["with_ignore_unknown"].as_array().unwrap() {
            assert_eq!(json!(with_ignore_unknown(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn the_constants_are_the_ts_text() {
        let f = fixture();
        let want = |name: &str| f["constants"].as_array().unwrap().iter().find(|c| c["name"] == name).unwrap()["out"].clone();
        assert_eq!(json!(SSH_CONFIG_PATH), want("SSH_CONFIG_PATH"));
        assert_eq!(json!(SSH_IGNORE_UNKNOWN_DETAIL), want("SSH_IGNORE_UNKNOWN_DETAIL"));
        assert_eq!(json!(PRELUDE), want("PRELUDE"));
        assert_eq!(json!(FINDER_METADATA), want("FINDER_METADATA"));
        assert_eq!(json!(CACHE_DIRS.to_vec()), want("CACHE_DIRS in order"));
    }
}
