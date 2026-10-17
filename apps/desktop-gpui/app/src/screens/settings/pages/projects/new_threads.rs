// SPDX-License-Identifier: AGPL-3.0-only
//! New threads in this project (`ProjectNewThreads`): the project's own agent, model and access over each agent's, each
//! row naming what it takes while unset and carrying the arrow back while set, written through the host's preferences.

use std::collections::HashMap;

use gpui_kit::{px, App, Entity, Font, Pixels, SharedString, TextRun, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::sidebar::ProjectsDefaults;
use wsp_host_client::ops::thread::Harnesses;
use wsp_host_client::protocol::{HarnessesListRequest, ProjectsDefaultsRequest};

use super::super::agents::marks::harness_mark;
use super::super::agents::model::{access_held, new_thread_defaults, reshape, w, Catalog, Prefs, ACCESS};
use super::look;
use crate::screens::settings::model::req;
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{size, text, ActivePalette, Select, SelectItem};

/// `PROJECTS_WORDS`.
const NEW_THREADS: &str = "New threads in this project";
const DEFAULT_AGENT: &str = "Default agent";
const MODEL: &str = "Model";
const ACCESS_TITLE: &str = "Access";
const AGENT_SET: &str = "Set here for this project. The arrow goes back to the default for every project.";
const AGENT_UNSET: &str = "Unset, so a new thread takes the default for every project.";
const OWN_SET: &str = "Set here for this project. The arrow goes back to the agent's own.";
const OWN_UNSET: &str = "Unset, so a new thread takes the agent's own.";

/// A word no agent, model or access is spelled as: the select's value while the project names none.
const UNSET: &str = "@unset";

/// `PROJECTS_WORDS.inherits`.
fn inherits(value: &str, from: Option<&str>) -> String {
    match from {
        Some(from) => format!("Default ({value}, from {from})"),
        None => format!("Default ({value})"),
    }
}

fn access_word(word: &str) -> String {
    w::ACCESS_WORDS.iter().find(|(v, _)| *v == word).map(|(_, l)| l.to_string()).unwrap_or_else(|| word.to_owned())
}

/// What each project's new threads start on as the host resolves it, read again whenever the record it is resolved
/// from moves.
#[derive(Default)]
struct Resolved {
    moved: String,
    defaults: HashMap<String, Value>,
}

fn resolved(ctx: &Ctx, id: &str, window: &mut Window, cx: &mut App) -> Option<Value> {
    let moved = ctx
        .data
        .preferences()
        .map(|p| {
            json!([p.default_agent, serde_json::to_value(&p.agent_defaults).ok(), serde_json::to_value(&p.project_defaults).ok()])
                .to_string()
        })
        .unwrap_or_default();
    let state = window.use_keyed_state("projects-defaults", cx, |_, _| Resolved::default());
    if state.read(cx).moved != moved {
        state.update(cx, |s, _| s.moved = moved);
        let (into, acts) = (state.downgrade(), ctx.acts.clone());
        ctx.acts.call(req::<ProjectsDefaultsRequest>(json!({})), cx, move |answer, cx| {
            if let Ok(ProjectsDefaults { defaults }) = answer {
                let defaults = defaults.iter().filter_map(|(id, d)| Some((id.clone(), serde_json::to_value(d).ok()?))).collect();
                _ = into.update(cx, |s, _| s.defaults = defaults);
                acts.notify(cx);
            }
        });
    }
    state.read(cx).defaults.get(id).cloned()
}

/// The agents' lists as the project's computer runs them, as its New thread offers them, read once per page.
fn catalogs(ctx: &Ctx, id: &str, window: &mut Window, cx: &mut App) -> Entity<Vec<Catalog>> {
    let (acts, id) = (ctx.acts.clone(), id.to_owned());
    window.use_keyed_state(SharedString::from(format!("project-{id}-catalogs")), cx, move |_, cx| {
        let (into, again) = (cx.weak_entity(), acts.clone());
        acts.call(req::<HarnessesListRequest>(json!({ "project": id })), cx, move |answer, cx| {
            if let Ok(Harnesses { harnesses }) = answer {
                _ = into.update(cx, |list: &mut Vec<Catalog>, _| *list = harnesses.iter().filter_map(reshape::<Catalog>).collect());
                again.notify(cx);
            }
        });
        Vec::new()
    })
}

/// A select that names what it inherits grows to say all of it, from the one width the rest take.
fn inherits_width(words: &str, window: &mut Window, cx: &App) -> Pixels {
    let font = Font { family: gpui_kit::component::ActiveTheme::theme(cx).font_family.clone(), ..gpui_kit::font("") };
    let run =
        TextRun { len: words.len(), font, color: cx.palette().foreground, background_color: None, underline: None, strikethrough: None };
    let line = window.text_system().shape_line(SharedString::from(words.to_owned()), text::FACT, &[run], None);
    // The trigger's 9 px sides, its 1 px edge, the gap and the 12 px chevron set 4 px into the right side.
    size::SELECT_WIDTH.max((line.width + px(36.)).ceil())
}

/// The card, or nothing until the default agent and the project's lists for its agent are read.
pub fn card(project_id: &str, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let prefs: Prefs = ctx.data.preferences().and_then(reshape::<Prefs>).unwrap_or_default();
    let own: Value = ctx
        .data
        .preferences()
        .and_then(|p| p.project_defaults.get(project_id))
        .and_then(|d| serde_json::to_value(d).ok())
        .unwrap_or_default();
    let own_of = |key: &str| own.get(key).and_then(Value::as_str).map(str::to_owned);
    let (own_agent, own_model, own_access) = (own_of("agent"), own_of("model"), own_of("access"));
    let resolved = resolved(ctx, project_id, window, cx);
    let catalogs = catalogs(ctx, project_id, window, cx);

    let first = ctx.data.harnesses.first()?;
    let global = prefs.default_agent.as_deref().and_then(|id| ctx.data.harnesses.iter().find(|h| h.harness == id)).unwrap_or(first);
    let global_label = global.label.clone();
    let agent_id = resolved
        .as_ref()
        .and_then(|r| r.pointer("/agent/value").and_then(Value::as_str).map(str::to_owned))
        .or_else(|| own_agent.clone())
        .unwrap_or_else(|| global.harness.to_string());
    let catalog = catalogs.read(cx).iter().find(|c| c.harness == agent_id).cloned()?;
    let picks = new_thread_defaults(&catalog, &prefs);

    let set = {
        let (acts, id) = (ctx.acts.clone(), project_id.to_owned());
        move |patch: Value, cx: &mut App| acts.set_preferences(json!({ "projectDefaults": { id.clone(): patch } }), cx)
    };
    let mut card = CardData::new("project-new-threads").head(NEW_THREADS);

    let unset_agent = inherits(&global_label, None);
    let harnesses: Vec<(String, String)> = ctx.data.harnesses.iter().map(|h| (h.harness.to_string(), h.label.clone())).collect();
    let agent_items = {
        let (unset, harnesses) = (unset_agent.clone(), harnesses.clone());
        move || {
            let mut items = vec![SelectItem::new(UNSET.to_owned(), unset.clone())];
            items.extend(harnesses.iter().map(|(id, label)| {
                let (mark_id, mark_label) = (id.clone(), label.clone());
                SelectItem::new(id.clone(), label.clone()).mark(move |cx| harness_mark(&mark_id, &mark_label, px(16.), cx))
            }));
            items
        }
    };
    let agent_value = own_agent.clone().unwrap_or_else(|| UNSET.to_owned());
    let agent_shown = if own_agent.is_some() {
        harnesses.iter().find(|(id, _)| *id == agent_value).map_or(agent_value.clone(), |(_, l)| l.clone())
    } else {
        unset_agent.clone()
    };
    let signature = format!("{unset_agent} {harnesses:?}");
    let state = look::select_state(format!("project-{project_id}-agent").into(), agent_items, signature, &agent_value, window, cx);
    let width = inherits_width(&agent_shown, window, cx) + if own_agent.is_some() { px(24.) } else { px(0.) };
    let agent_select = {
        let set = set.clone();
        Select::new(&state)
            .width(width)
            .on_change(move |next: &String, _, cx| set(json!({ "agent": if next == UNSET { Value::Null } else { json!(next) } }), cx))
    };
    let reset_agent = own_agent.is_some().then(|| set.clone());
    card = card.item(Item::row("project-agent", DEFAULT_AGENT, Some(if own_agent.is_none() { AGENT_UNSET } else { AGENT_SET }), |row| {
        let row = row.control(agent_select);
        match reset_agent {
            Some(set) => row.on_reset(move |_, _, cx| set(json!({ "agent": null }), cx)),
            None => row,
        }
    }));

    if let Some(model) = &picks.model {
        let unset_model = inherits(&catalog.model_of(&model.value).label, None);
        let model_set =
            own_model.is_some() && resolved.as_ref().and_then(|r| r.pointer("/model/from")).and_then(Value::as_str) == Some("project");
        let models: Vec<(String, String)> =
            catalog.models.iter().chain(catalog.legacy_models.iter().flatten()).map(|m| (m.value.clone(), m.label.clone())).collect();
        let value = if model_set { own_model.clone().unwrap_or_default() } else { UNSET.to_owned() };
        let shown = if model_set { catalog.model_of(&value).label } else { unset_model.clone() };
        let items = {
            let (unset, models) = (unset_model.clone(), models.clone());
            move || {
                let mut items = vec![SelectItem::new(UNSET.to_owned(), unset.clone())];
                items.extend(models.iter().map(|(v, l)| SelectItem::new(v.clone(), l.clone())));
                items
            }
        };
        let state = look::select_state(
            format!("project-{project_id}-model").into(),
            items,
            format!("{unset_model} {models:?}"),
            &value,
            window,
            cx,
        );
        let width = inherits_width(&shown, window, cx);
        let select = {
            let set = set.clone();
            Select::new(&state)
                .width(width)
                .on_change(move |next: &String, _, cx| set(json!({ "model": if next == UNSET { Value::Null } else { json!(next) } }), cx))
        };
        let reset = model_set.then(|| set.clone());
        card = card.item(Item::row("project-model", MODEL, Some(if model_set { OWN_SET } else { OWN_UNSET }), |row| {
            let row = row.control(select);
            match reset {
                Some(set) => row.on_reset(move |_, _, cx| set(json!({ "model": null }), cx)),
                None => row,
            }
        }));
    }

    if let Some(access) = &picks.access {
        let unset_access = inherits(&access_word(&access.value), Some(&catalog.label));
        let words: Vec<String> = ACCESS.iter().filter(|word| access_held(&catalog, word).is_none()).map(|w| w.to_string()).collect();
        let value = own_access.clone().unwrap_or_else(|| UNSET.to_owned());
        let shown = if own_access.is_some() { access_word(&value) } else { unset_access.clone() };
        let items = {
            let (unset, words) = (unset_access.clone(), words.clone());
            move || {
                let mut items = vec![SelectItem::new(UNSET.to_owned(), unset.clone())];
                items.extend(words.iter().map(|w| SelectItem::new(w.clone(), access_word(w))));
                items
            }
        };
        let state = look::select_state(
            format!("project-{project_id}-access").into(),
            items,
            format!("{unset_access} {words:?}"),
            &value,
            window,
            cx,
        );
        let width = inherits_width(&shown, window, cx);
        let select = {
            let set = set.clone();
            Select::new(&state)
                .width(width)
                .on_change(move |next: &String, _, cx| set(json!({ "access": if next == UNSET { Value::Null } else { json!(next) } }), cx))
        };
        let reset = own_access.is_some().then(|| set.clone());
        card = card.item(Item::row("project-access", ACCESS_TITLE, Some(if own_access.is_none() { OWN_UNSET } else { OWN_SET }), |row| {
            let row = row.control(select);
            match reset {
                Some(set) => row.on_reset(move |_, _, cx| set(json!({ "access": null }), cx)),
                None => row,
            }
        }));
    }
    Some(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unset_row_names_what_it_takes() {
        assert_eq!(inherits("Claude Code", None), "Default (Claude Code)");
        assert_eq!(inherits(&access_word("full"), Some("Claude Code")), "Default (Full, from Claude Code)");
    }
}
