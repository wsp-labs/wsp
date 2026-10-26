// SPDX-License-Identifier: AGPL-3.0-only
//! The tab's adds: a skill searched on skills.sh, and a tool server in the page's rows.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, App, Entity, SharedString, Window};
use serde_json::{json, Value};
use wsp_host_client::ops::settings_agents::SkillHits;
use wsp_host_client::protocol::{
    AgentsReportProjectsItem as Project, AgentsReportServersItem as McpRow, AgentsReportServersItemScope as Scope, ServersAddRequest,
    SkillHit, SkillsSearchRequest,
};

use super::drawing::{search_field, Submit};
use super::skills::{installed_as, where_now};
use super::{row_target, K};
use crate::screens::settings::model::req;
use crate::screens::settings::page::{Ctx, Level};
use crate::screens::settings::pages::agents::keyed;
use crate::screens::settings::pages::agents::model::{command_words, compact_count, entry, w};
use crate::screens::settings::pages::agents::parts::{field, refusal};
use crate::screens::settings::pages::agents::store::Store;
use crate::ui::{catalog, size, tip, ActivePalette, Button, Card, CardItem, Glyph, Line, Row, RowSkeleton, Segmented, Select, WordClass};

/// Add a skill: skills.sh searched by the host, each result by name and repo with its installs, or `installed`.
pub(super) fn add_skill(ctx: &Ctx, k: &K, query: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    let store = k.store.clone();
    let input = keyed::typed(
        "add-search",
        w::SEARCH_SKILLS_SH,
        query,
        false,
        {
            let store = store.clone();
            move |input, _, cx| {
                let q = keyed::words(input, cx).trim().to_owned();
                store.update(cx, |s, cx| ask_skills(s, q, cx));
            }
        },
        window,
        cx,
    );
    let typed = keyed::words(&input, cx).trim().to_owned();
    let asked = store.read(cx).add_asked.clone();
    if typed != asked && store.read(cx).add_pending.as_deref() != Some(typed.as_str()) {
        let pending = typed.clone();
        store.update(cx, |s, cx| {
            s.add_pending = Some(pending.clone());
            cx.spawn(async move |me, cx| {
                cx.background_executor().timer(std::time::Duration::from_millis(250)).await;
                _ = me.update(cx, |s, cx| {
                    if s.add_pending.as_deref() == Some(pending.as_str()) {
                        s.add_pending = None;
                        ask_skills(s, pending, cx);
                    }
                });
            })
            .detach();
        });
    }
    let link =
        Button::ghost("add-link", w::SKILLS_SH).xs().after(Glyph::ExternalLink).on_click(|_, _, cx| cx.open_url("https://skills.sh"));
    let field = div().debug_selector(|| "add-search".into()).flex_1().min_w_0().max_w(px(320.)).child(search_field(&input, cx));
    let toolbar = h_flex().justify_between().gap_3().child(field).child(link);
    let search = store.read(cx).search(&asked).cloned();
    let (rows, empty, reading): (Vec<(SkillHit, bool)>, Option<String>, bool) = if asked.is_empty() {
        (Vec::new(), Some(w::TYPE_TO_SEARCH.into()), false)
    } else {
        let hits: Vec<(SkillHit, bool)> = search
            .as_ref()
            .and_then(|s| s.hits.clone())
            .unwrap_or_default()
            .into_iter()
            .map(|h| (h.clone(), installed_as(&h, k.skills(), None).is_some()))
            .collect();
        match &search {
            Some(s) if s.error.is_some() => (Vec::new(), s.error.clone(), false),
            _ => {
                let empty = (search.as_ref().is_some_and(|s| s.hits.is_some()) && hits.is_empty()).then(|| w::no_hits(&asked));
                (hits, empty, search.as_ref().is_none_or(|s| s.reading))
            }
        }
    };
    let mut out = v_flex().gap_3().child(toolbar);
    if (reading || typed != asked) && rows.is_empty() {
        out = out.child(RowSkeleton::new("add-reading"));
    }
    let mut card = Card::new("kind-found");
    for (hit, there) in &rows {
        let level = Level {
            key: format!("found:{}\n{}", hit.id, asked).into(),
            name: hit.skill_id.clone().into(),
            up: Some(Box::new(Level { key: add_level(&asked).into(), name: w::ADD_SKILL.into(), up: None })),
        };
        let acts = ctx.acts.clone();
        let row = Row::new(SharedString::from(hit.id.clone()), hit.skill_id.clone())
            .note(hit.source.clone())
            .mono()
            .clip()
            .fact(if *there { w::INSTALLED.to_owned() } else { compact_count(hit.installs) })
            .on_open(move |_, _, cx| acts.open_level(Some(level.clone()), cx));
        let found = format!("found-{}", hit.id);
        card = card.item(CardItem::placed(move |corners| {
            div().debug_selector(move || found).child(CardItem::from(row).place(corners)).into_any_element()
        }));
    }
    if rows.is_empty() && !reading {
        if let Some(empty) = empty {
            card = card.item(Line::new("add-none", empty).empty());
        }
    }
    out.child(card).into_any_element()
}

/// The add's level, holding the query it was left with, as a found skill's crumb goes back to it (`AgentsLevel.query`).
fn add_level(query: &str) -> String {
    if query.is_empty() {
        "add".to_owned()
    } else {
        format!("add\n{query}")
    }
}

/// The query an add's level holds, or none where the level is not an add.
pub(super) fn add_query(level: &str) -> Option<&str> {
    match level.strip_prefix("add") {
        Some("") => Some(""),
        Some(rest) => rest.strip_prefix('\n'),
        None => None,
    }
}

/// `skills.search` once per query; a refused one is asked again when typed again.
fn ask_skills(s: &mut Store, q: String, cx: &mut gpui_kit::Context<Store>) {
    s.add_asked = q.clone();
    if q.is_empty() || s.search(&q).is_some_and(|x| x.error.is_none()) {
        cx.notify();
        return;
    }
    s.search_asked(q.clone());
    s.call(req::<SkillsSearchRequest>(json!({ "q": q })), cx, move |s, answer: Result<SkillHits, String>, _| {
        s.search_answered(&q, answer.map(|hits| hits.skills));
    });
    cx.notify();
}

/// The file an add for this agent lands in: the one the report already read servers from, else the first it reads.
pub(super) fn file_of(agent: &str, project: Option<&Project>, servers: &[McpRow]) -> Option<String> {
    let mcp = entry(agent)?.mcp.as_ref()?;
    match project {
        Some(p) => {
            let files: Vec<String> = mcp.project_files.iter().map(|f| format!("{}/{f}", p.path)).collect();
            let own = servers.iter().find(|r| {
                r.agent == agent
                    && r.scope == Scope::Project
                    && r.project.as_ref().is_some_and(|x| x.id == p.id)
                    && r.file.as_ref().is_some_and(|f| files.contains(f))
            });
            own.and_then(|r| r.file.clone()).or_else(|| files.first().cloned())
        }
        None => servers
            .iter()
            .find(|r| r.agent == agent && r.scope == Scope::User && r.file.is_some())
            .and_then(|r| r.file.clone())
            .or_else(|| mcp.files.first().map(|f| f.to_string())),
    }
}

/// Add a tool server, in the page's rows: the agent whose config takes it, its name, a command or an address, where
/// it goes, then its variables or headers, each value held here alone until the host takes it once.
pub(super) fn add_server(k: &K, window: &mut Window, cx: &mut App) -> AnyElement {
    let agents: Vec<String> = super::super::catalog::AGENTS
        .iter()
        .filter(|a| a.mcp.is_some() && k.agents().iter().any(|r| r.id == a.id && r.installed))
        .map(|a| a.id.to_owned())
        .collect();
    if agents.is_empty() {
        return Card::new("add-server").item(Line::new("add-server-none", w::no_server_agents(&k.rows.name)).empty()).into_any_element();
    }
    let form = window.use_keyed_state("add-server-form", cx, |_, _| ServerForm::default());
    let f = form.read(cx).clone();
    let agent = f.agent.clone().filter(|a| agents.contains(a)).unwrap_or_else(|| agents[0].clone());
    let command_road = f.road != "address";
    let place = where_now(k.projects(), f.picked.as_ref(), &k.rows.name);
    let set = |change: Box<dyn Fn(&mut ServerForm)>| {
        let form = form.clone();
        move |cx: &mut App| {
            form.update(cx, |f, cx| {
                change(f);
                cx.notify();
            })
        }
    };
    let agent_items = agents
        .iter()
        .map(|a| {
            let id = a.clone();
            let mark: keyed::Mark = Rc::new(move |cx: &App| catalog::mark(&id, &catalog::name(&id), px(13.), cx.palette().foreground, cx));
            (a.clone(), SharedString::from(catalog::name(a)), Some(mark))
        })
        .collect();
    let agent_select = keyed::pick("add-server-agent", agent_items, Some(agent.clone()), window, cx);
    let agent_select = {
        let form = form.clone();
        Select::new(&agent_select).on_change(move |a: &String, _, cx| {
            let a = Some(a.clone());
            form.update(cx, |f, cx| {
                f.agent = a;
                cx.notify();
            })
        })
    };

    // Enter in any field submits as the button does; the submit is made after the fields it reads.
    let enter: Rc<std::cell::RefCell<Option<Submit>>> = Rc::default();
    let on_enter = || {
        let enter = enter.clone();
        move |_: &Entity<gpui_kit::component::input::InputState>, _: &mut Window, cx: &mut App| {
            let go = enter.borrow().clone();
            if let Some(go) = go {
                go(cx);
            }
        }
    };
    let name = keyed::typed("add-server-name", "notion", "", false, on_enter(), window, cx);
    let reach = if command_road {
        keyed::typed("add-server-command", "npx -y @notionhq/notion-mcp-server", "", false, on_enter(), window, cx)
    } else {
        keyed::typed("add-server-url", "https://mcp.notion.com/mcp", "", false, on_enter(), window, cx)
    };
    let road = {
        let set = set(Box::new(|_| {}));
        let form = form.clone();
        Segmented::new("add-server-road", if command_road { 0 } else { 1 }).segment(w::BY_COMMAND).segment(w::BY_ADDRESS).on_pick(
            move |ix, _, cx| {
                let to = ["command", "address"][ix].to_string();
                form.update(cx, |f, _| {
                    if f.road != to {
                        f.pairs.clear();
                    }
                    f.road = to;
                });
                set(cx);
            },
        )
    };
    let mut card = Card::new("add-server")
        .item(Row::new("add-server-agent", w::AGENT).control(agent_select))
        .item(Row::new("add-server-name", w::SERVER_NAME).control(field(&name, Some(px(176.)), cx)))
        .item(Row::new("add-server-road", w::REACHED_BY).control(road))
        .item(Row::new("add-server-reach", if command_road { w::COMMAND } else { w::URL }).control(field(&reach, Some(px(320.)), cx)));
    if !place.options.is_empty() {
        let items = place.options.iter().map(|o| (o.value.clone(), SharedString::from(o.label.clone()), None)).collect();
        let select = keyed::pick("add-server-where", items, Some(place.value.clone()), window, cx);
        let (form2, projects) = (form.clone(), k.projects().to_vec());
        let select = Select::new(&select).on_change(move |v: &String, _, cx| {
            let picked = projects.iter().find(|p| p.id == *v).map(|p| (p.id.clone(), p.name.clone()));
            form2.update(cx, |f, cx| {
                f.picked = picked;
                cx.notify();
            })
        });
        card = card.item(Row::new("add-server-where", w::WHERE).note(place.lost.clone().unwrap_or_default()).control(select));
    }
    let pair_word = if command_road { w::VARIABLES } else { w::HEADERS };
    let mut pairs_card = Card::new("add-server-pairs").head(pair_word);
    let mut typed_pairs: Vec<(String, String)> = Vec::new();
    for id in &f.pairs {
        let pname = keyed::typed(
            format!("add-server-pair-{id}-name"),
            if command_road { "API_KEY" } else { "Authorization" },
            "",
            false,
            on_enter(),
            window,
            cx,
        );
        let pvalue = keyed::typed(format!("add-server-pair-{id}-value"), w::VALUE, "", true, on_enter(), window, cx);
        let (n, v) = (keyed::words(&pname, cx), keyed::words(&pvalue, cx));
        typed_pairs.push((n.clone(), v));
        let (form, id) = (form.clone(), *id);
        let row = h_flex().gap_2().child(field(&pname, None, cx)).child(field(&pvalue, None, cx)).child(
            Button::icon_xs(SharedString::from(format!("add-server-pair-{id}-drop")), Glyph::Close)
                .tooltip(w::remove_pair(n.trim()))
                .on_click(move |_, _, cx| {
                    form.update(cx, |f, cx| {
                        f.pairs.retain(|p| *p != id);
                        cx.notify();
                    })
                }),
        );
        pairs_card = pairs_card.item(CardItem::placed(move |corners| {
            corners.apply(v_flex()).px(corners.inset).py_3().min_h(size::ROW_FLOOR).justify_center().child(row).into_any_element()
        }));
    }
    let add_pair = {
        let form = form.clone();
        Button::add("add-server-pair-add", if command_road { w::ADD_VARIABLE } else { w::ADD_HEADER }).xs().on_click(move |_, _, cx| {
            form.update(cx, |f, cx| {
                f.pairs.push(f.next);
                f.next += 1;
                cx.notify();
            })
        })
    };
    pairs_card = pairs_card.under(add_pair);
    let named: Vec<(String, String)> = typed_pairs.into_iter().filter(|(n, v)| !n.trim().is_empty() || !v.is_empty()).collect();
    let name_typed = keyed::words(&name, cx).trim().to_owned();
    let reach_typed = keyed::words(&reach, cx).trim().to_owned();
    let ready = place.lost.is_none()
        && !name_typed.is_empty()
        && !reach_typed.is_empty()
        && named.iter().all(|(n, _)| !n.trim().is_empty())
        && !f.adding;
    let file = if place.lost.is_none() { file_of(&agent, place.project.as_ref(), k.servers()) } else { None };
    let submit = {
        let (form, store, place_id, project, agent) =
            (form.clone(), k.store.clone(), k.rows.place.clone(), place.project.clone(), agent.clone());
        move |cx: &mut App| {
            let words = if command_road { command_words(&reach_typed) } else { Some(Vec::new()) };
            let Some(words) = words else {
                form.update(cx, |f, cx| {
                    f.refused = Some(w::UNCLOSED_QUOTE.into());
                    cx.notify();
                });
                return;
            };
            let mut seen: Vec<String> = Vec::new();
            for (n, _) in &named {
                if seen.contains(&n.trim().to_owned()) {
                    form.update(cx, |f, cx| {
                        f.refused = Some(w::two_pairs_one_name(command_road).into());
                        cx.notify();
                    });
                    return;
                }
                seen.push(n.trim().to_owned());
            }
            let values: serde_json::Map<String, Value> = named.iter().map(|(n, v)| (n.trim().to_owned(), json!(v))).collect();
            let mut params =
                json!({ "target": row_target(&place_id, project.as_ref().map(|p| p.id.as_str())), "agent": agent, "name": name_typed });
            if project.is_some() {
                params["project"] = json!(true);
            }
            if command_road {
                params["command"] = json!(words.first().cloned().unwrap_or_default());
                params["args"] = json!(words.iter().skip(1).collect::<Vec<_>>());
                if !values.is_empty() {
                    params["env"] = Value::Object(values);
                }
            } else {
                params["url"] = json!(reach_typed);
                if !values.is_empty() {
                    params["headers"] = Value::Object(values);
                }
            }
            form.update(cx, |f, cx| {
                f.adding = true;
                f.refused = None;
                cx.notify();
            });
            let (form, place_id) = (form.clone(), place_id.clone());
            store.update(cx, |s, cx| {
                s.call(req::<ServersAddRequest>(params), cx, move |s, answer, cx| {
                    match answer {
                        Ok(_) => {
                            s.acts().open_level(None, cx);
                            form.update(cx, |f, _| *f = ServerForm::default());
                        }
                        Err(said) => form.update(cx, |f, cx| {
                            f.adding = false;
                            f.refused = Some(said);
                            cx.notify();
                        }),
                    }
                    s.refresh(&place_id, cx);
                })
            });
        }
    };
    let mut go = Button::primary("add-server-go", if f.adding { w::ADDING } else { w::ADD_SERVER_GO })
        .glyph(Glyph::Plus)
        .busy(f.adding)
        .held(!ready);
    if ready {
        let submit = Rc::new(submit);
        *enter.borrow_mut() = Some(submit.clone());
        go = go.on_click(move |_, _, cx| submit(cx));
    }
    let footer = v_flex()
        .gap_1()
        .child(
            h_flex()
                .gap_3()
                .child(
                    div()
                        .id("add-server-file")
                        .debug_selector(|| "add-server-file".into())
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .when_some(file.clone(), |el, file| el.tooltip(tip(file)))
                        .child(crate::ui::word(file.unwrap_or_default(), WordClass::Fact, cx)),
                )
                .child(go),
        )
        .child(refusal(f.refused.as_deref()));
    v_flex().gap(px(30.)).child(card).child(pairs_card).child(footer).into_any_element()
}

/// What the add server form holds besides its fields' words, kept while it is drawn.
#[derive(Clone, Default)]
struct ServerForm {
    agent: Option<String>,
    road: String,
    pairs: Vec<u32>,
    next: u32,
    picked: Option<(String, String)>,
    adding: bool,
    refused: Option<String>,
}
