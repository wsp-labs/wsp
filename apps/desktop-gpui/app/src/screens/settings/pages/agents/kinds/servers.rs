// SPDX-License-Identifier: AGPL-3.0-only
//! The Tool servers tab: a server's acts, its tools listed and its writes, its row and its own page.

use std::rc::Rc;

use gpui_kit::{App, Entity, Window};
use serde_json::json;
use wsp_host_client::ops::settings_agents::Tools;
use wsp_host_client::protocol::{
    AgentsReport as Report, AgentsReportServersItem as McpRow, ServersRemoveRequest, ServersToggleRequest, ServersToolsRequest,
};

use super::{
    asked_of, checks_on_show, held_command, row_target, server_sign_in, server_status, standings, tools_key, worst_of, Detail, Fact,
    RowView, ServerEntry, Standing, Under, UnderRow, K,
};
use crate::screens::settings::model::req;
use crate::screens::settings::pages::agents::model::{entry, tools_listed, w};
use crate::screens::settings::pages::agents::parts::{not_yet, Lead};
use crate::screens::settings::pages::agents::store::{server_row, Flow, Key, Store, ToolsState};
use crate::screens::settings::pages::agents::Rows;
use crate::ui::{catalog, Act, Glyph, Run};

struct ServerActs {
    all: Vec<Standing>,
    worst: Standing,
    asked: Option<Standing>,
    acts: Vec<Act>,
    sign_ins: Vec<(String, Act)>,
    flow: Option<(String, Flow)>,
}

fn server_acts(e: &ServerEntry, k: &K, cx: &App) -> ServerActs {
    let s = k.s(cx);
    let all = standings(e, &s.tools, k.rows);
    let worst = worst_of(&all);
    let asked = asked_of(&all);
    if e.rows.iter().all(|r| r.launch == Some(true)) {
        return ServerActs { all, worst, asked, acts: Vec::new(), sign_ins: Vec::new(), flow: None };
    }
    let place = k.rows.place.clone();
    let list = {
        let (store, place) = (k.store.clone(), place.clone());
        move |row: McpRow, refresh: bool, cx: &mut App| store.update(cx, |s, cx| list_tools(s, &place, &row, refresh, cx))
    };
    let check = {
        let (rows, list) = (e.rows.clone(), list.clone());
        Act::new("check", w::CHECK, Some(Glyph::Activity)).hover(w::STARTS_ONCE).run(move |_, cx| {
            for r in &rows {
                list(r.clone(), false, cx);
            }
        })
    };
    let primary = asked.as_ref().map_or_else(|| worst.row.clone(), |a| a.row.clone());
    let listing = asked.as_ref().is_some_and(|a| a.listing);
    let reconnect = if listing {
        Act::new("reconnect", w::LISTING, Some(Glyph::RefreshCw)).busy()
    } else {
        let target = if worst.state == "failed" { worst.row.clone() } else { primary };
        let list = list.clone();
        Act::new("reconnect", w::RECONNECT, Some(Glyph::RefreshCw)).run(move |_, cx| list(target.clone(), true, cx))
    };
    let busy = s.busy.contains(&k.key(&e.key));
    let unswitched = e.rows.iter().find(|r| !entry(&r.agent).and_then(|a| a.mcp.as_ref()).is_some_and(|m| m.switch));
    let turn = |on: bool| {
        let (id, label, glyph) = if on { ("turn-on", w::TURN_ON, Glyph::Power) } else { ("turn-off", w::TURN_OFF, Glyph::PowerOff) };
        if let Some(u) = unswitched {
            return not_yet(id, label, glyph).hover(w::no_switch(&catalog::name(&u.agent)));
        }
        if busy {
            return Act::new(id, label, Some(glyph)).busy();
        }
        let (store, key, rows, place) =
            (k.store.clone(), e.key.clone(), e.rows.iter().filter(|r| r.enabled != on).cloned().collect::<Vec<_>>(), place.clone());
        Act::new(id, label, Some(glyph))
            .run(move |_, cx| store.update(cx, |s, cx| server_write(s, &place, &key, rows.clone(), Some(on), cx)))
    };
    let turn_off = turn(false);
    let turn_on = turn(true);
    let mut files: Vec<String> = Vec::new();
    for f in e.rows.iter().filter_map(|r| r.file.clone()) {
        if !files.contains(&f) {
            files.push(f);
        }
    }
    let remove = {
        let act = Act::new("remove", w::REMOVE, Some(Glyph::Trash))
            .destructive()
            .confirm(w::remove_title(&e.name), w::leaves_files(&files, &k.rows.name));
        if busy {
            act.busy()
        } else {
            let (store, key, rows, place) = (k.store.clone(), e.key.clone(), e.rows.clone(), place.clone());
            act.run(move |_, cx| store.update(cx, |s, cx| server_write(s, &place, &key, rows.clone(), None, cx)))
        }
    };
    let reach = k.reach();
    let mut sign_ins: Vec<(String, Act)> = Vec::new();
    let mut flow: Option<(String, Flow)> = None;
    for st in &all {
        let id = server_row(&st.row.agent, &st.row.scope.to_string(), project_id!(st.row.project), &st.row.name);
        let f = s.flows.get(&k.key(&id));
        let act = super::super::sign_in_act(k.store, k.rows, &id, server_sign_in(&st.row, &reach, k.rows), f);
        if flow.is_none() {
            flow = f.cloned().map(|f| (id.clone(), f));
        }
        if st.state == "needs-sign-in" || act.id == "cancel" || (st.state == "unknown" && !e.stdio) {
            sign_ins.push((st.row.agent.clone(), act));
        }
    }
    let sign_in = sign_ins.iter().find(|(a, _)| *a == worst.row.agent).or(sign_ins.first()).map(|(_, a)| a.clone());
    let with_sign_in: Vec<Act> = sign_in.into_iter().collect();
    let acts = match worst.state.as_str() {
        "needs-sign-in" => [with_sign_in, vec![turn_off, remove]].concat(),
        "failed" => vec![reconnect, turn_off, remove],
        "off" => vec![turn_on, remove],
        "checking" => vec![turn_off, remove],
        _ if asked.as_ref().is_some_and(|a| a.answer.is_some()) => [with_sign_in, vec![reconnect, turn_off, remove]].concat(),
        _ if e.rows.iter().any(|r| held_command(r, k.rows)) => vec![check, turn_off, remove],
        _ => [with_sign_in, vec![turn_off, remove]].concat(),
    };
    let held = k.rows.held.is_some();
    ServerActs { all, worst, asked, acts: k.rows.hold(acts), sign_ins, flow: if held { None } else { flow } }
}

/// One server's tools asked of the host: `servers.tools`, its answer kept by the row.
fn list_tools(s: &mut Store, place: &str, row: &McpRow, refresh: bool, cx: &mut gpui_kit::Context<Store>) {
    let key = (place.to_owned(), tools_key(row));
    let was = s.tools.get(&key).and_then(|t| t.answer.clone());
    s.tools.insert(key.clone(), ToolsState { listing: true, answer: was, error: None });
    let mut params = json!({ "target": row_target(place, project_id!(row.project)), "agent": row.agent, "name": row.name });
    if refresh {
        params["refresh"] = json!(true);
    }
    s.call(req::<ServersToolsRequest>(params), cx, move |s, answer: Result<Tools, String>, _| {
        let state = s.tools.entry(key).or_default();
        state.listing = false;
        match answer {
            Ok(Tools { answer }) => {
                state.answer = Some(answer);
                state.error = None;
            }
            Err(said) => state.error = Some(said),
        }
    });
    cx.notify();
}

/// One entry's rows turned on or off, or removed with `None`, one agent after another; busy while they run.
fn server_write(s: &mut Store, place: &str, key: &str, rows: Vec<McpRow>, on: Option<bool>, cx: &mut gpui_kit::Context<Store>) {
    let k = (place.to_owned(), key.to_owned());
    s.busy.insert(k.clone());
    s.refused.remove(&k);
    write_next(s, place.to_owned(), k, rows, on, cx);
    cx.notify();
}

fn write_next(s: &mut Store, place: String, k: Key, mut rows: Vec<McpRow>, on: Option<bool>, cx: &mut gpui_kit::Context<Store>) {
    if rows.is_empty() {
        s.busy.remove(&k);
        s.refresh(&place, cx);
        return;
    }
    let row = rows.remove(0);
    let mut params =
        json!({ "target": row_target(&place, project_id!(row.project)), "agent": row.agent, "name": row.name, "scope": row.scope });
    let then = move |s: &mut Store, answer: Result<(), String>, cx: &mut gpui_kit::Context<Store>| match answer {
        Ok(()) => write_next(s, place, k, rows, on, cx),
        Err(said) => {
            s.busy.remove(&k);
            s.refused.insert(k, said);
            s.refresh(&place, cx);
        }
    };
    match on {
        Some(on) => {
            params["on"] = json!(on);
            s.call(req::<ServersToggleRequest>(params), cx, move |s, a, cx| then(s, a.map(|_| ()), cx));
        }
        None => s.call(req::<ServersRemoveRequest>(params), cx, move |s, a, cx| then(s, a.map(|_| ()), cx)),
    }
}

/// `leadOf`: its company's mark, else its own icon where it is reached over an address.
pub(super) fn server_lead(e: &ServerEntry, k: &K, cx: &App) -> Lead {
    let mark = catalog::server_mark(&e.reach, e.stdio);
    let icon = icon_host(e).filter(|_| mark.is_none()).and_then(|host| k.s(cx).icon(host, k.icons));
    Lead::Server { mark, icon }
}

/// The address a server's own icon is asked by: one reached over an address with no company's mark.
pub(super) fn icon_host(e: &ServerEntry) -> Option<&str> {
    (!e.stdio && !e.reach.is_empty() && catalog::server_mark(&e.reach, false).is_none()).then_some(e.reach.as_str())
}

pub(super) fn server_row_view(e: &ServerEntry, k: &K, cx: &App) -> RowView {
    let a = server_acts(e, k, cx);
    let status = server_status(&a.worst);
    let find = |ids: &[&str]| a.acts.iter().find(|x| ids.contains(&x.id)).cloned();
    let step = match a.worst.state.as_str() {
        "needs-sign-in" => find(&["sign-in", "cancel"]),
        "failed" => find(&["reconnect"]),
        "off" => find(&["turn-on"]),
        _ => find(&["check"]),
    };
    let waiting = a.flow.as_ref().is_some_and(|(_, f)| f.waits());
    let mut status = status;
    if waiting {
        status.words = w::WAITING_ON_YOU.into();
    }
    RowView {
        key: e.key.clone(),
        title: e.name.clone(),
        lead: server_lead(e, k, cx),
        marks: e.rows.iter().map(|r| r.agent.clone()).collect(),
        subtext: Some(e.reach.clone()),
        status: Some(status),
        quick: if waiting { None } else { step },
    }
}

pub(super) fn server_detail(e: &ServerEntry, k: &K, cx: &App) -> Detail {
    let a = server_acts(e, k, cx);
    let listed = a.asked.as_ref().and_then(|s| s.answer.as_ref()).and_then(tools_listed).map(<[_]>::to_vec);
    let mut status = server_status(&a.worst);
    status.count = None;
    let names: Vec<String> = {
        let mut out: Vec<String> = Vec::new();
        for n in e.rows.iter().flat_map(|r| r.env_names.iter()) {
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        out
    };
    let disagree = a.all.iter().any(|s| s.state != a.all[0].state);
    let holder = a.asked.as_ref().and_then(|s| s.answer.as_ref()).and_then(|x| x.holder.clone());
    let refused = a.asked.as_ref().and_then(|s| s.error.clone().or_else(|| s.answer.as_ref().and_then(|x| x.refused.clone())));
    let wrote = k.s(cx).refused.get(&k.key(&e.key)).cloned();
    let tools_fact = match (&listed, &holder) {
        (Some(t), _) => Fact::new("tools", w::TOOLS).value(w::tools_count(t.len())),
        (None, Some(h)) => Fact::new("tools", w::TOOLS).muted(w::keeps_sign_in(&catalog::name(h))),
        _ => Fact::new("tools", w::TOOLS).muted(if a.worst.state == "needs-sign-in" { w::SIGN_IN_TO_SEE } else { w::NOT_LISTED_YET }),
    };
    let recipe = e.rows.iter().find_map(|r| r.in_recipe);
    let launched = e.rows.iter().all(|r| r.launch == Some(true));
    let mut facts = vec![Fact {
        status: Some(status.clone()),
        note: (status.state == "failed").then(|| status.hover.clone()).flatten(),
        ..Fact::new("status", w::STATUS)
    }];
    facts.push(Fact { copy: true, ..Fact::new("reach", if e.stdio { w::COMMAND } else { w::URL }).value(e.reach.clone()) });
    if !names.is_empty() {
        facts.push(Fact::new("names", if e.stdio { w::ENVIRONMENT } else { w::HEADERS }).muted(names.join(", ")));
    }
    for (at, s) in a.all.iter().enumerate() {
        let act =
            if disagree && e.rows.len() > 1 { a.sign_ins.iter().find(|(ag, _)| *ag == s.row.agent).map(|(_, x)| x.clone()) } else { None };
        let act = match (&k.rows.held, act) {
            (Some(why), Some(act)) => Some(act.held(why)),
            (_, act) => act,
        };
        let mut fact = Fact {
            agent: Some(s.row.agent.clone()),
            act,
            ..Fact::new(&format!("config-{}", s.row.agent), if at == 0 { w::CONFIG_LOCATION } else { "" })
        };
        match &s.row.file {
            None => fact = fact.muted(w::ON_EVERY_LAUNCH),
            Some(file) => {
                fact = fact.value(file.clone());
                fact.copy = true;
            }
        }
        if disagree {
            fact.note_status = Some(server_status(s));
        }
        facts.push(fact);
    }
    if !launched {
        facts.push(tools_fact);
    }
    if let Some(recipe) = recipe {
        let f = Fact::new("recipe", w::RECIPE);
        facts.push(if recipe { f.value(w::IN_RECIPE) } else { f.muted(w::NOT_IN_RECIPE) });
    }
    let tools_row = a.asked.as_ref().map_or_else(|| e.rows[0].clone(), |s| s.row.clone());
    let refresh: Option<Run> = (!launched && k.rows.held.is_none()).then(|| {
        let (store, place) = (k.store.clone(), k.rows.place.clone());
        Rc::new(move |_: &mut Window, cx: &mut App| store.update(cx, |s, cx| list_tools(s, &place, &tools_row, true, cx))) as Run
    });
    let off = (a.worst.state == "off" && a.worst.row.enabled).then(|| a.worst.row.off.clone()).flatten();
    let quiet = if launched {
        Some(w::TOOLS_EVERY_TURN.to_owned())
    } else if refused.is_some() || listed.is_some() {
        None
    } else if let Some(h) = &holder {
        Some(w::keeps_sign_in(&catalog::name(h)))
    } else if a.worst.state == "needs-sign-in" {
        Some(w::SIGN_IN_TO_SEE.to_owned())
    } else {
        off
    };
    Detail {
        title: e.name.clone(),
        lead: server_lead(e, k, cx),
        marks: e.rows.iter().map(|r| r.agent.clone()).collect(),
        about: None,
        facts,
        acts: a.acts,
        flow: a.flow,
        refused: wrote,
        under: Some(Under {
            title: w::tools_of(&e.name),
            reading: a.asked.as_ref().is_some_and(|s| s.listing) || a.worst.state == "checking",
            rows: listed.map(|t| {
                t.into_iter().map(|t| UnderRow { key: t.name.clone(), title: t.name, subtext: t.description, count: None }).collect()
            }),
            read_at: a.asked.as_ref().and_then(|s| s.answer.as_ref()).map(|x| x.read_at.clone()),
            refused,
            empty: quiet,
            refresh,
        }),
        choices: Vec::new(),
        doc: None,
    }
}

/// `shown`: each time a report stands, the servers that are checked the moment the tab shows them are asked.
pub(super) fn servers_shown(store: &Entity<Store>, rows: &Rows, report: &Report, cx: &mut App) {
    let wanted: Vec<McpRow> = report
        .servers
        .iter()
        .filter(|r| checks_on_show(r, rows))
        .filter(|r| store.read(cx).tools.get(&(rows.place.clone(), tools_key(r))).is_none_or(|t| !t.listing))
        .cloned()
        .collect();
    let read_at = report.read_at.clone();
    store.update(cx, |s, cx| {
        let was = s.shown.insert(rows.place.clone(), read_at.clone());
        if was.as_deref() == Some(read_at.as_str()) {
            return;
        }
        for row in wanted {
            list_tools(s, &rows.place, &row, false, cx);
        }
    });
}
