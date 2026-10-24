// SPDX-License-Identifier: AGPL-3.0-only
//! The sign-ins that live on a computer just added, after `ComputerSignIns.tsx`: each agent there whose login every
//! workspace on it shares, where its sign-in stands, and Sign in, which runs the agent's own sign-in on that computer
//! through the Agents page's store, its page and code under the row as that store follows them. Every act is held
//! while the computer is not answering.

use gpui_kit::component::v_flex;
use gpui_kit::{prelude::*, App, Entity, SharedString, Window};
use wsp_host_client::protocol::PlaceView;

use super::words::{absent_of, add_words, place_name};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::screens::settings::pages::agents::model::{sign_in_word, signed_in, w, AgentRow};
use crate::screens::settings::pages::agents::store::{agent_row, Start, Store};
use crate::screens::settings::pages::agents::{flow, sign_in_start, Rows};
use crate::ui::act::{act_button, Act};
use crate::ui::{size, Glyph};

/// The agents whose one login serves every workspace on a computer (`sharedAgentsOn`), held to the catalog's by
/// `the_shared_agents_are_the_catalogs`.
const SHARED: [&str; 1] = ["codex"];

/// `AGENTS_LIST_WORDS.roadSentences`.
fn road(road: &str) -> &'static str {
    match road {
        "device" => "Signs in on a page with a device code.",
        "code" => "Signs in with a code you paste back.",
        "token" => "Signs in with a token you make.",
        "key" => "Signs in with a key.",
        "terminal" => "Signs in from its own terminal, where it asks you to pick.",
        _ => "Needs no sign-in.",
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

/// `signInAct` held by `holdAll`: Cancel with its X while a run goes, else Sign in with its door, either held with the
/// computer's away word while it is not answering, and Sign in held where the agent has no road there.
fn sign_in_act(store: &Entity<Store>, place: &str, key: &str, running: bool, start: Option<Start>, held: Option<&str>) -> Act {
    let (store, place, key) = (store.clone(), place.to_owned(), key.to_owned());
    let act = if running {
        Act::new("cancel", w::CANCEL, Some(Glyph::X)).run(move |_, cx| store.update(cx, |s, cx| s.cancel(&place, &key, cx)))
    } else {
        let act = Act::new("sign-in", w::SIGN_IN, Some(Glyph::LogIn));
        match start {
            Some(start) => act.run(move |_, cx| {
                let start = start.clone();
                store.update(cx, |s, cx| s.start(&place, &key, start, cx))
            }),
            None => act,
        }
    };
    match held {
        Some(why) => act.held(why),
        None => act,
    }
}

pub fn card(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<CardData> {
    let id = place.id.clone();
    let store = Store::of(&ctx.agents, &ctx.acts, cx);
    store.update(cx, |s, cx| s.read(&id, cx));
    let s = store.read(cx);
    let rows: Vec<AgentRow> = s
        .reads
        .get(&id)
        .and_then(|r| r.report.as_ref())?
        .agents
        .iter()
        .filter(|a| a.installed && SHARED.contains(&a.id.as_str()))
        .cloned()
        .collect();
    if rows.is_empty() {
        return None;
    }
    let name = place_name(place);
    let held = absent_of(place).map(|a| a.away);
    let lists = Rows::of(place, s);
    let names: Vec<String> = rows.iter().map(|r| r.name.clone()).collect();
    let mut card = CardData::new("sign-ins").head(add_words::sign_ins_on(&name)).lede(sign_ins_why(&names, &name));
    for row in rows {
        let key = agent_row(&row.id);
        let flow_now = store.read(cx).flows.get(&(id.clone(), key.clone())).cloned();
        let running = flow_now.as_ref().is_some_and(|f| f.runs());
        let word = if flow_now.as_ref().is_some_and(|f| f.waits()) { w::WAITING_ON_YOU } else { sign_in_word(&row) };
        let offered = running || !signed_in(&row);
        let act = sign_in_act(&store, &id, &key, running, sign_in_start(&row, &lists), held);
        let act = act_button(SharedString::from(format!("act-{}-{}", act.id, row.id)), &act, &ctx.acts);
        let note = road(&row.sign_in_road.to_string());
        let under = flow_now.map(|f| v_flex().px(size::CARD_INSET).pb_4().child(flow::view(&store, &id, &key, &f, true, window, cx)));
        let item_row = crate::ui::Row::new(SharedString::from(format!("sign-in-{}", row.id)), row.name.clone()).note(note).fact(word);
        let item_row = if offered { item_row.control(act) } else { item_row };
        let words = vec![SharedString::from(row.name.clone()), SharedString::from(note)];
        let el = v_flex().w_full().child(item_row).children(under);
        card = card.item(Item::any(SharedString::from(format!("sign-in-{}", row.id)), words, crate::ui::CardItem::any(el)));
    }
    Some(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui_kit::test]
    fn the_act_is_held_with_the_away_word_and_cancel_wears_its_cross(cx: &mut gpui_kit::TestAppContext) {
        let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let acts = crate::screens::settings::Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let store = cx.new(|_| Store::new(acts));
        let start = Some(Start::Copy { line: "x".into(), why: None });
        let held = sign_in_act(&store, "box1", "agents/codex", false, start.clone(), Some("no answer"));
        assert!(held.run.is_none() && held.hover.as_deref() == Some("no answer"), "held with why");
        let open = sign_in_act(&store, "box1", "agents/codex", false, start, None);
        assert!(open.run.is_some() && open.id == "sign-in" && matches!(open.glyph, Some(Glyph::LogIn)));
        let cancel = sign_in_act(&store, "box1", "agents/codex", true, None, None);
        assert!(cancel.run.is_some() && cancel.id == "cancel" && matches!(cancel.glyph, Some(Glyph::X)));
        assert!(sign_in_act(&store, "box1", "agents/codex", false, None, None).run.is_none(), "no road, no run");
    }

    #[test]
    fn the_shared_agents_are_the_catalogs() {
        let read = crate::screens::settings::words::tests::catalog_read();
        assert_eq!(serde_json::json!(SHARED), read["sharedAgents"]);
    }
}
