// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/commands.test.ts.
use wsp_collect::detect::commands::{called_commands, drop_plugins, guard_commands, Syntax, GUARD_BEGIN, GUARD_END};

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

fn rc() -> String {
    [
        "# starship: eval \"$(commented init zsh)\"",
        "export ZSH=\"$HOME/.oh-my-zsh\"",
        "plugins=(",
        "    git",
        "    eza",
        "    zsh-autosuggestions",
        ")",
        "plugins+=(zsh-syntax-highlighting)",
        "fpath=(/opt/homebrew/share/zsh/site-functions $fpath)",
        "source $ZSH/oh-my-zsh.sh",
        "eval \"$(starship init zsh)\"",
        "alias ls='eza -la' view=bat g=git",
        "alias please=\"sudo $(fc -ln -1)\"",
        "diskbloom --quiet",
        "autoload -U select-word-style",
        "select-word-style bash",
        "greet() { echo hi; }",
        "function bye { echo bye; }",
        "greet; bye",
        "[ -s \"$NVM_DIR/nvm.sh\" ] && \\. \"$NVM_DIR/nvm.sh\"",
        "command -v zoxide >/dev/null 2>&1 && eval \"$(zoxide init zsh)\"",
        "if command -v fnm > /dev/null; then eval `fnm env`; fi",
        "cat <<EOF",
        "not-a-command inside heredoc",
        "EOF",
        "FOO=1 BAR=2 mytool run",
        "/usr/local/bin/absolute --flag",
        "$HOME/bin/variable",
        "export PATH=\"$HOME/.local/bin:$PATH\"",
        "export MOTD=\"hello",
        "world\"",
        "print ${(j:,:)files}",
        "typeset -a more=(",
        "  first # (see docs)",
        "  second",
        ")",
        "",
    ]
    .join("\n")
}

mod called_commands {
    use super::*;

    #[test]
    fn names_each_command_the_file_calls_once_in_order() {
        // Inside $(...) and backticks, as an alias body's first word, and as a bare line's first word past its
        // environment and any wrapper such as sudo.
        assert_eq!(
            called_commands(&rc(), Syntax::Sh),
            names(&["starship", "eza", "bat", "git", "diskbloom", "zoxide", "fnm", "cat", "mytool"])
        );
    }

    #[test]
    fn a_builtin_a_keyword_a_function_or_alias_the_file_defines_is_no_call() {
        // Also an autoloaded name, an assignment, an array literal's words, a path, a variable, a comment, a heredoc
        // body, a word inside a string spanning lines and a zsh ${(flags)name}.
        let called = called_commands(&rc(), Syntax::Sh);
        for not in [
            "world",
            "print",
            "files",
            "first",
            "second",
            "see",
            "docs",
            "sudo",
            "fc",
            "export",
            "plugins",
            "zsh-autosuggestions",
            "zsh-syntax-highlighting",
            "fpath",
            "source",
            "eval",
            "alias",
            "ls",
            "view",
            "please",
            "autoload",
            "select-word-style",
            "greet",
            "bye",
            "echo",
            "command",
            "if",
            "then",
            "fi",
            "commented",
            "not-a-command",
            "inside",
            "EOF",
            "/usr/local/bin/absolute",
            "$HOME/bin/variable",
            "FOO=1",
        ] {
            assert!(!called.iter().any(|c| c == not), "{not} is not a call: {called:?}");
        }
    }

    #[test]
    fn zshs_precommand_and_reserved_words_are_no_calls() {
        // A guard by their name breaks the file or the word.
        let text = "noglob git status\nnocorrect git log\nfloat -x F=1.5\nprivate P=1\nrepeat 2 echo hi\n";
        assert_eq!(called_commands(text, Syntax::Sh), Vec::<String>::new());
    }

    #[test]
    fn a_fish_config_is_not_shell_and_yields_nothing_an_empty_file_yields_nothing() {
        assert_eq!(called_commands("", Syntax::Sh), Vec::<String>::new());
        assert_eq!(called_commands("set -gx EDITOR nvim\nstarship init fish | source\n", Syntax::Fish), Vec::<String>::new());
    }
}

mod guard_commands {
    use super::*;

    fn guard(name: &str) -> String {
        format!("{name}() {{ if (unset -f {name}; command -v {name}) >/dev/null 2>&1; then unset -f {name}; {name} \"$@\"; else return 127; fi; }}")
    }

    #[test]
    fn prepends_one_block_that_defines_a_guard_for_each_name() {
        // Each guard runs the command when it is there at call time and is a silent no-op when it is not, with a
        // one-line note naming them, and the file's own bytes stay as they were.
        let text = "eval \"$(starship init zsh)\"\nalias ls='eza -la'\ndiskbloom\n";
        let want = [
            GUARD_BEGIN.to_string(),
            guard("starship"),
            guard("eza"),
            guard("diskbloom"),
            "# Guarded above: a call to one of these that is not on this machine is silent instead of an error: starship, eza, diskbloom. Tick them in wsp init to install them.".to_string(),
            GUARD_END.to_string(),
            String::new(),
            "eval \"$(starship init zsh)\"".to_string(),
            "alias ls='eza -la'".to_string(),
            "diskbloom".to_string(),
            String::new(),
        ]
        .join("\n");
        assert_eq!(guard_commands(text, &names(&["starship", "eza", "diskbloom"]), &[]), want);
    }

    #[test]
    fn a_re_run_replaces_the_block_it_wrote_before() {
        // No missing name takes the block out.
        let text = "alias ls='eza -la'\n";
        let once = guard_commands(text, &names(&["eza", "starship"]), &[]);
        let again = guard_commands(&once, &names(&["eza"]), &[]);
        assert_eq!(again, guard_commands(text, &names(&["eza"]), &[]));
        assert_eq!(again.split(GUARD_BEGIN).count(), 2);
        assert_eq!(guard_commands(&once, &[], &[]), text);
        assert_eq!(guard_commands(text, &[], &[]), text);
    }

    #[test]
    fn names_each_plugin_left_out_on_its_own_note_line() {
        // After the no-ops and before the silenced line; plugins alone still get a block.
        let alone = [GUARD_BEGIN, "# plugin eza left out: eza is not on the image", GUARD_END, "", "plugins=(git)", ""].join("\n");
        assert_eq!(guard_commands("plugins=(git)\n", &[], &names(&["eza"])), alone);
        let both = [
            GUARD_BEGIN.to_string(),
            guard("diskbloom"),
            "# plugin eza left out: eza is not on the image".to_string(),
            "# plugin gcloud left out: gcloud is not on the image".to_string(),
            "# Guarded above: a call to one of these that is not on this machine is silent instead of an error: diskbloom. Tick them in wsp init to install them.".to_string(),
            GUARD_END.to_string(),
            String::new(),
            "plugins=(git)".to_string(),
            "diskbloom".to_string(),
            String::new(),
        ]
        .join("\n");
        assert_eq!(guard_commands("plugins=(git)\ndiskbloom\n", &names(&["diskbloom"]), &names(&["eza", "gcloud"])), both);
    }

    #[test]
    fn keeps_the_files_line_endings() {
        let want = format!(
            "{GUARD_BEGIN}\r\ndiskbloom() {{ if (unset -f diskbloom; command -v diskbloom) >/dev/null 2>&1; then unset -f diskbloom; diskbloom \"$@\"; else return 127; fi; }}\r\n# Guarded above: a call to one of these that is not on this machine is silent instead of an error: diskbloom. Tick them in wsp init to install them.\r\n{GUARD_END}\r\n\r\ndiskbloom\r\n"
        );
        assert_eq!(guard_commands("diskbloom\r\n", &names(&["diskbloom"]), &[]), want);
    }
}

mod drop_plugins {
    use super::*;

    fn should_drop(name: &str) -> bool {
        ["eza", "gcloud", "kubectl"].contains(&name)
    }

    #[test]
    fn takes_the_named_plugins_out_of_a_multi_line_list_and_a_plus_equals_line() {
        // Their lines go with them, they are named once in order, and every other byte stays.
        let text = [
            "plugins=(",
            "    git",
            "    eza  # listing",
            "    z",
            "    gcloud",
            "    kubectl",
            ")",
            "plugins+=(eza zsh-autosuggestions)",
            "alias e=eza",
            "",
        ]
        .join("\n");
        let got = drop_plugins(&text, &should_drop);
        let want = ["plugins=(", "    git", "    # listing", "    z", ")", "plugins+=(zsh-autosuggestions)", "alias e=eza", ""].join("\n");
        assert_eq!(got.text, want);
        assert_eq!(got.dropped, names(&["eza", "gcloud", "kubectl"]));
    }

    #[test]
    fn a_one_line_list_keeps_its_shape_a_list_with_nothing_to_drop_is_returned_as_it_is() {
        // Also another array, or a file without a list.
        let got = drop_plugins("plugins=(git eza z)\n", &should_drop);
        assert_eq!(got.text, "plugins=(git z)\n");
        assert_eq!(got.dropped, names(&["eza"]));
        for text in ["plugins=(git z)\n", "fpath=(eza $fpath)\n", "eval \"$(eza init)\"\n", ""] {
            let got = drop_plugins(text, &should_drop);
            assert_eq!(got.text, text);
            assert_eq!(got.dropped, Vec::<String>::new(), "{text:?}");
        }
    }
}

/// Rules of spec/commands.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use super::*;

    #[test]
    fn an_alias_body_runs_to_the_next_name_token() {
        // Each alias name is the file's own; each body's first word is a call, in order.
        assert_eq!(called_commands("alias ll='eza -l' v=nvim gs='git status'\nll\n", Syntax::Sh), vec!["eza", "nvim", "git"]);
    }

    #[test]
    fn a_name_defined_after_its_call_is_still_own() {
        assert_eq!(called_commands("mytool\nmytool() { :; }\n", Syntax::Sh), Vec::<String>::new());
    }

    #[test]
    fn an_array_literal_over_lines_with_a_comment_and_nested_parentheses_is_a_value() {
        assert_eq!(called_commands("arr=(\n  one # (c)\n  (two three)\n)\nafter\n", Syntax::Sh), vec!["after"]);
        // An array never closed swallows the rest of the file.
        assert_eq!(called_commands("x=(open\nnever\n", Syntax::Sh), Vec::<String>::new());
    }

    #[test]
    fn a_list_line_left_with_only_its_comment_keeps_the_comment_at_its_indent() {
        let got = drop_plugins("plugins=(\n  git # vcs\n  eza   # listing\n)\n", &|n: &str| n == "eza");
        assert_eq!(got.text, "plugins=(\n  git # vcs\n  # listing\n)\n");
        assert_eq!(got.dropped, vec!["eza"]);
        let one = drop_plugins("  plugins=(git eza) # mine\n", &|n: &str| n == "eza" || n == "git");
        assert_eq!(one.text, "  plugins=() # mine\n");
        assert_eq!(one.dropped, vec!["git", "eza"]);
    }
}
