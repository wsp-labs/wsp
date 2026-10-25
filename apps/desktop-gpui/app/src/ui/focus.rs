// SPDX-License-Identifier: AGPL-3.0-only
//! Keyboard focus as the web draws it (`focus-visible:ring-2`): a 2 px ring outside a focused button, switch, pick or
//! segment in `--ring`, inset on a row that opens a page, and the muted ink on a destructive dialog's buttons. gpui-kit's
//! own ring is 3 px at half strength, so wsp draws its own. A piece takes focus with `.tab_index(0)`, which Tab reaches.

use gpui_kit::{point, px, BoxShadow, Hsla};

/// `ring-2`: the 2 px ring outside the piece's edge.
pub fn ring(color: Hsla) -> BoxShadow {
    BoxShadow { color, offset: point(px(0.), px(0.)), blur_radius: px(0.), spread_radius: px(2.), inset: false }
}

/// `ring-2 ring-inset`: the same ring inside the edge, for a row that fills its card.
pub fn ring_inset(color: Hsla) -> BoxShadow {
    BoxShadow { inset: true, ..ring(color) }
}
