// SPDX-License-Identifier: AGPL-3.0-only
//! The rules Keybindings draws and checks a new chord against: the defaults (`keybindingDefaults.ts`), the person's
//! own chords over them (`keybindingOverrides.ts`) and the platform's spelling of a chord (`keybindings.ts`).

use std::collections::BTreeSet;

use gpui_kit::Keystroke;
use serde_json::{Map, Value};

use super::words;

/// One rule as a config holds it: the chord, the command, and when it may fire.
#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub key: String,
    pub command: String,
    pub when: Option<&'static str>,
}

/// One rule of `DEFAULT_KEYBINDINGS`.
pub struct DefaultRule {
    pub key: &'static str,
    pub command: &'static str,
    pub when: Option<&'static str>,
}

const fn d(key: &'static str, command: &'static str, when: Option<&'static str>) -> DefaultRule {
    DefaultRule { key, command, when }
}

const NOT_TERMINAL: Option<&str> = Some("!terminalFocus");
const NOT_OWNS_MOD: Option<&str> = Some("!terminalOwnsMod");
const TERMINAL: Option<&str> = Some("terminalFocus");
const FIND_OPEN: Option<&str> = Some("threadFindOpen && !terminalFocus");

/// `DEFAULT_KEYBINDINGS`, in its order.
pub const DEFAULTS: [DefaultRule; 43] = [
    d("mod+shift+b", "sidebar.toggle", None),
    d("mod+b", "sidebar.toggle", NOT_OWNS_MOD),
    d("mod+j", "terminal.toggle", None),
    d("mod+alt+b", "rightPanel.toggle", None),
    d("mod+d", "terminal.split", TERMINAL),
    d("mod+n", "terminal.new", TERMINAL),
    d("mod+=", "terminal.zoomIn", TERMINAL),
    d("mod+shift+=", "terminal.zoomIn", TERMINAL),
    d("mod+-", "terminal.zoomOut", TERMINAL),
    d("mod+0", "terminal.zoomReset", TERMINAL),
    d("mod+shift+j", "preview.toggle", None),
    d("mod+shift+p", "commandPalette.toggle", None),
    d("mod+k", "commandPalette.toggle", NOT_OWNS_MOD),
    d("mod+p", "files.quickOpen", NOT_OWNS_MOD),
    d("mod+shift+f", "files.search", None),
    d("mod+,", "settings.toggle", None),
    d("mod+t", "chat.new", NOT_TERMINAL),
    d("mod+alt+arrowleft", "workspace.previous", NOT_TERMINAL),
    d("mod+alt+arrowright", "workspace.next", NOT_TERMINAL),
    d("mod+alt+arrowup", "thread.previous", NOT_TERMINAL),
    d("mod+alt+arrowdown", "thread.next", NOT_TERMINAL),
    d("mod+shift+e", "thread.settle", NOT_TERMINAL),
    d("mod+alt+u", "thread.nextNeedsYou", NOT_TERMINAL),
    d("mod+o", "editor.open", NOT_OWNS_MOD),
    d("mod+f", "thread.find", Some("threadOpen && !terminalFocus && !previewFocus")),
    d("mod+g", "thread.findOlder", FIND_OPEN),
    d("f3", "thread.findOlder", FIND_OPEN),
    d("mod+shift+g", "thread.findNewer", FIND_OPEN),
    d("shift+f3", "thread.findNewer", FIND_OPEN),
    d("ctrl+tab", "workspace.next", Some("!terminalFocus && !panelTabsFocus")),
    d("ctrl+shift+tab", "workspace.previous", Some("!terminalFocus && !panelTabsFocus")),
    d("ctrl+tab", "rightPanel.nextTab", Some("panelTabsFocus")),
    d("ctrl+shift+tab", "rightPanel.previousTab", Some("panelTabsFocus")),
    d("mod+w", "rightPanel.closeTab", Some("previewFocus")),
    d("mod+1", "workspace.select.1", None),
    d("mod+2", "workspace.select.2", None),
    d("mod+3", "workspace.select.3", None),
    d("mod+4", "workspace.select.4", None),
    d("mod+5", "workspace.select.5", None),
    d("mod+6", "workspace.select.6", None),
    d("mod+7", "workspace.select.7", None),
    d("mod+8", "workspace.select.8", None),
    d("mod+9", "workspace.select.9", None),
];

/// `KEYBINDING_COMMANDS`: the commands this build dispatches.
pub const COMMANDS: [&str; 36] = [
    "sidebar.toggle",
    "terminal.toggle",
    "terminal.split",
    "terminal.new",
    "terminal.zoomIn",
    "terminal.zoomOut",
    "terminal.zoomReset",
    "rightPanel.toggle",
    "rightPanel.nextTab",
    "rightPanel.previousTab",
    "rightPanel.closeTab",
    "preview.toggle",
    "commandPalette.toggle",
    "files.quickOpen",
    "files.search",
    "settings.toggle",
    "chat.new",
    "workspace.next",
    "workspace.previous",
    "thread.next",
    "thread.previous",
    "thread.settle",
    "thread.nextNeedsYou",
    "thread.find",
    "thread.findOlder",
    "thread.findNewer",
    "editor.open",
    "workspace.select.1",
    "workspace.select.2",
    "workspace.select.3",
    "workspace.select.4",
    "workspace.select.5",
    "workspace.select.6",
    "workspace.select.7",
    "workspace.select.8",
    "workspace.select.9",
];

/// A chord parsed: its key and the modifiers it holds, `mod` being Command on macOS and Control elsewhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Shortcut {
    pub key: String,
    pub meta: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub modk: bool,
}

/// `parseKeybindingShortcut`.
pub fn parse(value: &str) -> Option<Shortcut> {
    let lower = value.to_lowercase();
    let mut tokens: Vec<&str> = lower.split('+').map(str::trim).collect();
    let mut trailing = 0;
    while tokens.last() == Some(&"") {
        trailing += 1;
        tokens.pop();
    }
    if trailing > 0 {
        tokens.push("+");
    }
    if tokens.is_empty() || tokens.iter().any(|t| t.is_empty()) {
        return None;
    }
    let mut s = Shortcut { key: String::new(), meta: false, ctrl: false, shift: false, alt: false, modk: false };
    let mut key = None;
    for token in tokens {
        match token {
            "cmd" | "meta" => s.meta = true,
            "ctrl" | "control" => s.ctrl = true,
            "shift" => s.shift = true,
            "alt" | "option" => s.alt = true,
            "mod" => s.modk = true,
            other => {
                if key.is_some() {
                    return None;
                }
                key = Some(match other {
                    "space" => " ",
                    "esc" => "escape",
                    k => k,
                });
            }
        }
    }
    s.key = key?.to_owned();
    Some(s)
}

/// `effectiveModifiers`: whether the chord holds Command and Control on this platform.
fn effective(s: &Shortcut, mac: bool) -> (bool, bool) {
    (s.meta || (s.modk && mac), s.ctrl || (s.modk && !mac))
}

/// `formatShortcutKeyLabel`.
fn key_label(key: &str) -> String {
    match key {
        " " => "Space".to_owned(),
        "escape" => "Esc".to_owned(),
        "arrowup" => "Up".to_owned(),
        "arrowdown" => "Down".to_owned(),
        "arrowleft" => "Left".to_owned(),
        "arrowright" => "Right".to_owned(),
        k if k.chars().count() == 1 => k.to_uppercase(),
        k => {
            let mut chars = k.chars();
            chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
        }
    }
}

/// `formatShortcutLabel`: the Mac's glyphs run together, elsewhere the words joined by +.
pub fn label(s: &Shortcut, mac: bool) -> String {
    let key = key_label(&s.key);
    let (meta, ctrl) = effective(s, mac);
    if mac {
        let glyph = |on: bool, g: &'static str| if on { g } else { "" };
        return format!("{}{}{}{}{key}", glyph(ctrl, "⌃"), glyph(s.alt, "⌥"), glyph(s.shift, "⇧"), glyph(meta, "⌘"));
    }
    let mut parts: Vec<&str> = Vec::new();
    for (on, word) in [(ctrl, "Ctrl"), (s.alt, "Alt"), (s.shift, "Shift"), (meta, "Meta")] {
        if on {
            parts.push(word);
        }
    }
    parts.push(&key);
    parts.join("+")
}

/// `shortcutConflictKey`.
fn conflict_key(s: &Shortcut, mac: bool) -> String {
    let (meta, ctrl) = effective(s, mac);
    let on = |b: bool, w: &'static str| if b { w } else { "" };
    format!("{}|{}|{}|{}|{}", s.key, on(meta, "meta"), on(ctrl, "ctrl"), on(s.shift, "shift"), on(s.alt, "alt"))
}

/// `browserTabClaimsShortcut`: Control with Tab, the mod alone with a digit or T, W or N, and on macOS Command and
/// Option with a side arrow.
pub fn tab_claims(s: &Shortcut, mac: bool) -> bool {
    let (meta, ctrl) = effective(s, mac);
    let mod_alone = if mac { meta && !ctrl } else { ctrl && !meta };
    if s.alt {
        return mac && matches!(s.key.as_str(), "arrowleft" | "arrowright") && mod_alone && !s.shift;
    }
    if s.key == "tab" {
        return ctrl && !meta;
    }
    matches!(s.key.as_str(), "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "t" | "w" | "n") && mod_alone && !s.shift
}

fn defaults() -> impl Iterator<Item = &'static DefaultRule> {
    DEFAULTS.iter()
}

/// The command's default rule on this chord, however the chord is spelled for the platform.
fn default_on(command: &str, key: &str, mac: bool) -> Option<&'static DefaultRule> {
    let chord = conflict_key(&parse(key)?, mac);
    defaults().find(|r| r.command == command && parse(r.key).is_some_and(|at| conflict_key(&at, mac) == chord))
}

/// The when a chord of the person's own takes: its default's on the same chord, else its first default's.
fn override_when(command: &str, key: &str, mac: bool) -> Option<&'static str> {
    default_on(command, key, mac).or_else(|| defaults().find(|r| r.command == command)).and_then(|r| r.when)
}

/// `rulesWith`: each override in place of its command's defaults; one for a command this build does not dispatch, or
/// whose chord does not parse, is left out.
pub fn rules_with(overrides: &Map<String, Value>, mac: bool) -> Vec<Rule> {
    let moved: Vec<(&str, &str)> = overrides
        .iter()
        .filter_map(|(command, key)| Some((command.as_str(), key.as_str()?)))
        .filter(|(command, key)| COMMANDS.contains(command) && parse(key).is_some())
        .collect();
    let kept = defaults().filter(|r| !moved.iter().any(|(command, _)| *command == r.command)).map(|r| Rule {
        key: r.key.to_owned(),
        command: r.command.to_owned(),
        when: r.when,
    });
    let added = moved.iter().map(|(command, key)| Rule {
        key: key.to_string(),
        command: command.to_string(),
        when: override_when(command, key, mac),
    });
    kept.chain(added).collect()
}

/// `keybindingFromKeyboardEvent`: the chord a key press makes, with mod for the platform's own; nothing for a key
/// pressed with no modifier, which is typing rather than a chord.
pub fn chord_of(keystroke: &Keystroke, mac: bool) -> Option<String> {
    let key = keystroke.key.to_lowercase();
    let token = match key.as_str() {
        "space" => "space".to_owned(),
        "escape" => "esc".to_owned(),
        "up" | "down" | "left" | "right" => format!("arrow{key}"),
        "enter" | "tab" | "backspace" | "delete" | "home" | "end" | "pageup" | "pagedown" => key.clone(),
        k if k.chars().count() == 1 => key.clone(),
        k if k.len() <= 3 && k.starts_with('f') && k[1..].chars().all(|c| c.is_ascii_digit()) && k.len() > 1 => key.clone(),
        _ => return None,
    };
    let m = &keystroke.modifiers;
    let mut parts: Vec<&str> = Vec::new();
    if mac {
        if m.platform {
            parts.push("mod");
        }
        if m.control {
            parts.push("ctrl");
        }
    } else {
        if m.control {
            parts.push("mod");
        }
        if m.platform {
            parts.push("meta");
        }
    }
    if m.alt {
        parts.push("alt");
    }
    if m.shift {
        parts.push("shift");
    }
    if parts.is_empty() {
        return None;
    }
    parts.push(&token);
    Some(parts.join("+"))
}

/// Whether one when clause holds in a context; the defaults' clauses are names, `!` and `&&`, with `||` read too.
fn holds(when: &str, context: &dyn Fn(&str) -> bool) -> bool {
    when.split("||").any(|all| {
        all.split("&&").all(|term| {
            let term = term.trim();
            let negations = term.chars().take_while(|c| *c == '!').count();
            let name = term[negations..].trim();
            let value = match name {
                "true" => true,
                "false" => false,
                name => context(name),
            };
            value ^ (negations % 2 == 1)
        })
    })
}

fn names_of(when: Option<&str>, into: &mut BTreeSet<String>) {
    for term in when.unwrap_or_default().split(['&', '|']) {
        let name = term.trim().trim_start_matches('!').trim();
        if !name.is_empty() {
            into.insert(name.to_owned());
        }
    }
}

/// `canFireTogether`: every context the two clauses' names can make is tried, terminalOwnsMod read off
/// terminalFocus and the platform as the matcher derives it.
fn can_fire_together(left: Option<&str>, right: Option<&str>, mac: bool) -> bool {
    let mut names = BTreeSet::from(["terminalFocus".to_owned()]);
    names_of(left, &mut names);
    names_of(right, &mut names);
    let names: Vec<String> = names.into_iter().filter(|n| !matches!(n.as_str(), "terminalOwnsMod" | "true" | "false")).collect();
    (0..1u32 << names.len()).any(|mask| {
        let on = |name: &str| -> bool {
            if name == "terminalOwnsMod" {
                let focus = names.iter().position(|n| n == "terminalFocus").is_some_and(|at| mask & (1 << at) != 0);
                return focus && !mac;
            }
            names.iter().position(|n| n == name).is_some_and(|at| mask & (1 << at) != 0)
        };
        left.is_none_or(|w| holds(w, &on)) && right.is_none_or(|w| holds(w, &on))
    })
}

/// `chordRefusal`: why a command cannot take this chord, or nothing where it can.
pub fn chord_refusal(rules: &[Rule], command: &str, chord: &str, mac: bool) -> Option<String> {
    let shortcut = parse(chord)?;
    let when = override_when(command, chord, mac);
    let key = conflict_key(&shortcut, mac);
    let mut holders: Vec<&str> = Vec::new();
    for rule in rules.iter().filter(|r| r.command != command) {
        let clashes = parse(&rule.key).is_some_and(|other| conflict_key(&other, mac) == key) && can_fire_together(when, rule.when, mac);
        let word = words::command(&rule.command);
        if clashes && !holders.contains(&word) {
            holders.push(word);
        }
    }
    if !holders.is_empty() {
        return Some(words::taken(&holders));
    }
    // A command's own default stands where it stood, so a chord moved off one goes back onto it.
    (tab_claims(&shortcut, mac) && default_on(command, chord, mac).is_none()).then(|| words::tab_keeps(&label(&shortcut, mac)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_chord_reads_in_the_platforms_spelling() {
        let s = parse("mod+alt+arrowleft").unwrap();
        assert_eq!(label(&s, true), "⌥⌘Left");
        assert_eq!(label(&s, false), "Ctrl+Alt+Left");
        assert_eq!(label(&parse("mod+,").unwrap(), false), "Ctrl+,");
        assert_eq!(label(&parse("shift+f3").unwrap(), false), "Shift+F3");
        assert!(tab_claims(&parse("mod+t").unwrap(), false));
        assert!(!tab_claims(&parse("mod+shift+t").unwrap(), false));
    }

    #[test]
    fn an_override_replaces_every_default_of_its_command() {
        let rules = rules_with(json!({ "sidebar.toggle": "mod+shift+y", "nope": "mod+x" }).as_object().unwrap(), false);
        let sidebar: Vec<&Rule> = rules.iter().filter(|r| r.command == "sidebar.toggle").collect();
        assert_eq!(sidebar.len(), 1);
        assert_eq!(sidebar[0].key, "mod+shift+y");
        assert!(rules.iter().all(|r| r.command != "nope"));
    }

    #[test]
    fn a_taken_chord_is_refused_with_its_holder() {
        let rules = rules_with(&Map::new(), false);
        assert_eq!(chord_refusal(&rules, "chat.new", "mod+j", false).as_deref(), Some("Taken by Toggle the terminal drawer"));
        // The terminal's own chords fire only while it has focus, so New thread off the terminal does not clash.
        assert_eq!(chord_refusal(&rules, "chat.new", "mod+d", false), None);
        assert_eq!(chord_refusal(&rules, "files.search", "mod+w", false).as_deref(), Some("Taken by Close panel tab"));
        let moved = rules_with(json!({ "chat.new": "mod+shift+y" }).as_object().unwrap(), false);
        assert_eq!(chord_refusal(&moved, "files.search", "mod+t", false).as_deref(), Some("A browser tab keeps Ctrl+T for itself"));
        assert_eq!(chord_refusal(&moved, "chat.new", "mod+t", false), None);
    }
}
