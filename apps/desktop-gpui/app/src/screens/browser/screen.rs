// SPDX-License-Identifier: AGPL-3.0-only
//! The browser tab's screen (`BrowserSurface.tsx`): the chrome row over the servers list or the page, with the bars
//! that say why a page is down or refused. The tab's place, history and page live in the [`Store`]; this screen is
//! built again each time its thread shows.

use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    div, prelude::*, App, ClipboardItem, Context, Entity, FocusHandle, PromptButton, PromptLevel, SharedString, Subscription, Task, Window,
};
use serde_json::Value;

use super::bars;
use super::chrome::ChromeRow;
use super::field::{AddressField, FieldEvent};
use super::list::{OnUrl, RecentRow, ServerList, ServerRow};
use super::more::MoreMenu;
use super::page::{self, Page, Road};
use super::ports::{self, PortEvent};
use super::reach::Reach;
use super::recents;
use super::store::{Asked, Key, Store, PLACEHOLDER};
use super::tabs::{self, ZOOM_STEP};
use super::url::{self, Place};
use super::words;
use crate::local_store::LocalStore;
use crate::screens::app_shell::keys::PREVIEW;
use crate::screens::terminal::link::{Link, LinkEvent, LinkHold, Target};
use crate::screens::{navigate, Host, Opened, Pane, Parts, Place as ScreenPlace, Route, Screen};

/// `CopyUrlButton`: the Check stands this long.
const COPIED: Duration = Duration::from_millis(1500);
/// `ZoomIndicator`'s `HIDE_AFTER_MS`.
const ZOOM_SHOWN: Duration = bars::HOLD;

pub struct BrowserScreen {
    key: Key,
    opened: Opened,
    host: Option<Host>,
    store: Entity<Store>,
    field: Entity<AddressField>,
    /// What the field holds while the servers list shows: the list's filter.
    query: String,
    /// Why a typed place opened in the system browser, under the row until the next place.
    hint: Option<SharedString>,
    menu_open: bool,
    copied_at: Option<Instant>,
    zoom_changed_at: Option<Instant>,
    focus: FocusHandle,
    /// Why this window holds no page, when it cannot.
    road: Road,
    /// The page could not be made, and why.
    failed: Option<String>,
    /// The tab's port and whether it was on the daemon's list at its last word.
    was_listed: Option<(u16, bool)>,
    _link: Option<LinkHold>,
    _subscriptions: Vec<Subscription>,
    _copied: Option<Task<()>>,
    _zoomed: Option<Task<()>>,
}

fn key_of(opened: &Opened) -> Key {
    Key {
        thread: opened.thread_id.clone().unwrap_or_default().into(),
        tab: opened.tab.clone().unwrap_or_else(|| SharedString::new_static(PLACEHOLDER)),
    }
}

impl BrowserScreen {
    /// The screen on `opened`, with no host: `--screen browser`'s checks, on a page this process serves.
    pub fn standalone(opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(None, opened, window, cx)
    }

    fn build(host: Option<Host>, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let key = key_of(&opened);
        let store = Store::get(cx);
        let remote = opened.workspace_id.as_deref().is_some_and(|w| !is_local(w, cx));
        let fresh = store.read(cx).entry(&key).is_none();
        store.update(cx, |s, cx| s.follow(window, cx));
        let refused = store.update(cx, |s, cx| {
            // B and Add a panel make a new tab and ask the cap first; the placeholder and a tab this pane made asked already.
            let asks = fresh && key.tab != PLACEHOLDER && !s.minted_here(&key.tab);
            let refused = asks && !s.room(&key.thread, cx);
            if !refused {
                s.ensure(&key, opened.workspace_id.clone()).remote = remote;
            }
            refused
        });
        if refused {
            window.defer(cx, |window, cx| window.dispatch_action(Box::new(crate::screens::app_shell::keys::ClosePanelTab), cx));
        }
        let field = cx.new(|cx| AddressField::new(window, cx));
        let mut subscriptions = vec![
            cx.observe(&store, |this, _, cx| {
                this.sync_field(cx);
                cx.notify();
            }),
            cx.subscribe_in(&field, window, |this, _, event: &FieldEvent, window, cx| match event {
                FieldEvent::Submit(text) => this.submit(text, window, cx),
                FieldEvent::Draft(text) => {
                    this.query = text.clone();
                    cx.notify();
                }
            }),
            cx.subscribe_in(&store, window, |this, _, asked: &Asked, window, cx| this.asked(asked, window, cx)),
        ];
        let link = match (&host, &opened.workspace_id) {
            (Some(host), workspace) => {
                let target = workspace.clone().map_or(Target::Here, Target::Workspace);
                let link = Link::get(host, target, cx);
                let hold = link.update(cx, |link, cx| {
                    link.watch_ports(cx);
                    link.hold()
                });
                let directory = workspace.clone().unwrap_or_default();
                subscriptions.push(cx.subscribe(&link, move |this, _, event: &LinkEvent, cx| this.ports_said(&directory, event, cx)));
                Some(hold)
            }
            _ => None,
        };
        let mut this = BrowserScreen {
            key,
            opened,
            host,
            store,
            field,
            query: String::new(),
            hint: None,
            menu_open: false,
            copied_at: None,
            zoom_changed_at: None,
            focus: cx.focus_handle(),
            road: page::road(window),
            failed: None,
            was_listed: None,
            _link: link,
            _subscriptions: subscriptions,
            _copied: None,
            _zoomed: None,
        };
        this.sync_field(cx);
        this
    }

    fn ports_said(&mut self, workspace: &str, event: &LinkEvent, cx: &mut Context<Self>) {
        let now = chrono::Utc::now().timestamp_millis();
        self.store.update(cx, |store, cx| {
            let directory = store.directory_mut(workspace);
            let changed = match event {
                LinkEvent::Ports(rows) => directory.sync_ports(rows),
                LinkEvent::Port(event) => PortEvent::from_json(event).is_some_and(|e| directory.feed_event(&e, now)),
                LinkEvent::Changed => false,
            };
            if changed {
                cx.notify();
            }
        });
        // The port listening again: the bar goes and the page loads once (`BrowserSurface`'s wasListening).
        let listed = self.port_listed(cx);
        if came_back(std::mem::replace(&mut self.was_listed, listed), listed) {
            if let Some(page) = self.page(cx) {
                page.update(cx, |p, _| p.reload());
            }
        }
    }

    /// The tab's port and whether it is on the daemon's list, once the daemon has said one.
    fn port_listed(&self, cx: &App) -> Option<(u16, bool)> {
        let Some(Place::Port(address)) = self.place(cx) else { return None };
        let directory = self.store.read(cx).directory(&self.workspace().unwrap_or_default())?;
        directory.seeded().then(|| (address.port, directory.ports().iter().any(|p| p.port == address.port)))
    }

    fn entry<'a>(&self, cx: &'a App) -> Option<&'a super::store::Entry> {
        self.store.read(cx).entry(&self.key)
    }

    fn place(&self, cx: &App) -> Option<Place> {
        self.entry(cx).and_then(|e| e.tab.current().cloned())
    }

    fn sync_field(&mut self, cx: &mut Context<Self>) {
        let shown = self.place(cx).map(|p| url::place_address(&p)).unwrap_or_default();
        self.field.update(cx, |field, cx| field.set_url(shown, cx));
    }

    fn workspace(&self) -> Option<String> {
        self.opened.workspace_id.clone()
    }

    /// Opens `text` as if typed into the field and entered.
    pub fn open_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.submit(text, window, cx);
    }

    /// `openUrl`: the typed text as a place, loaded into this tab, or a new tab made on it from the placeholder.
    fn submit(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let place = url::parse_target(text);
        self.open_place(place, window, cx);
    }

    fn open_place(&mut self, place: Place, window: &mut Window, cx: &mut Context<Self>) {
        self.hint = None;
        if let Road::SystemBrowser(why) = self.road {
            cx.open_url(&url::place_url(&place));
            self.hint = Some(why.into());
            cx.notify();
            return;
        }
        self.remember(&place, cx);
        if self.key.tab == PLACEHOLDER {
            super::open_at(&self.key.thread, self.workspace(), place, cx);
            return;
        }
        let key = self.key.clone();
        // A tab with no page asks the cap as a place opens in it, as New tab's does (`roomForBrowserTab`).
        let thread = self.key.thread.clone();
        if self.page(cx).is_none() && !self.store.update(cx, |s, cx| s.room(&thread, cx)) {
            return;
        }
        self.store.update(cx, |s, cx| s.navigate(&key, Some(place), cx));
        self.show_page(window, cx);
        cx.notify();
    }

    /// `recordVisit`: a place opened goes first in the workspace's recents.
    fn remember(&self, place: &Place, cx: &mut App) {
        let store_key = recents::recents_key(&self.workspace().unwrap_or_default());
        let Some(local) = cx.try_global::<LocalStore>() else { return };
        let had = local.get(&store_key).map(recents::decode).unwrap_or_default();
        let now = chrono::Utc::now().timestamp_millis();
        let next = recents::record_visit(&had, &url::place_url(place), now);
        if let Ok(value) = serde_json::to_value(next) {
            _ = cx.global_mut::<LocalStore>().set(&store_key, value);
        }
    }

    fn forget(&self, url: &str, cx: &mut App) {
        let store_key = recents::recents_key(&self.workspace().unwrap_or_default());
        let Some(local) = cx.try_global::<LocalStore>() else { return };
        let had = local.get(&store_key).map(recents::decode).unwrap_or_default();
        if let Ok(value) = serde_json::to_value(recents::remove_visit(&had, url)) {
            _ = cx.global_mut::<LocalStore>().set(&store_key, value);
        }
    }

    fn page(&self, cx: &App) -> Option<Entity<Page>> {
        self.entry(cx).and_then(|e| e.page.clone())
    }

    /// The tab's page shows when its place is a page, and waits hidden on the servers list.
    fn show_page(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.road != Road::Here {
            return;
        }
        let key = self.key.clone();
        let on_list = self.place(cx).is_none();
        if let (Some(port), Some(host), Some(workspace)) = (self.remote_port(cx), self.host.clone(), self.workspace()) {
            let load = self.entry(cx).map_or(0, |e| e.tab.loads + e.tab.reload_nonce);
            self.store.update(cx, |s, cx| {
                s.want_route(&host, &workspace, port, cx);
                s.probe(&host, &workspace, port, load, cx);
            });
        }
        match self.store.update(cx, |s, cx| s.page_for(&key, window, cx)) {
            Ok(Some(page)) => {
                page.update(cx, |p, cx| p.hold_hidden(on_list, cx));
                self.failed = None;
            }
            Ok(None) => {}
            Err(why) => self.failed = Some(why),
        }
    }

    fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.key.clone();
        match self.page(cx) {
            Some(page) if page.read(cx).can_go_back() && self.place(cx).is_some() => page.update(cx, |p, _| p.back()),
            _ => {
                self.store.update(cx, |s, cx| {
                    s.update_tab(
                        &key,
                        |tab| {
                            let moved = tab.index > 0;
                            while tab.index > 0 {
                                tab.back();
                            }
                            moved
                        },
                        cx,
                    )
                });
                self.show_page(window, cx);
            }
        }
        self.sync_field(cx);
    }

    fn forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.key.clone();
        if self.place(cx).is_none() {
            self.store.update(cx, |s, cx| {
                s.update_tab(
                    &key,
                    |tab| {
                        let moved = tab.index + 1 < tab.entries.len();
                        while tab.index + 1 < tab.entries.len() {
                            tab.forward();
                        }
                        moved
                    },
                    cx,
                )
            });
            self.show_page(window, cx);
        } else if let Some(page) = self.page(cx) {
            page.update(cx, |p, _| p.forward());
        }
        self.sync_field(cx);
    }

    fn can_back(&self, cx: &App) -> bool {
        let Some(entry) = self.entry(cx) else { return false };
        entry.tab.current().is_some() && (entry.can_back || entry.tab.index > 0)
    }

    fn can_forward(&self, cx: &App) -> bool {
        let Some(entry) = self.entry(cx) else { return false };
        match entry.tab.current() {
            None => entry.tab.index + 1 < entry.tab.entries.len(),
            Some(_) => entry.can_forward,
        }
    }

    /// Refresh reloads; while the page loads it is Stop, and stops.
    fn refresh(&mut self, cx: &mut Context<Self>) {
        // A route that failed is asked for again, and the page it brings back is its one load.
        if let (Some(port), Some(host), Some(workspace)) = (self.remote_port(cx), self.host.clone(), self.workspace()) {
            if matches!(self.route(cx).and_then(|r| r.reach.as_ref()), Some(Reach::Failed(_))) {
                self.store.update(cx, |s, cx| s.remint(&host, &workspace, port, cx));
                return;
            }
        }
        let Some(page) = self.page(cx) else { return };
        let loading = self.entry(cx).is_some_and(|e| e.loading);
        page.update(cx, |p, _| if loading { p.stop() } else { p.reload() });
    }

    fn hard_reload(&mut self, cx: &mut Context<Self>) {
        if let Some(page) = self.page(cx) {
            page.update(cx, |p, _| p.reload());
        }
        self.menu_open = false;
        cx.notify();
    }

    /// The address the page is on, as copy and the system browser take it.
    fn real_url(&self, cx: &App) -> Option<String> {
        let place = self.place(cx)?;
        Some(self.page(cx).map(|p| p.read(cx).url.clone()).filter(|u| !u.is_empty()).unwrap_or_else(|| url::place_url(&place)))
    }

    fn copy(&mut self, cx: &mut Context<Self>) {
        let Some(url) = self.real_url(cx) else { return };
        cx.write_to_clipboard(ClipboardItem::new_string(url));
        self.copied_at = Some(Instant::now());
        self.menu_open = false;
        self._copied = Some(wake_after(COPIED, cx));
        cx.notify();
    }

    /// `shell.openExternal` for http(s) alone, as today's Open in system browser.
    fn open_outside(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = self.real_url(cx).filter(|u| words::window_road(u) == words::WindowRoad::Tab) {
            cx.open_url(&url);
        }
        self.menu_open = false;
        cx.notify();
    }

    fn zoom(&mut self, to: impl FnOnce(f32) -> f32, cx: &mut Context<Self>) {
        let key = self.key.clone();
        let mut changed = false;
        self.store.update(cx, |s, cx| {
            s.update_tab(
                &key,
                |tab| {
                    changed = tab.set_zoom(to(tab.zoom));
                    changed
                },
                cx,
            )
        });
        if changed {
            self.zoom_changed_at = Some(Instant::now());
            self._zoomed = Some(wake_after(ZOOM_SHOWN, cx));
        }
    }

    /// `openNewBrowser`: a tab on the servers list; on the placeholder, nothing, since it is one already.
    fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.menu_open = false;
        if self.key.tab == PLACEHOLDER {
            cx.notify();
            return;
        }
        let (thread, workspace) = (self.key.thread.clone(), self.workspace());
        if let Some(made) = self.store.update(cx, |s, cx| s.open_new(&thread, workspace, None, cx)) {
            navigate(Route::Screen(format!("{}:{}", super::NAME, made.tab).into()), cx);
        }
    }

    /// A page of this tab asked for a new window or to leave for another app.
    fn asked(&mut self, asked: &Asked, window: &mut Window, cx: &mut Context<Self>) {
        match asked {
            // `guestWindow`: a web page opens as a tab, anything else goes nowhere.
            Asked::NewTab { from, url } if *from == self.key && words::window_road(url) == words::WindowRoad::Tab => {
                super::open_at(&self.key.thread, self.workspace(), url::place_of_url(url, None), cx);
            }
            // A page off screen asks nothing; its next ask waits for a press on it.
            Asked::Leave { from, url } if *from == self.key && self.page(cx).is_some_and(|p| p.read(cx).placed().is_some()) => {
                let answer = window.prompt(
                    PromptLevel::Info,
                    &words::leave_message(url),
                    Some(words::LEAVE_DETAIL),
                    // Cancel first: the default and Escape's answer, as today's question has it.
                    &[PromptButton::Cancel(words::CANCEL.into()), PromptButton::Ok(words::OPEN.into())],
                    cx,
                );
                let (url, key, store) = (url.clone(), from.clone(), self.store.downgrade());
                cx.spawn(async move |_, cx| {
                    let open = answer.await == Ok(1);
                    _ = store.update(cx, |s, _| s.left(&key, !open));
                    if open {
                        cx.update(|cx| cx.open_url(&url));
                    }
                })
                .detach();
            }
            _ => {}
        }
    }

    fn recents(&self, cx: &App) -> Vec<RecentRow> {
        let store_key = recents::recents_key(&self.workspace().unwrap_or_default());
        let had = cx.try_global::<LocalStore>().and_then(|l| l.get(&store_key)).map(recents::decode).unwrap_or_default();
        let now = chrono::Utc::now().timestamp_millis();
        recents::shown(&had, &self.query)
            .into_iter()
            .map(|e| RecentRow {
                url: e.url.clone().into(),
                label: recent_label(&e.url).into(),
                title: e.title.clone().map(SharedString::from),
                visited: recents::relative_label(e.last_visited_at, now).into(),
            })
            .collect()
    }

    fn servers(&self, cx: &App) -> Vec<ServerRow> {
        let workspace = self.workspace().unwrap_or_default();
        let Some(directory) = self.store.read(cx).directory(&workspace) else { return Vec::new() };
        recents::servers_shown(directory.ports(), &self.query)
            .into_iter()
            .map(|p| ServerRow {
                name: p.process.clone().unwrap_or_else(|| "Listening".to_owned()).into(),
                address: format!("localhost:{}", p.port).into(),
                url: url::loopback_url(p.port, "/").into(),
            })
            .collect()
    }

    /// Whether the list has any row before the filter: with none, the empty sentence stands, not the filter's miss.
    fn lists_hold_any(&self, cx: &App) -> bool {
        let store_key = recents::recents_key(&self.workspace().unwrap_or_default());
        let recents = cx.try_global::<LocalStore>().and_then(|l| l.get(&store_key)).map(recents::decode).unwrap_or_default();
        let workspace = self.workspace().unwrap_or_default();
        let servers = self.store.read(cx).directory(&workspace).is_some_and(|d| !d.ports().is_empty());
        !recents::shown(&recents, "").is_empty() || servers
    }

    /// The port the tab shows when its workspace is off this computer, framed through a minted route.
    fn remote_port(&self, cx: &App) -> Option<u16> {
        match (self.entry(cx)?.remote, self.place(cx)?) {
            (true, Place::Port(address)) => Some(address.port),
            _ => None,
        }
    }

    fn route<'a>(&self, cx: &'a App) -> Option<&'a super::reach::Route> {
        let port = self.remote_port(cx)?;
        self.store.read(cx).route(&self.workspace()?, port)
    }

    /// The down bar's sentence: the place's port is off a list the daemon has said and, off this computer, its route
    /// does not answer either, so it is not answering.
    fn down_sentence(&self, cx: &App) -> Option<String> {
        let Some(Place::Port(address)) = self.place(cx) else { return None };
        let workspace = self.workspace().unwrap_or_default();
        let directory = self.store.read(cx).directory(&workspace)?;
        if !directory.seeded() || directory.ports().iter().any(|p| p.port == address.port) {
            return None;
        }
        if let Some(route) = self.route(cx) {
            let minting = matches!(route.reach, None | Some(Reach::Minting));
            if minting || route.answering != Some(false) {
                return None;
            }
        }
        let computer = computer_name(self.opened.workspace_id.as_deref(), cx);
        Some(ports::stopped_sentence(address.port, &computer, directory.stopped(address.port), clock, false))
    }

    /// The tab's name: a site's title or host, a port's process and path, else "Browser".
    pub fn title(&self, cx: &App) -> SharedString {
        let Some(entry) = self.entry(cx) else { return Pane::Browser.label().into() };
        let directory = self.store.read(cx).directory(&self.workspace().unwrap_or_default());
        let process = |port: u16| directory.and_then(|d| d.ports().iter().find(|p| p.port == port).and_then(|p| p.process.clone()));
        tabs::tab_title(&entry.tab, process).map_or_else(|| Pane::Browser.label().into(), SharedString::from)
    }

    pub fn page_entity(&self, cx: &App) -> Option<Entity<Page>> {
        self.page(cx)
    }

    /// Whether the tab's page is loading; for the checks.
    pub fn loading(&self, cx: &App) -> bool {
        self.entry(cx).is_some_and(|e| e.loading && e.page.is_some())
    }

    /// Whether the tab is on a page rather than the servers list; for the checks.
    pub fn on_page(&self, cx: &App) -> bool {
        self.place(cx).is_some()
    }

    pub fn set_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.menu_open = open;
        cx.notify();
    }

    pub fn menu_open(&self) -> bool {
        self.menu_open
    }

    pub fn field(&self) -> &Entity<AddressField> {
        &self.field
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus
    }
}

/// `PreviewRecentUrlCard`'s label: host, path past the root, query and hash.
fn recent_label(raw: &str) -> String {
    match ::url::Url::parse(raw) {
        Ok(u) => {
            let path = if u.path() == "/" { "" } else { u.path() };
            let host = u.host_str().map(|h| u.port().map_or_else(|| h.to_owned(), |p| format!("{h}:{p}"))).unwrap_or_default();
            format!(
                "{host}{path}{}{}",
                u.query().map(|q| format!("?{q}")).unwrap_or_default(),
                u.fragment().map(|f| format!("#{f}")).unwrap_or_default()
            )
        }
        Err(_) => raw.to_owned(),
    }
}

/// `clockLabel`: the daemon's time in the person's zone, hours and minutes.
fn clock(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|t| crate::words::clock_label(&t.with_timezone(&chrono::Local)))
        .unwrap_or_else(|_| iso.to_owned())
}

/// `useComputerName`: the computer a workspace runs on as the places list names it (`workspaceComputerName`), this
/// computer's own name for a workspace here, the workspace's id until its record arrives; this computer for none.
fn computer_name(workspace: Option<&str>, cx: &App) -> String {
    use crate::screens::sidebar::{looks, menu};
    let sidebar = crate::screens::terminal::workspaces_sidebar(cx);
    let Some(sidebar) = sidebar.as_ref().map(|s| s.read(cx)) else { return workspace.unwrap_or("this computer").to_owned() };
    let places = &sidebar.around().places;
    let Some(workspace) = workspace else {
        return Some(menu::here_name(places)).filter(|n| !n.is_empty()).unwrap_or_else(|| "this computer".into());
    };
    let Some(record) = sidebar.workspace(workspace).0 else { return workspace.to_owned() };
    let row =
        menu::place_of(record, places).and_then(|id| places.iter().find(|p| p.get("id").and_then(Value::as_str) == Some(id.as_str())));
    match row {
        Some(place) => looks::place_name(place),
        None if record.get("kind").and_then(Value::as_str) == Some("local") => menu::here_name(places),
        None => record.get("provider").and_then(Value::as_str).map(looks::provider_key_name).unwrap_or_else(|| workspace.to_owned()),
    }
}

impl Screen for BrowserScreen {
    const NAME: &'static str = super::NAME;
    const PLACE: ScreenPlace = ScreenPlace::Panel(Pane::Browser);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::build(Some(host), opened, window, cx)
    }

    fn closed(_host: &Host, opened: Opened, cx: &mut App) {
        let key = key_of(&opened);
        let store = Store::get(cx);
        store.update(cx, |s, cx| s.closed(&key, cx));
    }

    fn parts(this: &Entity<Self>, _window: &mut Window, _cx: &mut App) -> Parts {
        let this = this.downgrade();
        Parts {
            title: Some(Rc::new(move |cx: &App| this.upgrade().map_or_else(|| Pane::Browser.label().into(), |s| s.read(cx).title(cx)))),
            ..Parts::default()
        }
    }
}

impl Render for BrowserScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.show_page(window, cx);
        let entry = self.entry(cx);
        let place = entry.and_then(|e| e.tab.current().cloned());
        let loading = entry.is_some_and(|e| e.loading && e.page.is_some());
        let zoom = entry.map_or(1., |e| e.tab.zoom);
        let has_page = place.is_some();
        let sheet = window.viewport_size().width <= crate::screens::app_shell::frame::SHEET_MAX_VIEWPORT;
        let this = cx.entity().downgrade();
        let act = |f: fn(&mut BrowserScreen, &mut Window, &mut Context<BrowserScreen>)| {
            let this = this.clone();
            Rc::new(move |window: &mut Window, cx: &mut App| {
                _ = this.update(cx, |s, cx| f(s, window, cx));
            }) as Rc<dyn Fn(&mut Window, &mut App)>
        };
        let more = MoreMenu {
            open: self.menu_open,
            has_page,
            zoom,
            on_toggle: act(|s, _, cx| s.set_menu(!s.menu_open, cx)),
            on_close: act(|s, _, cx| s.set_menu(false, cx)),
            on_hard_reload: act(|s, _, cx| s.hard_reload(cx)),
            on_copy: act(|s, _, cx| s.copy(cx)),
            on_open_outside: act(|s, _, cx| s.open_outside(cx)),
            on_new_tab: act(|s, _, cx| s.new_tab(cx)),
            on_zoom_in: act(|s, _, cx| s.zoom(|z| z + ZOOM_STEP, cx)),
            on_zoom_out: act(|s, _, cx| s.zoom(|z| z - ZOOM_STEP, cx)),
            on_zoom_reset: act(|s, _, cx| s.zoom(|_| 1., cx)),
        };
        let forwarded = match &place {
            Some(Place::Port(address)) => forwarded(self.opened.workspace_id.as_deref(), address.port, cx)
                .then(|| SharedString::from(url::loopback_address(address.port, &address.path))),
            _ => None,
        };
        let copied = self.copied_at.is_some_and(|at| at.elapsed() < COPIED);
        let laptop_url = match &place {
            Some(Place::Port(address)) => Some(url::loopback_url(address.port, &address.path)),
            _ => None,
        };
        let chrome = ChromeRow {
            field: self.field.clone(),
            loading,
            can_back: self.can_back(cx),
            can_forward: self.can_forward(cx),
            refresh_held: !has_page,
            has_page,
            // `data-preview-panel-mode`: the panel beside the column is inline; at the sheet's widths it is a sheet.
            inline: !sheet,
            forwarded,
            copy: has_page.then_some(copied),
            more: more.into_any_element(),
            on_back: act(|s, w, cx| s.back(w, cx)),
            on_forward: act(|s, w, cx| s.forward(w, cx)),
            on_refresh: act(|s, _, cx| s.refresh(cx)),
            on_open_outside: act(|s, _, cx| s.open_outside(cx)),
            on_open_on_laptop: Rc::new(move |_, cx: &mut App| {
                if let Some(url) = &laptop_url {
                    cx.open_url(url);
                }
            }),
            on_copy: act(|s, _, cx| s.copy(cx)),
        };
        let body = match &place {
            None => {
                let this = this.clone();
                let open: OnUrl = Rc::new(move |url, window, cx| {
                    _ = this.update(cx, |s, cx| s.open_place(url::parse_target(&url), window, cx));
                });
                let this = cx.entity().downgrade();
                let remove: OnUrl = Rc::new(move |url, _, cx| {
                    _ = this.update(cx, |s, cx| {
                        s.forget(&url, cx);
                        cx.notify();
                    });
                });
                div()
                    .flex_1()
                    .min_h_0()
                    .child(ServerList {
                        recents: self.recents(cx),
                        servers: self.servers(cx),
                        behind: None,
                        filtered: !self.query.trim().is_empty() && self.lists_hold_any(cx),
                        on_open: open,
                        on_remove: remove,
                    })
                    .into_any_element()
            }
            Some(_) => {
                let down = self.down_sentence(cx);
                let pill = self.zoom_changed_at.filter(|at| at.elapsed() < ZOOM_SHOWN).map(|_| zoom);
                let route = self.route(cx);
                let refusal = route.and_then(|r| r.refusal.clone());
                let failed_route = route.and_then(|r| match &r.reach {
                    Some(Reach::Failed(_)) => self.remote_port(cx),
                    _ => None,
                });
                let slot = match (self.page(cx), &self.failed, failed_route, &refusal) {
                    (_, _, Some(port), _) => {
                        let computer = computer_name(self.opened.workspace_id.as_deref(), cx);
                        bars::empty_sentence(words::no_route_line(port, &computer), None).into_any_element()
                    }
                    (_, _, _, Some(r)) if !r.keeps_frame => {
                        bars::empty_sentence(r.title.clone(), Some(r.detail.clone().into())).into_any_element()
                    }
                    // In a sheet the page leaves while a menu stands over it, as today's leaves under a modal.
                    (Some(_), _, _, _) if sheet && self.menu_open => div().flex_1().into_any_element(),
                    (Some(page), _, _, _) => Page::slot(&page, cx).into_any_element(),
                    (None, Some(why), _, _) => bars::empty_sentence(why.clone(), None).into_any_element(),
                    (None, None, _, _) => div().flex_1().into_any_element(),
                };
                let kept = refusal.filter(|r| r.keeps_frame);
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .when(failed_route.is_none(), |el| el.children(down.map(bars::down_bar)))
                    .children(kept.map(|r| bars::refusal_bar(r.title, r.detail)))
                    .child(slot)
                    .child(bars::zoom_pill(zoom, pill.is_some(), self.zoom_changed_at))
                    .into_any_element()
            }
        };
        div()
            .id("browser")
            .key_context(PREVIEW)
            .track_focus(&self.focus)
            .size_full()
            .flex()
            .flex_col()
            .child(chrome)
            .children(self.hint.clone().map(bars::hint_bar))
            .child(body)
    }
}

/// Draws the screen again once `wait` is up, as a timed look (the Check, the zoom pill) ends.
fn wake_after(wait: Duration, cx: &mut Context<BrowserScreen>) -> Task<()> {
    cx.spawn(async move |this, cx| {
        cx.background_executor().timer(wait + Duration::from_millis(20)).await;
        _ = this.update(cx, |_, cx| cx.notify());
    })
}

/// Whether the same port went from off the daemon's list to on it.
fn came_back(was: Option<(u16, bool)>, now: Option<(u16, bool)>) -> bool {
    matches!((was, now), (Some((before, false)), Some((after, true))) if before == after)
}

/// `isLocalWorkspace`: the workspace's record names this computer.
fn is_local(workspace: &str, cx: &App) -> bool {
    let Some(sidebar) = crate::screens::terminal::workspaces_sidebar(cx) else { return true };
    let kind = sidebar.read(cx).workspace(workspace).0.and_then(|r| r.get("kind")).and_then(Value::as_str).map(str::to_owned);
    kind.as_deref().unwrap_or("cloud") == "local"
}

/// Whether the host forwards `port` of `workspace` to this computer (`useForwarded`).
fn forwarded(workspace: Option<&str>, port: u16, cx: &App) -> bool {
    let (Some(workspace), Some(sidebar)) = (workspace, crate::screens::terminal::workspaces_sidebar(cx)) else { return false };
    sidebar.read(cx).around().forwards.iter().any(|f| {
        f.get("workspaceId").and_then(Value::as_str) == Some(workspace) && f.get("port").and_then(Value::as_u64) == Some(u64::from(port))
    })
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::{px, size, Entity, TestAppContext, VisualTestContext};

    use super::*;
    use crate::screens::browser::page::WAYLAND_LINE;
    use crate::screens::browser::url::Address;
    use crate::ui::{self, Side};

    fn open<'a>(cx: &'a mut TestAppContext, tab: &str, road: Road) -> (Entity<BrowserScreen>, &'a mut VisualTestContext) {
        let dir = tempfile::tempdir().expect("a temp folder").keep();
        cx.update(|cx| {
            gpui_kit::init(cx);
            ui::init(Side::Light, cx);
            crate::local_store::init(dir.join("store.json"), cx);
        });
        let opened = Opened { workspace_id: Some("w1".into()), thread_id: Some("t1".into()), tab: Some(tab.to_owned().into()), arg: None };
        let screen = std::rc::Rc::new(std::cell::RefCell::new(None));
        let made = screen.clone();
        let (_, cx) = cx.add_window_view(move |window, cx| {
            let view = cx.new(|cx| {
                let mut s = BrowserScreen::standalone(opened, window, cx);
                s.road = road;
                s
            });
            *made.borrow_mut() = Some(view.clone());
            Root::new(view, window, cx)
        });
        cx.simulate_resize(size(px(540.), px(700.)));
        cx.run_until_parked();
        let screen = screen.borrow().clone().expect("the screen is built");
        (screen, cx)
    }

    fn place(screen: &Entity<BrowserScreen>, cx: &mut VisualTestContext) -> Option<Place> {
        cx.read(|cx| screen.read(cx).place(cx))
    }

    fn port(port: u16) -> Place {
        Place::Port(Address { port, path: "/".into() })
    }

    #[test]
    fn a_port_that_listens_again_loads_once_and_another_port_does_not() {
        assert!(came_back(Some((3000, false)), Some((3000, true))));
        assert!(!came_back(Some((3000, true)), Some((3000, true))), "it was listening already");
        assert!(!came_back(Some((3000, false)), Some((4000, true))), "the tab moved to another port");
        assert!(!came_back(None, Some((3000, true))), "the first list says nothing came back");
    }

    #[gpui_kit::test]
    fn on_wayland_a_place_opens_in_the_system_browser_with_a_line_saying_why(cx: &mut TestAppContext) {
        let (screen, vcx) = open(cx, "browser:1", Road::SystemBrowser(WAYLAND_LINE));
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.open_text("3000", window, cx)));
        vcx.run_until_parked();
        assert_eq!(vcx.opened_url().as_deref(), Some("http://localhost:3000"));
        assert_eq!(vcx.read(|cx| screen.read(cx).hint.clone()), Some(WAYLAND_LINE.into()));
        assert_eq!(place(&screen, vcx), None, "the tab stays on the servers list");
    }

    #[gpui_kit::test]
    fn the_placeholders_first_place_makes_a_tab_of_its_own(cx: &mut TestAppContext) {
        let (screen, vcx) = open(cx, PLACEHOLDER, Road::Here);
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.open_text(":3000", window, cx)));
        vcx.run_until_parked();
        let made = Key { thread: "t1".into(), tab: "browser:p1".into() };
        let store = vcx.read(Store::get_existing);
        assert_eq!(vcx.read(|cx| store.read(cx).entry(&made).and_then(|e| e.tab.current().cloned())), Some(port(3000)));
        assert_eq!(place(&screen, vcx), None, "the placeholder itself stays on the list");
        let recents = vcx.read(|cx| cx.global::<LocalStore>().get(&recents::recents_key("w1")).map(recents::decode).unwrap_or_default());
        assert_eq!(recents.first().map(|e| e.url.as_str()), Some("http://localhost:3000"), "the place went first in recents");
    }

    #[gpui_kit::test]
    fn back_reaches_the_servers_list_and_forward_returns(cx: &mut TestAppContext) {
        let (screen, vcx) = open(cx, "browser:1", Road::Here);
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.open_text("3000", window, cx)));
        vcx.run_until_parked();
        assert_eq!(place(&screen, vcx), Some(port(3000)));
        assert!(vcx.read(|cx| screen.read(cx).can_back(cx)), "Back is live on a page, as the list is behind it");
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.back(window, cx)));
        assert_eq!(place(&screen, vcx), None, "Back reached the servers list");
        assert!(vcx.read(|cx| screen.read(cx).can_forward(cx)));
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.forward(window, cx)));
        assert_eq!(place(&screen, vcx), Some(port(3000)), "Forward came back to the page");
    }

    #[gpui_kit::test]
    fn the_tab_reads_the_port_and_its_process_never_the_title(cx: &mut TestAppContext) {
        let (screen, vcx) = open(cx, "browser:1", Road::Here);
        assert_eq!(vcx.read(|cx| screen.read(cx).title(cx)), SharedString::from("Browser"));
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.open_text("3000/a", window, cx)));
        assert_eq!(vcx.read(|cx| screen.read(cx).title(cx)), SharedString::from(":3000/a"));
        let store = vcx.read(Store::get_existing);
        vcx.update(|_, cx| {
            store.update(cx, |s, _| {
                s.directory_mut("w1").sync_ports(&serde_json::json!([{ "port": 3000, "pid": 7, "process": "vite" }]));
            })
        });
        assert_eq!(vcx.read(|cx| screen.read(cx).title(cx)), SharedString::from("vite :3000/a"));
    }

    #[gpui_kit::test]
    fn a_port_the_daemon_does_not_list_reads_down_only_once_it_has_said(cx: &mut TestAppContext) {
        let (screen, vcx) = open(cx, "browser:1", Road::Here);
        vcx.update(|window, cx| screen.update(cx, |s, cx| s.open_text("47802", window, cx)));
        assert_eq!(vcx.read(|cx| screen.read(cx).down_sentence(cx)), None, "nothing is said before the first watch reply");
        let store = vcx.read(Store::get_existing);
        vcx.update(|_, cx| store.update(cx, |s, _| s.directory_mut("w1").sync_ports(&serde_json::json!([{ "port": 3000 }]))));
        assert_eq!(
            vcx.read(|cx| screen.read(cx).down_sentence(cx)).as_deref(),
            Some(":47802 on w1 is not answering; this tab reconnects when it does")
        );
        vcx.update(|_, cx| store.update(cx, |s, _| s.directory_mut("w1").sync_ports(&serde_json::json!([{ "port": 47802 }]))));
        assert_eq!(vcx.read(|cx| screen.read(cx).down_sentence(cx)), None, "a listed port is listening");
    }
}
