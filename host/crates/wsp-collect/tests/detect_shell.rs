// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/shell.test.ts.
mod common;

use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::detect::shell::detect_shell;
use wsp_collect::manifest::ManifestEntry;

/// The row as JSON with its label replaced by a marker, as the TS `label: expect.any(String)` does.
fn any_label(row: &ManifestEntry) -> Value {
    let mut v = serde_json::to_value(row).unwrap();
    assert!(v["label"].is_string(), "label is a string in {v}");
    v["label"] = json!("<any string>");
    v
}

fn values(rows: &[ManifestEntry]) -> Vec<Value> {
    rows.iter().map(|r| serde_json::to_value(r).unwrap()).collect()
}

mod shell {
    use super::*;

    #[test]
    fn is_offered_as() {
        let rows = [
            ("~/.zshrc", "shell/zshrc", "~/.zshrc"),
            ("~/.zshenv", "shell/zshenv", "~/.zshenv"),
            ("~/.bashrc", "shell/bashrc", "~/.bashrc"),
            ("~/.bash_profile", "shell/bash_profile", "~/.bash_profile"),
            ("~/.profile", "shell/profile", "~/.profile"),
            ("~/.inputrc", "shell/inputrc", "~/.inputrc"),
            ("~/.aliases", "shell/aliases", "~/.aliases"),
            ("~/.config/fish/config.fish", "shell/fish", "~/.config/fish"),
            ("~/.config/starship.toml", "shell/starship", "~/.config/starship.toml"),
            ("~/.p10k.zsh", "shell/p10k", "~/.p10k.zsh"),
            ("~/.config/oh-my-posh/theme.json", "shell/oh-my-posh", "~/.config/oh-my-posh"),
            ("~/.tmux.conf", "shell/tmux", "~/.tmux.conf"),
            ("~/.config/tmux/tmux.conf", "shell/tmux", "~/.config/tmux"),
            ("~/.config/zellij/config.kdl", "shell/zellij", "~/.config/zellij"),
            ("~/.config/direnv/direnvrc", "shell/direnv", "~/.config/direnv"),
            ("~/.direnvrc", "shell/direnv", "~/.direnvrc"),
            ("~/.config/sheldon/plugins.toml", "shell/sheldon", "~/.config/sheldon"),
        ];
        for (file, id, path) in rows {
            let found = block_on(detect_shell(&fake_host(FakeLaptop::new().file(file, 10))));
            let got: Vec<Value> = found.iter().map(any_label).collect();
            assert_eq!(
                got,
                vec![json!({ "rung": "shell", "id": id, "label": "<any string>", "paths": [path], "bytes": 10, "default": "bring" })],
                "{file}"
            );
        }
    }

    #[test]
    fn records_the_login_shell_on_every_shell_row_when_shell_names_one_etc_shells_lists() {
        let shells = "# List of acceptable login shells\n/bin/bash\n/bin/zsh\n/opt/homebrew/bin/fish\n";
        let zsh = block_on(detect_shell(&fake_host(
            FakeLaptop::new().shell("/bin/zsh").file("~/.zshrc", 10).file("~/.config/fish/config.fish", 5).file("/etc/shells", shells),
        )));
        let got: Vec<(String, Option<String>)> = zsh.iter().map(|r| (r.id.clone(), r.login.clone())).collect();
        assert_eq!(got, vec![("shell/zshrc".to_string(), Some("zsh".to_string())), ("shell/fish".to_string(), Some("zsh".to_string()))]);
        let fish = block_on(detect_shell(&fake_host(
            FakeLaptop::new().shell("/opt/homebrew/bin/fish").file("~/.zshrc", 10).file("/etc/shells", shells),
        )));
        assert_eq!(fish[0].id, "shell/zshrc");
        assert_eq!(fish[0].login.as_deref(), Some("fish"));
    }

    #[test]
    fn a_shell_that_etc_shells_does_not_list_or_no_shell_at_all_leaves_the_rows_without() {
        let unlisted = block_on(detect_shell(&fake_host(
            FakeLaptop::new().shell("/usr/local/bin/nu").file("~/.zshrc", 10).file("/etc/shells", "/bin/bash\n/bin/zsh\n"),
        )));
        assert_eq!(unlisted[0].login, None);
        let missing = block_on(detect_shell(&fake_host(FakeLaptop::new().shell("/bin/zsh").file("~/.zshrc", 10))));
        assert_eq!(missing[0].login, None);
        let none = block_on(detect_shell(&fake_host(FakeLaptop::new().file("~/.zshrc", 10).file("/etc/shells", "/bin/zsh\n"))));
        assert_eq!(none[0].login, None);
    }

    #[test]
    fn oh_my_zsh_brings_only_the_custom_dir_the_framework_reinstalls() {
        let rows = block_on(detect_shell(&fake_host(
            FakeLaptop::new().file("~/.oh-my-zsh/oh-my-zsh.sh", 9000).file("~/.oh-my-zsh/custom/aliases.zsh", 200),
        )));
        assert_eq!(
            values(&rows),
            vec![json!({
                "rung": "shell", "id": "shell/oh-my-zsh", "label": "oh-my-zsh custom dir (the framework reinstalls)",
                "paths": ["~/.oh-my-zsh/custom"], "bytes": 200, "default": "bring",
            })]
        );
    }

    #[test]
    fn plugin_managers_that_reinstall_from_the_rc_file_are_listed_with_no_paths() {
        let rows = block_on(detect_shell(&fake_host(
            FakeLaptop::new().file("~/.local/share/zinit/zinit.git/zinit.zsh", 100).file("~/.zplug/init.zsh", 5),
        )));
        assert_eq!(
            values(&rows),
            vec![
                json!({ "rung": "shell", "id": "shell/zinit", "label": "zinit (reinstalls from ~/.zshrc)", "paths": [], "bytes": 0, "default": "bring" }),
                json!({ "rung": "shell", "id": "shell/zplug", "label": "zplug (reinstalls from ~/.zshrc)", "paths": [], "bytes": 0, "default": "bring" }),
            ]
        );
    }

    #[test]
    fn antidote_and_fisher_bring_their_plugin_list() {
        let rows = block_on(detect_shell(&fake_host(
            FakeLaptop::new()
                .file("~/.antidote/antidote.zsh", 5)
                .file("~/.zsh_plugins.txt", 60)
                .file("~/.config/fish/functions/fisher.fish", 5)
                .file("~/.config/fish/fish_plugins", 30),
        )));
        let got: Vec<(String, Vec<String>)> = rows.iter().map(|r| (r.id.clone(), r.paths.clone())).collect();
        let want: Vec<(String, Vec<String>)> = vec![
            ("shell/fish", vec!["~/.config/fish"]),
            ("shell/antidote", vec!["~/.zsh_plugins.txt"]),
            ("shell/fisher", vec!["~/.config/fish/fish_plugins"]),
        ]
        .into_iter()
        .map(|(id, p)| (id.to_string(), p.into_iter().map(String::from).collect()))
        .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn history_is_offered_unticked_in_one_row() {
        let rows = block_on(detect_shell(&fake_host(
            FakeLaptop::new().file("~/.zsh_history", 800_000).file("~/.local/share/atuin/history.db", 2_000_000),
        )));
        assert_eq!(
            values(&rows),
            vec![json!({
                "rung": "shell", "id": "shell/history", "label": "shell history",
                "paths": ["~/.zsh_history", "~/.local/share/atuin"], "bytes": 2_800_000, "default": "skip",
            })]
        );
    }

    #[test]
    fn an_empty_laptop_gives_no_shell_rows() {
        assert_eq!(block_on(detect_shell(&empty_host())), Vec::<ManifestEntry>::new());
    }
}
