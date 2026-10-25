// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/sources.test.ts.
mod common;

use std::collections::{HashMap, HashSet};

use common::fake_host::*;
use wsp_collect::collect::run_detector;
use wsp_collect::detect::sources::{bare_sources, guard_sources};
use wsp_collect::manifest::{ManifestEntry, Rung};

const HOME: &str = "/Users/dev";

fn rc() -> String {
    [
        r#"export PATH="$HOME/bin:$PATH""#,
        r#". "$HOME/.cargo/env""#,
        "source ~/.deno/env  # deno",
        r#"  source "${HOME}/.zsh/functions.zsh""#,
        ". /Users/dev/.local/share/x/env",
        "source '/Users/dev/.aliases'",
        r#"\. "$HOME/.bun/_bun""#,
        "[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh",
        r#"if [ -s "$HOME/.sdkman/bin/sdkman-init.sh" ]; then"#,
        r#"  source "$HOME/.sdkman/bin/sdkman-init.sh""#,
        "fi",
        "source $ZSH/oh-my-zsh.sh",
        r#". "$NVM_DIR/nvm.sh""#,
        "source /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
        "source ~/.secrets 2>/dev/null",
        "source ~/.extra || true",
        "# source ~/.disabled",
        "source ~/.deno/env",
        "source ~/.zsh/*.zsh",
        "source",
        ".",
        "sourced=1",
        "source ~/.x; source ~/.y",
        r#". "/Users/dev/.zsh/functions.zsh""#,
    ]
    .join("\n")
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

mod bare_sources {
    use super::*;

    #[test]
    fn a_line_that_is_one_source_or_dot_of_a_literal_path_and_nothing_else_once_per_fil() {
        assert_eq!(
            bare_sources(&rc(), HOME),
            strings(&[
                "~/.cargo/env",
                "~/.deno/env",
                "~/.zsh/functions.zsh",
                "~/.local/share/x/env",
                "~/.aliases",
                "~/.bun/_bun",
                "~/.sdkman/bin/sdkman-init.sh",
                "/opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh",
            ])
        );
    }

    #[test]
    fn a_guarded_redirected_chained_variable_glob_relative_or_commented_line_is_not_one() {
        let paths = bare_sources(&rc(), HOME);
        assert!(!paths.contains(&"~/.fzf.zsh".to_string()));
        assert!(!paths.contains(&"~/.secrets".to_string()));
        assert!(!paths.contains(&"~/.extra".to_string()));
        assert!(!paths.contains(&"~/.disabled".to_string()));
        assert!(!paths.contains(&"~/.x".to_string()));
        assert!(!paths.iter().any(|p| p.contains('*') || p.contains('$')));
        assert_eq!(bare_sources("", HOME), Vec::<String>::new());
        assert_eq!(
            bare_sources("source ~someone/.x\nsource ~\n. ~/\nsource ~/../etc/profile\nsource ~//x\nsource /\nsource /opt/../etc/x\nsource opt/x\nsource ./x\n", HOME),
            Vec::<String>::new()
        );
    }
}

mod guard_sources {
    use super::*;

    #[test]
    fn wraps_each_bare_line_whose_file_is_not_present_a_path_outside_home_always_keepin() {
        let present: HashSet<String> = ["~/.zsh/functions.zsh", "~/.aliases"].iter().map(|s| s.to_string()).collect();
        let text = rc();
        let out = guard_sources(&text, HOME, &|rel: &str| present.contains(&format!("~/{rel}")));
        let lines: Vec<&str> = out.split('\n').collect();
        let before: Vec<&str> = text.split('\n').collect();
        assert_eq!(lines.len(), before.len());
        assert_eq!(lines[1], r#"[ -r "$HOME/.cargo/env" ] && . "$HOME/.cargo/env""#);
        assert_eq!(lines[2], "[ -r ~/.deno/env ] && source ~/.deno/env # deno");
        assert_eq!(lines[3], before[3]);
        assert_eq!(lines[4], r#"[ -r "$HOME"/.local/share/x/env ] && . "$HOME"/.local/share/x/env"#);
        assert_eq!(lines[5], r#"[ -r "$HOME"'/.aliases' ] && source "$HOME"'/.aliases'"#);
        assert_eq!(lines[6], r#"[ -r "$HOME/.bun/_bun" ] && \. "$HOME/.bun/_bun""#);
        assert_eq!(lines[9], r#"  [ -r "$HOME/.sdkman/bin/sdkman-init.sh" ] && source "$HOME/.sdkman/bin/sdkman-init.sh""#);
        assert_eq!(
            lines[13],
            "[ -r /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh ] && source /opt/homebrew/share/zsh-autosuggestions/zsh-autosuggestions.zsh"
        );
        assert_eq!(lines[17], "[ -r ~/.deno/env ] && source ~/.deno/env");
        for i in [0, 7, 8, 10, 11, 12, 14, 15, 16, 18, 19, 20, 21, 22] {
            assert_eq!(lines[i], before[i], "line {i}");
        }
    }

    #[test]
    fn a_line_naming_this_computer_s_home_literally_is_wrapped_even_when_its_file_trave() {
        let rc = [
            r#". "/Users/dev/.zsh/functions.zsh""#,
            "source /Users/dev/.zsh/functions.zsh",
            "source '/Users/dev/.zsh/functions.zsh'  # fns",
            "source /Users/developer/.x",
            "",
        ]
        .join("\n");
        assert_eq!(
            guard_sources(&rc, HOME, &|_| true),
            [
                r#"[ -r "$HOME/.zsh/functions.zsh" ] && . "$HOME/.zsh/functions.zsh""#,
                r#"[ -r "$HOME"/.zsh/functions.zsh ] && source "$HOME"/.zsh/functions.zsh"#,
                r#"[ -r "$HOME"'/.zsh/functions.zsh' ] && source "$HOME"'/.zsh/functions.zsh' # fns"#,
                "[ -r /Users/developer/.x ] && source /Users/developer/.x",
                "",
            ]
            .join("\n")
        );
        assert_eq!(bare_sources(&rc, HOME), strings(&["~/.zsh/functions.zsh", "/Users/developer/.x"]));
    }

    #[test]
    fn everything_present_leaves_the_text_as_it_was_line_endings_and_trailing_newline_i() {
        let crlf = ". \"$HOME/.cargo/env\"\r\nalias g=git\r\n";
        assert_eq!(guard_sources(crlf, HOME, &|_| true), crlf);
        assert_eq!(
            guard_sources("source /opt/homebrew/etc/profile.d/z.sh\n", HOME, &|_| true),
            "[ -r /opt/homebrew/etc/profile.d/z.sh ] && source /opt/homebrew/etc/profile.d/z.sh\n"
        );
        assert_eq!(guard_sources(crlf, HOME, &|_| false), "[ -r \"$HOME/.cargo/env\" ] && . \"$HOME/.cargo/env\"\r\nalias g=git\r\n");
        assert_eq!(guard_sources("alias g=git", HOME, &|_| false), "alias g=git");
    }
}

mod the_shell_rung_records_the_bare_sources_of_each_zsh_and_bash_rc_file {
    use super::*;

    #[test]
    fn the_paths_land_on_the_rc_file_s_own_row_a_file_with_none_a_fish_config_and_input() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.zshrc", ". \"$HOME/.cargo/env\"\nsource ~/.zsh/aliases.zsh\n[ -f ~/.fzf.zsh ] && source ~/.fzf.zsh\n")
                .file("~/.zprofile", "eval \"$(/opt/homebrew/bin/brew shellenv)\"\n")
                .file("~/.bashrc", "source ~/.bash_aliases\nsource /opt/homebrew/etc/profile.d/bash_completion.sh\n")
                .file("~/.inputrc", "$include ~/.inputrc_local\n")
                .file("~/.config/fish/config.fish", "source ~/.config/fish/local.fish\n"),
        );
        let rows = block_on(run_detector(Rung::Shell, &host, &[])).unwrap();
        let by_id: HashMap<&str, &ManifestEntry> = rows.iter().map(|r| (r.id.as_str(), r)).collect();
        let row = |id: &str| *by_id.get(id).unwrap_or_else(|| panic!("no row {id}"));
        assert_eq!(row("shell/zshrc").sources, Some(strings(&["~/.cargo/env", "~/.zsh/aliases.zsh"])));
        assert_eq!(row("shell/bashrc").sources, Some(strings(&["~/.bash_aliases", "/opt/homebrew/etc/profile.d/bash_completion.sh"])));
        assert_eq!(row("shell/zprofile").sources, None);
        assert_eq!(row("shell/inputrc").sources, None);
        assert_eq!(row("shell/fish").sources, None);
    }
}
