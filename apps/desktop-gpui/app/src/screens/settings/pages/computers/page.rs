// SPDX-License-Identifier: AGPL-3.0-only
//! One computer's or cloud's own page, after `ComputerPage` in `computers.tsx`: its head with the state and the one
//! act it invites, the recipe it follows, its setup, its sign-ins, what runs there, its limits, its threads, its
//! agents, your image on it, its name and login, and Remove.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{PlaceView, PlacesDialRequest};

use super::list::{here_absence, place_state, running, state_of};
use super::remove::{call_whole, page_place, parting, remove_line};
use super::rows::{agents_link, behind_row, limits_card, name_card, recipe_card, spawn_card};
use super::words::{
    self, capitalised, cloud_plan_line, clouds_offered, dial_refused, dial_road, is_here, is_provider, key_held, limit_columns, limit_note,
    name_list, page_words as w, place_name, place_word, setup_word, setup_words, shape_line, size_name, sizes_sold, starts_today_line,
    state_words, where_words, Cell, DialRoad,
};
use crate::screens::settings::model::{req, At, GroupId, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{size, text, ActivePalette, Button, HeadRow, VersionFact};

/// `NEAR_LIMIT`: the share from which a bar takes the warning ink.
const NEAR_LIMIT: f64 = 0.9;

/// `PlanLines`: a cloud's plan, its credit and spend, and today's starts.
fn plan_lines(place: &PlaceView) -> Option<AnyElement> {
    let names: Vec<String> = sizes_sold(place).iter().map(|s| size_name(s.name.as_deref(), s.cpu, s.mem_mb)).collect();
    let plan = cloud_plan_line(place, &names)?;
    Some(
        v_flex()
            .child(div().child(plan))
            .children(limit_note(place).map(|n| div().child(n)))
            .children(starts_today_line(place).map(|s| div().child(s)))
            .into_any_element(),
    )
}

/// `LimitBand`: one column per limit on a grid of four, two under 768 px, its figure in the mono and a thin bar under
/// it, and how starts are counted.
fn limit_band(place: &PlaceView, narrow: bool, cx: &App) -> Option<AnyElement> {
    let columns = limit_columns(place);
    if columns.is_empty() {
        return None;
    }
    let p = cx.palette().clone();
    let mono = gpui_kit::component::ActiveTheme::theme(cx).mono_font_family.clone();
    let per = if narrow { 2 } else { 4 };
    let mut cols: Vec<AnyElement> = columns
        .into_iter()
        .map(|(k, label, used, limit)| {
            let share = if limit > 0 { used as f64 / limit as f64 } else { 0. };
            let (fill, ink) = if share >= 1. {
                (p.destructive, p.error_foreground)
            } else if share >= NEAR_LIMIT {
                (p.warning, p.warning_foreground)
            } else {
                (p.foreground.opacity(0.55), p.foreground)
            };
            v_flex()
                .id(SharedString::from(format!("limit-{k}")))
                .debug_selector(move || format!("limit-{k}"))
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().truncate().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(label))
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(text::XS.0)
                        .line_height(text::XS.1)
                        .text_color(ink)
                        .child(words::limit_words::of(used, limit)),
                )
                .child(
                    div()
                        .mt_1()
                        .h_1()
                        .w_full()
                        .rounded_full()
                        .bg(p.foreground.opacity(0.1))
                        .child(div().h_full().rounded_full().bg(fill).w(relative(share.min(1.) as f32))),
                )
                .into_any_element()
        })
        .collect();
    // `grid-cols-4 gap-x-6 gap-y-4`: a short last line keeps its columns' widths.
    cols.extend((0..(per - cols.len() % per) % per).map(|_| div().flex_1().min_w_0().into_any_element()));
    let mut lines = Vec::new();
    let mut cols = cols.into_iter();
    loop {
        let line: Vec<AnyElement> = cols.by_ref().take(per).collect();
        if line.is_empty() {
            break;
        }
        lines.push(h_flex().items_start().gap_x_6().children(line));
    }
    Some(
        div()
            .px(size::CARD_INSET)
            .pb_4()
            .child(
                v_flex()
                    .debug_selector(|| "limit-band".into())
                    .mt_3()
                    .mb_1()
                    .gap_3()
                    .pl(px(44.))
                    .child(v_flex().debug_selector(|| "limit-columns".into()).gap_y_4().children(lines))
                    .when(place.starts.is_some(), |el| {
                        el.child(
                            div()
                                .text_size(text::XS.0)
                                .line_height(text::XS.1)
                                .text_color(p.muted_foreground)
                                .child(words::limit_words::COUNTED),
                        )
                    }),
            )
            .into_any_element(),
    )
}

/// What a dial came to: busy while it waits, the host's own line where it answered, its refusal where it did not.
#[derive(Default)]
struct Dial {
    busy: bool,
    line: Option<String>,
    refused: Option<String>,
}

/// `PlaceStateLine`: the state as its word or its act, the sentence beside it, and while the computer is not
/// answering the dial beside that, whose answer takes the sentence's place and whose refusal stands under it.
fn state_line(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let cell = state_of(place, here_absence(ctx.data).as_ref());
    let behind_said = matches!(&cell, Cell::Update { .. }) || matches!(&cell, Cell::Word { word, .. } if word == state_words::BEHIND);
    if place.behind.is_some() && !is_provider(place) && behind_said {
        return None;
    }
    let dial = window.use_keyed_state(SharedString::from(format!("computer-{}-dial", place.id)), cx, |_, _| Dial::default());
    let (busy, line, refused) = {
        let d = dial.read(cx);
        (d.busy, d.line.clone(), d.refused.clone())
    };
    let away = !is_here(place) && words::absent_of(place).is_some();
    let kept = if away { dial_refused(place) } else { None };
    let sentence = if away { line.or(kept).or_else(|| cell.why().map(str::to_owned)) } else { cell.why().map(str::to_owned) };
    let road = dial_road(place);
    let p = cx.palette().clone();
    let state = div().flex_none().child(place_state(ctx, place, &cell, false, window, cx));
    let mut row = h_flex().id("place-state").min_w_0().gap(px(10.)).text_size(text::NOTE.0).line_height(text::NOTE.1).child(state);
    if let Some(sentence) = sentence.filter(|s| !s.is_empty()) {
        row = row.child(
            div()
                .id("place-sentence")
                .min_w_0()
                .truncate()
                .text_color(p.muted_foreground)
                .tooltip(crate::ui::tip(sentence.clone()))
                .child(sentence),
        );
    }
    if let (true, Some(road)) = (away, road) {
        let word = if busy {
            where_words::DIALLING
        } else {
            match road {
                DialRoad::Link => where_words::DIAL_LINK,
                DialRoad::Ssh => where_words::DIAL_SSH,
            }
        };
        let (acts, id) = (ctx.acts.clone(), place.id.clone());
        row = row.child(Button::outline("dial", word).xs().held(busy).on_click(move |_, _, cx| {
            dial.update(cx, |d, _| {
                d.busy = true;
                d.line = None;
                d.refused = None;
            });
            let (acts2, dial) = (acts.clone(), dial.clone());
            call_whole(&acts, req::<PlacesDialRequest>(json!({ "placeId": id })), cx, move |answer, cx| {
                dial.update(cx, |d, _| {
                    d.busy = false;
                    match answer {
                        Ok(answer) => d.line = Some(answer.line),
                        Err(failure) => d.refused = Some(failure.message),
                    }
                });
                acts2.reread(Reread::Places, cx);
            });
            acts.notify(cx);
        }));
    }
    Some(v_flex().w_full().gap_2().child(row).children(refused.map(|said| crate::ui::Refusal::new().said(said))).into_any_element())
}

/// The head card: the computer's head row with its band or its behind row, and the state line under the surface.
fn head_card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> CardData {
    let cloud = is_provider(place);
    let name = place_name(place);
    let mut head =
        HeadRow::new("computer-head", crate::ui::computer_glyph::computer_glyph(place, ctx.data.preferences(), cx), name.clone());
    if let Some(fact) = words::version_fact(place, ctx.data.host_version.as_deref()) {
        head = head.mark(VersionFact::new(fact.into_iter().map(SharedString::from).collect()));
    }
    if cloud && place.plan.is_some() {
        if let Some(lines) = plan_lines(place) {
            head = head.line(lines);
        }
    } else {
        let shape = shape_line(place);
        if !shape.is_empty() {
            head = head.line(shape);
        }
    }
    let under: Option<AnyElement> = if cloud {
        place.plan.as_ref().and_then(|_| limit_band(place, crate::ui::narrow::width(window).md, cx))
    } else {
        behind_row(ctx, place, window, cx).map(|row| row.into_any_element())
    };
    let surface = v_flex().w_full().child(head).children(under);
    let mut card = CardData::new("computer").item(Item::any("computer-head", vec![name.into()], crate::ui::CardItem::any(surface)));
    if let Some(line) = state_line(ctx, place, window, cx) {
        card = card.under(line);
    }
    card
}

/// `SetupCard`: the whole setup while it runs or once it ended failed; once it ended otherwise, one row with the log
/// behind Show setup log.
fn setup_card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let setup = place.setup.as_ref()?;
    let standing = match setup.state.to_string().as_str() {
        "running" => "running",
        _ => {
            let word = place_word(place, None).word;
            if word == setup_words::FAILED {
                "failed"
            } else if word == setup_words::NEEDS_YOU {
                "needs-you"
            } else {
                "ready"
            }
        }
    };
    let live = standing == "running" || standing == "failed";
    let shown_state = window.use_keyed_state(SharedString::from(format!("computer-{}-setup-log", place.id)), cx, |_, _| false);
    let shown = *shown_state.read(cx);
    let mut card = CardData::new("setup").head(where_words::HEAD_SETUP);
    if !live {
        let rows = super::setup::setup_rows(&serde_json::to_value(place).unwrap_or_default(), &place_name(place));
        let left: Vec<String> = super::setup::rows_left(&rows, super::setup::waiting_on_you).iter().map(|r| r.name.clone()).collect();
        let note = if left.is_empty() {
            capitalised(&setup_word(Some(setup), place.applied.as_ref()))
        } else if standing == "needs-you" {
            name_list(&left)
        } else {
            w::setup_failed(&name_list(&left))
        };
        let word = if standing == "needs-you" { setup_words::NEEDS_YOU } else { w::SETUP_DONE };
        let acts = ctx.acts.clone();
        card = card.item(Item::row("setup-summary", w::SETUP_TITLE, Some(&note), move |row| {
            row.fact(word).control(Button::outline("setup-log", if shown { w::HIDE_LOG } else { w::SHOW_LOG }).xs().on_click(
                move |_, _, cx| {
                    shown_state.update(cx, |s, _| *s = !*s);
                    acts.notify(cx);
                },
            ))
        }));
    }
    if live || shown {
        card = card.under(super::setup::list(ctx, place, window, cx));
    }
    Some(card)
}

/// `ActivityLink`: Activity scoped to this computer, its line off what the page already holds.
fn activity_link(ctx: &Ctx, place: &PlaceView) -> CardData {
    let (acts, pick, id) = (ctx.acts.clone(), ctx.activity.clone(), place.id.clone());
    let line = w::activity_line(running(place));
    CardData::new("computer-activity").item(Item::row("activity-on", w::ACTIVITY_ROW, Some(&line), move |row| {
        row.on_open(move |_, window, cx| {
            pick.update(cx, |p, cx| {
                p.pick_place(id.clone());
                cx.notify();
            });
            acts.go(At::Group(GroupId::Activity), window, cx)
        })
    }))
}

pub fn page(id: &str, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let parting = parting(window, cx);
    let Some(place) = page_place(ctx, id, &parting, cx) else { return Vec::new() };
    let place = &place;
    let here = is_here(place);
    let cloud = is_provider(place);
    let keys = ctx.data.setup.as_ref().map(|s| &s.keys);
    let held = if cloud { key_held(&place.name, keys) } else { !clouds_offered(keys).is_empty() };
    let mut cards = vec![head_card(ctx, place, window, cx)];
    if !cloud && place.picks.is_some() {
        cards.push(recipe_card(ctx, place, window, cx));
    }
    if !cloud {
        cards.extend(setup_card(ctx, place, window, cx));
    }
    if !cloud && !here {
        cards.extend(super::sign_ins::card(ctx, place, window, cx));
    }
    if super::super::activity::lists_computer(ctx, place, window, cx) {
        cards.push(activity_link(ctx, place));
    }
    cards.extend(limits_card(ctx, place, window, cx));
    if !cloud {
        cards.extend(spawn_card(ctx, place, window, cx));
    }
    if cloud {
        if held {
            cards.extend(super::image::agents_lists(ctx, place, window, cx));
        }
    } else {
        cards.push(agents_link(ctx, place));
    }
    if held {
        cards.extend(super::image::image_row(ctx, place, window, cx));
    }
    cards.extend(name_card(ctx, place));
    if !here {
        cards.push(remove_line(ctx, place, &parting, crate::ui::narrow::width(window).sm, cx));
    }
    cards
}
