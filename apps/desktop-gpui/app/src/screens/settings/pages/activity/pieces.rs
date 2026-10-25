// SPDX-License-Identifier: AGPL-3.0-only
//! The grid's cells for a row of Activity's own, laid as the grid lays its rows.

use gpui_kit::component::h_flex;
use gpui_kit::{prelude::*, px, AnyElement, Pixels};

use crate::ui::Col;

/// A grid's column gap, `gap-x-4`.
pub const COLUMN_GAP: Pixels = px(16.);

/// The cell boxes of a template, as the grid's own rows lay them: a fill column takes what is left.
pub fn cells(columns: &[Col], items: Vec<AnyElement>) -> Vec<AnyElement> {
    items
        .into_iter()
        .enumerate()
        .map(|(ix, el)| {
            let cell = h_flex().min_w_0();
            match columns.get(ix) {
                Some(Col::Px(w)) => cell.flex_none().w(px(*w)),
                _ => cell.flex_1(),
            }
            .child(el)
            .into_any_element()
        })
        .collect()
}
