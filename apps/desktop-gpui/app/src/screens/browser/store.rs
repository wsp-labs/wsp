// SPDX-License-Identifier: AGPL-3.0-only
//! What the browser pane keeps beyond any one screen, for the run: each tab's place and history (`browser/tabs.ts`),
//! its page while it holds one (`browser/guests.ts`), and each workspace's listening ports (`browser/model.ts`). A
//! tab's screen is built again each time its thread shows; the tab and its page wait here.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gpui_kit::{App, AppContext, Context, Entity, Global, SharedString, Subscription, Task, Window};

use super::page::{Page, PageEvent};
use super::ports::Directory;
use super::reach::{self, Reach, Route};
use super::tabs::Tab;
use super::url::{self, Place};
use super::words;
use crate::screens::Host;
use crate::ui::{notify, Notice, NoticeKind};
use wsp_host_client::protocol::{WorkspacesPortProbeRequest, WorkspacesPortReachRequest};

/// `GUEST_IDLE_MS`: a page not shown for this long is unloaded; its tab keeps where it was.
pub const IDLE: Duration = Duration::from_secs(5 * 60);
/// `GUEST_CAP`: pages held at once, each a web process of its own.
pub const CAP: usize = 10;
/// `LOADED_PANELS`: the threads shown last whose tabs keep their pages, beside the one on screen.
pub const LOADED_THREADS: usize = 3;

/// A tab by the panel it is in (its thread, or "" for this computer's own panel) and its id there.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Key {
    pub thread: SharedString,
    pub tab: SharedString,
}

/// The placeholder tab `mod+shift+j` opens when a panel has no browser tab; its first place makes a real tab.
pub const PLACEHOLDER: &str = "browser:new";

/// One tab: where it is and has been, and the page that shows it while it holds one.
pub struct Entry {
    pub workspace: Option<String>,
    /// The workspace is off this computer: a port is framed through the route the host mints for it.
    pub remote: bool,
    pub tab: Tab,
    pub page: Option<Entity<Page>>,
    /// The page's own history, as the web view last said it.
    pub can_back: bool,
    pub can_forward: bool,
    pub loading: bool,
    /// When its page left the screen; None while shown or holding none.
    hidden_since: Option<Instant>,
    /// The load count the page has loaded.
    loaded: u64,
    _page: Option<Subscription>,
}

impl Entry {
    fn new(workspace: Option<String>, place: Option<Place>) -> Self {
        Entry {
            workspace,
            remote: false,
            tab: Tab::new(place),
            page: None,
            can_back: false,
            can_forward: false,
            loading: false,
            hidden_since: None,
            loaded: 0,
            _page: None,
        }
    }
}

/// What a tab's page asked for that the pane answers: a new tab on its address, or a link that leaves wsp.
#[derive(Clone, Debug, PartialEq)]
pub enum Asked {
    NewTab { from: Key, url: String },
    Leave(String),
}

pub struct Store {
    entries: HashMap<Key, Entry>,
    directories: HashMap<String, Directory>,
    routes: HashMap<(String, u16), Route>,
    /// The threads whose panels showed a browser tab last, newest first.
    shown: Vec<SharedString>,
    minted: u64,
    _sweep: Option<Task<()>>,
}

impl gpui_kit::EventEmitter<Asked> for Store {}

struct Global_(Entity<Store>);

impl Global for Global_ {}

impl Store {
    /// The app's one store, made on first ask.
    pub fn get(cx: &mut App) -> Entity<Store> {
        if let Some(store) = cx.try_global::<Global_>() {
            return store.0.clone();
        }
        let store = cx.new(|_| Store {
            entries: HashMap::new(),
            directories: HashMap::new(),
            routes: HashMap::new(),
            shown: Vec::new(),
            minted: 0,
            _sweep: None,
        });
        let weak = store.downgrade();
        cx.on_window_closed(move |cx, window| _ = weak.update(cx, |store, cx| store.window_closed(window, cx))).detach();
        cx.set_global(Global_(store.clone()));
        store
    }

    /// The app's store once made; for tests that read what a screen did.
    #[cfg(test)]
    pub fn get_existing(cx: &App) -> Entity<Store> {
        cx.global::<Global_>().0.clone()
    }

    pub fn entry(&self, key: &Key) -> Option<&Entry> {
        self.entries.get(key)
    }

    /// The tab under `key`, made on the servers list the first time it is asked for.
    pub fn ensure(&mut self, key: &Key, workspace: Option<String>) -> &mut Entry {
        self.entries.entry(key.clone()).or_insert_with(|| Entry::new(workspace, None))
    }

    /// Whether this store minted `tab`, so its cap was asked as it was made.
    pub fn minted_here(&self, tab: &str) -> bool {
        tab.strip_prefix("browser:p").and_then(|n| n.parse::<u64>().ok()).is_some_and(|n| n >= 1 && n <= self.minted)
    }

    /// A new tab's id, one the panel's own numbering never gives.
    pub fn mint(&mut self) -> SharedString {
        self.minted += 1;
        format!("browser:p{}", self.minted).into()
    }

    /// A new tab on `place` in `thread`'s panel, unless the cap refuses it; the refusal says the cap's line.
    pub fn open_new(
        &mut self,
        thread: &SharedString,
        workspace: Option<String>,
        place: Option<Place>,
        cx: &mut Context<Self>,
    ) -> Option<Key> {
        if place.is_some() && !self.room(thread, cx) {
            return None;
        }
        let key = Key { thread: thread.clone(), tab: self.mint() };
        let mut entry = Entry::new(workspace, None);
        if let Some(place) = place {
            entry.tab.navigate(Some(place));
        }
        self.entries.insert(key.clone(), entry);
        Some(key)
    }

    /// `roomForBrowserTab`: whether one more tab may hold a page, saying the cap's line when it may not. What counts is
    /// the pages held, since the cap is for their memory.
    pub fn room(&mut self, thread: &SharedString, cx: &mut Context<Self>) -> bool {
        let held: Vec<&Key> = self.entries.iter().filter(|(_, e)| e.page.is_some()).map(|(k, _)| k).collect();
        let Some((open, elsewhere)) = over_cap(&held, thread) else { return true };
        notify(cx, Notice::new(NoticeKind::Note, words::cap_line(open, elsewhere)));
        false
    }

    /// The tab's place is now `place`, loaded into its page when it holds one; the same place loads nothing.
    pub fn navigate(&mut self, key: &Key, place: Option<Place>, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get_mut(key) else { return };
        if entry.tab.navigate(place) {
            self.load(key, cx);
        }
        self.prune_routes();
        cx.notify();
    }

    /// Loads the tab's place into its page when the page has not loaded this count yet.
    fn load(&mut self, key: &Key, cx: &mut Context<Self>) {
        let Some(target) = self.target(key) else { return };
        let Some(entry) = self.entries.get_mut(key) else { return };
        let Some(page) = &entry.page else { return };
        if entry.loaded == entry.tab.loads {
            return;
        }
        entry.loaded = entry.tab.loads;
        page.update(cx, |page, _| page.load(&target));
    }

    /// The tab's page for its slot in `window`: made now on its place when it holds none, as a shown tab whose page
    /// was unloaded loads again; the page hidden longest goes to keep the cap.
    pub fn page_for(&mut self, key: &Key, window: &mut Window, cx: &mut Context<Self>) -> Result<Option<Entity<Page>>, String> {
        let Some(entry) = self.entries.get(key) else { return Ok(None) };
        // A page lives in the window it was made in; a tab of the same name in another window makes its own.
        if entry.page.as_ref().is_some_and(|p| p.read(cx).window_id() != window.window_handle().window_id()) {
            self.drop_page(key);
        }
        let Some(entry) = self.entries.get(key) else { return Ok(None) };
        if entry.tab.current().is_none() || entry.page.is_some() {
            return Ok(entry.page.clone());
        }
        // A port off this computer has no page until its route is minted.
        let Some(target) = self.target(key) else { return Ok(None) };
        if self.entries.values().filter(|e| e.page.is_some()).count() >= CAP {
            let longest = self.entries.iter().filter_map(|(k, e)| e.hidden_since.map(|at| (k.clone(), at))).min_by_key(|(_, at)| *at);
            if let Some((longest, _)) = longest {
                self.drop_page(&longest);
            }
        }
        let zoom = self.entries.get(key).map_or(1., |e| e.tab.zoom);
        let page = Page::new(&target, zoom, window, cx)?;
        let key2 = key.clone();
        let sub = cx.subscribe(&page, move |store, page, event: &PageEvent, cx| store.page_said(&key2, &page, event, cx));
        let entry = self.entries.get_mut(key).expect("the entry was read above");
        entry.page = Some(page.clone());
        entry.loaded = entry.tab.loads;
        entry.loading = true;
        // Hidden until a frame draws it, so a page never shown still goes once the idle span is up.
        entry.hidden_since = Some(Instant::now());
        entry._page = Some(sub);
        self.schedule_sweep(cx);
        Ok(Some(page))
    }

    fn page_said(&mut self, key: &Key, page: &Entity<Page>, event: &PageEvent, cx: &mut Context<Self>) {
        // A page off this computer is on its port's minted route, which its own addresses are read back against.
        let route = self.entries.get(key).and_then(|e| match (e.remote, e.workspace.clone(), e.tab.current()) {
            (true, Some(workspace), Some(Place::Port(address))) => match self.routes.get(&(workspace, address.port))?.reach.as_ref()? {
                Reach::Ready(url) => Some((url.clone(), address.port)),
                _ => None,
            },
            _ => None,
        });
        let Some(entry) = self.entries.get_mut(key) else { return };
        let (back, forward) = page.read_with(cx, |p, _| (p.can_go_back(), p.can_go_forward()));
        entry.can_back = back;
        entry.can_forward = forward;
        match event {
            PageEvent::Loading(loading) => entry.loading = *loading,
            PageEvent::Moved(to) => {
                let place = url::place_of_url(to, route.as_ref().map(|(url, port)| (url.as_str(), *port)));
                entry.tab.follow(place);
            }
            PageEvent::Titled(title) => entry.tab.titled(title),
            PageEvent::NewWindow(url) => cx.emit(Asked::NewTab { from: key.clone(), url: url.clone() }),
            PageEvent::Leave(url) => cx.emit(Asked::Leave(url.clone())),
            PageEvent::Gone(_) => entry.loading = false,
            PageEvent::Focused => {}
            PageEvent::Shown(shown) => entry.hidden_since = (!shown).then(Instant::now),
        }
        if matches!(event, PageEvent::Shown(_)) {
            self.schedule_sweep(cx);
        }
        cx.notify();
    }

    /// A thread's panel showed a browser tab: the pages of threads past the last few shown go.
    pub fn thread_shown(&mut self, thread: &SharedString, cx: &mut Context<Self>) {
        if thread.is_empty() {
            return;
        }
        self.shown.retain(|t| t != thread);
        self.shown.insert(0, thread.clone());
        self.shown.truncate(LOADED_THREADS);
        let held: Vec<&Key> = self.entries.iter().filter(|(_, e)| e.page.is_some()).map(|(k, _)| k).collect();
        for key in past(&held, &self.shown) {
            self.drop_page(&key);
        }
        cx.notify();
    }

    /// A window closed: its tabs and their pages go with it.
    fn window_closed(&mut self, window: gpui_kit::WindowId, cx: &mut Context<Self>) {
        let gone: Vec<Key> = self
            .entries
            .iter()
            .filter(|(_, e)| e.page.as_ref().is_some_and(|p| p.read(cx).window_id() == window))
            .map(|(k, _)| k.clone())
            .collect();
        for key in gone {
            self.entries.remove(&key);
        }
        self.prune_routes();
        cx.notify();
    }

    /// The tab closed for good: its page and its place go.
    pub fn closed(&mut self, key: &Key, cx: &mut Context<Self>) {
        self.entries.remove(key);
        self.prune_routes();
        cx.notify();
    }

    /// Routes, and the asking and probing they carry, are kept only for a port some tab is on.
    fn prune_routes(&mut self) {
        let wanted: Vec<(String, u16)> = self
            .entries
            .values()
            .filter(|e| e.remote)
            .filter_map(|e| match (e.workspace.clone(), e.tab.current()) {
                (Some(workspace), Some(Place::Port(address))) => Some((workspace, address.port)),
                _ => None,
            })
            .collect();
        self.routes.retain(|key, _| wanted.contains(key));
    }

    fn drop_page(&mut self, key: &Key) {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.page = None;
            entry._page = None;
            entry.hidden_since = None;
            entry.loading = false;
            entry.can_back = false;
            entry.can_forward = false;
        }
    }

    /// One timer, for the page hidden longest: nothing ticks while every page is shown or none is held.
    fn schedule_sweep(&mut self, cx: &mut Context<Self>) {
        let Some(first) = self.entries.values().filter_map(|e| e.hidden_since).min() else {
            self._sweep = None;
            return;
        };
        let wait = (first + IDLE).saturating_duration_since(Instant::now());
        self._sweep = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            _ = this.update(cx, |store, cx| {
                let hidden: Vec<(Key, Instant)> =
                    store.entries.iter().filter_map(|(k, e)| e.hidden_since.map(|at| (k.clone(), at))).collect();
                for key in due(&hidden, Instant::now()) {
                    store.drop_page(&key);
                }
                store.schedule_sweep(cx);
                cx.notify();
            });
        }));
    }

    pub fn update_tab(&mut self, key: &Key, f: impl FnOnce(&mut Tab) -> bool, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.get_mut(key) else { return };
        if f(&mut entry.tab) {
            if let (Some(page), zoom) = (&entry.page, entry.tab.zoom) {
                page.update(cx, |p, _| p.set_zoom(zoom));
            }
            cx.notify();
        }
    }

    /// The workspace's listening ports, as its daemon link last said them.
    pub fn directory(&self, workspace: &str) -> Option<&Directory> {
        self.directories.get(workspace)
    }

    pub fn directory_mut(&mut self, workspace: &str) -> &mut Directory {
        self.directories.entry(workspace.to_owned()).or_default()
    }

    /// The address the tab's page loads, None while a remote port's route is being minted or failed.
    pub fn target(&self, key: &Key) -> Option<String> {
        let entry = self.entries.get(key)?;
        let place = entry.tab.current()?;
        match place {
            Place::Port(address) if entry.remote => {
                let workspace = entry.workspace.clone()?;
                match self.routes.get(&(workspace, address.port))?.reach.as_ref()? {
                    Reach::Ready(route) => real_url(place, Some(route)),
                    _ => None,
                }
            }
            _ => real_url(place, None),
        }
    }

    pub fn route(&self, workspace: &str, port: u16) -> Option<&Route> {
        self.routes.get(&(workspace.to_owned(), port))
    }

    /// Mints the port's route, once, and asks again before it expires (`usePortReach`).
    pub fn want_route(&mut self, host: &Host, workspace: &str, port: u16, cx: &mut Context<Self>) {
        let route = self.routes.entry((workspace.to_owned(), port)).or_default();
        if route.ask.is_none() {
            route.reach = Some(Reach::Minting);
            route.ask = Some(Self::ask_route(host.clone(), workspace.to_owned(), port, cx));
        }
    }

    /// Asks for the route again now: a refused fresh token, a reload of a route that failed.
    pub fn remint(&mut self, host: &Host, workspace: &str, port: u16, cx: &mut Context<Self>) {
        if let Some(route) = self.routes.get_mut(&(workspace.to_owned(), port)) {
            route.ask = None;
        }
        self.want_route(host, workspace, port, cx);
    }

    fn ask_route(host: Host, workspace: String, port: u16, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |this, cx| loop {
            let w = workspace.clone();
            let asked = host
                .run(move |client| async move {
                    client
                        .call(WorkspacesPortReachRequest {
                            workspace_id: w,
                            port: std::num::NonZeroU64::new(u64::from(port)).unwrap_or(std::num::NonZeroU64::MIN),
                        })
                        .await
                })
                .await;
            let (reach, wait) = match asked {
                Ok(reply) => {
                    let left = reply.reach.expires_at - chrono::Utc::now().timestamp_millis() as f64;
                    (Reach::Ready(reply.reach.url), Some(reach::next_ask(Duration::from_millis(left.max(0.) as u64))))
                }
                Err(failure) => (Reach::Failed(failure.message), None),
            };
            let key = (workspace.clone(), port);
            let alive = this.update(cx, |store, cx| {
                if let Some(route) = store.routes.get_mut(&key) {
                    route.reach = Some(reach);
                }
                // A tab waiting on this route makes its page now, or loads it into the one it has.
                let waiting: Vec<Key> =
                    store.entries.iter().filter(|(_, e)| e.workspace.as_deref() == Some(&workspace)).map(|(k, _)| k.clone()).collect();
                for k in waiting {
                    store.load(&k, cx);
                }
                cx.notify();
            });
            if alive.is_err() {
                return;
            }
            let Some(wait) = wait else { return };
            cx.background_executor().timer(wait).await;
        })
    }

    /// One fetch of the port's route for the tab's load `load` (`useProbedRoute`): a first 401 remints; 502 and 504
    /// say the port is down, fetched again every two seconds until it answers (`usePortAnswering`).
    pub fn probe(&mut self, host: &Host, workspace: &str, port: u16, load: u64, cx: &mut Context<Self>) {
        let key = (workspace.to_owned(), port);
        let Some(route) = self.routes.get_mut(&key) else { return };
        if route.probed_for == Some(load) || !matches!(route.reach, Some(Reach::Ready(_))) {
            return;
        }
        route.probed_for = Some(load);
        route.refusal = None;
        let host = host.clone();
        route.probe = Some(cx.spawn(async move |this, cx| loop {
            let (h, w) = (host.clone(), key.0.clone());
            let probed = h
                .run(move |client| async move {
                    client
                        .call(WorkspacesPortProbeRequest {
                            workspace_id: w,
                            port: std::num::NonZeroU64::new(u64::from(port)).unwrap_or(std::num::NonZeroU64::MIN),
                        })
                        .await
                })
                .await;
            let Ok(probed) = probed else { return };
            let (status, body) = (probed.probe.status, probed.probe.body);
            let down = matches!(status, 502 | 504);
            let remint = this
                .update(cx, |store, cx| {
                    let Some(route) = store.routes.get_mut(&key) else { return false };
                    route.answering = Some(!down);
                    let remint = status == 401 && route.reminted_for != Some(load);
                    if remint {
                        route.reminted_for = Some(load);
                        route.probed_for = None;
                    } else if let Some(Reach::Ready(url)) = &route.reach {
                        route.refusal = reach::read_probe(status, &body, port, url);
                    }
                    cx.notify();
                    remint
                })
                .unwrap_or(false);
            if remint {
                _ = this.update(cx, |store, cx| store.remint(&host, &key.0, port, cx));
                return;
            }
            if !down {
                return;
            }
            cx.background_executor().timer(reach::ANSWER_POLL).await;
        }));
    }

    /// The tabs that hold a page now; for the checks and the memory reading.
    pub fn pages_held(&self) -> usize {
        self.entries.values().filter(|e| e.page.is_some()).count()
    }
}

/// What a page is loaded on: a port of a workspace here is dialled as `localhost:<port>`, which is what it answers on;
/// a port off this computer through its minted route with the path put before its query; a site is its
/// own address.
pub fn real_url(place: &Place, route: Option<&str>) -> Option<String> {
    match (place, route) {
        (Place::Port(address), Some(route)) => url::frame_src(route, &address.path),
        _ => Some(url::place_url(place)),
    }
}

/// The cap's count when one more page would pass it: the pages held, and how many of them are in other threads.
pub fn over_cap(held: &[&Key], thread: &SharedString) -> Option<(usize, usize)> {
    (held.len() >= CAP).then(|| (held.len(), held.iter().filter(|k| k.thread != *thread).count()))
}

/// The held pages of threads past the last few shown; this computer's own panel keeps its pages.
pub fn past(held: &[&Key], shown: &[SharedString]) -> Vec<Key> {
    held.iter().filter(|k| !k.thread.is_empty() && !shown.contains(&k.thread)).map(|k| (*k).clone()).collect()
}

/// The pages hidden for the idle span by `now`.
pub fn due(hidden: &[(Key, Instant)], now: Instant) -> Vec<Key> {
    hidden.iter().filter(|(_, at)| now.saturating_duration_since(*at) >= IDLE).map(|(k, _)| k.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(thread: &str, tab: &str) -> Key {
        Key { thread: thread.to_owned().into(), tab: tab.to_owned().into() }
    }

    #[test]
    fn the_eleventh_page_is_refused_naming_those_in_other_threads() {
        let keys: Vec<Key> = (0..CAP).map(|i| key(if i < 7 { "t1" } else { "t2" }, &format!("browser:{i}"))).collect();
        let held: Vec<&Key> = keys.iter().collect();
        assert_eq!(over_cap(&held, &"t1".into()), Some((10, 3)));
        assert_eq!(over_cap(&held[..9], &"t1".into()), None, "nine held leave room for a tenth");
    }

    #[test]
    fn pages_past_the_last_three_threads_go_and_this_computers_stay() {
        let keys = [key("t1", "a"), key("t2", "b"), key("t5", "c"), key("", "d")];
        let held: Vec<&Key> = keys.iter().collect();
        let shown: Vec<SharedString> = vec!["t1".into(), "t2".into(), "t3".into()];
        assert_eq!(past(&held, &shown), vec![key("t5", "c")]);
    }

    #[gpui_kit::test]
    fn only_the_tabs_this_pane_made_skip_the_caps_question(cx: &mut gpui_kit::TestAppContext) {
        let store = cx.update(Store::get);
        let made = store.update(cx, |s, _| s.mint());
        store.read_with(cx, |s, _| {
            assert!(s.minted_here(&made), "{made} was made here, asking the cap as it was");
            assert!(!s.minted_here("browser:p2"), "one not made yet");
            assert!(!s.minted_here("browser:3"), "the panel's own number, from B or Add a panel");
        });
    }

    #[test]
    fn a_page_hidden_five_minutes_is_unloaded_and_one_hidden_less_stays() {
        let now = Instant::now();
        let hidden = [(key("t1", "a"), now - IDLE), (key("t1", "b"), now - IDLE + Duration::from_secs(1))];
        assert_eq!(due(&hidden, now), vec![key("t1", "a")]);
    }
}
