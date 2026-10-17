// SPDX-License-Identifier: AGPL-3.0-only
//! The sign-ins that live on a computer just added, after `ComputerSignIns.tsx`: each agent there whose login every
//! workspace on it shares, where its sign-in stands, and Sign in, which runs the agent's own sign-in on that computer
//! through the host, its page and code under the row. Every act is held while the computer is not answering.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, App, Entity, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{
    AgentRow, AgentsReadRequest, AgentsSignInRequest, AgentsSignInStopRequest, AgentsSignInsRequest, PlaceView,
};

use super::words::{absent_of, add_words, place_name};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{size, ActivePalette, Button, Glyph};

/// The agents whose one login serves every workspace on a computer (`sharedAgentsOn`).
const SHARED: [&str; 1] = ["codex"];

/// `AGENTS_LIST_WORDS`, the parts a sign-in row says.
mod list_words {
    pub const SIGN_IN: &str = "Sign in";
    pub const CANCEL: &str = "Cancel";
    pub const WAITING_ON_YOU: &str = "waiting on you";
    pub fn road(road: &str) -> &'static str {
        match road {
            "device" => "Signs in on a page with a device code.",
            "code" => "Signs in with a code you paste back.",
            "token" => "Signs in with a token you make.",
            "key" => "Signs in with a key.",
            "terminal" => "Signs in from its own terminal, where it asks you to pick.",
            _ => "Needs no sign-in.",
        }
    }
    /// `signInWord`.
    pub fn word(sign_in: &str) -> &'static str {
        match sign_in {
            "signed-in" => "signed in",
            "vault-key" => "signed in with a token",
            "none" => "needs sign-in",
            _ => "not checked",
        }
    }
}

/// `signInsWhy`: the agents' names as an English list.
fn sign_ins_why(agents: &[String], computer: &str) -> String {
    let list = match agents.len() {
        0 => String::new(),
        1 => agents[0].clone(),
        2 => format!("{} and {}", agents[0], agents[1]),
        n => format!("{}, and {}", agents[..n - 1].join(", "), agents[n - 1]),
    };
    format!(
        "{list} {} one login for every workspace on {computer}, so sign in once here.",
        if agents.len() == 1 { "keeps" } else { "keep" }
    )
}

/// One run of a sign-in: its id, and the page and code its last frame named.
#[derive(Clone, Default)]
struct Run {
    id: String,
    state: String,
    url: Option<String>,
    code: Option<String>,
}

/// The page's own read of the agents there, and the sign-in under way by agent.
#[derive(Default)]
struct Read {
    report: Option<Vec<AgentRow>>,
    runs: Vec<(String, Run)>,
}

fn read_report(ctx: &Ctx, place_id: &str, state: &Entity<Read>, cx: &mut App) {
    let (acts, state) = (ctx.acts.clone(), state.clone());
    ctx.acts.call(req_read(place_id), cx, move |answer, cx| {
        if let Ok(report) = answer {
            state.update(cx, |s, _| s.report = Some(report.report.agents));
            acts.notify(cx);
        }
    });
}

fn req_read(place_id: &str) -> AgentsReadRequest {
    crate::screens::settings::model::req(json!({ "target": { "placeId": place_id } }))
}

/// The run an agent's sign-in stands at, read from the host's runs after a start.
fn read_runs(acts: &crate::screens::settings::Acts, agent: String, state: &Entity<Read>, cx: &mut App) {
    let (acts2, state) = (acts.clone(), state.clone());
    acts.call(crate::screens::settings::model::req::<AgentsSignInsRequest>(json!({})), cx, move |answer, cx| {
        let Ok(runs) = answer else { return };
        if let Some(run) = runs.runs.iter().rev().find(|r| r.agent == agent && r.ended.is_none()) {
            let last = run.last.as_ref();
            let kept = Run {
                id: run.sign_in_id.clone(),
                state: last.map(|l| l.state.to_string()).unwrap_or_else(|| "running".into()),
                url: last.and_then(|l| l.url.clone()),
                code: last.and_then(|l| l.code.clone()),
            };
            state.update(cx, |s, _| {
                s.runs.retain(|(a, _)| *a != agent);
                s.runs.push((agent, kept));
            });
        }
        acts2.notify(cx);
    });
}

pub fn card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let id = place.id.clone();
    let state = window.use_keyed_state(SharedString::from(format!("computer-{}-sign-ins", place.id)), cx, |_, _| Read::default());
    if state.read(cx).report.is_none() {
        let started = window.use_keyed_state(SharedString::from(format!("computer-{}-sign-ins-asked", place.id)), cx, |_, _| false);
        if !*started.read(cx) && ctx.acts.host(cx).is_some() {
            started.update(cx, |s, _| *s = true);
            read_report(ctx, &id, &state, cx);
        }
    }
    let rows: Vec<AgentRow> =
        state.read(cx).report.clone()?.into_iter().filter(|a| a.installed && SHARED.contains(&a.id.as_str())).collect();
    if rows.is_empty() {
        return None;
    }
    let name = place_name(place);
    let held_why = absent_of(place).map(|a| a.away);
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    let mut card = CardData::new("sign-ins").head(add_words::sign_ins_on(&name)).lede(sign_ins_why(&names, &name));
    let p = cx.palette().clone();
    let mono = cx.theme().mono_font_family.clone();
    for row in rows {
        let run = state.read(cx).runs.iter().find(|(a, _)| *a == row.id).map(|(_, r)| r.clone());
        let running = run.as_ref().is_some_and(|r| r.state == "running" || r.state == "waiting");
        let waiting = run.as_ref().is_some_and(|r| r.state == "waiting");
        let sign_in = row.sign_in.to_string();
        let word = if waiting { list_words::WAITING_ON_YOU } else { list_words::word(&sign_in) };
        let offered = running || !matches!(sign_in.as_str(), "signed-in" | "vault-key");
        let (acts, agent, place_id, state2) = (ctx.acts.clone(), row.id.clone(), id.clone(), state.clone());
        let held = held_why.is_some();
        let act = if running {
            let sign_in_id = run.as_ref().map(|r| r.id.clone()).unwrap_or_default();
            Button::outline(SharedString::from(format!("act-cancel-{}", row.id)), list_words::CANCEL).xs().held(held).on_click(
                move |_, _, cx| {
                    let (acts2, state3, agent2) = (acts.clone(), state2.clone(), agent.clone());
                    acts.call(
                        crate::screens::settings::model::req::<AgentsSignInStopRequest>(json!({ "signInId": sign_in_id })),
                        cx,
                        move |_, cx| {
                            state3.update(cx, |s, _| s.runs.retain(|(a, _)| *a != agent2));
                            acts2.notify(cx);
                        },
                    );
                },
            )
        } else {
            Button::outline(SharedString::from(format!("act-sign-in-{}", row.id)), list_words::SIGN_IN)
                .glyph(Glyph::LogIn)
                .xs()
                .held(held)
                .on_click(move |_, _, cx| {
                    let (acts2, state3, agent2) = (acts.clone(), state2.clone(), agent.clone());
                    acts.call(
                        crate::screens::settings::model::req::<AgentsSignInRequest>(
                            json!({ "target": { "placeId": place_id }, "agent": agent }),
                        ),
                        cx,
                        move |answer, cx| match answer {
                            Ok(_) => read_runs(&acts2, agent2, &state3, cx),
                            Err(said) => acts2.failed(said, cx),
                        },
                    );
                })
        };
        let note = list_words::road(&row.sign_in_road.to_string());
        let flow = run.filter(|r| r.url.is_some() || r.code.is_some()).map(|r| {
            let url = r.url.clone();
            h_flex()
                .px(size::CARD_INSET)
                .pb_4()
                .gap_3()
                .children(r.code.map(|code| {
                    div().font_family(mono.clone()).text_size(px(20.)).line_height(px(28.)).text_color(p.foreground).child(code)
                }))
                .children(url.map(|url| {
                    Button::outline(SharedString::from(format!("open-{}", row.id)), "Open sign-in \u{2197}")
                        .xs()
                        .on_click(move |_, _, cx| cx.open_url(&url))
                }))
        });
        let item_row = crate::ui::Row::new(SharedString::from(format!("sign-in-{}", row.id)), row.name.clone()).note(note).fact(word);
        let item_row = if offered { item_row.control(act) } else { item_row };
        let words = vec![SharedString::from(row.name.clone()), SharedString::from(note)];
        let el = v_flex().w_full().child(item_row).children(flow);
        card = card.item(Item::any(SharedString::from(format!("sign-in-{}", row.id)), words, crate::ui::CardItem::any(el)));
    }
    Some(card)
}
