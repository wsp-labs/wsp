// SPDX-License-Identifier: AGPL-3.0-only
//! Recipes (`recipes.tsx`): the list, one row per recipe with the icon the person picked from the projects' set, what
//! it holds and the computers following it; a recipe open as its head row with the icon picker, its name and its
//! delete; and the empty card with the act under it.

use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, App, Entity, SharedString, Subscription, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{RecipeView, RecipesRemoveRequest, RecipesSaveRequest};

use super::devices::CANCEL;
use super::projects::look;
use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{
    chevron, glyph, glyph_frame, text, ActivePalette, AlertDialog, Button, Col, Field, Grid, GridHead, GridName, GridRow, HeadCell,
    HeadRow, Refusal, Select,
};

/// `RECIPE_COLUMNS`: the name, the computers, the chevron.
const RECIPE_COLUMNS: &[Col] = &[Col::Fill, Col::Px(140.), Col::Px(14.)];
const ADD_COMPUTER: &str = "Add a computer";
const NONE: &str = "No recipes yet. Save one at the end of Add a computer.";
const DELETE: &str = "Delete";

/// The icon a recipe wears: the person's pick, else the folder every project starts with.
fn icon_of(ctx: &Ctx, slug: &str) -> String {
    ctx.data.preferences().and_then(|p| p.recipe_look.get(slug)).map_or_else(|| "folder".to_owned(), |look| look.icon.to_string())
}

fn has_look(ctx: &Ctx, slug: &str) -> bool {
    ctx.data.preferences().is_some_and(|p| p.recipe_look.contains_key(slug))
}

/// What deleting a recipe leaves, said on its row and in its confirm.
fn leaving(recipe: &RecipeView) -> String {
    if recipe.machines.is_empty() {
        "Nothing follows it.".to_owned()
    } else {
        format!("{} keep what they have and follow nothing.", recipe.machines.join(" and "))
    }
}

/// The Add a computer press under the list; this app draws no Add a computer dialog, so the press says so.
fn add_computer(ctx: &Ctx) -> Button {
    let acts = ctx.acts.clone();
    Button::add("add-computer-button", ADD_COMPUTER).on_click(move |_, _, cx| acts.note("Add a computer is not in this app yet.", cx))
}

/// The recipe open in place of the list, by its slug, and the host's refusal of the last write to it.
#[derive(Default)]
struct Open {
    slug: Option<String>,
    refused: Option<String>,
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let Some(recipes) = ctx.data.recipes.as_ref() else { return Vec::new() };
    let open = window.use_keyed_state("recipes-open", cx, |_, _| Open::default());
    let shown = open.read(cx).slug.as_ref().and_then(|slug| recipes.iter().find(|r| r.slug == *slug));
    if let Some(recipe) = shown {
        return recipe_page(recipe, &open, ctx, window, cx);
    }
    if recipes.is_empty() {
        return vec![CardData::new("recipes").item(Item::empty("none", NONE)).under(add_computer(ctx))];
    }
    let p = cx.palette().clone();
    let rows = recipes.iter().map(|recipe| {
        let (state, slug) = (open.clone(), recipe.slug.clone());
        let acts = ctx.acts.clone();
        GridRow::new(SharedString::from(format!("recipe-{}", recipe.slug)), RECIPE_COLUMNS)
            .on_open(move |_, _, cx| {
                state.update(cx, |o, _| *o = Open { slug: Some(slug.clone()), refused: None });
                acts.notify(cx);
            })
            .cell(
                GridName::new(glyph_frame(look::glyph_of(&icon_of(ctx, &recipe.slug)), cx), recipe.name.clone())
                    .note(recipe.summary.clone()),
            )
            .cell(div().min_w_0().truncate().text_size(text::FACT).text_color(p.muted_foreground).child(recipe.machines.join(", ")))
            .cell(chevron(cx))
    });
    let grid = Grid::new("recipes")
        .head(GridHead::new(Some(RECIPE_COLUMNS), vec![HeadCell::new("Recipe"), HeadCell::new("Computers")]))
        .rows(rows);
    vec![CardData::new("recipes").body(v_flex().gap_2().child(grid).child(h_flex().child(add_computer(ctx))))]
}

/// The name field, its rename on Enter or on leaving it.
struct Name {
    field: Entity<InputState>,
    _events: Subscription,
}

fn recipe_page(recipe: &RecipeView, open: &Entity<Open>, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let p = cx.palette().clone();
    let icon = icon_of(ctx, &recipe.slug);
    let followed = !recipe.machines.is_empty();
    let on = recipe.machines.join(" and ");

    let icon_select = {
        let (acts, slug) = (ctx.acts.clone(), recipe.slug.clone());
        let state = look::select_state(
            format!("recipe-{}-icon", recipe.slug).into(),
            || look::icon_items("neutral"),
            String::new(),
            &icon,
            window,
            cx,
        );
        Select::new(&state).on_change(move |next: &String, _, cx| {
            acts.set_preferences(json!({ "recipeLook": { slug.clone(): { "icon": next } } }), cx);
        })
    };
    let line = if followed { format!("Followed by {on}.") } else { "No computer follows it yet.".to_owned() };
    let head = HeadRow::new("recipe-head", glyph(look::glyph_of(&icon), cx), recipe.name.clone()).line(line).slot(icon_select);
    let reach = format!("Ticks here reach {}.", if followed { on.as_str() } else { "every computer that follows it" });
    let mut cards = vec![CardData::new("recipe")
        .item(Item::any("recipe-head", vec![recipe.name.clone().into()], head))
        .under(div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(reach))];

    if let Some(said) = open.read(cx).refused.clone() {
        cards.push(CardData::new("recipe-refused").body(Refusal::new().said(said)));
    }

    let name = name_field(recipe, open, ctx, window, cx);
    name.update(cx, |field, cx| field.set_readonly(followed, cx));
    cards.push(CardData::new("rename").item(Item::row("name", "Name", None, |row| row.control(Field::new(&name)))));

    let leaving = leaving(recipe);
    let delete = delete_control(recipe, open, ctx, leaving.clone());
    cards.push(
        CardData::new("delete-line")
            .item(Item::row("delete", format!("Delete {}", recipe.name), Some(&leaving), |row| row.control(delete))),
    );
    cards
}

/// The recipe's name, kept while its page is drawn; a new name moves the recipe's file, which only a recipe no
/// computer follows may do, since the computers name the file.
fn name_field(recipe: &RecipeView, open: &Entity<Open>, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Entity<InputState> {
    let (recipe, open, acts) = (recipe.clone(), open.clone(), ctx.acts.clone());
    let look = has_look(ctx, &recipe.slug).then(|| icon_of(ctx, &recipe.slug));
    let held = window.use_keyed_state(SharedString::from(format!("recipe-{}-name", recipe.slug)), cx, move |window, cx| {
        let field = cx.new(|cx| InputState::new(window, cx).default_value(recipe.name.clone()));
        let events = cx.subscribe(&field, move |_, field, event: &InputEvent, cx| {
            if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let to = field.read(cx).value().trim().to_owned();
            if to.is_empty() || to == recipe.name || !recipe.machines.is_empty() {
                return;
            }
            rename(&recipe, to, look.clone(), &open, &acts, cx);
        });
        Name { field, _events: events }
    });
    held.read(cx).field.clone()
}

/// Saves the recipe under its new name, then takes the old file away and carries its icon over.
fn rename(recipe: &RecipeView, to: String, look: Option<String>, open: &Entity<Open>, acts: &crate::screens::settings::Acts, cx: &mut App) {
    let mut file = serde_json::to_value(&recipe.file).unwrap_or(Value::Null);
    file["name"] = Value::String(to.clone());
    let (old, open, acts) = (recipe.slug.clone(), open.clone(), acts.clone());
    acts.clone().call(req::<RecipesSaveRequest>(json!({ "name": to, "file": file })), cx, move |answer, cx| match answer {
        Ok(saved) => {
            let then = acts.clone();
            acts.call(req::<RecipesRemoveRequest>(json!({ "name": old.clone() })), cx, move |dropped, cx| {
                if let Err(said) = dropped {
                    open.update(cx, |o, _| o.refused = Some(said));
                }
                if let Some(icon) = look {
                    then.set_preferences(json!({ "recipeLook": { saved.recipe.slug: { "icon": icon }, old: null } }), cx);
                }
                then.reread(Reread::Recipes, cx);
            });
        }
        Err(said) => {
            open.update(cx, |o, _| o.refused = Some(said));
            acts.notify(cx);
        }
    });
}

/// Delete: asks, then takes the recipe away, its icon with it, and goes back to the list.
fn delete_control(recipe: &RecipeView, open: &Entity<Open>, ctx: &Ctx, leaving: String) -> impl IntoElement {
    let acts = ctx.acts.clone();
    let (slug, name, open) = (recipe.slug.clone(), recipe.name.clone(), open.clone());
    let look = has_look(ctx, &recipe.slug);
    Button::danger("delete", DELETE).xs().on_click(move |_, _, cx| {
        let (slug, name, leaving, open) = (slug.clone(), name.clone(), leaving.clone(), open.clone());
        acts.open_dialog(
            Rc::new(move |ctx, _, _| {
                let cancel = {
                    let acts = ctx.acts.clone();
                    Button::outline("delete-cancel", CANCEL).neutral_ring().on_click(move |_, _, cx| acts.close_dialog(cx))
                };
                let (acts, slug, open) = (ctx.acts.clone(), slug.clone(), open.clone());
                let confirm = Button::destructive("delete-confirm", DELETE).on_click(move |_, _, cx| {
                    let (acts, slug, open) = (acts.clone(), slug.clone(), open.clone());
                    acts.clone().call(req::<RecipesRemoveRequest>(json!({ "name": slug.clone() })), cx, move |answer, cx| {
                        acts.close_dialog(cx);
                        match answer {
                            Ok(_) => {
                                if look {
                                    acts.set_preferences(json!({ "recipeLook": { slug: null } }), cx);
                                }
                                acts.reread(Reread::Recipes, cx);
                                open.update(cx, |o, _| *o = Open::default());
                            }
                            Err(said) => open.update(cx, |o, _| o.refused = Some(said)),
                        }
                        acts.notify(cx);
                    });
                });
                AlertDialog::new(format!("Delete {name}?")).sentence(leaving.clone()).act(cancel).act(confirm).into_any_element()
            }),
            cx,
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(machines: &[&str]) -> RecipeView {
        serde_json::from_value(json!({ "name": "Rust box", "slug": "rust-box", "summary": "claude, codex", "machines": machines, "savedAt": "2026-10-17T00:00:00Z", "file": { "name": "Rust box" } })).unwrap()
    }

    #[test]
    fn deleting_says_what_follows_it() {
        assert_eq!(leaving(&recipe(&[])), "Nothing follows it.");
        assert_eq!(leaving(&recipe(&["box", "mini"])), "box and mini keep what they have and follow nothing.");
    }
}
