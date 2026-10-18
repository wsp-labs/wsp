// SPDX-License-Identifier: AGPL-3.0-only
//! Agents and each agent's page (`agents.tsx`, `agentPage.tsx`, `agentKinds.tsx`): the computer the page reads and
//! its tab, then on the agents tab which agent a new thread starts on and one row per agent there with its version,
//! its sign-in and the step it needs, each opening that agent's own page; the other tabs list the tool servers,
//! skills and plugins there, each item opening its own page in place of the list.

mod agent_page;
pub mod catalog;
pub mod flow;
pub(crate) mod keyed;
mod kinds;
pub mod model;
mod parts;
pub mod store;

pub use parts::agent_marks;

use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{AgentRowRoad, AgentRowSignInRoad, BrokerStatus, PlaceView, Preferences};

use crate::screens::settings::model::{At, GroupId};
use crate::screens::settings::page::{CardData, Ctx, Item};
use crate::ui::{act_button, framed, Act, ActivePalette, Card, CardItem, Glyph, Row, RowSkeleton, Segmented};
use model::{
    capitalised, entry, list_words, newer_than, read_when, refused_lines, row_status, sign_in_word, signed_in, version_line, w, AgentRow,
    Tone,
};
use parts::{not_yet, on_head, status_word, step_mark};
use store::{agent_row, Flow, Start};
pub use store::{Store, Tab};

/// The computer the Agents page and every agent's page read where none is picked (`HERE_PLACE_ID`).
const HERE: &str = "here";

/// The pages under Agents in the sidebar: one per agent a thread can run on, in the host's order (`agentSubPages`).
pub fn sub_pages(ctx: &Ctx) -> Vec<(At, SharedString)> {
    ctx.data.harnesses.iter().map(|h| (At::Agent(h.harness.to_string()), SharedString::from(h.label.clone()))).collect()
}

/// An agent's mark as its sidebar row under Agents draws it (`HarnessMark` at `size-4`).
pub fn glyph(id: &str, ctx: &Ctx, cx: &App) -> Option<AnyElement> {
    let label =
        ctx.data.harnesses.iter().find(|h| h.harness == id).map(|h| h.label.clone()).unwrap_or_else(|| crate::ui::catalog::name(id));
    Some(crate::ui::catalog::mark(id, &label, px(16.), cx.palette().foreground, cx))
}

/// `defaultAgentOf`: the person's default where a thread runs on it, else the first the host lists.
fn default_agent(ctx: &Ctx, prefs: Option<&Preferences>) -> Option<(String, String)> {
    let first = ctx.data.harnesses.first()?;
    let named = prefs.and_then(|p| p.default_agent.as_deref()).and_then(|id| ctx.data.harnesses.iter().find(|h| h.harness == id));
    let h = named.unwrap_or(first);
    Some((h.harness.to_string(), h.label.clone()))
}

fn kind_of(place: &PlaceView) -> String {
    serde_json::to_value(place.kind).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

/// The computers whose own agents a person manages: this one and every computer joined to it (`computersOf`).
fn computers<'a>(ctx: &'a Ctx) -> Vec<&'a PlaceView> {
    ctx.data.places.iter().filter(|p| kind_of(p) != "provider").collect()
}

fn place_name(place: &PlaceView) -> String {
    super::computers::place_name(place)
}

/// `usePickedPlace`: the one picked, else the one wsp runs on, else the first.
fn picked<'a>(ctx: &'a Ctx, store: &Store) -> Option<&'a PlaceView> {
    let all = computers(ctx);
    let want = store.place.as_deref().unwrap_or(HERE);
    all.iter().find(|p| p.id == want).or_else(|| all.iter().find(|p| p.id == HERE)).or_else(|| all.first()).copied()
}

/// What decides a row's acts where the list stands (`RowsContext`): the computer, whether it is this one, why every act
/// is held, and the broker's word.
pub struct Rows {
    pub place: String,
    pub here: bool,
    pub name: String,
    pub held: Option<String>,
    pub broker: Option<BrokerStatus>,
    /// What the recipe meant to put beside the agents and did not, which the report cannot show.
    pub misses: Vec<(String, Option<String>)>,
}

impl Rows {
    pub fn of(place: &PlaceView, store: &Store) -> Rows {
        let here = place.id == HERE;
        Rows {
            place: place.id.clone(),
            here,
            name: place_name(place),
            held: (!here && place.present == Some(false)).then(|| w::NO_ANSWER.to_owned()),
            broker: if here { store.broker.clone() } else { None },
            misses: recipe_misses(place),
        }
    }

    /// `holdAll`.
    fn hold(&self, acts: Vec<Act>) -> Vec<Act> {
        match &self.held {
            None => acts,
            Some(why) => acts.into_iter().map(|a| a.held(why)).collect(),
        }
    }
}

/// `recipeMissLines`: an agent, a skill file or a server the recipe's last run there failed or set aside.
fn recipe_misses(place: &PlaceView) -> Vec<(String, Option<String>)> {
    let rows = serde_json::to_value(&place.applied)
        .ok()
        .and_then(|a| a.get("rows").cloned())
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();
    rows.iter()
        .filter_map(|row| {
            let field = |name: &str| row.get(name).and_then(|v| v.as_str()).unwrap_or_default();
            let outcome = match field("outcome") {
                "failed" => "failed",
                "skipped" => "set aside",
                _ => return None,
            };
            let id = field("id");
            let agent = model::agent_of_row(id).is_some();
            if field("kind") != "file" && field("kind") != "server" && !agent {
                return None;
            }
            let said = match row.get("note").and_then(|v| v.as_str()) {
                Some(note) => format!("{outcome}: {note}"),
                None => outcome.to_owned(),
            };
            Some((field("label").to_owned(), Some(said)))
        })
        .collect()
}

/// `agentSignInStart`: how an agent's Sign in goes where the list stands.
pub fn sign_in_start(row: &AgentRow, rows: &Rows) -> Option<Start> {
    if model::broker_row(&row.id, rows.broker.as_ref()).is_some_and(|b| b.held.is_none()) {
        return Some(Start::Run { agent: row.id.clone(), server: None, finish: None, pastes: false, cloud: true });
    }
    match row.sign_in_road {
        AgentRowSignInRoad::Token => {
            let mint = crate::ui::catalog::sign_in(&row.id).and_then(|s| s.mint).map(str::to_owned);
            Some(Start::Vault { agent: row.id.clone(), mint, word: "token" })
        }
        AgentRowSignInRoad::Key => Some(Start::Vault { agent: row.id.clone(), mint: None, word: "key" }),
        // A joined computer's login runs in a terminal drawn in place on the web; with no terminal here, the line (D65).
        AgentRowSignInRoad::Terminal => Some(Start::Copy { line: format!("wsp agents signin {}", row.id), why: None }),
        AgentRowSignInRoad::None => None,
        road => {
            Some(Start::Run { agent: row.id.clone(), server: None, finish: None, pastes: road == AgentRowSignInRoad::Code, cloud: false })
        }
    }
}

/// `signInAct`: Sign in, or Cancel while its run goes.
fn sign_in_act(store: &Entity<Store>, rows: &Rows, row_id: &str, start: Option<Start>, flow: Option<&Flow>) -> Act {
    let (place, id) = (rows.place.clone(), row_id.to_owned());
    if flow.is_some_and(Flow::runs) {
        let store = store.clone();
        return Act::new("cancel", w::CANCEL, Some(Glyph::Close)).run(move |_, cx| store.update(cx, |s, cx| s.cancel(&place, &id, cx)));
    }
    let act = Act::new("sign-in", w::SIGN_IN, Some(Glyph::LogIn));
    match start {
        Some(start) => {
            let store = store.clone();
            act.run(move |_, cx| {
                let start = start.clone();
                store.update(cx, |s, cx| s.start(&place, &id, start, cx))
            })
        }
        None => act,
    }
}

/// The acts an agent's row offers, next step first, and the step it stands in its state's place (`actsOf`).
fn agent_acts(store: &Entity<Store>, row: &AgentRow, rows: &Rows, cx: &App) -> (Vec<Act>, Option<Act>) {
    if !row.installed {
        let acts = rows.hold(vec![not_yet("install", w::INSTALL, Glyph::Download)]);
        let step = acts.first().cloned();
        return (acts, step);
    }
    let s = store.read(cx);
    let row_id = agent_row(&row.id);
    let flow = s.flows.get(&(rows.place.clone(), row_id.clone()));
    let sign_in = sign_in_act(store, rows, &row_id, sign_in_start(row, rows), flow);
    let running = sign_in.id == "cancel";
    let hold = match row.road {
        AgentRowRoad::Own => Some(w::OWN_HOLD),
        AgentRowRoad::Shim => Some(w::SHIM_HOLD),
        _ => None,
    };
    let adding = s.adding.contains(&(rows.place.clone(), row.id.clone()));
    let mcp = entry(&row.id).is_some_and(|e| e.mcp.is_some());
    let add_tools = if !mcp {
        Act::new("add-tools", w::ADD_TOOLS, None).add().hover(w::add_tools_no_config(&row.name))
    } else if !rows.here {
        Act::new("add-tools", w::ADD_TOOLS, None).add().hover(w::ADD_TOOLS_HERE_ONLY)
    } else if adding {
        Act::new("add-tools", w::ADD_TOOLS, None).add().busy()
    } else {
        let (store, place, agent) = (store.clone(), rows.place.clone(), row.id.clone());
        Act::new("add-tools", w::ADD_TOOLS, None).add().run(move |_, cx| store.update(cx, |s, cx| s.add_tools(&place, &agent, cx)))
    };
    let mut update = not_yet("update", w::UPDATE, Glyph::CircleArrowUp);
    let mut uninstall = not_yet("uninstall", w::UNINSTALL, Glyph::Trash).destructive();
    if let Some(hold) = hold {
        update = update.hover(hold);
        uninstall = uninstall.hover(hold);
    }
    let tools: Vec<Act> = if row.wsp_tools { Vec::new() } else { vec![add_tools] };
    let updates: Vec<Act> = if newer_than(row).is_some() { vec![update] } else { Vec::new() };
    let open = Act::new("open-terminal", w::OPEN_IN_TERMINAL, Some(Glyph::SquareTerminal)).hover(w::FROM_PANEL);
    let cloud = model::broker_row(&row.id, rows.broker.as_ref());
    let ready = match &cloud {
        None => signed_in(row),
        Some(c) => c.held.is_some(),
    };
    let (step, acts): (Option<&str>, Vec<Act>) = if ready && !running {
        (Some("open-terminal"), [updates, tools, vec![open, uninstall]].concat())
    } else if row.sign_in_road == AgentRowSignInRoad::None && !running {
        (None, [updates, tools, vec![uninstall]].concat())
    } else {
        (Some(sign_in.id), [vec![sign_in], tools, updates, vec![uninstall]].concat())
    };
    let acts = rows.hold(acts);
    let step = step.and_then(|id| acts.iter().find(|a| a.id == id).cloned());
    (acts, step)
}

/// The sign-in a row draws under it while one stands, unless the computer is away.
fn row_flow(store: &Entity<Store>, rows: &Rows, row_id: &str, cx: &App) -> Option<Flow> {
    if rows.held.is_some() {
        return None;
    }
    store.read(cx).flows.get(&(rows.place.clone(), row_id.to_owned())).cloned()
}

/// A row and what stands under it on the row's text edge past its glyph frame (`UNDER_ROW`), as one item of its card.
fn with_under(row: Row, under: Option<AnyElement>) -> CardItem {
    match under {
        None => row.into(),
        Some(under) => CardItem::placed(move |corners| {
            corners.apply(gpui_kit::div()).child(row).child(div().pl(px(64.)).pr(px(20.)).pb_3().child(under)).into_any_element()
        }),
    }
}

/// `AgentLine`: one agent on the computer read, its version and the one step it needs, or its state.
fn agent_line(store: &Entity<Store>, row: &AgentRow, rows: &Rows, window: &mut Window, cx: &mut App) -> CardItem {
    let lead = framed(crate::ui::catalog::mark(&row.id, &row.name, px(16.), cx.palette().foreground, cx), cx);
    let id = SharedString::from(format!("agent-{}", row.id));
    let row_id = agent_row(&row.id);
    let (_, step) = agent_acts(store, row, rows, cx);
    let flow = row_flow(store, rows, &row_id, cx);
    let waiting = flow.as_ref().is_some_and(Flow::waits);
    let acts = store.read(cx).acts().clone();
    if !row.installed {
        let about = entry(&row.id).map(|e| e.description).unwrap_or_default();
        let mut line = Row::new(id, row.name.clone()).lead(lead).note(about).clip();
        if let Some(install) = step.filter(|s| s.run.is_some()) {
            line = line.control(act_button(format!("agent-{}-install", row.id), &install.word_only(), &acts));
        }
        return line.into();
    }
    let quick = if waiting { None } else { step };
    let step = quick.filter(|q| (q.id == "sign-in" || q.id == "cancel") && (q.run.is_some() || q.busy));
    let status = row_status(row, rows.broker.as_ref());
    let tone = if waiting || status.tone == Tone::Waiting {
        Tone::Waiting
    } else if status.tone == Tone::Quiet {
        Tone::Quiet
    } else {
        Tone::Good
    };
    let word = capitalised(if status.words.is_empty() { sign_in_word(row) } else { &status.words });
    let mut slot = h_flex().gap_3();
    if let Some(update) = row.update.clone() {
        let computer = rows.name.clone();
        let said = w::update_to(&update.to);
        let done = acts.clone();
        let copy = Act::new("agent-update", said, Some(Glyph::CircleArrowUp)).run(move |_, cx| {
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(update.command.clone()));
            done.done(w::update_copied(&update.command, &computer), cx);
        });
        slot = slot.child(step_mark(format!("agent-{}-update", row.id), &copy));
    }
    slot = slot.child(match step {
        Some(step) => act_button(format!("agent-{}-{}", row.id, step.id), &step.word_only(), &acts),
        None => status_word(format!("agent-{}-status", row.id), &word, tone, None, cx),
    });
    let at = At::Agent(row.id.clone());
    let go = acts.clone();
    let mut line = Row::new(id, row.name.clone()).lead(lead).control(slot).on_open(move |_, window, cx| go.go(at.clone(), window, cx));
    if let Some(version) = version_line(row, rows.broker.as_ref()) {
        line = line.note(version);
    }
    let under = flow.map(|flow| flow::view(store, &rows.place, &row_id, &flow, true, window, cx));
    with_under(line, under)
}

/// The New threads card: the agent one starts on where its project names none.
fn new_threads(ctx: &Ctx, prefs: Option<&Preferences>, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let (picked, _) = default_agent(ctx, prefs)?;
    let items = ctx
        .data
        .harnesses
        .iter()
        .map(|h| {
            let (id, label) = (h.harness.to_string(), h.label.clone());
            let (mark_id, mark_label) = (id.clone(), label.clone());
            let mark: keyed::Mark =
                std::rc::Rc::new(move |cx: &App| crate::ui::catalog::mark(&mark_id, &mark_label, px(13.), cx.palette().foreground, cx));
            (id, SharedString::from(label), Some(mark))
        })
        .collect();
    let select = keyed::pick("agents-default-agent", items, Some(picked), window, cx);
    let acts = ctx.acts.clone();
    let select =
        crate::ui::Select::new(&select).on_change(move |id: &String, _, cx| acts.set_preferences(json!({ "defaultAgent": id }), cx));
    let mut row = Row::new("default-agent", w::DEFAULT_AGENT).note(w::DEFAULT_AGENT_DESCRIPTION).control(select);
    if prefs.is_some_and(|p| p.default_agent.is_some()) {
        let acts = ctx.acts.clone();
        row = row.on_reset(move |_, _, cx| acts.set_preferences(json!({ "defaultAgent": null }), cx));
    }
    Some(Card::new("agents-new-threads").head(w::NEW_THREADS).item(row).into_any_element())
}

/// A card under a head drawn as an element: the head, then its surface.
fn headed(head: AnyElement, card: Card) -> AnyElement {
    v_flex().gap_3().child(head).child(card).into_any_element()
}

/// `NotReadCard`: what the host could not read there, each part it missed and why under it.
fn not_read(lines: Vec<(String, Option<String>)>) -> Option<AnyElement> {
    if lines.is_empty() {
        return None;
    }
    let rows = lines.into_iter().enumerate().map(|(at, (label, said))| {
        let title = if said.is_none() { capitalised(&label) } else { label };
        let mut row = Row::new(SharedString::from(format!("refused-{at}")), title);
        if let Some(said) = said {
            row = row.note(capitalised(&said));
        }
        row
    });
    Some(Card::new("not-read").head(w::NOT_READ).items(rows).into_any_element())
}

/// The head of the computer's first list: its name, when it was read, and the refresh.
fn computer_head(store: &Entity<Store>, key: &str, head: AnyElement, rows: &Rows, again: &str, cx: &App) -> AnyElement {
    let s = store.read(cx);
    let read = s.reads.get(&rows.place);
    let when = read.and_then(|r| r.read_at).map(|at| w::checked_now(&read_when(at.elapsed().unwrap_or_default().as_millis() as u64)));
    let reading = read.is_some_and(|r| r.reading);
    let (store, place) = (store.clone(), rows.place.clone());
    on_head(
        key,
        head,
        when,
        reading,
        Some(std::rc::Rc::new(move |_, cx: &mut App| store.update(cx, |s, cx| s.refresh(&place, cx)))),
        again,
        cx,
    )
}

/// `AgentsCards`: the agents on the computer, then the ones it could install, then what the host could not read.
fn agents_cards(store: &Entity<Store>, rows: &Rows, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    let (report, reading, error) = {
        let s = store.read(cx);
        let read = s.reads.get(&rows.place);
        (read.and_then(|r| r.report.clone()), read.is_some_and(|r| r.reading), read.and_then(|r| r.error.clone()))
    };
    let mut out = Vec::new();
    let on = w::on(&rows.name);
    match &report {
        None if reading => out.push(Card::new("agents-on").head(on.clone()).body(RowSkeleton::new("agents-reading")).into_any_element()),
        _ => {
            let installed: Vec<CardItem> = report
                .iter()
                .flat_map(|r| r.agents.iter())
                .filter(|a| a.installed)
                .map(|a| agent_line(store, a, rows, window, cx))
                .collect();
            let head = computer_head(store, "agents-on", on.into_any_element(), rows, w::READ_AGENTS_AGAIN, cx);
            out.push(headed(head, Card::new("agents-on").items(installed)));
        }
    }
    let available: Vec<CardItem> =
        report.iter().flat_map(|r| r.agents.iter()).filter(|a| !a.installed).map(|a| agent_line(store, a, rows, window, cx)).collect();
    if !available.is_empty() {
        out.push(Card::new("agents-available").head(w::AVAILABLE_TO_INSTALL).items(available).into_any_element());
    }
    let mut lines = report.as_ref().map(|r| refused_lines(&r.refused)).unwrap_or_default();
    lines.extend(rows.misses.iter().cloned());
    if report.is_none() {
        if let Some(error) = error {
            lines.push((error, None));
        }
    }
    out.extend(not_read(lines));
    out
}

/// `AgentsControls`: the computer the page reads and, on the Agents page, which list.
fn controls(ctx: &Ctx, store: &Entity<Store>, place: &PlaceView, tabs: bool, window: &mut Window, cx: &mut App) -> AnyElement {
    let items = computers(ctx)
        .into_iter()
        .map(|p| {
            let icon = crate::ui::ComputerIcon::of(p, ctx.data.preferences());
            let mark: keyed::Mark =
                std::rc::Rc::new(move |cx: &App| icon.draw(crate::ui::ActivePalette::palette(cx).foreground.opacity(0.8), px(14.)));
            (p.id.clone(), SharedString::from(place_name(p)), Some(mark))
        })
        .collect();
    let select = keyed::pick("agents-picker", items, Some(place.id.clone()), window, cx);
    let (pick_store, pick_acts) = (store.clone(), ctx.acts.clone());
    let select = crate::ui::Select::new(&select).width(px(224.)).height(px(32.)).on_change(move |id: &String, _, cx| {
        let id = id.clone();
        pick_store.update(cx, |s, cx| s.pick_place(id, cx));
        pick_acts.open_level(None, cx);
    });
    let tab = store.read(cx).tab;
    let tabs = tabs.then(|| {
        let control = Segmented::new("agents-tabs", Tab::ALL.iter().position(|t| *t == tab)).glyph_size(px(14.), px(6.));
        let control = w::TABS
            .iter()
            .zip([Glyph::Bot, Glyph::Server, Glyph::ScrollText, Glyph::Puzzle])
            .fold(control, |control, ((_, label), glyph)| control.glyph_segment(glyph, *label));
        let (store, acts) = (store.clone(), ctx.acts.clone());
        control.on_pick(move |ix, _, cx| {
            let t = Tab::ALL[ix];
            store.update(cx, |s, cx| {
                s.tab = t;
                cx.notify();
            });
            acts.open_level(None, cx);
        })
    });
    h_flex()
        .debug_selector(|| "agents-controls".into())
        .w_full()
        .flex_wrap()
        .justify_between()
        .gap_3()
        .child(select)
        .children(tabs)
        .into_any_element()
}

/// The agents tab: new threads' agent, then the agents on the picked computer and the ones it could install.
fn agents_tab(ctx: &Ctx, store: &Entity<Store>, rows: &Rows, window: &mut Window, cx: &mut App) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = new_threads(ctx, ctx.data.preferences(), window, cx).into_iter().collect();
    out.extend(agents_cards(store, rows, window, cx));
    out
}

/// The page itself, drawn in the one card the web's page is.
fn agents_page(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    let store = Store::of(&ctx.agents, &ctx.acts, cx);
    let place = picked(ctx, store.read(cx))?;
    let rows = Rows::of(place, store.read(cx));
    store.update(cx, |s, cx| {
        s.read(&place.id, cx);
        if rows.here && ctx.data.places.iter().any(|p| kind_of(p) == "provider") {
            s.ask_broker(cx);
        }
    });
    let tab = store.read(cx).tab;
    let tabs = tab == Tab::Agents || ctx.level.is_none();
    let mut parts = vec![controls(ctx, &store, place, tabs, window, cx)];
    match tab {
        Tab::Agents => parts.extend(agents_tab(ctx, &store, &rows, window, cx)),
        kind => parts.push(kinds::tab(ctx, &store, &rows, kind, window, cx)),
    }
    Some(v_flex().w_full().gap(px(30.)).children(parts).into_any_element())
}

/// What an agent's own page holds, as its row in a search says it.
fn agent_page_line() -> String {
    let words: Vec<String> = [w::MODEL, w::EFFORT, w::ACCESS, w::MODELS, w::PROGRAM, w::CONFIG_FOLDER, w::LAUNCH_ARGUMENTS, w::ENVIRONMENT]
        .iter()
        .map(|w| w.to_lowercase())
        .collect();
    capitalised(&list_words(&words))
}

/// The rows a search finds on the Agents page: the default agent, then one per agent page there is, each opening it:
/// the agents a thread runs on, then every other agent the picked computer's last read found installed.
fn search_items(ctx: &Ctx, cx: &App) -> Vec<Item> {
    let line = agent_page_line();
    let acts = ctx.acts.clone();
    let mut items = vec![Item::row("default-agent", w::DEFAULT_AGENT, Some(w::DEFAULT_AGENT_DESCRIPTION), move |row| {
        row.on_open(move |_, window, cx| acts.go(At::Group(GroupId::Agents), window, cx))
    })];
    let store = ctx.agents.read(cx);
    let place = store.place.as_deref().unwrap_or(HERE);
    let report = store.reads.get(place).and_then(|r| r.report.as_ref());
    let others = report
        .iter()
        .flat_map(|r| r.agents.iter())
        .filter(|row| row.installed && !ctx.data.harnesses.iter().any(|h| h.harness == row.id))
        .map(|row| (row.id.clone(), row.name.clone(), w::AGENT_HEAD_LINE.to_owned()));
    let pages: Vec<(String, String, String)> =
        ctx.data.harnesses.iter().map(|h| (h.harness.to_string(), h.label.clone(), line.clone())).chain(others).collect();
    for (id, label, line) in pages {
        let acts = ctx.acts.clone();
        let mark_id = id.clone();
        items.push(Item::row(format!("agent-{id}"), label.clone(), Some(&line), move |row| {
            row.lead(gpui_kit::div().child(AgentLead { id: mark_id, label }))
                .on_open(move |_, window, cx| acts.go(At::Agent(id.clone()), window, cx))
        }));
    }
    items
}

/// An agent's mark in its frame, drawn when its row is.
#[derive(IntoElement)]
struct AgentLead {
    id: String,
    label: String,
}

impl RenderOnce for AgentLead {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        framed(crate::ui::catalog::mark(&self.id, &self.label, px(16.), cx.palette().foreground, cx), cx)
    }
}

pub fn cards(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let mut card = CardData::new("agents").search(search_items(ctx, cx));
    if *ctx.at == At::Group(GroupId::Agents) {
        if let Some(page) = agents_page(ctx, window, cx) {
            card = card.body(page);
        }
    }
    vec![card]
}

pub fn page(id: &str, ctx: &Ctx, window: &mut Window, cx: &mut App) -> Vec<CardData> {
    let store = Store::of(&ctx.agents, &ctx.acts, cx);
    let Some(place) = picked(ctx, store.read(cx)) else { return Vec::new() };
    let rows = Rows::of(place, store.read(cx));
    store.update(cx, |s, cx| {
        s.read(&place.id, cx);
        s.ask_accounts(cx);
        s.ask_catalogs(&place.id, cx);
    });
    let mut parts = vec![controls(ctx, &store, place, false, window, cx)];
    parts.extend(agent_page::page(id, ctx, &store, &rows, window, cx));
    vec![CardData::new(SharedString::from(format!("agent-{id}"))).body(v_flex().w_full().gap(px(30.)).children(parts))]
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::model::records::{agent, catalog, report};
    use super::*;
    use crate::screens::settings::model::Data;
    use crate::screens::settings::Acts;

    #[test]
    fn a_login_that_asks_to_pick_is_the_line_for_a_terminal_on_every_computer() {
        let row = agent(json!({ "id": "opencode", "name": "OpenCode", "signInRoad": "terminal" }));
        for (place, here) in [(HERE, true), ("box1", false)] {
            let rows = Rows { place: place.into(), here, name: place.into(), held: None, broker: None, misses: Vec::new() };
            assert_eq!(
                sign_in_start(&row, &rows),
                Some(Start::Copy { line: "wsp agents signin opencode".into(), why: None }),
                "on {place}, with no terminal to draw in place (D65)"
            );
        }
    }

    struct Listed {
        store: Entity<Store>,
    }

    impl Render for Listed {
        fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            let rows = Rows { place: HERE.into(), here: true, name: "here".into(), held: None, broker: None, misses: Vec::new() };
            let row = agent(json!({ "id": "claude", "name": "Claude Code" }));
            div().w(px(760.)).child(Card::new("agents-test").item(agent_line(&self.store, &row, &rows, window, cx)))
        }
    }

    /// `SignInFlowView` in the Agents list keeps its refusal slot's room (`reserve` unset), so the block under a token
    /// field stands as tall as the web's.
    #[gpui_kit::test]
    fn a_token_field_under_an_agent_keeps_its_refusal_slots_room(cx: &mut gpui_kit::TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
        let (_, cx) = cx.add_window_view(|_, cx| {
            let store = cx.new(|_| Store::new(acts));
            let flow = Flow::Vault {
                agent: "claude".into(),
                mint: Some("claude setup-token".into()),
                word: "token",
                saving: false,
                refused: None,
            };
            store.update(cx, |s, _| s.flows.insert((HERE.to_owned(), agent_row("claude")), flow));
            Listed { store }
        });
        cx.simulate_resize(gpui_kit::size(px(800.), px(600.)));
        cx.run_until_parked();
        let mint = cx.debug_bounds("copy-row").expect("the sign-in under the row, its mint line first");
        let slot = cx.debug_bounds("refusal").expect("the empty refusal slot stands under the token field");
        assert_eq!(slot.size.height, px(36.), "{slot:?}");
        assert!(slot.top() > mint.bottom(), "{slot:?} under {mint:?}");
    }

    #[gpui_kit::test]
    fn a_search_finds_an_agent_installed_there_that_no_thread_runs_on(cx: &mut gpui_kit::TestAppContext) {
        let runtime = Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
        let window = cx.add_empty_window();
        window.update(|_, cx| {
            let data = Data { harnesses: vec![catalog(json!({ "harness": "claude", "label": "Claude Code" }))], ..Data::default() };
            let at = At::Group(GroupId::Agents);
            let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), None, runtime);
            let activity = cx.new(|_| super::super::activity::Pick::default());
            let agents = cx.new(|_| Store::new(acts.clone()));
            let mut read = report(json!({}));
            read.agents = vec![
                agent(json!({ "id": "claude", "name": "Claude Code" })),
                agent(json!({ "id": "pi", "name": "Pi" })),
                agent(json!({ "id": "amp", "name": "Amp", "installed": false })),
            ];
            agents.update(cx, |s, _| s.reads.entry(HERE.to_owned()).or_default().report = Some(read));
            let ctx = Ctx::new(
                &data,
                &at,
                None,
                acts,
                activity,
                agents,
                cx.new(|_| crate::screens::settings::pages::computers::RowMenu::default()),
            );
            let found: Vec<(String, String)> =
                search_items(&ctx, cx).iter().skip(1).map(|i| (i.words[0].to_string(), i.words[1].to_string())).collect();
            assert_eq!(
                found,
                [("Claude Code".to_owned(), agent_page_line()), ("Pi".to_owned(), w::AGENT_HEAD_LINE.to_owned())],
                "the harness once, as its page; the other installed agent once; the one not installed not at all"
            );
        });
    }
}
