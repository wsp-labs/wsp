// SPDX-License-Identifier: AGPL-3.0-only
//! The person's Ghostty config as the terminal pane takes it, read the way Ghostty's config reference says: config.ghostty
//! then config under XDG, then the same pair in the Mac's Application Support, a later file winning, each followed by
//! the files its config-file lines include; the last theme line resolved against the config's themes folder and the
//! bundled ones, its colors under the config's own. Port of packages/collect/src/ghostty-config.ts at a3b870a6e; the
//! spec is spec/ghostty_config.md.
use std::collections::{HashMap, HashSet};

use serde_json::{json, Map, Value};
use wsp_wire::types::js_number::JsInt;
use wsp_wire::views::project_bundle::{TerminalRgb, TerminalScheme};
use wsp_words::js::{js_trim, number_string, to_number};

use crate::host::{expand, Host, Platform};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhosttyDirective {
    pub key: String,
    pub value: String,
}

/// Ghostty ships JetBrains Mono and draws with it until a config names a font.
pub const GHOSTTY_BUILT_IN_FONT: &str = "JetBrains Mono";
pub const MAC_APP_SUPPORT: &str = "~/Library/Application Support/com.mitchellh.ghostty";
pub const MAC_BUNDLED_THEMES: &str = "/Applications/Ghostty.app/Contents/Resources/ghostty/themes";
pub const LINUX_SHARED_THEMES: [&str; 2] = ["/usr/local/share/ghostty/themes", "/usr/share/ghostty/themes"];
/// Ghostty's blur intensity for `background-blur = true`.
pub const DEFAULT_BLUR: i64 = 20;

const CURSOR_STYLES: [&str; 4] = ["block", "bar", "underline", "block_hollow"];

/// The key = value lines of one file, in order and with repeats; comments, blanks and lines with no = are not
/// directives.
pub fn parse_ghostty_directives(text: &str) -> Vec<GhosttyDirective> {
    let mut out = Vec::new();
    for raw in text.split('\n') {
        let line = js_trim(raw.strip_prefix('\u{FEFF}').unwrap_or(raw));
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(at) = line.find('=') else {
            continue;
        };
        let key = js_trim(&line[..at]);
        if key.is_empty() {
            continue;
        }
        out.push(GhosttyDirective { key: key.to_string(), value: unquote(js_trim(&line[at + 1..])).to_string() });
    }
    out
}

/// `replace(/^"(.*)"$/, "$1")`: one enclosing pair of quotes, with no line break inside.
fn unquote(value: &str) -> &str {
    match value.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) if !inner.contains(['\r', '\u{2028}', '\u{2029}']) => inner,
        _ => value,
    }
}

fn xdg_config_dir(host: &Host) -> String {
    host.xdg_config_home.clone().unwrap_or_else(|| format!("{}/.config", host.home))
}

/// Where Ghostty looks for its config on this computer, in load order.
pub fn ghostty_config_paths(host: &Host) -> Vec<String> {
    let xdg = format!("{}/ghostty", xdg_config_dir(host));
    let mut paths = vec![format!("{xdg}/config.ghostty"), format!("{xdg}/config")];
    if host.platform == Platform::Darwin {
        paths.push(expand(&host.home, &format!("{MAC_APP_SUPPORT}/config.ghostty")));
        paths.push(expand(&host.home, &format!("{MAC_APP_SUPPORT}/config")));
    }
    paths
}

/// `#RRGGBB` or `RRGGBB`, surrounding whitespace allowed, any case.
pub fn hex_color(value: &str) -> Option<TerminalRgb> {
    let t = js_trim(value);
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let part = |at: usize| i64::from_str_radix(&hex[at..at + 2], 16).ok();
    Some(TerminalRgb { r: JsInt::from(part(0)?), g: JsInt::from(part(2)?), b: JsInt::from(part(4)?) })
}

/// The theme a `theme` value names for the scheme: the one name, or the light:/dark: pair's side.
pub fn theme_for(value: &str, scheme: TerminalScheme) -> String {
    let mut sides: HashMap<&str, &str> = HashMap::new();
    for part in value.split(',') {
        if let Some(at) = part.find(':').filter(|&at| at > 0) {
            sides.insert(js_trim(&part[..at]), js_trim(&part[at + 1..]));
        }
    }
    let name = match scheme {
        TerminalScheme::Light => "light",
        TerminalScheme::Dark => "dark",
    };
    match sides.get(name) {
        Some(side) => side.to_string(),
        None if sides.is_empty() => js_trim(value).to_string(),
        None => String::new(),
    }
}

/// Where a theme name is looked for, in order: the config's own themes folder, then the bundled ones.
fn theme_paths(host: &Host, name: &str) -> Vec<String> {
    if name.starts_with('/') {
        return vec![name.to_string()];
    }
    let mut dirs = vec![format!("{}/ghostty/themes", xdg_config_dir(host))];
    if host.platform == Platform::Darwin {
        dirs.push(MAC_BUNDLED_THEMES.to_string());
    } else {
        dirs.extend(LINUX_SHARED_THEMES.iter().map(|dir| dir.to_string()));
    }
    dirs.into_iter().map(|dir| format!("{dir}/{name}")).collect()
}

fn dir_of(path: &str) -> &str {
    match path.rfind('/') {
        Some(at) if at > 0 => &path[..at],
        _ => "/",
    }
}

fn resolve_include(value: &str, from: &str, home: &str) -> Option<String> {
    let path = unquote(value.strip_prefix('?').unwrap_or(value));
    if path.is_empty() {
        return None;
    }
    let expanded = expand(home, path);
    Some(if expanded.starts_with('/') { expanded } else { format!("{}/{expanded}", dir_of(from)) })
}

fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    format!("/{}", out.join("/"))
}

/// Config files in load order, each once, with the files each includes after it; an unreadable file is skipped.
async fn load_with_includes(host: &Host, paths: &[String], files: &mut Vec<String>, into: &mut Vec<GhosttyDirective>) {
    let mut seen: HashSet<String> = HashSet::new();
    let mut stack: Vec<String> = paths.iter().rev().cloned().collect();
    while let Some(path) = stack.pop() {
        let own = normalize(&path);
        if seen.contains(&own) {
            continue;
        }
        let Some(text) = host.fs.read_text(&own).await else {
            continue;
        };
        seen.insert(own.clone());
        files.push(own.clone());
        let directives = parse_ghostty_directives(&text);
        let mut includes: Vec<String> = Vec::new();
        for directive in &directives {
            if directive.key != "config-file" {
                continue;
            }
            if directive.value.is_empty() {
                includes.clear();
            } else if let Some(resolved) = resolve_include(&directive.value, &own, &host.home) {
                includes.push(resolved);
            }
        }
        into.extend(directives);
        stack.extend(includes.into_iter().rev());
    }
}

/// `Number(value.trim())` when the trimmed value is not empty and the result is finite.
fn finite(value: &str) -> Option<f64> {
    let t = js_trim(value);
    if t.is_empty() {
        return None;
    }
    let n = to_number(t);
    n.is_finite().then_some(n)
}

/// `N=COLOR` with N an integer from 0 to 15 as `Number` reads it, and a hex colour.
fn palette_entry(value: &str) -> Option<(usize, TerminalRgb)> {
    let at = value.find('=')?;
    let index = to_number(&value[..at]);
    let color = hex_color(&value[at + 1..])?;
    (index.is_finite() && index.fract() == 0.0 && (0.0..=15.0).contains(&index)).then_some((index as usize, color))
}

/// One or two non-negative numbers, comma separated; one value sets both sides.
fn padding(value: &str) -> Option<(f64, f64)> {
    let parts: Vec<Option<f64>> = value.split(',').map(finite).collect();
    let a = parts[0]?;
    if parts.len() > 2 || a < 0.0 {
        return None;
    }
    let b = if parts.len() == 2 {
        let b = parts[1]?;
        if b < 0.0 {
            return None;
        }
        b
    } else {
        a
    };
    Some((a, b))
}

fn blur(value: &str) -> Option<f64> {
    let word = js_trim(value).to_lowercase();
    if word == "false" {
        return Some(0.0);
    }
    if word == "true" || word.starts_with("macos-glass-") {
        return Some(DEFAULT_BLUR as f64);
    }
    let n = finite(&word)?;
    (n.fract() == 0.0 && n >= 0.0).then_some(n)
}

/// A number as JavaScript's JSON writes it: no `.0` on an integer, and digits kept where serde_json can hold them.
fn js_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9007199254740992.0 {
        Value::from(n as i64)
    } else if n.fract() == 0.0 && n.abs() < 1e21 {
        serde_json::from_str(&number_string(n)).unwrap_or(Value::from(n))
    } else {
        Value::from(n)
    }
}

fn rgb_value(color: TerminalRgb) -> Value {
    serde_json::to_value(color).unwrap_or(Value::Null)
}

/// The keys the pane honours, applied in order over the answer's map; a value that does not parse leaves its key.
fn apply(out: &mut Map<String, Value>, directive: &GhosttyDirective) {
    let value = directive.value.as_str();
    match directive.key.as_str() {
        "font-family" => {
            if value.is_empty() {
                out.insert("fontFamily".into(), json!([]));
            } else if let Some(Value::Array(list)) = out.get_mut("fontFamily") {
                list.push(Value::String(value.to_string()));
            }
        }
        "font-size" => {
            if let Some(n) = finite(value).filter(|n| *n > 0.0) {
                out.insert("fontSize".into(), js_number(n));
            }
        }
        "background" | "foreground" | "selection-background" | "cursor-color" => {
            if let Some(color) = hex_color(value) {
                let name = match directive.key.as_str() {
                    "background" => "background",
                    "foreground" => "foreground",
                    "selection-background" => "selectionBackground",
                    _ => "cursorColor",
                };
                out.insert(name.into(), rgb_value(color));
            }
        }
        "palette" => {
            if let Some((index, color)) = palette_entry(value) {
                if let Some(Value::Array(palette)) = out.get_mut("palette") {
                    palette[index] = rgb_value(color);
                }
            }
        }
        "cursor-style" => {
            if CURSOR_STYLES.contains(&value) {
                out.insert("cursorStyle".into(), json!(value));
            }
        }
        "cursor-style-blink" => match value {
            "true" => {
                out.insert("cursorStyleBlink".into(), json!(true));
            }
            "false" => {
                out.insert("cursorStyleBlink".into(), json!(false));
            }
            "" => {
                out.shift_remove("cursorStyleBlink");
            }
            _ => {}
        },
        "window-padding-x" => {
            if let Some((left, right)) = padding(value) {
                out.insert("windowPaddingX".into(), json!({ "left": js_number(left), "right": js_number(right) }));
            }
        }
        "window-padding-y" => {
            if let Some((top, bottom)) = padding(value) {
                out.insert("windowPaddingY".into(), json!({ "top": js_number(top), "bottom": js_number(bottom) }));
            }
        }
        "background-opacity" => {
            if let Some(n) = finite(value) {
                out.insert("backgroundOpacity".into(), js_number(n.clamp(0.0, 1.0)));
            }
        }
        "background-blur" | "background-blur-radius" => {
            if let Some(n) = blur(value) {
                out.insert("backgroundBlur".into(), js_number(n));
            }
        }
        _ => {}
    }
}

/// The person's Ghostty config as the pane applies it; with no file, files and fontFamily empty, sixteen `None`
/// palette slots and every other key absent.
/// The answer is an ordered JSON map in the TS object's key order (coordinator's ruling on #2446): `files`,
/// `fontFamily`, `palette`, then each applied key in the order it was first set (theme file first, then the config),
/// a key that `cursor-style-blink =` deleted going to the end when set again, and `theme` last. Its keys and values
/// are wsp-wire's `TerminalConfig` fields in camelCase, so `serde_json::from_value` reads it into that struct.
pub async fn read_ghostty_config(host: &Host, scheme: TerminalScheme) -> serde_json::Map<String, serde_json::Value> {
    let mut files: Vec<String> = Vec::new();
    let mut own: Vec<GhosttyDirective> = Vec::new();
    load_with_includes(host, &ghostty_config_paths(host), &mut files, &mut own).await;
    let theme = own.iter().rev().find(|d| d.key == "theme").map(|d| theme_for(&d.value, scheme)).filter(|t| !t.is_empty());
    let mut out: Map<String, Value> = Map::new();
    out.insert("fontFamily".into(), json!([]));
    out.insert("palette".into(), Value::Array(vec![Value::Null; 16]));
    if let Some(name) = &theme {
        for path in theme_paths(host, name) {
            let Some(text) = host.fs.read_text(&path).await else {
                continue;
            };
            files.push(path);
            for directive in parse_ghostty_directives(&text) {
                if directive.key != "theme" && directive.key != "config-file" {
                    apply(&mut out, &directive);
                }
            }
            break;
        }
    }
    for directive in &own {
        apply(&mut out, directive);
    }
    let mut answer: Map<String, Value> = Map::new();
    answer.insert("files".into(), json!(files));
    for (key, value) in out {
        answer.insert(key, value);
    }
    if let Some(theme) = theme {
        answer.insert("theme".into(), json!(theme));
    }
    answer
}
