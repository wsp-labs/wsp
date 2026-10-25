// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/collect/test/terminal.test.ts.
mod common;
use common::fake_host::*;
use serde_json::{json, Value};
use wsp_collect::detect::terminal::{alacritty_font, detect_terminal_font, ghostty_font, iterm_font, kitty_font, wezterm_font};
use wsp_collect::host::Platform;

/// As defaults export writes it: every key on its own line, its value on the next.
const ITERM_PLIST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<plist version=\"1.0\">
<dict>
	<key>Default Bookmark Guid</key>
	<string>B-2</string>
	<key>New Bookmarks</key>
	<array>
		<dict>
			<key>Guid</key>
			<string>B-1</string>
			<key>Name</key>
			<string>Default</string>
			<key>Normal Font</key>
			<string>Monaco 12</string>
		</dict>
		<dict>
			<key>Background Color</key>
			<dict>
				<key>Red Component</key>
				<real>0</real>
			</dict>
			<key>Guid</key>
			<string>B-2</string>
			<key>Name</key>
			<string>Work</string>
			<key>Normal Font</key>
			<string>JetBrainsMonoNF-Regular 13</string>
		</dict>
	</array>
</dict>
</plist>";

mod terminal_font_parsers {
    use super::*;

    #[test]
    fn ghostty() {
        let rows: [(&str, Option<&str>); 3] = [
            (
                "# font-family = Commented\nfont-family = Hack Nerd Font Mono\nfont-family = Symbols Nerd Font\n",
                Some("Hack Nerd Font Mono"),
            ),
            ("font-family = Menlo\nfont-family = \nfont-family = Iosevka\n", Some("Iosevka")),
            ("font-size = 16\n", None),
        ];
        for (text, family) in rows {
            assert_eq!(ghostty_font(text).as_deref(), family, "{text:?}");
        }
    }

    #[test]
    fn iterm2_the_default_bookmarks_normal_font_without_its_size_else_the_first() {
        assert_eq!(iterm_font(ITERM_PLIST).as_deref(), Some("JetBrainsMonoNF-Regular"));
        let no_default = ITERM_PLIST.replacen("<key>Default Bookmark Guid</key>\n\t<string>B-2</string>", "", 1);
        assert_eq!(iterm_font(&no_default).as_deref(), Some("Monaco"));
        let unknown_default = ITERM_PLIST.replacen("<string>B-2</string>\n\t<key>New", "<string>B-9</string>\n\t<key>New", 1);
        assert_eq!(iterm_font(&unknown_default).as_deref(), Some("Monaco"));
        assert_eq!(iterm_font("<plist><dict></dict></plist>"), None);
    }

    #[test]
    fn wezterm() {
        let rows: [(&str, Option<&str>); 10] = [
            ("config.font = wezterm.font(\"Fira Code\")\n", Some("Fira Code")),
            ("config.font = wezterm.font('Iosevka Term', { weight = 'Bold' })\n", Some("Iosevka Term")),
            ("config.font = wezterm.font({ family = \"Cascadia Code\", weight = \"DemiBold\" })\n", Some("Cascadia Code")),
            ("config.font = wezterm.font { family = 'Monaspace Neon' }\n", Some("Monaspace Neon")),
            ("config.font = wezterm.font_with_fallback({ \"Victor Mono\", \"Symbols Nerd Font Mono\" })\n", Some("Victor Mono")),
            (
                "config.font = wezterm.font_with_fallback { { family = \"Recursive Mono\", weight = \"Medium\" }, \"Noto Color Emoji\" }\n",
                Some("Recursive Mono"),
            ),
            ("config.font_size = 14\n", Some("JetBrains Mono")),
            ("-- config.font = wezterm.font(\"Old\")\nconfig.font = wezterm.font(\"Fira Code\")\n", Some("Fira Code")),
            (
                "--[[\nconfig.font = wezterm.font(\"Old\")\n]]\nconfig.font = wezterm.font(\"Fira Code\") -- was wezterm.font(\"Older\")\n",
                Some("Fira Code"),
            ),
            ("-- config.font = wezterm.font(\"Old\")\n", Some("JetBrains Mono")),
        ];
        for (text, family) in rows {
            assert_eq!(wezterm_font(text).as_deref(), family, "{text:?}");
        }
    }

    #[test]
    fn kitty() {
        let rows: [(&str, Option<&str>); 5] = [
            ("font_family      Fira Code Retina\nbold_font auto\n", Some("Fira Code Retina")),
            ("font_family family=\"Iosevka Term\" style=\"Medium\"\n", Some("Iosevka Term")),
            ("font_family family=Hack\n", Some("Hack")),
            ("# font_family Menlo\nfont_size 13\n", None),
            ("font_family monospace\n", None),
        ];
        for (text, family) in rows {
            assert_eq!(kitty_font(text).as_deref(), family, "{text:?}");
        }
    }

    #[test]
    fn alacritty() {
        let rows: [(&str, Option<&str>); 7] = [
            ("[font.normal]\nfamily = \"Fira Code\"\nstyle = \"Retina\"\n", Some("Fira Code")),
            ("[font]\nsize = 13\nnormal = { family = \"Hack\", style = \"Regular\" }\n", Some("Hack")),
            ("[font]\nnormal.family = \"Iosevka\"\n", Some("Iosevka")),
            ("font:\n  size: 13\n  normal:\n    family: Fira Code\n    style: Regular\n", Some("Fira Code")),
            ("font:\n  normal:\n    family: \"Hack Nerd Font\"\n", Some("Hack Nerd Font")),
            ("[font]\nsize = 13\n", None),
            ("[font.normal]\nfamily = \"monospace\"\n", None),
        ];
        for (text, family) in rows {
            assert_eq!(alacritty_font(text).as_deref(), family, "{text:?}");
        }
    }
}

mod terminal_font_row {
    use super::*;

    fn row(font: &str, label: &str) -> Value {
        json!({ "rung": "shell", "id": "shell/terminal-font", "label": label, "paths": [], "bytes": 0, "default": "bring", "font": font })
    }

    fn rows_of(host: &FakeHost) -> Value {
        serde_json::to_value(block_on(detect_terminal_font(host))).unwrap()
    }

    fn iterm_laptop() -> FakeLaptop {
        FakeLaptop::new()
            .file("~/Library/Preferences/com.googlecode.iterm2.plist", 4000)
            .exec("defaults export com.googlecode.iterm2 -", ITERM_PLIST)
    }

    #[test]
    fn ghostty_with_no_font_family_or_with_no_config_at_all_when_it_is_the_running() {
        let jetbrains = json!([row("JetBrains Mono", "terminal font: JetBrains Mono (Ghostty)")]);
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().file("~/.config/ghostty/config", "font-size = 16\n"))), jetbrains);
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().terminal("ghostty").file("~/.zshrc", 10))), jetbrains);
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().file("/Applications/Ghostty.app/", 5000))), jetbrains);
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().platform(Platform::Linux).file("/Applications/Ghostty.app/", 5000))), json!([]));
        // The running terminal's default beats another terminal's config.
        let with_iterm = |terminal: &str| {
            let mut laptop = iterm_laptop();
            laptop.terminal = Some(terminal.to_string());
            fake_host(laptop)
        };
        assert_eq!(rows_of(&with_iterm("ghostty")), jetbrains);
        assert_eq!(rows_of(&with_iterm("WezTerm")), json!([row("JetBrains Mono", "terminal font: JetBrains Mono (WezTerm)")]));
    }

    #[test]
    fn ghostty_on_a_linux_computer_with_no_config_draws_with_the_same_built_in_font() {
        // Found on PATH rather than in /Applications.
        let jetbrains = json!([row("JetBrains Mono", "terminal font: JetBrains Mono (Ghostty)")]);
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().platform(Platform::Linux).which(&["ghostty"]).file("~/.zshrc", 10))), jetbrains);
        // Nothing of Ghostty there is no row, as a Mac without it has none.
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().platform(Platform::Linux).file("~/.zshrc", 10))), json!([]));
    }

    #[test]
    fn with_no_running_terminal_known_a_config_that_names_a_font_beats_an_installed() {
        let iterm = iterm_laptop().file("/Applications/Ghostty.app/", 5000);
        assert_eq!(rows_of(&fake_host(iterm)), json!([row("JetBrainsMonoNF-Regular", "terminal font: JetBrainsMonoNF-Regular (iTerm2)")]));
        let kitty =
            FakeLaptop::new().file("/Applications/Ghostty.app/", 5000).file("~/.config/kitty/kitty.conf", "font_family Fira Code\n");
        assert_eq!(rows_of(&fake_host(kitty)), json!([row("Fira Code", "terminal font: Fira Code (kitty)")]));
        // A Ghostty config, even one naming no font, says Ghostty is in use.
        let configured = FakeLaptop::new()
            .file("/Applications/Ghostty.app/", 5000)
            .file("~/.config/ghostty/config", "font-size = 16\n")
            .file("~/.config/kitty/kitty.conf", "font_family Fira Code\n");
        assert_eq!(rows_of(&fake_host(configured)), json!([row("JetBrains Mono", "terminal font: JetBrains Mono (Ghostty)")]));
    }

    #[test]
    fn reads_ghosttys_macos_config_location_and_prefers_the_one_under_config() {
        let mac = rows_of(&fake_host(
            FakeLaptop::new().file("~/Library/Application Support/com.mitchellh.ghostty/config", "font-family = Iosevka\n"),
        ));
        assert_eq!(mac[0]["font"], json!("Iosevka"));
        let both = rows_of(&fake_host(
            FakeLaptop::new()
                .file("~/.config/ghostty/config", "font-family = Hack\n")
                .file("~/Library/Application Support/com.mitchellh.ghostty/config", "font-family = Iosevka\n"),
        ));
        assert_eq!(both[0]["font"], json!("Hack"));
    }

    #[test]
    fn the_terminal_the_collector_runs_in_wins_over_the_others_configs() {
        let laptop = || {
            iterm_laptop()
                .file("~/.config/ghostty/config", "font-family = Hack\n")
                .file("~/.config/kitty/kitty.conf", "font_family Fira Code\n")
        };
        assert_eq!(rows_of(&fake_host(laptop())), json!([row("Hack", "terminal font: Hack (Ghostty)")]));
        assert_eq!(
            rows_of(&fake_host(laptop().terminal("iTerm.app"))),
            json!([row("JetBrainsMonoNF-Regular", "terminal font: JetBrainsMonoNF-Regular (iTerm2)")])
        );
        assert_eq!(rows_of(&fake_host(laptop().terminal("kitty"))), json!([row("Fira Code", "terminal font: Fira Code (kitty)")]));
    }

    #[test]
    fn iterm2s_plist_is_read_through_defaults_never_as_a_file_and_only_on_a_mac() {
        let host = fake_host(iterm_laptop());
        assert_eq!(rows_of(&host), json!([row("JetBrainsMonoNF-Regular", "terminal font: JetBrainsMonoNF-Regular (iTerm2)")]));
        assert!(
            !host.calls().contains(&"read /Users/dev/Library/Preferences/com.googlecode.iterm2.plist".to_string()),
            "{:?}",
            host.calls()
        );
        let linux = fake_host(iterm_laptop().platform(Platform::Linux));
        assert_eq!(rows_of(&linux), json!([]));
    }

    #[test]
    fn wezterm_kitty_and_alacritty_configs_each_name_the_row_after_their_terminal() {
        let one = |path: &str, text: &str| rows_of(&fake_host(FakeLaptop::new().file(path, text)));
        assert_eq!(
            one("~/.wezterm.lua", "config.font = wezterm.font(\"Fira Code\")\n"),
            json!([row("Fira Code", "terminal font: Fira Code (WezTerm)")])
        );
        assert_eq!(
            one("~/.config/wezterm/wezterm.lua", "return {}\n"),
            json!([row("JetBrains Mono", "terminal font: JetBrains Mono (WezTerm)")])
        );
        assert_eq!(
            one("~/.config/alacritty/alacritty.toml", "[font.normal]\nfamily = \"Hack\"\n"),
            json!([row("Hack", "terminal font: Hack (Alacritty)")])
        );
        assert_eq!(
            one("~/.alacritty.yml", "font:\n  normal:\n    family: Hack\n"),
            json!([row("Hack", "terminal font: Hack (Alacritty)")])
        );
    }

    #[test]
    fn a_terminal_whose_config_names_no_usable_font_is_passed_over_for_the_next_one() {
        // None known means no row.
        let laptop = FakeLaptop::new()
            .file("~/.config/kitty/kitty.conf", "font_size 13\n")
            .file("~/.config/alacritty/alacritty.toml", "[font.normal]\nfamily = \"Hack\"\n");
        assert_eq!(rows_of(&fake_host(laptop)), json!([row("Hack", "terminal font: Hack (Alacritty)")]));
        assert_eq!(rows_of(&fake_host(FakeLaptop::new().file("~/.zshrc", 10))), json!([]));
    }
}

/// Rules of spec/terminal.md the TS cases leave unpinned.
mod pins {
    use super::*;

    #[test]
    fn usable_strips_one_quote_at_each_end_and_drops_generic_names() {
        assert_eq!(kitty_font("font_family \"Fira Code'\n"), Some("Fira Code".into()));
        assert_eq!(kitty_font("font_family   \"  Hack  \"  \n"), Some("Hack".into()));
        assert_eq!(kitty_font("font_family 'MONO'\n"), None);
        assert_eq!(kitty_font("font_family \"\"\n"), None);
        assert_eq!(ghostty_font("font-family = Menlo\nfont-family = monospace\n"), None);
    }
}

/// Every case of tests/fixtures/oracle/w2/terminal.json (oracle/w2-terminal.oracle.ts): spec terminal's wave-2 rules.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, w2};
    use serde_json::{json, Value};
    use wsp_collect::detect::terminal::{alacritty_font, detect_terminal_font, ghostty_font, iterm_font, kitty_font, wezterm_font};

    #[test]
    fn every_config_parser_reads_the_font_the_ts_read() {
        let f = w2("terminal");
        let parsers: [(&str, fn(&str) -> Option<String>); 5] = [
            ("kitty_font", kitty_font),
            ("wezterm_font", wezterm_font),
            ("alacritty_font", alacritty_font),
            ("ghostty_font", ghostty_font),
            ("iterm_font", iterm_font),
        ];
        for (key, parse) in parsers {
            for case in cases(&f, key) {
                assert_eq!(json!(parse(case["args"][0].as_str().unwrap())), case["out"], "{key}: {}", case["name"]);
            }
        }
    }

    #[test]
    fn detect_terminal_font_answers_and_reads_what_the_ts_did() {
        for case in cases(&w2("terminal"), "detect_terminal_font") {
            let h = host(&case["laptop"]);
            let got: Vec<Value> = block_on(detect_terminal_font(&h)).iter().map(|r| serde_json::to_value(r).unwrap()).collect();
            assert_eq!(Value::Array(got), case["out"], "{}", case["name"]);
            if !case["calls"].is_null() {
                assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
            }
        }
    }
}
