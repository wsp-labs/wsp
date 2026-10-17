// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Computers, after `computers.tsx`'s `computersCards`: the computers this wsp runs on, the adds that have
//! not reached Set up, and the clouds that lend it machines, each list on `LIST_COLUMNS` so the state column is one
//! line down the page, each row opening its own page.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, AnyElement, App, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{PendingComputer, PlaceView, PlacesUpdateRequest, StatusSubscribeRequest};

use super::glyph::computer_glyph;
use super::parts::state_mark;
use super::words::{
    self, add_words, built_when, clouds_offered, fmt_mem_gb, is_provider, names_place, now_ms, pending_word, place_name, state_cell,
    state_words, version_fact, where_words, Absent, Cell, Mark,
};
use crate::screens::settings::model::{req, At, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::grid::LIST_COLUMNS;
use crate::ui::{
    chevron, framed, glyph_frame, num, text, ActivePalette, Button, Glyph, Grid, GridHead, GridName, GridRow, HeadCell, StateCell,
};

/// The computer the host runs on is never read as absent here: its daemon's reach rides its workspace's status,
/// which these pages do not hold.
pub fn state_of(place: &PlaceView, here: Option<&Absent>) -> Cell {
    let absent = if words::is_here(place) { here.cloned() } else { words::absent_of(place) };
    state_cell(place, absent.as_ref(), !words::is_here(place))
}

/// The computer the host runs on, read off its own workspace's daemon (`absenceAt`): its terminals and files
/// stopped while that daemon is silent, and nothing before the host's status snapshot has answered.
pub fn here_absence(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<Absent> {
    let read = window.use_keyed_state("computers-here-reach", cx, |_, _| None::<Option<(String, bool)>>);
    if read.read(cx).is_none() {
        let asked = window.use_keyed_state("computers-here-reach-asked", cx, |_, _| false);
        // Asked only once the host is there: an ask before it answers "no host" and would never be asked again.
        if !*asked.read(cx) && ctx.acts.host(cx).is_some() {
            asked.update(cx, |a, _| *a = true);
            let (acts, read) = (ctx.acts.clone(), read.clone());
            ctx.acts.call(req::<StatusSubscribeRequest>(json!({})), cx, move |answer, cx| {
                let here =
                    answer.ok().and_then(|s| s.statuses.into_iter().find(|w| w.kind.as_ref().is_some_and(|k| k.to_string() == "local")));
                let silent = here.map(|w| {
                    let reach = w.reach.state.to_string();
                    (w.name, reach == "unreachable" || reach == "no-daemon")
                });
                read.update(cx, |r, _| *r = Some(silent));
                acts.notify(cx);
            });
        }
    }
    let (name, silent) = read.read(cx).clone().flatten()?;
    if !silent {
        return None;
    }
    let here = words::here_name(&ctx.data.places);
    Some(words::own_daemon_down(if here.is_empty() { &name } else { &here }))
}

/// What a computer's cap counts: the threads running there, a cloud's machines holding a slot, as the host last
/// listed the row.
pub fn running(place: &PlaceView) -> usize {
    place.running.unwrap_or(0) as usize
}

/// The glyph in its frame.
pub fn place_glyph(ctx: &Ctx, place: &PlaceView, cx: &App) -> AnyElement {
    framed(computer_glyph(place, ctx.data.preferences(), cx), cx).into_any_element()
}

/// `UpdateControl`: one word in both states, held while it runs.
fn update_control(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> AnyElement {
    let updating = window.use_keyed_state(SharedString::from(format!("computer-{}-updating", place.id)), cx, |_, _| false);
    let busy = *updating.read(cx);
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    Button::outline(SharedString::from(format!("update-{}", place.id)), where_words::UPDATE)
        .xs()
        .held(busy)
        .on_click(move |_, _, cx| {
            updating.update(cx, |u, _| *u = true);
            let (acts2, updating) = (acts.clone(), updating.clone());
            acts.call(req::<PlacesUpdateRequest>(json!({ "placeId": id })), cx, move |answer, cx| {
                updating.update(cx, |u, _| *u = false);
                match answer {
                    Ok(_) => acts2.reread(Reread::Places, cx),
                    Err(said) => acts2.failed(said, cx),
                }
                acts2.notify(cx);
            });
            acts.notify(cx);
        })
        .into_any_element()
}

/// `PlaceState`: the cell's word, its mark, or the act itself. Sign in goes to the computer's page.
pub fn place_state(ctx: &Ctx, place: &PlaceView, cell: &Cell, sign_in: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let id = SharedString::from(format!("state-{}", place.id));
    let why = cell.why().map(str::to_owned);
    let with_why = |c: StateCell| match &why {
        Some(why) => c.why(why.clone()),
        None => c,
    };
    match cell {
        Cell::Update { .. } => with_why(StateCell::act(id, update_control(ctx, place, window, cx))).into_any_element(),
        Cell::SignIn { .. } => {
            let (acts, at) = (ctx.acts.clone(), At::Computer(place.id.clone()));
            let button = Button::outline(SharedString::from(format!("sign-in-{}", place.id)), state_words::SIGN_IN)
                .xs()
                .held(!sign_in)
                .on_click(move |_, window, cx| acts.go(at.clone(), window, cx));
            with_why(StateCell::act(id, button)).into_any_element()
        }
        Cell::Word { word, mark: Some(mark), why: w } => {
            let mark_el = state_mark(format!("mark-{}", place.id), *mark, w.clone().or_else(|| Some(word.clone())), cx);
            with_why(StateCell::act(id, mark_el.unwrap_or_else(|| div().into_any_element()))).into_any_element()
        }
        Cell::Word { word, .. } => with_why(StateCell::word(id, word.clone())).into_any_element(),
    }
}

/// A row as the search reads and draws it: the name a person types to find a computer, over its state.
fn search_row(ctx: &Ctx, place: &PlaceView, here: Option<&Absent>) -> Item {
    let at = At::Computer(place.id.clone());
    let acts = ctx.acts.clone();
    let words = state_of(place, here).words();
    Item::row(format!("place-{}", place.id), place_name(place), Some(&words), move |row| {
        row.on_open(move |_, window, cx| acts.go(at.clone(), window, cx))
    })
}

fn opens(ctx: &Ctx, place: &PlaceView) -> impl Fn(&gpui_kit::ClickEvent, &mut Window, &mut App) + 'static {
    let (acts, at) = (ctx.acts.clone(), At::Computer(place.id.clone()));
    move |_, window, cx| acts.go(at.clone(), window, cx)
}

/// `ComputerListRow`: its cores, its memory, the threads running there, and its state.
fn computer_row(ctx: &Ctx, place: &PlaceView, here: Option<&Absent>, window: &mut Window, cx: &mut App) -> GridRow {
    let cell = state_of(place, here);
    let mut name = GridName::new(place_glyph(ctx, place, cx), place_name(place));
    if place.default {
        name = name.tag(where_words::DEFAULT);
    }
    if let Some(fact) = version_fact(place, None) {
        name = name.fact(fact.into_iter().map(SharedString::from).collect());
    }
    let cores = place.shape.as_ref().map(|s| words::js_number(s.cpu)).unwrap_or_default();
    let memory = place.shape.as_ref().map(|s| fmt_mem_gb(s.mem_mb)).unwrap_or_default();
    let mut row = GridRow::new(SharedString::from(format!("place-row-{}", place.id)), LIST_COLUMNS)
        .on_open(opens(ctx, place))
        .cell(name)
        .cell(num(cores, cx))
        .cell(num(memory, cx))
        .cell(num(running(place).to_string(), cx))
        .cell(place_state(ctx, place, &cell, true, window, cx))
        .cell(chevron(cx));
    if let Some(why) = cell.why() {
        row = row.hover(why.to_owned());
    }
    row
}

/// `CloudListRow`: its name, its image and when it was sealed, how many machines run there, and its state.
fn cloud_row(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> GridRow {
    let cell = state_of(place, None);
    let p = cx.palette().clone();
    let note = match ctx.data.image.as_ref() {
        None => ctx.data.image_reading.then(|| super::image::words::READING.to_owned()),
        Some(view) => Some(match view.copies.iter().find(|c| names_place(place, &c.place)) {
            None => where_words::NO_IMAGE_YET.to_owned(),
            Some(copy) => where_words::image_sealed(copy.version.get(), &built_when(&copy.built_at, now_ms())),
        }),
    };
    let mut name = GridName::new(place_glyph(ctx, place, cx), place_name(place));
    if let Some(note) = note {
        name = name.note(note);
    }
    let machines = div()
        .w_full()
        .flex()
        .justify_end()
        .whitespace_nowrap()
        .text_size(text::FACT)
        .text_color(p.muted_foreground)
        .child(where_words::running(running(place)));
    let mut row = GridRow::new(SharedString::from(format!("place-row-{}", place.id)), LIST_COLUMNS)
        .on_open(opens(ctx, place))
        .cell(name)
        .cell2(div())
        .cell(machines)
        .cell(place_state(ctx, place, &cell, true, window, cx))
        .cell(chevron(cx));
    if let Some(why) = cell.why() {
        row = row.hover(why.to_owned());
    }
    row
}

/// `PendingRow`: what an add is called, how far it got, and its mark; opening it carries on in Add a computer.
fn pending_row(ctx: &Ctx, pending: &PendingComputer, cx: &mut App) -> GridRow {
    let place = pending.place_id.as_ref().and_then(|id| ctx.data.places.iter().find(|p| p.id == *id));
    let state = pending_word(pending);
    let why = match &state.sentence {
        Some(sentence) => format!("{}: {sentence}", state.word),
        None => state.word.clone(),
    };
    let (glyph, name) = match place {
        Some(place) => (place_glyph(ctx, place, cx), place_name(place)),
        None => (glyph_frame(Glyph::Server, cx).into_any_element(), pending.name.clone().unwrap_or_else(|| pending.address.clone())),
    };
    let mark = if pending.failed.is_some() { Mark::Failed } else { Mark::Pending };
    let mark_el =
        state_mark(format!("pending-mark-{}", pending.id), mark, Some(why.clone()), cx).unwrap_or_else(|| div().into_any_element());
    let acts = ctx.acts.clone();
    GridRow::new(SharedString::from(format!("pending-row-{}", pending.id)), LIST_COLUMNS)
        .hover(why.clone())
        .on_open(move |_, _, cx| acts.note(ADD_ELSEWHERE, cx))
        .cell(GridName::new(glyph, name))
        .cell2(div())
        .cell(div())
        .cell(StateCell::act(SharedString::from(format!("pending-state-{}", pending.id)), mark_el).why(why))
        .cell(chevron(cx))
}

/// What the Add a computer and Add a cloud buttons and a pending add's row say: this app draws neither dialog.
const ADD_ELSEWHERE: &str = "Add a computer and Add a cloud open in the next build of this app.";

/// A list's header row on the template.
fn head(cells: Vec<HeadCell>) -> GridHead {
    GridHead::new(Some(LIST_COLUMNS), cells)
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let here = here_absence(ctx, window, cx);
    let places = &ctx.data.places;
    let computers: Vec<&PlaceView> =
        places.iter().filter(|p| !is_provider(p) && !ctx.data.pending.iter().any(|q| q.place_id.as_deref() == Some(&p.id))).collect();
    let clouds: Vec<&PlaceView> = places.iter().filter(|p| is_provider(p)).collect();
    let offered = !clouds_offered(ctx.data.setup.as_ref().map(|s| &s.keys)).is_empty();
    let mut computers_body = v_flex().w_full().gap_2();
    if !computers.is_empty() {
        let rows: Vec<GridRow> = computers.iter().map(|place| computer_row(ctx, place, here.as_ref(), window, cx)).collect();
        computers_body = computers_body.child(
            Grid::new("computers")
                .head(head(vec![
                    HeadCell::new(where_words::HEAD_COMPUTER),
                    HeadCell::new(where_words::HEAD_CORES).num(),
                    HeadCell::new(where_words::HEAD_MEMORY).num(),
                    HeadCell::new(where_words::HEAD_THREADS).num(),
                ]))
                .rows(rows),
        );
    }
    if let Some(said) = &ctx.data.places_refused {
        computers_body = computers_body.child(crate::ui::Refusal::new().said(where_words::not_read(said)));
    }
    let acts = ctx.acts.clone();
    computers_body = computers_body
        .child(h_flex().child(Button::add("add-computer-button", add_words::TITLE).on_click(move |_, _, cx| acts.note(ADD_ELSEWHERE, cx))));
    let mut cards = vec![CardData::new("computers")
        .body(computers_body)
        .search(computers.iter().map(|place| search_row(ctx, place, here.as_ref())).collect())];

    if !ctx.data.pending.is_empty() {
        let rows: Vec<GridRow> = ctx.data.pending.iter().map(|pending| pending_row(ctx, pending, cx)).collect();
        let grid = Grid::new("pending")
            .head(head(vec![HeadCell::new(where_words::HEAD_PENDING), HeadCell::new(""), HeadCell::new(""), HeadCell::new("")]))
            .rows(rows);
        cards.push(CardData::new("pending").body(div().w_full().child(grid)));
    }

    if !clouds.is_empty() || offered {
        let mut clouds_body = v_flex().w_full().gap_2();
        clouds_body = if clouds.is_empty() {
            clouds_body.child(crate::ui::section_head(where_words::HEAD_CLOUD, cx))
        } else {
            let rows: Vec<GridRow> = clouds.iter().map(|place| cloud_row(ctx, place, window, cx)).collect();
            clouds_body.child(Grid::new("clouds").head(head(vec![HeadCell::new(where_words::HEAD_CLOUD)])).rows(rows))
        };
        if offered {
            let acts = ctx.acts.clone();
            clouds_body = clouds_body.child(
                h_flex()
                    .child(Button::add("add-cloud-button", add_words::ADD_CLOUD).on_click(move |_, _, cx| acts.note(ADD_ELSEWHERE, cx))),
            );
        }
        cards.push(
            CardData::new("clouds").body(clouds_body).search(clouds.iter().map(|place| search_row(ctx, place, here.as_ref())).collect()),
        );
    }
    cards
}
