// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/shell-quote.test.ts at a3b870a6e.
//! Not ported: shell-quote.test.ts:43 "no other source file spells out the '\\'' rule" scans the TS source tree for a
//! second copy of the rule, which a Rust crate has no counterpart of.

use std::process::Command;

use wsp_wire::types::shell_quote::{in_folder, shell_line, shell_quote};

const TABLE: &[&str] = &[
    "",
    "plain",
    "don't",
    "'",
    "''",
    "a\nb",
    "-n",
    "--flag=x",
    "héllo wörld ✓",
    "$HOME `id` $(reboot)",
    "back\\slash",
    "a b\tc",
    "say \"hi\"",
    "semi; and && or ||",
];

fn sh(script: &str) -> String {
    let out = Command::new("/bin/sh").arg("-c").arg(script).output().expect("runs sh");
    assert!(out.status.success(), "sh failed on {script:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).expect("utf8")
}

mod shell_quote_words {
    use super::*;

    // shell-quote.test.ts:12 at a3b870a6e
    #[test]
    fn sh_reads_j_back_byte_for_byte() {
        for input in TABLE {
            let out = sh(&format!("printf '%s' {}", shell_quote(input)));
            assert_eq!(out, *input, "sh read {input:?} back differently");
        }
    }

    // shell-quote.test.ts:17 at a3b870a6e
    #[test]
    fn several_words_survive_as_separate_argv_entries() {
        let argv = ["don't", "", "-n", "$x y"];
        let words: Vec<String> = argv.iter().map(|w| shell_quote(w)).collect();
        let out = sh(&format!("printf '%s\\n' {}", words.join(" ")));
        assert_eq!(out, "don't\n\n-n\n$x y\n");
    }
}

mod shell_line_words {
    use super::*;

    // shell-quote.test.ts:25 at a3b870a6e
    #[test]
    fn leaves_a_word_sh_reads_as_itself_bare_and_quotes_every_other_so_the_line_reads_as_t() {
        assert_eq!(
            shell_line(&["npx", "-y", "@wsp-labs/wsp@0.1.2", "mcp", "--state", "/Users/p/.wsp/state.json"]),
            "npx -y @wsp-labs/wsp@0.1.2 mcp --state /Users/p/.wsp/state.json"
        );
        assert_eq!(
            shell_line(&["/opt/node/bin/node", "--disable-warning=ExperimentalWarning", "/opt/wsp/dist/bin.js", "mcp"]),
            "/opt/node/bin/node --disable-warning=ExperimentalWarning /opt/wsp/dist/bin.js mcp"
        );
        assert_eq!(
            shell_line(&["wsp", "mcp", "--state", "/Users/p/my dir/state.json", "", "$HOME", "~/x", "a*"]),
            r"wsp mcp --state '/Users/p/my dir/state.json' '' '$HOME' '~/x' 'a*'"
        );
    }

    // shell-quote.test.ts:31 at a3b870a6e
    #[test]
    fn sh_reads_the_line_back_as_the_same_argv() {
        let argv = ["/opt/node/bin/node", "--state", "/Users/p/my dir/state.json", "don't", "", "$x y", "@wsp-labs/wsp@0.1.2"];
        let out = sh(&format!("printf '%s\\n' {}", shell_line(&argv)));
        assert_eq!(out, format!("{}\n", argv.join("\n")));
    }
}

mod in_folder_command {
    use super::*;

    // shell-quote.test.ts:50 at a3b870a6e
    #[test]
    fn prefixes_the_command_with_a_cd_into_the_quoted_folder_or_into_when_none_was_named() {
        assert_eq!(in_folder(Some("/root/work/my proj"), "'git' 'status'"), "cd '/root/work/my proj' && 'git' 'status'");
        assert_eq!(in_folder(None, "claude -p"), "cd ~ && claude -p");
    }
}
