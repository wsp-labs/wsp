// SPDX-License-Identifier: AGPL-3.0-only
//! The Plugins tab: a plugin's state, its acts, its row and its own page.

use gpui_kit::App;
use serde_json::json;
use wsp_host_client::protocol::{PluginRow, PluginRowScope, PluginsToggleRequest};

use super::skills::write;
use super::{plugin_key, plugin_missing, row_target, Detail, Fact, RowView, Under, UnderRow, K};
use crate::screens::settings::model::req;
use crate::screens::settings::pages::agents::model::{w, Status, Tone};
use crate::screens::settings::pages::agents::parts::Lead;
use crate::ui::{Act, Glyph};

fn plugin_status(row: &PluginRow, computer: &str) -> Option<Status> {
    if let Some(missing) = plugin_missing(row, computer) {
        return Some(Status::new("missing", Tone::Waiting, w::MISSING).hover(missing));
    }
    if row.scope == PluginRowScope::User {
        return None;
    }
    let st = if row.on { Status::new("on", Tone::Good, w::ON) } else { Status::new("off", Tone::Quiet, w::OFF) };
    Some(match &row.set_in {
        Some(file) => st.hover(format!("{} is set in the repo at {file}, so it is not switched here.", row.id)),
        None => st,
    })
}

fn plugin_acts(row: &PluginRow, k: &K, cx: &App) -> Vec<Act> {
    if row.scope != PluginRowScope::User || row.missing.is_some() {
        return Vec::new();
    }
    let busy = k.s(cx).busy.contains(&k.key(&plugin_key(row)));
    let (id, label, g) = if row.on { ("turn-off", w::TURN_OFF, Glyph::PowerOff) } else { ("turn-on", w::TURN_ON, Glyph::Power) };
    let act = Act::new(id, label, Some(g));
    let act = if busy {
        act.busy()
    } else {
        let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
        act.run(move |_, cx| {
            let params =
                json!({ "target": row_target(&place, project_id!(row.project)), "agent": row.agent, "plugin": row.id, "on": !row.on });
            store.update(cx, |s, cx| write(s, &place, &plugin_key(&row), req::<PluginsToggleRequest>(params), cx))
        })
    };
    k.rows.hold(vec![act])
}

/// A marketplace's own page where its source is a GitHub repo or an address.
pub(super) fn source_page(source: Option<&str>) -> Option<String> {
    let source = source?;
    if source.starts_with("https://") {
        return Some(source.to_owned());
    }
    let parts: Vec<&str> = source.split('/').collect();
    let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.' || c == '-');
    (parts.len() == 2 && parts.iter().all(|p| ok(p))).then(|| format!("https://github.com/{source}"))
}

pub(super) fn plugin_row_view(row: &PluginRow, computer: &str) -> RowView {
    RowView {
        key: plugin_key(row),
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::Puzzle),
        marks: vec![row.agent.clone()],
        subtext: Some(row.marketplace.clone()),
        status: plugin_status(row, computer),
        quick: None,
    }
}

pub(super) fn plugin_detail(row: &PluginRow, k: &K, cx: &App) -> Detail {
    let status = plugin_status(row, &k.rows.name).unwrap_or_else(|| {
        if row.on {
            Status::new("on", Tone::Good, w::ON)
        } else {
            Status::new("off", Tone::Quiet, w::OFF)
        }
    });
    let mut facts = vec![Fact { status: Some(status), ..Fact::new("status", w::STATUS) }];
    facts
        .push(Fact { href: source_page(row.source.as_deref()), ..Fact::new("marketplace", w::MARKETPLACE).value(row.marketplace.clone()) });
    if let Some(v) = &row.version {
        facts.push(Fact::new("version", w::VERSION).value(v.clone()));
    }
    facts.push(Fact::new("scope", w::SCOPE).value(w::scope(row.scope)));
    if let Some(p) = &row.path {
        facts.push(Fact { copy: true, ..Fact::new("path", w::PATH).value(p.clone()) });
    }
    if let Some(p) = &row.set_in {
        facts.push(Fact { copy: true, ..Fact::new("set-in", w::SET_IN).value(p.clone()) });
    }
    let b = &row.brings;
    let brought: Vec<UnderRow> = [&b.skills, &b.commands, &b.subagents, &b.hooks, &b.servers, &b.lsp, &b.apps]
        .into_iter()
        .zip(w::BRINGS)
        .filter(|(names, _)| !names.is_empty())
        .map(|(names, title)| UnderRow {
            key: title.to_owned(),
            title: title.to_owned(),
            subtext: Some(names.join(", ")),
            count: Some(names.len().to_string()),
        })
        .collect();
    Detail {
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::Puzzle),
        marks: vec![row.agent.clone()],
        about: plugin_missing(row, &k.rows.name).or_else(|| row.description.clone()),
        facts,
        acts: plugin_acts(row, k, cx),
        flow: None,
        refused: k.s(cx).refused.get(&k.key(&plugin_key(row))).cloned(),
        under: (!brought.is_empty()).then_some(Under {
            title: w::WHAT_IT_BRINGS.into(),
            reading: false,
            rows: Some(brought),
            read_at: None,
            refused: None,
            empty: None,
            refresh: None,
        }),
        choices: Vec::new(),
        doc: None,
    }
}
