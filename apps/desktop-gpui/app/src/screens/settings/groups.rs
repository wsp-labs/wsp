// SPDX-License-Identifier: AGPL-3.0-only
//! The one table of settings groups (`groups.ts`): each group's name, glyph, the defaults it can restore, the pages
//! under it and the word at its sidebar row's edge. The sidebar, the search, the crumbs and Restore defaults read it.

use gpui_kit::SharedString;
use serde_json::Value;
use wsp_host_client::protocol::Preferences;

use super::model::{At, GroupId};
use super::page::Ctx;
use super::pages;
use crate::ui::Glyph;

/// The defaults a group's rows can be put back to: whether any row is off them, and the one patch that restores them.
pub struct Restore {
    pub off: fn(&Preferences) -> bool,
    pub patch: fn() -> Value,
}

pub struct Group {
    pub id: GroupId,
    pub name: &'static str,
    pub glyph: Glyph,
    pub restore: Option<Restore>,
}

pub fn group(id: GroupId) -> Group {
    let (name, glyph, restore) = match id {
        GroupId::General => ("General", Glyph::SlidersHorizontal, None),
        GroupId::Appearance => {
            ("Appearance", Glyph::Palette, Some(Restore { off: pages::appearance::off_defaults, patch: pages::appearance::defaults }))
        }
        GroupId::Agents => ("Agents", Glyph::Bot, None),
        GroupId::Computers => ("Computers", Glyph::Monitor, None),
        GroupId::Recipes => ("Recipes", Glyph::ListChecks, None),
        GroupId::Projects => ("Projects", Glyph::Folder, None),
        GroupId::Usage => ("Usage", Glyph::Gauge, None),
        GroupId::Activity => ("Activity", Glyph::Activity, None),
        GroupId::Keybindings => {
            ("Keybindings", Glyph::Keyboard, Some(Restore { off: pages::keybindings::off_defaults, patch: pages::keybindings::defaults }))
        }
        GroupId::Privacy => {
            ("Privacy", Glyph::Shield, Some(Restore { off: pages::privacy::off_defaults, patch: pages::privacy::defaults }))
        }
        GroupId::Devices => ("Devices", Glyph::Smartphone, None),
        GroupId::Account => ("Account", Glyph::User, None),
    };
    Group { id, name, glyph, restore }
}

/// The pages under a group in the sidebar, where the group lists nouns that each have a page of their own.
pub fn sub_pages(id: GroupId, ctx: &Ctx) -> Vec<(At, SharedString)> {
    match id {
        GroupId::Computers => pages::computers::sub_pages(ctx),
        GroupId::Projects => pages::projects::sub_pages(ctx),
        GroupId::Agents => pages::agents::sub_pages(ctx),
        _ => Vec::new(),
    }
}

/// One mono word at the right edge of the group's sidebar row, where the group has a state worth a glance.
pub fn meta(id: GroupId, ctx: &Ctx) -> Option<SharedString> {
    match id {
        GroupId::General => pages::about::meta(ctx),
        _ => None,
    }
}
