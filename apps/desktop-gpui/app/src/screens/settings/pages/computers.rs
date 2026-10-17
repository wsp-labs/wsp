// SPDX-License-Identifier: AGPL-3.0-only
//! Computers and each computer's page (`computers.tsx`, `computerSettings.tsx`).

mod glyph;
mod image;
mod list;
mod page;
mod parts;
mod remove;
mod rows;
mod sign_ins;
mod words;

use gpui_kit::{AnyElement, App, SharedString, Window};

use crate::screens::settings::model::At;
use crate::screens::settings::page::{CardData, Ctx};

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    list::cards(ctx, window, cx)
}

pub fn page(id: &str, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    page::page(id, ctx, window, cx)
}

/// The pages under Computers in the sidebar: one per computer the list draws, off the same list.
pub fn sub_pages(ctx: &Ctx) -> Vec<(At, SharedString)> {
    ctx.data.places.iter().map(|place| (At::Computer(place.id.clone()), place_name(place).into())).collect()
}

/// A computer's own icon as its sidebar row draws it (`SubComputerGlyph`): 16 px in the sidebar's icon ink.
pub fn glyph(id: &str, ctx: &Ctx, cx: &App) -> Option<AnyElement> {
    use crate::ui::ActivePalette as _;
    let place = ctx.data.places.iter().find(|p| p.id == id)?;
    Some(glyph::glyph_in(place, ctx.data.preferences(), cx.palette().sidebar_icon_color))
}

/// A computer's name as every page says it (`placeName`).
pub fn place_name(place: &wsp_host_client::protocol::PlaceView) -> String {
    words::place_name(place)
}
