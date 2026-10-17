// SPDX-License-Identifier: AGPL-3.0-only
//! What opens over the composer: the command and mention menu and the stash, each a glass tab on the box's top edge
//! (ComposerCommandMenu.tsx, ComposerStashMenu.tsx), and the pickers' popups above their triggers: the model picker
//! with its rail of agents (ComposerModelPicker.tsx) and the reasoning and access menus (ComposerOptionPickers.tsx).

use gpui_kit::component::input::Input;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{anchored, deferred, div, point, prelude::*, px, Anchor, AnyElement, App, Context, Div, FontWeight, SharedString};

use super::model::{Choice, Stash};
use super::parts::{access_glyph, agent_mark, attached_surface, INSET};
use super::{Composer, Popup};
use crate::ui::{ActivePalette, Glyph, Palette};

/// The values a popup's rows pick, in the order the keyboard walks them.
pub fn popup_rows(c: &Composer, popup: Popup, cx: &App) -> Vec<String> {
    let r = c.resolved();
    match popup {
        Popup::Model => rail_agent(c).map(|a| model_rows(c, &a, cx).iter().map(|m| m.value.clone()).collect()).unwrap_or_default(),
        Popup::Reasoning => r.efforts.iter().chain(&r.windows).map(|e| e.value.clone()).collect(),
        Popup::Access => r.agent.map(|a| a.permission_modes.iter().map(|m| m.value.clone()).collect()).unwrap_or_default(),
        Popup::Stash => (0..c.stash.entries.len()).map(|ix| ix.to_string()).collect(),
    }
}

/// Whether the legacy models show: as the person left the fold, else open when the model in force is one of them.
pub fn legacy_open(c: &Composer, agent: &super::model::Agent) -> bool {
    c.legacy_open.unwrap_or_else(|| c.resolved().model.is_some_and(|m| agent.legacy_models.contains(&m)))
}

/// The model rows the picker lists for `agent`: its models, then its legacy ones while their fold is open.
fn model_rows(c: &Composer, agent: &super::model::Agent, cx: &App) -> Vec<Choice> {
    let query = c.search.read(cx).value().trim().to_lowercase();
    if !query.is_empty() {
        let hit = |m: &&Choice| m.label.to_lowercase().contains(&query) || m.value.to_lowercase().contains(&query);
        return agent.models.iter().chain(&agent.legacy_models).filter(hit).cloned().collect();
    }
    let legacy = if legacy_open(c, agent) { agent.legacy_models.as_slice() } else { &[] };
    agent.models.iter().chain(legacy).cloned().collect()
}

fn rail_agent(c: &Composer) -> Option<super::model::Agent> {
    let rail = c.rail.clone().or_else(|| c.resolved().agent.map(|a| a.harness))?;
    c.agents.iter().find(|a| a.harness == rail).cloned()
}

/// `dropdown-glass` and `popover-shadow`: the card, a 10% hairline, radius 12.
fn popover_surface(p: &Palette, cx: &App) -> Div {
    let _ = cx;
    div()
        .rounded(px(12.))
        .border_1()
        .border_color(p.popover_edge)
        .bg(p.popover.opacity(0.96))
        .backdrop_blur(px(8.))
        .shadow(p.popover_shadow.clone())
        .overflow_hidden()
}

/// A popup's place: above its trigger, 4 px off, its left edge on the trigger's.
pub fn above(el: impl IntoElement) -> AnyElement {
    div()
        .absolute()
        .top_0()
        .left_0()
        .child(
            deferred(anchored().anchor(Anchor::BottomLeft).offset(point(px(0.), px(-4.))).snap_to_window_with_margin(px(8.)).child(el))
                .with_priority(1),
        )
        .into_any_element()
}

/// The open picker's popup, for the trigger that opened it.
pub fn picker(c: &Composer, popup: Popup, cx: &mut Context<Composer>) -> Option<AnyElement> {
    if c.popup != Some(popup) {
        return None;
    }
    Some(above(match popup {
        Popup::Model => model_picker(c, cx),
        Popup::Reasoning => option_menu(c, Popup::Reasoning, cx),
        Popup::Access => option_menu(c, Popup::Access, cx),
        Popup::Stash => return None,
    }))
}

fn default_badge(p: &Palette) -> impl IntoElement {
    div().ml_2().flex_none().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child("default")
}

/// The reasoning or the access menu: 256 px, group labels, rows of 28 px picked by a fill and no check mark.
fn option_menu(c: &Composer, popup: Popup, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let r = c.resolved();
    let Some(agent) = r.agent.clone() else { return div().into_any_element() };
    let groups: Vec<(&str, &[Choice], Option<&Choice>)> = match popup {
        Popup::Reasoning => vec![("Reasoning", &r.efforts, r.effort.as_ref()), ("Context window", &r.windows, r.window.as_ref())],
        _ => vec![("Access", &agent.permission_modes, r.access.as_ref())],
    };
    let mut list = v_flex().debug_selector(|| "composer-option-menu".into()).p_1();
    let mut n = 0;
    let mut first = true;
    for (label, choices, picked) in groups {
        if choices.is_empty() {
            continue;
        }
        if !first {
            list = list.child(div().mx_2().my_1().h(px(1.)).bg(p.border));
        }
        first = false;
        list = list.child(div().px_2().py(px(6.)).text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(label));
        if popup == Popup::Access && c.running {
            list = list.child(
                div()
                    .px_2()
                    .pb(px(6.))
                    .mt(px(-6.))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(p.muted_foreground)
                    .child("Applies to the turn running now"),
            );
        }
        for choice in choices {
            let ix = n;
            n += 1;
            let checked = picked.is_some_and(|c| c.value == choice.value);
            let active = c.popup_active == Some(ix);
            let value = choice.value.clone();
            let described = choice.description.is_some();
            list =
                list.child(
                    h_flex()
                        .id(SharedString::from(format!("option-{}", choice.value)))
                        .min_h(px(28.))
                        .px_2()
                        .py(if described { px(6.) } else { px(4.) })
                        .when(described, |el| el.items_start())
                        .rounded(px(8.))
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(p.foreground)
                        .when(checked, |el| el.bg(p.foreground.opacity(0.08)))
                        .when(active, |el| el.bg(p.accent))
                        .hover(|s| s.bg(p.accent))
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, window, cx| this.pick(popup, value.clone(), window, cx)))
                        .child(
                            h_flex()
                                .min_w_0()
                                .items_start()
                                .gap_2()
                                .when(popup == Popup::Access, |el| {
                                    el.child(
                                        div().mt(px(3.)).child(
                                            Icon::new(access_glyph(&choice.value)).with_size(px(14.)).text_color(p.muted_foreground),
                                        ),
                                    )
                                })
                                .child(
                                    v_flex()
                                        .min_w_0()
                                        .child(
                                            h_flex()
                                                .child(div().truncate().child(choice.label.clone()))
                                                .when(choice.is_default == Some(true), |el| el.child(default_badge(&p))),
                                        )
                                        .children(choice.description.clone().map(|d| {
                                            div().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(d)
                                        })),
                                ),
                        ),
                );
        }
    }
    popover_surface(&p, cx).w(px(256.)).child(list).into_any_element()
}

/// The model picker: a 48 px rail of agents beside the search and the chosen agent's models, 400 px wide.
fn model_picker(c: &Composer, cx: &mut Context<Composer>) -> AnyElement {
    let p = cx.palette().clone();
    let r = c.resolved();
    let shown = rail_agent(c);
    let mut rail = v_flex().w(px(48.)).flex_none().gap_1().p(px(6.)).border_r_1().border_color(p.border);
    rail = rail.child(
        h_flex()
            .size(px(36.))
            .justify_center()
            .rounded(px(8.))
            .text_color(p.muted_foreground)
            .child(Icon::new(Glyph::Star).with_size(px(16.))),
    );
    rail = rail.child(div().mx(px(6.)).my(px(2.)).h(px(1.)).bg(p.border));
    for agent in &c.agents {
        let on = shown.as_ref().is_some_and(|s| s.harness == agent.harness);
        let harness = agent.harness.clone();
        rail = rail.child(
            h_flex()
                .id(SharedString::from(format!("rail-{}", agent.harness)))
                .relative()
                .size(px(36.))
                .justify_center()
                .rounded(px(8.))
                .text_color(if on { p.foreground } else { p.muted_foreground })
                .cursor_pointer()
                .hover(|s| s.text_color(p.foreground))
                .on_click(cx.listener(move |this, _, _, cx| this.set_rail(harness.clone(), cx)))
                .when(on, |el| el.child(div().absolute().top_2().bottom_2().left(px(-6.)).w(px(2.)).rounded_full().bg(p.primary)))
                .child(agent_mark(&agent.harness, &agent.label, px(20.), cx)),
        );
    }
    let search = h_flex()
        .mx_2()
        .gap(px(10.))
        .px(px(6.))
        .h(px(45.))
        .border_b_1()
        .border_color(p.primary)
        .text_size(px(15.))
        .line_height(px(20.))
        .text_color(p.placeholder)
        .child(Icon::new(Glyph::Search).with_size(px(16.)).text_color(p.muted_foreground))
        .child(div().flex_1().text_color(p.foreground).child(Input::new(&c.search).appearance(false).text_size(px(15.)).h(px(20.)).p_0()));
    let mut list = v_flex().debug_selector(|| "composer-model-list".into()).p(px(6.)).overflow_hidden();
    if let Some(agent) = &shown {
        let mono = cx.theme().mono_font_family.clone();
        let rows = model_rows(c, agent, cx);
        let searching = !c.search.read(cx).value().trim().is_empty();
        let open = legacy_open(c, agent) && !searching;
        for (ix, model) in rows.iter().enumerate() {
            if ix == agent.models.len() && !agent.legacy_models.is_empty() && !searching {
                list = list.child(legacy_fold(open, &p, cx));
            }
            let selected =
                r.agent.as_ref().is_some_and(|a| a.harness == agent.harness) && r.model.as_ref().is_some_and(|m| m.value == model.value);
            let active = c.popup_active == Some(ix);
            let value = model.value.clone();
            list = list.child(
                h_flex()
                    .id(SharedString::from(format!("model-{}", model.value)))
                    .gap_3()
                    .px_3()
                    .py(px(10.))
                    .rounded(px(10.))
                    .text_color(p.foreground)
                    .when(selected, |el| el.bg(p.foreground.opacity(0.08)))
                    .when(active, |el| el.bg(p.accent))
                    .hover(|s| s.bg(p.accent))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| this.pick(Popup::Model, value.clone(), window, cx)))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .child(div().truncate().text_size(px(15.)).line_height(px(20.)).child(model.label.clone()))
                                    .when(model.is_default == Some(true), |el| {
                                        el.child(
                                            div().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child("default"),
                                        )
                                    }),
                            )
                            .child(
                                h_flex()
                                    .mt_1()
                                    .gap(px(6.))
                                    .text_size(px(13.))
                                    .line_height(px(18.))
                                    .text_color(p.muted_foreground)
                                    .child(agent_mark(&agent.harness, &agent.label, px(14.), cx))
                                    .child(agent.label.clone()),
                            ),
                    )
                    .when(ix < 9, |el| {
                        let key = if cfg!(target_os = "macos") { format!("⌘{}", ix + 1) } else { format!("Ctrl+{}", ix + 1) };
                        el.child(
                            h_flex()
                                .h(px(24.))
                                .px(px(6.))
                                .rounded(px(8.))
                                .bg(p.muted)
                                .font_family(mono.clone())
                                .text_size(px(12.))
                                .text_color(p.muted_foreground)
                                .child(key),
                        )
                    })
                    .child(
                        div()
                            .p_1()
                            .opacity(0.6)
                            .child(Icon::new(Glyph::Star).with_size(px(16.)).text_color(p.muted_foreground.opacity(0.6))),
                    ),
            );
        }
        if rows.len() == agent.models.len() && !agent.legacy_models.is_empty() && !searching {
            list = list.child(legacy_fold(open, &p, cx));
        }
        if rows.is_empty() {
            let note = if searching { "No model matches".to_owned() } else { format!("{} reported no models", agent.label) };
            list = list.child(div().px_2().py_3().text_center().text_size(px(12.)).text_color(p.muted_foreground).child(note));
        }
    }
    popover_surface(&p, cx)
        .w(px(400.))
        .max_h(px(416.))
        .child(h_flex().items_stretch().child(rail).child(v_flex().flex_1().min_w_0().child(search).child(list)))
        .into_any_element()
}

/// The fold over an agent's legacy models.
fn legacy_fold(open: bool, p: &Palette, cx: &mut Context<Composer>) -> AnyElement {
    h_flex()
        .id("model-legacy")
        .gap_3()
        .px_3()
        .py(px(10.))
        .rounded(px(10.))
        .text_size(px(15.))
        .line_height(px(20.))
        .text_color(p.foreground)
        .cursor_pointer()
        .hover(|s| s.bg(p.accent))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.legacy_open = Some(!open);
            cx.notify();
        }))
        .child(div().flex_1().child("Legacy models"))
        .child(
            Icon::new(if open { Glyph::ChevronDown } else { Glyph::ChevronRight })
                .with_size(px(16.))
                .text_color(p.muted_foreground.opacity(0.6)),
        )
        .into_any_element()
}

/// The command or mention menu, or the stash, on the box's top edge.
pub fn menu_layer(c: &Composer, cx: &mut Context<Composer>) -> Option<AnyElement> {
    let p = cx.palette().clone();
    let body = if let Some(menu) = &c.menu {
        command_menu(menu, &p, cx)
    } else if c.popup == Some(Popup::Stash) && !c.stash.entries.is_empty() {
        stash_menu(c, &p, cx)
    } else {
        return None;
    };
    Some(
        div()
            .absolute()
            .left(INSET)
            .right(INSET)
            .bottom_full()
            .child(deferred(attached_surface(true, cx).child(body)).with_priority(1))
            .into_any_element(),
    )
}

fn command_menu(menu: &super::Menu, p: &Palette, cx: &mut Context<Composer>) -> AnyElement {
    let mut list =
        v_flex().id("composer-command-menu").debug_selector(|| "composer-command-menu".into()).max_h(px(288.)).p_2().overflow_y_scroll();
    if menu.items.is_empty() {
        let note = match menu.trigger {
            super::model::Trigger::Mention(_) => "No file matches",
            super::model::Trigger::Slash(_) => "No command matches",
        };
        return list
            .child(div().px_3().py_2().text_size(px(12.)).line_height(px(16.)).text_color(p.muted_foreground).child(note))
            .into_any_element();
    }
    let mut group: Option<&str> = None;
    for (ix, item) in menu.items.iter().enumerate() {
        if group != Some(item.group.as_str()) {
            list = list.child(
                div()
                    .px_3()
                    .py(px(6.))
                    .when(group.is_some(), |el| el.mt(px(6.)))
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(p.muted_foreground)
                    .child(item.group.clone()),
            );
            group = Some(item.group.as_str());
        }
        let active = ix == menu.active;
        list = list.child(
            h_flex()
                .id(("menu-item", ix))
                .gap_3()
                .px_3()
                .py_2()
                .rounded(px(10.))
                .text_size(px(12.))
                .line_height(px(16.))
                .cursor_pointer()
                .when(active, |el| el.bg(p.accent).text_color(p.foreground))
                .on_click(cx.listener(move |this, _, window, cx| this.take_menu(Some(ix), window, cx)))
                .child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_2()
                        .when(item.file, |el| el.child(Icon::new(Glyph::File).with_size(px(14.)).text_color(p.muted_foreground)))
                        .child(
                            div()
                                .min_w_0()
                                .truncate()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(p.foreground)
                                .when(item.note.is_some(), |el| el.max_w(gpui_kit::relative(0.45)).flex_none())
                                .child(item.label.clone()),
                        )
                        .children(
                            item.note.clone().map(|note| div().flex_1().min_w_0().truncate().text_color(p.muted_foreground).child(note)),
                        ),
                ),
        );
    }
    list.into_any_element()
}

fn stash_menu(c: &Composer, p: &Palette, cx: &mut Context<Composer>) -> AnyElement {
    let mono = cx.theme().mono_font_family.clone();
    let head = h_flex()
        .id("stash-head")
        .min_h(px(24.))
        .gap_1()
        .cursor_pointer()
        .on_click(cx.listener(|this, _, _, cx| this.toggle_popup(Popup::Stash, cx)))
        .child(
            h_flex()
                .w(px(24.))
                .flex_none()
                .justify_center()
                .child(Icon::new(Glyph::Bookmark).with_size(px(12.)).text_color(p.muted_foreground)),
        )
        .child(
            div()
                .flex_1()
                .font_family(mono.clone())
                .text_size(px(11.))
                .text_color(p.muted_foreground)
                .child(format!("Stashed {}", c.stash.entries.len())),
        )
        .child(
            h_flex().size(px(24.)).justify_center().child(Icon::new(Glyph::ChevronDown).with_size(px(14.)).text_color(p.muted_foreground)),
        );
    let mut list = v_flex().gap(px(1.));
    for (ix, entry) in c.stash.entries.iter().enumerate() {
        let active = c.popup_active == Some(ix);
        let files = entry.files.len();
        list = list.child(
            h_flex()
                .id(("stash-row", ix))
                .min_h(px(24.))
                .gap_1()
                .rounded(px(6.))
                .when(active, |el| el.bg(p.accent))
                .child(
                    h_flex()
                        .w(px(24.))
                        .flex_none()
                        .justify_center()
                        .child(Icon::new(Glyph::FileText).with_size(px(12.)).text_color(p.muted_foreground)),
                )
                .child(
                    div()
                        .id(("stash-take", ix))
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .cursor_pointer()
                        .text_color(p.foreground.opacity(0.8))
                        .on_click(cx.listener(move |this, _, window, cx| this.pick(Popup::Stash, ix.to_string(), window, cx)))
                        .child(Stash::snippet(entry)),
                )
                .when(files > 0, |el| {
                    el.child(div().font_family(mono.clone()).text_size(px(11.)).text_color(p.muted_foreground).child(if files == 1 {
                        "1 file".to_owned()
                    } else {
                        format!("{files} files")
                    }))
                })
                .child(
                    div()
                        .font_family(mono.clone())
                        .text_size(px(11.))
                        .text_color(p.muted_foreground)
                        .child(super::model::ago(entry.at.elapsed())),
                )
                .child(
                    h_flex()
                        .id(("stash-delete", ix))
                        .size(px(24.))
                        .justify_center()
                        .rounded(px(8.))
                        .text_color(p.muted_foreground)
                        .cursor_pointer()
                        .hover(|s| s.bg(p.accent).text_color(p.foreground))
                        .on_click(cx.listener(move |this, _, _, cx| this.delete_stashed(ix, cx)))
                        .child(Icon::new(Glyph::X).with_size(px(14.))),
                ),
        );
    }
    v_flex()
        .debug_selector(|| "composer-stash-menu".into())
        .px_1()
        .py_1()
        .text_size(px(12.))
        .line_height(px(16.))
        .child(head)
        .child(list)
        .into_any_element()
}
