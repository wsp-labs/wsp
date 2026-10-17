// SPDX-License-Identifier: AGPL-3.0-only
//! Every page of Settings, one module each, each a plain function of `Ctx` that states its cards as data.

pub mod about;
pub mod account;
pub mod activity;
pub mod agents;
pub mod appearance;
pub mod computers;
pub mod devices;
pub mod general;
pub mod keybindings;
pub mod privacy;
pub mod projects;
pub mod recipes;

use gpui_kit::{AnyElement, App, Window};

use super::model::{At, GroupId};
use super::page::{CardData, Ctx};

/// A group's page as cards.
pub fn group_cards(id: GroupId, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    match id {
        GroupId::General => {
            let mut cards = general::cards(ctx, window, cx);
            cards.extend(about::cards(ctx, window, cx));
            cards
        }
        GroupId::Appearance => appearance::cards(ctx, window, cx),
        GroupId::Agents => agents::cards(ctx, window, cx),
        GroupId::Computers => computers::cards(ctx, window, cx),
        GroupId::Recipes => recipes::cards(ctx, window, cx),
        GroupId::Projects => projects::cards(ctx, window, cx),
        // Usage has its own screen and ticket; its group stands in the sidebar.
        GroupId::Usage => Vec::new(),
        GroupId::Activity => activity::cards(ctx, window, cx),
        GroupId::Keybindings => keybindings::cards(ctx, window, cx),
        GroupId::Privacy => privacy::cards(ctx, window, cx),
        GroupId::Devices => devices::cards(ctx, window, cx),
        GroupId::Account => account::cards(ctx, window, cx),
    }
}

/// Any page as cards: a group's, or a computer's, a project's or an agent's own.
pub fn cards(at: &At, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    match at {
        At::Group(id) => group_cards(*id, ctx, window, cx),
        At::Computer(id) => computers::page(id, ctx, window, cx),
        At::Project(id) => projects::page(id, ctx, window, cx),
        At::Agent(id) => agents::page(id, ctx, window, cx),
    }
}

/// The glyph a page under a group wears on its sidebar row: a project's, a computer's, an agent's mark.
pub fn sub_glyph(at: &At, ctx: &Ctx, cx: &App) -> Option<AnyElement> {
    match at {
        At::Agent(id) => agents::glyph(id, ctx, cx),
        At::Project(id) => projects::glyph(id, ctx, cx),
        At::Computer(id) => computers::glyph(id, ctx, cx),
        At::Group(_) => None,
    }
}

/// A control at the top bar's right while a group's page is open, where the page splits into tabs.
pub fn head(id: GroupId, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match id {
        GroupId::Activity => activity::head(ctx, window, cx),
        _ => None,
    }
}
