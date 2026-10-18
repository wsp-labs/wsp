// SPDX-License-Identifier: AGPL-3.0-only
//! One agent's own page (`agentPage.tsx`): its head (version, sign-in, the vendor's update and whether it is on there),
//! what a new thread runs it with and the models its picker lists, all kept in the preferences, then how it runs on
//! the picked computer, written through `agents.setup`.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    canvas, div, prelude::*, px, relative, AnyElement, App, Context, Entity, KeyDownEvent, Modifiers, Pixels, SharedString, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    AccountsAnswerAccountsItem as Account, HarnessCatalog, Preferences, PreferencesAgentDefaultsValueModels as Picker,
};

use super::keyed;
use super::model::{
    access_held, command_words, default_not_run_line, efforts_for, entry, env_refusal, listed_models, model_id_refusal, model_of,
    new_thread_defaults, own, plan_word, shaped, shell_line, sign_in_head, unmarked, w, with_custom, AgentRow, SignInHead, ACCESS,
};
use super::parts::{refusal, refused_field};
use super::store::Store;
use super::Rows;
use crate::screens::dialogs::layer;
use crate::screens::settings::page::{Ctx, DialogBuild};
use crate::screens::settings::Acts;
use crate::ui::catalog;
use crate::ui::{
    size, text, tip, ActivePalette, AlertDialog, Button, Card, CardItem, Glyph, HeadRow, Refusal, Row, Segmented, Select, Skeleton, Switch,
};

pub fn page(id: &str, ctx: &Ctx, store: &Entity<Store>, rows: &Rows, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    let (row, reading, accounts, own) = {
        let s = store.read(cx);
        let read = s.reads.get(&rows.place);
        (
            read.and_then(|r| r.report.as_ref()).and_then(|r| r.agents.iter().find(|a| a.id == id)).cloned(),
            read.is_some_and(|r| r.reading),
            s.accounts.clone(),
            s.catalogs.get(&rows.place).cloned(),
        )
    };
    // What the picked computer's own binary lists, so a default picked here is one that computer runs.
    let catalog = own.unwrap_or_else(|| ctx.data.harnesses.clone()).into_iter().find(|c| c.harness == id);
    let label =
        catalog.as_ref().map(|c| c.label.clone()).or_else(|| row.as_ref().map(|r| r.name.clone())).unwrap_or_else(|| catalog::name(id));
    let computer = rows.name.clone();
    let account = accounts.and_then(|all| all.into_iter().find(|a| a.agent == id && a.computers.contains(&computer)));
    let mut out = vec![head(id, &label, row.as_ref(), account.as_ref(), store, rows, ctx, cx)];
    let prefs = ctx.data.preferences();
    if let Some(catalog) = &catalog {
        out.push(new_threads(catalog, prefs, ctx, window, cx));
        out.extend(models_card(catalog, prefs, ctx, window, cx));
    }
    if catalog.is_none() && row.as_ref().is_some_and(|r| r.installed) {
        out.push(
            div()
                .mt(px(-8.))
                .px_1()
                .text_size(text::NOTE.0)
                .line_height(text::NOTE.1)
                .text_color(cx.palette().muted_foreground)
                .child(w::no_threads_yet(&label))
                .into_any_element(),
        );
    }
    if catalog.is_some() && row.is_none() && reading {
        out.push(runs_skeleton());
    }
    if let (Some(_), Some(row)) = (&catalog, &row) {
        out.extend(runs(id, &label, row, store, rows, ctx));
    }
    out
}

/// The head: its mark, its name and version, how it is signed in there, and the update and the switch.
#[allow(clippy::too_many_arguments)]
fn head(
    id: &str,
    label: &str,
    row: Option<&AgentRow>,
    account: Option<&Account>,
    store: &Entity<Store>,
    rows: &Rows,
    ctx: &Ctx,
    cx: &App,
) -> AnyElement {
    let brand = crate::ui::catalog::plan_brand(id);
    let plan = row.and_then(|r| r.sign_in_plan.clone()).or_else(|| account.and_then(|a| a.plan.clone()).filter(|p| !p.is_empty()));
    let mut head = HeadRow::new("agent-head", catalog::mark(id, label, px(16.), cx.palette().foreground, cx), label.to_owned());
    if let Some(version) = row.and_then(|r| r.version.clone()) {
        head = head.mark(version);
    }
    if let Some(row) = row {
        let line = if row.installed {
            let said = sign_in_head(row, &rows.name, plan.map(|p| plan_word(&p, brand)));
            head_line(said, account.and_then(|a| a.address.clone()))
        } else {
            w::NOT_INSTALLED_HERE.into_any_element()
        };
        head = head.line(line);
    }
    if let Some(row) = row.filter(|r| r.installed) {
        let mut slot = h_flex().gap_3();
        if let Some(update) = row.update.clone() {
            let (acts, computer) = (ctx.acts.clone(), rows.name.clone());
            slot = slot.child(Button::outline("agent-update", w::update_to(&update.to)).xs().tooltip(update.command.clone()).on_click(
                move |_, _, cx| {
                    cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(update.command.clone()));
                    acts.done(w::update_copied(&update.command, &computer), cx);
                },
            ));
        }
        if let Some(setup) = &row.setup {
            let busy = store.read(cx).busy.contains(&(rows.place.clone(), format!("agent-on-{id}")));
            let (store, place, agent) = (store.clone(), rows.place.clone(), id.to_owned());
            slot = slot.child(Switch::new("agent-on", setup.on).held(busy).on_change(move |on, _, cx| {
                let on = *on;
                let (place, agent) = (place.clone(), agent.clone());
                store.update(cx, |s, cx| {
                    let key = (place.clone(), format!("agent-on-{agent}"));
                    s.busy.insert(key.clone());
                    s.setup(&place, &agent, json!({ "on": on }), cx, move |s, answer, cx| {
                        s.busy.remove(&key);
                        if let Err(said) = answer {
                            s.failed(said, cx);
                        }
                    });
                });
            }));
        }
        head = head.slot(slot);
    }
    Card::new("agent").item(head).into_any_element()
}

/// New threads: model and effort, and access, the same on every computer, kept in the preferences.
fn new_threads(catalog: &HarnessCatalog, prefs: Option<&Preferences>, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let id = catalog.harness.clone();
    let own = own(prefs, &id);
    let defaults = new_thread_defaults(catalog, prefs);
    let model = defaults.model.as_ref().map(|m| m.value.clone());
    let set = {
        let (acts, id) = (ctx.acts.clone(), id.clone());
        Rc::new(move |patch: Value, cx: &mut App| acts.set_preferences(json!({ "agentDefaults": { id.clone(): patch } }), cx))
    };
    let models = listed_models(catalog);
    let mut card = Card::new("agent-new-threads").head(w::NEW_THREADS);
    if !models.is_empty() {
        let mut listed = models.clone();
        if let Some(m) = &model {
            if !listed.iter().any(|l| l.value == *m) {
                listed.push(model_of(catalog, m));
            }
        }
        let items = listed.iter().map(|m| (m.value.clone(), SharedString::from(m.label.clone()), None)).collect();
        let model_select = keyed::pick(format!("agent-{id}-model"), items, model.clone(), window, cx);
        let pick_set = set.clone();
        let model_select = Select::new(&model_select).on_change(move |v: &String, _, cx| pick_set(json!({ "model": v }), cx));
        let efforts = efforts_for(catalog, model.as_ref().map(|m| model_of(catalog, m)).as_ref());
        let mut control = h_flex().gap_2().child(model_select);
        if !efforts.is_empty() {
            let items = efforts.iter().map(|o| (o.value.clone(), SharedString::from(o.label.clone()), None)).collect();
            let effort = defaults.effort.as_ref().map(|e| e.value.clone());
            let effort_select = keyed::pick(format!("agent-{id}-effort"), items, effort, window, cx);
            let pick_set = set.clone();
            control = control
                .child(Select::new(&effort_select).width(px(96.)).on_change(move |v: &String, _, cx| pick_set(json!({ "effort": v }), cx)));
        }
        let table = ctx.data.harnesses.iter().find(|c| c.harness == id);
        let note = default_not_run_line(catalog, table, prefs).unwrap_or_else(|| w::MODEL_DESCRIPTION.to_owned());
        let mut row = Row::new("agent-model", w::MODEL).note(note).control(control);
        if own.model.is_some() || own.effort.is_some() {
            let set = set.clone();
            row = row.on_reset(move |_, _, cx| set(json!({ "model": null, "effort": null }), cx));
        }
        card = card.item(row);
    }
    let access = defaults.access.as_ref().map(|a| a.value.clone()).unwrap_or_default();
    let control = ACCESS.iter().fold(Segmented::new("agent-access", ACCESS.iter().position(|w| *w == access)), |control, word| {
        let label = w::ACCESS_WORDS.iter().find(|(v, _)| v == word).map(|(_, l)| l.to_string()).unwrap_or_default();
        match access_held(catalog, word) {
            Some(why) => control.held_segment(label, why),
            None => control.segment(label),
        }
    });
    let pick_set = set.clone();
    let mut row = Row::new("agent-access", w::ACCESS)
        .note(w::access_description(&catalog.label))
        .control(control.on_pick(move |ix, _, cx| pick_set(json!({ "access": ACCESS[ix] }), cx)));
    if own.access.is_some() {
        row = row.on_reset(move |_, _, cx| set(json!({ "access": null }), cx));
    }
    card.item(row).into_any_element()
}

/// One model on the Models card: shown in the picker or not, and whether the person added it by id.
#[derive(Clone)]
struct Listed {
    value: String,
    label: String,
    added: bool,
    shown: bool,
}

/// Writes the picker: the hidden ones, the order where it moved, and the ids the person added.
fn write_picker(id: &str, picker: Option<&Picker>, next: &[Listed], custom: &[String], moved: bool, acts: &Acts, cx: &mut App) {
    let hide: Vec<String> = next.iter().filter(|m| !m.shown).map(|m| m.value.clone()).collect();
    let order: Vec<String> = if moved {
        next.iter().filter(|m| m.shown).map(|m| m.value.clone()).collect()
    } else {
        picker.map(|p| p.order.clone()).unwrap_or_default()
    };
    let kept: serde_json::Map<String, Value> = [("hide", hide), ("order", order), ("custom", custom.to_vec())]
        .into_iter()
        .filter(|(_, list)| !list.is_empty())
        .map(|(key, list)| (key.to_owned(), json!(list)))
        .collect();
    let models = if kept.is_empty() { Value::Null } else { Value::Object(kept) };
    acts.set_preferences(json!({ "agentDefaults": { id: { "models": models } } }), cx);
}

/// `movedBefore`: `moved` taken out and put back just before `before`.
fn moved_before(ids: &[String], moved: &str, before: &str) -> Vec<String> {
    let rest: Vec<String> = ids.iter().filter(|v| *v != moved).cloned().collect();
    match rest.iter().position(|v| v == before) {
        Some(at) => [&rest[..at], &[moved.to_owned()], &rest[at..]].concat(),
        None => ids.to_vec(),
    }
}

/// `step`: the shown models with `value` one up or down, where that stays inside its own list (the current or the
/// legacy one, which the host orders each on its own).
fn stepped(shown: &[String], legacy: &[String], value: &str, by: isize) -> Option<Vec<String>> {
    let at = shown.iter().position(|v| v == value)?;
    let to = at.checked_add_signed(by).filter(|to| *to < shown.len())?;
    if legacy.contains(&shown[at]) != legacy.contains(&shown[to]) {
        return None;
    }
    let mut next = shown.to_vec();
    let moved = next.remove(at);
    next.insert(to, moved);
    Some(next)
}

/// Writes the shown models in a new order.
type Reorder = Rc<dyn Fn(Vec<String>, &mut App)>;

/// A model's row as it is dragged within its list.
#[derive(Clone)]
struct DraggedModel {
    value: String,
    legacy: bool,
    label: String,
    note: String,
    width: Pixels,
}

/// GPUI draws no ghost of what is dragged: the row itself at half strength, where it was taken (D72).
struct ModelGhost(DraggedModel);

impl Render for ModelGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let m = &self.0;
        let grip = Button::icon_xs("model-ghost-grip", Glyph::GripVertical);
        div()
            .debug_selector(|| "model-ghost".into())
            .w(m.width)
            .opacity(0.5)
            .bg(cx.palette().card)
            .child(Row::new("model-ghost", m.label.clone()).lead(div().ml(px(-6.)).child(grip)).note(m.note.clone()))
    }
}

/// Models: every model the agent offers, the picker's own first in its order, then the ones hidden from it.
fn models_card(catalog: &HarnessCatalog, prefs: Option<&Preferences>, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let id = catalog.harness.clone();
    let picker = own(prefs, &id).models;
    let custom = picker.as_ref().map(|p| p.custom.clone()).unwrap_or_default();
    let mut base = unmarked(catalog);
    base.hidden_models = Vec::new();
    let shaped = shaped(&with_custom(&base, picker.as_ref()), picker.as_ref());
    let mut items: Vec<Listed> = listed_models(&shaped)
        .into_iter()
        .map(|m| Listed { added: m.added == Some(true), value: m.value, label: m.label, shown: true })
        .collect();
    items.extend(shaped.hidden_models.iter().map(|m| Listed {
        value: m.value.clone(),
        label: m.label.clone(),
        added: m.added == Some(true),
        shown: false,
    }));
    if items.is_empty() {
        return None;
    }
    let legacy: Vec<String> = shaped.legacy_models.iter().map(|m| m.value.clone()).collect();
    let shown: Vec<String> = items.iter().filter(|m| m.shown).map(|m| m.value.clone()).collect();
    // The shown models in a new order, written as the picker's order; the hidden ones keep their place after them.
    let reorder: Reorder = {
        let (acts, id, picker, all, custom) = (ctx.acts.clone(), id.clone(), picker.clone(), items.clone(), custom.clone());
        Rc::new(move |order: Vec<String>, cx: &mut App| {
            let mut next: Vec<Listed> = order.iter().filter_map(|v| all.iter().find(|m| m.value == *v)).cloned().collect();
            next.extend(all.iter().filter(|m| !m.shown).cloned());
            write_picker(&id, picker.as_ref(), &next, &custom, true, &acts, cx);
        })
    };
    let dragged = window.use_keyed_state(SharedString::from(format!("agent-{id}-model-dragged")), cx, |_, _| None::<String>);
    let fallback = new_thread_defaults(catalog, prefs).model.map(|m| m.value);
    let mut card = Card::new("agent-models").head(w::MODELS).lede(w::MODELS_DESCRIPTION);
    for m in &items {
        let lead = if m.shown {
            let (value, shown, legacy, reorder) = (m.value.clone(), shown.clone(), legacy.clone(), reorder.clone());
            div()
                .debug_selector({
                    let value = m.value.clone();
                    move || format!("model-grip-{value}")
                })
                .on_key_down(move |event: &KeyDownEvent, _, cx| {
                    let by = match event.keystroke.key.as_str() {
                        "up" => -1,
                        "down" => 1,
                        _ => return,
                    };
                    if event.keystroke.modifiers != Modifiers::none() {
                        return;
                    }
                    cx.stop_propagation();
                    if let Some(next) = stepped(&shown, &legacy, &value, by) {
                        reorder(next, cx);
                    }
                })
                .child(
                    Button::icon_xs(SharedString::from(format!("model-{}-grip", m.value)), Glyph::GripVertical)
                        .tooltip(w::reorder(&m.label)),
                )
                .into_any_element()
        } else {
            div().size(px(24.)).flex_none().into_any_element()
        };
        let note = if m.added && catalog.legacy_listed == Some(true) {
            w::not_in_list(&catalog.label)
        } else if m.label == m.value {
            String::new()
        } else {
            m.value.clone()
        };
        let mut control = h_flex().gap_1();
        if custom.contains(&m.value) {
            let (acts, id, picker, all, custom, value) =
                (ctx.acts.clone(), id.clone(), picker.clone(), items.clone(), custom.clone(), m.value.clone());
            control = control.child(
                Button::icon_xs(SharedString::from(format!("model-{}-remove", m.value)), Glyph::Close)
                    .tooltip(w::remove_named(&m.label))
                    .on_click(move |_, _, cx| {
                        let next: Vec<Listed> = all.iter().filter(|l| l.value != value).cloned().collect();
                        let ids: Vec<String> = custom.iter().filter(|v| **v != value).cloned().collect();
                        let moved = picker.as_ref().is_some_and(|p| !p.order.is_empty());
                        write_picker(&id, picker.as_ref(), &next, &ids, moved, &acts, cx);
                    }),
            );
        }
        let (acts, idc, pk, all, cu, value) =
            (ctx.acts.clone(), id.clone(), picker.clone(), items.clone(), custom.clone(), m.value.clone());
        control = control.child(div().ml_2().child(
            Switch::new(SharedString::from(format!("model-{}-shown", m.value)), m.shown).on_change(move |on, _, cx| {
                let next: Vec<Listed> =
                    all.iter().map(|l| if l.value == value { Listed { shown: *on, ..l.clone() } } else { l.clone() }).collect();
                write_picker(&idc, pk.as_ref(), &next, &cu, false, &acts, cx);
            }),
        ));
        let mut row = Row::new(SharedString::from(format!("model-{}", m.value)), m.label.clone())
            .lead(div().ml(px(-6.)).child(lead))
            .note(note.clone())
            .control(control);
        if fallback.as_deref() == Some(m.value.as_str()) {
            row = row.mark_word(w::DEFAULT_MODEL);
        }
        if !m.shown {
            card = card.item(row);
            continue;
        }
        // The row's width as last laid out, which its ghost takes.
        let width = Rc::new(std::cell::Cell::new(px(0.)));
        let taken = DraggedModel { value: m.value.clone(), legacy: legacy.contains(&m.value), label: m.label.clone(), note, width: px(0.) };
        let faded = dragged.read(cx).as_deref() == Some(m.value.as_str());
        let (on_drag, on_drop, measure) = (dragged.clone(), dragged.clone(), width.clone());
        let (target, target_legacy, shown, reorder) = (m.value.clone(), taken.legacy, shown.clone(), reorder.clone());
        card = card.item(CardItem::placed(move |corners| {
            div()
                .id(SharedString::from(format!("model-{}-drag", taken.value)))
                .debug_selector({
                    let value = taken.value.clone();
                    move || format!("model-row-{value}")
                })
                .relative()
                .when(faded, |el| el.opacity(0.5))
                .on_drag(taken, move |m, _, _, cx| {
                    on_drag.update(cx, |d, cx| {
                        *d = Some(m.value.clone());
                        cx.notify();
                    });
                    let ghost = DraggedModel { width: width.get(), ..m.clone() };
                    let gone = on_drag.clone();
                    cx.new(|cx| {
                        cx.on_release(move |_, cx| {
                            gone.update(cx, |d, cx| {
                                *d = None;
                                cx.notify();
                            })
                        })
                        .detach();
                        ModelGhost(ghost)
                    })
                })
                .can_drop(move |d, _, _| d.downcast_ref::<DraggedModel>().is_some_and(|d| d.legacy == target_legacy))
                .on_drop(move |d: &DraggedModel, _, cx| {
                    on_drop.update(cx, |d, cx| {
                        *d = None;
                        cx.notify();
                    });
                    if d.value != target {
                        reorder(moved_before(&shown, &d.value, &target), cx);
                    }
                })
                .child(CardItem::from(row).place(corners))
                .child(canvas(move |bounds, _, _| measure.set(bounds.size.width), |_, _, _, _| {}).absolute().inset_0())
                .into_any_element()
        }));
    }
    let refused_key = format!("agent-{id}-model-refused");
    // The refusal and the words it refused, so the next change to them clears it (`setRefused(null)` on change).
    let refused = window.use_keyed_state(SharedString::from(refused_key), cx, |_, _| None::<(String, String)>);
    let add = {
        let (acts, id, picker, all, custom, refused) =
            (ctx.acts.clone(), id.clone(), picker.clone(), items.clone(), custom.clone(), refused.clone());
        move |input: &Entity<gpui_kit::component::input::InputState>, window: &mut Window, cx: &mut App| {
            let typed = keyed::words(input, cx);
            let value = typed.trim().to_owned();
            if value.is_empty() {
                return;
            }
            if let Some(said) = model_id_refusal(&value) {
                refused.update(cx, |r, cx| {
                    *r = Some((typed, said));
                    cx.notify();
                });
                return;
            }
            keyed::clear(input, window, cx);
            if all.iter().any(|m| m.value == value) {
                return;
            }
            let mut next = all.clone();
            next.push(Listed { value: value.clone(), label: value.clone(), added: true, shown: true });
            let mut ids = custom.clone();
            ids.push(value);
            let moved = picker.as_ref().is_some_and(|p| !p.order.is_empty());
            write_picker(&id, picker.as_ref(), &next, &ids, moved, &acts, cx);
        }
    };
    let input = keyed::typed(format!("agent-{id}-model-id"), w::MODEL_ID_PLACEHOLDER, "", false, add.clone(), window, cx);
    let typed = keyed::words(&input, cx);
    if refused.read(cx).as_ref().is_some_and(|(at, _)| *at != typed) {
        refused.update(cx, |r, _| *r = None);
    }
    let held = typed.trim().is_empty();
    let mut button = Button::outline("agent-models-add", w::ADD).xs().held(held);
    if !held {
        let input = input.clone();
        button = button.on_click(move |_, window, cx| add(&input, window, cx));
    }
    let said = refused.read(cx).clone().map(|(_, said)| said);
    let field_el = refused_field(&input, said.is_some(), window, cx);
    let add_row = CardItem::placed(move |corners| {
        corners
            .apply(v_flex())
            .debug_selector(|| "agent-models-add-row".into())
            .px(corners.inset)
            .py_3()
            .min_h(size::ROW_FLOOR)
            .justify_center()
            .child(h_flex().gap_2().child(field_el).child(button))
            .into_any_element()
    });
    card = card.item(add_row);
    if let Some(said) = said {
        // `-mt-3` inside the card, under the add row's rule, as the web's slot stands.
        card = card.item(CardItem::placed(move |corners| {
            corners
                .apply(div())
                .debug_selector(|| "agent-models-refusal".into())
                .mt(px(-12.))
                .px(corners.inset)
                .child(Refusal::new().said(said))
                .into_any_element()
        }));
    }
    Some(card.into_any_element())
}

/// How it runs while the computer's agents are read: its head and its four rows in their shape.
fn runs_skeleton() -> AnyElement {
    let rows = ["program", "config", "args", "env"].map(|k| {
        CardItem::placed(move |corners| {
            corners
                .apply(v_flex())
                .id(SharedString::from(format!("agent-runs-reading-{k}")))
                .px(corners.inset)
                .py_3()
                .min_h(size::ROW_FLOOR)
                .justify_center()
                .gap_2()
                .child(Skeleton::new(px(128.), px(14.)))
                .child(Skeleton::new(px(224.), px(12.)))
                .into_any_element()
        })
    });
    Card::new("agent-runs").head(w::HOW_IT_RUNS).items(rows).into_any_element()
}

/// The change a typed value makes, or the refusal said before anything is sent.
type Change = Rc<dyn Fn(&str) -> Result<Value, String>>;

/// One sheet over the page: a line a person types, saved through `agents.setup`.
#[derive(Clone)]
struct Sheet {
    key: &'static str,
    title: &'static str,
    line: String,
    initial: String,
    placeholder: String,
    /// The change a typed value makes, or the refusal said before anything is sent.
    change: Change,
    put_back: Option<Value>,
    put_back_word: String,
    /// What the page says once the host took a value.
    after: Option<String>,
}

fn open_sheet(sheet: Sheet, place: String, agent: String, store: Entity<Store>, acts: &Acts, cx: &mut App) {
    store.update(cx, |s, _| {
        s.sheet_saving = false;
        s.sheet_refusal = None;
    });
    let sheet = Rc::new(sheet);
    acts.open_dialog(
        DialogBuild::dialog(move |_, window, cx| {
            let write = {
                let (store, place, agent, after) = (store.clone(), place.clone(), agent.clone(), sheet.after.clone());
                Rc::new(move |change: Value, done: bool, cx: &mut App| {
                    let (place, agent, after) = (place.clone(), agent.clone(), after.clone());
                    store.update(cx, |s, cx| {
                        s.sheet_saving = true;
                        s.sheet_refusal = None;
                        s.setup(&place, &agent, change, cx, move |s, answer, cx| {
                            s.sheet_saving = false;
                            match answer {
                                Ok(()) => {
                                    s.close_dialog(cx);
                                    if let (true, Some(after)) = (done, after) {
                                        s.done(after, cx);
                                    }
                                }
                                Err(said) => s.sheet_refusal = Some(said),
                            }
                        });
                        cx.notify();
                    });
                })
            };
            let save = {
                let (sheet, write, store) = (sheet.clone(), write.clone(), store.clone());
                move |input: &Entity<gpui_kit::component::input::InputState>, _: &mut Window, cx: &mut App| {
                    let value = keyed::words(input, cx).trim().to_owned();
                    match (sheet.change)(&value) {
                        Ok(change) => write(change, !value.is_empty(), cx),
                        Err(said) => store.update(cx, |s, cx| {
                            s.sheet_refusal = Some(said);
                            cx.notify();
                        }),
                    }
                }
            };
            let input =
                keyed::typed(format!("agent-sheet-{}", sheet.key), &sheet.placeholder, &sheet.initial, false, save.clone(), window, cx);
            keyed::focus_once(format!("agent-sheet-{}-focused", sheet.key), &input, window, cx);
            let (saving, said) = {
                let s = store.read(cx);
                (s.sheet_saving, s.sheet_refusal.clone())
            };
            let mono = cx.theme().mono_font_family.clone();
            let close = store.clone();
            let mut save_button = Button::primary(format!("agent-{}-save", sheet.key), w::SAVE).held(saving);
            if !saving {
                let input = input.clone();
                save_button = save_button.on_click(move |_, window, cx| save(&input, window, cx));
            }
            let mut dialog = AlertDialog::new(sheet.title)
                .layered()
                .on_outside(layer::dismiss)
                .sentence(sheet.line.clone())
                .child(v_flex().gap_3().child(div().font_family(mono).child(sheet_field(&input, cx))).child(refusal(said.as_deref())));
            if let Some(change) = sheet.put_back.clone() {
                let write = write.clone();
                dialog = dialog.act(
                    Button::ghost(format!("agent-{}-put-back", sheet.key), sheet.put_back_word.clone())
                        .held(saving)
                        .on_click(move |_, _, cx| write(change.clone(), false, cx)),
                );
            }
            dialog
                .act(
                    Button::outline(format!("agent-{}-cancel", sheet.key), w::CANCEL)
                        .on_click(move |_, _, cx| close.update(cx, |s, cx| s.close_dialog(cx))),
                )
                .act(save_button)
                .into_any_element()
        }),
        cx,
    );
}

/// A sheet's field: `FIELD`'s 40 px, the whole width.
fn sheet_field(input: &Entity<gpui_kit::component::input::InputState>, cx: &App) -> AnyElement {
    let p = cx.palette();
    Styled::h(gpui_kit::component::input::Input::new(input), px(40.))
        .w_full()
        .rounded(size::BUTTON_RADIUS)
        .px(px(11.))
        .py_0()
        .border_color(p.input)
        .bg(p.input_fill)
        .text_size(text::FACT.0)
        .into_any_element()
}

/// Whose variables the environment sheet edits, and where.
#[derive(Clone)]
struct EnvOn {
    label: String,
    computer: String,
    names: Vec<String>,
    place: String,
    agent: String,
}

/// The environment sheet: the names every launch carries, each with its remove, and a name and a value to add one.
fn open_env(on: EnvOn, store: Entity<Store>, acts: &Acts, cx: &mut App) {
    let EnvOn { label, computer, names, place, agent } = on;
    store.update(cx, |s, _| {
        s.sheet_saving = false;
        s.sheet_refusal = None;
        s.env_kept = names.clone();
    });
    acts.open_dialog(
        DialogBuild::dialog(move |_, window, cx| {
            let p = cx.palette().clone();
            // A staged value lives only while the sheet is drawn, as the web's sheet keeps it in its own state.
            let staged = window.use_keyed_state("agent-env-staged", cx, |_, _| Vec::<(String, String)>::new());
            let added = staged.read(cx).clone();
            let (kept, saving, said) = {
                let s = store.read(cx);
                (s.env_kept.clone(), s.sheet_saving, s.sheet_refusal.clone())
            };
            let shown: Vec<String> =
                kept.iter().cloned().chain(added.iter().map(|(n, _)| n.clone()).filter(|n| !kept.contains(n))).collect();
            let mut list = v_flex().debug_selector(|| "agent-env-names".into());
            if shown.is_empty() {
                list = list.child(div().py_2().text_size(px(13.)).text_color(p.muted_foreground).child(w::NO_VARIABLES));
            }
            let count = shown.len();
            for (at, name) in shown.into_iter().enumerate() {
                let (store, staged) = (store.clone(), staged.clone());
                let gone = name.clone();
                list = list.child(
                    h_flex()
                        .h(px(36.))
                        .justify_between()
                        .gap_3()
                        .when(at + 1 < count, |el| el.border_b_1().border_color(p.border.opacity(0.6)))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_size(px(13.))
                                .child(name.clone()),
                        )
                        .child(
                            Button::icon_xs(SharedString::from(format!("env-{name}-remove")), Glyph::Close)
                                .tooltip(w::remove_named(&name))
                                .on_click(move |_, _, cx| {
                                    staged.update(cx, |a, cx| {
                                        a.retain(|(n, _)| *n != gone);
                                        cx.notify();
                                    });
                                    store.update(cx, |s, cx| {
                                        s.env_kept.retain(|n| *n != gone);
                                        cx.notify();
                                    })
                                }),
                        ),
                );
            }
            let name_field = keyed::typed("agent-env-name", w::VARIABLE_NAME, "", false, |_, _, _| {}, window, cx);
            let stage = {
                let (store, staged, name_field, label) = (store.clone(), staged.clone(), name_field.clone(), label.clone());
                move |value_field: &Entity<gpui_kit::component::input::InputState>, window: &mut Window, cx: &mut App| {
                    let name = keyed::words(&name_field, cx).trim().to_owned();
                    let value = keyed::words(value_field, cx);
                    if name.is_empty() || value.is_empty() {
                        return;
                    }
                    if let Some(said) = env_refusal(&name, &label) {
                        store.update(cx, |s, cx| {
                            s.sheet_refusal = Some(said);
                            cx.notify();
                        });
                        return;
                    }
                    keyed::clear(&name_field, window, cx);
                    keyed::clear(value_field, window, cx);
                    staged.update(cx, |a, cx| {
                        a.retain(|(n, _)| *n != name);
                        a.push((name, value));
                        cx.notify();
                    });
                    store.update(cx, |s, cx| {
                        s.sheet_refusal = None;
                        cx.notify();
                    });
                }
            };
            let value_field = keyed::typed("agent-env-value", w::VARIABLE_VALUE, "", true, stage.clone(), window, cx);
            let held = keyed::words(&name_field, cx).trim().is_empty() || keyed::words(&value_field, cx).is_empty();
            let mut add = Button::outline("agent-env-add", w::ADD).held(held);
            if !held {
                let value_field = value_field.clone();
                add = add.on_click(move |_, window, cx| stage(&value_field, window, cx));
            }
            let save = {
                let (store, staged, names, place, agent) = (store.clone(), staged.clone(), names.clone(), place.clone(), agent.clone());
                move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                    let (place, agent) = (place.clone(), agent.clone());
                    let names = names.clone();
                    let added = staged.read(cx).clone();
                    store.update(cx, |s, cx| {
                        let mut env = serde_json::Map::new();
                        for (n, v) in &added {
                            env.insert(n.clone(), json!(v));
                        }
                        for was in names.iter().filter(|n| !s.env_kept.contains(n)) {
                            env.insert(was.clone(), Value::Null);
                        }
                        if env.is_empty() {
                            s.close_dialog(cx);
                            return;
                        }
                        s.sheet_saving = true;
                        s.setup(&place, &agent, json!({ "env": env }), cx, |s, answer, cx| {
                            s.sheet_saving = false;
                            match answer {
                                Ok(()) => s.close_dialog(cx),
                                Err(said) => s.sheet_refusal = Some(said),
                            }
                        });
                        cx.notify();
                    });
                }
            };
            let close = store.clone();
            let mono = cx.theme().mono_font_family.clone();
            AlertDialog::new(w::ENVIRONMENT)
                .layered()
                .on_outside(layer::dismiss)
                .sentence(w::environment_sheet(&label, &computer))
                .child(
                    v_flex()
                        .gap_3()
                        .child(list)
                        .child(
                            h_flex()
                                .gap_2()
                                .font_family(mono)
                                .child(div().w(relative(0.4)).child(sheet_field(&name_field, cx)))
                                .child(div().flex_1().min_w_0().child(sheet_field(&value_field, cx)))
                                .child(add),
                        )
                        .child(refusal(said.as_deref())),
                )
                .act(Button::outline("agent-env-cancel", w::CANCEL).on_click(move |_, _, cx| close.update(cx, |s, cx| s.close_dialog(cx))))
                .act(Button::primary("agent-env-save", w::SAVE).held(saving).on_click(save))
                .into_any_element()
        }),
        cx,
    );
}

/// How it runs on the picked computer: program, config folder, launch words and variables.
fn runs(id: &str, label: &str, row: &AgentRow, store: &Entity<Store>, rows: &Rows, ctx: &Ctx) -> Option<AnyElement> {
    let setup = row.setup.clone()?;
    let bin = entry(id).map(|e| e.bin.to_owned()).unwrap_or_else(|| id.to_owned());
    let (place, computer) = (rows.place.clone(), rows.name.clone());
    let direct = |change: Value| {
        let (store, place, agent) = (store.clone(), place.clone(), id.to_owned());
        move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
            let change = change.clone();
            store.update(cx, |s, cx| {
                s.setup(&place, &agent, change, cx, |s, answer, cx| {
                    if let Err(said) = answer {
                        s.failed(said, cx);
                    }
                })
            });
        }
    };
    let change = |key: &'static str, word: &'static str, sheet: Sheet| {
        let (store, place, agent, acts) = (store.clone(), place.clone(), id.to_owned(), ctx.acts.clone());
        Button::outline(format!("agent-{key}"), word)
            .xs()
            .on_click(move |_, _, cx| open_sheet(sheet.clone(), place.clone(), agent.clone(), store.clone(), &acts, cx))
    };
    let program_sheet = Sheet {
        key: "program",
        title: w::PROGRAM,
        line: w::program_sheet(label, &computer),
        initial: setup.program.clone().unwrap_or_default(),
        placeholder: row.path.clone().unwrap_or_else(|| bin.clone()),
        change: Rc::new(|v: &str| Ok(if v.is_empty() { json!({ "program": null }) } else { json!({ "program": v }) })),
        put_back: setup.program.as_ref().map(|_| json!({ "program": null })),
        put_back_word: w::put_back(label),
        after: None,
    };
    let config_sheet = Sheet {
        key: "config",
        title: w::CONFIG_FOLDER,
        line: w::config_sheet(label, &computer),
        initial: setup.config_dir.clone().unwrap_or_default(),
        placeholder: String::new(),
        change: Rc::new(|v: &str| Ok(if v.is_empty() { json!({ "configDir": null }) } else { json!({ "configDir": v }) })),
        put_back: setup.config_dir.as_ref().map(|_| json!({ "configDir": null })),
        put_back_word: w::put_back(label),
        after: Some(w::config_dir_sign_in(label)),
    };
    let args_sheet = Sheet {
        key: "args",
        title: w::LAUNCH_ARGUMENTS,
        line: w::arguments_sheet(label, &computer),
        initial: shell_line(&setup.args),
        placeholder: String::new(),
        change: Rc::new(|v: &str| match command_words(v) {
            None => Err(w::ARGUMENTS_UNCLOSED.to_owned()),
            Some(words) if words.is_empty() => Ok(json!({ "args": null })),
            Some(words) => Ok(json!({ "args": words })),
        }),
        put_back: (!setup.args.is_empty()).then(|| json!({ "args": null })),
        put_back_word: w::put_back(label),
        after: None,
    };
    let mut program = Row::new("agent-program", w::PROGRAM)
        .note(setup.program.clone().or_else(|| row.path.clone()).unwrap_or_else(|| bin.clone()))
        .mono()
        .control(change("program-change", w::CHANGE, program_sheet));
    if setup.program.is_some() {
        program = program.on_reset(direct(json!({ "program": null })));
    }
    let mut config = Row::new("agent-config", w::CONFIG_FOLDER)
        .note(setup.config_dir.clone().unwrap_or_else(|| w::own_folder(label)))
        .control(change("config-change", w::CHANGE, config_sheet));
    if setup.config_dir.is_some() {
        config = config.mono().on_reset(direct(json!({ "configDir": null })));
    }
    let mut args = Row::new("agent-args", w::LAUNCH_ARGUMENTS).note(w::arguments_count(setup.args.len())).control(change(
        "args-edit",
        w::EDIT,
        args_sheet,
    ));
    if !setup.args.is_empty() {
        args = args.on_reset(direct(json!({ "args": null })));
    }
    let env = {
        let (store, place, agent, acts, label, computer, names) =
            (store.clone(), place.clone(), id.to_owned(), ctx.acts.clone(), label.to_owned(), computer.clone(), setup.env_names.clone());
        Row::new("agent-env", w::ENVIRONMENT).note(w::variables_count(setup.env_names.len())).control(
            Button::outline("agent-env-edit", w::EDIT).xs().on_click(move |_, _, cx| {
                open_env(
                    EnvOn {
                        label: label.clone(),
                        computer: computer.clone(),
                        names: names.clone(),
                        place: place.clone(),
                        agent: agent.clone(),
                    },
                    store.clone(),
                    &acts,
                    cx,
                )
            }),
        )
    };
    Some(Card::new("agent-runs").head(w::HOW_IT_RUNS).item(program).item(config).item(args).item(env).into_any_element())
}

/// The sign-in words and the account's address after them, `Spaced`: a space and 8 px between, never a dot.
fn head_line(said: SignInHead, address: Option<String>) -> AnyElement {
    let line = if address.is_some() { format!("{} ", said.line) } else { said.line };
    h_flex()
        .flex_wrap()
        .gap_x_2()
        .child(
            div()
                .id("agent-sign-in")
                .debug_selector(|| "agent-sign-in-words".into())
                .flex_none()
                .whitespace_nowrap()
                .tooltip(tip(said.whole))
                .child(line),
        )
        .children(address.map(|a| div().debug_selector(|| "agent-address".into()).flex_none().whitespace_nowrap().child(a)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Context, Font, TestAppContext, TextRun};

    use super::*;

    struct Head;

    impl Render for Head {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let said = SignInHead { line: "Signed in with Max".into(), whole: "Signed in with Max".into() };
            div().w(px(600.)).text_size(px(13.)).child(head_line(said, Some("me@example.com".into())))
        }
    }

    #[gpui_kit::test]
    fn the_account_address_stands_a_space_and_8_px_after_the_sign_in_words(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let (_, cx) = cx.add_window_view(|_, _| Head);
        cx.run_until_parked();
        let address = cx.debug_bounds("agent-address").expect("the address");
        let words = cx.debug_bounds("agent-sign-in-words").expect("the sign-in words");
        let width = |text: &str, cx: &mut gpui_kit::VisualTestContext| {
            cx.update(|window, _| {
                let font: Font = window.text_style().font();
                let run = TextRun {
                    len: text.len(),
                    font,
                    color: gpui_kit::black(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                window.text_system().shape_line(text.to_owned().into(), px(13.), &[run], None).width
            })
        };
        let spaced = width("Signed in with Max ", cx) + px(8.);
        let gap = address.left() - words.left();
        // Within a pixel, as layout rounds each box to the pixel; a space is about 3.6 px at 13 px.
        assert!((gap - spaced).abs() < px(1.), "the address at {gap:?} from the words' start, Spaced puts it at {spaced:?}");
    }

    #[gpui_kit::test]
    fn a_staged_value_goes_with_the_environment_sheet(cx: &mut TestAppContext) {
        use std::cell::RefCell;

        use crate::screens::settings::model::{At, Data, GroupId};
        use crate::screens::settings::SettingsScreen;
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        // Every list of staged pairs made from here on, held weakly, so the test sees whether one outlives the sheet.
        type Staged = Vec<(String, String)>;
        let made: Rc<RefCell<Vec<gpui_kit::WeakEntity<Staged>>>> = Rc::default();
        let seen = made.clone();
        cx.update(|cx| cx.observe_new(move |_: &mut Staged, _, cx| seen.borrow_mut().push(cx.entity().downgrade())).detach());
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let mut screen = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let view = cx.new(|cx| SettingsScreen::with_data(Data::default(), At::Group(GroupId::Agents), runtime.clone(), window, cx));
            screen = Some(view.clone());
            gpui_kit::component::Root::new(view, window, cx)
        });
        let screen = screen.unwrap();
        cx.simulate_resize(gpui_kit::size(px(1280.), px(900.)));
        let frame = |cx: &mut gpui_kit::VisualTestContext| {
            cx.run_until_parked();
            cx.update(|window, cx| window.simulate_next_frame(cx));
            cx.run_until_parked();
        };
        let on =
            EnvOn { label: "Claude Code".into(), computer: "mac".into(), names: Vec::new(), place: "here".into(), agent: "claude".into() };
        let store = screen.read_with(cx, |s, _| s.agents.clone());
        let acts = Acts::new(screen.downgrade(), None, runtime);
        cx.update(|_, cx| open_env(on, store, &acts, cx));
        frame(cx);
        frame(cx);
        assert!(cx.update(|_, cx| layer::is_open(crate::screens::settings::DIALOG, cx)), "the sheet is open");
        // Its first tab stop is the name field.
        cx.simulate_input("API_KEY");
        cx.simulate_keystrokes("tab");
        cx.simulate_input("s3cret");
        cx.simulate_keystrokes("enter");
        frame(cx);
        let holding = |cx: &mut gpui_kit::VisualTestContext| {
            cx.update(|_, cx| made.borrow().iter().filter_map(|w| w.upgrade()).any(|e| e.read(cx).iter().any(|(_, v)| v == "s3cret")))
        };
        assert!(holding(cx), "Enter staged the value in the sheet");
        screen.update(cx, |s, cx| s.close_dialog(cx));
        frame(cx);
        // The layer draws a closed dialog on for its 200 ms fade out, by the wall's clock.
        std::thread::sleep(std::time::Duration::from_millis(250));
        frame(cx);
        frame(cx);
        assert!(!holding(cx), "closed without a save, the sheet took the value with it");
    }
}
