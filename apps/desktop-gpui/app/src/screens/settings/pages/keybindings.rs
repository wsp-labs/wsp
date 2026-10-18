// SPDX-License-Identifier: AGPL-3.0-only
//! Keybindings (`keybindings.ts`, `ChordKeys.tsx`): one line per command, the label at the left and its chords'
//! keycaps at the right, worded per platform, then the keys that are not rules. A command's keycaps are the button
//! that changes its chord, the person's own chord replacing all of its defaults; the nine jumps share a line one chord
//! cannot stand for, and the fixed keys are not rules, so both stay as drawn.

mod rules;

use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::component::h_flex;
use gpui_kit::{div, prelude::*, px, App, Entity, KeystrokeEvent, SharedString, Subscription, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::Preferences;

use super::general::record;
use crate::screens::app_shell::keys::{self, parse, Bound};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::Acts;
use crate::ui::{ActivePalette, Button, KeyCaps};
use rules::{chord_of, chord_refusal, commands, label};

/// `KEYBINDINGS_WORDS` in `format.ts` and `CHORD_WORDS`, `KEYBINDING_WORDS` and `JUMP_WORD` in `keybindingWords.ts`.
mod words {
    pub const WINDOW_AND_PANELS: &str = "Window and panels";
    pub const TASKS_AND_THREADS: &str = "Tasks and threads";
    pub const TERMINAL: &str = "Terminal, while it has focus";
    pub const FIXED: &str = "Fixed";
    pub const SEND_MESSAGE: &str = "Send the message";
    pub const SUBMIT_COMMENT: &str = "Submit a review comment";
    pub const LEAVE_SETTINGS: &str = "Leave Settings";
    pub const CLOSE_WINDOW: &str = "Close the window";
    pub const JUMP: &str = "Jump to a task";
    pub const CAPTURING: &str = "Press keys";
    pub const RESET: &str = "Reset";

    pub fn command(command: &str) -> &'static str {
        match command {
            "commandPalette.toggle" => "Search",
            "files.quickOpen" => "Find a file",
            "files.search" => "Search in files",
            "settings.toggle" => "Settings",
            "sidebar.toggle" => "Toggle the sidebar",
            "terminal.toggle" => "Toggle the terminal drawer",
            "rightPanel.toggle" => "Toggle the right panel",
            "rightPanel.nextTab" => "Next panel tab",
            "rightPanel.previousTab" => "Previous panel tab",
            "rightPanel.closeTab" => "Close panel tab",
            "preview.toggle" => "Toggle the preview",
            "chat.new" => "New thread",
            "workspace.next" => "Next task",
            "workspace.previous" => "Previous task",
            "thread.next" => "Next thread",
            "thread.previous" => "Previous thread",
            "thread.settle" => "Settle thread",
            "editor.open" => "Open in editor",
            "thread.nextNeedsYou" => "Next thread that needs you",
            "thread.find" => "Find in thread",
            "thread.findOlder" => "Step to the older match",
            "thread.findNewer" => "Step to the newer match",
            "terminal.split" => "Split the terminal",
            "terminal.new" => "New terminal",
            "terminal.zoomIn" => "Zoom in",
            "terminal.zoomOut" => "Zoom out",
            "terminal.zoomReset" => "Reset the zoom",
            _ => JUMP,
        }
    }

    pub fn taken(holders: &[&str]) -> String {
        format!("Taken by {}", holders.join(" and "))
    }
    pub fn tab_keeps(chord: &str) -> String {
        format!("A browser tab keeps {chord} for itself")
    }
}

/// The cards, each the commands it lists in the order they are read; the jump line stands for all nine.
const SHELL: [&str; 11] = [
    "commandPalette.toggle",
    "files.quickOpen",
    "files.search",
    "settings.toggle",
    "sidebar.toggle",
    "terminal.toggle",
    "rightPanel.toggle",
    "rightPanel.nextTab",
    "rightPanel.previousTab",
    "rightPanel.closeTab",
    "preview.toggle",
];
const WORK: [&str; 12] = [
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
];
const TERMINAL: [&str; 5] = ["terminal.split", "terminal.new", "terminal.zoomIn", "terminal.zoomOut", "terminal.zoomReset"];

/// The desktop app holds the window, so the chords a browser keeps for its own tabs reach it.
const DESKTOP: bool = true;
const MAC: bool = cfg!(target_os = "macos");

fn is_jump(command: &str) -> bool {
    command.starts_with("workspace.select.")
}

/// `chordsOf`: every rule for the command, each label one keycap; the nine jumps fold into the first and the last.
fn chords_of(rules: &[Bound], command: &str, mac: bool) -> Vec<String> {
    let labels: Vec<String> = rules
        .iter()
        .filter(|r| if is_jump(command) { is_jump(r.command) } else { r.command == command })
        .filter_map(|r| parse(&r.chord))
        .filter(|s| DESKTOP || !rules::tab_claims(s, mac))
        .map(|s| label(&s, mac))
        .collect();
    if is_jump(command) {
        return match (labels.first(), labels.last()) {
            (Some(first), Some(last)) => vec![first.clone(), last.clone()],
            _ => Vec::new(),
        };
    }
    labels
}

fn keycaps(keys: &[String]) -> KeyCaps {
    let chords: Vec<[&str; 1]> = keys.iter().map(|k| [k.as_str()]).collect();
    let slices: Vec<&[&str]> = chords.iter().map(|c| c.as_slice()).collect();
    KeyCaps::new(&slices)
}

/// The one line listening for a chord, the reason it refused the last one, and the hold on the window's keys.
#[derive(Default)]
struct Capture {
    command: Option<String>,
    refused: Option<String>,
    _keys: Option<Subscription>,
}

fn stop(capture: &Entity<Capture>, acts: &Acts, cx: &mut App) {
    capture.update(cx, |c, _| *c = Capture::default());
    acts.notify(cx);
}

/// Starts listening: the next chord pressed anywhere in the window is this command's, and never reaches what it names.
fn listen(capture: &Entity<Capture>, command: &str, rules: Rc<Vec<Bound>>, acts: &Acts, cx: &mut App) {
    // Weak: a strong handle would keep the Capture, and with it this listener, alive after the page stops drawing.
    let (held, acts_held, command_held) = (capture.downgrade(), acts.clone(), command.to_owned());
    let keys = cx.intercept_keystrokes(move |event: &KeystrokeEvent, _, cx| {
        let Some(capture) = held.upgrade() else { return };
        cx.stop_propagation();
        let (acts, command) = (acts_held.clone(), command_held.clone());
        if event.keystroke.key == "escape" {
            cx.defer(move |cx| stop(&capture, &acts, cx));
            return;
        }
        let Some(chord) = chord_of(&event.keystroke, MAC) else {
            capture.update(cx, |c, _| c.refused = None);
            acts.notify(cx);
            return;
        };
        if let Some(refused) = chord_refusal(&rules, &command, &chord, MAC) {
            capture.update(cx, |c, _| c.refused = Some(refused));
            acts.notify(cx);
            return;
        }
        let own: Vec<&keys::Rule> = keys::DEFAULTS.iter().filter(|r| r.command == command).collect();
        let value = if own.len() == 1 && own[0].chord == chord { Value::Null } else { Value::String(chord) };
        acts.set_preferences(json!({ "keybindings": { command.clone(): value } }), cx);
        cx.defer(move |cx| stop(&capture, &acts, cx));
    });
    capture.update(cx, |c, _| *c = Capture { command: Some(command.to_owned()), refused: None, _keys: Some(keys) });
    acts.notify(cx);
}

/// `ChordKeys`: the keycaps as the button that changes them, with Reset beside them while the chord is the person's.
fn chord_keys(
    ctx: &Ctx,
    capture: &Entity<Capture>,
    command: &str,
    keys: &[String],
    rules: &Rc<Vec<Bound>>,
    overridden: bool,
    cx: &App,
) -> impl IntoElement {
    let p = cx.palette().clone();
    let state = capture.read(cx);
    let capturing = state.command.as_deref() == Some(command);
    let refused = if capturing { state.refused.clone() } else { None };
    let reset = (overridden && !capturing).then(|| {
        let (acts, command) = (ctx.acts.clone(), command.to_owned());
        Button::ghost(SharedString::from(format!("reset-chord-{command}")), words::RESET)
            .xs()
            .on_click(move |_, _, cx| acts.set_preferences(json!({ "keybindings": { command.clone(): null } }), cx))
    });
    let inside = if !capturing {
        keycaps(keys).into_any_element()
    } else {
        let (text, ink) = match refused {
            None => (SharedString::new_static(words::CAPTURING), p.muted_foreground),
            Some(refused) => (SharedString::from(refused), p.error_foreground),
        };
        div().text_size(px(12.)).line_height(px(16.)).text_color(ink).child(text).into_any_element()
    };
    let (capture, acts, rules, command) = (capture.clone(), ctx.acts.clone(), rules.clone(), command.to_owned());
    h_flex().flex_none().gap_2().children(reset).child(
        h_flex()
            .id(SharedString::from(format!("chord-{command}")))
            .debug_selector(|| "chord".into())
            .h(px(28.))
            .mx(px(-6.))
            .px(px(6.))
            .rounded(px(10.))
            .cursor_pointer()
            .when(capturing, |el| el.bg(p.accent))
            .hover(|s| s.bg(p.accent))
            .on_click(move |_, _, cx| {
                if capturing {
                    stop(&capture, &acts, cx);
                } else {
                    listen(&capture, &command, rules.clone(), &acts, cx);
                }
            })
            .child(inside),
    )
}

fn lines_of(
    ctx: &Ctx,
    capture: &Entity<Capture>,
    rules: &Rc<Vec<Bound>>,
    overrides: &BTreeMap<String, Option<String>>,
    commands: &[&str],
    cx: &App,
) -> Vec<Item> {
    commands
        .iter()
        .filter_map(|&command| {
            let keys = chords_of(rules, command, MAC);
            if keys.is_empty() {
                return None;
            }
            if is_jump(command) {
                return Some(Item::line("workspace.select", words::JUMP, None, |line| line.keys(keycaps(&keys).joiner("to"))));
            }
            let control = chord_keys(ctx, capture, command, &keys, rules, overrides.contains_key(command), cx);
            Some(Item::line(command, words::command(command), None, |line| line.right(control)))
        })
        .collect()
}

/// `fixedLines`: the keys that are not rules; the send is the key picked on General.
fn fixed_lines(send_with: &str) -> Vec<Item> {
    let mod_enter = parse("mod+enter").map(|s| label(&s, MAC)).unwrap_or_else(|| "Enter".to_owned());
    let send = if send_with == "mod-enter" { mod_enter.clone() } else { "Enter".to_owned() };
    let close_window = parse("mod+shift+w").filter(|_| DESKTOP).map(|s| label(&s, MAC));
    let mut lines = vec![
        Item::line("send", words::SEND_MESSAGE, None, |line| line.keys(keycaps(&[send]))),
        Item::line("submit-comment", words::SUBMIT_COMMENT, None, |line| line.keys(keycaps(&[mod_enter]))),
        Item::line("leave-settings", words::LEAVE_SETTINGS, None, |line| line.keys(keycaps(&["Esc".to_owned()]))),
    ];
    if let Some(close) = close_window {
        lines.push(Item::line("close-window", words::CLOSE_WINDOW, None, |line| line.keys(keycaps(&[close]))));
    }
    lines
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let Some(p) = record(ctx) else { return Vec::new() };
    let overrides = keys::overrides_of(p.get("keybindings"));
    let rules = Rc::new(keys::bound(MAC, &overrides));
    let capture = window.use_keyed_state("keybindings-capture", cx, |_, _| Capture::default());
    vec![
        CardData::new("shell").head(words::WINDOW_AND_PANELS).items(lines_of(ctx, &capture, &rules, &overrides, &SHELL, cx)),
        CardData::new("work").head(words::TASKS_AND_THREADS).items(lines_of(ctx, &capture, &rules, &overrides, &WORK, cx)),
        CardData::new("terminal").head(words::TERMINAL).items(lines_of(ctx, &capture, &rules, &overrides, &TERMINAL, cx)),
        CardData::new("fixed").head(words::FIXED).items(fixed_lines(p["sendWith"].as_str().unwrap_or("enter"))),
    ]
}

/// `KEYBINDING_DEFAULTS`: every command back on its defaults.
pub fn defaults() -> Value {
    json!({ "keybindings": Value::Object(commands().map(|c| (c.to_owned(), Value::Null)).collect()) })
}

/// `keybindingsOffDefaults`: whether any command this build dispatches wears a chord of the person's own.
pub fn off_defaults(p: &Preferences) -> bool {
    commands().any(|c| p.keybindings.contains_key(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_read_every_chord_of_a_command() {
        let rules = keys::bound(false, &BTreeMap::new());
        assert_eq!(chords_of(&rules, "sidebar.toggle", false), ["Ctrl+Shift+B", "Ctrl+B"]);
        assert_eq!(chords_of(&rules, "workspace.select.1", false), ["Ctrl+1", "Ctrl+9"]);
        let mac = keys::bound(true, &BTreeMap::new());
        assert_eq!(chords_of(&mac, "commandPalette.toggle", true), ["⇧⌘P", "⌘K"]);
    }

    #[test]
    fn restore_clears_every_command() {
        let patch = defaults();
        assert_eq!(patch["keybindings"].as_object().unwrap().len(), commands().count());
        assert!(patch["keybindings"]["workspace.select.9"].is_null());
    }

    #[gpui_kit::test]
    fn the_listener_goes_with_the_page(cx: &mut gpui_kit::TestAppContext) {
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let capture = gpui_kit::AppContext::new(cx, |_| Capture::default());
        let gone = capture.downgrade();
        cx.update(|cx| listen(&capture, "chat.new", Rc::new(keys::bound(false, &BTreeMap::new())), &acts, cx));
        drop(capture);
        cx.run_until_parked();
        assert!(gone.upgrade().is_none(), "the window's key listener still holds the Capture the page let go of");
    }
}
