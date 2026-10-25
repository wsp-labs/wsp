// SPDX-License-Identifier: AGPL-3.0-only
//! The person's Ghostty config as the terminal pane takes it: Ghostty's own lookup order and include rules over a
//! laptop described as data, the theme resolved against the theme directories and the bundled themes, and only the
//! keys the pane honours in the answer. Port of packages/collect/test/ghostty-config.test.ts.
mod common;

use common::fake_host::*;
use wsp_collect::detect::terminal::ghostty_font;
use wsp_collect::ghostty_config::{parse_ghostty_directives, read_ghostty_config, GhosttyDirective};
use wsp_collect::host::Platform;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::project_bundle::{
    TerminalConfig, TerminalConfigWindowPaddingX, TerminalConfigWindowPaddingY, TerminalCursorStyle, TerminalRgb, TerminalScheme,
};

fn rgb(hex: &str) -> TerminalRgb {
    let part = |at: usize| i64::from_str_radix(&hex[at..at + 2], 16).unwrap();
    TerminalRgb { r: part(0), g: part(2), b: part(4) }
}

fn none() -> Vec<Option<TerminalRgb>> {
    vec![None; 16]
}

/// The config with nothing set but its files: no font, sixteen unset palette slots, no other key.
fn bare(files: &[&str]) -> TerminalConfig {
    TerminalConfig {
        files: files.iter().map(|f| f.to_string()).collect(),
        font_family: vec![],
        font_size: None,
        theme: None,
        background: None,
        foreground: None,
        palette: none(),
        selection_background: None,
        cursor_color: None,
        cursor_style: None,
        cursor_style_blink: None,
        window_padding_x: None,
        window_padding_y: None,
        background_opacity: None,
        background_blur: None,
    }
}

fn num(n: f64) -> Option<JsNumber> {
    Some(JsNumber(n))
}

fn read(host: &FakeHost) -> TerminalConfig {
    read_as(host, TerminalScheme::Dark)
}

fn read_as(host: &FakeHost, scheme: TerminalScheme) -> TerminalConfig {
    serde_json::from_value(serde_json::Value::Object(block_on(read_ghostty_config(&host.host, scheme)))).expect("a TerminalConfig")
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// Catppuccin Mocha as Ghostty ships it, word for word.
const MOCHA: &str = "palette = 0=#45475a
palette = 1=#f38ba8
palette = 2=#a6e3a1
palette = 3=#f9e2af
palette = 4=#89b4fa
palette = 5=#f5c2e7
palette = 6=#94e2d5
palette = 7=#a6adc8
palette = 8=#585b70
palette = 9=#f37799
palette = 10=#89d88b
palette = 11=#ebd391
palette = 12=#74a8fc
palette = 13=#f2aede
palette = 14=#6bd7ca
palette = 15=#bac2de
background = #1e1e2e
foreground = #cdd6f4
cursor-color = #f5e0dc
cursor-text = #1e1e2e
selection-background = #585b70
selection-foreground = #cdd6f4
";
const BUNDLED: &str = "/Applications/Ghostty.app/Contents/Resources/ghostty/themes";

mod parse_ghostty_directives_cases {
    use super::*;

    #[test]
    fn reads_key_value_lines_in_order_keeps_repeats_strips_quotes_skips_blanks_and_comm() {
        let directive = |key: &str, value: &str| GhosttyDirective { key: key.to_string(), value: value.to_string() };
        assert_eq!(
            parse_ghostty_directives(
                "# a comment\n\nfont-family = \"Berkeley Mono\"\nfont-family = Symbols Nerd Font\n  theme=Catppuccin Mocha  \nbackground = #1e1e2e\nnot-a-line\n"
            ),
            vec![
                directive("font-family", "Berkeley Mono"),
                directive("font-family", "Symbols Nerd Font"),
                directive("theme", "Catppuccin Mocha"),
                directive("background", "#1e1e2e"),
            ]
        );
    }
}

mod read_ghostty_config_cases {
    use super::*;

    #[test]
    fn with_no_config_file_answers_the_empty_object_no_files_no_font_sixteen_unset_pale() {
        let host = fake_host(FakeLaptop::new().file("~/.zshrc", 10));
        assert_eq!(read(&host), bare(&[]));
    }

    #[test]
    fn takes_every_key_the_pane_honours_from_a_real_config_ignores_the_ones_it_does_not() {
        let config = "theme = Catppuccin Mocha
font-family = \"Berkeley Mono\"
font-family = Symbols Nerd Font Mono
font-size = 13.5
background = #000000
palette = 0x1=#ff0000
palette = 16=#123456
cursor-color = cell-foreground
cursor-style = underline
cursor-style-blink = false
window-padding-x = 2,4
window-padding-y = 6
background-opacity = 0.85
background-blur-radius = 20
selection-foreground = navy
keybind = ctrl+a=copy_to_clipboard
mouse-hide-while-typing = true
";
        let host =
            fake_host(FakeLaptop::new().file("~/.config/ghostty/config", config).file(&format!("{BUNDLED}/Catppuccin Mocha"), MOCHA));
        let mut palette = vec![Some(rgb("45475a")), Some(rgb("ff0000"))];
        // 0x1 is slot one in Ghostty's own notation, so the theme's slot zero stands and its slot one is replaced.
        palette.extend(
            [
                "a6e3a1", "f9e2af", "89b4fa", "f5c2e7", "94e2d5", "a6adc8", "585b70", "f37799", "89d88b", "ebd391", "74a8fc", "f2aede",
                "6bd7ca", "bac2de",
            ]
            .iter()
            .map(|h| Some(rgb(h))),
        );
        assert_eq!(
            read(&host),
            TerminalConfig {
                files: vec!["/Users/dev/.config/ghostty/config".to_string(), format!("{BUNDLED}/Catppuccin Mocha")],
                font_family: strings(&["Berkeley Mono", "Symbols Nerd Font Mono"]),
                font_size: num(13.5),
                theme: Some("Catppuccin Mocha".to_string()),
                background: Some(rgb("000000")),
                foreground: Some(rgb("cdd6f4")),
                palette,
                selection_background: Some(rgb("585b70")),
                // cell-foreground is a color the pane cannot resolve, so the theme's cursor stands.
                cursor_color: Some(rgb("f5e0dc")),
                cursor_style: Some(TerminalCursorStyle::Underline),
                cursor_style_blink: Some(false),
                window_padding_x: Some(TerminalConfigWindowPaddingX { left: JsNumber(2.0), right: JsNumber(4.0) }),
                window_padding_y: Some(TerminalConfigWindowPaddingY { top: JsNumber(6.0), bottom: JsNumber(6.0) }),
                background_opacity: num(0.85),
                background_blur: Some(20),
            }
        );
    }

    #[test]
    fn an_empty_font_family_resets_the_list_a_bare_hex_color_and_true_for_the_blur_are() {
        let config = "font-family = Menlo\nfont-family = \"\"\nfont-family = Iosevka\nforeground = CDD6F4\nbackground-blur = true\nbackground-opacity = lots\ncursor-style = wobble\nfont-size = big\n";
        let host = fake_host(FakeLaptop::new().file("~/.config/ghostty/config", config));
        assert_eq!(
            read(&host),
            TerminalConfig {
                font_family: strings(&["Iosevka"]),
                foreground: Some(rgb("cdd6f4")),
                background_blur: Some(20),
                ..bare(&["/Users/dev/.config/ghostty/config"])
            }
        );
    }

    #[test]
    fn follows_config_file_includes_after_the_file_that_names_them_relative_to_it_an_op() {
        let host = fake_host(
            FakeLaptop::new()
                .file("~/.config/ghostty/config", "config-file = colors\nconfig-file = ?absent\nfont-size = 12\n")
                .file("~/.config/ghostty/colors", "font-size = 15\nconfig-file = ../ghostty/config\nbackground = #101010\n"),
        );
        assert_eq!(
            read(&host),
            TerminalConfig {
                font_size: num(15.0),
                background: Some(rgb("101010")),
                ..bare(&["/Users/dev/.config/ghostty/config", "/Users/dev/.config/ghostty/colors"])
            }
        );
    }

    #[test]
    fn loads_config_ghostty_then_config_under_xdg_then_the_application_support_pair_the() {
        // Ghostty's config reference (ghostty.org/docs/config, File Location) lists config.ghostty before config in each
        // folder and says conflicting values in later files override earlier ones, so a plain config beside a renamed one
        // wins, and the Mac's Application Support pair wins over the XDG pair.
        let laptop = || {
            FakeLaptop::new()
                .file("~/.config/ghostty/config.ghostty", "font-size = 20\nbackground = #222222\n")
                .file("~/.config/ghostty/config", "font-size = 10\nbackground = #111111\n")
                .file("~/Library/Application Support/com.mitchellh.ghostty/config.ghostty", "font-size = 13\n")
                .file("~/Library/Application Support/com.mitchellh.ghostty/config", "font-size = 12\n")
        };
        let mac = read(&fake_host(laptop()));
        assert_eq!(
            mac.files,
            strings(&[
                "/Users/dev/.config/ghostty/config.ghostty",
                "/Users/dev/.config/ghostty/config",
                "/Users/dev/Library/Application Support/com.mitchellh.ghostty/config.ghostty",
                "/Users/dev/Library/Application Support/com.mitchellh.ghostty/config",
            ])
        );
        assert_eq!(mac.font_size, num(12.0));
        assert_eq!(mac.background, Some(rgb("111111")));

        let linux = read(&fake_host(laptop().platform(Platform::Linux)));
        assert_eq!(linux.files, strings(&["/Users/dev/.config/ghostty/config.ghostty", "/Users/dev/.config/ghostty/config"]));
        assert_eq!(linux.font_size, num(10.0));

        let xdg = read(&fake_host(
            FakeLaptop::new()
                .xdg_config_home("/Users/dev/cfg")
                .file("/Users/dev/cfg/ghostty/config", "font-size = 9\n")
                .file("~/.config/ghostty/config", "font-size = 11\n"),
        ));
        assert_eq!(xdg.files, strings(&["/Users/dev/cfg/ghostty/config"]));
        assert_eq!(xdg.font_size, num(9.0));
    }

    #[test]
    fn resolves_a_theme_against_the_config_s_themes_folder_before_the_bundled_ones_take() {
        let laptop = || {
            FakeLaptop::new()
                .file("~/.config/ghostty/config", "theme = light:Mine,dark:Catppuccin Mocha\n")
                .file("~/.config/ghostty/themes/Mine", "background = #fafafa\ntheme = Catppuccin Mocha\nconfig-file = ../config\n")
                .file(&format!("{BUNDLED}/Mine"), "background = #bad000\n")
                .file(&format!("{BUNDLED}/Catppuccin Mocha"), MOCHA)
                .file("/tmp/own", "foreground = #abcdef\n")
        };
        let light = read_as(&fake_host(laptop()), TerminalScheme::Light);
        assert_eq!(light.files, strings(&["/Users/dev/.config/ghostty/config", "/Users/dev/.config/ghostty/themes/Mine"]));
        assert_eq!(light.theme.as_deref(), Some("Mine"));
        assert_eq!(light.background, Some(rgb("fafafa")));

        let dark = read_as(&fake_host(laptop()), TerminalScheme::Dark);
        assert_eq!(dark.theme.as_deref(), Some("Catppuccin Mocha"));
        assert_eq!(dark.background, Some(rgb("1e1e2e")));

        assert_eq!(read(&fake_host(laptop())).theme.as_deref(), Some("Catppuccin Mocha"));

        let absolute = read(&fake_host(laptop().file("~/.config/ghostty/config", "theme = /tmp/own\n")));
        assert_eq!(absolute.files, strings(&["/Users/dev/.config/ghostty/config", "/tmp/own"]));
        assert_eq!(absolute.theme.as_deref(), Some("/tmp/own"));
        assert_eq!(absolute.foreground, Some(rgb("abcdef")));

        // Ghostty looks in the config's themes folder and the bundled ones only; a theme left in Application Support is not found.
        let app_support_only = fake_host(
            FakeLaptop::new()
                .file("~/.config/ghostty/config", "theme = Stray\n")
                .file("~/Library/Application Support/com.mitchellh.ghostty/themes/Stray", "background = #010203\n"),
        );
        assert_eq!(
            read(&app_support_only),
            TerminalConfig { theme: Some("Stray".to_string()), ..bare(&["/Users/dev/.config/ghostty/config"]) }
        );

        // A theme nobody has is no theme: the name is kept so the person can see what was asked for.
        let missing = fake_host(FakeLaptop::new().file("~/.config/ghostty/config", "theme = Nope\n"));
        assert_eq!(read(&missing), TerminalConfig { theme: Some("Nope".to_string()), ..bare(&["/Users/dev/.config/ghostty/config"]) });
    }

    #[test]
    fn is_the_one_ghostty_reader_the_terminal_font_detector_s_primary_face_comes_from_t() {
        assert_eq!(ghostty_font("font-family = \"Berkeley Mono\"\nfont-family = Symbols Nerd Font\n"), Some("Berkeley Mono".to_string()));
    }
}

/// The coordinator's ruling on #2446: the answer keeps the TS object's key order (spec ghostty_config rule 8).
/// Expected text printed by the TS with JSON.stringify.
mod key_order {
    use super::*;

    fn text(files: &[(&str, &str)]) -> String {
        let laptop = files.iter().fold(FakeLaptop::new(), |l, (p, t)| l.file(p, *t));
        serde_json::to_string(&block_on(read_ghostty_config(&fake_host(laptop), TerminalScheme::Dark))).unwrap()
    }

    const NULLS: &str = "[null,null,null,null,null,null,null,null,null,null,null,null,null,null,null,null]";

    #[test]
    fn a_blink_deleted_and_set_again_moves_after_the_keys_set_meanwhile() {
        let got = text(&[("~/.config/ghostty/config", "cursor-style-blink = true\nfont-size = 12\ncursor-style-blink =\ncursor-style-blink = false\n")]);
        let want = format!(r#"{{"files":["/Users/dev/.config/ghostty/config"],"fontFamily":[],"palette":{NULLS},"fontSize":12,"cursorStyleBlink":false}}"#);
        assert_eq!(got, want);
    }

    #[test]
    fn the_theme_files_keys_come_first_and_theme_comes_last() {
        let got = text(&[
            ("~/.config/ghostty/config", "font-size = 13\ntheme = dark:Mine,light:Other\nbackground = #000000\n"),
            ("~/.config/ghostty/themes/Mine", "foreground = #ffffff\nbackground = #111111\npalette = 2=#222222\n"),
        ]);
        let want = concat!(
            r#"{"files":["/Users/dev/.config/ghostty/config","/Users/dev/.config/ghostty/themes/Mine"],"fontFamily":[],"#,
            r#""palette":[null,null,{"r":34,"g":34,"b":34},null,null,null,null,null,null,null,null,null,null,null,null,null],"#,
            r#""foreground":{"r":255,"g":255,"b":255},"background":{"r":0,"g":0,"b":0},"fontSize":13,"theme":"Mine"}"#
        );
        assert_eq!(got, want);
    }

    #[test]
    fn numbers_are_written_as_javascript_writes_them() {
        let got = text(&[("~/.config/ghostty/config", "window-padding-y = 2\nfont-family =\nbackground-opacity = 0.5\n")]);
        let want = format!(r#"{{"files":["/Users/dev/.config/ghostty/config"],"fontFamily":[],"palette":{NULLS},"windowPaddingY":{{"top":2,"bottom":2}},"backgroundOpacity":0.5}}"#);
        assert_eq!(got, want);
    }
}

/// Every case of tests/fixtures/oracle/w2/ghostty_config.json (oracle/w2-ghostty_config.oracle.ts): spec
/// ghostty_config's wave-2 rules. The config is compared as JSON text, so key order and number text count.
mod w2_oracle {
    use super::*;
    use common::oracle::{cases, host, w2};
    use serde_json::{json, Value};

    fn scheme(v: &Value) -> TerminalScheme {
        if v == "light" {
            TerminalScheme::Light
        } else {
            TerminalScheme::Dark
        }
    }

    #[test]
    fn directives_themes_and_paths_answer_what_the_ts_answered() {
        let f = w2("ghostty_config");
        for case in cases(&f, "parse_ghostty_directives") {
            let got: Vec<Value> =
                parse_ghostty_directives(case["args"][0].as_str().unwrap()).iter().map(|d| json!({ "key": d.key, "value": d.value })).collect();
            assert_eq!(Value::Array(got), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "theme_for") {
            let got = wsp_collect::ghostty_config::theme_for(case["args"][0].as_str().unwrap(), scheme(&case["args"][1]));
            assert_eq!(json!(got), case["out"], "{}", case["name"]);
        }
        for case in cases(&f, "ghostty_config_paths") {
            let h = host(&case["laptop"]);
            assert_eq!(json!(wsp_collect::ghostty_config::ghostty_config_paths(&h)), case["out"], "{}", case["name"]);
        }
    }

    #[test]
    fn every_config_reads_to_the_ts_answer_byte_for_byte_with_the_same_reads() {
        for case in cases(&w2("ghostty_config"), "read_ghostty_config") {
            let h = host(&case["laptop"]);
            let got = block_on(read_ghostty_config(&h, scheme(&case["args"][0])));
            assert_eq!(serde_json::to_string(&got).unwrap(), case["out_text"].as_str().unwrap(), "{}", case["name"]);
            assert_eq!(json!(h.calls()), case["calls"], "calls of {}", case["name"]);
        }
    }
}
