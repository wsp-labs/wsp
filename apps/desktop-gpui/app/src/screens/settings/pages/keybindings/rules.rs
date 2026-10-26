// SPDX-License-Identifier: AGPL-3.0-only
//! How Keybindings reads and checks the frame's rules (`keys.rs`): the platform's spelling of a chord (`keybindings.ts`)
//! and why a command cannot take one (`keybindingOverrides.ts`).

use std::collections::BTreeSet;

use gpui_kit::Keystroke;

use super::words;
use crate::screens::app_shell::keys::{self, default_on, override_when, parse, same_chord, Bound, Chord};

/// `KEYBINDING_COMMANDS`: each command the frame's table holds, once, in the table's order.
pub fn commands() -> impl Iterator<Item = &'static str> {
    let table = keys::DEFAULTS;
    table.iter().enumerate().filter(|(at, rule)| table[..*at].iter().all(|r| r.command != rule.command)).map(|(_, rule)| rule.command)
}

/// `effectiveModifiers`: whether the chord holds Command and Control on this platform.
fn effective(s: &Chord, mac: bool) -> (bool, bool) {
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
pub fn label(s: &Chord, mac: bool) -> String {
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

/// `browserTabClaimsShortcut`: Control with Tab, the mod alone with a digit or T, W or N, and on macOS Command and
/// Option with a side arrow.
pub fn tab_claims(s: &Chord, mac: bool) -> bool {
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
pub fn chord_refusal(rules: &[Bound], command: &str, chord: &str, mac: bool) -> Option<String> {
    let shortcut = parse(chord)?;
    let when = override_when(command, chord, mac);
    let mut holders: Vec<&str> = Vec::new();
    for rule in rules.iter().filter(|r| r.command != command) {
        let clashes = same_chord(&rule.chord, chord, mac) && can_fire_together(when, rule.when, mac);
        let word = words::command(rule.command);
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
    use std::collections::BTreeMap;

    use super::*;

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
    fn the_commands_are_the_webs() {
        let web = concat!(env!("CARGO_MANIFEST_DIR"), "/../../web/src/keybindingTypes.ts");
        let types = std::fs::read_to_string(web).unwrap();
        let list = types.split("KEYBINDING_COMMANDS = [").nth(1).and_then(|rest| rest.split("] as const").next()).unwrap();
        let slots = types.split("WORKSPACE_SELECT_SLOTS = [").nth(1).and_then(|rest| rest.split(']').next()).unwrap();
        let mut theirs: BTreeSet<String> =
            list.lines().filter_map(|line| line.trim().strip_prefix('"')?.split('"').next().map(str::to_owned)).collect();
        assert!(list.contains("...WORKSPACE_SELECT_SLOTS.map(workspaceSelectCommand)"), "{list}");
        theirs.extend(slots.split(',').map(|slot| format!("workspace.select.{}", slot.trim())));
        let ours: Vec<&str> = commands().collect();
        assert_eq!(ours.len(), theirs.len(), "{ours:?}");
        assert_eq!(ours.into_iter().map(str::to_owned).collect::<BTreeSet<_>>(), theirs);
    }

    #[test]
    fn a_taken_chord_is_refused_with_its_holder() {
        let rules = keys::bound(false, &BTreeMap::new());
        assert_eq!(chord_refusal(&rules, "chat.new", "mod+j", false).as_deref(), Some("Taken by Toggle the terminal drawer"));
        // The terminal's own chords fire only while it has focus, so New thread off the terminal does not clash.
        assert_eq!(chord_refusal(&rules, "chat.new", "mod+d", false), None);
        assert_eq!(chord_refusal(&rules, "files.search", "mod+w", false).as_deref(), Some("Taken by Close panel tab"));
        let moved = keys::bound(false, &BTreeMap::from([("chat.new".to_owned(), Some("mod+shift+y".to_owned()))]));
        assert_eq!(chord_refusal(&moved, "files.search", "mod+t", false).as_deref(), Some("A browser tab keeps Ctrl+T for itself"));
        assert_eq!(chord_refusal(&moved, "chat.new", "mod+t", false), None);
    }
}
