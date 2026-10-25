// SPDX-License-Identifier: AGPL-3.0-only
//! What a person sets on one computer, after `computerSettings.tsx` and the recipe and agents rows of
//! `computers.tsx`: the older wsp it runs and its update, the recipe it follows, threads at once, the nap, the turn
//! limit, a cloud's own limits, whether its threads may open threads, and its name and ssh login. Every set goes
//! through places.set and the row comes back as the host now reads it; a row off its default carries the reset arrow.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, App, ClickEvent, Entity, SharedString, Subscription, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceView, PlacesFollowRequest, PlacesSetRequest, PlacesUpdateRequest};

use super::words::{
    cloud_cap, delete_choices, fmt_mem_gb, fmt_rate, here_name, is_here, is_provider, nap_choices, page_words as w, place_name,
    size_from_word, size_name, size_word, sizes_sold, span_word, threads_cap, threads_default, turn_limit_choices, with_value, NO_RECIPE,
};
use crate::screens::dialogs::layer;
use crate::screens::settings::model::{req, At, GroupId, Reread};
use crate::screens::settings::page::{CardData, Ctx, DialogBuild, Item};
use crate::ui::{size, text, ActivePalette, AlertDialog, Button, Refusal, Row, Select, SelectItem, SelectState, Stepper, Switch};

/// A select a page keeps across frames: its state and the words it offers.
struct Picker {
    list: Entity<SelectState<SharedString>>,
    items: (Vec<SharedString>, Vec<Option<String>>),
}

/// The select keyed by `key`, offering `items` with `value` picked; `on_pick` runs with the words picked. An item with
/// a place in `shorts` has the trigger say those words once it is picked.
pub fn picker(
    key: SharedString,
    items: Vec<SharedString>,
    shorts: &[SharedString],
    value: &SharedString,
    window: &mut Window,
    cx: &mut App,
    on_pick: impl Fn(&SharedString, &mut App) + 'static,
) -> Select<SharedString> {
    picker_with(key, items, shorts, Vec::new(), value, window, cx, on_pick)
}

/// `picker` with a project glyph before each item's words, by the icon's word, where `icons` names one.
#[allow(clippy::too_many_arguments)]
fn picker_with(
    key: SharedString,
    items: Vec<SharedString>,
    shorts: &[SharedString],
    icons: Vec<Option<String>>,
    value: &SharedString,
    window: &mut Window,
    cx: &mut App,
    on_pick: impl Fn(&SharedString, &mut App) + 'static,
) -> Select<SharedString> {
    let of = |items: &[SharedString], shorts: &[SharedString], icons: &[Option<String>]| {
        items
            .iter()
            .enumerate()
            .map(|(ix, i)| {
                let item = SelectItem::new(i.clone(), i.clone());
                let item = match icons.get(ix).cloned().flatten() {
                    Some(icon) => item.glyph(crate::screens::settings::pages::projects::look::glyph_of(&icon)),
                    None => item,
                };
                match shorts.get(ix) {
                    Some(short) => item.short(short.clone()),
                    None => item,
                }
            })
            .collect::<Vec<_>>()
    };
    let first = (items.clone(), icons.clone());
    let first_shorts = shorts.to_vec();
    let start = value.clone();
    let state = window.use_keyed_state(key, cx, move |_, cx| Picker {
        list: cx.new(|_| SelectState::new(of(&first.0, &first_shorts, &first.1), Some(&start))),
        items: first,
    });
    let (list, stale) = {
        let picker = state.read(cx);
        (picker.list.clone(), picker.items.0 != items || picker.items.1 != icons)
    };
    if stale {
        let fresh = of(&items, shorts, &icons);
        list.update(cx, |list, cx| list.set_items(fresh, cx));
        state.update(cx, |picker, _| picker.items = (items, icons));
    }
    if list.read(cx).value() != Some(value) {
        let value = value.clone();
        list.update(cx, |list, cx| list.set_value(Some(&value), cx));
    }
    Select::new(&list).on_change(move |word, _, cx| on_pick(word, cx))
}

/// The reset arrow's press: that one setting back to its default.
fn reset_on(ctx: &Ctx, place_id: &str, word: &'static str) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let (ctx_acts, id) = (ctx.acts.clone(), place_id.to_owned());
    move |_, _, cx| {
        let acts = ctx_acts.clone();
        ctx_acts.call(req::<PlacesSetRequest>(json!({ "placeId": id, "reset": [word] })), cx, move |answer, cx| match answer {
            Ok(_) => acts.reread(Reread::Places, cx),
            Err(said) => acts.failed(said, cx),
        });
    }
}

/// `Stepper` with the last number sent standing while the host's row still holds the one before it.
#[allow(clippy::too_many_arguments)]
fn sent_stepper(
    ctx: &Ctx,
    place: &PlaceView,
    k: &str,
    value: u64,
    max: Option<u64>,
    ask: fn(u64) -> Value,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let sent = window.use_keyed_state(SharedString::from(format!("computer-{}-{k}-sent", place.id)), cx, |_, _| None::<u64>);
    let shown = sent.read(cx).unwrap_or(value);
    if *sent.read(cx) == Some(value) {
        sent.update(cx, |s, _| *s = None);
    }
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    Stepper::new(format!("{k}-{}", place.id), shown).max(max).on_change(move |next, _, cx| {
        sent.update(cx, |s, _| *s = Some(next));
        let (acts2, sent) = (acts.clone(), sent.clone());
        let mut params = json!({ "placeId": id });
        if let (Value::Object(into), Value::Object(ask)) = (&mut params, ask(next)) {
            into.extend(ask);
        }
        acts.call(req::<PlacesSetRequest>(params), cx, move |answer, cx| {
            if let Err(said) = answer {
                sent.update(cx, |s, _| *s = None);
                acts2.failed(said, cx);
            }
            acts2.reread(Reread::Places, cx);
        });
        acts.notify(cx);
    })
}

/// `SpanSelect`: stretches of time, none among them.
#[allow(clippy::too_many_arguments)]
fn span_select(
    ctx: &Ctx,
    place: &PlaceView,
    k: &str,
    value: Option<i64>,
    choices: &[Option<i64>],
    none: &'static str,
    set: fn(Option<i64>) -> Value,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let offered = with_value(choices, value);
    let items: Vec<SharedString> = offered.iter().map(|ms| span_word(*ms, none).into()).collect();
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    let select = picker(
        SharedString::from(format!("computer-{}-{k}", place.id)),
        items.clone(),
        &[],
        &span_word(value, none).into(),
        window,
        cx,
        move |word, cx| {
            let Some(ms) = items.iter().position(|i| i == word).map(|ix| offered[ix]) else { return };
            if ms == value {
                return;
            }
            set_ask(&acts, &id, set(ms), cx);
        },
    );
    select
}

fn set_ask(acts: &crate::screens::settings::Acts, id: &str, ask: Value, cx: &mut App) {
    let mut params = json!({ "placeId": id });
    if let (Value::Object(into), Value::Object(ask)) = (&mut params, ask) {
        into.extend(ask);
    }
    let acts2 = acts.clone();
    acts.call(req::<PlacesSetRequest>(params), cx, move |answer, cx| match answer {
        Ok(_) => acts2.reread(Reread::Places, cx),
        Err(said) => acts2.failed(said, cx),
    });
}

/// `DollarsField`: a figure saved on Enter or when the field is left, put back where it reads no figure.
fn dollars_field(ctx: &Ctx, place: &PlaceView, value: f64, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let shown = words_dollars(value);
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    let state = window.use_keyed_state(SharedString::from(format!("computer-{}-spend", place.id)), cx, move |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).default_value(shown.clone()));
        let saved = Rc::new(RefCell::new(value));
        let held = saved.clone();
        let sub = cx.subscribe_in(&input, window, move |_, input, event: &InputEvent, window, cx| {
            if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
                return;
            }
            let typed = input.read(cx).value().to_string();
            let now = *held.borrow();
            match typed.trim().parse::<f64>() {
                Ok(usd) if !typed.trim().is_empty() && usd.is_finite() && usd >= 0. && usd != now => {
                    set_ask(&acts, &id, json!({ "spendPerDayUsd": usd }), cx);
                }
                _ => input.update(cx, |input, cx| input.set_value(words_dollars(now), window, cx)),
            }
        });
        (input, saved, sub)
    });
    let input = state.read(cx).0.clone();
    *state.read(cx).1.borrow_mut() = value;
    let p = cx.palette().clone();
    h_flex()
        .flex_none()
        .w(size::SELECT_WIDTH)
        .h(size::CONTROL_HEIGHT)
        .rounded(size::CONTROL_RADIUS)
        .border_1()
        .border_color(p.input)
        .bg(p.input_fill)
        .child(div().flex_none().pl(px(11.)).text_size(text::FACT.0).text_color(p.muted_foreground).child("$"))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .px(px(4.))
                .child(Input::new(&input).appearance(false).text_size(text::FACT.0).font_features(text::tabular()).h(px(28.)).w_full()),
        )
}

fn words_dollars(usd: f64) -> String {
    if usd.fract() == 0. {
        format!("{}", usd as i64)
    } else {
        format!("{usd}")
    }
}

/// `SizeSelect`: the sizes a cloud's plan sells a sub-thread, by class name, figures and rate.
fn size_select(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let sold = sizes_sold(place);
    let word_of = |cpu: f64, mem: f64, name: Option<&str>, rate: Option<f64>| {
        w::size_choice(&size_name(name, cpu, mem), cpu, &fmt_mem_gb(mem), rate.map(fmt_rate).as_deref())
    };
    let keys: Vec<String> = sold.iter().map(|s| size_word(s.cpu, s.mem_mb)).collect();
    let items: Vec<SharedString> =
        sold.iter().map(|s| word_of(s.cpu, s.mem_mb, s.name.as_deref(), Some(s.rate_usd_per_hour)).into()).collect();
    let shorts: Vec<SharedString> = sold.iter().map(|s| word_of(s.cpu, s.mem_mb, s.name.as_deref(), None).into()).collect();
    let now = place.size.as_ref().map(|s| size_word(s.cpu as f64, s.mem_mb as f64)).unwrap_or_default();
    let value: SharedString = keys.iter().position(|k| *k == now).map(|ix| items[ix].clone()).unwrap_or_else(|| now.clone().into());
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    let picked = items.clone();
    let key = SharedString::from(format!("computer-{}-sub-size", place.id));
    let select = picker(key, items, &shorts, &value, window, cx, move |word, cx| {
        let Some(key) = picked.iter().position(|i| i == word).map(|ix| keys[ix].clone()) else { return };
        if let Some((cpu, mem_mb)) = size_from_word(&key) {
            set_ask(&acts, &id, json!({ "size": { "cpu": cpu, "memMb": mem_mb } }), cx);
        }
    });
    select
}

/// `BehindRow`: the row under a computer's head while it runs an older daemon than this wsp deploys.
pub fn behind_row(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<Row> {
    let behind = place.behind.as_ref()?;
    let name = place_name(place);
    let here = here_name(&ctx.data.places);
    let title = if is_here(place) { w::BEHIND_HERE_TITLE.to_owned() } else { w::behind_title(&here) };
    let update = behind.act.to_string() == "update";
    let note = if update { w::behind_update(&name, &here) } else { w::behind_install(&behind.fix) };
    let mut row = Row::new("behind", title).warning().note(note);
    if update {
        let busy_state = window.use_keyed_state(SharedString::from(format!("computer-{}-behind-busy", place.id)), cx, |_, _| false);
        let busy = *busy_state.read(cx);
        let (acts, id) = (ctx.acts.clone(), place.id.clone());
        row = row.control(
            Button::outline("update-wsp", if busy { w::UPDATING.to_owned() } else { w::update(&name) }).xs().held(busy).on_click(
                move |_, _, cx| {
                    busy_state.update(cx, |b, _| *b = true);
                    let (acts2, busy_state) = (acts.clone(), busy_state.clone());
                    acts.call(req::<PlacesUpdateRequest>(json!({ "placeId": id })), cx, move |answer, cx| {
                        busy_state.update(cx, |b, _| *b = false);
                        match answer {
                            Ok(_) => acts2.reread(Reread::Places, cx),
                            Err(said) => acts2.failed(said, cx),
                        }
                    });
                    acts.notify(cx);
                },
            ),
        );
    }
    Some(row)
}

/// Each recipe's icon as the select draws it before its name, the folder until one is picked; None draws none.
fn recipe_icons(preferences: Option<&wsp_host_client::protocol::Preferences>, slugs: &[String]) -> Vec<Option<String>> {
    slugs
        .iter()
        .map(|slug| {
            (slug != NO_RECIPE).then(|| {
                preferences.and_then(|p| p.recipe_look.get(slug)).map_or_else(|| "folder".to_owned(), |look| look.icon.to_string())
            })
        })
        .collect()
}

/// `RecipeCard`: the recipe a computer follows, its changes reaching it on their own; None where it follows none.
pub fn recipe_card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> CardData {
    let name = place_name(place);
    let recipes = ctx.data.recipes.clone().unwrap_or_default();
    let slugs: Vec<String> = recipes.iter().map(|r| r.slug.clone()).chain(std::iter::once(NO_RECIPE.to_owned())).collect();
    let items: Vec<SharedString> =
        recipes.iter().map(|r| SharedString::from(r.name.clone())).chain(std::iter::once("None".into())).collect();
    let value = place.recipe.as_deref().filter(|r| *r != NO_RECIPE).unwrap_or(NO_RECIPE);
    let shown = slugs.iter().position(|s| s == value).map(|ix| items[ix].clone()).unwrap_or_else(|| "None".into());
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    let picked = items.clone();
    let icons = recipe_icons(ctx.data.preferences(), &slugs);
    let key = SharedString::from(format!("computer-{}-recipe", place.id));
    let select = picker_with(key, items, &[], icons, &shown, window, cx, move |word, cx| {
        let Some(slug) = picked.iter().position(|i| i == word).map(|ix| slugs[ix].clone()) else { return };
        let acts2 = acts.clone();
        acts.call(req::<PlacesFollowRequest>(json!({ "placeId": id, "recipe": slug })), cx, move |answer, cx| match answer {
            Ok(_) => {
                acts2.reread(Reread::Places, cx);
                acts2.reread(Reread::Recipes, cx);
            }
            Err(said) => acts2.failed(said, cx),
        });
    });
    let note = w::recipe_line(&name);
    CardData::new("recipe").item(Item::row("follows", w::RECIPE_TITLE, Some(&note), |row| row.control(select)))
}

/// `LimitsCard`: threads at once where the computer said its shape, the nap where workspaces nap, the turn limit
/// where the host says one, and a cloud's own limits ahead of them.
pub fn limits_card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let name = place_name(place);
    let threads = threads_cap(place);
    let set = place.settings.as_ref();
    // The host writes napMs with napDefault where workspaces nap, and turnLimitMs on every row, null where off.
    let naps = place.nap_default.is_some();
    let cloud = is_provider(place);
    let mut card = CardData::new("computer-limits").head(w::LIMITS);
    if let (true, Some((machines, spend))) = (cloud, cloud_cap(place)) {
        let plan_cap = place.plan.as_ref().map(|p| p.machines_at_once);
        let note = w::machines_line(&name, plan_cap);
        let control =
            sent_stepper(ctx, place, "machines-at-once", machines, plan_cap.map(|c| c as u64), |n| json!({ "machines": n }), window, cx)
                .into_any_element();
        let reset = set.and_then(|s| s.machines).map(|_| reset_on(ctx, &place.id, "machines"));
        card = card.item(Item::row("machines-at-once", w::MACHINES_TITLE, Some(&note), move |row| with_reset(row.control(control), reset)));
        let note = w::spend_line(&name);
        let control = dollars_field(ctx, place, spend, window, cx).into_any_element();
        let reset = set.and_then(|s| s.spend_per_day_usd).map(|_| reset_on(ctx, &place.id, "spend"));
        card = card.item(Item::row("spend-per-day", w::SPEND_TITLE, Some(&note), move |row| with_reset(row.control(control), reset)));
        if place.size.is_some() {
            let control = size_select(ctx, place, window, cx).into_any_element();
            let reset = set.and_then(|s| s.size.as_ref()).map(|_| reset_on(ctx, &place.id, "size"));
            card = card.item(Item::row("sub-size", w::SIZE_TITLE, Some(w::SIZE_LINE), move |row| with_reset(row.control(control), reset)));
        }
        if let Some(after) = place.delete_after_ms {
            let value = (after != 0).then_some(after);
            let control = span_select(
                ctx,
                place,
                "delete-after",
                value,
                &delete_choices(),
                w::DELETE_AT_ONCE,
                |ms| json!({ "deleteAfterMs": ms.unwrap_or(0) }),
                window,
                cx,
            )
            .into_any_element();
            let reset = set.and_then(|s| s.delete_after_ms).map(|_| reset_on(ctx, &place.id, "delete-after"));
            card = card
                .item(Item::row("delete-after", w::DELETE_TITLE, Some(w::DELETE_LINE), move |row| with_reset(row.control(control), reset)));
        }
    }
    if let Some(threads) = threads {
        let note = match (threads_default(place), place.shape.as_ref()) {
            (Some(fallback), Some(shape)) => w::threads_line(fallback, &name, &fmt_mem_gb(shape.mem_mb)),
            _ => w::THREADS_LINE_BARE.to_owned(),
        };
        let control =
            sent_stepper(ctx, place, "threads-at-once", threads, None, |n| json!({ "threads": n }), window, cx).into_any_element();
        let reset = set.and_then(|s| s.threads).map(|_| reset_on(ctx, &place.id, "threads"));
        card = card.item(Item::row("threads-at-once", w::THREADS_AT_ONCE, Some(&note), move |row| with_reset(row.control(control), reset)));
    }
    if naps {
        let control =
            span_select(ctx, place, "nap-after", place.nap_ms, &nap_choices(), w::NAP_NEVER, |ms| json!({ "napMs": ms }), window, cx)
                .into_any_element();
        let reset = set.and_then(|s| s.nap_ms).map(|_| reset_on(ctx, &place.id, "nap"));
        card = card.item(Item::row("nap-after", w::NAP_TITLE, Some(w::NAP_LINE), move |row| with_reset(row.control(control), reset)));
    }
    {
        let control = span_select(
            ctx,
            place,
            "turn-limit",
            place.turn_limit_ms,
            &turn_limit_choices(),
            w::TURN_LIMIT_OFF,
            |ms| json!({ "turnLimitMs": ms }),
            window,
            cx,
        )
        .into_any_element();
        let reset = set.and_then(|s| s.turn_limit_ms).map(|_| reset_on(ctx, &place.id, "turn-limit"));
        card = card.item(Item::row("turn-limit", w::TURN_LIMIT_TITLE, Some(w::TURN_LIMIT_LINE), move |row| {
            with_reset(row.control(control), reset)
        }));
    }
    Some(card)
}

fn with_reset(row: Row, reset: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>) -> Row {
    match reset {
        Some(reset) => row.on_reset(reset),
        None => row,
    }
}

/// `SpawnCard`: whether a thread here may open threads of its own, and how far.
pub fn spawn_card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let spawn = place.spawn.as_ref()?;
    let set_spawn = place.settings.as_ref().and_then(|s| s.spawn.as_ref());
    let machines = (place.takes_forks == Some(true)).then_some(spawn.max_machines);
    let note = w::spawn_line(spawn.max_depth.get(), machines);
    let (acts, id) = (ctx.acts.clone(), place.id.clone());
    let switch = Switch::new("agents-start-agents", spawn.spawn)
        .on_change(move |on, _, cx| set_ask(&acts, &id, json!({ "spawn": { "spawn": *on } }), cx));
    let reset = set_spawn.filter(|s| s.spawn.is_some() || s.max_machines.is_some()).map(|_| reset_on(ctx, &place.id, "spawn"));
    let mut card = CardData::new("computer-spawn").head(w::THREADS_HERE).item(Item::row(
        "agents-start-agents",
        w::SPAWN_TITLE,
        Some(&note),
        move |row| with_reset(row.control(switch), reset),
    ));
    if spawn.spawn {
        let control =
            sent_stepper(ctx, place, "levels-deep", spawn.max_depth.get(), None, |n| json!({ "spawn": { "maxDepth": n } }), window, cx)
                .into_any_element();
        let reset = set_spawn.and_then(|s| s.max_depth).map(|_| reset_on(ctx, &place.id, "max-depth"));
        card =
            card.item(Item::row("levels-deep", w::LEVELS_TITLE, Some(w::LEVELS_LINE), move |row| with_reset(row.control(control), reset)));
    }
    Some(card)
}

/// `AgentsLink`: a computer's agents, tool servers and skills live on the Agents page.
pub fn agents_link(ctx: &Ctx, place: &PlaceView) -> CardData {
    let (acts, agents, id) = (ctx.acts.clone(), ctx.agents.clone(), place.id.clone());
    CardData::new("computer-agents").item(Item::row("agents-on", w::agents_on(&place_name(place)), Some(w::AGENTS_ON_LINE), move |row| {
        row.on_open(move |_, window, cx| {
            agents.update(cx, |s, cx| s.pick_place(id.clone(), cx));
            acts.go(At::Group(GroupId::Agents), window, cx)
        })
    }))
}

/// Which field a `FieldSheet` changes.
#[derive(Clone, Copy, PartialEq)]
enum Sheet {
    Name,
    Ssh,
}

type OnEnter = Rc<dyn Fn(&mut App)>;

/// What a sheet holds while it is open: its field, whether a save is on its way, and the host's refusal.
struct SheetState {
    input: Entity<InputState>,
    saving: bool,
    refusal: Option<String>,
    /// What Enter in the field does, as the frame last drew Save: the form's submit (`Sheet`).
    on_enter: Option<OnEnter>,
    _typed: Subscription,
}

/// `FieldSheet`: one line a person types, Save handing it to places.set; a refusal stands under the field.
fn field_sheet(ctx: &Ctx, place_id: &str, sheet: Sheet, window: &mut Window, cx: &mut App) -> gpui_kit::AnyElement {
    let Some(place) = ctx.data.places.iter().find(|p| p.id == place_id) else { return div().into_any_element() };
    let name = place_name(place);
    let ssh = place.road.as_ref().and_then(|r| r.ssh.clone());
    let (title, line, initial, placeholder, field) = match sheet {
        Sheet::Name => (w::NAME_TITLE, w::name_sheet(&name), place.name.clone(), place.name.clone(), "name"),
        Sheet::Ssh => (w::SSH_TITLE, w::ssh_sheet(&name), ssh.clone().unwrap_or_default(), "user@host".to_owned(), "ssh"),
    };
    let key = SharedString::from(format!("computer-{place_id}-sheet-{field}"));
    let state = window.use_keyed_state(key, cx, move |window, cx| {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder.clone()).default_value(initial.clone()));
        input.update(cx, |input, cx| input.focus(window, cx));
        let typed = cx.subscribe(&input, |state: &mut SheetState, _, event: &InputEvent, cx| match event {
            InputEvent::Change => {
                state.refusal = None;
                cx.notify();
            }
            InputEvent::PressEnter { .. } => {
                if let Some(save) = state.on_enter.clone() {
                    cx.defer(move |cx| save(cx));
                }
            }
            _ => {}
        });
        SheetState { input, saving: false, refusal: None, on_enter: None, _typed: typed }
    });
    let p = cx.palette().clone();
    let (input, saving, refusal) = {
        let s = state.read(cx);
        (s.input.clone(), s.saving, s.refusal.clone())
    };
    let value = input.read(cx).value().trim().to_owned();
    let held = value.is_empty();
    let checking = (sheet == Sheet::Ssh && saving).then(|| w::ssh_checking(&value, &name));
    let unchanged = match sheet {
        Sheet::Name => value == place.name,
        Sheet::Ssh => Some(value.clone()) == ssh,
    };
    let acts = ctx.acts.clone();
    let id = place_id.to_owned();
    let save_state = state.clone();
    let save = Rc::new(move |cx: &mut App| {
        if unchanged {
            acts.close_dialog(cx);
            return;
        }
        state.update(cx, |s, _| {
            s.saving = true;
            s.refusal = None;
        });
        let (acts2, state) = (acts.clone(), state.clone());
        acts.call(req::<PlacesSetRequest>(json!({ "placeId": id, field: value })), cx, move |answer, cx| {
            state.update(cx, |s, _| s.saving = false);
            match answer {
                Ok(_) => {
                    acts2.reread(Reread::Places, cx);
                    acts2.close_dialog(cx);
                }
                Err(said) => state.update(cx, |s, _| s.refusal = Some(said)),
            }
            acts2.notify(cx);
        });
        acts.notify(cx);
    });
    let enter = save.clone();
    save_state.update(cx, |s, _| s.on_enter = (!held && !saving).then_some(enter as OnEnter));
    let cancel = ctx.acts.clone();
    let mono = sheet == Sheet::Ssh;
    let field_el = div()
        .w_full()
        .h(px(40.))
        .rounded(size::BUTTON_RADIUS)
        .border_1()
        .border_color(if refusal.is_some() { p.destructive } else { p.input })
        .bg(p.input_fill)
        .px(px(11.))
        .flex()
        .items_center()
        .text_size(if mono { text::FACT.0 } else { text::SETTING_TITLE.0 })
        .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
        .child(gpui_kit::component::input::Input::new(&input).appearance(false).w_full());
    let mut slot = Refusal::new();
    if let Some(said) = refusal {
        slot = slot.said(said);
    }
    if let Some(checking) = checking {
        slot = slot.quiet(checking);
    }
    AlertDialog::new(title)
        .layered()
        .on_outside(layer::dismiss)
        .sentence(line)
        .child(v_flex().gap_3().pt_1().child(field_el).child(slot))
        .act(Button::outline("sheet-cancel", w::CANCEL_WORD).neutral_ring().on_click(move |_, _, cx| cancel.close_dialog(cx)))
        .act(
            Button::primary(SharedString::from(format!("computer-{field}-save")), w::SAVE)
                .held(held || saving)
                .on_click(move |_, _, cx| save(cx)),
        )
        .into_any_element()
}

/// `NameCard`: what a computer the person added is called and the ssh login the host reaches it by.
pub fn name_card(ctx: &Ctx, place: &PlaceView) -> Option<CardData> {
    if is_provider(place) || is_here(place) {
        return None;
    }
    let name = place_name(place);
    let ssh = place.road.as_ref().and_then(|r| r.ssh.clone());
    let opens = |sheet: Sheet| {
        let (acts, id) = (ctx.acts.clone(), place.id.clone());
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
            let id = id.clone();
            acts.open_dialog(DialogBuild::dialog(move |ctx, window, cx| field_sheet(ctx, &id, sheet, window, cx)), cx);
        }
    };
    let name_open = opens(Sheet::Name);
    let ssh_open = opens(Sheet::Ssh);
    let ssh_note = ssh.clone().unwrap_or_else(|| w::SSH_NONE.to_owned());
    let mono = ssh.is_some();
    Some(
        CardData::new("computer-name-login")
            .item(Item::row("computer-name", w::NAME_TITLE, Some(&name), move |row| {
                row.control(
                    div()
                        .debug_selector(|| "computer-name-change".into())
                        .child(Button::outline("computer-name-change", w::CHANGE).xs().on_click(name_open)),
                )
            }))
            .item(Item::row("computer-ssh", w::SSH_TITLE, Some(&ssh_note), move |row| {
                let row = row.control(
                    div()
                        .debug_selector(|| "computer-ssh-change".into())
                        .child(Button::outline("computer-ssh-change", w::CHANGE).xs().on_click(ssh_open)),
                );
                if mono {
                    row.mono()
                } else {
                    row
                }
            })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_recipe_on_the_select_wears_its_icon_and_none_wears_none() {
        let prefs: wsp_host_client::protocol::Preferences = serde_json::from_value(json!({ "theme": "light", "sidebarMode": "list",
            "terminalSize": "app", "terminalZoom": {}, "access": {}, "projectLook": {}, "projectOrder": [], "labs": false,
            "recipeLook": { "web": { "icon": "rocket" } } }))
        .unwrap();
        let slugs = ["web".to_owned(), "api".to_owned(), NO_RECIPE.to_owned()];
        assert_eq!(recipe_icons(Some(&prefs), &slugs), [Some("rocket".to_owned()), Some("folder".to_owned()), None]);
    }

    /// The Agents row of box1's page alone, built off Settings' own records and acts, above Settings itself.
    struct AgentsRow(Entity<crate::screens::settings::SettingsScreen>);

    impl Render for AgentsRow {
        fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let card = self.0.update(cx, |s, cx| {
                let place = s.data.places.iter().find(|p| p.id == "box1").cloned().expect("box1");
                let acts = crate::screens::settings::Acts::new(cx.weak_entity(), None, s.runtime.clone());
                let at = s.at.clone();
                let ctx = Ctx::new(&s.data, &at, None, acts, s.activity.clone(), s.agents.clone(), s.computers_menu.clone());
                agents_link(&ctx, &place).render()
            });
            v_flex().size_full().child(div().debug_selector(|| "agents-on-box1".into()).w(px(760.)).child(card)).child(self.0.clone())
        }
    }

    #[gpui_kit::test]
    fn the_agents_row_on_a_boxs_page_opens_agents_reading_that_box(cx: &mut gpui_kit::TestAppContext) {
        use crate::screens::settings::model::Data;
        use crate::screens::settings::SettingsScreen;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let place = |v: Value| serde_json::from_value::<PlaceView>(v).unwrap();
        let places = vec![
            place(json!({ "id": "here", "kind": "computer", "name": "mac", "default": true })),
            place(json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "road": { "ssh": "me@box1" } })),
        ];
        let data = Data { places, places_read: true, ..Data::default() };
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let mut screen = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| SettingsScreen::with_data(data, At::Computer("box1".into()), runtime, window, cx));
            screen = Some(view.clone());
            gpui_kit::component::Root::new(cx.new(|_| AgentsRow(view)), window, cx)
        });
        let screen = screen.unwrap();
        cx.simulate_resize(gpui_kit::size(px(1280.), px(1600.)));
        cx.run_until_parked();
        let row = cx.debug_bounds("agents-on-box1").expect("the Agents row");
        cx.simulate_click(row.center(), gpui_kit::Modifiers::none());
        cx.run_until_parked();
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        assert_eq!(screen.read_with(cx, |s, _| s.at.clone()), At::Group(GroupId::Agents));
        let store = screen.read_with(cx, |s, _| s.agents.clone());
        let (picked, read) = store.read_with(cx, |a, _| (a.place.clone(), a.reads.keys().cloned().collect::<Vec<_>>()));
        assert_eq!(picked.as_deref(), Some("box1"), "the row picked its computer");
        assert_eq!(read, ["box1"], "the Agents page reads box1, not this computer");
    }
}
