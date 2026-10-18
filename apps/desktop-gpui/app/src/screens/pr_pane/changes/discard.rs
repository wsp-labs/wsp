// SPDX-License-Identifier: AGPL-3.0-only
//! The one confirmation before a file is put back as its last commit has it (`diffs/DiscardDialog.tsx`): nothing keeps
//! what goes, so the dialog says so, the confirming button is the red one, and a refusal lands in the note's place
//! without closing anything. It stands in gpui-kit's dialog layer, which occludes the pane under it.

use gpui_kit::component::WindowExt as _;
use gpui_kit::{div, prelude::*, px, App, Context, Entity, Subscription, WeakEntity, Window};

use super::model::base_name;
use super::{words, ChangesScreen};
use crate::ui::{AlertDialog, Button, Refusal};

/// `max-w-110`, an alert dialog's popup.
const WIDTH: f32 = 440.;

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
    /// Opens the dialog in the window's dialog layer. A press on the scrim does not close an alert, which asks for its
    /// answer; Escape closes it unless the discard is on its way; Enter does nothing, so no key discards.
    pub fn open(screen: &Entity<ChangesScreen>, window: &mut Window, cx: &mut App) {
        let view = DiscardDialog::view(screen, cx);
        let screen = screen.downgrade();
        window.open_dialog(cx, move |dialog, _, cx| {
            let busy = screen.upgrade().is_some_and(|s| s.read(cx).discard_state().is_some_and(|d| d.busy));
            let closed = screen.clone();
            dialog
                .w(px(WIDTH))
                .p_0()
                .border_0()
                .bg(gpui_kit::transparent_black())
                .close_button(false)
                .overlay_closable(false)
                .keyboard(!busy)
                .on_ok(|_, _, _| false)
                .on_close(move |_, _, cx| _ = closed.update(cx, |s, cx| s.discard_closed(cx)))
                .child(view.clone())
        });
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
