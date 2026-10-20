// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript on its way, and `SetupCard` standing as a new thread's first content.

use gpui_kit::component::v_flex;
use gpui_kit::{div, prelude::*, px, App, Pixels, SharedString};

use super::SM_TIGHT;
use crate::ui::ActivePalette;

/// `TRANSCRIPT_LOADING`: the transcript on its way, centred over the composer, as a thread not read yet and a fork its
/// agent is still making both stand (ChatView's `!hydrated || (empty && forkWaits)`).
pub fn loading(composer_inset: Pixels, cx: &App) -> impl IntoElement {
    div()
        .debug_selector(|| "transcript-loading".into())
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .pb(composer_inset)
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .text_color(cx.palette().muted_foreground)
        .child("Loading transcript")
}

/// A create this window asked for with no message, landed: `SetupCard` standing as the thread's first content until
/// its first message, every step done, centred in the room over the composer (`SetupRoom`).
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    pub name: String,
    /// Each step's words, as the create's log named it (`creationSteps`).
    pub steps: Vec<String>,
}

pub fn setup_card(setup: &Setup, composer_inset: Pixels) -> impl IntoElement {
    let steps = if setup.steps.is_empty() { vec!["Asking wsp to start it".to_owned()] } else { setup.steps.clone() };
    let card = crate::ui::Card::new("setting-up").head(format!("{} is set up", setup.name)).items(
        steps
            .into_iter()
            .enumerate()
            .map(|(i, name)| crate::ui::StepRow::new(SharedString::from(format!("step-{i}")), crate::ui::Step::Done, name)),
    );
    v_flex()
        .debug_selector(|| "setting-up-room".into())
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .bottom(composer_inset)
        .gap(px(16.))
        .px(px(20.))
        .pt(px(16.))
        .child(
            div().flex().flex_1().items_center().justify_center().pb(px(24.)).child(div().w_full().max_w(px(768.)).min_w_0().child(card)),
        )
}
