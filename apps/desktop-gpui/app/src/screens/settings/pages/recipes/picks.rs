// SPDX-License-Identifier: AGPL-3.0-only
//! A recipe's pick lists, one under another (`recipes.tsx`'s sections over `add/PickLists.tsx`): every agent offered
//! with how a ticked one signs in, and the servers, CLIs and skills the recipe holds, each ticked. A tick hands its
//! change to `save`; no filter or order tools stand over a recipe's lists.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gpui_kit::component::{h_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, KeyDownEvent, SharedString};
use serde_json::Value;

use super::choices::{
    agent_way, cli_note, copies_keys, copy_keys, fmt_calls, holds, keyed_servers, keys_left, keys_named, leave_keys, more_skills,
    server_note, set_copy_keys, size_tone, tick_many, ticked_clis, way_words, RecipeOptions, TickKind, FIRST_SKILLS,
};
use crate::screens::settings::page::{CardData, Item};
use crate::screens::settings::pages::agents::agent_marks;
use crate::ui::catalog;
use crate::ui::fmt_bytes;
use crate::ui::{section_head, text, word, ActivePalette, Card, CardItem, Choice, Glyph, PickRow, WordClass};

/// A tick's change, made to the recipe's file as the host holds it when the save goes.
pub type Change = Box<dyn FnOnce(&Value) -> Value>;

/// Where a tick's change goes.
pub type Save = Rc<dyn Fn(Change, &mut App)>;

/// The row each list last took a press on, where a shift-click's range starts.
pub type Anchors = Rc<RefCell<HashMap<TickKind, String>>>;

/// What the lists are drawn from: the recipe's file as drawn, what the host offers, the computers that follow the
/// recipe by name (empty where none does) and the computer the host runs on.
pub struct Picks {
    pub file: Rc<Value>,
    pub options: Rc<RecipeOptions>,
    pub computer: String,
    pub here: String,
    pub save: Save,
    pub anchors: Anchors,
}

impl Picks {
    /// A press that saves the recipe as `next` makes it.
    fn on_press(
        &self,
        next: impl Fn(&Value, &RecipeOptions) -> Value + 'static,
    ) -> impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut App) {
        let (options, save, next) = (self.options.clone(), self.save.clone(), Rc::new(next));
        move |_, _, cx| {
            let (options, next) = (options.clone(), next.clone());
            save(Box::new(move |file| next(file, &options)), cx)
        }
    }

    /// A press ticks or clears its row; a shift-press every row of `shown` from the one pressed before, as this one went
    /// (`PickList`'s tick).
    fn ticks(&self, kind: TickKind, name: &str, shown: &Rc<Vec<String>>) -> Tick {
        let (name, on) = (name.to_owned(), holds(&self.file, kind, name));
        let (options, save, anchors, shown) = (self.options.clone(), self.save.clone(), self.anchors.clone(), shown.clone());
        Rc::new(move |shift, cx| {
            let anchor = anchors.borrow_mut().insert(kind, name.clone());
            let at = |key: &str| shown.iter().position(|k| k == key);
            let range = match (anchor.filter(|_| shift).and_then(|a| at(&a)), at(&name)) {
                (Some(from), Some(to)) => shown[from.min(to)..=from.max(to)].to_vec(),
                _ => vec![name.clone()],
            };
            let changes: Vec<(String, bool)> = range.into_iter().map(|key| (key, !on)).collect();
            let options = options.clone();
            save(Box::new(move |file| tick_many(file, kind, &changes, &options)), cx);
        })
    }
}

/// A row's tick, told whether Shift was held.
type Tick = Rc<dyn Fn(bool, &mut App)>;

/// A press on the row as a tick, Shift read off the press.
fn press(tick: &Tick) -> impl Fn(&gpui_kit::ClickEvent, &mut gpui_kit::Window, &mut App) {
    let tick = tick.clone();
    move |event, _, cx| tick(event.modifiers().shift, cx)
}

/// `Size`: a size in the weight table's ink, mono at 13 px.
fn size_cell(bytes: f64, cx: &App) -> AnyElement {
    div()
        .debug_selector(|| "pick-size".into())
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::FACT.0)
        .line_height(text::FACT.1)
        .text_color(size_tone(bytes).ink(cx.palette()))
        .child(fmt_bytes(bytes as u64))
        .into_any_element()
}

/// A pick row as a card's item, found by its name and its note. Shift with Space on its tick ranges as a shift-press
/// does, since a keyboard press reaches a row only unmodified.
fn item(id: SharedString, name: &str, note: Option<&str>, row: PickRow, tick: Tick) -> Item {
    let mut words = vec![SharedString::from(name.to_owned())];
    words.extend(note.map(|n| SharedString::from(n.to_owned())));
    let row = CardItem::from(row).map(move |el, _| {
        div()
            .on_key_down(move |event: &KeyDownEvent, _, cx| {
                let m = event.keystroke.modifiers;
                if event.keystroke.key == "space" && m.shift && !(m.control || m.alt || m.platform || m.function) {
                    cx.stop_propagation();
                    tick(true, cx);
                }
            })
            .child(el)
            .into_any_element()
    });
    Item::any(id, words, row)
}

/// The four lists as cards: Agents always, the others where the recipe holds any of their kind. Each head stands in its
/// card's body, since the web draws the page as one card the sidebar's trail does not list.
pub fn cards(picks: &Picks, cx: &App) -> Vec<CardData> {
    let file = picks.file.as_ref();
    let held = |kind: &str| file.get(kind).and_then(Value::as_object).is_some_and(|t| !t.is_empty());
    let mut cards = vec![agents(picks, cx)];
    if held("mcp") {
        cards.push(servers(picks, cx));
    }
    if held("clis") {
        cards.push(clis(picks, cx));
    }
    if held("skills") {
        cards.push(skills(picks, cx));
    }
    cards
}

/// Every agent offered; a ticked one says as its note how it signs in.
fn agents(picks: &Picks, cx: &App) -> CardData {
    let shown = Rc::new(picks.options.agents.iter().map(|a| a.id.clone()).collect::<Vec<_>>());
    let rows = picks.options.agents.iter().map(|agent| {
        let on = holds(&picks.file, TickKind::Agents, &agent.id);
        let note = on.then(|| way_words(&agent_way(&picks.file, agent), &picks.computer, agent.kind.as_deref()));
        let id = SharedString::from(format!("pick-agent-{}", agent.id));
        let tick = picks.ticks(TickKind::Agents, &agent.id, &shown);
        let mut row = PickRow::new(id.clone(), agent.name.clone(), on)
            .framed(catalog::mark(&agent.id, &agent.name, px(20.), cx.palette().foreground, cx))
            .on_click(press(&tick));
        if let Some(note) = &note {
            row = row.note(note.clone());
        }
        if let Some(bytes) = agent.bytes {
            row = row.slot(size_cell(bytes, cx));
        }
        item(id, &agent.name, note.as_deref(), row, tick)
    });
    CardData::new("recipe-agents").body(section_head("Agents", cx)).items(rows.collect::<Vec<_>>())
}

/// The servers the recipe holds, and under them the one question about the keys the keyed ones carry.
fn servers(picks: &Picks, cx: &App) -> CardData {
    let held: Vec<_> = picks.options.mcp.iter().filter(|s| holds(&picks.file, TickKind::Mcp, &s.name)).collect();
    let shown = Rc::new(held.iter().map(|s| s.name.clone()).collect::<Vec<_>>());
    let rows = held.into_iter().map(|server| {
        let note = server_note(&picks.file, server, &picks.computer);
        let id = SharedString::from(format!("pick-server-{}", server.name));
        let tick = picks.ticks(TickKind::Mcp, &server.name, &shown);
        let mut row = PickRow::new(id.clone(), server.name.clone(), true).glyph(Glyph::Plug).on_click(press(&tick));
        if let Some(marks) = agent_marks(format!("pick-server-{}-marks", server.name), &server.agents, cx) {
            row = row.marks(marks);
        }
        if let Some(note) = &note {
            row = row.note(note.clone());
        }
        item(id, &server.name, note.as_deref(), row, tick)
    });
    let card = CardData::new("recipe-servers").body(section_head("MCP servers", cx)).items(rows.collect::<Vec<_>>());
    let keyed = keyed_servers(&picks.file, &picks.options);
    if keyed.is_empty() {
        return card;
    }
    let copied = copies_keys(&picks.file);
    let names: Vec<String> = keyed.iter().map(|(name, _)| name.clone()).collect();
    let answer = |id: &str, name: String, note: String, yes: bool| {
        let choice = Choice::new(SharedString::from(format!("server-keys-{id}")), name, copied == yes).glyph(Glyph::KeyRound).note(note);
        if copied == yes {
            choice
        } else {
            choice.on_click(picks.on_press(move |file, _| set_copy_keys(file, yes)))
        }
    };
    let keys = Card::new("server-keys").items([
        answer("copy", copy_keys(&picks.computer), keys_named(&keyed), true),
        answer("leave", leave_keys(&picks.here), keys_left(&names, &picks.computer), false),
    ]);
    card.under(div().flex_1().min_w_0().child(keys))
}

/// The CLIs the recipe holds, the ones the agents ran most first, each with how often and what it weighs.
fn clis(picks: &Picks, cx: &App) -> CardData {
    let held = ticked_clis(&picks.file, &picks.options);
    let shown = Rc::new(held.iter().map(|c| c.name.clone()).collect::<Vec<_>>());
    let rows = held.into_iter().map(|cli| {
        let note = cli_note(cli);
        let id = SharedString::from(format!("pick-cli-{}", cli.name));
        let tick = picks.ticks(TickKind::Clis, &cli.name, &shown);
        let mut row = PickRow::new(id.clone(), cli.name.clone(), true).glyph(Glyph::Terminal).note(note.clone()).on_click(press(&tick));
        if let Some(version) = &cli.version {
            row = row.tag(version.clone());
        }
        if cli.calls.is_some() || cli.bytes.is_some() {
            // The sizes run from "1 MB" to "517 MB": a column as wide as the widest keeps every count on one edge.
            let size = div().flex().justify_end().min_w(px(56.)).children(cli.bytes.map(|b| size_cell(b, cx)));
            row = row.slot(h_flex().gap_3().children(cli.calls.map(|n| word(fmt_calls(n), WordClass::Fact, cx))).child(size));
        }
        item(id, &cli.name, Some(&note), row, tick)
    });
    CardData::new("recipe-clis").body(section_head("CLIs", cx)).items(rows.collect::<Vec<_>>())
}

/// The first skills the recipe holds, then how many more.
fn skills(picks: &Picks, cx: &App) -> CardData {
    let rows: Vec<_> = picks.options.skills.iter().filter(|s| holds(&picks.file, TickKind::Skills, &s.name)).collect();
    let names = Rc::new(rows.iter().take(FIRST_SKILLS).map(|s| s.name.clone()).collect::<Vec<_>>());
    let shown = rows.iter().take(FIRST_SKILLS).map(|skill| {
        let id = SharedString::from(format!("pick-skill-{}", skill.name));
        let tick = picks.ticks(TickKind::Skills, &skill.name, &names);
        let row = PickRow::new(id.clone(), skill.name.clone(), true)
            .glyph(Glyph::ScrollText)
            .slot(word(skill.from.clone(), WordClass::Fact, cx))
            .on_click(press(&tick));
        item(id, &skill.name, None, row, tick)
    });
    let mut card = CardData::new("recipe-skills").body(section_head("Skills", cx)).items(shown.collect::<Vec<_>>());
    if rows.len() > FIRST_SKILLS {
        card = card.item(Item::empty("more-skills", more_skills(rows.len() - FIRST_SKILLS)));
    }
    card
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use gpui_kit::component::{v_flex, Root};
    use gpui_kit::{div, point, prelude::*, px, size, Context, Modifiers, TestAppContext, VisualTestContext, Window};
    use serde_json::{json, Value};

    use super::super::choices::tests::{file, options};
    use super::super::choices::RecipeOptions;
    use super::{cards, Picks};
    use crate::ui::{self, Side};

    /// The lists drawn off `file`, each save made to it as the host would hold it.
    fn picks(file: Value, options: RecipeOptions, saved: &Rc<RefCell<Vec<Value>>>) -> Picks {
        let (saved, held) = (saved.clone(), file.clone());
        Picks {
            file: Rc::new(file),
            options: Rc::new(options),
            computer: "mini".into(),
            here: "zingzy's MacBook".into(),
            save: Rc::new(move |change, _| saved.borrow_mut().push(change(&held))),
            anchors: Rc::default(),
        }
    }

    /// Seven skills, gh and pnpm, and every server ticked.
    fn full() -> (Value, RecipeOptions) {
        let mut options = options();
        options.skills =
            (1..=7).map(|n| serde_json::from_value(json!({ "name": format!("skill-{n}"), "from": "~/.claude/skills" })).unwrap()).collect();
        let mut file = file();
        file["mcp"] = json!({ "docs": { "agents": ["claude"] }, "sentry": { "agents": ["codex"] } });
        file["clis"] = json!({ "pnpm": { "via": "npm" }, "gh": { "via": "apt", "needs": ["git"] } });
        file["skills"] =
            (1..=7).map(|n| (format!("skill-{n}"), json!({ "from": "~/.claude/skills" }))).collect::<serde_json::Map<_, _>>().into();
        (file, options)
    }

    struct Lone(Picks);

    impl Render for Lone {
        fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            div().w(px(688.)).p(px(24.)).child(v_flex().gap(px(30.)).children(cards(&self.0, cx).into_iter().map(|card| card.render())))
        }
    }

    fn draw(cx: &mut TestAppContext, picks: Picks) -> &mut VisualTestContext {
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
        });
        let view = cx.new(|_| Lone(picks));
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
        cx.simulate_resize(size(px(688.), px(2400.)));
        cx.run_until_parked();
        cx
    }

    #[gpui_kit::test]
    fn the_four_lists_say_what_the_recipe_holds(cx: &mut TestAppContext) {
        let saved = Rc::default();
        let (file, options) = full();
        let drawn = cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            cards(&picks(file, options, &saved), cx)
                .into_iter()
                .map(|card| {
                    let words = card.items.iter().map(|i| i.words.iter().map(|w| w.to_string()).collect::<Vec<_>>()).collect::<Vec<_>>();
                    (card.id.to_string(), card.head.is_none() && card.body.is_some(), words, card.under.len())
                })
                .collect::<Vec<_>>()
        });
        let w = |items: &[&[&str]]| items.iter().map(|i| i.iter().map(|s| s.to_string()).collect::<Vec<_>>()).collect::<Vec<_>>();
        let skills: Vec<Vec<String>> =
            (1..=5).map(|n| vec![format!("skill-{n}")]).chain([vec!["2 more skills on this recipe.".to_owned()]]).collect();
        assert_eq!(
            drawn,
            [
                ("recipe-agents".into(), true, w(&[&["Claude Code", "Sign in on mini"], &["Codex"]]), 0),
                (
                    "recipe-servers".into(),
                    true,
                    w(&[&["docs", "Signs in on mini."], &["sentry", "Skipped until you copy its keys."]]),
                    1
                ),
                ("recipe-clis".into(), true, w(&[&["gh", "apt. Needs git."], &["pnpm", "npm"]]), 0),
                ("recipe-skills".into(), true, skills, 0),
            ],
            "heads in the bodies, out of the trail; every agent offered, then only what the recipe holds; the CLIs most called first; five skills and how many more"
        );
        let bare = json!({ "name": "Web app", "agents": {}, "mcp": {}, "clis": {}, "skills": {} });
        let only = cx.update(|cx| cards(&picks(bare, super::super::choices::tests::options(), &saved), cx).len());
        assert_eq!(only, 1, "Agents stands alone where the recipe holds nothing else");
    }

    #[gpui_kit::test]
    fn a_row_draws_its_tag_marks_and_slot(cx: &mut TestAppContext) {
        let saved = Rc::default();
        let (file, options) = full();
        let cx = draw(cx, picks(file, options, &saved));
        let tag = cx.debug_bounds("pick-tag").expect("gh's version stands after its name");
        let size = cx.debug_bounds("pick-size").expect("gh's size is drawn");
        let slot = cx.debug_bounds("pick-slot").expect("a slot is drawn");
        assert!(size.right() <= slot.right() && size.left() >= slot.left(), "the size stands in the slot");
        // `text-[13px]` drops `text-xs`'s leading, so the line is the page's 1.5: what a slot under the name adds.
        assert_eq!(size.size.height, px(19.5), "the size's line");
        assert!(tag.right() < slot.left(), "the tag stands with the name, left of the slot");
        assert!(cx.debug_bounds("radio").is_some(), "the keys question is drawn under the servers");
    }

    #[gpui_kit::test]
    fn a_press_ticks_a_shift_press_ticks_the_range_and_the_slot_ticks_nothing(cx: &mut TestAppContext) {
        let saved: Rc<RefCell<Vec<Value>>> = Rc::default();
        let mut options = options();
        options.agents[1].bytes = Some(300. * 1024. * 1024.);
        let none = json!({ "name": "Web app", "agents": {} });
        let cx = draw(cx, picks(none.clone(), options, &saved));
        let card = cx.debug_bounds("card").expect("the agents card");
        let floor = f32::from(cx.debug_bounds("row").unwrap().size.height);
        assert_eq!(floor, 61., "a second pick row stands at the floor plus the rule over it, as the web's wrapper draws it");
        let (codex, slot) = (cx.debug_bounds("title").unwrap().center(), cx.debug_bounds("pick-slot").unwrap().center());
        cx.simulate_click(slot, Modifiers::none());
        assert!(saved.borrow().is_empty(), "a press on the size is not a tick");
        cx.simulate_click(codex, Modifiers::none());
        let mut one = none.clone();
        one["agents"] = json!({ "codex": {} });
        assert_eq!(saved.borrow().last(), Some(&one), "a press ticks its row");
        cx.simulate_click(point(card.left() + px(200.), card.top() + px(10.)), Modifiers::none());
        let mut claude = none.clone();
        claude["agents"] = json!({ "claude": { "signin": "machine" } });
        assert_eq!(saved.borrow().last(), Some(&claude), "the first row, ticked the first way it signs in");
        cx.simulate_click(codex, Modifiers { shift: true, ..Modifiers::none() });
        let mut both = none;
        both["agents"] = json!({ "claude": { "signin": "machine" }, "codex": {} });
        assert_eq!(saved.borrow().last(), Some(&both), "a shift-press ticks every row from the one pressed before");
        assert_eq!(saved.borrow().len(), 3);
    }

    #[gpui_kit::test]
    fn shift_with_space_on_a_tick_ranges_as_a_shift_press_does(cx: &mut TestAppContext) {
        let saved: Rc<RefCell<Vec<Value>>> = Rc::default();
        let none = json!({ "name": "Web app", "agents": {} });
        let lists = picks(none.clone(), options(), &saved);
        lists.anchors.borrow_mut().insert(super::TickKind::Agents, "claude".into());
        let cx = draw(cx, lists);
        cx.update(|window, cx| {
            window.focus_next(cx);
            window.focus_next(cx);
        });
        cx.simulate_keystrokes("shift-space");
        cx.run_until_parked();
        let mut both = none;
        both["agents"] = json!({ "claude": { "signin": "machine" }, "codex": {} });
        assert_eq!(saved.borrow().as_slice(), [both], "Codex's tick with Shift ticks every row from Claude Code, pressed before");
    }
}
