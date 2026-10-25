// SPDX-License-Identifier: AGPL-3.0-only
//! Settings, after `apps/web/src/settings/`: its own sidebar in the app sidebar's place (the groups, a search field,
//! the pages under a group, Back), the crumbs and the header's act in the top bar, and the open page in the centre,
//! one card per section. Every page is a plain function of `Ctx`, so the search walks every group's rows.

pub mod frame;
pub mod groups;
pub mod kept;
pub mod model;
pub mod page;
pub mod pages;
pub mod words;

mod acts;
mod parts;
mod reads;

#[cfg(test)]
pub(crate) mod door;
#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    div, prelude::*, px, AnyElement, AnyWindowHandle, App, Bounds, Context, Entity, Focusable as _, Pixels, ScrollHandle, SharedString,
    StyleRefinement, Subscription, Task, Window,
};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::ops::settings::{Image, Recipes};
use wsp_host_client::ops::sidebar::{ProjectImages, Sessions, Workspaces};
use wsp_host_client::ops::thread::Harnesses;
use wsp_host_client::protocol::{
    HarnessesListRequest, ImageGetRequest, Preferences, ProjectsIconsRequest, RecipesListRequest, ReleaseCheckRequest, SessionsListRequest,
    WorkspacesListRequest,
};
use wsp_host_client::{Env, Event, Failure, HostClient, Op};

use crate::screens::app_shell::{Host, Opened, Parts, Place, Screen};
use crate::screens::dialogs::layer::{self, Closes};
use crate::ui::theme;
use crate::ui::ActivePalette;
pub use acts::{Acts, Nav};
use model::{req, At, Data, GroupId, Push, Read, Reread};
pub use page::{CardData, Ctx, Item};
use parts::SettingsDialog;
pub use parts::{Part, SettingsPart, DIALOG};
use reads::{devices, held_of, hold_into, places, read_all, refusal, Held, HOLD};

/// How Settings is opened: the host it reads and the page it stands on first.
pub struct Opening {
    pub state: PathBuf,
    pub env: Env,
    pub runtime: Arc<Runtime>,
    pub at: At,
    /// How far down the page starts, once its cards are drawn: a capture's own scroll.
    pub scroll: f32,
}

/// How long a capture's scroll waits for the page to grow once its records landed.
const SCROLL_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// What a notice says: an act that failed, one that went through, or a plain fact.
#[derive(Clone, Copy)]
enum Said {
    Failed,
    Done,
    Note,
}

/// One frame of news from the host task for the screen's thread.
enum Arrival {
    Connected(Arc<HostClient>),
    Read(Box<Read>),
    Pushed(Value),
    Failed(String),
}

/// Escape with nothing of Settings' own to close, in a window with no frame to close it (a test's).
pub struct Closed;

impl gpui_kit::EventEmitter<Closed> for SettingsScreen {}

pub struct SettingsScreen {
    pub(crate) data: Data,
    /// Activity's view and computer, which its tabs pick and its page reads. Held here and not in a keyed state: the
    /// frame draws the tabs in a view of its own, and a keyed state belongs to the view that asked for it.
    activity: Entity<pages::activity::Pick>,
    /// The Agents pages' tab, computer, reports and sign-ins, kept while the window is open as the web's store keeps them.
    agents: Entity<pages::agents::Store>,
    /// The Computers rows' one menu, read by a test for the rows it asked for where the Mac draws it natively.
    computers_menu: Entity<pages::computers::RowMenu>,
    at: At,
    /// What the sidebar's field holds; a page of results stands in the centre while it holds text.
    search: String,
    search_field: Entity<InputState>,
    page_scroll: ScrollHandle,
    host: Option<Arc<HostClient>>,
    runtime: Arc<Runtime>,
    /// Why the host could not be reached, said in place of every page.
    unreachable: Option<String>,
    /// The words of the last act that failed or went through, shown until the next.
    notice: Option<(SharedString, Said)>,
    /// The page under a group's list that stands in place of it (one server or skill under Agents), for the crumbs.
    level: Option<page::Level>,
    /// The computer a remove is taking out while its dialog still has something to say: its page stays on screen.
    parting: Option<String>,
    /// Drawn as the page alone: the frame draws the sidebar, the crumbs and the header's act as three `SettingsPart`s.
    framed: bool,
    /// What the last frame worked out for the sidebar, which a framed screen's sidebar part draws from.
    drawn: Drawn,
    /// A capture's scroll, laid onto the page once it is taller than that, or as far as it goes once it has settled.
    start_scroll: f32,
    /// When the screen opened, and when its records landed, for the capture's scroll.
    opened: std::time::Instant,
    read_at: Option<std::time::Instant>,
    /// The field still holds the text a move from outside a frame cleared.
    clear_field: bool,
    /// The app's one copy of the preferences, which `data.preferences` mirrors and every write goes through.
    prefs: Entity<crate::prefs::Prefs>,
    /// The last record the host sent, for what the agents' lists are marked by.
    landed: Option<Preferences>,
    _prefs: Subscription,
    /// Held on the root, so Settings' own keys reach it from the moment it opens.
    focus: gpui_kit::FocusHandle,
    /// The window Settings stands in, whose page is kept for its next opening and whose layer its dialogs stand on.
    window: AnyWindowHandle,
    /// When the held reads were last asked of the host.
    asked: std::time::Instant,
    /// The page's scroll easing toward a card a trail row named.
    glide: Option<Glide>,
    /// A row a search result named, scrolled to and flashed once its page draws it.
    reveal: Option<Reveal>,
    /// How many glides and reveals have started, each one's motion keyed by its own.
    started: usize,
    /// The Usage page while its group is open, built on the way in and dropped on the way out as the web mounts it.
    usage: Option<(Entity<crate::screens::usage::UsagePage>, Subscription)>,
    /// The Activity page while Settings stands on it.
    activity_page: Option<Entity<pages::activity::ActivityPage>>,
    /// The host task, stopped with the screen.
    _follow: Option<tokio::task::AbortHandle>,
    _receive: Task<()>,
    _search: Subscription,
}

impl Drop for SettingsScreen {
    fn drop(&mut self) {
        if let Some(task) = self._follow.take() {
            task.abort();
        }
    }
}

impl SettingsScreen {
    pub fn open(opening: Opening, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Opening { state, env, runtime, at, scroll } = opening;
        let dial = async move { HostClient::connect(&state, &env).await.map(Arc::new) };
        SettingsScreen::start(dial, runtime, at, scroll, false, window, cx)
    }

    /// Settings on a host already dialled, as the app's frame holds it: drawn as its page alone, with the sidebar, the
    /// crumbs and the header's act as `SettingsPart`s the frame lays in its own places.
    pub fn with_client(
        client: Arc<HostClient>,
        runtime: Arc<Runtime>,
        at: At,
        scroll: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let acts = Acts::new(cx.weak_entity(), None, runtime.clone());
        let agents = pages::agents::store::kept(&client, window, cx, |cx| cx.new(|_| pages::agents::Store::new(acts)));
        agents.update(cx, |s, _| s.show());
        cx.observe(&agents, |_, _, cx| cx.notify()).detach();
        let key = Arc::as_ptr(&client) as usize;
        let fresh = cx.try_global::<Held>().and_then(|h| h.0.get(&key)).is_some_and(|(asked, _)| asked.elapsed() < HOLD);
        let held = fresh.then(|| cx.global_mut::<Held>().0.remove(&key)).flatten();
        let mut screen = SettingsScreen::start(async move { Ok(client) }, runtime, at, scroll, held.is_some(), window, cx);
        if let Some((asked, reads)) = held {
            hold_into(&mut screen.data, reads);
            screen.asked = asked;
        }
        screen.agents = agents;
        screen.framed = true;
        screen
    }

    fn start(
        dial: impl Future<Output = Result<Arc<HostClient>, Failure>> + Send + 'static,
        runtime: Arc<Runtime>,
        at: At,
        scroll: f32,
        held: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        kept::keep(window.window_handle().window_id(), &at, cx);
        cx.observe_global::<crate::update::Updates>(|_, cx| cx.notify()).detach();
        let search_field = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
        let search = cx.subscribe(&search_field, |this, field, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.search = field.read(cx).value().to_string();
                cx.notify();
            }
        });
        let (tx, mut rx) = mpsc::unbounded();
        let follow = runtime.spawn(async move {
            let host = match dial.await {
                Ok(host) => host,
                Err(failure) => {
                    _ = tx.unbounded_send(Arrival::Failed(failure.message));
                    return;
                }
            };
            _ = tx.unbounded_send(Arrival::Connected(host.clone()));
            let (mut read, mut held, mut attempt) = (false, held, 0);
            loop {
                // Listening before the reads, so a change that lands between a read and its answer is not lost. A
                // socket that went is a fresh dial here, and everything is read again on it, as the web app pulls on
                // a reconnect.
                let events = host.events().await;
                if events.is_ok() || !read {
                    read_all(&host, &tx, std::mem::take(&mut held)).await;
                    read = true;
                }
                if let Ok(mut events) = events {
                    attempt = 0;
                    while let Some(next) = events.next().await {
                        if let Ok(Event::Host(event)) = next {
                            if let Ok(value) = serde_json::to_value(&*event) {
                                _ = tx.unbounded_send(Arrival::Pushed(value));
                            }
                        }
                    }
                }
                if tx.is_closed() {
                    return;
                }
                attempt += 1;
                pages::activity::redial(attempt).await;
            }
        });
        // The window lets go of the project images as Settings goes, and the held reads are kept for its next opening.
        cx.on_release(|this, cx| {
            for image in this.data.project_images.clear() {
                cx.drop_image(image, None);
            }
            if let Some(host) = this.host.as_ref().filter(|_| this.framed) {
                let key = Arc::as_ptr(host) as usize;
                let reads = held_of(&mut this.data);
                cx.default_global::<Held>().0.insert(key, (this.asked, reads));
            }
        })
        .detach();
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |this, cx| this.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        let (prefs, mirror) = mirrored(cx);
        let data = Data { preferences: prefs.read(cx).shown().cloned(), ..Data::default() };
        SettingsScreen {
            data,
            activity: activity_pick(pages::activity::kept(window, cx), cx),
            agents: agents_store(runtime.clone(), cx),
            computers_menu: cx.new(|_| pages::computers::RowMenu::default()),
            at,
            search: String::new(),
            search_field,
            page_scroll: ScrollHandle::new(),
            host: None,
            runtime,
            unreachable: None,
            notice: None,
            level: None,
            clear_field: false,
            parting: None,
            start_scroll: scroll,
            opened: std::time::Instant::now(),
            read_at: None,
            framed: false,
            drawn: Drawn::default(),
            prefs,
            landed: None,
            _prefs: mirror,
            focus: focused(window, cx),
            window: window.window_handle(),
            asked: std::time::Instant::now(),
            glide: None,
            reveal: None,
            started: 0,
            usage: None,
            activity_page: None,
            _follow: Some(follow.abort_handle()),
            _receive: receive,
            _search: search,
        }
    }

    /// A screen with its records given, for a test that draws it with no host behind it.
    pub fn with_data(data: Data, at: At, runtime: Arc<Runtime>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search_field = cx.new(|cx| InputState::new(window, cx).placeholder("Search settings"));
        let search = cx.subscribe(&search_field, |this, field, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.search = field.read(cx).value().to_string();
                cx.notify();
            }
        });
        let landed = data.preferences.clone();
        let (prefs, mirror) = mirrored(cx);
        if let Some(p) = &landed {
            prefs.update(cx, |store, cx| store.landed(p, cx));
        }
        SettingsScreen {
            data,
            activity: activity_pick(cx.new(|_| pages::activity::Pick::default()), cx),
            agents: agents_store(runtime.clone(), cx),
            computers_menu: cx.new(|_| pages::computers::RowMenu::default()),
            at,
            search: String::new(),
            search_field,
            page_scroll: ScrollHandle::new(),
            host: None,
            runtime,
            unreachable: None,
            notice: None,
            level: None,
            clear_field: false,
            parting: None,
            start_scroll: 0.,
            opened: std::time::Instant::now(),
            read_at: None,
            framed: false,
            drawn: Drawn::default(),
            prefs,
            landed,
            _prefs: mirror,
            focus: focused(window, cx),
            window: window.window_handle(),
            asked: std::time::Instant::now(),
            glide: None,
            reveal: None,
            started: 0,
            usage: None,
            activity_page: None,
            _follow: None,
            _receive: Task::ready(()),
            _search: search,
        }
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        match arrival {
            Arrival::Connected(host) => {
                self.host = Some(host);
                // Each opening of General asks for the newest release, an opening that starts on it as well.
                if self.at.group() == GroupId::General {
                    self.check_release(cx);
                }
            }
            Arrival::Read(read) => match *read {
                Read::Preferences(Ok(p)) => self.preferences_landed(p, cx),
                read => {
                    if let Some(said) = refusal(&read) {
                        self.notice = Some((said.into(), Said::Failed));
                    }
                    // A fresh socket reads the projects again, and the counts on them with it.
                    let again = matches!(read, Read::Projects(_)) && self.data.standing.asked();
                    self.data.take(read);
                    if again || self.data.standing.again() {
                        self.reread(Reread::Standing, cx);
                    }
                }
            },
            Arrival::Pushed(event) => {
                if event.get("type").and_then(Value::as_str) == Some("preferences.changed") {
                    // The app's copy draws this screen again if the record moved what it shows.
                    if let Some(Ok(p)) = event.get("preferences").cloned().map(serde_json::from_value::<Preferences>) {
                        self.preferences_landed(p, cx);
                    }
                    return;
                } else {
                    match self.data.push(&event) {
                        Push::Nothing => return,
                        Push::Drawn => {}
                        Push::Reread(what) => self.reread(what, cx),
                    }
                }
            }
            Arrival::Failed(said) => {
                eprintln!("desktop-gpui: {said}");
                self.unreachable = Some(said);
            }
        }
        cx.notify();
    }

    fn apply_theme(&mut self, cx: &mut Context<Self>) {
        show_theme(self.data.preferences(), cx);
    }

    /// A record the host sent, in the order it sent them, handed to the app's copy, which keeps every write still out
    /// over it (`preferenceSetsInFlight`); the agents' lists read again where it moved what the host marks them by
    /// (`preferencesLanded`).
    fn preferences_landed(&mut self, p: Preferences, cx: &mut Context<Self>) {
        let marks = |p: &Preferences| {
            (p.default_agent.clone(), serde_json::to_value(&p.agent_defaults).ok(), serde_json::to_value(&p.project_defaults).ok())
        };
        if self.landed.as_ref().is_some_and(|before| marks(before) != marks(&p)) {
            self.reread(Reread::Harnesses, cx);
        }
        self.prefs.update(cx, |store, cx| store.landed(&p, cx));
        self.landed = Some(p);
    }

    /// Asks the host to read the newest release again; the host's own floor keeps that to one ask.
    fn check_release(&mut self, cx: &mut Context<Self>) {
        if self.host.is_none() {
            return;
        }
        self.call(req::<ReleaseCheckRequest>(json!({})), cx, |this, answer, cx| this.release_checked(answer.map(|r| r.release), cx));
    }

    /// release.check's answer: the release read, or a refusal said as the other refused reads are.
    fn release_checked(&mut self, answer: Result<wsp_host_client::protocol::ReleaseView, String>, cx: &mut Context<Self>) {
        match answer {
            Ok(release) => self.arrive(Arrival::Read(Box::new(Read::Release(Ok(release)))), cx),
            Err(said) => self.failed(format!("Newest release not read: {said}"), cx),
        }
    }

    /// The page Settings stands on: a computer or project the host no longer lists falls back to its group, but for the
    /// computer a remove is still saying something about (`useSettingsAt`).
    pub fn resolved(&self) -> At {
        match &self.at {
            At::Computer(id)
                if self.parting.as_deref() != Some(id) && self.data.places_read && !self.data.places.iter().any(|p| p.id == *id) =>
            {
                At::Group(GroupId::Computers)
            }
            At::Project(id) if self.data.projects_read && !self.data.projects.iter().any(|p| p.id == *id) => At::Group(GroupId::Projects),
            at => at.clone(),
        }
    }

    /// Escape, in the web's order: an open list, then a dialog, then the search's text while the field has the focus.
    /// True when one of them took it; false when Settings itself is to close, which puts back a theme the Appearance
    /// page showed under the pointer.
    pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if crate::ui::select::close_open_list(cx) {
            true
        } else if layer::top(cx).is_some() {
            layer::dismiss(window, cx);
            true
        } else if !self.search.is_empty() && self.search_field.read(cx).focus_handle(cx).is_focused(window) {
            self.search.clear();
            self.search_field.update(cx, |field, cx| field.set_value("", window, cx));
            cx.notify();
            true
        } else {
            self.apply_theme(cx);
            false
        }
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn at(&self) -> &At {
        &self.at
    }

    pub fn agents(&self) -> &Entity<pages::agents::Store> {
        &self.agents
    }

    pub fn activity(&self) -> &Entity<pages::activity::Pick> {
        &self.activity
    }

    /// The labels of the rows the Computers rows' menu last asked for.
    pub fn computers_menu_asked(&self, cx: &App) -> Vec<SharedString> {
        self.computers_menu.read(cx).asked().to_vec()
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    /// Moves to a page; the search is cleared, as the web's `go` does.
    pub fn go(&mut self, at: At, window: &mut Window, cx: &mut Context<Self>) {
        crate::screens::app_shell::shut_sidebar_sheet(cx);
        kept::keep(self.window.window_id(), &at, cx);
        self.at = at;
        self.level = None;
        self.glide = None;
        self.agents.update(cx, |s, _| s.show());
        self.page_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        if !self.search.is_empty() {
            self.search.clear();
            self.search_field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        if self.at.group() == GroupId::General {
            self.check_release(cx);
        }
        self.reread_on_open(cx);
        cx.notify();
    }

    /// The recipes are read again each time their page opens, as the web's page reads them on every mount.
    fn reread_on_open(&mut self, cx: &mut Context<Self>) {
        if self.at == At::Group(GroupId::Recipes) {
            self.reread(Reread::Recipes, cx);
        }
    }

    /// The same, from a place that holds no window: the field's text is cleared on the next frame.
    fn go_later(&mut self, at: At, cx: &mut Context<Self>) {
        kept::keep(self.window.window_id(), &at, cx);
        self.at = at;
        self.level = None;
        self.glide = None;
        self.page_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        self.clear_field = !self.search.is_empty();
        self.search.clear();
        self.reread_on_open(cx);
        cx.notify();
    }

    fn spawn_read(&mut self, cx: &mut Context<Self>, read: impl Future<Output = Read> + Send + 'static) {
        let task = self.runtime.spawn(read);
        cx.spawn(async move |this, cx| {
            let Ok(read) = task.await else { return };
            _ = this.update(cx, |this, cx| this.arrive(Arrival::Read(Box::new(read)), cx));
        })
        .detach();
    }

    pub fn reread(&mut self, what: Reread, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        match what {
            Reread::Recipes => self.spawn_read(cx, async move {
                Read::Recipes(host.call(req::<RecipesListRequest>(json!({}))).await.map(|r: Recipes| r.recipes).map_err(said))
            }),
            Reread::Harnesses => self.spawn_read(cx, async move {
                Read::Harnesses(host.call(req::<HarnessesListRequest>(json!({}))).await.map(|r: Harnesses| r.harnesses).map_err(said))
            }),
            Reread::Image => {
                self.data.image_reading = true;
                self.spawn_read(cx, async move {
                    Read::Image(host.call(req::<ImageGetRequest>(json!({}))).await.map(|r: Image| r.view).map_err(said))
                })
            }
            Reread::Devices => self.spawn_read(cx, async move { devices(&host).await }),
            Reread::Places => self.spawn_read(cx, async move { places(&host).await }),
            Reread::Standing => {
                if self.data.standing.reading() {
                    return;
                }
                self.data.standing.asking();
                let again = host.clone();
                self.spawn_read(cx, async move {
                    Read::Workspaces(
                        host.call(req::<WorkspacesListRequest>(json!({}))).await.map(|r: Workspaces| r.workspaces).map_err(said),
                    )
                });
                self.spawn_read(cx, async move {
                    Read::Sessions(again.call(req::<SessionsListRequest>(json!({}))).await.map(|r: Sessions| r.sessions).map_err(said))
                });
            }
        }
    }

    /// After a frame: the project images the bound let go leave the window, and those it wants are asked for.
    fn settle_images(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (ask, gone) = self.data.project_images.settle();
        for image in gone {
            cx.drop_image(image, Some(window));
        }
        if ask.is_empty() {
            return;
        }
        self.call(req::<ProjectsIconsRequest>(json!({ "hashes": ask.clone() })), cx, move |this, answer, cx| match answer {
            Ok(ProjectImages { icons }) => {
                if this.data.project_images.landed(icons) {
                    cx.notify();
                }
            }
            Err(said) => {
                this.data.project_images.failed(&ask);
                this.failed(format!("Project images not read: {said}"), cx);
            }
        });
    }

    /// Writes a patch of preferences, laid onto the records at once. The record the host keeps comes back as
    /// preferences.changed, on the socket that carries every other client's writes in the order the host made them;
    /// the answer, which rides another road to this thread, says only whether the write stood.
    pub fn set_preferences(&mut self, patch: Value, cx: &mut Context<Self>) {
        let host = self.host.clone().map(|host| (host, self.runtime.clone()));
        let window = self.window;
        self.prefs.update(cx, |store, cx| {
            store.write(patch, host, cx, move |answer, cx| {
                let words = match answer {
                    Ok(None) => return,
                    // What the set kept but could not finish, which the web app says as a failure.
                    Ok(Some(notice)) => notice,
                    Err(said) => format!("That setting was not saved: {}. It shows the host's value again.", said.trim_end_matches('.')),
                };
                // The window says it, as Settings may have closed before the host answered.
                _ = window.update(cx, |_, window, cx| crate::ui::notify_in(window, cx, crate::ui::Notice::error(words)));
            })
        });
    }

    /// One op on the host, its answer handed back on the screen's thread.
    pub fn call<O>(
        &mut self,
        op: O,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, Result<O::Reply, String>, &mut Context<Self>) + 'static,
    ) where
        O: Op + Send + 'static,
        O::Reply: Send + 'static,
    {
        let Some(host) = self.host.clone() else {
            then(self, Err("No host answered.".to_owned()), cx);
            return;
        };
        let (tx, rx) = oneshot::channel();
        self.runtime.spawn(async move {
            _ = tx.send(host.call(op).await.map_err(said));
        });
        cx.spawn(async move |this, cx| {
            let Ok(answer) = rx.await else { return };
            _ = this.update(cx, |this, cx| then(this, answer, cx));
        })
        .detach();
    }

    pub fn failed(&mut self, said: String, cx: &mut Context<Self>) {
        self.notice = Some((said.into(), Said::Failed));
        cx.notify();
    }

    pub fn done(&mut self, line: String, cx: &mut Context<Self>) {
        self.notice = Some((line.into(), Said::Done));
        cx.notify();
    }

    pub fn note(&mut self, line: String, cx: &mut Context<Self>) {
        self.notice = Some((line.into(), Said::Note));
        cx.notify();
    }

    /// Stands `dialog` on the window's dialogs layer, in place of a Settings dialog already standing.
    pub fn open_dialog(&mut self, dialog: page::DialogBuild, cx: &mut Context<Self>) {
        let closes = if dialog.dismissible { Closes::Outside } else { Closes::Escape };
        let screen = cx.entity();
        let view = cx.new(|cx| SettingsDialog::new(&screen, dialog.build, cx));
        let window = self.window;
        cx.defer(move |cx| _ = window.update(cx, |_, window, cx| layer::open(DIALOG, view.into(), closes, None, window, cx)));
    }

    /// The Usage page, built once its group opens on a host that answered, and let go once Settings moves off it.
    fn usage_page(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) -> Option<Entity<crate::screens::usage::UsagePage>> {
        let host = self.host.clone().filter(|_| open);
        let Some(client) = host else {
            self.usage = None;
            return None;
        };
        if self.usage.is_none() {
            let page = crate::screens::usage::page(Host::new(client.clone(), self.runtime.clone()), window, cx);
            let acts = Acts::new(cx.weak_entity(), Some(client), self.runtime.clone());
            page.update(cx, |page, _| page.acts = Some(acts));
            // The header's tabs are drawn from the page, so the parts hear of it through the screen.
            let moved = cx.observe(&page, |_, _, cx| cx.notify());
            self.usage = Some((page, moved));
        }
        self.usage.as_ref().map(|(page, _)| page.clone())
    }

    /// The Activity page while Settings stands on it.
    pub fn activity_page(&self) -> Option<&Entity<pages::activity::ActivityPage>> {
        self.activity_page.as_ref()
    }

    /// The Usage page while Settings stands on it.
    pub fn usage(&self) -> Option<&Entity<crate::screens::usage::UsagePage>> {
        self.usage.as_ref().map(|(page, _)| page)
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        let window = self.window;
        cx.defer(move |cx| _ = window.update(cx, |_, window, cx| layer::close(DIALOG, window, cx)));
    }

    pub fn runtime(&self) -> &Arc<Runtime> {
        &self.runtime
    }

    pub fn host(&self) -> Option<&Arc<HostClient>> {
        self.host.as_ref()
    }
}

/// The window's theme as the preferences pick it: the side asked for, or the system's.
fn show_theme(p: Option<&Preferences>, cx: &mut App) {
    let Some(p) = p else { return };
    let id = theme::picked(&p.theme.to_string(), &p.light_theme, &p.dark_theme, theme::is_dark(cx.window_appearance())).id;
    if theme::current(cx).id != id {
        theme::set_theme(id, cx);
    }
}

/// The app's copy of the preferences, mirrored into `data.preferences` and drawn again whenever it moves.
fn mirrored(cx: &mut Context<SettingsScreen>) -> (Entity<crate::prefs::Prefs>, Subscription) {
    let prefs = crate::prefs::store(cx);
    let mirror = cx.observe(&prefs, |this, prefs, cx| {
        this.data.preferences = prefs.read(cx).shown().cloned();
        this.apply_theme(cx);
        cx.notify();
    });
    (prefs, mirror)
}

/// Activity's pick, drawn again on this screen whichever view moved it.
fn activity_pick(pick: Entity<pages::activity::Pick>, cx: &mut Context<SettingsScreen>) -> Entity<pages::activity::Pick> {
    cx.observe(&pick, |_, _, cx| cx.notify()).detach();
    pick
}

/// The Agents pages' store, drawn again on this screen whenever it moves.
fn agents_store(runtime: Arc<Runtime>, cx: &mut Context<SettingsScreen>) -> Entity<pages::agents::Store> {
    let acts = Acts::new(cx.weak_entity(), None, runtime);
    let store = cx.new(|_| pages::agents::Store::new(acts));
    cx.observe(&store, |_, _, cx| cx.notify()).detach();
    store
}

/// A failure in the host's words.
fn said(failure: Failure) -> String {
    failure.message
}

/// A failure as `refusalParts` splits it: what happened, and what to do about it, which the host's words end with.
fn said_and_fix(failure: Failure) -> (String, Option<String>) {
    (failure.said().to_owned(), failure.fix.map(String::from))
}

impl Render for SettingsScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("SettingsScreen");
        let screen = cx.entity().downgrade();
        let room = self.page_scroll.max_offset().y;
        if self.start_scroll > 0. {
            // Timed, not counted: a window with no display to wait on draws frames as fast as it can.
            let now = std::time::Instant::now();
            if self.data.preferences.is_some() && self.read_at.is_none() {
                self.read_at = Some(now);
            }
            if now.duration_since(self.opened) > SCROLL_WAIT * 5 {
                self.start_scroll = 0.;
            } else if self.read_at.is_some_and(|read| room >= px(self.start_scroll) || now.duration_since(read) > SCROLL_WAIT) {
                let scroll = px(std::mem::take(&mut self.start_scroll)).min(room);
                self.page_scroll.set_offset(gpui_kit::point(px(0.), -scroll));
            } else {
                cx.on_next_frame(window, |_, _, cx| cx.notify());
            }
        }
        if std::mem::take(&mut self.clear_field) {
            self.search_field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        if let Some((words, said)) = self.notice.take() {
            use crate::ui::Notice;
            let note = match said {
                Said::Failed => Notice::error(words),
                Said::Done => Notice::done(words),
                Said::Note => Notice::note(words),
            };
            window.defer(cx, move |window, cx| crate::ui::notify_in(window, cx, note));
        }
        self.glide_on(window, cx);
        let flash = self.reveal_on(window, cx);
        let p = cx.palette().clone();
        let font = gpui_kit::component::ActiveTheme::theme(&**cx).font_family.clone();
        let at = self.resolved();
        // A narrow window lists the results in the sidebar and keeps the page in the centre, as the web's phone sheet does.
        let width = crate::ui::narrow::width(window);
        let narrow = width.md;
        let searching = !self.search.is_empty() && !narrow;
        if searching || at != At::Group(GroupId::Appearance) || !window.is_window_active() {
            // A theme the Appearance page showed under the pointer goes with the page and the window's focus (`ThemePicker.tsx`).
            self.apply_theme(cx);
        }
        let usage = self.usage_page(!searching && at == At::Group(GroupId::Usage), window, cx);
        let acts = Acts::new(screen.clone(), self.host.clone(), self.runtime.clone());
        let ctx =
            Ctx::new(&self.data, &at, self.level.as_ref(), acts, self.activity.clone(), self.agents.clone(), self.computers_menu.clone());
        let locked = !searching && matches!(at, At::Group(GroupId::Usage | GroupId::Activity));
        let activity = activity_page(&mut self.activity_page, (!searching && at == At::Group(GroupId::Activity)).then_some(&ctx), cx);
        let mut body = frame::Body::build(&ctx, if searching { &self.search } else { "" }, self.unreachable.as_deref(), window, cx);
        let found = (narrow && !self.search.is_empty()).then(|| frame::Body::build(&ctx, &self.search, None, window, cx));
        let results = found.as_ref().map(|f| f.results.clone());
        let hits = found.map_or_else(|| body.hits.clone(), |f| f.hits);
        if let Some((id, seen, fill)) = flash {
            let ink = p.foreground;
            for item in body.cards.iter_mut().flat_map(|c| c.items.iter_mut()).filter(|item| item.id == id) {
                let el = std::mem::replace(&mut item.el, crate::ui::CardItem::any(gpui_kit::div()));
                let seen = seen.clone();
                item.el = el.map(move |el, corners| flashed(el, corners, seen, ink.opacity(fill)));
            }
        }
        let trail = in_view(&body.trail, &self.page_scroll);
        let root = h_flex()
            .id("settings")
            .key_context("Settings")
            .track_focus(&self.focus)
            .size_full()
            .relative()
            // On the way back up, so an open select's list takes its own Escape first.
            .on_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && event.keystroke.modifiers == gpui_kit::Modifiers::none() {
                    if !this.escape(window, cx) {
                        cx.emit(Closed);
                    }
                    cx.stop_propagation();
                }
            }))
            .text_color(p.foreground)
            .font_family(font)
            // In the frame its centre column lays the ground and the grain.
            .when(!self.framed, |el| el.bg(p.background).child(crate::ui::grain(window.viewport_size())));
        let root = if self.framed {
            let drawn = Drawn { hits, results, trail: trail.clone() };
            // The frame draws the sidebar part before this page, so the part hears of what this frame worked out on the next.
            if self.drawn != drawn {
                self.drawn = drawn;
                cx.on_next_frame(window, |_, _, cx| cx.notify());
            }
            let page = match (usage, activity) {
                (Some(usage), _) => frame::page_of(usage.into_any_element(), &self.page_scroll, width.sm, window, cx).into_any_element(),
                (None, Some(activity)) => own_page(activity),
                (None, None) => frame::page(body, &self.page_scroll, locked, width.sm, window, cx).into_any_element(),
            };
            root.child(v_flex().flex_1().min_w_0().h_full().child(page))
        } else {
            let height = window.viewport_size().height;
            let sidebar = frame::sidebar(&ctx, &self.search_field, hits.as_deref(), results.as_deref(), &trail, true, height, window, cx);
            let top_bar = frame::top_bar(&ctx, searching, window, cx);
            let page = match activity {
                Some(activity) => own_page(activity),
                None => frame::page(body, &self.page_scroll, locked, width.sm, window, cx).into_any_element(),
            };
            root.child(sidebar).child(v_flex().flex_1().min_w_0().h_full().child(top_bar).child(page))
        };
        cx.defer_in(window, |this, window, cx| this.settle_images(window, cx));
        // In the frame the shell draws the dialogs layer over the whole window.
        root.children(if self.framed { None } else { layer::draw(window, cx) })
    }
}

/// The Activity page while Settings stands on it, `ctx` its records: made as Settings comes to it, let go as it leaves.
fn activity_page(
    held: &mut Option<Entity<pages::activity::ActivityPage>>,
    ctx: Option<&Ctx>,
    cx: &mut App,
) -> Option<Entity<pages::activity::ActivityPage>> {
    let Some(ctx) = ctx else {
        *held = None;
        return None;
    };
    let page = held.get_or_insert_with(|| cx.new(|cx| pages::activity::ActivityPage::new(cx))).clone();
    let of = pages::activity::page_of(ctx);
    page.update(cx, |page, _| page.set(of));
    Some(page)
}

/// A page that scrolls itself, drawn cached in the centre: a scroll step draws it alone.
fn own_page(view: Entity<impl Render>) -> AnyElement {
    div().flex_1().min_h_0().child(view.cached(StyleRefinement::default().size_full())).into_any_element()
}

/// The page's scroll on its way to a card, eased as the web's smooth `scrollIntoView` eases it.
struct Glide {
    from: Pixels,
    to: Pixels,
    /// The offset the last frame laid on; the page standing anywhere else means the person scrolled it.
    set: Pixels,
    duration: std::time::Duration,
    curve: gpui_kit::base::Easing,
    started: usize,
}

/// Where a revealed row was last drawn, set as it is drawn.
type Seen = Rc<Cell<Option<Bounds<Pixels>>>>;

/// A row a search result named: where it was drawn, once it was, and whether the page has been scrolled to it yet.
struct Reveal {
    id: SharedString,
    seen: Seen,
    /// Where the frame before drew it: the page is scrolled once two frames agree, its layout settled.
    last: Option<Bounds<Pixels>>,
    /// Frames drawn without the row, given up on at ten as the web's `revealItem` does.
    tries: u8,
    scrolled: bool,
    started: usize,
}

impl SettingsScreen {
    /// Lays the glide's next offset on the page, or lets it go once it arrived or the person scrolled.
    fn glide_on(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_kit::base::{animate_keyframes, Keyframe, Keyframes, MotionStatus, Timing};
        let Some(glide) = self.glide.as_mut() else { return };
        let offset = self.page_scroll.offset();
        if (offset.y - glide.set).abs() > px(0.5) {
            self.glide = None;
            return;
        }
        let frames = Keyframes::try_new([Keyframe::new(0., f32::from(glide.from)), Keyframe::new(1., f32::from(glide.to))])
            .expect("two frames, at 0 and 1");
        let timing = Timing::new(glide.duration).ease(glide.curve.clone());
        let now = animate_keyframes((glide.started, "settings-glide"), &frames, timing, window, cx);
        glide.set = px(now.value);
        self.page_scroll.set_offset(gpui_kit::point(offset.x, glide.set));
        if now.status == MotionStatus::Finished {
            self.glide = None;
        }
    }

    /// The row a search result named, with where it was drawn and its flash's fill this frame: the page is scrolled to
    /// put it in the middle once it has been drawn, then it flashes, or under reduced motion does not.
    fn reveal_on(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<(SharedString, Seen, f32)> {
        use crate::ui::motion;
        use gpui_kit::base::{animate_keyframes, Keyframe, Keyframes, MotionStatus, Timing};
        let reveal = self.reveal.as_mut()?;
        if !reveal.scrolled {
            match reveal.seen.get() {
                Some(row) if reveal.last != Some(row) => {
                    reveal.last = Some(row);
                    cx.on_next_frame(window, |_, _, cx| cx.notify());
                }
                Some(row) => {
                    // `scrollIntoView({ block: "center" })`, at once.
                    let scroll = &self.page_scroll;
                    let offset = scroll.offset();
                    let y = (offset.y - (row.center().y - scroll.bounds().center().y)).clamp(-scroll.max_offset().y, px(0.));
                    scroll.set_offset(gpui_kit::point(offset.x, y));
                    reveal.scrolled = true;
                    cx.on_next_frame(window, |_, _, cx| cx.notify());
                }
                None if reveal.tries >= 10 => {
                    self.reveal = None;
                    return None;
                }
                None => {
                    reveal.tries += 1;
                    cx.on_next_frame(window, |_, _, cx| cx.notify());
                }
            }
            return Some((reveal.id.clone(), reveal.seen.clone(), 0.));
        }
        if motion::reduced(cx) {
            self.reveal = None;
            return None;
        }
        let frames = Keyframes::try_new([Keyframe::new(0., motion::FLASH_FILL), Keyframe::new(1., 0.)]).expect("two frames, at 0 and 1");
        let timing = Timing::new(motion::FLASH).ease(motion::ease_out());
        let now = animate_keyframes((reveal.started, "settings-flash"), &frames, timing, window, cx);
        if now.status == MotionStatus::Finished {
            self.reveal = None;
            return None;
        }
        Some((reveal.id.clone(), reveal.seen.clone(), now.value))
    }
}

/// A row a search result named: where it lies is kept as it is drawn, and its flash's fill stands under it.
fn flashed(row: gpui_kit::AnyElement, corners: crate::ui::card::Corners, seen: Seen, fill: gpui_kit::Hsla) -> gpui_kit::AnyElement {
    let r = corners.radius - px(1.);
    let ground = gpui_kit::div()
        .absolute()
        .inset_0()
        .bg(fill)
        .when(corners.top, |el| el.rounded_tl(r).rounded_tr(r))
        .when(corners.bottom, |el| el.rounded_bl(r).rounded_br(r));
    let probe = gpui_kit::canvas(
        move |bounds, window, _| {
            if seen.get() != Some(bounds) {
                seen.set(Some(bounds));
                window.refresh();
            }
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0();
    gpui_kit::div()
        .relative()
        .debug_selector(|| "settings-revealed".into())
        .when(fill.a > 0., |el| el.child(ground.debug_selector(|| "settings-flash".into())))
        .child(row)
        .child(probe)
        .into_any_element()
}

/// What a frame worked out that the sidebar draws: each group's matches while searching, the trail's heads in view.
#[derive(Default, Clone, PartialEq)]
struct Drawn {
    hits: Option<Vec<(GroupId, usize)>>,
    results: Option<Vec<frame::Results>>,
    trail: Vec<(SharedString, usize, bool)>,
}

/// The trail's heads, each with whether its card is on screen now; a card is the page column's child at its index, and
/// the handle keeps where it lies with the page unscrolled.
fn in_view(trail: &[(SharedString, usize)], scroll: &ScrollHandle) -> Vec<(SharedString, usize, bool)> {
    let shown = scroll.bounds();
    let y = scroll.offset().y;
    trail
        .iter()
        .map(|(head, ix)| {
            let seen = scroll.bounds_for_item(*ix).is_some_and(|b| b.bottom() + y > shown.top() && b.top() + y < shown.bottom());
            (head.clone(), *ix, seen)
        })
        .collect()
}

/// A focus handle focused now, so a screen's keys reach it before anything inside it takes focus.
fn focused(window: &mut Window, cx: &mut Context<SettingsScreen>) -> gpui_kit::FocusHandle {
    let focus = cx.focus_handle();
    focus.focus(window, cx);
    focus
}

/// The page a route's argument names and how far down it opens: none with no argument, where Settings opens where it
/// last was. `computer:here` is one page, split on its first ':' only by the frame, and `general@588` opens General
/// 588 px down, as a capture of a page taller than the window takes it. A word that names no page is an error.
pub fn route(arg: Option<&str>) -> Result<(Option<At>, f32), NoSuchPage> {
    let Some(arg) = arg.filter(|a| !a.is_empty()) else { return Ok((None, 0.)) };
    let (page, scroll) = match arg.rsplit_once('@') {
        Some((page, px)) => (page, px.parse().unwrap_or(0.)),
        None => (arg, 0.),
    };
    At::parse(page).map(|at| (Some(at), scroll)).ok_or(NoSuchPage)
}

/// A route's word that names no page of Settings.
#[derive(Debug, PartialEq)]
pub struct NoSuchPage;

/// `LINK_WORDS.gone` for a Settings page.
pub const NO_SUCH_PAGE: &str = "That link names a Settings page this wsp does not have.";

impl Screen for SettingsScreen {
    const NAME: &'static str = "settings";
    const PLACE: Place = Place::Main;

    /// Settings on the page the route names, or where this window last had it; a page it names that is not here opens
    /// where it last was and says so.
    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let route = route(opened.arg.as_deref());
        let unknown = route.is_err();
        let (at, scroll) = route.unwrap_or((None, 0.));
        let at = at.unwrap_or_else(|| kept::last(window.window_handle().window_id(), cx));
        let mut screen = SettingsScreen::with_client(host.client().clone(), host.runtime().clone(), at, scroll, window, cx);
        if unknown {
            screen.notice = Some((NO_SUCH_PAGE.into(), Said::Failed));
        }
        screen
    }

    /// A route to another page of Settings while it stands open, as `settings:usage` from a link; one naming a page
    /// that is not here leaves the page and says so.
    fn show(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        match route(opened.arg.as_deref()) {
            Ok((Some(at), scroll)) => {
                self.start_scroll = scroll;
                self.go(at, window, cx);
            }
            Ok((None, _)) => {}
            Err(NoSuchPage) => self.failed(NO_SUCH_PAGE.to_owned(), cx),
        }
    }

    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        SettingsScreen::escape(self, window, cx)
    }

    fn parts(this: &Entity<Self>, _window: &mut Window, cx: &mut App) -> Parts {
        let part = |part: Part, cx: &mut App| gpui_kit::AnyView::from(cx.new(|cx| SettingsPart::new(this, part, cx)));
        Parts {
            sidebar: Some(part(Part::Sidebar, cx)),
            crumbs: Some(part(Part::Crumbs, cx)),
            actions: Some(part(Part::Actions, cx)),
            ..Parts::default()
        }
    }
}
