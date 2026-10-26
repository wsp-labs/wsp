// SPDX-License-Identifier: AGPL-3.0-only
//! The one hover card the app shows at a time, as Base UI's `TooltipProvider` keeps one tooltip open: a sidebar
//! tile's card, the message ruler's, the context ring's, a usage segment's tip, a chart's. Opening one closes the one
//! that stood, and a card the pointer opened closes once the pointer is off both its opener and the card.

use std::time::{Duration, Instant};

use gpui_kit::{
    canvas, prelude::*, App, Bounds, DispatchPhase, EntityId, Global, MouseExitEvent, MouseMoveEvent, Pixels, Point, SharedString, Task,
};

/// Base UI's `TooltipProvider` `timeout`: a card that closed this recently lets the next one open at once.
const NEXT_AT_ONCE: Duration = Duration::from_millis(400);

/// What opened a card.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum By {
    /// The pointer: the card closes once the pointer has been off its opener and the card this long.
    Pointer(Duration),
    /// The keyboard's focus: the pointer elsewhere leaves it standing until another card opens or it is closed.
    Focus,
}

/// The pointer, closing the card as soon as it leaves.
pub const POINTER: By = By::Pointer(Duration::ZERO);

struct Open {
    owner: EntityId,
    key: SharedString,
    by: By,
    opener: Option<Bounds<Pixels>>,
    card: Option<Bounds<Pixels>>,
    leaving: Option<Task<()>>,
    /// The mark the card's last drawing keeps, which goes the first frame the card is not drawn.
    drawn: Option<EntityId>,
}

/// Kept by a drawn card from one frame to the next, and let go the first frame it is not drawn.
struct Drawn;

impl Open {
    fn is(&self, owner: EntityId, key: &str) -> bool {
        self.owner == owner && self.key == key
    }

    fn holds(&self, at: Point<Pixels>) -> bool {
        [self.opener, self.card].into_iter().flatten().any(|b| b.contains(&at))
    }
}

/// The card that stands, if one does, and when the last one closed.
#[derive(Default)]
struct Slot {
    open: Option<Open>,
    closed_at: Option<Instant>,
}

impl Global for Slot {}

/// Opens `owner`'s card `key`, closing whichever card stood; the owner that held it is told to draw again.
pub fn open(owner: EntityId, key: impl Into<SharedString>, by: By, cx: &mut App) {
    let key = key.into();
    let slot = cx.default_global::<Slot>();
    if let Some(open) = slot.open.as_mut().filter(|o| o.is(owner, &key)) {
        open.by = by;
        open.leaving = None;
        return;
    }
    let was = slot.open.replace(Open { owner, key, by, opener: None, card: None, leaving: None, drawn: None });
    if let Some(was) = was {
        cx.notify(was.owner);
    }
    cx.notify(owner);
}

/// Closes `owner`'s card, only `key` when one is named.
pub fn close(owner: EntityId, key: Option<&str>, cx: &mut App) {
    let Some(slot) = cx.try_global::<Slot>() else { return };
    if slot.open.as_ref().is_some_and(|o| o.owner == owner && key.is_none_or(|k| o.key == k)) {
        shut(cx);
    }
}

fn shut(cx: &mut App) {
    let slot = cx.default_global::<Slot>();
    let Some(was) = slot.open.take() else { return };
    slot.closed_at = Some(Instant::now());
    cx.notify(was.owner);
}

/// Whether `owner`'s card `key` is the one that stands.
pub fn shows(owner: EntityId, key: &str, cx: &App) -> bool {
    cx.try_global::<Slot>().and_then(|s| s.open.as_ref()).is_some_and(|o| o.is(owner, key))
}

/// The key of `owner`'s card while one of its cards stands.
pub fn open_key(owner: EntityId, cx: &App) -> Option<SharedString> {
    cx.try_global::<Slot>().and_then(|s| s.open.as_ref()).filter(|o| o.owner == owner).map(|o| o.key.clone())
}

/// Whether a card stands or one closed under `NEXT_AT_ONCE` ago, so the next opens with no delay.
pub fn at_once(cx: &App) -> bool {
    cx.try_global::<Slot>().is_some_and(|s| s.open.is_some() || s.closed_at.is_some_and(|at| at.elapsed() < NEXT_AT_ONCE))
}

/// Laid over the opener while its card stands: keeps where the opener was drawn.
pub fn opener(owner: EntityId, key: impl Into<SharedString>) -> impl IntoElement {
    let key = key.into();
    canvas(move |bounds, _, cx| place(owner, &key, bounds, false, cx), |_, _, _, _| {}).absolute().inset_0()
}

/// Laid over the card: keeps where the card was drawn, and closes it once the pointer is off both it and its opener,
/// in the capture phase, so no listener under it can keep the pointer's moves from it.
pub fn card(owner: EntityId, key: impl Into<SharedString>) -> impl IntoElement {
    let key = key.into();
    let placed = key.clone();
    canvas(
        move |bounds, window, cx| {
            place(owner, &placed, bounds, true, cx);
            // A card the pointer opened and no frame draws any more, its opener gone from under it, no longer stands.
            let id = SharedString::from(format!("hover-card-{owner:?}-{placed}"));
            let mark = window.use_keyed_state(id, cx, |_, cx| {
                let mark = cx.entity_id();
                cx.on_release(move |_: &mut Drawn, cx| {
                    let slot = cx.default_global::<Slot>();
                    if slot.open.as_ref().is_some_and(|o| o.drawn == Some(mark) && o.by != By::Focus) {
                        slot.open = None;
                    }
                })
                .detach();
                Drawn
            });
            if let Some(open) = cx.default_global::<Slot>().open.as_mut().filter(|o| o.is(owner, &placed)) {
                open.drawn = Some(mark.entity_id());
            }
        },
        move |_, _, window, cx| {
            // Drawn again with the pointer gone from it, as when its opener came back under a pointer elsewhere.
            let at = window.mouse_position();
            let drawn = key.clone();
            window.defer(cx, move |_, cx| pointer_at(owner, &drawn, at, cx));
            let moved = key.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    pointer_at(owner, &moved, e.position, cx);
                }
            });
            window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                let slot = cx.try_global::<Slot>().and_then(|s| s.open.as_ref());
                if phase == DispatchPhase::Capture && slot.is_some_and(|o| o.is(owner, &key) && o.by != By::Focus) {
                    shut(cx);
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

fn place(owner: EntityId, key: &str, bounds: Bounds<Pixels>, card: bool, cx: &mut App) {
    if let Some(open) = cx.default_global::<Slot>().open.as_mut().filter(|o| o.is(owner, key)) {
        *if card { &mut open.card } else { &mut open.opener } = Some(bounds);
    }
}

/// The pointer at `at`: a card the pointer opened closes once it is off both the opener and the card, at once or
/// after the card's own wait, and stays when the pointer comes back within the wait.
fn pointer_at(owner: EntityId, key: &str, at: Point<Pixels>, cx: &mut App) {
    let Some(open) = cx.default_global::<Slot>().open.as_mut().filter(|o| o.is(owner, key)) else { return };
    let By::Pointer(wait) = open.by else { return };
    if open.holds(at) {
        open.leaving = None;
    } else if wait.is_zero() {
        shut(cx);
    } else if open.leaving.is_none() {
        let key = SharedString::from(key.to_owned());
        let task = cx.spawn(async move |cx| {
            cx.background_executor().timer(wait).await;
            cx.update(|cx| close(owner, Some(&key), cx));
        });
        if let Some(open) = cx.default_global::<Slot>().open.as_mut() {
            open.leaving = Some(task);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{px, TestAppContext};

    struct Owner;

    /// Catches: two cards standing at once, and the owner whose card was closed by another's opening not told to
    /// draw again, so its card stays on screen.
    #[gpui_kit::test]
    fn opening_a_card_closes_the_one_that_stood_and_tells_its_owner(cx: &mut TestAppContext) {
        let (a, b) = (cx.new(|_| Owner), cx.new(|_| Owner));
        let (ida, idb) = (a.entity_id(), b.entity_id());
        let told = std::rc::Rc::new(std::cell::Cell::new(0));
        let seen = told.clone();
        let _watch = cx.update(|cx| cx.observe(&a, move |_, _| seen.set(seen.get() + 1)));
        cx.update(|cx| open(ida, "a", POINTER, cx));
        cx.run_until_parked();
        let before = told.get();
        cx.update(|cx| open(idb, "b", By::Focus, cx));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(!shows(ida, "a", cx), "A still shows once B opened");
            assert!(shows(idb, "b", cx), "B does not show");
            assert_eq!(open_key(ida, cx), None);
            assert!(at_once(cx));
        });
        assert!(told.get() > before, "A's owner was not told its card closed");
        cx.update(|cx| close(ida, None, cx));
        cx.update(|cx| assert!(shows(idb, "b", cx), "closing A's card closed B's"));
        cx.update(|cx| close(idb, Some("b"), cx));
        cx.update(|cx| assert_eq!(open_key(idb, cx), None));
    }

    /// Catches: a card with a wait of its own (the context ring's 150 ms to cross to Compact) closing the moment the
    /// pointer leaves, or not staying when the pointer comes back within the wait.
    #[gpui_kit::test]
    fn a_card_with_a_wait_stays_while_the_pointer_crosses_to_it(cx: &mut TestAppContext) {
        let owner = cx.new(|_| Owner).entity_id();
        let (ring, card) = (
            gpui_kit::bounds(gpui_kit::point(px(0.), px(0.)), gpui_kit::size(px(28.), px(28.))),
            gpui_kit::bounds(gpui_kit::point(px(0.), px(34.)), gpui_kit::size(px(288.), px(100.))),
        );
        cx.update(|cx| {
            open(owner, "ring", By::Pointer(Duration::from_millis(150)), cx);
            place(owner, "ring", ring, false, cx);
            place(owner, "ring", card, true, cx);
            pointer_at(owner, "ring", gpui_kit::point(px(10.), px(31.)), cx);
        });
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(shows(owner, "ring", cx), "the card closed before its wait");
            pointer_at(owner, "ring", gpui_kit::point(px(10.), px(40.)), cx);
        });
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update(|cx| {
            assert!(shows(owner, "ring", cx), "the card closed with the pointer back on it");
            pointer_at(owner, "ring", gpui_kit::point(px(400.), px(400.)), cx);
        });
        cx.executor().advance_clock(Duration::from_millis(200));
        cx.run_until_parked();
        cx.update(|cx| assert!(!shows(owner, "ring", cx), "the card stood past its wait with the pointer off it"));
    }
}
