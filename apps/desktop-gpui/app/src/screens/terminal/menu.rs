// SPDX-License-Identifier: AGPL-3.0-only
//! The shell's right-click menu as `actions/terminalActions.ts` lists it: the clipboard, the screen, then the panes,
//! each row held with today's refusal where it cannot run, opened on the app's one context menu (`ui::context_menu`).

use gpui_kit::{Keystroke, SharedString};

use super::toolbar::{Act, SPLIT_LIMIT};
use crate::screens::app_shell::keys::{self, DEFAULTS};
use crate::ui::context_menu::MenuItem;

/// `NOTHING_SELECTED`, `NO_TERMINAL` in `actions/format.ts`.
pub const NOTHING_SELECTED: &str = "Nothing is selected";
pub const NO_TERMINAL: &str = "No terminal is active";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Copy,
    AddToChat,
    Paste,
    Clear,
    Pane(Act),
}

/// What the rows read: the shell the press landed on, its selection, and whether its group has room for a split.
#[derive(Clone, Copy, Debug, Default)]
pub struct Target {
    pub active: bool,
    pub selection: bool,
    pub can_split: bool,
    /// A composer stands beside the shell to take the excerpt.
    pub composer: bool,
}

/// A command's chord in the terminal-focus context, as the menu shows it beside its row.
fn chord(command: &str, mac: bool) -> Option<Keystroke> {
    let rule = DEFAULTS.iter().find(|r| r.command == command && r.when == Some("terminalFocus"))?;
    Keystroke::parse(&keys::keystroke(rule.chord, mac)?).ok()
}

pub fn rows(t: Target, mac: bool) -> Vec<(Row, MenuItem)> {
    let row = |row: Row, label: &'static str, group: &'static str, refusal: Option<&'static str>| {
        let item = MenuItem { label: label.into(), group, refusal: refusal.map(SharedString::new_static), keys: None, destructive: false };
        (row, item)
    };
    let split_refusal = (!t.can_split).then_some(SPLIT_LIMIT);
    let mut out = vec![row(
        Row::Copy,
        "Copy",
        "clipboard",
        if !t.active {
            Some(NO_TERMINAL)
        } else if !t.selection {
            Some(NOTHING_SELECTED)
        } else {
            None
        },
    )];
    if t.selection {
        out.push(row(Row::AddToChat, "Add to chat", "clipboard", (!t.composer).then_some(NO_TERMINAL)));
    }
    out.push(row(Row::Paste, "Paste", "clipboard", None));
    out.push(row(Row::Clear, "Clear", "screen", (!t.active).then_some(NO_TERMINAL)));
    let (mut split, mut new) = (
        row(Row::Pane(Act::Split), "Split terminal horizontally", "panes", split_refusal),
        row(Row::Pane(Act::New), "New terminal", "panes", None),
    );
    split.1.keys = chord("terminal.split", mac);
    new.1.keys = chord("terminal.new", mac);
    out.push(split);
    out.push(row(Row::Pane(Act::SplitVertical), "Split terminal vertically", "panes", split_refusal));
    out.push(new);
    let mut close = row(Row::Pane(Act::Close), "Close terminal", "panes", None);
    close.1.destructive = true;
    out.push(close);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(rows: &[(Row, MenuItem)]) -> Vec<(String, &'static str, Option<String>)> {
        rows.iter().map(|(_, i)| (i.label.to_string(), i.group, i.refusal.as_ref().map(|r| r.to_string()))).collect()
    }

    #[test]
    fn the_rows_read_as_todays_without_a_selection() {
        let rows = rows(Target { active: true, selection: false, can_split: true, composer: false }, false);
        let none = None;
        assert_eq!(
            read(&rows),
            [
                ("Copy".into(), "clipboard", Some("Nothing is selected".into())),
                ("Paste".into(), "clipboard", none.clone()),
                ("Clear".into(), "screen", none.clone()),
                ("Split terminal horizontally".into(), "panes", none.clone()),
                ("Split terminal vertically".into(), "panes", none.clone()),
                ("New terminal".into(), "panes", none.clone()),
                ("Close terminal".into(), "panes", none),
            ]
        );
        assert!(rows.last().unwrap().1.destructive, "Close is the destructive row");
        assert_eq!(rows.iter().filter(|(_, i)| i.destructive).count(), 1);
        let keys = |label: &str| rows.iter().find(|(_, i)| i.label == label).and_then(|(_, i)| i.keys.clone()).map(|k| k.unparse());
        assert_eq!(keys("Split terminal horizontally").as_deref(), Some("ctrl-d"));
        assert_eq!(keys("New terminal").as_deref(), Some("ctrl-n"));
        assert_eq!(keys("Close terminal"), None);
    }

    #[test]
    fn a_selection_brings_add_to_chat_and_frees_copy() {
        let rows = rows(Target { active: true, selection: true, can_split: false, composer: false }, false);
        let read = read(&rows);
        assert_eq!(read[0], ("Copy".into(), "clipboard", None));
        assert_eq!(
            read[1],
            ("Add to chat".into(), "clipboard", Some("No terminal is active".into())),
            "no composer stands beside the drawer yet"
        );
        assert_eq!(read[4].2.as_deref(), Some("Max 4 per group"));
        assert_eq!(read[5].2.as_deref(), Some("Max 4 per group"));
    }

    #[test]
    fn with_no_active_shell_copy_and_clear_are_refused() {
        let rows = rows(Target::default(), false);
        let read = read(&rows);
        assert_eq!(read[0].2.as_deref(), Some("No terminal is active"));
        assert_eq!(read[2].2.as_deref(), Some("No terminal is active"));
    }
}
