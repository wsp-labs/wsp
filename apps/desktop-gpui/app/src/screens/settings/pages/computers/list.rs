// SPDX-License-Identifier: AGPL-3.0-only
//! Settings > Computers, after `computers.tsx`'s `computersCards`: the computers this wsp runs on, the adds that have
//! not reached Set up, and the clouds that lend it machines, each list on `LIST_COLUMNS` so the state column is one
//! line down the page, each row opening its own page.

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, AnyElement, App, MouseDownEvent, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    InitJob, PendingComputer, PlaceView, PlacesUpdateRequest, Preferences, PreferencesComputerLookValueIcon as Icon,
};

use super::words::{
    self, add_words, built_when, clouds_offered, fmt_mem_gb, is_provider, names_place, now_ms, pending_word, place_name, setup_words,
    state_cell, state_words, version_fact, where_words, Absent, Cell,
};
use crate::screens::settings::model::{req, At, Data, Reread};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::sidebar::menu::{self, values};
use crate::ui::computer_glyph::{computer_glyph, default_icon};
use crate::ui::context_menu::{ContextMenu, MenuItem};
use crate::ui::grid::{LIST_COLUMNS, LIST_COLUMNS_NARROW};
use crate::ui::{
    chevron, framed, glyph_frame, num, text, ActivePalette, Button, Glyph, Grid, GridHead, GridName, GridRow, HeadCell, MarkState,
    StateCell, StatusMark,
};

/// The computer the host runs on is never read as absent here: its daemon's reach rides its workspace's status,
/// which these pages do not hold.
pub fn state_of(place: &PlaceView, here: Option<&Absent>) -> Cell {
    let absent = if words::is_here(place) { here.cloned() } else { words::absent_of(place) };
    state_cell(place, absent.as_ref(), !words::is_here(place))
}

/// The computer the host runs on, read off its own workspace's daemon (`absenceAt`): the sidebar's `absenceOf` over
/// the status Settings last heard of that workspace, so it moves while the page is open; nothing before one arrives.
pub fn here_absence(data: &Data) -> Option<Absent> {
    let status = data.here_status.as_ref()?;
    menu::absence_of(status, Some(status), &values(&data.places), None)
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
            let mark_el = StatusMark::state(format!("mark-{}", place.id), *mark).tooltip(w.clone().unwrap_or_else(|| word.clone()));
            with_why(StateCell::act(id, mark_el)).into_any_element()
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
    if let Some(fact) = version_fact(place, ctx.data.host_version.as_deref()) {
        name = name.fact(fact.into_iter().map(SharedString::from).collect());
    }
    let cores = place.shape.as_ref().map(|s| words::js_number(s.cpu)).unwrap_or_default();
    let memory = place.shape.as_ref().map(|s| fmt_mem_gb(s.mem_mb)).unwrap_or_default();
    let mut row = GridRow::new(SharedString::from(format!("place-row-{}", place.id)), LIST_COLUMNS)
        .narrow(LIST_COLUMNS_NARROW)
        .on_open(opens(ctx, place))
        .on_menu(row_menu(ctx, place))
        .cell(name)
        .wide(div().w_full().debug_selector(|| "place-cores".into()).child(num(cores, cx)))
        .wide(div().w_full().debug_selector(|| "place-memory".into()).child(num(memory, cx)))
        .cell(div().w_full().debug_selector(|| "place-threads".into()).child(num(running(place).to_string(), cx)))
        .cell(place_state(ctx, place, &cell, true, window, cx))
        .cell(chevron(cx));
    if let Some(band) = super::live::band(ctx, place, window, cx) {
        row = row.line(band);
    }
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
        .text_size(text::FACT.0)
        .text_color(p.muted_foreground)
        .font_features(text::tabular())
        .child(where_words::running(running(place)));
    let setup = cloud_setup(super::image::job_of(ctx), &place.id);
    let state = match setup {
        None => place_state(ctx, place, &cell, true, window, cx),
        Some(mark) => {
            let why = if mark == MarkState::Failed { setup_words::FAILED } else { setup_words::SETTING_UP };
            let mark_el = StatusMark::state(format!("setup-mark-{}", place.id), mark).tooltip(why);
            StateCell::act(SharedString::from(format!("state-{}", place.id)), mark_el).why(why).into_any_element()
        }
    };
    let acts = ctx.acts.clone();
    let row = GridRow::new(SharedString::from(format!("place-row-{}", place.id)), LIST_COLUMNS).narrow(LIST_COLUMNS_NARROW);
    let row = match setup {
        None => row.on_open(opens(ctx, place)),
        Some(_) => row.on_open(move |_, _, cx| acts.note(ADD_ELSEWHERE, cx)),
    };
    let mut row = row.on_menu(row_menu(ctx, place)).cell(name).cell_spans(machines, 3, 1).cell(state).cell(chevron(cx));
    if let Some(why) = cell.why() {
        row = row.hover(why.to_owned());
    }
    row
}

/// `settingUpCloud`: a setup for this cloud under way, picks being made or an image building, its mark working; one
/// that failed waits on the person, its mark failed.
pub fn cloud_setup(job: Option<&InitJob>, provider: &str) -> Option<MarkState> {
    let job = job.filter(|j| j.place.as_ref().is_some_and(|p| p.id == provider))?;
    let phase = job.phase.to_string();
    if phase == "failed" {
        Some(MarkState::Failed)
    } else if super::image::job_over(&phase) {
        None
    } else {
        Some(MarkState::Working)
    }
}

/// `ComputerIcon`'s options, in its order.
const ICONS: [Icon; 11] = [
    Icon::Laptop,
    Icon::Desktop,
    Icon::MacMini,
    Icon::MacStudio,
    Icon::Tower,
    Icon::Server,
    Icon::Cloud,
    Icon::Cpu,
    Icon::Drive,
    Icon::Container,
    Icon::Home,
];

/// `computerIconWord`: an icon as a person reads its name.
fn icon_word(icon: Icon) -> String {
    match icon {
        Icon::MacMini => "Mac mini".to_owned(),
        Icon::MacStudio => "Mac Studio".to_owned(),
        other => crate::screens::settings::words::capitalised(&other.to_string()),
    }
}

/// `computerActions`: one item per icon but the one the computer wears, its pick or what it is.
pub fn icon_items(place: &PlaceView, preferences: Option<&Preferences>) -> Vec<(Icon, String)> {
    let wears = preferences.and_then(|p| p.computer_look.get(&place.id)).map_or_else(|| default_icon(place), |look| look.icon);
    ICONS.into_iter().filter(|icon| *icon != wears).map(|icon| (icon, format!("{} icon", icon_word(icon)))).collect()
}

/// What picking an icon writes: the computer's look, its icon.
pub fn icon_patch(place: &str, icon: Icon) -> Value {
    json!({ "computerLook": { place: { "icon": icon.to_string() } } })
}

/// The rows' one menu, the page's own off the Mac, held across frames by the list.
#[derive(Default)]
pub struct RowMenu(ContextMenu);

impl RowMenu {
    /// The labels of the rows the menu last asked for, whoever draws it.
    pub fn asked(&self) -> &[SharedString] {
        self.0.asked()
    }
}

/// `useRowMenu`: a right press on the row opens the icons it may show as, and a pick saves it as its look.
fn row_menu(ctx: &Ctx, place: &PlaceView) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let (menu, acts, id) = (ctx.computers_menu.clone(), ctx.acts.clone(), place.id.clone());
    let items = icon_items(place, ctx.data.preferences());
    move |event, window, cx| {
        cx.stop_propagation();
        let rows: Vec<MenuItem> = items
            .iter()
            .map(|(_, label)| MenuItem { label: label.clone().into(), group: "icon", refusal: None, keys: None, destructive: false })
            .collect();
        let (acts, id, icons) = (acts.clone(), id.clone(), items.iter().map(|(icon, _)| *icon).collect::<Vec<_>>());
        menu.update(cx, |m, cx| {
            m.0.open(event.position, rows, window, cx, move |_, pick, _, cx| {
                if let Some(icon) = pick.and_then(|ix| icons.get(ix)) {
                    acts.set_preferences(icon_patch(&id, *icon), cx);
                }
            })
        });
    }
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
    let mark = if pending.failed.is_some() { MarkState::Failed } else { MarkState::Pending };
    let mark_el = StatusMark::state(format!("pending-mark-{}", pending.id), mark).tooltip(why.clone());
    let acts = ctx.acts.clone();
    GridRow::new(SharedString::from(format!("pending-row-{}", pending.id)), LIST_COLUMNS)
        .narrow(LIST_COLUMNS_NARROW)
        .hover(why.clone())
        .on_open(move |_, _, cx| acts.note(ADD_ELSEWHERE, cx))
        .cell(GridName::new(glyph, name))
        .cell_spans(div(), 3, 1)
        .cell(StateCell::act(SharedString::from(format!("pending-state-{}", pending.id)), mark_el).why(why))
        .cell(chevron(cx))
}

/// Why Add a computer is held: its dialog comes with a later build.
pub const ADD_COMPUTER_LATER: &str = "Add a computer opens in the next build of this app.";

/// What the Add a cloud button and a pending add's row say: this app draws neither dialog.
const ADD_ELSEWHERE: &str = "Add a computer and Add a cloud open in the next build of this app.";

/// A list's header row on the template.
fn head(cells: Vec<HeadCell>) -> GridHead {
    GridHead::new(Some(LIST_COLUMNS), cells).narrow(LIST_COLUMNS_NARROW)
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let here = here_absence(ctx.data);
    let places = &ctx.data.places;
    if ctx.data.places_read {
        super::live::forget_gone(places, cx);
    }
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
                    HeadCell::new(where_words::HEAD_CORES).num().wide_only(),
                    HeadCell::new(where_words::HEAD_MEMORY).num().wide_only(),
                    HeadCell::new(where_words::HEAD_THREADS).num(),
                ]))
                .rows(rows),
        );
    }
    if let Some(open) = ctx.computers_menu.read(cx).0.overlay() {
        computers_body = computers_body.child(div().absolute().debug_selector(|| "computer-icon-menu".into()).child(open));
    }
    if let Some(said) = &ctx.data.places_refused {
        computers_body = computers_body.child(crate::ui::Refusal::new().said(where_words::not_read(said)));
    }
    computers_body =
        computers_body.child(h_flex().child(super::super::projects::held_add("add-computer-button", add_words::TITLE, ADD_COMPUTER_LATER)));
    let mut cards = vec![CardData::new("computers")
        .body(computers_body)
        .search(computers.iter().map(|place| search_row(ctx, place, here.as_ref())).collect())];

    if !ctx.data.pending.is_empty() {
        let rows: Vec<GridRow> = ctx.data.pending.iter().map(|pending| pending_row(ctx, pending, cx)).collect();
        let grid = Grid::new("pending")
            .head(head(vec![
                HeadCell::new(where_words::HEAD_PENDING),
                HeadCell::new("").wide_only(),
                HeadCell::new("").wide_only(),
                HeadCell::new(""),
            ]))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn job(place: &str, phase: &str) -> InitJob {
        serde_json::from_value(json!({ "id": "j", "road": "manual", "phase": phase, "keys": {}, "step": 0, "stoppable": true,
            "screens": [], "rows": [], "progress": { "done": 0, "total": 0 }, "log": [], "place": { "id": place, "name": place } }))
        .unwrap()
    }

    #[test]
    fn a_clouds_setup_stands_in_its_state_cell_until_it_ends_well() {
        assert_eq!(cloud_setup(Some(&job("solari", "answering")), "solari"), Some(MarkState::Working));
        assert_eq!(cloud_setup(Some(&job("solari", "building")), "solari"), Some(MarkState::Working));
        assert_eq!(cloud_setup(Some(&job("solari", "failed")), "solari"), Some(MarkState::Failed));
        assert_eq!(cloud_setup(Some(&job("solari", "done")), "solari"), None);
        assert_eq!(cloud_setup(Some(&job("solari", "cancelled")), "solari"), None);
        assert_eq!(cloud_setup(Some(&job("box", "building")), "solari"), None, "another cloud's setup");
        assert_eq!(cloud_setup(None, "solari"), None);
    }

    #[test]
    fn the_icon_menu_offers_every_icon_but_the_one_worn_and_writes_the_look() {
        let here: PlaceView = serde_json::from_value(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })).unwrap();
        let labels = |prefs: Option<&Preferences>| icon_items(&here, prefs).into_iter().map(|(_, l)| l).collect::<Vec<_>>();
        let worn = labels(None);
        assert_eq!(worn.len(), 10);
        assert!(!worn.contains(&"Desktop icon".to_owned()), "the computer here wears the desktop");
        assert_eq!(worn[..3], ["Laptop icon", "Mac mini icon", "Mac Studio icon"]);
        let prefs: Preferences = serde_json::from_value(json!({ "theme": "light", "sidebarMode": "list", "terminalSize": "app",
            "terminalZoom": {}, "access": {}, "projectLook": {}, "projectOrder": [], "labs": false,
            "computerLook": { "here": { "icon": "server" } } }))
        .unwrap();
        let picked = labels(Some(&prefs));
        assert!(picked.contains(&"Desktop icon".to_owned()) && !picked.contains(&"Server icon".to_owned()));
        assert_eq!(icon_patch("here", Icon::MacMini), json!({ "computerLook": { "here": { "icon": "mac-mini" } } }));
    }

    #[test]
    fn this_computer_reads_as_stopped_while_its_own_daemon_is_silent() {
        let status = |reach: &str| json!({ "id": "w", "kind": "local", "name": "wsp-host", "reach": { "state": reach } });
        let mut data = Data { here_status: Some(status("no-daemon")), ..Data::default() };
        let stopped = here_absence(&data).expect("a silent daemon");
        assert_eq!(
            (stopped.away, stopped.sentence.as_str()),
            ("stopped", "wsp-host's terminals and files stopped"),
            "by its workspace's name"
        );
        let here = json!({ "id": "here", "kind": "computer", "name": "mac", "label": "Studio", "default": true });
        data.places = vec![serde_json::from_value(here).unwrap()];
        assert_eq!(here_absence(&data).map(|a| a.sentence).as_deref(), Some("Studio's terminals and files stopped"), "by its own name");
        data.here_status = Some(status("unreachable"));
        assert!(here_absence(&data).is_some());
        data.here_status = Some(status("ok"));
        assert_eq!(here_absence(&data), None);
    }
}
