// SPDX-License-Identifier: AGPL-3.0-only
//! The one confirmation before a file is put back as its last commit has it (`diffs/DiscardDialog.tsx`): nothing keeps
//! what goes, so the dialog says so, the confirming button is the red one, and a refusal lands in the note's place
//! without closing anything. It stands on the window's dialogs layer, which centres it over the blurred page.

use std::rc::Rc;

use gpui_kit::{div, prelude::*, App, Context, Entity, Subscription, WeakEntity, Window};

use super::model::base_name;
use super::{words, ChangesScreen};
use crate::screens::dialogs::layer::{self, Closes};
use crate::ui::{AlertDialog, Button, Refusal};

/// The dialog's name on the layer.
pub const KEY: &str = "changes-discard";

pub struct DiscardState {
    pub path: String,
    /// A file no commit has, which a discard removes rather than puts back.
    pub deletes: bool,
    pub busy: bool,
    pub refusal: Option<String>,
}

impl DiscardState {
    pub fn new(path: String, deletes: bool) -> Self {
        DiscardState { path, deletes, busy: false, refusal: None }
    }
}

/// The dialog's view; what it says is the screen's [`DiscardState`], so it draws again whenever the screen does.
pub struct DiscardDialog {
    screen: WeakEntity<ChangesScreen>,
    _screen: Subscription,
}

impl DiscardDialog {
    /// Opens the dialog on the dialogs layer as an alert: a press on the scrim is no answer, Escape closes it unless
    /// the discard is on its way (the screen holds it then), and Enter does nothing, so no key discards.
    pub fn open(screen: &Entity<ChangesScreen>, window: &mut Window, cx: &mut App) {
        let view = DiscardDialog::view(screen, cx);
        let closed = screen.downgrade();
        let on_close: layer::OnClose = Rc::new(move |_, cx| _ = closed.update(cx, |s, cx| s.discard_closed(cx)));
        layer::open(KEY, view.into(), Closes::Escape, Some(on_close), window, cx);
    }

    /// The dialog's body, drawing the screen's state.
    pub fn view(screen: &Entity<ChangesScreen>, cx: &mut App) -> Entity<DiscardDialog> {
        let watched = screen.clone();
        cx.new(|cx| DiscardDialog { screen: watched.downgrade(), _screen: cx.observe(&watched, |_, _, cx| cx.notify()) })
    }
}

impl Render for DiscardDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(screen) = self.screen.upgrade() else { return div().into_any_element() };
        let Some(state) = screen.read(cx).discard_state() else { return div().into_any_element() };
        let name = base_name(&state.path).to_owned();
        let refusal = match &state.refusal {
            Some(said) => Refusal::new().said(said.clone()),
            None => Refusal::new().quiet(if state.deletes { words::discard_deletes(&name) } else { words::DISCARD_NOTE.to_owned() }),
        };
        let (cancel, go) = (screen.downgrade(), screen.downgrade());
        div()
            .debug_selector(|| "discard-dialog".into())
            .child(
                AlertDialog::new(words::discard_title(&name))
                    .child(refusal)
                    .act(
                        Button::outline("discard-cancel", words::CANCEL)
                            .neutral_ring()
                            .held(state.busy)
                            .on_click(move |_, window, cx| _ = cancel.update(cx, |s, cx| s.close_discard(window, cx))),
                    )
                    .act(
                        Button::destructive("discard-confirm", words::DISCARD)
                            .held(state.busy)
                            .on_click(move |_, window, cx| _ = go.update(cx, |s, cx| s.discard(window, cx))),
                    ),
            )
            .into_any_element()
    }
}
