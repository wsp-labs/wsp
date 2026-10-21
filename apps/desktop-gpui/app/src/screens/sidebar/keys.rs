// SPDX-License-Identifier: AGPL-3.0-only
//! The list's keys (`WorkspaceSidebar.tsx` onKeyDown): up and down through the rows, Home and End, left and right
//! to fold a tile's tree, Alt+Shift+up and down to move a root, Enter or Space to press a row.

use gpui_kit::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, Window};
use serde_json::{json, Value};

use super::model::{self, Item, Under};
use super::rows::Drawn;
use super::{Asked, Sidebar};
use crate::ui::Palette;

/// What a key does to the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walk {
    Next,
    Previous,
    First,
    Last,
}

/// The row a walk lands on from `at` among `n` rows (`Sidebar.logic.ts` resolveAdjacentThreadId): down from none is
/// the first row, up or Home or End from none goes nowhere, and the ends hold.
pub fn walk(at: Option<usize>, n: usize, to: Walk) -> Option<usize> {
    if n == 0 {
        return None;
    }
    match (to, at) {
        (Walk::Next, None) => Some(0),
        (_, None) => None,
        (Walk::Next, Some(i)) => Some((i + 1).min(n - 1)),
        (Walk::Previous, Some(i)) => Some(i.saturating_sub(1)),
        (Walk::First, Some(_)) => Some(0),
        (Walk::Last, Some(_)) => Some(n - 1),
    }
}

impl Sidebar {
    /// A row pressed with the pointer becomes the row the keys walk from, with no ring until a key moves it.
    pub(crate) fn point_at(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.key_row = self.tree_row(key);
        self.keyboard = false;
        window.focus(&self.focus, cx);
    }

    fn tile_at(&self, row: usize) -> Option<&model::Tile> {
        match self.drawn_rows.get(row)? {
            Drawn::Line(ix) => match &self.lines.get(*ix)?.item {
                Item::Tile(t) => Some(t),
                _ => None,
            },
            _ => None,
        }
    }

    pub(crate) fn key_down(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.is_open() || self.switcher.is_some() || self.dialog.is_some() || self.renaming.is_some() {
            return;
        }
        let k = &e.keystroke;
        let n = self.drawn_rows.len();
        if k.modifiers.alt && k.modifiers.shift && matches!(k.key.as_str(), "up" | "down") {
            if let Some(t) = self.key_row.and_then(|r| self.tile_at(r)).filter(|t| t.moves && !t.inbox).cloned() {
                let key = self.threads[t.thread].key.clone();
                self.move_root(&key, if k.key == "up" { model::Move::Up } else { model::Move::Down }, cx);
            }
            cx.stop_propagation();
            return;
        }
        if k.modifiers.alt || k.modifiers.control || k.modifiers.platform {
            return;
        }
        let to = match k.key.as_str() {
            "down" => Some(Walk::Next),
            "up" => Some(Walk::Previous),
            "home" => Some(Walk::First),
            "end" => Some(Walk::Last),
            _ => None,
        };
        if let Some(to) = to {
            if let Some(at) = walk(self.key_row, n, to) {
                self.key_row = Some(at);
                self.keyboard = true;
                self.list.scroll_to_reveal_item(at);
                cx.notify();
            }
            cx.stop_propagation();
            return;
        }
        let Some(row) = self.key_row else { return };
        match k.key.as_str() {
            "left" | "right" => {
                let Some(t) = self.tile_at(row).filter(|t| !t.inbox).cloned() else { return };
                let key = self.threads[t.thread].key.clone();
                match (k.key.as_str(), t.under) {
                    ("left", Under::Open) => self.mark_threads(vec![key], json!({ "folded": true }), cx),
                    ("right", Under::Folded(_)) => self.mark_threads(vec![key], json!({ "folded": false }), cx),
                    _ => return,
                }
                cx.stop_propagation();
            }
            "enter" | "space" => {
                self.press(row, window, cx);
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    /// Enter or Space on a row: what a click on it does.
    fn press(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(d) = self.drawn_rows.get(row).cloned() else { return };
        match d {
            Drawn::Line(ix) => match self.lines[ix].item.clone() {
                Item::Tile(t) => {
                    let key = self.threads[t.thread].key.clone();
                    self.select(&key, cx);
                    self.keyboard = true;
                    window.focus(&self.focus, cx);
                }
                Item::Head { section, .. } => self.toggle_section(section, cx),
                Item::Group { attempt, .. } => self.toggle_group(&attempt, cx),
                Item::Finished { lead, .. } => {
                    let key = self.threads[lead].key.clone();
                    self.toggle_finished(&key, cx);
                }
                Item::More { lead, .. } => {
                    let key = lead.map(|l| self.threads[l].key.clone());
                    self.more(key.as_deref(), cx);
                }
                Item::Subagent { .. } => {}
                Item::Machine { workspace } => cx.emit(Asked::NewThreadIn(workspace)),
            },
            Drawn::ForwardsHead => {
                if !self.foot_folds.remove("forwards") {
                    self.foot_folds.insert("forwards".to_owned());
                }
                self.rebuild(cx);
            }
            Drawn::Forward(i) => {
                if let Some(port) = self.forwards_shown().nth(i).and_then(|f| f.get("port").and_then(Value::as_u64)) {
                    cx.open_url(&format!("http://localhost:{port}/"));
                }
            }
            Drawn::Launch(_) | Drawn::Creation(_) | Drawn::Workspace(_) => {}
        }
    }

    /// The 2 px ring in the ring's ink round the row the keys stand on, where the sidebar has the keyboard.
    pub(crate) fn ring(&self, row: usize, p: &Palette, window: &Window) -> Option<AnyElement> {
        if !self.keyboard || self.key_row != Some(row) || !self.focus.is_focused(window) {
            return None;
        }
        let (depth, gap) = match self.drawn_rows.get(row)? {
            Drawn::Line(ix) => (self.lines[*ix].depth as f32, self.lines[*ix].gap),
            _ => (0., false),
        };
        Some(
            div()
                .absolute()
                .left(px(8. + 21. * depth))
                .right(px(8.))
                .top(px(if gap { 12. } else { 0. }))
                .bottom_0()
                .rounded(px(8.))
                .border_2()
                .border_color(p.ring)
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{walk, Walk};

    #[test]
    fn the_walk_holds_at_the_ends_and_starts_down_from_nothing() {
        assert_eq!(walk(None, 3, Walk::Next), Some(0));
        assert_eq!(walk(None, 3, Walk::Previous), None);
        assert_eq!(walk(None, 3, Walk::Last), None);
        assert_eq!(walk(Some(2), 3, Walk::Next), Some(2));
        assert_eq!(walk(Some(0), 3, Walk::Previous), Some(0));
        assert_eq!(walk(Some(1), 3, Walk::Last), Some(2));
        assert_eq!(walk(Some(1), 0, Walk::Next), None);
    }
}
