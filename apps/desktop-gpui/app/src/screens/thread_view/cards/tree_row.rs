// SPDX-License-Identifier: AGPL-3.0-only
//! A child's row in the Threads section (`ThreadRow`, `SubagentRow`): its second line, message field, card and acts.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, FontWeight, Pixels, SharedString, Window};

use super::spawn::{go, listed_status};
use super::subagent_fold::subagent_route;
use super::threads::{on_sidebar, restore_child, settle_children, stop_child, stop_task};
use super::{SM_TIGHT, XS};
use crate::screens::Route;
use crate::thread_view::ThreadView;
use crate::transcript::{ChildPart, Listed};
use crate::ui::{catalog, tip, ActivePalette, Button, Glyph, Palette, StatusMark};

/// How long Stop stands armed after its first press (`ARM_MS`).
const STOP_ARMED: std::time::Duration = std::time::Duration::from_millis(2_000);

/// A child's row (`ThreadRow`, `SubagentRow`): the mark, the title over the computer and the note, then the status slot,
/// which yields on hover to Send a message and its state act, then More; a press opens the thread, or a subagent's
/// page. While a message is typed its field stands in the second line's place. `inert` draws it for the shut
/// section's peek, which takes no act of its own.
pub(super) fn tree_row(node: &Listed, inert: bool, p: &Palette, window: &mut Window, cx: &mut Context<ThreadView>) -> AnyElement {
    let id = match &node.subagent {
        Some(call) => format!("tree-sub-{}-{call}", node.at.thread),
        None => format!("tree-row-{}", node.at.thread),
    };
    let group = SharedString::from(format!("{id}-group"));
    let settled = node.part == ChildPart::Settled;
    let message = (!inert && node.subagent.is_none())
        .then(|| window.use_keyed_state(SharedString::from(format!("{id}-sending")), cx, |_, _| None::<Sending>));
    let sending = message.as_ref().and_then(|m| m.read(cx).as_ref().map(|s| s.input.clone()));
    let two = sending.is_some() || node.note.is_some() || node.computer.is_some();
    let mark = match &node.subagent {
        Some(_) => Icon::new(Glyph::Bot).with_size(px(13.)).text_color(p.muted_foreground).into_any_element(),
        None => catalog::mark(&node.harness, &catalog::name(&node.harness), px(13.), p.muted_foreground, cx),
    };
    let second = match &sending {
        Some(input) => Some(send_field(&id, input, message.clone(), p).into_any_element()),
        None if two => Some(second_line(node, &id, p, cx).into_any_element()),
        None => None,
    };
    let status = StatusMark::new(listed_status(node)).key(SharedString::from(format!("{id}-crab")));
    let send = message.clone().map(|m| open_send(node, m));
    let acts = if inert || sending.is_some() { (Vec::new(), 0) } else { tree_acts(node, &id, send.clone(), p, window, cx) };
    let menu = if inert { Vec::new() } else { child_menu(node, send) };
    let (acts, count) = acts;
    let slot = if acts.is_empty() {
        div().flex_none().child(status).into_any_element()
    } else {
        // `ACTS_ROOM`: as wide as the acts it holds with 4 px between, so the title's width never moves as they show.
        let room = [0., 24., 52., 80.][count.min(3)];
        h_flex()
            .relative()
            .flex_none()
            .justify_end()
            .min_w(px(room))
            .child(div().flex().group_hover(group.clone(), |s| s.invisible()).child(status))
            .child(
                h_flex()
                    .debug_selector(|| "child-acts".into())
                    // A press on an act is not a press on the row.
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .right_0()
                    .gap(px(4.))
                    .opacity(0.)
                    .group_hover(group.clone(), |s| s.opacity(1.))
                    .children(acts),
            )
            .into_any_element()
    };
    let press: Option<Route> = match &node.subagent {
        _ if inert => None,
        None => Some(Route::Thread { workspace_id: node.at.workspace.clone(), thread_id: node.at.thread.clone() }),
        Some(call) => Some(Route::Screen(subagent_route(&node.at, call).into())),
    };
    let ink = if settled { p.muted_foreground } else { p.foreground };
    let title_id = SharedString::from(format!("{id}-title"));
    let title_selector = format!("{id}-title");
    // `ThreadLink` and the subagent's own link: underlined under the pointer where a press opens something.
    let title = div()
        .id(title_id)
        .debug_selector(move || title_selector.clone())
        .min_w_0()
        .truncate()
        .text_color(ink)
        .when(press.is_some(), |el| el.hover(|s| s.underline()))
        .child(node.title.clone());
    let title = match &node.subagent {
        Some(_) if !inert => {
            let node = node.clone();
            title.tooltip_show_delay(CARD_DELAY).tooltip(move |window, cx| {
                let node = node.clone();
                crate::ui::tooltip_view(move |cx| subagent_card(&node, &cx.palette().clone(), cx), window, cx)
            })
        }
        _ => title,
    };
    let accent = p.accent;
    let selector = id.clone();
    let armed = window.use_keyed_state(SharedString::from(format!("{id}-armed")), cx, |_, _| None::<std::time::Instant>);
    h_flex()
        .id(SharedString::from(id))
        .group(group)
        .debug_selector(move || selector.clone())
        .min_w_0()
        .gap(px(10.))
        .px(px(8.))
        .rounded(px(8.))
        .text_size(SM_TIGHT.0)
        .line_height(SM_TIGHT.1)
        .when(!inert, |el| el.hover(move |s| s.bg(accent)))
        .map(|el| if two { el.min_h(px(48.)).py(px(6.)) } else { el.h(px(36.)) })
        .child(div().flex().flex_none().items_center().justify_center().size(px(13.)).child(mark))
        .child(v_flex().flex_1().min_w(px(112.)).child(title).children(second))
        .child(slot)
        // The pointer leaving the row disarms its Stop.
        .on_hover(move |over, _, cx| {
            if !*over {
                armed.update(cx, |a, cx| {
                    if a.take().is_some() {
                        cx.notify();
                    }
                });
            }
        })
        .when_some(press.filter(|_| sending.is_none()), |el, route| el.cursor_pointer().on_click(move |_, _, cx| go(route.clone(), cx)))
        .when(!menu.is_empty(), |el| {
            let (view, menu) = (cx.weak_entity(), Rc::new(menu));
            el.on_mouse_down(gpui_kit::MouseButton::Right, move |e, window, cx| {
                open_child_menu(&view, menu.clone(), e.position, window, cx)
            })
        })
        .into_any_element()
}

/// What opens a child's message field, from its act, its menu or its More.
type Opens = Rc<dyn Fn(&mut Window, &mut App)>;

/// `CARD_DELAY_MS`: how long the pointer rests on a subagent's title before its card opens.
const CARD_DELAY: std::time::Duration = std::time::Duration::from_millis(450);

/// A message being typed to a child in its row (`RowNameInput` with `messageTo`), and what its field hears.
pub(super) struct Sending {
    input: gpui_kit::Entity<gpui_kit::component::input::InputState>,
    _events: gpui_kit::Subscription,
}

/// The second line (`SecondLine`): the computer it runs on with its glyph, where that is not the lead's, then the note.
fn second_line(node: &Listed, id: &str, p: &Palette, cx: &App) -> gpui_kit::Div {
    let computer = node.computer.clone().map(|name| {
        let glyph = crate::screens::terminal::workspaces_sidebar(cx).and_then(|s| {
            let s = s.read(cx);
            let place = s.around().places.iter().find(|pl| crate::screens::sidebar::looks::place_name(pl) == name)?;
            let glyph = crate::screens::sidebar::looks::computer_glyph(place, s.preferences(), px(12.), p.muted_foreground);
            let selector = format!("{id}-computer-glyph");
            Some(div().debug_selector(move || selector.clone()).flex_none().child(glyph))
        });
        let selector = format!("{id}-computer");
        h_flex()
            .id(SharedString::from(selector.clone()))
            .debug_selector(move || selector.clone())
            .flex_none()
            .gap(px(4.))
            .tooltip(tip(format!("Runs on {name}")))
            .children(glyph)
            .child(name)
    });
    h_flex()
        .min_w_0()
        .gap(px(12.))
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(p.muted_foreground)
        .children(computer)
        .children(node.note.clone().map(|note| div().min_w_0().truncate().child(note)))
}

/// The one-line message field in the second line's place: `h-4.5 text-xs`, `rounded-sm border px-1`, the ring's edge
/// while it has the keys; Escape shuts it with nothing sent.
fn send_field(
    id: &str,
    input: &gpui_kit::Entity<gpui_kit::component::input::InputState>,
    message: Option<gpui_kit::Entity<Option<Sending>>>,
    p: &Palette,
) -> gpui_kit::Div {
    let selector = format!("{id}-send-field");
    div().flex().h(px(18.)).min_w_0().text_size(XS.0).child(
        div()
            .debug_selector(move || selector.clone())
            .flex()
            .flex_1()
            .min_w_0()
            .items_center()
            .rounded(px(6.))
            .border_1()
            .border_color(p.ring)
            .px(px(4.))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .capture_key_down(move |e: &gpui_kit::KeyDownEvent, _, cx| {
                if e.keystroke.key == "escape" {
                    if let Some(message) = &message {
                        message.update(cx, |m, cx| {
                            *m = None;
                            cx.notify();
                        });
                    }
                    cx.stop_propagation();
                }
            })
            .child(gpui_kit::component::input::Input::new(input).appearance(false).p_0().h(px(16.)).text_size(XS.0).line_height(px(16.))),
    )
}

/// Opens a child's message field, focused: Enter sends what it holds to the child as the composer would
/// (`sendToThread`, `sessions.start` through the sidebar's runner, which says a refusal), a blank Enter or leaving it
/// shuts it with nothing sent.
fn open_send(node: &Listed, message: gpui_kit::Entity<Option<Sending>>) -> Opens {
    let (workspace, thread, harness) = (node.at.workspace.clone(), node.at.thread.clone(), node.harness.clone());
    Rc::new(move |window, cx| {
        use gpui_kit::component::input::{InputEvent, InputState};
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("Send a message"));
        let (workspace, thread, harness, held) = (workspace.clone(), thread.clone(), harness.clone(), message.downgrade());
        let events = window.subscribe(&input, cx, move |input, event: &InputEvent, _, cx| {
            let typed = match event {
                InputEvent::PressEnter { .. } => input.read(cx).value().trim().to_owned(),
                InputEvent::Blur => String::new(),
                _ => return,
            };
            _ = held.update(cx, |m, cx| {
                *m = None;
                cx.notify();
            });
            if typed.is_empty() {
                return;
            }
            let params = serde_json::json!({
                "workspaceId": workspace, "thread": thread, "harness": harness, "prompt": typed,
                "requestId": crate::screens::composer::link::request_id(),
            });
            on_sidebar(format!("send {workspace} {thread} {harness} {typed}"), cx, |s, cx| {
                s.write_op::<wsp_host_client::protocol::SessionsStartRequest>(params, cx)
            });
        });
        input.update(cx, |state, cx| state.focus(window, cx));
        message.update(cx, |m, cx| {
            *m = Some(Sending { input, _events: events });
            cx.notify();
        });
    })
}

/// A subagent's card (`SubagentCard`): its title, the status row with why, what it was asked under the message glyph,
/// then its agent's mark and model.
fn subagent_card(node: &Listed, p: &Palette, cx: &App) -> AnyElement {
    use crate::screens::sidebar::model;
    let model = node
        .model
        .as_deref()
        .map(|m| subagent_model(m, &node.harness, &node.at.workspace, cx))
        .unwrap_or_else(|| catalog::name(&node.harness));
    let line = |glyph: AnyElement, text: String, clamp: bool| {
        h_flex()
            .items_start()
            .gap(px(8.))
            .min_w_0()
            .child(div().flex_none().mt(px(3.)).child(glyph))
            .child(div().min_w_0().when(clamp, |el| el.line_clamp(3)).child(text))
    };
    let (glyph, tone) = model::kind_slot(node.kind);
    let key = format!("card-crab-{}-{}", node.at.thread, node.subagent.as_deref().unwrap_or(""));
    let status = crate::screens::sidebar::card::status_row(
        node.kind,
        glyph,
        tone,
        key,
        model::kind_word(node.kind).to_owned(),
        node.slot.clone(),
        p,
    );
    let reason = node.note.clone().map(|r| div().pl(px(20.)).mt(px(2.)).line_clamp(3).child(r));
    let asked = node.asked.clone().map(|a| line(Icon::new(Glyph::MessageSquareText).with_size(px(12.)).into_any_element(), a, true));
    let agent = line(catalog::mark(&node.harness, &catalog::name(&node.harness), px(12.), p.muted_foreground, cx), model, false);
    v_flex()
        .debug_selector(|| "subagent-card".into())
        .max_w(px(288.))
        .py(px(4.))
        .gap(px(6.))
        .whitespace_normal()
        .child(div().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(node.title.clone()))
        .child(
            v_flex().gap(px(4.)).text_color(p.muted_foreground).child(v_flex().child(status).children(reason)).children(asked).child(agent),
        )
        .into_any_element()
}

/// `useSubagentModel`: the model by its agent's catalog label on that workspace, the id where the catalog has none.
fn subagent_model(model: &str, harness: &str, workspace: &str, cx: &App) -> String {
    let picked = model.split(':').next().unwrap_or(model);
    crate::screens::terminal::workspaces_sidebar(cx)
        .and_then(|s| {
            let s = s.read(cx);
            let catalog = s.around().catalogs.get(workspace)?.iter().find(|c| c.harness == harness)?;
            catalog.models.iter().find(|(v, _)| v == picked).map(|(_, label)| label.clone())
        })
        .unwrap_or_else(|| model.to_owned())
}

/// A Stop that takes two presses (`StopAct`): the square glyph, then "Stop?" in the danger ink for two seconds, when a
/// second press runs `stop`.
fn stop_act(
    id: &str,
    label: &'static str,
    armed: &gpui_kit::Entity<Option<std::time::Instant>>,
    stop: Rc<dyn Fn(&mut App)>,
    p: &Palette,
    cx: &App,
) -> AnyElement {
    use crate::screens::sidebar::menu::act_words as words;
    let is_armed = armed.read(cx).is_some_and(|at| at.elapsed() < STOP_ARMED);
    let armed = armed.clone();
    let press = move |_: &gpui_kit::ClickEvent, _: &mut Window, cx: &mut App| {
        cx.stop_propagation();
        let was = armed.read(cx).is_some_and(|at| at.elapsed() < STOP_ARMED);
        if was {
            stop(cx);
        }
        let at = (!was).then(std::time::Instant::now);
        armed.update(cx, |a, cx| {
            *a = at;
            cx.notify();
        });
        if let Some(at) = at {
            let armed = armed.downgrade();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(STOP_ARMED).await;
                // Only the arm this press made lapses; a later arm keeps its own two seconds.
                _ = armed.update(cx, |a, cx| {
                    if *a == Some(at) {
                        *a = None;
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    };
    let selector = format!("{id}-act-stop");
    if is_armed {
        h_flex()
            .id(SharedString::from(format!("{id}-stop")))
            .debug_selector(move || selector.clone())
            .h(px(24.))
            .px(px(6.))
            .rounded(px(8.))
            .text_size(XS.0)
            .line_height(XS.1)
            .font_weight(FontWeight::MEDIUM)
            .text_color(p.error_foreground)
            .cursor_pointer()
            .tooltip(tip(words::STOP_AGAIN))
            .child(words::STOP_ARMED)
            .on_click(press)
            .into_any_element()
    } else {
        div()
            .debug_selector(move || selector.clone())
            .child(Button::icon_xs(SharedString::from(format!("{id}-stop")), Glyph::Square).tooltip(label).on_click(press))
            .into_any_element()
    }
}

/// The acts a row yields its slot to (`Slot` in ThreadRows.tsx), and whether one stops: a thread's Send a message and
/// the state act that can run now (Stop in two presses, Settle or Restore), then More with every act the row takes; a
/// running subagent's Stop subagent alone. While Stop is armed the others give way to its word.
fn tree_acts(
    node: &Listed,
    id: &str,
    send: Option<Opens>,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> (Vec<AnyElement>, usize) {
    use crate::screens::sidebar::menu::{act_words as words, settle_refusal};
    let armed = window.use_keyed_state(SharedString::from(format!("{id}-armed")), cx, |_, _| None::<std::time::Instant>);
    let is_armed = armed.read(cx).is_some_and(|at| at.elapsed() < STOP_ARMED);
    if node.subagent.is_some() {
        let (Some(task), true) = (node.task.clone(), node.running) else { return (Vec::new(), 0) };
        let session = node.session.clone();
        return (vec![stop_act(id, SUBAGENT_STOP, &armed, Rc::new(move |cx| stop_task(&session, &task, cx)), p, cx)], 2);
    }
    let settled = node.part == ChildPart::Settled;
    let stops = !settled && (node.running || node.working);
    let settles = !settled && !node.running;
    let held = settle_refusal(node.working);
    let mut out: Vec<AnyElement> = Vec::new();
    let more_send = send.clone();
    if let Some(send) = send.filter(|_| !is_armed) {
        let selector = format!("{id}-act-send");
        out.push(
            div()
                .debug_selector(move || selector.clone())
                .child(Button::icon_xs(SharedString::from(format!("{id}-send")), Glyph::MessageSquare).tooltip(SEND_WORD).on_click(
                    move |_, window, cx| {
                        cx.stop_propagation();
                        send(window, cx)
                    },
                ))
                .into_any_element(),
        );
    }
    let (session, thread) = (node.session.clone(), node.at.thread.clone());
    if stops {
        let label = if node.under { words::STOP_TREE } else { words::STOP };
        out.push(stop_act(id, label, &armed, Rc::new(move |cx| stop_child(&session, cx)), p, cx));
    } else if settles && held.is_none() {
        out.push(
            div()
                .debug_selector({
                    let id = format!("{id}-act-settle");
                    move || id.clone()
                })
                .child(Button::icon_xs(SharedString::from(format!("{id}-settle")), Glyph::Archive).tooltip(words::SETTLE).on_click(
                    move |_, _, cx| {
                        cx.stop_propagation();
                        settle_children(vec![thread.clone()], cx)
                    },
                ))
                .into_any_element(),
        );
    } else if settled {
        out.push(
            div()
                .debug_selector({
                    let id = format!("{id}-act-restore");
                    move || id.clone()
                })
                .child(
                    Button::icon_xs(SharedString::from(format!("{id}-restore")), Glyph::ArchiveRestore).tooltip(words::RESTORE).on_click(
                        move |_, _, cx| {
                            cx.stop_propagation();
                            restore_child(thread.clone(), cx)
                        },
                    ),
                )
                .into_any_element(),
        );
    }
    // `ACTS_ROOM` counts what the slot shows unarmed, Send, the state act and More, at least two where one stops.
    let shown = usize::from(more_send.is_some()) + usize::from(stops || (settles && held.is_none()) || settled) + 1;
    let room = if stops { shown.max(2) } else { shown };
    if is_armed {
        return (out, room);
    }
    use gpui_kit::component::menu::DropdownMenu as _;
    let acts = Rc::new(child_menu(node, more_send));
    let trigger = crate::ui::menu::Trigger(
        div()
            .id(SharedString::from(format!("{id}-more-menu")))
            .debug_selector({
                let id = format!("{id}-act-more");
                move || id.clone()
            })
            .flex()
            .child(Button::icon_xs(SharedString::from(format!("{id}-more")), Glyph::Ellipsis).tooltip("More")),
    );
    out.push(
        trigger
            .dropdown_menu(move |menu, _, cx| {
                let p = cx.palette();
                crate::ui::menu_of(menu.surface(crate::ui::menu::surface(p)), &acts, p.error_foreground, p.muted_foreground)
            })
            .into_any_element(),
    );
    (out, room)
}

/// `CHILD_WORDS.send` and `CHILD_WORDS.stopSubagent`.
const SEND_WORD: &str = "Send a message";
const SUBAGENT_STOP: &str = "Stop subagent";

/// Every act a row takes, in the order its More and its right-click list them (`childActions`): a thread's Send a
/// message, which opens its field where `send` is given, and its state acts; a running subagent's Stop subagent.
fn child_menu(node: &Listed, send: Option<Opens>) -> Vec<crate::ui::MenuAct> {
    use crate::screens::sidebar::menu::{act_words as words, settle_refusal};
    let act = |group: u8, glyph: Glyph, title: &str, why: Option<String>, run: Opens| crate::ui::MenuAct {
        group,
        glyph: Some(glyph),
        title: SharedString::from(title.to_owned()),
        destructive: false,
        held: why.is_some(),
        why: why.map(SharedString::from),
        keys: None,
        run: Rc::new(move |window, cx| run(window, cx)),
    };
    if node.subagent.is_some() {
        let (Some(task), true) = (node.task.clone(), node.running) else { return Vec::new() };
        let session = node.session.clone();
        return vec![act(1, Glyph::Square, SUBAGENT_STOP, None, Rc::new(move |_, cx| stop_task(&session, &task, cx)))];
    }
    let settled = node.part == ChildPart::Settled;
    let (stops, settles) = (!settled && (node.running || node.working), !settled && !node.running);
    let mut menu: Vec<crate::ui::MenuAct> = send.map(|send| act(0, Glyph::MessageSquare, SEND_WORD, None, send)).into_iter().collect();
    let (session, thread) = (node.session.clone(), node.at.thread.clone());
    if stops {
        menu.push(act(1, Glyph::Square, words::STOP, None, Rc::new(move |_, cx| stop_child(&session, cx))));
    }
    if settles {
        let ids = vec![thread.clone()];
        menu.push(act(
            1,
            Glyph::Archive,
            words::SETTLE,
            settle_refusal(node.working),
            Rc::new(move |_, cx| settle_children(ids.clone(), cx)),
        ));
    }
    if settled {
        menu.push(act(1, Glyph::ArchiveRestore, words::RESTORE, None, Rc::new(move |_, cx| restore_child(thread.clone(), cx))));
    }
    menu
}

/// Opens a row's acts at the pointer, as its right-click does (`openContextMenu`).
fn open_child_menu(
    view: &gpui_kit::WeakEntity<ThreadView>,
    acts: Rc<Vec<crate::ui::MenuAct>>,
    at: gpui_kit::Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    let items = acts
        .iter()
        .map(|a| crate::ui::context_menu::MenuItem {
            label: a.title.clone(),
            group: if a.group == 0 { "talk" } else { "state" },
            refusal: a.why.clone(),
            keys: None,
            destructive: a.destructive,
        })
        .collect();
    _ = view.update(cx, |v, cx| {
        v.row_menu.open(at, items, window, cx, move |_, pick, window, cx| {
            if let Some(act) = pick.and_then(|ix| acts.get(ix)) {
                (act.run)(window, cx);
            }
        })
    });
}
