// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's keys: the box's edits, its command and mention menu, and Up, Down, Enter, Tab and Esc.

use gpui_kit::component::input::{Enter, Escape, IndentInline, InlineToken, InputEvent, MoveDown, MoveUp, TextareaState};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{px, Context, Entity, EntityInputHandler as _, Focusable as _, SharedString, Window};

use super::model::{self, Hold, ItemKind, Sources, Trigger};
use super::{menus, Composer, ComposerEvent, Menu, PickRow, Popup, StashDraft, MENU_CAP, TEXT};

impl Composer {
    pub(super) fn on_input(&mut self, _: &Entity<TextareaState>, event: &InputEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            InputEvent::Change => {
                self.notice = None;
                if matches!(self.popup, Some(Popup::Reasoning | Popup::Access | Popup::Stash)) {
                    self.popup = None;
                }
                self.edited_in(window, cx)
            }
            // A click back into the text puts away the model picker, whose search had the keys.
            InputEvent::Focus if self.popup == Some(Popup::Model) => {
                self.popup = None;
                cx.notify();
            }
            _ => {}
        }
    }

    fn edited(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.measure(window, cx);
        self.keep_draft(cx);
        cx.notify();
    }

    pub(super) fn edited_in(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edited(window, cx);
        self.refresh_menu(cx);
    }

    /// How many lines the draft takes at the width it has beside the pickers.
    fn measure(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.text(cx);
        let width = self.line_width.get();
        self.lines = if text.contains('\n') {
            2
        } else if width <= px(0.) || text.is_empty() {
            1
        } else {
            let font = gpui_kit::font(cx.theme().font_family.clone());
            let run = gpui_kit::TextRun {
                len: text.len(),
                font,
                color: gpui_kit::Hsla::default(),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            window
                .text_system()
                .shape_text(text.into(), TEXT, &[run], Some(width), None)
                .map(|lines| lines.iter().map(|l| l.wrap_boundaries().len() + 1).sum())
                .unwrap_or(1)
        };
    }

    pub(super) fn refresh_menu(&mut self, cx: &mut Context<Self>) {
        let (text, cursor) = self.editor.read_with(cx, |e, _| (e.value().to_string(), e.cursor()));
        let found = model::trigger(&text, cursor);
        let Some((trigger, range)) = found else {
            self.menu = None;
            self.dismissed = None;
            cx.notify();
            return;
        };
        if self.dismissed.as_ref() == Some(&range) || self.hold.as_ref().is_some_and(Hold::shuts) {
            self.menu = None;
            return;
        }
        self.dismissed = None;
        if matches!(trigger, Trigger::Mention(_)) && self.folder_files.is_none() {
            self.folder_files = Some(Vec::new());
            cx.emit(ComposerEvent::Files);
        }
        if matches!(trigger, Trigger::Hash(_)) && self.host_items.is_none() {
            self.host_items = Some(Vec::new());
            self.items_note = Some("Loading pull requests and issues".to_owned());
            cx.emit(ComposerEvent::Items);
        }
        let commands = self.commands_offered();
        let from = Sources {
            commands: &commands,
            skills: &self.skills,
            files: self.folder_files.as_deref().unwrap_or_default(),
            items: self.host_items.as_deref().unwrap_or_default(),
        };
        let items = model::menu_items(&trigger, from, MENU_CAP);
        let active = self.menu.as_ref().filter(|m| m.trigger == trigger).map_or(0, |m| m.active.min(items.len().saturating_sub(1)));
        self.menu = Some(Menu { trigger, range, items, active });
        cx.notify();
    }

    /// Puts the menu's active row in place of its token: a command as words, a file as a chip.
    pub(super) fn take_menu(&mut self, ix: Option<usize>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(menu) = self.menu.take() else { return };
        let Some(item) = menu.items.get(ix.unwrap_or(menu.active)).cloned() else { return };
        let range = menu.range.clone();
        let chip = match &item.kind {
            ItemKind::Command => None,
            ItemKind::File => Some((format!("file:{}", item.insert), item.label.clone())),
            ItemKind::Skill => Some((format!("skill:{}", item.insert), item.label.trim_start_matches('$').to_owned())),
            ItemKind::Reference { pull } => Some((format!("{}:{}", if *pull { "pr" } else { "issue" }, item.label), item.label.clone())),
        };
        // A chip holds one line, so an item's chip carries its head and the send puts its quoted body back.
        let insert = match &item.kind {
            ItemKind::Reference { .. } => {
                let head = item.insert.lines().next().unwrap_or_default().to_owned();
                self.expansions.insert(head.clone(), item.insert.clone());
                head
            }
            _ => item.insert.clone(),
        };
        self.editor.update(cx, |e, cx| match chip {
            Some((id, label)) => {
                let token = InlineToken::new(SharedString::from(id), insert.clone()).with_label(label);
                if e.replace_range_with_token(range, token, window, cx).is_ok() {
                    e.replace(" ", window, cx);
                }
            }
            None => {
                e.set_selected_range(range, cx);
                e.replace(insert.clone(), window, cx);
            }
        });
        self.menu = None;
        self.edited(window, cx);
    }

    fn menu_step(&mut self, by: isize, cx: &mut Context<Self>) -> bool {
        if let Some(menu) = &mut self.menu {
            let n = menu.items.len() as isize;
            if n > 0 {
                menu.active = (menu.active as isize + by).rem_euclid(n) as usize;
            }
            cx.notify();
            return true;
        }
        if let Some(popup) = self.popup {
            let n = menus::popup_rows(self, popup, cx).len() as isize;
            if n > 0 {
                let from = self.popup_active.map_or(if by > 0 { -1 } else { n }, |a| a as isize);
                self.popup_active = Some((from + by).rem_euclid(n) as usize);
            }
            cx.notify();
            return true;
        }
        false
    }

    pub(super) fn on_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_step(-1, cx) || self.recall(true, window, cx) {
            cx.stop_propagation();
        }
    }

    pub(super) fn on_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu_step(1, cx) || self.recall(false, window, cx) {
            cx.stop_propagation();
        }
    }

    /// Up on the first line or Down on the last walks the thread's sent prompts (composerPromptHistory.ts); the caret
    /// lands at the start going back and at the end going forward.
    fn recall(&mut self, back: bool, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let (text, cursor) = self.editor.read_with(cx, |e, _| (e.value().to_string(), e.cursor()));
        let edge = if back { !text[..cursor.min(text.len())].contains('\n') } else { !text[cursor.min(text.len())..].contains('\n') };
        if !edge {
            return false;
        }
        let Some((at, prompt)) = model::recall_step(&self.recalls, self.recalled.as_ref(), &text, back) else { return false };
        self.recalled = at;
        self.set_text(&prompt, window, cx);
        if back {
            self.editor.update(cx, |e, cx| e.set_selected_range(0..0, cx));
        }
        true
    }

    pub(super) fn on_pick_row(&mut self, action: &PickRow, window: &mut Window, cx: &mut Context<Self>) {
        if self.popup != Some(Popup::Model) {
            return;
        }
        cx.stop_propagation();
        if let Some(value) = menus::popup_rows(self, Popup::Model, cx).get(action.0 - 1).cloned() {
            self.pick(Popup::Model, value, window, cx);
        }
    }

    /// Enter picks the menu's row, else sends on the chord `sendWith` names, Enter or Cmd or Ctrl and Enter
    /// (`enterSends`); taken here before the text field's own Enter, which breaks the line on Shift and on the chord
    /// that does not send. An Enter while the field holds marked text is the input method's, as `isComposing` is
    /// today's: GPUI matches bindings before the input method sees the key.
    pub(super) fn on_enter(&mut self, action: &Enter, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.update(cx, |e, cx| e.marked_text_range(window, cx).is_some()) {
            cx.stop_propagation();
            return;
        }
        if action.shift {
            return;
        }
        if self.menu.as_ref().is_some_and(|m| !m.items.is_empty()) {
            cx.stop_propagation();
            self.take_menu(None, window, cx);
        } else if let Some(popup) = self.popup.filter(|_| self.popup_active.is_some()) {
            cx.stop_propagation();
            if let Some(value) = self.popup_active.and_then(|ix| menus::popup_rows(self, popup, cx).get(ix).cloned()) {
                self.pick(popup, value, window, cx);
            }
        } else if action.secondary == self.send_with_mod {
            cx.stop_propagation();
            self.submit(window, cx);
        }
    }

    pub(super) fn on_tab(&mut self, _: &IndentInline, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.as_ref().is_some_and(|m| !m.items.is_empty()) {
            self.take_menu(None, window, cx);
            cx.stop_propagation();
        }
    }

    pub(super) fn on_escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if self.bar.is_some() {
            self.fold_bar(window, cx);
            cx.stop_propagation();
        } else if let Some(menu) = self.menu.take() {
            self.dismissed = Some(menu.range);
            cx.stop_propagation();
        } else if self.popup.take().is_some() {
            self.editor.focus_handle(cx).focus(window, cx);
            cx.stop_propagation();
        }
        cx.notify();
    }

    pub(super) fn on_stash(&mut self, _: &StashDraft, window: &mut Window, cx: &mut Context<Self>) {
        self.stash_draft(window, cx);
    }
}
