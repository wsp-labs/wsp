// SPDX-License-Identifier: AGPL-3.0-only
//! The surface grain (`surface-grain` in index.css): one noise tile repeated under a surface's content, which lifts a
//! dark ground by a few levels and is part of every surface the web draws it on.

use gpui_kit::{div, img, prelude::*, px, Pixels, Size};

use super::icons::GRAIN;

const TILE: Pixels = px(256.);

/// The grain over a box of `size`, laid behind the content of a `relative()` parent.
pub fn grain(size: Size<Pixels>) -> impl IntoElement {
    let (cols, rows) = ((size.width / TILE).ceil() as usize, (size.height / TILE).ceil() as usize);
    div().absolute().top_0().left_0().size_full().overflow_hidden().children(
        (0..rows)
            .flat_map(move |row| (0..cols).map(move |col| img(GRAIN).absolute().left(TILE * col as f32).top(TILE * row as f32).size(TILE))),
    )
}
