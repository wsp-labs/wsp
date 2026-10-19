// SPDX-License-Identifier: AGPL-3.0-only
//! What Add a computer draws on each step (`AddComputerDialog.tsx`, `PickLists.tsx`): its head, its body and its
//! foot.

use std::rc::Rc;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, ClickEvent, Context, Focusable as _, Hsla, PathPromptOptions, SharedString, Window};
use serde_json::{json, Value};

use super::choices;
use super::model::{self, github_pick, str_of, AddStep, Standing, StepLine};
use super::rows::RowAct;
use super::words;
use super::{AddComputer, Folder, FolderFields, List, HUES, ICONS};
use crate::screens::dialogs::layer;
use crate::screens::dialogs::picks::{pick_line, Changes, PickItem, PickList, Tick, RANGE_HOVER};
use crate::screens::dialogs::sign_in::{self, Flow, SignInView};
use crate::screens::dialogs::steps::{self, retry_act, skip_act, ReadyPage, SignInWait, StepLog};
use crate::screens::sidebar::looks;
use crate::ui::card::CardItem;
use crate::ui::{
    catalog, fmt_bytes, size, size_ink, spinner, text, ActivePalette, Button, Card, Choice, CopyRow, Dialog, Field, Glyph, GridHead,
    HeadCell, Line, PickRow, Refusal, Select, SelectItem, SelectState, Step, StepRow,
};

/// An icon's or a hue's name as its select says it.
pub fn icon_word(id: &str) -> String {
    words::capitalised(id)
}

/// A project hue's ink, the muted one for none (`PROJECT_HUES`).
fn hue_ink(hue: &str, cx: &App) -> Hsla {
    looks::hue(hue, cx.palette()).unwrap_or(cx.palette().muted_foreground)
}

/// `Size`: a size in the weight table's ink (`sizeTone`), drawn only where the host measured the row.
fn size_cell(bytes: Option<u64>, cx: &App) -> Option<AnyElement> {
    let bytes = bytes?;
    Some(div().text_size(text::FACT.0).line_height(text::FACT.1).text_color(size_ink(bytes, cx)).child(fmt_bytes(bytes)).into_any_element())
}

fn fact(words: impl Into<SharedString>, cx: &App) -> AnyElement {
    div().text_size(text::FACT.0).line_height(text::FACT.1).text_color(cx.palette().muted_foreground).child(words.into()).into_any_element()
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

fn dialog_card(id: impl Into<SharedString>) -> Card {
    Card::new(id).inset(size::DIALOG_INSET)
}

/// What a pick list's press hands the view: the changes, put on the picks by `put`.
fn on_set(
    cx: &mut Context<AddComputer>,
    put: impl Fn(&Value, &Value, &Changes) -> Value + 'static,
) -> impl Fn(Changes, &mut Window, &mut App) + 'static {
    let weak = cx.entity().downgrade();
    move |changes, _, cx| {
        let _ = weak.update(cx, |this, cx| {
            let (Some(picks), Some(options)) = (this.picks.clone(), this.options.clone()) else { return };
            let next = put(&picks, &options, &changes);
            this.set_picks(next, None, cx);
        });
    }
}

impl AddComputer {
    fn picks_now(&self, cx: &App) -> Value {
        self.picks.clone().unwrap_or_else(|| choices::no_picks(&self.box_name(cx)))
    }

    fn where_body(&mut self, host_key: Option<String>, sheet: bool, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = cx.palette().clone();
        let address = self.address.read(cx).value().trim().to_owned();
        // `size="lg"`: 36 px and 14 px at 640 px and wider, 40 px and 16 px under it.
        let (high, type_size) = if sheet { (px(40.), px(16.)) } else { (px(36.), px(14.)) };
        let mut out = vec![Field::new(&self.address).sheet().height(high).type_size(type_size).into_any_element()];
        if let Some(key) = host_key {
            out.push(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_size(text::NOTE.0)
                            .line_height(text::NOTE.1)
                            .text_color(p.muted_foreground)
                            .child(format!("{address}'s host key is new to {}.", self.here())),
                    )
                    .child(CopyRow::new("host-key", key))
                    .into_any_element(),
            );
        } else if let Some(said) = &self.hosts_refused {
            out.push(refusal(&model::Said { said: words::hosts_not_read(&said.said), fix: said.fix.clone() }).into_any_element());
        } else if !self.hosts.is_empty() {
            let picked = self
                .hosts
                .iter()
                .find(|h| str_of(h, "alias") == Some(address.as_str()))
                .and_then(|h| str_of(h, "alias"))
                .map(str::to_owned);
            let rows: Vec<Choice> = self
                .hosts
                .iter()
                .map(|h| {
                    let alias = str_of(h, "alias").unwrap_or_default().to_owned();
                    let pick = alias.clone();
                    let login = [str_of(h, "user"), str_of(h, "hostName").or(Some(alias.as_str()))]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join("@");
                    let choice =
                        Choice::new(SharedString::from(format!("host-{alias}")), alias.clone(), picked.as_deref() == Some(alias.as_str()))
                            .glyph(Glyph::Server)
                            .fact(login);
                    // Before a pick every name reads as an answer; after one, the others dim.
                    let choice = if picked.is_none() { choice.full_ink() } else { choice };
                    choice.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.typed = Some(pick.clone());
                        cx.notify();
                    }))
                })
                .collect();
            out.push(
                v_flex()
                    .debug_selector(|| "ssh-hosts".into())
                    .child(GridHead::new(None, vec![HeadCell::new(words::SUGGESTED)]))
                    .child(dialog_card("ssh-hosts").items(rows))
                    .into_any_element(),
            );
        }
        out
    }

    fn checks_body(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let sudo = model::asked_sudo(self.job.as_ref());
        let typed = !self.sudo.read(cx).value().is_empty();
        let rows: Vec<StepRow> = model::check_rows(self.job.as_ref())
            .into_iter()
            .map(|line| {
                let failed = line.state == Step::Failed;
                let row = step_row(&line);
                if !failed {
                    return row;
                }
                let row = if sudo { row.act(div().w(px(176.)).child(Field::new(&self.sudo))) } else { row };
                row.act(
                    Button::outline("try-again", "Try again")
                        .xs()
                        .held(sudo && !typed)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.try_again(window, cx))),
                )
            })
            .collect();
        vec![dialog_card("checks").items(rows).into_any_element()]
    }

    /// The step's body once the options are read: a refusal where they were refused, the spinner while they come.
    fn options_body(
        &mut self,
        cx: &mut Context<Self>,
        draw: impl FnOnce(&mut Self, Value, &mut Context<Self>) -> Vec<AnyElement>,
    ) -> Vec<AnyElement> {
        match self.options.clone() {
            Some(options) => draw(self, options, cx),
            None => match &self.options_refused {
                Some(said) => vec![refusal(said).into_any_element()],
                None => vec![h_flex()
                    .debug_selector(|| "options-loading".into())
                    .flex_1()
                    .min_h(px(200.))
                    .justify_center()
                    .child(spinner(cx))
                    .into_any_element()],
            },
        }
    }

    fn start_from_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let here = self.here();
        let picks = self.picks_now(cx);
        let name = str_of(&picks, "name").unwrap_or_default().to_owned();
        let all = choices::everything(&name, &options, &self.here_projects(), self.preferences.get("projectLook").unwrap_or(&json!({})));
        let pick = |id: &str, next: Value, cx: &mut Context<Self>| pick_from(cx, id.to_owned(), next);
        let mut rows = vec![Choice::new("from-here", format!("Everything on {here}"), self.from == "here")
            .glyph(Glyph::ListChecks)
            .note(format!("{}.", words::recipe_summary(&all)))
            .on_click(pick("here", all.clone(), cx))];
        for recipe in self.recipes.clone() {
            let slug = str_of(&recipe, "slug").unwrap_or_default().to_owned();
            let icon = self.preferences.pointer(&format!("/recipeLook/{slug}/icon")).and_then(Value::as_str).unwrap_or("folder").to_owned();
            let day = str_of(&recipe, "savedAt")
                .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                .map(|d| d.format("%b %-d, %Y").to_string());
            let file = recipe.get("file").cloned().unwrap_or(json!({}));
            rows.push(
                Choice::new(
                    SharedString::from(format!("from-{slug}")),
                    str_of(&recipe, "name").unwrap_or_default().to_owned(),
                    self.from == slug,
                )
                .glyph(looks::project_icon(&icon))
                .note(words::saved_recipe(str_of(&recipe, "summary").unwrap_or_default(), day.as_deref()))
                .on_click(pick(&slug, choices::from_recipe(&file, &name), cx)),
            );
        }
        rows.push(
            Choice::new("from-none", "Pick each step", self.from == "none")
                .glyph(Glyph::ListChecks)
                .note("Nothing ticked.")
                .on_click(pick("none", choices::no_picks(&name), cx)),
        );
        vec![dialog_card("start-from").items(rows).into_any_element()]
    }

    fn agents_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let name = self.box_name(cx);
        let here = self.here();
        let versions =
            self.places.iter().find(|p| str_of(p, "id") == Some("here")).and_then(|p| p.get("agentVersions")).cloned().unwrap_or(json!({}));
        let agents = list(&options, "agents").to_vec();
        let items: Vec<PickItem> = agents
            .iter()
            .map(|a| {
                let id = str_of(a, "id").unwrap_or_default();
                PickItem::new(id.to_owned(), str_of(a, "name").unwrap_or(id).to_owned(), choices::has(&picks, "agents", id))
            })
            .collect();
        let mut selects = std::collections::HashMap::new();
        for a in &agents {
            let id = str_of(a, "id").unwrap_or_default().to_owned();
            let ways: Vec<String> = list(a, "signins").iter().filter_map(Value::as_str).map(str::to_owned).collect();
            if !choices::has(&picks, "agents", &id) || ways.is_empty() {
                continue;
            }
            let way = picks
                .pointer(&format!("/agents/{id}/signin"))
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| ways[0].clone());
            let kind = str_of(a, "kind").map(str::to_owned);
            let items: Vec<SelectItem<String>> =
                ways.iter().map(|w| SelectItem::new(w.clone(), sign_in_word(w, &name, kind.as_deref()))).collect();
            let state = self.ways.entry(id.clone()).or_insert_with(|| cx.new(|_| SelectState::new(Vec::new(), None))).clone();
            state.update(cx, |s, cx| {
                s.set_items(items, cx);
                if s.value() != Some(&way) {
                    s.set_value(Some(&way), cx);
                }
            });
            let weak = cx.entity().downgrade();
            let agent = id.clone();
            selects.insert(
                id,
                Select::new(&state).on_change(move |next: &String, _, cx| {
                    let next = next.clone();
                    let agent = agent.clone();
                    let _ = weak.update(cx, |this, cx| {
                        let picks = this.picks_now(cx);
                        this.set_picks(choices::sign_in(&picks, &agent, &next), None, cx);
                    });
                }),
            );
        }
        let selects = Rc::new(std::cell::RefCell::new(selects));
        let state = self.lists[&List::Agents].clone();
        let row = {
            let (picks, versions, here, agents, name) = (picks.clone(), versions.clone(), here.clone(), agents.clone(), name.clone());
            move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
                let id = item.key().to_string();
                let agent = agents.iter().find(|a| str_of(a, "id") == Some(&id)).cloned().unwrap_or(json!({}));
                let label = item.name().to_string();
                let way = picks.pointer(&format!("/agents/{id}/signin")).and_then(Value::as_str).unwrap_or("vault");
                let ways: Vec<&str> = list(&agent, "signins").iter().filter_map(Value::as_str).collect();
                // Ticked with no way this computer can serve but its own road, it says that road.
                let road = (item.is_on() && ways.iter().all(|w| *w == "machine")).then(|| catalog::sign_in(&id).map(|s| s.road)).flatten();
                let note = if item.is_on() && (way == "token" || way == "key") {
                    Some(way_note(&id, &label, way, &here))
                } else {
                    match road {
                        Some(road @ ("token" | "key")) => Some(words::paste_once_set_up(road)),
                        Some("terminal") => Some(words::at_its_terminal_once_set_up(if name.is_empty() { "that computer" } else { &name })),
                        _ => None,
                    }
                };
                let mut row = PickRow::new(SharedString::from(format!("agent-{id}")), label.clone(), item.is_on())
                    .on_click(tick.press())
                    .hover(RANGE_HOVER)
                    .locked(item.is_locked());
                let mark_id = id.clone();
                let mark_label = label.clone();
                row = row.framed(gpui_kit::div().child(AgentMark { harness: mark_id, label: mark_label }));
                if let Some(v) = versions.get(&id).and_then(Value::as_str) {
                    row = row.tag(v.to_owned());
                }
                if let Some(note) = note {
                    row = row.note(note);
                }
                let select = selects.borrow_mut().remove(&id);
                let bytes = agent.get("bytes").and_then(Value::as_u64);
                if select.is_some() || bytes.is_some() {
                    row = row.slot(h_flex().gap_3().children(bytes.map(SizeCell)).children(select));
                }
                vec![row.into()]
            }
        };
        vec![PickList::new(
            "agents",
            &state,
            items,
            on_set(cx, |picks, options, changes| choices::tick_many(picks, "agents", changes, options)),
            row,
        )
        .into_any_element()]
    }

    fn servers_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let name = self.box_name(cx);
        let copied = choices::copies_keys(&picks);
        let servers = list(&options, "mcp").to_vec();
        let items: Vec<PickItem> = servers
            .iter()
            .map(|s| {
                let n = str_of(s, "name").unwrap_or_default();
                PickItem::new(n.to_owned(), n.to_owned(), choices::has(&picks, "mcp", n))
            })
            .collect();
        let row = {
            let (servers, name) = (servers.clone(), name.clone());
            move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
                let s = servers.iter().find(|s| str_of(s, "name") == Some(item.key())).cloned().unwrap_or(json!({}));
                let keyed = !list(&s, "keys").is_empty();
                let note = if item.is_on() && keyed && !copied {
                    Some(words::KEYS_STAY.to_owned())
                } else {
                    str_of(&s, "kind").and_then(|k| words::server_sign_in(k, &name))
                };
                let agents: Vec<String> = list(&s, "agents").iter().filter_map(Value::as_str).map(str::to_owned).collect();
                let mut row = PickRow::new(SharedString::from(format!("server-{}", item.key())), item.name().clone(), item.is_on())
                    .glyph(Glyph::Plug)
                    .marks(
                        h_flex().gap_1().children(agents.into_iter().map(|a| AgentMark { label: catalog::name(&a), harness: a }.small())),
                    )
                    .on_click(tick.press())
                    .hover(RANGE_HOVER)
                    .locked(item.is_locked());
                if let Some(note) = note {
                    row = row.note(note);
                }
                vec![row.into()]
            }
        };
        let state = self.lists[&List::Servers].clone();
        let mut out =
            vec![PickList::new("servers", &state, items, on_set(cx, |p, o, c| choices::tick_many(p, "mcp", c, o)), row).into_any_element()];
        let keyed = choices::keyed_servers(&picks, &options);
        if !keyed.is_empty() {
            let names: Vec<String> = keyed.iter().map(|(n, _)| n.clone()).collect();
            let copy = picks.clone();
            let leave = picks.clone();
            out.push(
                dialog_card("server-keys")
                    .item(
                        Choice::new("keys-copy", words::copy_keys_to(&name), copied)
                            .glyph(Glyph::KeyRound)
                            .note(words::keys_named(&keyed))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.set_picks(choices::set_copy_keys(&copy, true), None, cx)
                            })),
                    )
                    .item(
                        Choice::new("keys-leave", words::leave_keys(&self.here()), !copied)
                            .glyph(Glyph::KeyRound)
                            .note(words::keys_left(&names, &name))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.set_picks(choices::set_copy_keys(&leave, false), None, cx)
                            })),
                    )
                    .into_any_element(),
            );
        }
        out
    }

    fn clis_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let mut clis = list(&options, "clis").to_vec();
        clis.sort_by_key(|c| std::cmp::Reverse(c.get("calls").and_then(Value::as_u64).unwrap_or(0)));
        let items: Vec<PickItem> = clis
            .iter()
            .map(|c| {
                let n = str_of(c, "name").unwrap_or_default();
                PickItem::new(n.to_owned(), n.to_owned(), choices::has(&picks, "clis", n))
            })
            .collect();
        let mut out = Vec::new();
        let unticked_used = clis.iter().any(|c| {
            c.get("calls").and_then(Value::as_u64).unwrap_or(0) > 0 && !choices::has(&picks, "clis", str_of(c, "name").unwrap_or_default())
        });
        if unticked_used {
            out.push(
                h_flex()
                    .justify_end()
                    .child(Button::outline("tick-used", "Select the ones your agents used").xs().glyph(Glyph::ListChecks).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            let (Some(picks), Some(options)) = (this.picks.clone(), this.options.clone()) else { return };
                            this.set_picks(choices::tick_used_clis(&picks, &options), None, cx);
                        }),
                    ))
                    .into_any_element(),
            );
        }
        let row = move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
            let c = clis.iter().find(|c| str_of(c, "name") == Some(item.key())).cloned().unwrap_or(json!({}));
            let via = str_of(&c, "via").unwrap_or("apt");
            let needs: Vec<String> = list(&c, "needs").iter().filter_map(Value::as_str).map(str::to_owned).collect();
            let note = if needs.is_empty() { via.to_owned() } else { format!("{via}. Needs {}.", needs.join(", ")) };
            let mut row = PickRow::new(SharedString::from(format!("cli-{}", item.key())), item.name().clone(), item.is_on())
                .glyph(Glyph::Terminal)
                .note(note)
                .on_click(tick.press())
                .hover(RANGE_HOVER)
                .locked(item.is_locked());
            if let Some(v) = str_of(&c, "version") {
                row = row.tag(v.to_owned());
            }
            let calls = c.get("calls").and_then(Value::as_u64);
            let bytes = c.get("bytes").and_then(Value::as_u64);
            if calls.is_some() || bytes.is_some() {
                row = row.slot(
                    h_flex()
                        .gap_3()
                        .children(calls.map(CallsCell))
                        .child(h_flex().min_w(px(56.)).justify_end().children(bytes.map(SizeCell))),
                );
            }
            vec![row.into()]
        };
        let state = self.lists[&List::Clis].clone();
        out.push(PickList::new("clis", &state, items, on_set(cx, |p, o, c| choices::tick_many(p, "clis", c, o)), row).into_any_element());
        out
    }

    fn skills_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let skills = list(&options, "skills").to_vec();
        let items: Vec<PickItem> = skills
            .iter()
            .map(|s| {
                let n = str_of(s, "name").unwrap_or_default();
                PickItem::new(n.to_owned(), n.to_owned(), choices::has(&picks, "skills", n))
            })
            .collect();
        let row = move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
            let from = skills
                .iter()
                .find(|s| str_of(s, "name") == Some(item.key()))
                .and_then(|s| str_of(s, "from"))
                .unwrap_or_default()
                .to_owned();
            vec![PickRow::new(SharedString::from(format!("skill-{}", item.key())), item.name().clone(), item.is_on())
                .glyph(Glyph::ScrollText)
                .slot(FactCell(from))
                .on_click(tick.press())
                .hover(RANGE_HOVER)
                .locked(item.is_locked())
                .into()]
        };
        let state = self.lists[&List::Skills].clone();
        vec![PickList::new("skills", &state, items, on_set(cx, |p, o, c| choices::tick_many(p, "skills", c, o)), row).into_any_element()]
    }

    fn plugins_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let items: Vec<PickItem> = list(&options, "plugins")
            .iter()
            .map(|p| {
                let n = str_of(p, "name").unwrap_or_default();
                PickItem::new(n.to_owned(), n.to_owned(), choices::has(&picks, "plugins", n))
            })
            .collect();
        let row = |item: &PickItem, tick: Tick| -> Vec<CardItem> {
            let key = item.key().to_string();
            let at = key.rfind('@').filter(|a| *a > 0);
            let name = at.map_or(key.clone(), |a| key[..a].to_owned());
            let mut row = PickRow::new(SharedString::from(format!("plugin-{key}")), name, item.is_on())
                .glyph(Glyph::Puzzle)
                .on_click(tick.press())
                .hover(RANGE_HOVER)
                .locked(item.is_locked());
            if let Some(a) = at {
                row = row.tag(key[a + 1..].to_owned());
            }
            vec![row.into()]
        };
        let state = self.lists[&List::Plugins].clone();
        vec![PickList::new("plugins", &state, items, on_set(cx, |p, o, c| choices::tick_many(p, "plugins", c, o)), row).into_any_element()]
    }

    fn github_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let picked = github_pick(&picks).to_owned();
        let name = self.box_name(cx);
        let here = self.here();
        let offered = list(&options, "configs").iter().find(|c| str_of(c, "id") == Some("github")).cloned().unwrap_or(json!({}));
        let rows: Vec<Choice> = list(&offered, "signins")
            .iter()
            .filter_map(Value::as_str)
            .map(|way| {
                let (title, note) = match way {
                    "vault" => (
                        format!("Use the token from {here}"),
                        str_of(&offered, "account").map(|a| {
                            let scopes: Vec<String> =
                                list(&offered, "scopes").iter().filter_map(Value::as_str).map(str::to_owned).collect();
                            words::github_account(a, &scopes)
                        }),
                    ),
                    "machine" => (format!("Sign in on {name}"), Some("A tab opens in your browser here.".to_owned())),
                    _ => (
                        words::SKIP_FOR_NOW.to_owned(),
                        Some("Private repos will not clone until you sign in. You can do it later in Settings.".to_owned()),
                    ),
                };
                let next = choices::set_github(&picks, way);
                let choice = Choice::new(SharedString::from(format!("github-{way}")), title, picked == way)
                    .glyph(Glyph::Github)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.set_picks(next.clone(), None, cx)));
                match note {
                    Some(n) => choice.note(n),
                    None => choice,
                }
            })
            .collect();
        vec![dialog_card("github").items(rows).into_any_element()]
    }

    /// The folders a person may import: ones added by hand, ones the picks hold that are no project here, then every
    /// project here.
    fn folders(&self, cx: &App) -> Vec<Folder> {
        let picks = self.picks_now(cx);
        let facts = self.options.as_ref().map(|o| list(o, "folders").to_vec()).unwrap_or_default();
        let fact_of = |path: &str| facts.iter().find(|f| str_of(f, "path") == Some(path)).cloned().unwrap_or(json!({}));
        let here = self.here_projects();
        let looks = self.preferences.get("projectLook").cloned().unwrap_or(json!({}));
        let with_facts = |mut f: Folder| {
            let fact = fact_of(&f.path);
            f.bytes = fact.get("bytes").and_then(Value::as_u64);
            f.private = fact.get("private") == Some(&json!(true));
            f.unpushed = fact.get("unpushed").and_then(Value::as_u64);
            f
        };
        let mut out: Vec<Folder> = self.added.iter().cloned().map(with_facts).collect();
        if let Some(table) = picks.get("folders").and_then(Value::as_object) {
            for (key, row) in table {
                if here.iter().any(|p| choices::folder_key(str_of(p, "name").unwrap_or_default()) == *key)
                    || out.iter().any(|f| &f.key == key)
                {
                    continue;
                }
                out.push(with_facts(Folder {
                    key: key.clone(),
                    name: str_of(row, "name").unwrap_or(key).to_owned(),
                    path: str_of(row, "from").unwrap_or_default().to_owned(),
                    remote: None,
                    icon: str_of(row, "icon").unwrap_or("folder").to_owned(),
                    hue: str_of(row, "hue").unwrap_or("neutral").to_owned(),
                    bytes: None,
                    private: false,
                    unpushed: None,
                }));
            }
        }
        for p in here {
            let id = str_of(&p, "id").unwrap_or_default();
            out.push(with_facts(Folder {
                key: choices::folder_key(str_of(&p, "name").unwrap_or_default()),
                name: str_of(&p, "name").unwrap_or_default().to_owned(),
                path: str_of(&p, "path").unwrap_or_default().to_owned(),
                remote: Some(str_of(&p, "remote").unwrap_or_default().to_owned()),
                icon: looks.pointer(&format!("/{id}/icon")).and_then(Value::as_str).unwrap_or("folder").to_owned(),
                hue: looks.pointer(&format!("/{id}/hue")).and_then(Value::as_str).unwrap_or("neutral").to_owned(),
                bytes: None,
                private: false,
                unpushed: None,
            }));
        }
        out
    }

    /// The fields a ticked folder opens, made on first draw and kept by its key.
    fn folder_fields(&mut self, key: &str, name: &str, icon: &str, hue: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.folders.contains_key(key) {
            let field = cx.new(|cx| InputState::new(window, cx).default_value(name.to_owned()));
            let icons = ICONS.iter().map(|i| SelectItem::new(i.to_string(), icon_word(i)).glyph(looks::project_icon(i))).collect();
            let icon_state = cx.new(|_| SelectState::new(icons, Some(&icon.to_owned())));
            let hues = HUES
                .iter()
                .map(|h| {
                    let h2 = h.to_string();
                    SelectItem::new(h.to_string(), icon_word(h))
                        .mark(move |cx| div().size(px(12.)).rounded_full().bg(hue_ink(&h2, cx)).into_any_element())
                })
                .collect();
            let hue_state = cx.new(|_| SelectState::new(hues, Some(&hue.to_owned())));
            let k = key.to_owned();
            let sub = cx.subscribe(&field, move |this, field, e: &InputEvent, cx| {
                if matches!(e, InputEvent::Change) {
                    let name = field.read(cx).value().to_string();
                    this.put_folder(&k, |row| row["name"] = json!(name), cx);
                }
            });
            self.folders.insert(key.to_owned(), FolderFields { name: field, icon: icon_state, hue: hue_state, _subs: vec![sub] });
        }
    }

    /// One field of a ticked folder's row changed.
    fn put_folder(&mut self, key: &str, change: impl FnOnce(&mut Value), cx: &mut Context<Self>) {
        let Some(picks) = self.picks.clone() else { return };
        let Some(mut row) = picks.pointer(&format!("/folders/{key}")).cloned() else { return };
        change(&mut row);
        self.set_picks(choices::tick_folder(&picks, key, Some(row)), None, cx);
    }

    fn projects_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let name = self.box_name(cx);
        let no_github = github_pick(&picks) == "skip";
        let folders = self.folders(cx);
        let on_box: Vec<String> = self
            .projects
            .iter()
            .filter(|p| str_of(p, "computer") == self.place_id.as_deref())
            .filter_map(|p| str_of(p, "name").map(str::to_owned))
            .collect();
        let row_of = |f: &Folder| -> Value {
            let held = picks.pointer(&format!("/folders/{}", f.key));
            let mut row = json!({
                "from": f.path,
                "name": held.and_then(|h| str_of(h, "name")).unwrap_or(&f.name),
                "icon": held.and_then(|h| str_of(h, "icon")).unwrap_or(&f.icon),
                "hue": held.and_then(|h| str_of(h, "hue")).unwrap_or(&f.hue),
                "keep": held.and_then(|h| h.get("keep")).cloned().unwrap_or(json!([])),
            });
            if let Some(image) = held.and_then(|h| h.get("image")).filter(|i| !i.is_null()) {
                row["image"] = image.clone();
            }
            row
        };
        let rows: Vec<(Folder, Value, bool)> =
            folders.iter().map(|f| (f.clone(), row_of(f), choices::has(&picks, "folders", &f.key))).collect();
        for (f, row, on) in &rows {
            if *on {
                self.folder_fields(
                    &f.key,
                    str_of(row, "name").unwrap_or_default(),
                    str_of(row, "icon").unwrap_or("folder"),
                    str_of(row, "hue").unwrap_or("neutral"),
                    window,
                    cx,
                );
            }
        }
        let items: Vec<PickItem> =
            rows.iter().map(|(f, row, on)| PickItem::new(f.key.clone(), str_of(row, "name").unwrap_or_default().to_owned(), *on)).collect();
        let fields: std::collections::HashMap<String, Fields> =
            self.folders.iter().map(|(k, f)| (k.clone(), (f.name.clone(), f.icon.clone(), f.hue.clone()))).collect();
        let weak = cx.entity().downgrade();
        let row = {
            let rows = rows.clone();
            move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
                let Some((f, row, on)) = rows.iter().find(|(f, _, _)| f.key.as_str() == item.key().as_ref()).cloned() else {
                    return Vec::new();
                };
                let shown = str_of(&row, "name").unwrap_or_default().to_owned();
                let clash = on && on_box.contains(&shown);
                let note = if on && no_github && f.private {
                    Some(words::NEEDS_GITHUB.to_owned())
                } else {
                    f.remote.as_deref().map(|r| if r.is_empty() { words::NO_REMOTE.to_owned() } else { words::remote_line(r, f.unpushed) })
                };
                let icon = str_of(&row, "icon").unwrap_or("folder").to_owned();
                let hue = str_of(&row, "hue").unwrap_or("neutral").to_owned();
                let mut pick = PickRow::new(SharedString::from(format!("folder-{}", f.key)), shown.clone(), on)
                    .framed(HuedGlyph { icon: icon.clone(), hue: hue.clone() })
                    .tag(f.path.clone())
                    .on_click(tick.press())
                    .hover(RANGE_HOVER)
                    .locked(item.is_locked());
                if let Some(note) = note {
                    pick = pick.note(note);
                }
                if let Some(size) = f.bytes {
                    pick = pick.slot(SizeCell(size));
                }
                let mut out: Vec<CardItem> = vec![pick.into()];
                if let (true, Some((field, icon_state, hue_state))) = (on, fields.get(&f.key)) {
                    let taken = clash.then(|| format!("{name} already has a project called {shown}."));
                    out.push(pick_line(
                        format!("{}-name", f.key),
                        "Name",
                        v_flex().items_end().gap_1().child(Field::new(field).invalid(clash)).children(taken.map(TakenLine)),
                    ));
                    let (k1, k2, w1, w2) = (f.key.clone(), f.key.clone(), weak.clone(), weak.clone());
                    out.push(pick_line(
                        format!("{}-icon", f.key),
                        "Icon",
                        Select::new(icon_state).on_change(move |next: &String, _, cx| {
                            let next = next.clone();
                            let k = k1.clone();
                            let _ = w1.update(cx, |this, cx| {
                                this.put_folder(
                                    &k,
                                    |row| {
                                        row["icon"] = json!(next);
                                        if let Some(o) = row.as_object_mut() {
                                            o.remove("image");
                                        }
                                    },
                                    cx,
                                )
                            });
                        }),
                    ));
                    out.push(pick_line(
                        format!("{}-hue", f.key),
                        "Colour",
                        Select::new(hue_state).on_change(move |next: &String, _, cx| {
                            let next = next.clone();
                            let k = k2.clone();
                            let _ = w2.update(cx, |this, cx| this.put_folder(&k, |row| row["hue"] = json!(next), cx));
                        }),
                    ));
                }
                out
            }
        };
        let put = {
            let rows = rows.clone();
            move |picks: &Value, _: &Value, changes: &Changes| -> Value {
                changes.iter().fold(picks.clone(), |next, (key, on)| match rows.iter().find(|(f, _, _)| f.key.as_str() == key.as_ref()) {
                    Some((f, row, _)) => choices::tick_folder(&next, &f.key, on.then(|| row.clone())),
                    None => next,
                })
            }
        };
        let state = self.lists[&List::Projects].clone();
        let weak = cx.entity().downgrade();
        let set = move |changes: Changes, _: &mut Window, cx: &mut App| {
            let _ = weak.update(cx, |this, cx| {
                let picks = this.picks_now(cx);
                this.set_picks(put(&picks, &Value::Null, &changes), None, cx);
            });
        };
        vec![
            PickList::new("projects", &state, items, set, row).into_any_element(),
            h_flex()
                .child(Button::add("add-folder", "Add a folder…").on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                    let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
                    cx.spawn(async move |this, cx| {
                        let Ok(Ok(Some(paths))) = picked.await else { return };
                        let Some(path) = paths.into_iter().next() else { return };
                        let _ = this.update(cx, |this, cx| this.add_folder(&path.to_string_lossy(), cx));
                    })
                    .detach();
                })))
                .into_any_element(),
        ]
    }

    /// A folder picked by hand: it stands at the top, ticked.
    pub fn add_folder(&mut self, path: &str, cx: &mut Context<Self>) {
        let name = path.split('/').rfind(|s| !s.is_empty()).unwrap_or(path).to_owned();
        let folder = Folder {
            key: choices::folder_key(&name),
            name: name.clone(),
            path: path.to_owned(),
            remote: None,
            icon: "folder".into(),
            hue: "neutral".into(),
            bytes: None,
            private: false,
            unpushed: None,
        };
        self.added.retain(|f| f.key != folder.key);
        self.added.insert(0, folder.clone());
        let picks = self.picks_now(cx);
        let row = json!({ "from": path, "name": name, "icon": "folder", "hue": "neutral", "keep": [] });
        self.set_picks(choices::tick_folder(&picks, &folder.key, Some(row)), None, cx);
    }

    fn other_body(&mut self, options: Value, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let picks = self.picks_now(cx);
        let configs = list(&options, "configs").to_vec();
        let items: Vec<PickItem> = [("git", "Git"), ("shell", "Shell")]
            .into_iter()
            .filter(|(id, _)| configs.iter().any(|c| str_of(c, "id") == Some(id)))
            .map(|(id, name)| PickItem::new(id, name, picks.pointer(&format!("/configs/{id}")).is_some_and(|v| !v.is_null())))
            .collect();
        let row = move |item: &PickItem, tick: Tick| -> Vec<CardItem> {
            let label = configs
                .iter()
                .find(|c| str_of(c, "id") == Some(item.key()))
                .and_then(|c| str_of(c, "label"))
                .unwrap_or_default()
                .to_owned();
            let glyph = if item.key().as_ref() == "git" { Glyph::GitCommitHorizontal } else { Glyph::SquareTerminal };
            vec![PickRow::new(SharedString::from(format!("config-{}", item.key())), item.name().clone(), item.is_on())
                .glyph(glyph)
                .note(label)
                .on_click(tick.press())
                .hover(RANGE_HOVER)
                .locked(item.is_locked())
                .into()]
        };
        let state = self.lists[&List::Configs].clone();
        let set = on_set(cx, |picks, _, changes| changes.iter().fold(picks.clone(), |next, (id, on)| choices::tick_config(&next, id, *on)));
        vec![PickList::new("configs", &state, items, set, row).into_any_element()]
    }

    fn summary_body(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = cx.palette().clone();
        let picks = self.picks_now(cx);
        let name = self.box_name(cx);
        let here = self.here();
        let keys =
            |kind: &str| picks.get(kind).and_then(Value::as_object).map(|t| t.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let counts = words::recipe_counts(&picks);
        let count = |kind: &str| counts.iter().find(|(k, _)| *k == kind).map_or(0, |(_, n)| *n).to_string();
        let github = match github_pick(&picks) {
            "vault" => format!("Token from {here}"),
            "machine" => format!("Sign in on {name}"),
            _ => words::SKIP_FOR_NOW.to_owned(),
        };
        let folders: Vec<String> = picks
            .get("folders")
            .and_then(Value::as_object)
            .map(|t| t.values().map(|f| str_of(f, "name").or_else(|| str_of(f, "from")).unwrap_or_default().to_owned()).collect())
            .unwrap_or_default();
        let other: Vec<&str> =
            ["git", "shell"].into_iter().filter(|id| picks.pointer(&format!("/configs/{id}")).is_some_and(|v| !v.is_null())).collect();
        let lines = [
            ("Agents", keys("agents").iter().map(|a| catalog::name(a)).collect::<Vec<_>>().join(", ")),
            ("MCP servers", keys("mcp").join(", ")),
            ("CLIs", keys("clis").join(", ")),
            ("Skills", count("skills")),
            ("Plugins", count("plugins")),
            ("GitHub", github),
            ("Projects", folders.join(", ")),
            ("Other config", other.join(", ")),
        ];
        let estimate = self.estimate.clone();
        let free = estimate
            .as_ref()
            .and_then(|e| e.get("freeBytes"))
            .and_then(Value::as_u64)
            .or_else(|| self.place().and_then(|p| p.get("diskFreeBytes")).and_then(Value::as_u64));
        let short = model::too_big(estimate.as_ref());
        let mut card = dialog_card("summary-lines");
        for (label, value) in lines {
            let shown = if value.is_empty() || value == "0" { "None".to_owned() } else { value };
            card = card.item(Line::new(SharedString::from(format!("summary-{label}")), label).fact(shown));
        }
        if estimate.is_some() || free.is_some() {
            let said = match &estimate {
                None => format!("{} free", fmt_bytes(free.unwrap_or(0))),
                Some(e) => words::disk_line(
                    &fmt_bytes(e.get("neededBytes").and_then(Value::as_u64).unwrap_or(0)),
                    &fmt_bytes(e.get("keptBytes").and_then(Value::as_u64).unwrap_or(0)),
                    free.map(fmt_bytes).as_deref(),
                ),
            };
            let mut line = Line::new("disk", format!("Disk on {name}")).right(
                div()
                    .debug_selector(|| "disk".into())
                    .text_size(text::FACT.0)
                    .line_height(text::FACT.1)
                    .text_color(if short { p.error_foreground } else { p.muted_foreground })
                    .child(said),
            );
            if let Some(n) = estimate.as_ref().and_then(|e| e.get("unmeasured")).and_then(Value::as_u64).filter(|n| *n > 0) {
                line = line.hover(words::unmeasured(n));
            }
            card = card.item(line);
        }
        let mut out = vec![card.into_any_element()];
        if short {
            let e = estimate.unwrap_or(json!({}));
            let b = |k: &str| fmt_bytes(e.get(k).and_then(Value::as_u64).unwrap_or(0));
            let (said, fix) = words::disk_short(&name, &fmt_bytes(free.unwrap_or(0)), &b("neededBytes"), &b("keptBytes"));
            out.push(div().debug_selector(|| "disk-short".into()).child(Refusal::new().said(said).fix(fix)).into_any_element());
        }
        let icon = self.recipe_icon.read(cx).value().cloned().unwrap_or_else(|| "rocket".into());
        let mut save = dialog_card("save-recipe").item(
            PickRow::new("save", "Save as a recipe", self.save_on)
                .glyph(looks::project_icon(&icon))
                .note("The next box starts from these picks.")
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.save_on = !this.save_on;
                    cx.notify();
                })),
        );
        if self.save_on {
            save = save.item(pick_line("recipe-name", "Name", Field::new(&self.recipe_name))).item(pick_line("recipe-icon", "Icon", {
                let view = cx.entity().downgrade();
                Select::new(&self.recipe_icon).on_change(move |_: &String, _, cx| {
                    let _ = view.update(cx, |_, cx| cx.notify());
                })
            }));
        }
        out.push(save.into_any_element());
        if let Some(said) = &self.refused {
            out.push(div().debug_selector(|| "setup-refused".into()).child(refusal(said)).into_any_element());
        }
        out
    }

    /// The running view: each row of the setup with what it asks, the first open on its log where it failed.
    fn running_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let Some(place) = self.place().cloned() else { return Vec::new() };
        let name = model::place_name(&place);
        let here = self.here();
        let rows: Vec<StepLine> = model::setup_rows(&place, &name)
            .into_iter()
            .map(|mut r| {
                if r.state == Step::Running {
                    r.since = model::running_since(place.get("setup"), &r.id);
                }
                r
            })
            .collect();
        // A field a sign-in under its row takes a token, a key or a code in, made on first draw and kept by its row.
        for r in &rows {
            let wants = matches!(self.sign_ins.of(&r.id), Some(Flow::Vault { .. }) | Some(Flow::Run { paste: true, .. }));
            if wants && !self.sign_in_fields.contains_key(&r.id) {
                let field = cx.new(|cx| InputState::new(window, cx));
                self.sign_in_fields.insert(r.id.clone(), field);
            }
        }
        let own_picks = str_of(&place, "recipe").is_none_or(|r| r == "none");
        let busy = self.busy;
        let asks = rows.iter().find(|r| matches!(r.state, Step::NeedsYou | Step::Failed)).map(|r| r.id.clone());
        let items: Vec<StepRow> = rows
            .iter()
            .map(|r| {
                let ways = r.sign_in.as_deref().map(|agent| model::sign_in_ways(agent, place.get("picks"))).unwrap_or_default();
                let flow = self.sign_ins.of(&r.id).cloned();
                let mut line = r.clone();
                // With the key's field open under it, the row's fix says to paste the key there, not to put it in a file.
                if ways.contains(&"key") && matches!(flow, Some(Flow::Vault { .. })) {
                    let agent = r.sign_in.clone().unwrap_or_default();
                    line.fix = Some(ways_fix(&agent, &catalog::name(&agent), &ways, &here, &name));
                }
                let mut row = step_row(&line);
                let opens = model::opens_log(r);
                let open = opens && *self.turned.get(&r.id).unwrap_or(&(r.state == Step::Failed));
                let id = r.id.clone();
                row = row
                    .toggle(open, opens.then(|| cx.listener(move |this: &mut Self, _: &ClickEvent, _, cx| this.open_row(&id, !open, cx))));
                if let Some(wait) = &r.wait {
                    let (retry_id, skip_id) = (r.id.clone(), r.id.clone());
                    row = row.under(
                        SignInWait::new(SharedString::from(format!("wait-{}", r.id)))
                            .code(str_of(wait, "code").map(str::to_owned))
                            .url(str_of(wait, "url").map(str::to_owned))
                            .expired(str_of(wait, "state") == Some("expired"))
                            .busy(busy)
                            .on_retry(cx.listener(move |this, _: &ClickEvent, _, cx| this.row_act(retry_id.clone(), RowAct::Retry, cx)))
                            .on_skip(cx.listener(move |this, _: &ClickEvent, _, cx| this.row_act(skip_id.clone(), RowAct::Skip, cx))),
                    );
                } else if let Some(flow) = &flow {
                    row = row.under(self.sign_in_view(&r.id, flow, cx));
                } else if open {
                    let lines = self.logs.as_ref().map(|l| l.get(&r.id).cloned().unwrap_or_default());
                    if let Some(lines) = lines {
                        row = row.under(StepLog::new(
                            SharedString::from(format!("log-{}", r.id)),
                            lines.into_iter().map(SharedString::from).collect(),
                        ));
                    }
                }
                if let Some((_, said)) = self.row_refused.as_ref().filter(|(id, _)| id == &r.id) {
                    row = row.under(refusal(said));
                }
                if asks.as_ref() == Some(&r.id) {
                    row = row.under(scroll_to(self.scroll.clone(), self.shown_asks.clone()));
                }
                let act = |cx: &mut Context<Self>, act: RowAct, id: &str| row_listener(cx, id.to_owned(), act);
                let skip = |cx: &mut Context<Self>| {
                    skip_act(SharedString::from(format!("skip-{}", r.id)), words::SKIP, busy).on_click(act(cx, RowAct::Skip, &r.id))
                };
                if r.wait.is_some() {
                    return row;
                }
                // A sign-in drawn under its row is closed by Cancel there, never by the Sign in that opened it.
                if flow.is_some() {
                    return row.act(tagged(
                        "cancel",
                        Button::outline(SharedString::from(format!("cancel-{}", r.id)), sign_in::CANCEL)
                            .xs()
                            .glyph(Glyph::X)
                            .on_click(act(cx, RowAct::Cancel, &r.id)),
                    ));
                }
                if !ways.is_empty() && matches!(r.state, Step::Failed | Step::Skipped) {
                    for way in ways {
                        let glyph = if way == "key" { Glyph::KeyRound } else { Glyph::LogIn };
                        row = row.act(tagged(
                            way,
                            Button::outline(SharedString::from(format!("sign-in-{way}-{}", r.id)), way_label(way, &name))
                                .xs()
                                .glyph(glyph)
                                .held(busy)
                                .on_click(act(cx, RowAct::Way(way), &r.id)),
                        ));
                    }
                    if r.state == Step::Failed && r.sub {
                        row = row.act(skip(cx));
                    }
                    return row;
                }
                let sign_in = r.sign_in.is_some().then(|| {
                    tagged(
                        "sign-in",
                        Button::outline(SharedString::from(format!("sign-in-{}", r.id)), words::sign_in_on(&name))
                            .xs()
                            .glyph(Glyph::LogIn)
                            .held(busy)
                            .on_click(act(cx, RowAct::SignIn, &r.id)),
                    )
                });
                match r.state {
                    Step::Skipped if r.copy_keys && own_picks => row.act(tagged(
                        "copy-keys",
                        Button::outline(SharedString::from(format!("copy-keys-{}", r.id)), words::COPY_KEYS).xs().held(busy).on_click(act(
                            cx,
                            RowAct::CopyKeys,
                            &r.id,
                        )),
                    )),
                    Step::Skipped => match sign_in {
                        Some(sign_in) => row.act(sign_in),
                        None => row,
                    },
                    Step::Failed => {
                        let row = match sign_in {
                            Some(sign_in) => row.act(sign_in),
                            None => row.act(tagged(
                                "retry",
                                retry_act(SharedString::from(format!("retry-{}", r.id)), busy).on_click(act(cx, RowAct::Retry, &r.id)),
                            )),
                        };
                        if r.sub {
                            row.act(skip(cx))
                        } else {
                            row
                        }
                    }
                    _ => row,
                }
            })
            .collect();
        vec![dialog_card("setup").items(items).into_any_element()]
    }

    /// One sign-in under its row, its field the row's own and its Save and Submit going through the sign-ins.
    fn sign_in_view(&self, row: &str, flow: &Flow, cx: &mut Context<Self>) -> SignInView {
        let mut view = SignInView::new(SharedString::from(format!("sign-in-{row}")), flow);
        if let Some(field) = self.sign_in_fields.get(row) {
            view = view.field(field);
        }
        let (save_row, code_row) = (row.to_owned(), row.to_owned());
        let (save, code) = (cx.entity().downgrade(), cx.entity().downgrade());
        view.on_save(move |key, _, cx| {
            let _ = save.update(cx, |this, cx| {
                if let Some(ask) = this.sign_ins.save(&save_row, &key) {
                    this.send_sign_in(ask, cx);
                }
                cx.notify();
            });
        })
        .on_code(move |typed, _, cx| {
            let _ = code.update(cx, |this, cx| {
                if let Some(ask) = this.sign_ins.code(&code_row, &typed) {
                    this.send_sign_in(ask, cx);
                }
                cx.notify();
            });
        })
    }
}

/// The list opens on the first row that waits on the person, once, when it first appears: the body scrolls so that
/// row's opened part stands in its middle.
fn scroll_to(scroll: gpui_kit::ScrollHandle, shown: Rc<std::cell::Cell<bool>>) -> impl IntoElement {
    let mark = gpui_kit::canvas(
        |_, _, _| {},
        move |at, _, window, _| {
            if shown.replace(true) {
                return;
            }
            let body = scroll.bounds();
            let offset = scroll.offset();
            let max = scroll.max_offset();
            let by = at.center().y - body.center().y;
            let y = (offset.y - by).min(px(0.)).max(-max.y);
            scroll.set_offset(gpui_kit::point(offset.x, y));
            window.refresh();
        },
    )
    .w_full()
    .h(px(0.));
    div().debug_selector(|| "asks-mark".into()).child(mark)
}

/// A refusal as the host said it, `RefusalSlot`: the sentence in the refusal's ink and its fix after it in the
/// foreground.
fn refusal(said: &model::Said) -> Refusal {
    let r = Refusal::new().said(said.said.clone());
    match &said.fix {
        Some(fix) => r.fix(fix.clone()),
        None => r,
    }
}

/// A row act under the selector its kind names (`act-retry`, `act-token`), so a test finds it among the row's acts.
fn tagged(kind: &'static str, act: Button) -> impl IntoElement {
    div().debug_selector(move || format!("act-{kind}")).child(act)
}

fn pick_from(cx: &mut Context<AddComputer>, id: String, next: Value) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let weak = cx.entity().downgrade();
    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        let _ = weak.update(cx, |this, cx| this.set_picks(next.clone(), Some(id.clone()), cx));
    }
}

fn row_listener(cx: &mut Context<AddComputer>, id: String, act: RowAct) -> impl Fn(&ClickEvent, &mut Window, &mut App) + 'static {
    let weak = cx.entity().downgrade();
    move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
        let _ = weak.update(cx, |this, cx| this.row_act(id.clone(), act, cx));
    }
}

/// A ticked folder's name field and its icon and colour selects.
type Fields = (gpui_kit::Entity<InputState>, gpui_kit::Entity<SelectState<String>>, gpui_kit::Entity<SelectState<String>>);

/// `signInWayDoes`: what one way of signing in does, `field` where the key is pasted under the row.
pub fn way_does(agent: &str, name: &str, way: &str, here: &str, field: bool) -> String {
    match way {
        "token" => {
            let mint = catalog::sign_in(agent).and_then(|s| s.mint).map_or_else(|| format!("{name}'s own command"), str::to_owned);
            format!("{mint} runs on {here} and asks your browser once")
        }
        "key" if field => {
            format!("paste your {}; wsp saves it to ~/.wsp/.env without showing it", catalog::key_env(agent).unwrap_or("key"))
        }
        "key" => format!(
            "put {} in ~/.wsp/.env on {here} first, and wsp reads it from there without showing it",
            catalog::key_env(agent).unwrap_or("the key")
        ),
        _ => format!("{name}'s own sign-in runs there"),
    }
}

/// `wayNote`: what a row picked to take a token or a key says it does, as its failed row's fix opens each way with.
fn way_note(agent: &str, name: &str, way: &str, here: &str) -> String {
    let does = way_does(agent, name, way, here, false);
    // The token's sentence opens with the command that makes it, which keeps its own case.
    if way == "token" {
        format!("{does}.")
    } else {
        format!("{}.", words::capitalised(&does))
    }
}

/// `signInWaysFix`: a sentence per way, each opening with its button's name and saying what pressing it does.
fn ways_fix(agent: &str, name: &str, ways: &[&str], here: &str, place: &str) -> String {
    ways.iter().map(|w| format!("{}: {}.", way_label(w, place), way_does(agent, name, w, here, true))).collect::<Vec<_>>().join(" ")
}

/// `signInWayLabel`: each way's name, on its button and at the head of its sentence.
fn way_label(way: &str, place: &str) -> String {
    match way {
        "token" => "Make a token".into(),
        "key" => "Use an API key".into(),
        _ => words::sign_in_on(place),
    }
}

/// `signInWords`: each way of signing in as the picker says it.
fn sign_in_word(way: &str, name: &str, kind: Option<&str>) -> String {
    match way {
        "vault" => "Copy the key".into(),
        "token" => "Make a token".into(),
        "key" => "Use an API key".into(),
        _ => {
            let on = if name.is_empty() { "Sign in there".to_owned() } else { format!("Sign in on {name}") };
            if kind == Some("device") {
                format!("{on} with a code")
            } else {
                on
            }
        }
    }
}

/// One row of a list of steps as `StepLine` holds it.
fn step_row(r: &StepLine) -> StepRow {
    let mut row = StepRow::new(SharedString::from(format!("step-{}", r.id)), r.state, r.name.clone()).nested(r.sub);
    if let Some(note) = &r.note {
        row = row.note(note.clone());
    }
    match (r.since, r.ms) {
        (Some(at), _) => row = row.since(at),
        (None, Some(ms)) => row = row.took(ms),
        _ => {}
    }
    if let Some(said) = r.said.clone() {
        row = row.said(said, r.fix.clone().map(SharedString::from));
    }
    row
}

/// An agent's own mark at 20 px in its frame, or 14 px after a server's name.
#[derive(IntoElement)]
struct AgentMark {
    harness: String,
    label: String,
}

impl AgentMark {
    fn small(self) -> SmallMark {
        SmallMark(self)
    }
}

impl RenderOnce for AgentMark {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        catalog::mark(&self.harness, &self.label, px(20.), cx.palette().foreground.opacity(0.8), cx)
    }
}

#[derive(IntoElement)]
struct SmallMark(AgentMark);

impl RenderOnce for SmallMark {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        catalog::mark(&self.0.harness, &self.0.label, px(14.), cx.palette().muted_foreground, cx)
    }
}

#[derive(IntoElement)]
struct SizeCell(u64);

impl RenderOnce for SizeCell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().children(size_cell(Some(self.0), cx))
    }
}

#[derive(IntoElement)]
struct CallsCell(u64);

impl RenderOnce for CallsCell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().debug_selector(|| "cli-calls".into()).child(fact(words::fmt_calls(self.0), cx))
    }
}

#[derive(IntoElement)]
struct FactCell(String);

impl RenderOnce for FactCell {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div().child(fact(self.0, cx))
    }
}

/// "X already has a project called Y." under the name field, in the refusal's ink.
#[derive(IntoElement)]
struct TakenLine(String);

impl RenderOnce for TakenLine {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .debug_selector(|| "name-taken".into())
            .text_size(text::FACT.0)
            .line_height(px(18.))
            .text_color(cx.palette().error_foreground)
            .child(self.0)
    }
}

/// A folder's glyph in its hue, at 16 px.
#[derive(IntoElement)]
struct HuedGlyph {
    icon: String,
    hue: String,
}

impl RenderOnce for HuedGlyph {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let ink = if self.hue == "neutral" { cx.palette().foreground.opacity(0.8) } else { hue_ink(&self.hue, cx) };
        Icon::new(looks::project_icon(&self.icon)).with_size(size::GLYPH).text_color(ink)
    }
}

impl Render for AddComputer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(typed) = self.typed.take() {
            self.address.update(cx, |a, cx| a.set_value(typed, window, cx));
        }
        let name = self.box_name(cx);
        let here = self.here();
        let steps = model::steps_for(self.recipes.len());
        // A field that held focus and is no longer drawn would leave Escape with nowhere to go.
        let address_shown = self.step == AddStep::Where || model::asked_host_key(self.job.as_ref()).is_some();
        let sudo_shown = self.step == AddStep::Checks && model::asked_sudo(self.job.as_ref());
        let lost = (!address_shown && self.address.read(cx).focus_handle(cx).is_focused(window))
            || (!sudo_shown && self.sudo.read(cx).focus_handle(cx).is_focused(window));
        if lost {
            window.focus(&self.focus, cx);
        }
        let host_key = if self.step == AddStep::Checks { model::asked_host_key(self.job.as_ref()) } else { None };
        let step = self.step;
        let place = self.place().cloned();
        if step == AddStep::Ready {
            if let Some(place) = &place {
                let glyph = looks::computer_glyph(place, &self.preferences, px(24.), cx.palette().foreground.opacity(0.8));
                let line = place.get("picks").filter(|p| !p.is_null()).map(|p| format!("{}.", words::recipe_summary(p)));
                let page = ReadyPage::new(
                    glyph,
                    model::place_name(place),
                    Button::primary("start-task", "Start a task here")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.start_task(window, cx))),
                )
                .line(line)
                .on_close(cx.listener(|this, _: &ClickEvent, window, cx| this.close(window, cx)));
                return Dialog::new(format!("{} is ready", model::place_name(place)))
                    .layered()
                    .frame()
                    .focus(&self.focus)
                    .on_outside(layer::dismiss)
                    .page(page);
            }
        }
        let standing = place.as_ref().map_or(Standing::Running, model::standing);
        let (title, line): (String, Option<String>) = match step {
            AddStep::Where => (words::TITLE.into(), Some(words::WHERE.into())),
            AddStep::Checks if host_key.is_some() => (words::TITLE.into(), Some(words::WHERE.into())),
            AddStep::Running | AddStep::Ready => match standing {
                Standing::Failed => (format!("Setup on {name} failed"), None),
                Standing::NeedsYou => (format!("{name} needs you"), Some(words::REST_DONE.into())),
                Standing::Ready => {
                    let setup = place.as_ref().and_then(|p| p.get("setup"));
                    let at = |k: &str| setup.and_then(|s| str_of(s, k)).and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok());
                    let took = match (at("startedAt"), at("finishedAt")) {
                        (Some(a), Some(b)) => Some(format!("Set up in {}.", steps::fmt_duration((b - a).num_milliseconds() as f64))),
                        _ => None,
                    };
                    (format!("{name} is ready"), took)
                }
                Standing::Running => (format!("Setting up {name}"), None),
            },
            s => (words::step_title(s).into(), Some(words::step_line(s, &name, &here))),
        };
        let (count, why) = if step == AddStep::Running || step == AddStep::Ready {
            let rows = place.as_ref().map(|p| model::setup_rows(p, &name)).unwrap_or_default();
            let (done, of) = model::setup_count(&rows);
            (format!("{done} of {of} done"), format!("{done} of {of} steps done"))
        } else {
            model::step_figure(&steps, step, host_key.is_some())
        };
        let body: Vec<AnyElement> = match step {
            AddStep::Where => self.where_body(None, steps::narrow(window), cx),
            AddStep::Checks if host_key.is_some() => self.where_body(host_key.clone(), steps::narrow(window), cx),
            AddStep::Checks => self.checks_body(cx),
            AddStep::StartFrom => self.options_body(cx, |this, o, cx| this.start_from_body(o, cx)),
            AddStep::Agents => {
                let options = self.options.clone();
                match options {
                    Some(o) => self.agents_body(o, cx),
                    None => self.options_body(cx, |_, _, _| Vec::new()),
                }
            }
            AddStep::Mcp => self.options_body(cx, |this, o, cx| this.servers_body(o, cx)),
            AddStep::Clis => self.options_body(cx, |this, o, cx| this.clis_body(o, cx)),
            AddStep::Skills => self.options_body(cx, |this, o, cx| this.skills_body(o, cx)),
            AddStep::Plugins => self.options_body(cx, |this, o, cx| this.plugins_body(o, cx)),
            AddStep::Github => self.options_body(cx, |this, o, cx| this.github_body(o, cx)),
            AddStep::Projects => self.projects_body(window, cx),
            AddStep::Other => self.options_body(cx, |this, o, cx| this.other_body(o, cx)),
            AddStep::Summary => self.summary_body(cx),
            AddStep::Running | AddStep::Ready => self.running_body(window, cx),
        };
        let at = steps.iter().position(|s| *s == step);
        let back_to = at.and_then(|i| i.checked_sub(1)).map(|i| steps[i]);
        let can_back = back_to.is_some_and(|b| b != AddStep::Checks || self.add_id.is_some());
        let back = back_to.filter(|_| can_back).map(|to| {
            Button::outline("back", "Back")
                .held(self.starting)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.go(to, cx)))
                .into_any_element()
        });
        let next = at.and_then(|i| steps.get(i + 1)).copied();
        let primary = |word: &str, held: bool| Button::primary("continue", word.to_owned()).held(held);
        let close = |word: &'static str, cx: &mut Context<Self>| {
            tagged(word, Button::outline("close", word).on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close(window, cx))))
                .into_any_element()
        };
        let mut acts: Vec<AnyElement> = Vec::new();
        let address_empty = self.address.read(cx).value().trim().is_empty();
        match step {
            AddStep::Where => {
                acts.push(
                    Button::outline("cancel", "Cancel")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close(window, cx)))
                        .into_any_element(),
                );
                acts.push(
                    primary("Connect", address_empty)
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.connect(None, None, window, cx)))
                        .into_any_element(),
                );
            }
            AddStep::Checks if host_key.is_some() => {
                acts.push(
                    Button::outline("cancel", "Cancel")
                        .on_click(cx.listener(|this, _: &ClickEvent, window, cx| this.close(window, cx)))
                        .into_any_element(),
                );
                let key = host_key.clone();
                acts.push(
                    primary("Trust and connect", false)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| this.connect(key.clone(), None, window, cx)))
                        .into_any_element(),
                );
            }
            AddStep::Checks => {
                acts.extend(back);
                let first = model::first_pick(self.recipes.len());
                acts.push(
                    primary("Continue", self.place_id.is_none() || !self.recipes_read())
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| this.go(first, cx)))
                        .into_any_element(),
                );
            }
            AddStep::Projects => {
                acts.extend(back);
                let on_box: Vec<String> = self
                    .projects
                    .iter()
                    .filter(|p| str_of(p, "computer") == self.place_id.as_deref())
                    .filter_map(|p| str_of(p, "name").map(str::to_owned))
                    .collect();
                let taken = self
                    .picks
                    .as_ref()
                    .and_then(|p| p.get("folders"))
                    .and_then(Value::as_object)
                    .is_some_and(|t| t.values().any(|f| on_box.iter().any(|n| Some(n.as_str()) == str_of(f, "name"))));
                acts.push(
                    primary("Continue", taken)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(n) = next {
                                this.go(n, cx)
                            }
                        }))
                        .into_any_element(),
                );
            }
            AddStep::Summary => {
                acts.extend(back);
                let held = place.is_none() || model::empty(self.picks.as_ref()) || self.starting || model::too_big(self.estimate.as_ref());
                acts.push(
                    primary(&format!("Set up {name}"), held)
                        .busy(self.starting)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.set_up(cx)))
                        .into_any_element(),
                );
            }
            AddStep::Running | AddStep::Ready => match standing {
                Standing::Ready => {
                    acts.push(close("Close", cx));
                    acts.push(
                        tagged(
                            "Next",
                            primary("Next", false).on_click(cx.listener(|this, _: &ClickEvent, _, cx| this.go(AddStep::Ready, cx))),
                        )
                        .into_any_element(),
                    );
                }
                Standing::Running => acts.push(close("Run in background", cx)),
                _ => acts.push(close("Close", cx)),
            },
            _ => {
                acts.extend(back);
                acts.push(
                    primary("Continue", false)
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if let Some(n) = next {
                                this.go(n, cx)
                            }
                        }))
                        .into_any_element(),
                );
            }
        }
        let mut dialog = Dialog::new(title)
            .layered()
            .frame()
            .count_words(count, why)
            .children(body)
            .scroll(&self.scroll)
            .focus(&self.focus)
            .on_outside(layer::dismiss);
        if let Some(line) = line {
            dialog = dialog.line(line);
        }
        if step.picks() {
            dialog = dialog.state(steps::saved_line(self.saves, window, cx));
        }
        for act in acts {
            dialog = dialog.act(act);
        }
        dialog
    }
}

#[cfg(test)]
mod tests {
    use super::{way_note, ways_fix};

    #[test]
    fn each_way_says_what_it_does_in_the_catalogs_words() {
        assert_eq!(way_note("claude", "Claude Code", "token", "Studio"), "claude setup-token runs on Studio and asks your browser once.");
        assert_eq!(
            way_note("claude", "Claude Code", "key", "Studio"),
            "Put ANTHROPIC_API_KEY in ~/.wsp/.env on Studio first, and wsp reads it from there without showing it."
        );
        assert_eq!(
            ways_fix("claude", "Claude Code", &["key", "machine"], "Studio", "lab-box"),
            "Use an API key: paste your ANTHROPIC_API_KEY; wsp saves it to ~/.wsp/.env without showing it. Sign in on lab-box: Claude Code's own sign-in runs there."
        );
    }
}
