// SPDX-License-Identifier: AGPL-3.0-only
//! Computers and each computer's page (`computers.tsx`, `computerSettings.tsx`).

mod ansi;
mod image;
mod list;
mod live;
mod page;
mod remove;
mod rows;
mod setup;
mod sign_ins;
mod words;

pub use words::{is_provider, js_number, provider_key_name, ProviderWords, PROVIDERS};

use gpui_kit::{AnyElement, App, SharedString, Window};

pub use image::words::copy_build_of;
pub use list::{icon_items, RowMenu, ADD_COMPUTER_LATER};

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
    Some(crate::ui::ComputerIcon::of(place, ctx.data.preferences()).draw(cx.palette().sidebar_icon_color, crate::ui::size::GLYPH))
}

/// A computer's name as every page says it (`placeName`).
pub fn place_name(place: &wsp_host_client::protocol::PlaceView) -> String {
    words::place_name(place)
}
