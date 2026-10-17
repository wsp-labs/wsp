// SPDX-License-Identifier: AGPL-3.0-only
//! One agent's own page (`agentPage.tsx`): its head (version, sign-in, the vendor's update and whether it is on there),
//! what a new thread runs it with and the models its picker lists, all kept in the preferences, then how it runs on
//! the picked computer, written through `agents.setup`.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Entity, SharedString, Window};
use serde_json::{json, Value};

use super::keyed;
use super::marks::harness_mark;
use super::model::{
    access_held, agent_name, command_words, entry, env_refusal, model_id_refusal, new_thread_defaults, plan_word, shell_line, sign_in_head,
    w, AgentRow, Catalog, Model, Picker, Prefs, ACCESS,
};
use super::parts::{field, refusal};
use super::store::Store;
use super::{catalogs, prefs, Rows};
use crate::screens::settings::page::Ctx;
use crate::screens::settings::Acts;
use crate::ui::{
    size, text, tip, ActivePalette, AlertDialog, Button, Card, CardItem, Glyph, HeadRow, Row, Segment, SegmentedControl, Select, Skeleton,
    Switch,
};

/// The models an agent's picker lists, in its order, then the ones taken off it.
fn picker_models(catalog: &Catalog) -> Vec<Model> {
    catalog.models.iter().chain(catalog.legacy_models.iter().flatten()).cloned().collect()
}

pub fn page(id: &str, ctx: &Ctx, store: &Entity<Store>, rows: &Rows, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    let (row, reading, accounts) = {
        let s = store.read(cx);
        let read = s.reads.get(&rows.place);
        (
            read.and_then(|r| r.report.as_ref()).and_then(|r| r.agents.iter().find(|a| a.id == id)).cloned(),
            read.is_some_and(|r| r.reading),
            s.accounts.clone(),
        )
    };
    let catalog = catalogs(ctx).into_iter().find(|c| c.harness == id);
    let label =
        catalog.as_ref().map(|c| c.label.clone()).or_else(|| row.as_ref().map(|r| r.name.clone())).unwrap_or_else(|| agent_name(id));
    let computer = rows.name.clone();
    let account = accounts.and_then(|all| all.into_iter().find(|a| a.agent == id && a.computers.contains(&computer)));
    let mut out = vec![head(id, &label, row.as_ref(), account.as_ref(), store, rows, ctx, cx)];
    let prefs = prefs(ctx);
    if let Some(catalog) = &catalog {
        out.push(new_threads(catalog, &prefs, ctx, window, cx));
        out.extend(models_card(catalog, &prefs, ctx, window, cx));
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
    account: Option<&super::store::Account>,
    store: &Entity<Store>,
    rows: &Rows,
    ctx: &Ctx,
    cx: &App,
) -> AnyElement {
    let brand = entry(id).and_then(|e| e.plan_brand);
    let plan = row.and_then(|r| r.sign_in_plan.clone()).or_else(|| account.and_then(|a| a.plan.clone()).filter(|p| !p.is_empty()));
    let mut head = HeadRow::new("agent-head", harness_mark(id, label, px(16.), cx), label.to_owned());
    if let Some(version) = row.and_then(|r| r.version.clone()) {
        head = head.mark(version);
    }
    if let Some(row) = row {
        let line = if row.installed {
            let said = sign_in_head(row, &rows.name, plan.map(|p| plan_word(&p, brand)));
            h_flex()
                .flex_wrap()
                .gap_x_2()
                .child(div().id("agent-sign-in").tooltip(tip(said.whole)).child(said.line))
                .children(account.and_then(|a| a.address.clone()))
                .into_any_element()
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
fn new_threads(catalog: &Catalog, prefs: &Prefs, ctx: &Ctx, window: &mut Window, cx: &mut App) -> AnyElement {
    let id = catalog.harness.clone();
    let own = prefs.own(&id);
    let defaults = new_thread_defaults(catalog, prefs);
    let model = defaults.model.as_ref().map(|m| m.value.clone());
    let set = {
        let (acts, id) = (ctx.acts.clone(), id.clone());
        Rc::new(move |patch: Value, cx: &mut App| acts.set_preferences(json!({ "agentDefaults": { id.clone(): patch } }), cx))
    };
    let models = picker_models(catalog);
    let mut card = Card::new("agent-new-threads").head(w::NEW_THREADS);
    if !models.is_empty() {
        let mut listed = models.clone();
        if let Some(m) = &model {
            if !listed.iter().any(|l| l.value == *m) {
                listed.push(catalog.model_of(m));
            }
        }
        let items = listed.iter().map(|m| (m.value.clone(), SharedString::from(m.label.clone()), None)).collect();
        let model_select = keyed::pick(format!("agent-{id}-model"), items, model.clone(), window, cx);
        let pick_set = set.clone();
        let model_select = Select::new(&model_select).on_change(move |v: &String, _, cx| pick_set(json!({ "model": v }), cx));
        let efforts = catalog.efforts_for(model.as_ref().map(|m| catalog.model_of(m)).as_ref());
        let mut control = h_flex().gap_2().child(model_select);
        if !efforts.is_empty() {
            let items = efforts.iter().map(|o| (o.value.clone(), SharedString::from(o.label.clone()), None)).collect();
            let effort = defaults.effort.as_ref().map(|e| e.value.clone());
            let effort_select = keyed::pick(format!("agent-{id}-effort"), items, effort, window, cx);
            let pick_set = set.clone();
            control = control
                .child(Select::new(&effort_select).width(px(96.)).on_change(move |v: &String, _, cx| pick_set(json!({ "effort": v }), cx)));
        }
        let mut row = Row::new("agent-model", w::MODEL).note(w::MODEL_DESCRIPTION).control(control);
        if own.model.is_some() || own.effort.is_some() {
            let set = set.clone();
            row = row.on_reset(move |_, _, cx| set(json!({ "model": null, "effort": null }), cx));
        }
        card = card.item(row);
    }
    let access = defaults.access.as_ref().map(|a| a.value.clone()).unwrap_or_default();
    let segments = ACCESS
        .iter()
        .map(|word| {
            let label = w::ACCESS_WORDS.iter().find(|(v, _)| v == word).map(|(_, l)| l.to_string()).unwrap_or_default();
            let segment = Segment::new(word.to_string(), label);
            match access_held(catalog, word) {
                Some(why) => segment.held(why),
                None => segment,
            }
        })
        .collect();
    let pick_set = set.clone();
    let mut row = Row::new("agent-access", w::ACCESS).note(w::access_description(&catalog.label)).control(
        SegmentedControl::new("agent-access", access, segments).on_change(move |word, _, cx| pick_set(json!({ "access": word }), cx)),
    );
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
        picker.and_then(|p| p.order.clone()).unwrap_or_default()
    };
    let kept = Picker {
        hide: (!hide.is_empty()).then_some(hide),
        order: (!order.is_empty()).then_some(order),
        custom: (!custom.is_empty()).then(|| custom.to_vec()),
    };
    let models = if kept.is_empty() { Value::Null } else { serde_json::to_value(&kept).unwrap_or(Value::Null) };
    acts.set_preferences(json!({ "agentDefaults": { id: { "models": models } } }), cx);
}

/// Models: every model the agent offers, the picker's own first in its order, then the ones hidden from it.
fn models_card(catalog: &Catalog, prefs: &Prefs, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let id = catalog.harness.clone();
    let picker = prefs.own(&id).models;
    let custom = picker.as_ref().and_then(|p| p.custom.clone()).unwrap_or_default();
    let mut base = catalog.unmarked();
    base.hidden_models = None;
    let shaped = base.with_custom(picker.as_ref()).shaped(picker.as_ref());
    let mut items: Vec<Listed> = picker_models(&shaped)
        .into_iter()
        .map(|m| Listed { added: m.added == Some(true), value: m.value, label: m.label, shown: true })
        .collect();
    items.extend(shaped.hidden_models.iter().flatten().map(|m| Listed {
        value: m.value.clone(),
        label: m.label.clone(),
        added: m.added == Some(true),
        shown: false,
    }));
    if items.is_empty() {
        return None;
    }
    let fallback = new_thread_defaults(catalog, prefs).model.map(|m| m.value);
    let p = cx.palette().clone();
    let mut card = Card::new("agent-models").head(w::MODELS).lede(w::MODELS_DESCRIPTION);
    for m in &items {
        let lead = if m.shown {
            Button::icon_xs(SharedString::from(format!("model-{}-grip", m.value)), Glyph::GripVertical)
                .tooltip(w::reorder(&m.label))
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
                        let moved = picker.as_ref().is_some_and(|p| p.order.is_some());
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
            .note(note)
            .control(control);
        if fallback.as_deref() == Some(m.value.as_str()) {
            row = row.mark_word(w::DEFAULT_MODEL);
        }
        card = card.item(row);
    }
    let refused_key = format!("agent-{id}-model-refused");
    let refused = window.use_keyed_state(SharedString::from(refused_key), cx, |_, _| None::<String>);
    let add = {
        let (acts, id, picker, all, custom, refused) =
            (ctx.acts.clone(), id.clone(), picker.clone(), items.clone(), custom.clone(), refused.clone());
        move |input: &Entity<gpui_kit::component::input::InputState>, window: &mut Window, cx: &mut App| {
            let value = keyed::words(input, cx).trim().to_owned();
            if value.is_empty() {
                return;
            }
            if let Some(said) = model_id_refusal(&value) {
                refused.update(cx, |r, cx| {
                    *r = Some(said);
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
            let moved = picker.as_ref().is_some_and(|p| p.order.is_some());
            write_picker(&id, picker.as_ref(), &next, &ids, moved, &acts, cx);
        }
    };
    let input = keyed::typed(format!("agent-{id}-model-id"), w::MODEL_ID_PLACEHOLDER, "", false, add.clone(), window, cx);
    let typed = keyed::words(&input, cx);
    if !typed.is_empty() && refused.read(cx).is_some() && model_id_refusal(typed.trim()).is_none() {
        refused.update(cx, |r, _| *r = None);
    }
    let held = typed.trim().is_empty();
    let mut button = Button::outline("agent-models-add", w::ADD).xs().held(held);
    if !held {
        let input = input.clone();
        button = button.on_click(move |_, window, cx| add(&input, window, cx));
    }
    let said = refused.read(cx).clone();
    let field_el = field(&input, None, cx);
    let add_row = CardItem::placed(move |corners| {
        corners
            .apply(v_flex())
            .px(corners.inset)
            .py_3()
            .min_h(size::ROW_FLOOR)
            .justify_center()
            .child(h_flex().gap_2().child(field_el).child(button))
            .into_any_element()
    });
    card = card.item(add_row);
    let refusal_line = said.map(|said| {
        div().px(size::CARD_INSET).mt(px(-12.)).text_size(text::FACT).line_height(px(18.)).text_color(p.error_foreground).child(said)
    });
    Some(v_flex().gap_3().child(card).children(refusal_line).into_any_element())
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
        Rc::new(move |_, window, cx| {
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
        .text_size(text::FACT)
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
        s.env_added.clear();
    });
    acts.open_dialog(
        Rc::new(move |_, window, cx| {
            let p = cx.palette().clone();
            let (kept, added, saving, said) = {
                let s = store.read(cx);
                (s.env_kept.clone(), s.env_added.clone(), s.sheet_saving, s.sheet_refusal.clone())
            };
            let shown: Vec<String> =
                kept.iter().cloned().chain(added.iter().map(|(n, _)| n.clone()).filter(|n| !kept.contains(n))).collect();
            let mut list = v_flex().debug_selector(|| "agent-env-names".into());
            if shown.is_empty() {
                list = list.child(div().py_2().text_size(px(13.)).text_color(p.muted_foreground).child(w::NO_VARIABLES));
            }
            let count = shown.len();
            for (at, name) in shown.into_iter().enumerate() {
                let store = store.clone();
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
                                    store.update(cx, |s, cx| {
                                        s.env_kept.retain(|n| *n != gone);
                                        s.env_added.retain(|(n, _)| *n != gone);
                                        cx.notify();
                                    })
                                }),
                        ),
                );
            }
            let name_field = keyed::typed("agent-env-name", w::VARIABLE_NAME, "", false, |_, _, _| {}, window, cx);
            let stage = {
                let (store, name_field, label) = (store.clone(), name_field.clone(), label.clone());
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
                    store.update(cx, |s, cx| {
                        s.env_added.retain(|(n, _)| *n != name);
                        s.env_added.push((name, value));
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
                let (store, names, place, agent) = (store.clone(), names.clone(), place.clone(), agent.clone());
                move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
                    let (place, agent) = (place.clone(), agent.clone());
                    let names = names.clone();
                    store.update(cx, |s, cx| {
                        let mut env = serde_json::Map::new();
                        for (n, v) in &s.env_added {
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
        initial: shell_line(setup.args.as_deref().unwrap_or_default()),
        placeholder: String::new(),
        change: Rc::new(|v: &str| match command_words(v) {
            None => Err(w::ARGUMENTS_UNCLOSED.to_owned()),
            Some(words) if words.is_empty() => Ok(json!({ "args": null })),
            Some(words) => Ok(json!({ "args": words })),
        }),
        put_back: setup.args.as_ref().map(|_| json!({ "args": null })),
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
    let mut args = Row::new("agent-args", w::LAUNCH_ARGUMENTS)
        .note(w::arguments_count(setup.args.as_ref().map_or(0, Vec::len)))
        .control(change("args-edit", w::EDIT, args_sheet));
    if setup.args.is_some() {
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
