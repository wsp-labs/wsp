// SPDX-License-Identifier: AGPL-3.0-only
//! Cutting secrets out of a carried rc file. Port of the cases of packages/collect/test/shell-rc.test.ts that call
//! stripExports. Every shell text is written with the same escapes as the TS string it came from.
mod common;

use wsp_collect::detect::shell_rc::{is_secret_name, strip_exports, Stripped};

fn strip(text: &str) -> Stripped {
    strip_exports(text, None, &mut Vec::new(), false)
}

fn script(text: &str) -> Stripped {
    strip_exports(text, None, &mut Vec::new(), true)
}

fn login(l: &str) -> Option<String> {
    if l.contains("plain0") {
        Some("a login".to_string())
    } else {
        None
    }
}

fn cut(text: &str) -> String {
    strip_exports(text, Some(&login), &mut Vec::new(), true).carried
}

fn assert_stripped(got: &Stripped, names: &[&str], carried: &str) {
    assert_eq!(got.names, names.iter().map(|s| s.to_string()).collect::<Vec<String>>());
    assert_eq!(got.carried, carried);
}

mod shell_rc_exports {
    use super::*;

    #[test]
    fn a_shells_or_oh_my_zshs_own_setting_that_holds_a_secret_word_alone_is_plain() {
        let settings = [
            "fish_key_bindings",
            "fish_sequence_key_delay_ms",
            "DISABLE_FZF_KEY_BINDINGS",
            "PASSWORD_STORE_DIR",
            "PASSWORD_STORE_KEY",
            "PASSWORD_STORE_SIGNING_KEY",
            "TIKTOKEN_CACHE_DIR",
            "CLAUDE_CODE_SKIP_BEDROCK_AUTH",
            "CLAUDE_CODE_SKIP_ANTHROPIC_GOOGLE_CLOUD_AUTH",
        ];
        let caught: Vec<&str> = settings.iter().copied().filter(|n| is_secret_name(n)).collect();
        assert_eq!(caught, Vec::<&str>::new());
        let fish = "set -g fish_key_bindings fish_vi_key_bindings\nset -U fish_sequence_key_delay_ms 200\nset -gx PGPASSWORD plain0pg\n";
        assert_stripped(
            &script(fish),
            &["PGPASSWORD"],
            "set -g fish_key_bindings fish_vi_key_bindings\nset -U fish_sequence_key_delay_ms 200\ntrue\n",
        );
    }

    #[test]
    fn lists_the_names_and_cuts_the_lines_exported_or_plain_single_or_multiple_per_line() {
        let text = "export PATH=$HOME/bin:$PATH\nexport ANTHROPIC_API_KEY=sk-redacted\nexport KEYTIMEOUT=1\nGITHUB_TOKEN=ghp_redacted\n  export A=1 DB_PASSWORD=x\nalias ll='ls -l'\n";
        assert_stripped(
            &strip(text),
            &["ANTHROPIC_API_KEY", "GITHUB_TOKEN", "DB_PASSWORD"],
            "export PATH=$HOME/bin:$PATH\nexport KEYTIMEOUT=1\nalias ll='ls -l'\n",
        );
    }

    #[test]
    fn in_a_script_a_cut_command_stands_as_true_at_its_own_indent_so_a_block_it_was_the() {
        let text = "if [ -f ~/.work ]; then\n  export GH_TOKEN=x\nfi\ndb() {\n\texport API_KEY=\"a\nb\"\n}\n";
        assert_stripped(
            &strip_exports(text, Some(&login), &mut Vec::new(), true),
            &["GH_TOKEN", "API_KEY"],
            "if [ -f ~/.work ]; then\n  true\nfi\ndb() {\n\ttrue\n}\n",
        );
        assert_eq!(strip_exports(text, Some(&login), &mut Vec::new(), false).carried, "if [ -f ~/.work ]; then\nfi\ndb() {\n}\n");
        assert_eq!(cut("cat <<EOF\nplain0\nEOF\n"), "cat <<EOF\nEOF\n");
        assert_eq!(cut("# psql plain0\nalias g=git\n"), "alias g=git\n");
        assert_eq!(
            cut("case $OSTYPE in\n  darwin*) export GH_TOKEN=x ;;\n  *) export A=1 ;;\nesac\n"),
            "case $OSTYPE in\n  darwin*) true ;;\n  *) export A=1 ;;\nesac\n"
        );
    }

    #[test]
    fn in_a_script_a_cut_command_that_shares_its_line_stands_as_true_and_the_rest_of() {
        assert_eq!(cut("if x; then\n  export GH_TOKEN=x; fi\n"), "if x; then\n  true; fi\n");
        assert_eq!(cut("for f in a; do\n  export LAB_TOKEN=x; done\n"), "for f in a; do\n  true; done\n");
        assert_eq!(cut("if x\nthen export GH_TOKEN=x\n  y\nfi\n"), "if x\nthen true\n  y\nfi\n");
        assert_eq!(cut("{ export API_KEY=x; export IN=1; }\ndb() { export API_KEY=x; }\n"), "{ true; export IN=1; }\ndb() { true; }\n");
        assert_eq!(
            cut("case $x in\n  a)\n    export GH_TOKEN=x ;;\n  b) export B_TOKEN=y ;&\n  c) export C_TOKEN=z ;|\nesac\n"),
            "case $x in\n  a)\n    true ;;\n  b) true ;&\n  c) true ;|\nesac\n"
        );
        assert_eq!(cut("if x; then\n  export API_KEY=\"a\nb\"; export IN=1; fi\n"), "if x; then\n  true; export IN=1; fi\n");
        assert_eq!(cut("[ -n x ] && export GH_TOKEN=x || y\n"), "[ -n x ] && true || y\n");
        assert_stripped(
            &strip_exports(
                "if x; then export A=1 GH_TOKEN=x; elif y; then eval 'export B_KEY=1; z'; fi\n",
                Some(&login),
                &mut Vec::new(),
                true,
            ),
            &["GH_TOKEN", "B_KEY"],
            "if x; then true; elif y; then true; fi\n",
        );
    }

    #[test]
    fn a_cut_command_holds_together_through_its_substitutions_quotes_arrays_and() {
        assert_eq!(cut("  export GH_TOKEN=\"$(pass show \"gh; x\" | head -1)\" 2>&1 && y; fi\n"), "  true && y; fi\n");
        assert_eq!(cut("export A_TOKEN=$((1 + 2)) `echo ;` $'a\\';' ${X:-;}; fi\n"), "true; fi\n");
        assert_eq!(
            cut("typeset -a API_KEY=(a b); fi\nif test -n x; set -gx GH_TOKEN (pass show gh); end\n"),
            "true; fi\nif test -n x; true; end\n"
        );
        assert_eq!(cut("export GH_TOKEN=x; cat <<EOF; fi\nbody\nEOF\nnext\n"), "true\nnext\n");
        assert_eq!(cut("  y; fi # plain0\n"), "  y; fi\n");
        assert_eq!(cut("for t in plain0; do y; done\ncase $x in (a) y plain0;; (b) z;; esac\n"), "true\n");
    }

    #[test]
    fn in_a_script_what_a_cut_command_decided_never_runs_a_condition_holding_one_stands() {
        assert_eq!(cut("if [ \"$T\" = plain0 ]; then\n  y\nfi\n"), "if false; then\n  y\nfi\n");
        assert_eq!(
            cut("if ! [[ -z x || $T = plain0 ]]; then y; elif x && test $T = plain0; then y; fi\n"),
            "if false; then y; elif false; then y; fi\n"
        );
        assert_eq!(
            cut("while [ \"$T\" = plain0 ]; do y; done\nuntil ! [[ -z x && $T = plain0 ]]; do y; done\nwhile x; do y plain0; done\n"),
            "while false; do y; done\nuntil true; do y; done\nwhile x; do true; done\n"
        );
        assert_eq!(
            cut("if [[ $T = plain0 ]] { y } elif [[ $T = plain0 ]] { y }\nuntil [[ $T = plain0 ]] { y }\n"),
            "if [[ -n '' ]] { y } elif [[ -n '' ]] { y }\nuntil [[ -z '' ]] { y }\n"
        );
        assert_eq!(
            cut("[ \"$T\" != plain0 ] && return\ncurl -fs https://plain0@h || y\necho plain0 | gh auth login --with-token\n  x && grep -q plain0 f && y; z\n"),
            "true\ntrue\ntrue\n  true; z\n"
        );
        assert_eq!(
            cut(
                "if export GH_TOKEN=x; then y; fi\nexport GH_TOKEN=x && y\nGH_TOKEN=$(pass show gh) || y\nGH_TOKEN=x gh auth status && y\n"
            ),
            "if true; then y; fi\ntrue && y\ntrue\ntrue\n"
        );
    }

    #[test]
    fn an_assignment_behind_a_precommand_word_a_declaration_builtin_or_fishs_and_or_and() {
        let text = [
            "time -p export A_TOKEN=x",
            "noglob builtin typeset -x B_TOKEN=x",
            "nocorrect export C_TOKEN=x",
            "- export D_TOKEN=x",
            "command -p export E_TOKEN=x",
            "exec -a lab env F_TOKEN=x make",
            "float -x G_KEY=x",
            "integer H_KEY=1",
            "private I_TOKEN=x",
            "and set -gx J_TOKEN x",
            "or set -x K_TOKEN x",
            "not set -gx L_TOKEN x",
            "set --global --export M_TOKEN x",
            "and set -gx --path N_TOKEN x",
            "if not set -q GH_TOKEN",
            "set -qg GH_TOKEN; or echo none",
            "set --erase GH_TOKEN; set -S GH_TOKEN",
            "time make && noglob git status",
            "and git status; or command -v git",
            "",
        ]
        .join("\n");
        assert_stripped(
            &script(&text),
            &["A_TOKEN", "B_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "F_TOKEN", "G_KEY", "H_KEY", "I_TOKEN", "J_TOKEN", "K_TOKEN", "L_TOKEN", "M_TOKEN", "N_TOKEN"],
            "time -p true\nnoglob builtin true\nnocorrect true\n- true\ncommand -p true\nexec -a lab true\ntrue\ntrue\ntrue\nand true\nor true\nnot true\ntrue\nand true\nif not set -q GH_TOKEN\nset -qg GH_TOKEN; or echo none\nset --erase GH_TOKEN; set -S GH_TOKEN\ntime make && noglob git status\nand git status; or command -v git\n",
        );
    }

    #[test]
    fn zshs_repeat_and_coproc_a_backslash_before_the_word_and_execs_bundled_flags_hide() {
        let text = [
            "repeat 1 export A_TOKEN=x",
            "coproc export B_TOKEN=x",
            "\\export C_TOKEN=x",
            "\\builtin export D_TOKEN=x",
            "exec -cla lab env E_TOKEN=x zsh",
            "while noglob F_TOKEN=x check; do sleep 1; done",
            "until time G_TOKEN=x check; do sleep 1; done",
            "repeat 3 echo hi",
            "",
        ]
        .join("\n");
        assert_stripped(
            &script(&text),
            &["A_TOKEN", "B_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "F_TOKEN", "G_TOKEN"],
            "repeat 1 true\ncoproc true\ntrue\n\\builtin true\nexec -cla lab true\nwhile false; do sleep 1; done\nuntil true; do sleep 1; done\nrepeat 3 echo hi\n",
        );
    }

    #[test]
    fn an_assignment_sudo_doas_nohup_or_nice_passes_on_is_cut_and_a_line_naming_the() {
        let text = [
            "sudo -u root GH_TOKEN=x gh auth status",
            "doas GH_TOKEN=x cmd",
            "nohup env GH_TOKEN=x cmd &",
            "nice -n 5 env GH_TOKEN=x cmd",
            "sudo --preserve-env=GH_TOKEN gh auth status",
            "sudo printenv GH_TOKEN",
            "",
        ]
        .join("\n");
        assert_stripped(
            &script(&text),
            &["GH_TOKEN"],
            "true\ntrue\ntrue &\ntrue\nsudo --preserve-env=GH_TOKEN gh auth status\nsudo printenv GH_TOKEN\n",
        );
    }

    #[test]
    fn a_script_with_nothing_to_cut_comes_back_byte_for_byte() {
        let text = "if [ -f ~/.work ]; then\n  export EDITOR=vim\nfi\n\nalias g=git\n";
        let none = |_: &str| -> Option<String> { None };
        assert_stripped(&strip_exports(text, Some(&none), &mut Vec::new(), true), &[], text);
    }

    #[test]
    fn a_quoted_value_spanning_lines_and_a_backslash_continuation_are_cut_whole() {
        assert_stripped(&strip("export A=1\nexport API_KEY=\"first\nsecond\"\nexport B=2\n"), &["API_KEY"], "export A=1\nexport B=2\n");
        assert_eq!(strip("export DB_PASSWORD='p\nq\nr'\nalias x=y\n").carried, "alias x=y\n");
        assert_stripped(&strip("export A_TOKEN=abc\\\n  def\\\n  ghi\nexport C=3\n"), &["A_TOKEN"], "export C=3\n");
        assert_eq!(strip("export QUOTED=\"a\\\"b\"\nexport A_KEY=\"x\\\"y\nz\"\nlast\n").carried, "export QUOTED=\"a\\\"b\"\nlast\n");
    }

    #[test]
    fn an_array_spanning_lines_under_a_secret_name_is_cut_whole_quotes_comments_and() {
        let text = [
            "API_KEY=(",
            "  hunter2-prod",
            "  hunter2-stage",
            ")",
            "declare -a GH_TOKEN=(",
            "  \"a ) b\"",
            "  'c)'",
            "  $(pass show gh)",
            ")",
            "typeset -a LAB_SECRET=(",
            "  x # note (",
            "  *(.)",
            ")",
            "LAB_TOKEN+=(",
            "  more",
            ")",
            "if x; then",
            "  DB_PASSWORD=(",
            "    p # (",
            "  ); export IN=1; fi",
            "PLAIN=(",
            "  one",
            "  two",
            ")",
            "",
        ]
        .join("\n");
        assert_stripped(
            &script(&text),
            &["API_KEY", "GH_TOKEN", "LAB_SECRET", "LAB_TOKEN", "DB_PASSWORD"],
            "true\ntrue\ntrue\ntrue\nif x; then\n  true; export IN=1; fi\nPLAIN=(\n  one\n  two\n)\n",
        );
        let none = |_: &str| -> Option<String> { None };
        assert_eq!(
            strip_exports("PLAIN=(\n  one\n  two\n)\nLIST=( a\n  b )\n", Some(&none), &mut Vec::new(), true).carried,
            "PLAIN=(\n  one\n  two\n)\nLIST=( a\n  b )\n"
        );
    }

    #[test]
    fn an_array_under_a_secret_name_is_cut_whole_past_a_comment_right_after_its_a() {
        let text = [
            "API_KEY=(#prod)",
            "  hunter2-prod",
            ")",
            "DB_PASSWORD=(#it's here",
            "  hunter2-db",
            ")",
            "GH_TOKEN=(",
            "  ${x%)}",
            "  hunter2-gh",
            ")",
            "LAB_SECRET=(",
            "  $(op read \"op://Private/Lab (work)/key\")",
            "  hunter2-lab",
            ")",
            "export AFTER=1",
            "",
        ]
        .join("\n");
        assert_stripped(&script(&text), &["API_KEY", "DB_PASSWORD", "GH_TOKEN", "LAB_SECRET"], "true\ntrue\ntrue\ntrue\nexport AFTER=1\n");
    }

    #[test]
    fn crlf_files_are_stripped_too_and_keep_their_line_endings() {
        assert_stripped(&strip("export A_TOKEN=x\r\nexport B=1\r\n"), &["A_TOKEN"], "export B=1\r\n");
    }

    #[test]
    fn typeset_declare_and_fish_set_are_assignments_as_well() {
        let text = "typeset -gx ANTHROPIC_API_KEY=sk-x\ndeclare -x DB_PASSWORD=pw\ndeclare -rx EDITOR=vim\nset -gx OPENAI_API_KEY sk-y\nset -g fish_greeting ''\nset -Ux HOMEBREW_GITHUB_API_TOKEN t\n";
        assert_stripped(
            &strip(text),
            &["ANTHROPIC_API_KEY", "DB_PASSWORD", "OPENAI_API_KEY", "HOMEBREW_GITHUB_API_TOKEN"],
            "declare -rx EDITOR=vim\nset -g fish_greeting ''\n",
        );
    }

    #[test]
    fn heredocs_command_substitutions_and_backticks_spanning_lines_are_cut_whole() {
        assert_stripped(&strip("export API_KEY=$(cat <<EOF\nsecret-value\nEOF\n)\nexport B=2\n"), &["API_KEY"], "export B=2\n");
        assert_eq!(strip("export A_TOKEN=$(\n  op read x\n)\nnext\n").carried, "next\n");
        assert_eq!(strip("export DB_PASSWORD=`cat \n  file`\nnext\n").carried, "next\n");
        assert_eq!(strip("export X_SECRET=\"$(cat <<-EOT\n\tv\n\tEOT\n)\"\nnext\n").carried, "next\n");
        assert_stripped(
            &strip("cat <<EOF\nexport NOT_A_TOKEN=inside-heredoc\nPLAIN=1\nEOF\n"),
            &["NOT_A_TOKEN"],
            "cat <<EOF\nPLAIN=1\nEOF\n",
        );
    }

    #[test]
    fn readonly_local_a_quoted_assignment_and_a_flagless_fish_set_are_assignments_too() {
        assert_stripped(
            &strip("readonly MY_TOKEN=x\nlocal DB_PASSWORD=y\nexport \"GH_TOKEN=z\"\nexport 'API_KEY=w'\nset MY_KEY val\nset plain val\nreadonly PLAIN=1\n"),
            &["MY_TOKEN", "DB_PASSWORD", "GH_TOKEN", "API_KEY", "MY_KEY"],
            "set plain val\nreadonly PLAIN=1\n",
        );
    }

    #[test]
    fn a_comments_stray_quote_an_arithmetic_shift_or_a_stray_backtick_on_a_kept_line() {
        let template = "# Set list of themes to pick from when loading at random\n# Setting this variable when ZSH_THEME=random will cause zsh to load\n# a theme from this variable instead of looking in $ZSH/themes/\n# If set to an empty array, this variable will have no effect.\n# ZSH_THEME_RANDOM_CANDIDATES=( \"robbyrussell\" \"agnoster\" )\n# Uncomment the following line to use case-sensitive completion.\n# Caution: this setting can cause issues with multiline prompts in zsh < 5.7.1 (see #5765)\n# Uncomment one of the following lines to change the auto-update behavior\n# zstyle ':omz:update' mode disabled  # disable automatic updates\n# Uncomment if you don't want to see the prompt.\nplugins=(git)\nexport ANTHROPIC_API_KEY=sk-ant-fake\n";
        assert_stripped(&strip(template), &["ANTHROPIC_API_KEY"], &template.replace("export ANTHROPIC_API_KEY=sk-ant-fake\n", ""));
        assert_stripped(&strip("(( y = x << 2 ))\nexport GH_TOKEN=fake\nnext\n"), &["GH_TOKEN"], "(( y = x << 2 ))\nnext\n");
        assert_stripped(&strip("# see `man zsh\nexport DB_PASSWORD=fake\nnext\n"), &["DB_PASSWORD"], "# see `man zsh\nnext\n");
        assert_eq!(strip("echo 'a' # it's fine\nexport A_TOKEN=fake\n").names, vec!["A_TOKEN".to_string()]);
    }

    #[test]
    fn a_here_string_or_a_shift_inside_arithmetic_on_a_kept_line_opens_nothing_so_the() {
        assert_stripped(&strip("read x <<< hello\nexport A_TOKEN=fake\nnext\n"), &["A_TOKEN"], "read x <<< hello\nnext\n");
        assert_eq!(strip("cat <<< \"hello\"\nexport B_TOKEN=fake\n").names, vec!["B_TOKEN".to_string()]);
        assert_stripped(&strip("(( f = 1 << SHIFT ))\nexport C_TOKEN=fake\nnext\n"), &["C_TOKEN"], "(( f = 1 << SHIFT ))\nnext\n");
        assert_stripped(
            &strip("(( f = 1 << SHIFT ))\ncat <<EOF\nexport NOT_TOKEN=inside\nEOF\nexport D_TOKEN=fake\n"),
            &["NOT_TOKEN", "D_TOKEN"],
            "(( f = 1 << SHIFT ))\ncat <<EOF\nEOF\n",
        );
        assert_eq!(strip("read x <<< $y\nexport E_TOKEN=fake\n").names, vec!["E_TOKEN".to_string()]);
    }

    #[test]
    fn an_arithmetic_block_spanning_kept_lines_opens_no_heredoc_on_its_later_line() {
        assert_stripped(
            &strip("(( f = 1 +\n  (1 << SHIFT) ))\nexport A_TOKEN=fake\nnext\n"),
            &["A_TOKEN"],
            "(( f = 1 +\n  (1 << SHIFT) ))\nnext\n",
        );
    }

    #[test]
    fn a_zsh_glob_flag_or_a_brace_expansion_holding_is_not_a_comment_so_an_export_after() {
        assert_stripped(&strip("[[ $x == (#i)yes ]] && export A_TOKEN=fake\nnext\n"), &["A_TOKEN"], "next\n");
        assert_stripped(&strip("echo {#a,b}; export B_TOKEN=fake\nnext\n"), &["B_TOKEN"], "next\n");
        assert_stripped(&strip("echo {#a,b} # it's\nexport C_TOKEN=fake\n"), &["C_TOKEN"], "echo {#a,b} # it's\n");
    }

    #[test]
    fn inside_double_quotes_counts_for_nothing_an_unbalanced_suppresses_heredoc() {
        let text = "echo \"((\"\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport A_TOKEN=fake\nnext\n";
        assert_stripped(&strip(text), &["NOT_TOKEN", "A_TOKEN"], "echo \"((\"\ncat <<EOF\nEOF\nnext\n");
        assert_stripped(
            &strip("(( x = 1\nl1\nl2\nl3\nl4\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport B_TOKEN=fake\n"),
            &["NOT_TOKEN", "B_TOKEN"],
            "(( x = 1\nl1\nl2\nl3\nl4\ncat <<EOF\nEOF\n",
        );
        assert_stripped(
            &strip("echo \"$((1+2))\"\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport C_TOKEN=fake\n"),
            &["NOT_TOKEN", "C_TOKEN"],
            "echo \"$((1+2))\"\ncat <<EOF\nEOF\n",
        );
    }

    #[test]
    fn an_arithmetic_block_spanning_kept_lines_keeps_a_shift_on_every_one_of_them() {
        assert_stripped(
            &strip("((\na = 1 +\nb << SHIFT\n))\nexport A_TOKEN=fake\nnext\n"),
            &["A_TOKEN"],
            "((\na = 1 +\nb << SHIFT\n))\nnext\n",
        );
        assert_stripped(
            &strip("((\na = 1 +\nb = 2 +\nc = 3 +\nd << SHIFT\n))\nexport B_TOKEN=fake\nexport C_KEY=fake\nalias x=y\n"),
            &["B_TOKEN", "C_KEY"],
            "((\na = 1 +\nb = 2 +\nc = 3 +\nd << SHIFT\n))\nalias x=y\n",
        );
    }

    #[test]
    fn the_arithmetic_depth_is_one_stream_across_cut_and_kept_lines() {
        assert_stripped(&strip("export A_TOKEN=fake; (( y = 1 +\nexport B_KEY=fake\n"), &["A_TOKEN", "B_KEY"], "");
        assert_stripped(
            &strip("export A_TOKEN=fake; (( y = 1 +\nz << SHIFT ))\nexport B_KEY=fake\nnext\n"),
            &["A_TOKEN", "B_KEY"],
            "z << SHIFT ))\nnext\n",
        );
        assert_stripped(
            &strip("(( x = 1 +\nexport A_TOKEN=fake )); y=2\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport B_KEY=fake\n"),
            &["A_TOKEN", "NOT_TOKEN", "B_KEY"],
            "(( x = 1 +\ncat <<EOF\nEOF\n",
        );
        assert_stripped(
            &strip("(( x = 1 +\nexport A_TOKEN=\"a\nb\" )); y=2\ncat <<EOF\nexport NOT_TOKEN=data\nEOF\nexport B_KEY=fake\n"),
            &["A_TOKEN", "NOT_TOKEN", "B_KEY"],
            "(( x = 1 +\ncat <<EOF\nEOF\n",
        );
        assert_stripped(
            &strip("(( x = 1 +\nexport A_TOKEN=fake +\n2 << SHIFT ))\nexport B_TOKEN=fake\nnext\n"),
            &["A_TOKEN", "B_TOKEN"],
            "(( x = 1 +\n2 << SHIFT ))\nnext\n",
        );
    }

    #[test]
    fn a_parameter_expansion_spanning_lines_and_a_backslash_quoted_heredoc_word_are_cut() {
        assert_stripped(&strip("export A_TOKEN=${SECRET:-\nfake}\nnext\n"), &["A_TOKEN"], "next\n");
        assert_stripped(&strip("export API_KEY=$(cat <<\\EOF\nline)\nfake\nEOF\n)\nnext\n"), &["API_KEY"], "next\n");
        assert_stripped(
            &strip("cat <<\\EOF\nexport NOT_TOKEN=inside\nEOF\nexport F_TOKEN=fake\n"),
            &["NOT_TOKEN", "F_TOKEN"],
            "cat <<\\EOF\nEOF\n",
        );
    }

    #[test]
    fn a_comment_right_after_or_on_a_cut_line_ends_the_scan_there_and_var_is_a_length() {
        assert_stripped(
            &strip("export A_TOKEN=x;# don't\nalias a=b\nexport B_KEY=y &# it's\nalias c=d\n"),
            &["A_TOKEN", "B_KEY"],
            "alias a=b\nalias c=d\n",
        );
        assert_stripped(&strip("export A_TOKEN=${#x}rest\nnext\n"), &["A_TOKEN"], "next\n");
        assert_stripped(&strip("echo ${#arr}\nexport B_KEY=v\nnext\n"), &["B_KEY"], "echo ${#arr}\nnext\n");
    }

    #[test]
    fn a_cut_lines_trailing_comment_does_not_swallow_the_lines_after_it() {
        assert_stripped(
            &strip("export A_TOKEN=fake # don't share\nalias b=c\nexport X_SECRET=fake # `note\nalias d=e\n"),
            &["A_TOKEN", "X_SECRET"],
            "alias b=c\nalias d=e\n",
        );
    }

    #[test]
    fn an_export_that_is_not_the_first_word_of_the_line_is_found_and_the_whole_line_cut() {
        let shapes = [
            "[ -z \"$X\" ] && export FOO_TOKEN=lit",
            "if true; then export BAR_KEY=lit; fi",
            "cd /tmp && export BAZ_SECRET=lit",
            "env QUX_TOKEN=lit somecommand",
            "{ export A_KEY=lit; }",
            "alias x='y'; export E_TOKEN=lit",
            "LAB=1 H_TOKEN=lit cmd",
        ];
        let names = ["FOO_TOKEN", "BAR_KEY", "BAZ_SECRET", "QUX_TOKEN", "A_KEY", "E_TOKEN", "H_TOKEN"];
        for (i, line) in shapes.iter().enumerate() {
            let got = strip(&format!("{line}\nnext\n"));
            assert_eq!(got.names, vec![names[i].to_string()], "{line}");
            assert_eq!(got.carried, "next\n", "{line}");
        }
        assert_stripped(&strip("exec env J_TOKEN=lit cmd\nnext\n"), &["J_TOKEN"], "next\n");
        assert_stripped(&strip("eval 'export I_TOKEN=lit'\nnext\n"), &["I_TOKEN"], "next\n");
        assert_stripped(
            &strip("eval \"cd /tmp; export K_SECRET=lit\"\ncommand export L_KEY=lit\nbuiltin export M_TOKEN=lit\nnext\n"),
            &["K_SECRET", "L_KEY", "M_TOKEN"],
            "next\n",
        );
        assert_eq!(strip("echo \"a && export NOT_TOKEN=1\"\n").names, Vec::<String>::new());
    }

    #[test]
    fn an_alias_or_abbr_body_is_shell_again_so_a_secret_it_assigns_is_cut_like_a_bare() {
        let text = [
            "alias gh='GH_TOKEN=lit gh'",
            "alias gl=\"GITLAB_TOKEN=lit glab\"",
            "alias x=';export FAKE_TOKEN=1'",
            "alias -g DBP='DB_PASSWORD=lit' a1='A1_TOKEN=lit'",
            "\\alias bs='env BS_TOKEN=lit gh'",
            "alias -- dd='DD_TOKEN=lit gh'",
            "alias fa 'FA_TOKEN=lit gh'",
            "alias fs 'set -lx FS_TOKEN lit; gh'",
            "abbr -a ab 'AB_TOKEN=lit gh'",
            "abbr --add aw AW_TOKEN=lit gh",
            "abbr ap --position anywhere -- 'AP_TOKEN=lit'",
            "abbr -a -r '^g.+' ar 'AR_TOKEN=lit'",
            "abbr -ac git ac 'AC_TOKEN=lit'",
            "alias lp='LAB=1 LP_TOKEN=lit gh'",
            "if x; then alias in='IN_TOKEN=lit gh'; fi",
            "alias tok='printenv GH_TOKEN'",
            "alias ft 'printenv GH_TOKEN'",
            "abbr -a fe 'echo $GH_TOKEN'",
            "alias g=git",
            "",
        ]
        .join("\n");
        let carried = format!(
            "{}if x; then true; fi\nalias tok='printenv GH_TOKEN'\nalias ft 'printenv GH_TOKEN'\nabbr -a fe 'echo $GH_TOKEN'\nalias g=git\n",
            "true\n".repeat(14)
        );
        assert_stripped(
            &script(&text),
            &[
                "GH_TOKEN",
                "GITLAB_TOKEN",
                "FAKE_TOKEN",
                "DB_PASSWORD",
                "A1_TOKEN",
                "BS_TOKEN",
                "DD_TOKEN",
                "FA_TOKEN",
                "FS_TOKEN",
                "AB_TOKEN",
                "AW_TOKEN",
                "AP_TOKEN",
                "AR_TOKEN",
                "AC_TOKEN",
                "LP_TOKEN",
                "IN_TOKEN",
            ],
            &carried,
        );
    }

    #[test]
    fn a_quoted_builtin_set_or_variable_name_is_read_as_the_bare_one_a_case_patterns() {
        let text = [
            "\"export\" A_TOKEN=lit",
            "ex'port' B_TOKEN=lit",
            "export \"C_TOKEN=lit\"",
            "export \"D_TOKEN\"=lit",
            "'typeset' -x E_TOKEN=lit",
            "set -gx \"F_TOKEN\" lit",
            "\"set\" -gx G_TOKEN lit",
            "set -gx 'PLAIN' 1",
            "export \"PATH=$HOME/bin:$PATH\"",
            "case $x in",
            "  \"I_KEY=\"*) echo has ;;",
            "esac",
            "",
        ]
        .join("\n");
        assert_stripped(
            &script(&text),
            &["A_TOKEN", "B_TOKEN", "C_TOKEN", "D_TOKEN", "E_TOKEN", "F_TOKEN", "G_TOKEN", "I_KEY"],
            "true\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\nset -gx 'PLAIN' 1\nexport \"PATH=$HOME/bin:$PATH\"\ncase $x in\n  true) echo has ;;\nesac\n",
        );
    }

    #[test]
    fn a_heredoc_body_line_or_an_eval_or_alias_body_over_lines_that_assigns_a_secret_is() {
        let text = [
            "cat > ~/.lab.env <<EOF",
            "API_KEY=lit",
            "PLAIN=1",
            "EOF",
            "cat >> ~/.lab.env <<'EOF'",
            "  export GH_TOKEN=lit",
            "DB_PASSWORD=\"lit",
            "lit\"",
            "LAB=2",
            "EOF",
            "eval 'cd /tmp",
            "export LAB_TOKEN=lit'",
            "eval \"",
            "  LAB_SECRET=lit",
            "\"",
            "alias gh='cd /tmp",
            "AL_TOKEN=lit gh'",
            "eval 'echo it'\\''s",
            "export QT_TOKEN=lit'",
            "x=\\'; eval $'echo it\\'s",
            "export AC_TOKEN=lit'",
            "if x; then eval '",
            "  export IF_KEY=lit",
            "'; y; fi",
            "eval 'echo kept",
            "export EVAL_PLAIN=1'",
            "cat <<EOF",
            "KEYTIMEOUT=1",
            "it's",
            "EOF",
            "export AFTER=1",
            "",
        ]
        .join("\n");
        assert_stripped(
            &script(&text),
            &["API_KEY", "GH_TOKEN", "DB_PASSWORD", "LAB_TOKEN", "LAB_SECRET", "AL_TOKEN", "QT_TOKEN", "AC_TOKEN", "IF_KEY"],
            concat!(
                "cat > ~/.lab.env <<EOF\nPLAIN=1\nEOF\ncat >> ~/.lab.env <<'EOF'\nLAB=2\nEOF\n",
                "true\ntrue\ntrue\ntrue\nx=\\'; true\nif x; then true; y; fi\n",
                "eval 'echo kept\nexport EVAL_PLAIN=1'\ncat <<EOF\nKEYTIMEOUT=1\nit's\nEOF\nexport AFTER=1\n"
            ),
        );
        assert_stripped(&strip("eval 'cd /tmp\nexport LAB_TOKEN=lit'\nnext\n"), &["LAB_TOKEN"], "next\n");
        for word in ["\\eval", "\"eval\"", "'eval'", "e\\val", "LANG=C eval"] {
            let got = script(&format!("{word} 'export LAB_TOKEN=lit'\n{word} 'cd /tmp\nexport LAB_TOKEN=lit'\nnext\n"));
            assert_eq!(got.names, vec!["LAB_TOKEN".to_string()], "{word}");
            assert_eq!(got.carried, "true\ntrue\nnext\n", "{word}");
        }
        assert_stripped(&strip("cat <<EOF\nexport \\\nAPI_KEY=lit\nEOF\nnext\n"), &["API_KEY"], "cat <<EOF\nexport \\\n\nEOF\nnext\n");
        let tok = |l: &str| -> Option<String> {
            if l.contains("tok-") {
                Some("a token".to_string())
            } else {
                None
            }
        };
        let mut held: Vec<String> = Vec::new();
        assert_stripped(&strip_exports("eval 'cd /tmp\necho tok-lit'\nnext\n", Some(&tok), &mut held, true), &[], "true\nnext\n");
        assert_eq!(held, vec!["a token".to_string()]);
        assert_stripped(
            &script("X=\"$(cat <<EOF\nAPI_KEY=lit\nPLAIN=1\nEOF\n)\"\nnext\n"),
            &["API_KEY"],
            "X=\"$(cat <<EOF\nPLAIN=1\nEOF\n)\"\nnext\n",
        );
        let fish = "abbr -a gw 'git commit -m \\'wip\\''\nset -gx API_KEY lit\nabbr -a gs 'git status'\n";
        assert_stripped(&script(fish), &["API_KEY"], &fish.replace("set -gx API_KEY lit", "true"));
    }

    #[test]
    fn ansi_c_quoting_on_a_cut_line_closes_at_its_own_quote() {
        assert_stripped(&strip("export D_PASSWORD=$'it\\'s'\nline1\nline2\nline3\n"), &["D_PASSWORD"], "line1\nline2\nline3\n");
    }
}

/// Rules of spec/shell_rc_strip.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use wsp_collect::detect::shell_rc::strip_exports;

    #[test]
    fn secret_in_names_go_to_held_and_never_to_names() {
        let mut held = vec!["KEEP".to_string()];
        let find = |l: &str| if l.contains("sk-") { Some("OPENAI".to_string()) } else { None };
        let got = strip_exports("echo sk-123\nexport PLAIN=1\necho sk-456\n", Some(&find), &mut held, true);
        assert_eq!(got.names, Vec::<String>::new());
        assert_eq!(got.carried, "true\nexport PLAIN=1\ntrue\n");
        // Each name once, after what the caller already held.
        assert_eq!(held, vec!["KEEP", "OPENAI"]);
    }

    #[test]
    fn an_empty_file_and_a_lone_cut_line_carry_nothing_but_their_newline_rule() {
        let mut held = Vec::new();
        let empty = strip_exports("", None, &mut held, false);
        assert_eq!((empty.names, empty.carried), (Vec::<String>::new(), String::new()));
        let cut = strip_exports("export A_TOKEN=x\n", None, &mut held, false);
        assert_eq!((cut.names, cut.carried), (vec!["A_TOKEN".to_string()], String::new()));
        let script = strip_exports("export A_TOKEN=x\n", None, &mut held, true);
        assert_eq!((script.names, script.carried), (vec!["A_TOKEN".to_string()], "true\n".to_string()));
        assert!(held.is_empty());
    }
}

/// Every case of tests/fixtures/oracle/w2/shell_rc.json and shell_rc_strip.json (oracle/w2-shell_rc*.oracle.ts):
/// specs shell_rc's and shell_rc_strip's wave-2 rules. A `secret_in` is recorded as data: it answers `name` (the line
/// itself for `$line`) for a line holding `contains`, else nothing.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, strs, w2};
    use serde_json::{json, Value};
    use std::cell::RefCell;
    use wsp_collect::detect::shell_rc::{
        is_rc_path, is_secret_name, rc_files, simple_commands, source_command, sourced_path, sourced_paths,
    };
    use wsp_collect::detect::shell_rc_strip::strip_exports;

    fn s(v: &Value) -> &str {
        v.as_str().unwrap()
    }

    /// Runs `strip_exports` as the case says and answers its result and every line `secret_in` was handed.
    fn strip(case: &Value) -> (Value, Vec<String>) {
        let a = &case["args"];
        let seen = RefCell::new(Vec::new());
        let rule = a[1].clone();
        let secret_in = |line: &str| -> Option<String> {
            seen.borrow_mut().push(line.to_string());
            let holds = line.contains(rule["contains"].as_str().unwrap());
            holds.then(|| if rule["name"] == "$line" { line.to_string() } else { rule["name"].as_str().unwrap().to_string() })
        };
        let mut held = strs(&a[2]);
        let r = strip_exports(
            s(&a[0]),
            (!rule.is_null()).then_some(&secret_in as &dyn Fn(&str) -> Option<String>),
            &mut held,
            a[3].as_bool().unwrap(),
        );
        (json!({ "names": r.names, "carried": r.carried, "held": held }), seen.into_inner())
    }

    #[test]
    fn the_rc_rules_answer_what_the_ts_answered() {
        let f = w2("shell_rc");
        for case in cases(&f, "is_rc_path") {
            assert_eq!(json!(is_rc_path(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "rc_files") {
            assert_eq!(json!(rc_files(&strs(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "is_secret_name") {
            assert_eq!(json!(is_secret_name(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "simple_commands") {
            assert_eq!(json!(simple_commands(s(&case["args"][0]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "source_command") {
            let got = source_command(s(&case["args"][0]))
                .map(|c| json!({ "indent": c.indent, "word": c.word, "token": c.token, "rest": c.rest }))
                .unwrap_or(Value::Null);
            assert_eq!(got, case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "sourced_path") {
            assert_eq!(json!(sourced_path(s(&case["args"][0]), s(&case["args"][1]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "sourced_paths") {
            assert_eq!(json!(sourced_paths(s(&case["args"][0]), s(&case["args"][1]))), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "strip_exports_assigned") {
            assert_eq!(strip(case).0, case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn strip_exports_carries_cuts_and_asks_what_the_ts_did() {
        let f = w2("shell_rc_strip");
        for case in cases(&f, "strip_exports") {
            assert_eq!(strip(case).0, case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "strip_exports_calls") {
            assert_eq!(json!(strip(case).1), case["out"], "{}", case["name"]);
        }
    }
}
