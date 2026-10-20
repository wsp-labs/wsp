// SPDX-License-Identifier: AGPL-3.0-only
//! The transcript's cards as the web app draws them: prompts, plans, subagent folds, children, changed files, forks, files.

mod changed_files;
mod files;
mod plan;
mod prompt;
mod rule_line;
mod setup;
mod spawn;
mod start_failed;
mod strip;
mod subagent_fold;
mod threads;
mod tree_row;

use gpui_kit::component::{h_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Hsla, Pixels, SharedString, Window};

use crate::thread_view::ThreadView;
use crate::transcript::{self, Row, RowKind};
use crate::ui::{ActivePalette, Glyph, Palette};

use changed_files::changed_files;
pub use files::{attachment_line, drop_images, files, held, image_size, let_go, thumbnail, unbase64, Released, Shot, IMAGE_PREVIEW};
use plan::plan;
pub use plan::{plan_filename, plan_text};
use prompt::prompt;
pub(crate) use prompt::refusal;
pub use prompt::{answer, answer_request, refusal_of, Refusal, PICK_JOIN};
use rule_line::{fork, rule_line, slate_line, wake_button};
pub use rule_line::{machine_wait, readings, Readings};
pub use setup::{loading, setup_card, Setup};
use spawn::spawn;
use start_failed::start_failed;
pub use strip::strip;
use subagent_fold::{subagent, subagent_page};
pub use subagent_fold::{subagent_route, SUBAGENT};
use threads::threads;

/// `text-sm` alone, and `text-sm leading-5`.
const SM_TIGHT: (Pixels, Pixels) = (px(14.), px(20.));
/// `text-xs`, `text-xs leading-4`.
const XS: (Pixels, Pixels) = (px(12.), px(16.));
/// `text-[11px] leading-4`.
const META: (Pixels, Pixels) = (px(11.), px(16.));
/// `pb-4`: every card row's foot.
const FOOT: Pixels = px(16.);

/// The row's foot, `pb-4`; none where the next row hangs on the same rail, which the card pads as a foot cannot see it.
pub fn foot(kind: &RowKind) -> Pixels {
    match kind {
        RowKind::Subagent { .. } | RowKind::Threads { .. } | RowKind::Rule => px(0.),
        RowKind::Permission { asker: Some(_), .. } | RowKind::Question { asker: Some(_), .. } => px(0.),
        _ => FOOT,
    }
}

/// Whether the row after `ix` is a prompt a subagent raised, which hangs on the same fold's rail.
fn rail_follows(view: &ThreadView, ix: usize) -> bool {
    matches!(
        view.rows().get(ix + 1).map(|r| &r.kind),
        Some(RowKind::Permission { asker: Some(_), .. } | RowKind::Question { asker: Some(_), .. })
    )
}

pub fn card(view: &mut ThreadView, ix: usize, row: &Row, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let p = cx.palette().clone();
    match &row.kind {
        RowKind::Permission { .. } | RowKind::Question { .. } => prompt(view, ix, row, &p, window, cx),
        RowKind::Plan { title, open } => plan(view, row, title, *open, &p, window, cx),
        RowKind::Subagent { id, title, state, open, lines } => {
            let page = subagent_page(view, id);
            subagent(view, id, title, state.as_deref(), *open, lines, rail_follows(view, ix), page, &p, cx)
        }
        RowKind::Spawn { children } => spawn(view, &row.key, children, &p, cx),
        RowKind::ChangedFiles { label, files, others, .. } => {
            changed_files(&row.key, label, files, others, row.turn.as_deref(), &p, window, cx)
        }
        RowKind::Fork { open, .. } => fork(&row.text, open.clone(), &p, cx),
        RowKind::StartFailed { failed } => start_failed(view, row, failed, &p, window, cx),
        RowKind::Threads { open, .. } => threads(&row.key, *open, view.listing(), &p, window, cx),
        RowKind::Rule if row.key == transcript::PAUSED_KEY => {
            let wake = wake_button("workspace-wake", view.followed_workspace().or(view.listing().and_then(|l| l.workspace.as_deref())));
            rule_line("workspace-paused", &row.text, wake, p.muted_foreground)
        }
        RowKind::Rule => rule_line("machine-line", &row.text, None, p.muted_foreground),
        RowKind::Slate => slate_line(&row.key, &row.text, &p, window, cx),
        _ => div().into_any_element(),
    }
}

/// A ghost act at `xs`: 24 px high, a 12 px glyph, the muted word, and where `live` the foreground under the pointer.
fn ghost_xs(id: SharedString, g: Glyph, label: Option<&'static str>, live: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    h_flex()
        .id(id)
        .flex_none()
        .justify_center()
        .h(px(24.))
        .when(label.is_none(), |el| el.w(px(24.)))
        .when(label.is_some(), |el| el.px(px(8.)))
        .gap(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(gpui_kit::transparent_black())
        .text_size(XS.0)
        .line_height(XS.1)
        .text_color(p.muted_foreground)
        .when(live, |el| el.cursor_pointer().hover(|s| s.bg(p.accent).text_color(p.foreground)))
        .child(Icon::new(g).with_size(px(12.)).text_color(p.muted_foreground))
        .children(label)
}

/// What `ink` at `share` over `ground` shows, as one solid color.
fn mix_over(ink: Hsla, share: f32, ground: Hsla) -> Hsla {
    ground.blend(ink.opacity(share))
}

#[cfg(test)]
mod tests;
