// SPDX-License-Identifier: AGPL-3.0-only
//! The acts a page's row raises, laid onto the screen once whatever is running is over.

use std::rc::Rc;
use std::sync::Arc;

use futures::channel::oneshot;
use gpui_kit::{px, App, Context, SharedString, WeakEntity, Window};
use serde_json::Value;
use tokio::runtime::Runtime;
use wsp_host_client::{HostClient, Op};

use super::model::{At, Read, Reread};
use super::{page, said, Arrival, Closed, Glide, Reveal, SettingsScreen};
use crate::screens::app_shell::keys;
use crate::ui::Notice;

/// The acts a page's row raises, held past the frame that drew it. Each is laid onto the screen once whatever is
/// running is over, so an act raised while the screen draws or takes an answer never re-enters it.
#[derive(Clone)]
pub struct Acts {
    screen: WeakEntity<SettingsScreen>,
    host: Option<Arc<HostClient>>,
    runtime: Arc<Runtime>,
    pane: Option<ToPane>,
}

type ToPane = Rc<dyn Fn(Nav, &mut App)>;

/// A page move a row raises, which a right panel's pane takes where no Settings screen stands.
pub enum Nav {
    Go(At),
    Level(Option<page::Level>),
}

impl Acts {
    pub fn new(screen: WeakEntity<SettingsScreen>, host: Option<Arc<HostClient>>, runtime: Arc<Runtime>) -> Self {
        Acts { screen, host, runtime, pane: None }
    }

    /// The acts of a pane that draws Settings' rows: their page moves go to `nav`.
    pub fn in_pane(mut self, nav: impl Fn(Nav, &mut App) + 'static) -> Self {
        self.pane = Some(Rc::new(nav));
        self
    }

    /// Hands `nav` to the pane, once this update ends, where the acts are the pane's.
    fn to_pane(&self, nav: Nav, cx: &mut App) -> bool {
        let Some(pane) = self.pane.clone() else { return false };
        cx.defer(move |cx| pane(nav, cx));
        true
    }

    fn with(&self, cx: &mut App, f: impl FnOnce(&mut SettingsScreen, &mut Context<SettingsScreen>) + 'static) {
        let screen = self.screen.clone();
        cx.defer(move |cx| {
            if let Some(screen) = screen.upgrade() {
                screen.update(cx, f);
            }
        });
    }

    pub fn go(&self, at: At, window: &mut Window, cx: &mut App) {
        if self.to_pane(Nav::Go(at.clone()), cx) {
            return;
        }
        let screen = self.screen.clone();
        window.defer(cx, move |window, cx| {
            if let Some(screen) = screen.upgrade() {
                screen.update(cx, |s, cx| s.go(at, window, cx));
            }
        });
    }

    pub fn go_later(&self, at: At, cx: &mut App) {
        if self.to_pane(Nav::Go(at.clone()), cx) {
            return;
        }
        self.with(cx, |s, cx| s.go_later(at, cx));
    }

    pub fn set_preferences(&self, patch: Value, cx: &mut App) {
        self.with(cx, |s, cx| s.set_preferences(patch, cx));
    }

    /// One op on the host; `then` runs on the app's thread, outside the screen, with the answer or the host's words.
    pub fn call<O>(&self, op: O, cx: &mut App, then: impl FnOnce(Result<O::Reply, String>, &mut App) + 'static)
    where
        O: Op + Send + 'static,
        O::Reply: Send + 'static,
    {
        let Some(host) = self.host.clone() else {
            cx.defer(move |cx| then(Err("No host answered.".to_owned()), cx));
            return;
        };
        let (tx, rx) = oneshot::channel();
        self.runtime.spawn(async move {
            _ = tx.send(host.call(op).await.map_err(said));
        });
        cx.spawn(async move |cx| {
            let Ok(answer) = rx.await else { return };
            cx.update(|cx| then(answer, cx));
        })
        .detach();
    }

    /// As [`Acts::with`], said as a toast where no Settings screen stands, as for the acts of a right panel's pane.
    fn say(&self, notice: Notice, cx: &mut App, f: impl FnOnce(&mut SettingsScreen, &mut Context<SettingsScreen>) + 'static) {
        let screen = self.screen.clone();
        cx.defer(move |cx| match screen.upgrade() {
            Some(screen) => screen.update(cx, f),
            None => crate::ui::notify(cx, notice),
        });
    }

    pub fn failed(&self, said: impl Into<String>, cx: &mut App) {
        let said = said.into();
        self.say(Notice::error(said.clone()), cx, |s, cx| s.failed(said, cx));
    }

    pub fn done(&self, line: impl Into<String>, cx: &mut App) {
        let line = line.into();
        self.say(Notice::done(line.clone()), cx, |s, cx| s.done(line, cx));
    }

    /// A plain fact about an act, neither a failure nor a success: an add whose dialog this app does not draw.
    pub fn note(&self, line: impl Into<String>, cx: &mut App) {
        let line = line.into();
        self.say(Notice::note(line.clone()), cx, |s, cx| s.note(line, cx));
    }

    pub fn reread(&self, what: Reread, cx: &mut App) {
        self.with(cx, move |s, cx| s.reread(what, cx));
    }

    /// Lays an act's answer onto the records as the read it stands for would land, so it outlives the page that asked.
    pub fn take(&self, read: Read, cx: &mut App) {
        self.with(cx, move |s, cx| s.arrive(Arrival::Read(Box::new(read)), cx));
    }

    pub fn open_dialog(&self, dialog: page::DialogBuild, cx: &mut App) {
        self.with(cx, |s, cx| s.open_dialog(dialog, cx));
    }

    pub fn close_dialog(&self, cx: &mut App) {
        self.with(cx, |s, cx| s.close_dialog(cx));
    }

    /// Keeps a computer's page on screen while its remove dialog has something to say, or lets it go with `None`.
    pub fn hold_parting(&self, id: Option<String>, cx: &mut App) {
        self.with(cx, move |s, cx| {
            s.parting = id;
            cx.notify();
        });
    }

    /// Stands a page in place of the open group's list, or puts the list back with `None`.
    pub fn open_level(&self, level: Option<page::Level>, cx: &mut App) {
        if self.to_pane(Nav::Level(level.clone()), cx) {
            return;
        }
        self.with(cx, move |s, cx| {
            s.level = level;
            s.page_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
            cx.notify();
        });
    }

    /// Asks the screen to draw again, after a page's own state moved.
    pub fn notify(&self, cx: &mut App) {
        self.with(cx, |_, cx| cx.notify());
    }

    /// Scrolls the page to its `ix`th card, as a press on the sidebar's trail does: smoothly, or at once under reduced
    /// motion.
    pub fn reveal_section(&self, ix: usize, cx: &mut App) {
        self.with(cx, move |s, cx| {
            // To the card's top, as far as the page goes (`scrollIntoView({ block: "start" })`).
            let scroll = &s.page_scroll;
            if let Some(card) = scroll.bounds_for_item(ix) {
                let to = (scroll.bounds().top() - card.top()).clamp(-scroll.max_offset().y, px(0.));
                let from = scroll.offset().y;
                if crate::ui::motion::reduced(cx) || (to - from).abs() < px(0.5) {
                    scroll.set_offset(gpui_kit::point(scroll.offset().x, to));
                    s.glide = None;
                } else {
                    s.started += 1;
                    let (duration, curve) = crate::ui::motion::smooth_scroll(to - from);
                    s.glide = Some(Glide { from, to, set: from, duration, curve, started: s.started });
                }
            }
            cx.notify();
        });
    }

    /// Scrolls the open page to the row `id` once it is drawn and flashes the row once (`revealItem`).
    pub fn reveal_item(&self, id: SharedString, cx: &mut App) {
        self.with(cx, move |s, cx| {
            s.started += 1;
            s.reveal = Some(Reveal { id, seen: Rc::default(), last: None, tries: 0, scrolled: false, started: s.started });
            cx.notify();
        });
    }

    /// Back: the frame closes Settings, as Escape with nothing open does; a window with no frame hears `Closed`.
    pub fn close(&self, window: &mut Window, cx: &mut App) {
        self.with(cx, |s, cx| {
            s.apply_theme(cx);
            cx.emit(Closed);
        });
        window.dispatch_action(Box::new(keys::CloseSettings), cx);
    }

    /// The runtime and the host, for a page that follows something of its own (Activity's processes).
    pub fn host(&self, _cx: &App) -> Option<(Arc<HostClient>, Arc<Runtime>)> {
        Some((self.host.clone()?, self.runtime.clone()))
    }
}
