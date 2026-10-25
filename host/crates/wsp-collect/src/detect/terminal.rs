// SPDX-License-Identifier: AGPL-3.0-only
//! The font the person's terminal draws with, read from its config, so the app's terminal pane can default to it.
//! Nothing here travels to the machine. Port of packages/collect/src/detect/terminal.ts at a3b870a6e; the spec is
//! spec/terminal.md.
use std::sync::LazyLock;

use fancy_regex::Regex;
use serde_json::Value;
use wsp_wire::views::project_bundle::TerminalScheme;
use wsp_words::js::{js_trim, JS_WS};
use wsp_words::regexp::escape_reg_exp;

use crate::detect::common::{entry, exists, EntrySpec, RowOptions};
use crate::detect::tools::argv;
use crate::ghostty_config::{parse_ghostty_directives, read_ghostty_config, GHOSTTY_BUILT_IN_FONT};
use crate::host::{expand, Host, Platform, RunOptions};
use crate::manifest::{ManifestEntry, RowDefault, Rung};

pub const TERMINAL_FONT_ID: &str = "shell/terminal-font";

const TERM: &str = r"\n\r\x{2028}\x{2029}";
const LINE_START: &str = r"(?:^|(?<=[\n\r\x{2028}\x{2029}]))";
const LINE_END: &str = r"(?:$|(?=[\n\r\x{2028}\x{2029}]))";

static ITERM_DEFAULT_GUID: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"<key>Default Bookmark Guid</key>[{JS_WS}]*<string>([^<]*)</string>")).expect("ITERM_DEFAULT_GUID compiles")
});
static ITERM_NORMAL_FONT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"<key>Normal Font</key>[{JS_WS}]*<string>([^<]*)</string>")).expect("ITERM_NORMAL_FONT compiles")
});
static ITERM_SIZE: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"[{JS_WS}]+[0-9.]+$")).expect("ITERM_SIZE compiles"));
static WEZ_BLOCK_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"--\[\[[\s\S]*?\]\]").expect("WEZ_BLOCK_COMMENT compiles"));
static WEZ_LINE_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"--[^{TERM}]*")).expect("WEZ_LINE_COMMENT compiles"));
static WEZ_FONT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"(?<![A-Za-z0-9_])font(?:_with_fallback)?[{JS_WS}]*\(?[{JS_WS}]*\{{?[{JS_WS}]*(?:\{{[{JS_WS}]*)?(?:family[{JS_WS}]*=[{JS_WS}]*)?(["'])([^"']+)\1"#
    ))
    .expect("WEZ_FONT compiles")
});
static KITTY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"{LINE_START}[{JS_WS}]*font_family[{JS_WS}]+([^{TERM}]+?)[{JS_WS}]*{LINE_END}")).expect("KITTY_LINE compiles")
});
static KITTY_KEYED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"(?<![A-Za-z0-9_])family[{JS_WS}]*=[{JS_WS}]*(?:"([^"]*)"|([^{JS_WS}]+))"#)).expect("KITTY_KEYED compiles")
});
static ALACRITTY_TOML_TABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"\[font\.normal\][^\[]*?{LINE_START}[{JS_WS}]*family[{JS_WS}]*=[{JS_WS}]*"([^"]+)""#))
        .expect("ALACRITTY_TOML_TABLE compiles")
});
static ALACRITTY_TOML_INLINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"{LINE_START}[{JS_WS}]*normal[{JS_WS}]*=[{JS_WS}]*\{{[^}}]*(?<![A-Za-z0-9_])family[{JS_WS}]*=[{JS_WS}]*"([^"]+)""#
    ))
    .expect("ALACRITTY_TOML_INLINE compiles")
});
static ALACRITTY_TOML_DOTTED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"{LINE_START}[{JS_WS}]*normal\.family[{JS_WS}]*=[{JS_WS}]*"([^"]+)""#)).expect("ALACRITTY_TOML_DOTTED compiles")
});
static ALACRITTY_YAML: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"{LINE_START}[{JS_WS}]*normal:[{JS_WS}]*\n(?:[{JS_WS}]+(?!family:)[A-Za-z0-9_]+:[^{TERM}]*\n)*[{JS_WS}]+family:[{JS_WS}]*([^{TERM}]+?)[{JS_WS}]*{LINE_END}"
    ))
    .expect("ALACRITTY_YAML compiles")
});

/// A family a config falls back to when it names none; not a font the pane can ask for.
const GENERIC: [&str; 3] = ["monospace", "mono", ""];

/// A family name with its quotes and surrounding space off, or `None` for a generic one (terminal.ts:14).
fn usable(family: Option<&str>) -> Option<String> {
    let quote = |c: char| c == '"' || c == '\'';
    let trimmed = js_trim(family?);
    let unquoted = trimmed.strip_prefix(quote).unwrap_or(trimmed);
    let name = js_trim(unquoted.strip_suffix(quote).unwrap_or(unquoted));
    (!GENERIC.contains(&name.to_lowercase().as_str())).then(|| name.to_string())
}

/// The first usable face of a font-family list; a generic word resets what came before it, as an empty line does.
fn primary_face(families: &[String]) -> Option<String> {
    let mut kept: Vec<String> = Vec::new();
    for family in families {
        match usable(Some(family.as_str())) {
            None => kept.clear(),
            Some(value) => kept.push(value),
        }
    }
    kept.into_iter().next()
}

fn group1(re: &Regex, text: &str) -> Option<String> {
    re.captures(text).ok().flatten().map(|c| c[1].to_string())
}

/// Ghostty: the first font-family line is the primary face; an empty one resets the list.
pub fn ghostty_font(text: &str) -> Option<String> {
    let families: Vec<String> = parse_ghostty_directives(text).into_iter().filter(|d| d.key == "font-family").map(|d| d.value).collect();
    primary_face(&families)
}

/// iTerm2: the default bookmark's Normal Font, else the first bookmark's; the value is "<font name> <size>".
/// A bookmark's keys are written in sorted order, so its Guid precedes its Normal Font and the next Guid ends it.
pub fn iterm_font(xml: &str) -> Option<String> {
    let at = group1(&ITERM_DEFAULT_GUID, xml).and_then(|guid| {
        let pattern = Regex::new(&format!(r"<key>Guid</key>[{JS_WS}]*<string>{}</string>", escape_reg_exp(&guid))).ok()?;
        pattern.find(xml).ok().flatten().map(|m| m.start())
    });
    let bookmark = match at {
        None => xml,
        Some(at) => {
            let rest = &xml[at..];
            rest[1..].find("<key>Guid</key>").map_or(rest, |next| &rest[..next + 1])
        }
    };
    let font = group1(&ITERM_NORMAL_FONT, bookmark).map(|f| ITERM_SIZE.replace(&f, "").into_owned());
    usable(font.as_deref())
}

/// With no usable family named, WezTerm's bundled font: this never answers `None`.
pub fn wezterm_font(lua: &str) -> Option<String> {
    let without_blocks = WEZ_BLOCK_COMMENT.replace_all(lua, "");
    let live = WEZ_LINE_COMMENT.replace_all(&without_blocks, "");
    let family = WEZ_FONT.captures(&live).ok().flatten().map(|c| c[2].to_string());
    Some(usable(family.as_deref()).unwrap_or_else(|| GHOSTTY_BUILT_IN_FONT.to_string()))
}

/// kitty: font_family, in the bare form or the family="..." style="..." form.
pub fn kitty_font(conf: &str) -> Option<String> {
    let value = group1(&KITTY_LINE, conf)?;
    let family = match KITTY_KEYED.captures(&value).ok().flatten() {
        Some(keyed) => keyed.get(1).or_else(|| keyed.get(2)).map(|g| g.as_str().to_string()),
        None => Some(value),
    };
    usable(family.as_deref())
}

/// Alacritty: font.normal.family in the toml forms or the older yaml.
pub fn alacritty_font(text: &str) -> Option<String> {
    let toml = [&*ALACRITTY_TOML_TABLE, &*ALACRITTY_TOML_INLINE, &*ALACRITTY_TOML_DOTTED].into_iter().find_map(|re| group1(re, text));
    if toml.is_some() {
        return usable(toml.as_deref());
    }
    usable(group1(&ALACRITTY_YAML, text).as_deref())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Ghostty,
    ITerm2,
    WezTerm,
    Kitty,
    Alacritty,
}

/// Every terminal, in table order.
const TERMINALS: [Kind; 5] = [Kind::Ghostty, Kind::ITerm2, Kind::WezTerm, Kind::Kitty, Kind::Alacritty];

fn name(kind: Kind) -> &'static str {
    match kind {
        Kind::Ghostty => "Ghostty",
        Kind::ITerm2 => "iTerm2",
        Kind::WezTerm => "WezTerm",
        Kind::Kitty => "kitty",
        Kind::Alacritty => "Alacritty",
    }
}

/// The TERM_PROGRAM word that names the terminal (a case-insensitive match, as the TS `/.../i` is).
fn program(kind: Kind) -> &'static str {
    match kind {
        Kind::Ghostty => "ghostty",
        Kind::ITerm2 => "iterm",
        Kind::WezTerm => "wezterm",
        Kind::Kitty => "kitty",
        Kind::Alacritty => "alacritty",
    }
}

fn built_in(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Ghostty | Kind::WezTerm => Some(GHOSTTY_BUILT_IN_FONT),
        Kind::ITerm2 | Kind::Kitty | Kind::Alacritty => None,
    }
}

async fn first_text(host: &Host, paths: &[&str]) -> Option<String> {
    for p in paths {
        if let Some(text) = host.fs.read_text(&expand(&host.home, p)).await {
            return Some(text);
        }
    }
    None
}

async fn configured(host: &Host, kind: Kind) -> Option<String> {
    match kind {
        Kind::Ghostty => {
            let config = read_ghostty_config(host, TerminalScheme::Dark).await;
            let files = config.get("files").and_then(Value::as_array).map_or(0, Vec::len);
            if files == 0 {
                return None;
            }
            let families: Vec<String> = config
                .get("fontFamily")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect();
            Some(primary_face(&families).unwrap_or_else(|| GHOSTTY_BUILT_IN_FONT.to_string()))
        }
        Kind::ITerm2 => {
            if !matches!(host.platform, Platform::Darwin) || !exists(host, "~/Library/Preferences/com.googlecode.iterm2.plist").await {
                return None;
            }
            let xml = host.exec.run("defaults", &argv(&["export", "com.googlecode.iterm2", "-"]), RunOptions::default()).await?;
            iterm_font(&xml)
        }
        Kind::WezTerm => {
            let lua = first_text(host, &["~/.wezterm.lua", "~/.config/wezterm/wezterm.lua"]).await?;
            wezterm_font(&lua)
        }
        Kind::Kitty => {
            let conf = first_text(host, &["~/.config/kitty/kitty.conf"]).await?;
            kitty_font(&conf)
        }
        Kind::Alacritty => {
            let text = first_text(
                host,
                &["~/.config/alacritty/alacritty.toml", "~/.alacritty.toml", "~/.config/alacritty/alacritty.yml", "~/.alacritty.yml"],
            )
            .await?;
            alacritty_font(&text)
        }
    }
}

/// Whether the terminal is on this computer, for a terminal with no config to answer its built-in default.
async fn installed(host: &Host, kind: Kind) -> bool {
    match kind {
        Kind::Ghostty => match host.platform {
            Platform::Darwin => exists(host, "/Applications/Ghostty.app").await,
            Platform::Linux => host.exec.which("ghostty").await,
        },
        _ => false,
    }
}

fn font_row(kind: Kind, font: &str) -> ManifestEntry {
    let mut row = entry(EntrySpec {
        rung: Rung::Shell,
        id: TERMINAL_FONT_ID.to_string(),
        label: format!("terminal font: {font} ({})", name(kind)),
        paths: Vec::new(),
        bytes: 0,
        opt: RowOptions { default: Some(RowDefault::Bring), ..RowOptions::default() },
    });
    row.font = Some(font.to_string());
    row
}

/// At most one row.
pub async fn detect_terminal_font(host: &Host) -> Vec<ManifestEntry> {
    let running = host.terminal.as_deref().map(str::to_ascii_lowercase).unwrap_or_default();
    let is_own = |kind: Kind| running.contains(program(kind));
    let own: Vec<Kind> = TERMINALS.into_iter().filter(|k| is_own(*k)).collect();
    let others: Vec<Kind> = TERMINALS.into_iter().filter(|k| !own.contains(k)).collect();
    for kind in &own {
        if let Some(font) = configured(host, *kind).await.or_else(|| built_in(*kind).map(str::to_string)) {
            return vec![font_row(*kind, &font)];
        }
    }
    for kind in &others {
        if let Some(font) = configured(host, *kind).await {
            return vec![font_row(*kind, &font)];
        }
    }
    for kind in &others {
        if let Some(font) = built_in(*kind) {
            if installed(host, *kind).await {
                return vec![font_row(*kind, font)];
            }
        }
    }
    Vec::new()
}
