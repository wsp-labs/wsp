// SPDX-License-Identifier: AGPL-3.0-only
//! The Skills tab: a skill's acts and writes, its row, its own page, and a skills.sh result's page.

use std::rc::Rc;

use gpui_kit::App;
use serde_json::json;
use wsp_host_client::ops::settings_agents::Preview;
use wsp_host_client::protocol::{
    AgentsReportProjectsItem as Project, AgentsReportSkillsItem as SkillRow, AgentsReportSkillsItemScope as SkillScope, SkillHit,
    SkillsAddRequest, SkillsGetRequest, SkillsPreviewRequest, SkillsRemoveRequest, SkillsToggleRequest,
};

use super::{
    by_name, real_path, row_key, row_target, skill_agents, skill_key, skill_off, skill_source, Choice, ChoiceOption, Detail, DocView, Fact,
    Load, RowView, K,
};
use crate::screens::settings::model::req;
use crate::screens::settings::pages::agents::model::{compact_count, entry, w, Status, Tone};
use crate::screens::settings::pages::agents::parts::{not_yet, Lead};
use crate::screens::settings::pages::agents::store::{Doc, Picks, Store};
use crate::ui::{Act, Glyph};

fn skill_acts(row: &SkillRow, k: &K, cx: &App) -> Vec<Act> {
    let source = skill_source(row);
    if source == "system" {
        return Vec::new();
    }
    let key = skill_key(row);
    let busy = k.s(cx).busy.contains(&k.key(&key));
    let off = skill_off(row);
    let turn = if source == "project" {
        not_yet("turn-off", w::TURN_OFF, Glyph::PowerOff).hover(w::lives_in_repo(&real_path(row)))
    } else {
        let (id, label, g) = if off { ("turn-on", w::TURN_ON, Glyph::Power) } else { ("turn-off", w::TURN_OFF, Glyph::PowerOff) };
        let act = Act::new(id, label, Some(g));
        if busy {
            act
        } else {
            let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
            act.run(move |_, cx| {
                let params = json!({ "target": row_target(&place, project_id!(row.project)), "name": row.name, "project": row.scope == SkillScope::Project, "on": off });
                store.update(cx, |s, cx| write(s, &place, &skill_key(&row), req::<SkillsToggleRequest>(params), cx))
            })
        }
    };
    let remove = {
        let act = Act::new("remove", w::REMOVE, Some(Glyph::Trash))
            .destructive()
            .confirm(w::remove_title(&row.name), w::remove_body(&k.rows.name));
        if busy {
            act
        } else {
            let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
            act.run(move |_, cx| {
                let params =
                    json!({ "target": row_target(&place, project_id!(row.project)), "name": row.name, "project": row.scope == SkillScope::Project });
                store.update(cx, |s, cx| write(s, &place, &skill_key(&row), req::<SkillsRemoveRequest>(params), cx))
            })
        }
    };
    k.rows.hold(vec![turn, remove])
}

/// One write: busy while it runs, its refusal kept until the next try, the report read again after.
pub(super) fn write<O>(s: &mut Store, place: &str, key: &str, op: O, cx: &mut gpui_kit::Context<Store>)
where
    O: wsp_host_client::Op + Send + 'static,
    O::Reply: Send + 'static,
{
    let k = (place.to_owned(), key.to_owned());
    s.busy.insert(k.clone());
    s.refused.remove(&k);
    let place = place.to_owned();
    s.call(op, cx, move |s, answer, cx| {
        s.busy.remove(&k);
        if let Err(said) = answer {
            s.refused.insert(k, said);
        }
        s.refresh(&place, cx);
    });
    cx.notify();
}

pub(super) fn skill_row_view(row: &SkillRow) -> RowView {
    RowView {
        key: row_key(&["skill", &row.scope.to_string()], project_id!(row.project), &row.name),
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::ScrollText),
        marks: skill_agents(row),
        subtext: Some(real_path(row)),
        status: None,
        quick: None,
    }
}

pub(super) fn skill_detail(row: &SkillRow, k: &K, cx: &App) -> Detail {
    let source = skill_source(row);
    let on = || Status::new("on", Tone::Good, w::ON);
    let off = || Status::new("off", Tone::Quiet, w::OFF);
    let status = match source {
        "system" => Fact {
            status: Some(Status::new("on", Tone::Good, w::ALWAYS_ON)),
            note: Some(w::KEPT_CURRENT.into()),
            ..Fact::new("status", w::STATUS)
        },
        "project" => Fact {
            status: Some(if skill_off(row) { off() } else { on() }),
            note: Some(w::IN_REPO.into()),
            ..Fact::new("status", w::STATUS)
        },
        _ => Fact { status: Some(if skill_off(row) { off() } else { on() }), ..Fact::new("status", w::STATUS) },
    };
    let mut facts = vec![status];
    if let Some(d) = &row.description {
        facts.push(Fact { prose: true, ..Fact::new("description", w::DESCRIPTION).value(d.clone()) });
    }
    let mut ordered = row.paths.clone();
    ordered.sort_by_key(|p| p.agent.is_some());
    for (at, p) in ordered.iter().enumerate() {
        let mut f = Fact { copy: true, ..Fact::new(&format!("path-{at}"), if at == 0 { w::PATH } else { "" }).value(p.path.clone()) };
        match &p.agent {
            None => f.note = Some(w::SHARED.into()),
            Some(a) => f.agent = Some(a.clone()),
        }
        facts.push(f);
    }
    let key = (k.rows.place.clone(), skill_key(row));
    let s = k.s(cx);
    let doc = s.previews.get(&key).cloned().unwrap_or(Doc::default());
    let load = {
        let (store, place, row) = (k.store.clone(), k.rows.place.clone(), row.clone());
        Rc::new(move |cx: &mut App| {
            store.update(cx, |s, cx| {
                let key = (place.clone(), skill_key(&row));
                if s.previews.contains_key(&key) {
                    return;
                }
                s.previews.insert(key.clone(), Doc::default());
                let params =
                    json!({ "target": row_target(&place, project_id!(row.project)), "name": row.name, "project": row.scope == SkillScope::Project });
                s.call(req::<SkillsPreviewRequest>(params), cx, move |s, answer: Result<Preview, String>, _| {
                    s.previews.insert(key, doc_of(answer));
                });
            })
        }) as Load
    };
    Detail {
        title: row.name.clone(),
        lead: Lead::Glyph(Glyph::ScrollText),
        marks: skill_agents(row),
        about: None,
        facts,
        acts: skill_acts(row, k, cx),
        flow: None,
        refused: s.refused.get(&key).cloned(),
        under: None,
        choices: Vec::new(),
        doc: Some(DocView { key: skill_key(row), doc, load: Some(load) }),
    }
}

fn doc_of(answer: Result<Preview, String>) -> Doc {
    match answer {
        Ok(Preview { preview }) => Doc { text: Some(preview.text), size: preview.size, error: None },
        Err(said) => Doc { error: Some(said), ..Doc::default() },
    }
}

/// The skill already in the folder a hit installs into.
pub(super) fn installed_as<'a>(hit: &SkillHit, skills: &'a [SkillRow], project: Option<&Project>) -> Option<&'a SkillRow> {
    skills.iter().find(|s| {
        s.name == hit.skill_id
            && match project {
                None => s.scope == SkillScope::User,
                Some(p) => s.scope == SkillScope::Project && s.project.as_ref().is_some_and(|x| x.id == p.id),
            }
    })
}

/// `whereNow`: where an add goes, the home or a project, and the line that says a picked project has left.
pub(super) struct WhereNow {
    pub(super) options: Vec<ChoiceOption>,
    pub(super) value: String,
    pub(super) project: Option<Project>,
    pub(super) lost: Option<String>,
}

pub(super) fn where_now(projects: &[Project], pick: Option<&(String, String)>, computer: &str) -> WhereNow {
    let mut projects = projects.to_vec();
    projects.sort_by(|a, b| by_name(&a.name, &b.name));
    let options = if projects.is_empty() && pick.is_none() {
        Vec::new()
    } else {
        let mut o = vec![ChoiceOption { value: "home".into(), label: w::GLOBAL.into(), agent: None, held: None }];
        o.extend(projects.iter().map(|p| ChoiceOption { value: p.id.clone(), label: p.name.clone(), agent: None, held: None }));
        o
    };
    match pick {
        None => WhereNow { options, value: "home".into(), project: None, lost: None },
        Some((id, name)) => match projects.iter().find(|p| p.id == *id) {
            Some(p) => WhereNow { options, value: id.clone(), project: Some(p.clone()), lost: None },
            None => WhereNow { options, value: id.clone(), project: None, lost: Some(w::project_left(name, computer)) },
        },
    }
}

pub(super) fn remote_detail(hit: &SkillHit, k: &K, cx: &App) -> Detail {
    let s = k.s(cx);
    let installed: Vec<String> = k.agents().iter().filter(|a| a.installed).map(|a| a.id.clone()).collect();
    let picks = s.picks.get(&hit.id).cloned().unwrap_or_default();
    let picked_agents = picks.agents.clone().unwrap_or_else(|| installed.clone());
    let place = where_now(k.projects(), picks.project.as_ref(), &k.rows.name);
    let there = if place.lost.is_none() { installed_as(hit, k.skills(), place.project.as_ref()) } else { None };
    let busy = s.busy.contains(&k.key(&hit.id));
    let project_pick = picks.project.is_some();
    let offered: Vec<ChoiceOption> = k
        .agents()
        .iter()
        .filter(|a| a.installed)
        .map(|a| {
            let shared = entry(&a.id).is_some_and(|e| !if project_pick { e.own_skills.1 } else { e.own_skills.0 });
            ChoiceOption {
                value: a.id.clone(),
                label: a.name.clone(),
                agent: Some(a.id.clone()),
                held: shared.then(|| w::READS_SHARED.into()),
            }
        })
        .collect();
    let set_picks = {
        let (store, id) = (k.store.clone(), hit.id.clone());
        move |next: Picks, cx: &mut App| {
            let id = id.clone();
            store.update(cx, |s, cx| {
                s.picks.insert(id, next);
                cx.notify();
            })
        }
    };
    let agents_choice = {
        let (set, picks) = (set_picks.clone(), picks.clone());
        Choice {
            id: "agents",
            label: w::AGENTS,
            many: true,
            options: offered.clone(),
            value: offered.iter().filter(|o| o.held.is_some() || picked_agents.contains(&o.value)).map(|o| o.value.clone()).collect(),
            lost: None,
            set: Rc::new(move |agents, cx| set(Picks { agents: Some(agents), ..picks.clone() }, cx)),
        }
    };
    let mut choices = vec![agents_choice];
    if !place.options.is_empty() {
        let (set, picks, projects) = (set_picks.clone(), picks.clone(), k.projects().to_vec());
        choices.push(Choice {
            id: "where",
            label: w::WHERE,
            many: false,
            options: place.options.clone(),
            value: vec![place.value.clone()],
            lost: place.lost.clone(),
            set: Rc::new(move |v, cx| {
                let project = v.first().and_then(|v| projects.iter().find(|p| p.id == *v)).map(|p| (p.id.clone(), p.name.clone()));
                set(Picks { project, ..picks.clone() }, cx)
            }),
        });
    }
    let mut install =
        Act::new("install", if busy { w::INSTALLING.to_owned() } else { w::install_name(&hit.skill_id) }, Some(Glyph::Download));
    if busy {
        install = install.busy();
    } else if let Some(lost) = &place.lost {
        install = install.hover(lost.clone());
    } else if there.is_some() {
        install = install.hover(w::already_on(&k.rows.name));
    } else {
        let agents: Vec<String> =
            picked_agents.iter().filter(|a| offered.iter().any(|o| o.value == **a && o.held.is_none())).cloned().collect();
        let (store, place_id, id, project) = (k.store.clone(), k.rows.place.clone(), hit.id.clone(), place.project.clone());
        install = install.run(move |_, cx| {
            let params =
                json!({ "target": row_target(&place_id, project.as_ref().map(|p| p.id.as_str())), "skill": id, "agents": agents, "project": project.is_some() });
            store.update(cx, |s, cx| write(s, &place_id, &id, req::<SkillsAddRequest>(params), cx))
        });
    }
    let doc = s.remote.get(&hit.id).cloned().unwrap_or(Doc::default());
    let load = {
        let (store, id) = (k.store.clone(), hit.id.clone());
        Rc::new(move |cx: &mut App| {
            store.update(cx, |s, cx| {
                if s.remote.contains_key(&id) {
                    return;
                }
                s.remote.insert(id.clone(), Doc::default());
                let id = id.clone();
                s.call(req::<SkillsGetRequest>(json!({ "skill": id })), cx, move |s, answer: Result<Preview, String>, _| {
                    s.remote.insert(id, doc_of(answer));
                });
            })
        }) as Load
    };
    let status = match there {
        None => Status::new("not-installed", Tone::Quiet, w::NOT_INSTALLED),
        Some(_) => Status::new("installed", Tone::Good, w::INSTALLED),
    };
    Detail {
        title: hit.skill_id.clone(),
        lead: Lead::Glyph(Glyph::ScrollText),
        marks: Vec::new(),
        about: None,
        facts: vec![
            Fact { status: Some(status), ..Fact::new("status", w::STATUS) },
            Fact::new("source", w::SOURCE).value(hit.source.clone()),
            Fact::new("installs", w::INSTALLS).value(compact_count(hit.installs)),
        ],
        acts: k.rows.hold(vec![install]),
        flow: None,
        refused: s.refused.get(&k.key(&hit.id)).cloned(),
        under: None,
        choices: if there.is_none() || !place.options.is_empty() { choices } else { Vec::new() },
        doc: Some(DocView { key: hit.id.clone(), doc, load: Some(load) }),
    }
}
