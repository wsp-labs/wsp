// SPDX-License-Identifier: AGPL-3.0-only
//! Remove at the foot of a computer's page and its one confirmation, after `RemoveLine` in `computers.tsx` and
//! `RemoveComputerDialog.tsx`: the sentence computed from what the computer holds, places.holds read as the dialog
//! opens with the confirm held until it lands, then places.remove; a refusal stands under the sentence with the
//! sudo field where the login asks a password, and a cloud that refused its key turns to its Forget. A remove the
//! host made with something still to say keeps the dialog, and the page under it, until Done.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use gpui_kit::component::input::InputState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Entity, Window};
use serde_json::json;
use wsp_host_client::ops::sidebar::{Sessions, Workspaces};
use wsp_host_client::protocol::{
    PlaceHolds, PlaceRemoved, PlaceView, PlacesHoldsRequest, PlacesRemoveRequest, SessionView, SessionsListRequest, WorkspaceOut,
    WorkspacesListRequest,
};
use wsp_host_client::{Failure, Op};

use super::words::{
    add_words, forget_sentence, forget_title, forgets_only, held_on, here_name, is_offline, is_provider, names_place, place_name,
    provider_forget_sentence, remove_dialog_title, remove_sentence, removed_title, sub_held_line, unsaved_said, where_words, Holding,
    LEAVE_LINE, LEAVE_TAKES,
};
use crate::screens::settings::model::{req, At, GroupId, Reread};
use crate::screens::settings::page::{CardData, Ctx, DialogBuild, Item};
use crate::screens::settings::Acts;
use crate::ui::{size, text, ActivePalette, AlertDialog, Button, CopyRow, Field, Refusal};

/// `PLACE_SUDO_KIND`.
const SUDO_KIND: &str = "sudo";

/// One op on the host with the failure whole, its kind kept, for a dialog whose road turns on which refusal it got.
pub fn call_whole<O>(acts: &Acts, op: O, cx: &mut App, then: impl FnOnce(Result<O::Reply, Failure>, &mut App) + 'static)
where
    O: Op + Send + 'static,
    O::Reply: Send + 'static,
{
    let Some((host, runtime)) = acts.host(cx) else {
        then(Err(Failure::new("No host answered.")), cx);
        return;
    };
    let (tx, rx) = futures::channel::oneshot::channel();
    runtime.spawn(async move {
        _ = tx.send(host.call(op).await);
    });
    let acts = acts.clone();
    cx.spawn(async move |cx| {
        if let Ok(answer) = rx.await {
            cx.update(|cx| {
                then(answer, cx);
                acts.notify(cx);
            });
        }
    })
    .detach();
}

/// The place a page keeps drawing after the host dropped it, while its Remove dialog still has something to say, and
/// the token its dialog holds while it stands.
pub type Parting = Rc<RefCell<Option<(PlaceView, Weak<()>)>>>;

/// The parting place of the computer pages, kept as long as one is drawn.
pub fn parting(window: &mut Window, cx: &mut App) -> Parting {
    window.use_keyed_state("computer-parting", cx, |_, _| Parting::default()).read(cx).clone()
}

/// The place a page stands on: the host's row, else the one parting while its dialog still stands. A dialog closed
/// some other way than Done lets the parting go, and the frame falls back to the list, as the web's close does.
pub fn page_place(ctx: &Ctx, id: &str, parting: &Parting, cx: &mut App) -> Option<PlaceView> {
    if let Some(place) = ctx.data.places.iter().find(|p| p.id == id) {
        return Some(place.clone());
    }
    let held = parting.borrow().clone();
    let (place, alive) = held?;
    if place.id != id {
        return None;
    }
    if alive.upgrade().is_some() {
        return Some(place);
    }
    parting.borrow_mut().take();
    let acts = ctx.acts.clone();
    cx.defer(move |cx| acts.hold_parting(None, cx));
    None
}

/// What the dialog holds while it is open.
struct Asking {
    place: PlaceView,
    holds: Option<PlaceHolds>,
    /// The host's workspaces and sessions, read as the dialog opens.
    held: Option<(Vec<WorkspaceOut>, Vec<SessionView>)>,
    refusal: Option<(String, Option<String>)>,
    busy: bool,
    asks_sudo: bool,
    key_refused: bool,
    made_note: Option<String>,
    password: Entity<InputState>,
}

/// `RemoveLine`: a tall line with what Remove takes, its button opening the confirmation; under 640 px the words over
/// the button, which takes the line's width.
pub fn remove_line(ctx: &Ctx, place: &PlaceView, parting: &Parting, narrow: bool, cx: &App) -> CardData {
    let p = cx.palette().clone();
    let cloud = is_provider(place);
    let name = place_name(place);
    let note = if cloud {
        where_words::REMOVE_CLOUD_DESCRIPTION.to_owned()
    } else {
        where_words::remove_description(&name, &here_name(&ctx.data.places))
    };
    let title = where_words::remove_title(&name);
    let (acts, place_owned, parting) = (ctx.acts.clone(), place.clone(), parting.clone());
    let button = Button::danger("remove", where_words::REMOVE).xs().on_click(move |_, _, cx| open(&acts, &place_owned, &parting, cx));
    let words = v_flex()
        .min_w_0()
        .gap_1()
        .child(
            div()
                .text_size(text::SETTING_TITLE.0)
                .line_height(text::SETTING_TITLE.1)
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .child(title.clone()),
        )
        .when(!note.is_empty(), |el| {
            el.child(
                div()
                    .debug_selector(|| "remove-note".into())
                    .max_w(px(576.))
                    .text_size(text::DESCRIPTION.0)
                    .line_height(text::DESCRIPTION.1)
                    .text_color(p.muted_foreground)
                    .child(note.clone()),
            )
        });
    let line = if narrow { v_flex().gap_3() } else { h_flex().justify_between().gap_8() };
    let line = line
        .id("remove-line")
        .debug_selector(|| "remove-line".into())
        .px(size::CARD_INSET)
        .py(size::ROW_PAD_Y)
        .min_h(size::ROW_FLOOR)
        .child(words)
        .child(div().debug_selector(|| "remove-button".into()).flex().flex_col().child(button));
    CardData::new("remove").item(Item::any("remove-line", vec![title.into(), note.into()], crate::ui::CardItem::any(line)))
}

/// Opens the confirmation and reads what goes with the computer.
fn open(acts: &Acts, place: &PlaceView, parting: &Parting, cx: &mut App) {
    let id = place.id.clone();
    let state: Rc<RefCell<Option<Entity<Asking>>>> = Rc::default();
    let alive = Rc::new(());
    let (place, parting) = (place.clone(), parting.clone());
    acts.open_dialog(
        DialogBuild::alert(move |ctx, window, cx| {
            let asking = state.borrow().clone();
            let asking = match asking {
                Some(asking) => asking,
                None => {
                    let password = cx.new(|cx| InputState::new(window, cx).masked(true));
                    let asking = cx.new(|_| Asking {
                        place: place.clone(),
                        holds: None,
                        held: None,
                        refusal: None,
                        busy: false,
                        asks_sudo: false,
                        key_refused: false,
                        made_note: None,
                        password,
                    });
                    *state.borrow_mut() = Some(asking.clone());
                    read_holds(&ctx.acts, &id, &asking, cx);
                    read_held(&ctx.acts, &asking, cx);
                    asking
                }
            };
            dialog(ctx, &asking, &parting, &alive, cx)
        }),
        cx,
    );
}

fn read_holds(acts: &Acts, id: &str, asking: &Entity<Asking>, cx: &mut App) {
    let asking = asking.clone();
    call_whole(acts, req::<PlacesHoldsRequest>(json!({ "placeId": id })), cx, move |answer, cx| {
        asking.update(cx, |a, _| match answer {
            Ok(holds) => a.holds = Some(holds),
            Err(failure) => {
                if a.holds.is_none() {
                    a.refusal = Some((failure.message, None));
                }
            }
        })
    });
}

/// The workspaces and the sessions the host lists, which the sentence folds onto this computer; a list refused counts
/// nothing, as the web's store then holds none.
fn read_held(acts: &Acts, asking: &Entity<Asking>, cx: &mut App) {
    let (acts2, asking) = (acts.clone(), asking.clone());
    call_whole(acts, req::<WorkspacesListRequest>(json!({})), cx, move |workspaces, cx| {
        let workspaces = workspaces.map(|Workspaces { workspaces }| workspaces).unwrap_or_default();
        call_whole(&acts2, req::<SessionsListRequest>(json!({})), cx, move |sessions, cx| {
            let sessions = sessions.map(|Sessions { sessions }| sessions).unwrap_or_default();
            asking.update(cx, |a, _| a.held = Some((workspaces, sessions)));
        });
    });
}

/// What the computer holds, as the sentence reads it: the workspaces standing there and the projects recorded there.
fn holding_of(ctx: &Ctx, a: &Asking) -> Holding {
    let workspaces = a.held.as_ref().map(|(w, s)| held_on(&a.place, &ctx.data.places, w, s)).unwrap_or_default();
    let projects = ctx.data.projects.iter().filter(|p| p.computer == a.place.id).map(|p| p.name.clone()).collect();
    let images = a.holds.as_ref().filter(|h| !h.images.is_empty()).map(|h| h.images.iter().map(|i| (i.id.clone(), i.size_bytes)).collect());
    Holding { workspaces, projects, images }
}

/// The dialog itself, built again each frame from what it holds.
fn dialog(ctx: &Ctx, asking: &Entity<Asking>, parting: &Parting, alive: &Rc<()>, cx: &mut App) -> AnyElement {
    let a = asking.read(cx);
    let place = a.place.clone();
    let here = here_name(&ctx.data.places);
    let holding = holding_of(ctx, a);
    let unsaved: Vec<String> = a.holds.as_ref().map(|h| h.unsaved.clone()).unwrap_or_default();
    let forgets = a.key_refused || a.holds.as_ref().is_some_and(forgets_only);
    let reading = (a.holds.is_none() || a.held.is_none()) && a.refusal.is_none();
    let cloud = is_provider(&place);
    let image_bytes = if cloud {
        None
    } else {
        ctx.data
            .image
            .as_ref()
            .and_then(|v| v.copies.iter().find(|c| names_place(&place, &c.place)))
            .and_then(|c| c.size_bytes)
            .map(|b| b as f64)
    };
    let title = if a.made_note.is_some() {
        removed_title(&place, forgets)
    } else if forgets {
        forget_title(&place)
    } else {
        remove_dialog_title(&place)
    };
    let sentence = if a.key_refused {
        provider_forget_sentence(&place, &holding, &here)
    } else if let (true, Some(holds)) = (forgets, a.holds.as_ref()) {
        forget_sentence(&place, holds)
    } else {
        remove_sentence(&place, &holding, &here, image_bytes)
    };
    let (said, fix, quiet): (Option<String>, Option<String>, Option<String>) = if let Some(note) = &a.made_note {
        (None, None, Some(note.clone()))
    } else if let Some((said, fix)) = &a.refusal {
        (Some(said.clone()), fix.clone(), None)
    } else if reading {
        (None, None, Some(where_words::READING_HOLDS.to_owned()))
    } else if !unsaved.is_empty() {
        let note = if cloud {
            format!("{} {}", sub_held_line(&unsaved).unwrap_or_default(), where_words::UNSAVED_FIX_CLOUD)
        } else {
            format!("{}. {}", unsaved_said(&place_name(&place), &unsaved), where_words::UNSAVED_FIX)
        };
        (None, None, Some(note))
    } else {
        (None, None, None)
    };
    let p = cx.palette().clone();
    let password = a.password.clone();
    let typed = password.read(cx).value().to_string();
    let (asks_sudo, busy, made) = (a.asks_sudo, a.busy, a.made_note.is_some());
    let mut popup = AlertDialog::new(title).layered().sentence(sentence);
    if is_offline(&place) {
        popup = popup.child(
            v_flex()
                .gap_3()
                .child(CopyRow::new("leave-line", LEAVE_LINE))
                .child(div().text_size(text::NOTE.0).line_height(text::NOTE.1).text_color(p.muted_foreground).child(LEAVE_TAKES)),
        );
    }
    let mut slot = Refusal::new();
    if let Some(said) = said {
        slot = slot.said(said);
    }
    if let Some(fix) = fix {
        slot = slot.fix(fix);
    }
    if let Some(quiet) = quiet {
        slot = slot.quiet(quiet);
    }
    popup = popup.child(slot);
    if asks_sudo {
        popup = popup.child(Field::new(&password));
    }
    if made {
        let (acts, parting) = (ctx.acts.clone(), parting.clone());
        popup = popup.act(Button::outline("remove-done", where_words::DONE).neutral_ring().on_click(move |_, window, cx| {
            parting.borrow_mut().take();
            acts.hold_parting(None, cx);
            acts.close_dialog(cx);
            acts.go(At::Group(GroupId::Computers), window, cx);
        }));
    } else {
        let cancel = ctx.acts.clone();
        popup = popup
            .act(Button::outline("remove-cancel", where_words::CANCEL).neutral_ring().on_click(move |_, _, cx| cancel.close_dialog(cx)));
        let word = if busy {
            if forgets {
                where_words::FORGETTING
            } else {
                where_words::REMOVING
            }
        } else if forgets {
            where_words::FORGET
        } else if !unsaved.is_empty() {
            where_words::REMOVE_ANYWAY
        } else {
            where_words::REMOVE
        };
        let held = busy || reading || (asks_sudo && typed.is_empty());
        let (acts, asking, parting, alive) = (ctx.acts.clone(), asking.clone(), parting.clone(), Rc::downgrade(alive));
        let force = !unsaved.is_empty();
        popup = popup.act(
            Button::destructive("remove-confirm", word)
                .held(held)
                .on_click(move |_, window, cx| remove(&acts, &asking, &parting, alive.clone(), force, forgets, window, cx)),
        );
    }
    popup.into_any_element()
}

/// places.remove, and what its answer leaves on screen.
#[allow(clippy::too_many_arguments)]
fn remove(
    acts: &Acts,
    asking: &Entity<Asking>,
    parting: &Parting,
    alive: Weak<()>,
    force: bool,
    forget: bool,
    window: &mut Window,
    cx: &mut App,
) {
    let (place, typed, sudo) = {
        let a = asking.read(cx);
        (a.place.clone(), a.password.read(cx).value().to_string(), a.asks_sudo)
    };
    let password = asking.read(cx).password.clone();
    password.update(cx, |input, cx| input.set_value("", window, cx));
    asking.update(cx, |a, _| {
        a.busy = true;
        a.refusal = None;
    });
    // Held from the ask on: the host drops the place from the list before its answer lands.
    *parting.borrow_mut() = Some((place.clone(), alive));
    acts.hold_parting(Some(place.id.clone()), cx);
    let mut params = json!({ "placeId": place.id, "force": force, "forget": forget });
    if sudo && !typed.is_empty() {
        params["sudoPassword"] = json!(typed);
    }
    let (acts2, asking2, parting2) = (acts.clone(), asking.clone(), parting.clone());
    call_whole(acts, req::<PlacesRemoveRequest>(params), cx, move |answer: Result<PlaceRemoved, Failure>, cx| {
        asking2.update(cx, |a, _| a.busy = false);
        match answer {
            Ok(answer) if answer.removed && answer.note.is_some() => asking2.update(cx, |a, _| a.made_note = answer.note),
            Ok(answer) if !answer.removed => {
                parting2.borrow_mut().take();
                acts2.hold_parting(None, cx);
                let said = answer.note.unwrap_or_else(|| where_words::remove_unmade(&place_name(&place)));
                asking2.update(cx, |a, _| a.refusal = Some((said, None)));
            }
            Ok(_) => {
                parting2.borrow_mut().take();
                acts2.hold_parting(None, cx);
                acts2.close_dialog(cx);
                acts2.reread(Reread::Places, cx);
                acts2.go_later(At::Group(GroupId::Computers), cx);
            }
            Err(failure) => {
                parting2.borrow_mut().take();
                acts2.hold_parting(None, cx);
                let kind = failure.kind.as_deref();
                let sudo = kind == Some(SUDO_KIND);
                let refused_key = kind == Some("auth") && is_provider(&place);
                let fix = if sudo {
                    Some(add_words::SUDO_FIX.to_owned())
                } else if refused_key {
                    Some(where_words::key_refused_fix(&place_name(&place)))
                } else {
                    None
                };
                asking2.update(cx, |a, _| {
                    a.asks_sudo = sudo;
                    if refused_key {
                        a.key_refused = true;
                    }
                    a.refusal = Some((failure.message, fix));
                });
                // The link may have come or gone since the dialog opened, which moves the road the host takes.
                let asking3 = asking2.clone();
                call_whole(&acts2, req::<PlacesHoldsRequest>(json!({ "placeId": place.id })), cx, move |answer, cx| {
                    if let Ok(holds) = answer {
                        asking3.update(cx, |a, _| a.holds = Some(holds));
                    }
                });
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::screens::settings::door::{self, Door};

    struct Blank;

    impl Render for Blank {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[gpui_kit::test]
    fn a_failed_remove_keeps_its_refusal_past_the_holds_read_again(cx: &mut gpui_kit::TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        // The remove asks for a password, and the holds read again after it is refused as well, in other words.
        let door = Door::start(json!({
            "places.remove": { "error": "box1 asks for the password of me.", "kind": "sudo" },
            "places.holds": { "error": "The link to box1 is down." },
        }));
        let runtime = door::runtime();
        let acts = Acts::new(gpui_kit::WeakEntity::new_invalid(), Some(door.client(&runtime)), runtime);
        let (_, cx) = cx.add_window_view(|window, cx| gpui_kit::component::Root::new(cx.new(|_| Blank), window, cx));
        let place: PlaceView = serde_json::from_value(
            json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false, "road": { "ssh": "me@box1" } }),
        )
        .unwrap();
        let asking = cx.update(|window, cx| {
            let password = cx.new(|cx| InputState::new(window, cx).masked(true));
            let holds = None;
            cx.new(|_| Asking {
                place,
                holds,
                held: None,
                refusal: None,
                busy: false,
                asks_sudo: false,
                key_refused: false,
                made_note: None,
                password,
            })
        });
        let alive = Rc::new(());
        cx.update(|window, cx| remove(&acts, &asking, &Parting::default(), Rc::downgrade(&alive), false, false, window, cx));
        let deadline = Instant::now() + Duration::from_secs(20);
        while door.asked_of("places.holds").is_empty() {
            assert!(Instant::now() < deadline, "the holds were never read again");
            cx.run_until_parked();
            std::thread::sleep(Duration::from_millis(20));
        }
        std::thread::sleep(Duration::from_millis(300));
        cx.run_until_parked();
        let (refusal, sudo) = asking.read_with(cx, |a, _| (a.refusal.clone(), a.asks_sudo));
        assert!(sudo, "the password field stands");
        assert_eq!(
            refusal,
            Some(("box1 asks for the password of me.".to_owned(), Some(add_words::SUDO_FIX.to_owned()))),
            "the remove's refusal and its fix stand, not the holds' words"
        );
    }
}
