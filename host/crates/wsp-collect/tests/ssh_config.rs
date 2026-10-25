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
