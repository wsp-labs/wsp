// SPDX-License-Identifier: AGPL-3.0-only
//! The rc files, which names mean a secret, and the files an rc file sources. Port of the cases of
//! packages/collect/test/shell-rc.test.ts that do not cut a file (those are detect_shell_rc_strip.rs).
use wsp_collect::detect::shell_rc::{is_rc_path, is_secret_name, rc_files, sourced_paths, FISH_CONF_D, RC_NAMES, RC_PATHS};
use wsp_collect::detect::sources::bare_sources;

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

mod shell_rc_exports {
    use super::*;

    #[test]
    fn names_the_rc_files_the_shell_rung_carries_plus_fishs_config_and_its_conf_d() {
        let all = [
            ".zshrc",
            ".zshenv",
            ".zprofile",
            ".zlogin",
            ".bashrc",
            ".bash_profile",
            ".profile",
            ".inputrc",
            ".aliases",
            ".zsh_aliases",
            ".config/fish/config.fish",
        ];
        assert_eq!(RC_PATHS.to_vec(), all.to_vec());
        assert_eq!(FISH_CONF_D, ".config/fish/conf.d");
        assert_eq!(
            RC_NAMES.to_vec(),
            vec![
                ".zshrc",
                ".zshenv",
                ".zprofile",
                ".zlogin",
                ".bashrc",
                ".bash_profile",
                ".profile",
                ".inputrc",
                ".aliases",
                ".zsh_aliases",
                "config.fish"
            ]
        );
        let mut want = strings(&all);
        want.push(".config/fish/conf.d/work.fish".to_string());
        want.push(".config/fish/conf.d/env.fish".to_string());
        assert_eq!(rc_files(&strings(&["work.fish", "notes.txt", "env.fish"])), want);
        for rel in [".zshrc", ".config/fish/config.fish", ".config/fish/conf.d/work.fish"] {
            assert!(is_rc_path(rel), "{rel}");
        }
        for rel in [
            "x/.zshrc",
            ".config/fish/conf.d/sub/deep.fish",
            ".config/fish/conf.d/notes.txt",
            ".config/fish/functions/x.fish",
            "config.fish",
        ] {
            assert!(!is_rc_path(rel), "{rel}");
        }
    }

    #[test]
    fn the_paths_an_rc_file_sources_by_a_literal_path_home_and_absolute_forms_quoted_or() {
        let text = [
            "source ~/.zsh/secrets.zsh",
            ". \"$HOME/.zsh/work.zsh\"",
            "[ -f ~/.zsh/local.zsh ] && source ~/.zsh/local.zsh",
            "if [ -f ${HOME}/.zsh/if.zsh ]; then . ${HOME}/.zsh/if.zsh; fi",
            "source /Users/dev/.zsh/abs.zsh 2>/dev/null",
            "source '/Users/dev/.zsh/quoted.zsh'",
            "source $ZDOTDIR/.zshrc",
            "source ~/.zsh/*.zsh",
            "for f in ~/.zsh/*.zsh; do source $f; done",
            "source \"$(brew --prefix)/etc/x\"",
            "# source ~/.zsh/commented.zsh",
            "alias s='source ~/.zsh/aliased.zsh'",
            "source relative/path.zsh",
            "source ~/.zsh/secrets.zsh",
            "\\. \"$HOME/.nvm/nvm.sh\"",
        ]
        .join("\n");
        assert_eq!(
            sourced_paths(&text, "/Users/dev"),
            strings(&[
                "/Users/dev/.zsh/secrets.zsh",
                "/Users/dev/.zsh/work.zsh",
                "/Users/dev/.zsh/local.zsh",
                "/Users/dev/.zsh/if.zsh",
                "/Users/dev/.zsh/abs.zsh",
                "/Users/dev/.zsh/quoted.zsh",
                "/Users/dev/.nvm/nvm.sh"
            ])
        );
    }

    #[test]
    fn the_alias_fallback_and_the_source_guard_read_the_same_files_nvms_escaped_dot() {
        let nvm = "\\. \"$HOME/.nvm/nvm.sh\"\n";
        assert_eq!(sourced_paths(nvm, "/Users/dev"), strings(&["/Users/dev/.nvm/nvm.sh"]));
        assert_eq!(bare_sources(nvm, "/Users/dev"), strings(&["~/.nvm/nvm.sh"]));
        for line in [
            "source ~/../etc/profile\n",
            "source /Users/dev/../etc/profile\n",
            "source ~//x\n",
            ". ~/.zsh//x\n",
            "source ~/.zsh/my\\ file.zsh\n",
            "source ~/.x\"y\"\n",
        ] {
            assert_eq!(sourced_paths(line, "/Users/dev"), Vec::<String>::new(), "{line}");
            assert_eq!(bare_sources(line, "/Users/dev"), Vec::<String>::new(), "{line}");
        }
    }

    #[test]
    fn names_that_mean_a_secret_a_secret_word_alone_between_underscores_and_in_capitals() {
        let secret = [
            "ANTHROPIC_API_KEY",
            "GITHUB_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
            "DB_PASSWORD",
            "KEY",
            "npm_token",
            "OPENAI_APIKEY",
            "PGPASSWORD",
            "MYSQL_PWD",
            "GHTOKEN",
            "LAB_CLIENTSECRET",
            "smtp_passwd",
            "MYAPIKEY",
            "PASSWORD_STORE_GITHUB_TOKEN",
            "password_store_token",
        ];
        let missed: Vec<&str> = secret.iter().copied().filter(|n| !is_secret_name(n)).collect();
        assert_eq!(missed, Vec::<&str>::new());
        let plain = [
            "KEYTIMEOUT",
            "PATH",
            "EDITOR",
            "SECRETS_DIR",
            "TOKENIZERS_PARALLELISM",
            "HISTKEY",
            "MONKEY",
            "MAX_TOKENS",
            "PWD",
            "OLDPWD",
            "_PWD",
            "fish_prompt_pwd_dir_length",
            "old_pwd",
            "ctoken",
        ];
        let caught: Vec<&str> = plain.iter().copied().filter(|n| is_secret_name(n)).collect();
        assert_eq!(caught, Vec::<&str>::new());
    }

    #[test]
    fn pass_or_passphrase_as_a_word_in_any_case_and_in_capitals_glued_at_the_end_or_a() {
        let secret = [
            "SSHPASS",
            "BORG_PASSPHRASE",
            "REDISCLI_AUTH",
            "RCLONE_CONFIG_PASS",
            "GITHUB_PAT",
            "DB_PASS",
            "PASS",
            "SMTP_PASS_FILE",
            "GPGPASSPHRASE",
            "LAB_ROOTPASS",
            "NPM_CONFIG__AUTH",
            "DB_REPASS",
            "db_pass",
            "borg_passphrase",
            "_mysql_pass",
        ];
        let missed: Vec<&str> = secret.iter().copied().filter(|n| !is_secret_name(n)).collect();
        assert_eq!(missed, Vec::<&str>::new());
        let plain = [
            "BYPASS",
            "PATH",
            "PATCH",
            "AUTHOR",
            "COMPASS",
            "PROXY_BYPASS",
            "LAB_ENCOMPASS",
            "SSH_ASKPASS",
            "SUDO_ASKPASS",
            "GIT_ASKPASS",
            "SSH_AUTH_SOCK",
            "AUTH_SOCK",
            "PAT",
            "AUTH",
            "XAUTHORITY",
            "MANPATH",
            "QUILT_PATCHES",
            "PASSED",
            "pat",
            "_target_opt_pat",
            "sshpass",
            "github_pat",
        ];
        let caught: Vec<&str> = plain.iter().copied().filter(|n| is_secret_name(n)).collect();
        assert_eq!(caught, Vec::<&str>::new());
    }
}

/// Rules of spec/shell_rc.md the TS cases leave unpinned, values read off the TS at a3b870a6e.
mod pins {
    use wsp_collect::detect::shell_rc::{is_secret_name, simple_commands, source_command, sourced_path, SourceCommand};

    #[test]
    fn simple_commands_split_outside_quotes_and_drop_keywords_and_comments() {
        assert_eq!(simple_commands(""), Vec::<String>::new());
        assert_eq!(simple_commands("a;;b"), vec!["a", "b"]);
        assert_eq!(simple_commands("if true; then export A=1; fi"), vec!["true", "export A=1", " fi"]);
        assert_eq!(simple_commands(r#"echo 'a;b' "c|d" e\;f # g; h"#), vec![r#"echo 'a;b' "c|d" e\;f "#]);
        // Right after an array's `=(` a `#` starts a comment, as bash and zsh read it.
        assert_eq!(simple_commands("x=(# a comment) y"), vec!["x="]);
    }

    #[test]
    fn a_hash_after_a_no_break_space_starts_a_comment() {
        // JavaScript's \s holds U+00A0 and not U+0085.
        assert_eq!(simple_commands("echo x\u{a0}#c"), vec!["echo x\u{a0}"]);
        assert_eq!(simple_commands("echo x\u{85}#c"), vec!["echo x\u{85}#c"]);
    }

    #[test]
    fn sourced_path_folds_home_and_refuses_anything_a_shell_would_expand() {
        let cases = [
            ("~/a/b", Some("/h/a/b")),
            ("\"$HOME/x\"", Some("/h/x")),
            ("${HOME}/y", Some("/h/y")),
            ("'/abs/z'", Some("/abs/z")),
            ("~/a//b", None),
            ("/a/./b", None),
            ("~/a*", None),
            ("rel", None),
            ("~", None),
            ("\"~/q'", None),
        ];
        for (token, want) in cases {
            assert_eq!(sourced_path(token, "/h").as_deref(), want, "{token}");
        }
        assert_eq!(
            source_command("  \\. \"$HOME/.nvm/nvm.sh\" --no-use"),
            Some(SourceCommand {
                indent: "  ".into(),
                word: "\\.".into(),
                token: "\"$HOME/.nvm/nvm.sh\"".into(),
                rest: " --no-use".into()
            })
        );
        assert_eq!(source_command("sourcex ~/a"), None);
        // ß uppercases to SS, and TOKEN alone is a secret's word in any case.
        assert!(is_secret_name("\u{df}_token"));
    }
}
