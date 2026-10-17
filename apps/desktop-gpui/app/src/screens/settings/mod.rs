// SPDX-License-Identifier: AGPL-3.0-only
//! Settings, after `apps/web/src/settings/`: its own sidebar in the app sidebar's place (the groups, a search field,
//! the pages under a group, Back), the crumbs and the header's act in the top bar, and the open page in the centre,
//! one card per section. Every page is a plain function of `Ctx`, so the search walks every group's rows.

pub mod frame;
pub mod groups;
pub mod model;
pub mod page;
pub mod pages;

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;

use futures::channel::{mpsc, oneshot};
use futures::StreamExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, WindowExt as _};
use gpui_kit::{prelude::*, px, App, Context, Entity, ScrollHandle, SharedString, Subscription, Task, WeakEntity, Window};
use serde_json::{json, Value};
use tokio::runtime::Runtime;
use wsp_host_client::ops::settings::{
    Account, CapabilitiesReply, Devices, Editors, Image, Places, PreferencesReply, PreferencesSet, Recipes, Release, SshInclude,
};
use wsp_host_client::ops::sidebar::Projects;
use wsp_host_client::ops::thread::Harnesses;
use wsp_host_client::protocol::{
    AccountGetRequest, CapabilitiesGetRequest, DevicesListRequest, EditorListRequest, HarnessesListRequest, ImageGetRequest,
    InitGetRequest, PlacesListRequest, PreferencesGetRequest, ProjectsListRequest, RecipesListRequest, ReleaseCheckRequest,
    ReleaseGetRequest, SshIncludeRequest,
};
use wsp_host_client::{Env, Event, Failure, HostClient, Op};

use crate::ui::theme;
use crate::ui::ActivePalette;
use model::{req, At, Data, GroupId, Push, Read, Reread};
pub use page::{CardData, Ctx, Item};

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

/// Back was pressed, or Escape: whoever holds Settings closes it.
pub struct Closed;

impl gpui_kit::EventEmitter<Closed> for SettingsScreen {}

pub struct SettingsScreen {
    pub(crate) data: Data,
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
    /// A dialog the page asked for, drawn over the screen until closed.
    dialog: Option<page::DialogBuild>,
    /// The page under a group's list that stands in place of it (one server or skill under Agents), for the crumbs.
    level: Option<page::Level>,
    /// The computer a remove is taking out while its dialog still has something to say: its page stays on screen.
    parting: Option<String>,
    /// Drawn as the page alone, the frame drawing the sidebar and the crumbs from `part`.
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
    pub fn new(opening: Opening, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let Opening { state, env, runtime, at, scroll } = opening;
        let dial = async move { HostClient::connect(&state, &env).await.map(Arc::new) };
        SettingsScreen::start(dial, runtime, at, scroll, window, cx)
    }

    /// Settings on a host already dialled, as the app's frame holds it: drawn as its page alone, with the sidebar and
    /// the crumbs as parts the frame lays in its own sidebar column and header row (`part`).
    pub fn with_client(client: Arc<HostClient>, runtime: Arc<Runtime>, at: At, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut screen = SettingsScreen::start(async move { Ok(client) }, runtime, at, 0., window, cx);
        screen.framed = true;
        screen
    }

    fn start(
        dial: impl Future<Output = Result<Arc<HostClient>, Failure>> + Send + 'static,
        runtime: Arc<Runtime>,
        at: At,
        scroll: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            // Listening before the reads, so a change that lands between a read and its answer is not lost.
            let events = host.events().await;
            read_all(&host, &tx).await;
            let Ok(mut events) = events else { return };
            while let Some(next) = events.next().await {
                if let Ok(Event::Host(event)) = next {
                    if let Ok(value) = serde_json::to_value(&*event) {
                        _ = tx.unbounded_send(Arrival::Pushed(value));
                    }
                }
            }
        });
        let receive = cx.spawn(async move |this, cx| {
            while let Some(arrival) = rx.next().await {
                if this.update(cx, |this, cx| this.arrive(arrival, cx)).is_err() {
                    break;
                }
            }
        });
        SettingsScreen {
            data: Data::default(),
            at,
            search: String::new(),
            search_field,
            page_scroll: ScrollHandle::new(),
            host: None,
            runtime,
            unreachable: None,
            notice: None,
            dialog: None,
            level: None,
            clear_field: false,
            parting: None,
            start_scroll: scroll,
            opened: std::time::Instant::now(),
            read_at: None,
            framed: false,
            drawn: Drawn::default(),
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
        SettingsScreen {
            data,
            at,
            search: String::new(),
            search_field,
            page_scroll: ScrollHandle::new(),
            host: None,
            runtime,
            unreachable: None,
            notice: None,
            dialog: None,
            level: None,
            clear_field: false,
            parting: None,
            start_scroll: 0.,
            opened: std::time::Instant::now(),
            read_at: None,
            framed: false,
            drawn: Drawn::default(),
            _follow: None,
            _receive: Task::ready(()),
            _search: search,
        }
    }

    fn arrive(&mut self, arrival: Arrival, cx: &mut Context<Self>) {
        match arrival {
            Arrival::Connected(host) => self.host = Some(host),
            Arrival::Read(read) => {
                let read = *read;
                let preferences = matches!(read, Read::Preferences(Ok(_)));
                self.data.take(read);
                if preferences {
                    self.apply_theme(cx);
                }
            }
            Arrival::Pushed(event) => match self.data.push(&event) {
                Push::Nothing => return,
                Push::Drawn => {
                    if event.get("type").and_then(Value::as_str) == Some("preferences.changed") {
                        self.apply_theme(cx);
                    }
                }
                Push::Reread(what) => self.reread(what, cx),
            },
            Arrival::Failed(said) => {
                eprintln!("desktop-gpui: {said}");
                self.unreachable = Some(said);
            }
        }
        cx.notify();
    }

    /// The window's theme as the preferences pick it: the side asked for, or the system's.
    fn apply_theme(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.data.preferences() else { return };
        let mode = p.theme.to_string();
        let dark = match mode.as_str() {
            "dark" => true,
            "light" => false,
            _ => matches!(cx.window_appearance(), gpui_kit::WindowAppearance::Dark | gpui_kit::WindowAppearance::VibrantDark),
        };
        let id = if dark { p.dark_theme.clone() } else { p.light_theme.clone() };
        let id = if theme::theme_by_id(&id).is_some() {
            id
        } else if dark {
            theme::DARK.to_owned()
        } else {
            theme::LIGHT.to_owned()
        };
        if theme::current(cx).id != id {
            theme::set_theme(&id, cx);
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

    /// Escape takes the nearest thing open: a dialog, then the field's text, then Settings itself.
    pub fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.dialog.is_some() {
            self.close_dialog(cx);
        } else if !self.search.is_empty() {
            self.search.clear();
            self.search_field.update(cx, |field, cx| field.set_value("", window, cx));
            cx.notify();
        } else {
            cx.emit(Closed);
        }
    }

    pub fn data(&self) -> &Data {
        &self.data
    }

    pub fn at(&self) -> &At {
        &self.at
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    /// Moves to a page; the search is cleared, as the web's `go` does.
    pub fn go(&mut self, at: At, window: &mut Window, cx: &mut Context<Self>) {
        self.at = at;
        self.level = None;
        self.page_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        if !self.search.is_empty() {
            self.search.clear();
            self.search_field.update(cx, |field, cx| field.set_value("", window, cx));
        }
        if self.at.group() == GroupId::General {
            if let Some(host) = self.host.clone() {
                self.spawn_read(cx, async move {
                    Read::Release(host.call(req::<ReleaseCheckRequest>(json!({}))).await.map(|r| r.release).map_err(said))
                });
            }
        }
        cx.notify();
    }

    /// The same, from a place that holds no window: the field's text is cleared on the next frame.
    fn go_later(&mut self, at: At, cx: &mut Context<Self>) {
        self.at = at;
        self.level = None;
        self.page_scroll.set_offset(gpui_kit::point(px(0.), px(0.)));
        self.clear_field = !self.search.is_empty();
        self.search.clear();
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
        }
    }

    /// Writes a patch of preferences, laid onto the records at once and then as the host answers.
    pub fn set_preferences(&mut self, patch: Value, cx: &mut Context<Self>) {
        if let Some(current) = self.data.preferences.take() {
            let mut merged = serde_json::to_value(&current).unwrap_or(Value::Null);
            merge(&mut merged, &patch);
            self.data.preferences = Some(serde_json::from_value(merged).unwrap_or(current));
            self.apply_theme(cx);
            cx.notify();
        }
        self.call(PatchPreferences { patch }, cx, |this, answer, cx| match answer {
            Ok(PreferencesSet { preferences, notice }) => {
                this.data.preferences = Some(preferences);
                this.apply_theme(cx);
                if let Some(notice) = notice {
                    this.notice = Some((notice.into(), Said::Done));
                }
                cx.notify();
            }
            Err(failure) => this.failed(failure, cx),
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

    pub fn open_dialog(&mut self, dialog: page::DialogBuild, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        cx.notify();
    }

    pub fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    pub fn runtime(&self) -> &Arc<Runtime> {
        &self.runtime
    }

    pub fn host(&self) -> Option<&Arc<HostClient>> {
        self.host.as_ref()
    }
}

/// `preferences.set` with its patch sent as written: the typed request drops a null, and a null is how a patch deletes a
/// key (a row's reset, a project's default put back).
#[derive(serde::Serialize)]
struct PatchPreferences {
    patch: Value,
}

impl Op for PatchPreferences {
    const NAME: &'static str = "preferences.set";
    type Reply = PreferencesSet;
}

/// A failure in the host's words.
fn said(failure: Failure) -> String {
    failure.message
}

/// Lays a patch onto a record, key by key; a null deletes.
fn merge(into: &mut Value, patch: &Value) {
    match (into, patch) {
        (Value::Object(into), Value::Object(patch)) => {
            for (key, value) in patch {
                match value {
                    Value::Null => {
                        into.remove(key);
                    }
                    Value::Object(_) if into.get(key).is_some_and(Value::is_object) => merge(into.get_mut(key).unwrap(), value),
                    _ => {
                        into.insert(key.clone(), value.clone());
                    }
                }
            }
        }
        (into, patch) => *into = patch.clone(),
    }
}

async fn places(host: &HostClient) -> Read {
    Read::Places(host.call(req::<PlacesListRequest>(json!({}))).await.map(|r: Places| (r.places, r.pending)).map_err(said))
}

async fn devices(host: &HostClient) -> Read {
    Read::Devices(host.call(req::<DevicesListRequest>(json!({}))).await.map(|r: Devices| r.devices).map_err(|f| {
        let ticket = f.kind.as_deref() == Some("ticket");
        (f.message, ticket)
    }))
}

/// Every read Settings makes as it opens, each sent as it lands (`settingsReads.ts` and the store's `pull`).
async fn read_all(host: &HostClient, tx: &mpsc::UnboundedSender<Arrival>) {
    let send = |read: Read| {
        _ = tx.unbounded_send(Arrival::Read(Box::new(read)));
    };
    let preferences = async {
        send(Read::Preferences(
            host.call(req::<PreferencesGetRequest>(json!({}))).await.map(|r: PreferencesReply| r.preferences).map_err(said),
        ))
    };
    let places = async { send(places(host).await) };
    let projects =
        async { send(Read::Projects(host.call(req::<ProjectsListRequest>(json!({}))).await.map(|r: Projects| r.projects).map_err(said))) };
    let harnesses = async {
        send(Read::Harnesses(host.call(req::<HarnessesListRequest>(json!({}))).await.map(|r: Harnesses| r.harnesses).map_err(said)))
    };
    let recipes =
        async { send(Read::Recipes(host.call(req::<RecipesListRequest>(json!({}))).await.map(|r: Recipes| r.recipes).map_err(said))) };
    let release =
        async { send(Read::Release(host.call(req::<ReleaseGetRequest>(json!({}))).await.map(|r: Release| r.release).map_err(said))) };
    let capabilities = async {
        send(Read::Capabilities(
            host.call(req::<CapabilitiesGetRequest>(json!({}))).await.map(|r: CapabilitiesReply| r.capabilities).map_err(said),
        ))
    };
    let setup = async { send(Read::Setup(host.call(req::<InitGetRequest>(json!({}))).await.map(|r| r.setup).map_err(said))) };
    let account =
        async { send(Read::Account(host.call(req::<AccountGetRequest>(json!({}))).await.map(|r: Account| r.account).map_err(said))) };
    let devices = async { send(devices(host).await) };
    let image = async { send(Read::Image(host.call(req::<ImageGetRequest>(json!({}))).await.map(|r: Image| r.view).map_err(said))) };
    let editors =
        async { send(Read::Editors(host.call(req::<EditorListRequest>(json!({}))).await.map(|r: Editors| r.editors).map_err(said))) };
    let ssh = async {
        send(Read::SshInclude(host.call(req::<SshIncludeRequest>(json!({}))).await.map(|r: SshInclude| r.ssh_include).map_err(said)))
    };
    futures::join!(preferences, places, projects, harnesses, recipes, release, capabilities, setup, account, devices, image, editors, ssh);
}

impl Render for SettingsScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
            use gpui_kit::component::notification::Notification;
            let note = match said {
                Said::Failed => Notification::error(words),
                Said::Done => Notification::success(words),
                Said::Note => Notification::info(words),
            };
            window.push_notification(note, cx);
        }
        let p = cx.palette().clone();
        let font = gpui_kit::component::ActiveTheme::theme(&**cx).font_family.clone();
        let width = window.viewport_size().width;
        let at = self.resolved();
        let searching = !self.search.is_empty();
        if searching || at != At::Group(GroupId::Appearance) {
            // A theme the Appearance page showed under the pointer goes with the page (`ThemePicker.tsx`).
            self.apply_theme(cx);
        }
        let acts = Acts::new(screen.clone(), self.host.clone(), self.runtime.clone());
        let ctx = Ctx::new(&self.data, &at, self.level.as_ref(), acts);
        let locked = !searching && matches!(at, At::Group(GroupId::Usage | GroupId::Activity));
        let body = frame::Body::build(&ctx, &self.search, self.unreachable.as_deref(), window, cx);
        let trail = in_view(&body.trail, &self.page_scroll);
        let dialog = self.dialog.clone().map(|build| build(&ctx, window, cx));
        let root = h_flex()
            .id("settings")
            .key_context("Settings")
            .size_full()
            .relative()
            .capture_key_down(cx.listener(|this, event: &gpui_kit::KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" && event.keystroke.modifiers == gpui_kit::Modifiers::none() {
                    this.escape(window, cx);
                    cx.stop_propagation();
                }
            }))
            .bg(p.background)
            .text_color(p.foreground)
            .font_family(font)
            .child(crate::ui::grain(window.viewport_size()));
        let root = if self.framed {
            self.drawn = Drawn { hits: body.hits.clone(), trail: trail.clone() };
            root.child(frame::page(body, &self.page_scroll, width, locked, cx))
        } else {
            let sidebar = frame::sidebar(&ctx, &self.search_field, body.hits.as_deref(), &trail, true, window.viewport_size().height, cx);
            let top_bar = frame::top_bar(&ctx, searching, false, window, cx);
            root.child(sidebar).child(v_flex().flex_1().min_w_0().h_full().child(top_bar).child(frame::page(
                body,
                &self.page_scroll,
                width,
                locked,
                cx,
            )))
        };
        root.children(dialog.map(|d| frame::dialog_layer(d, screen, cx)))
    }
}

/// What a frame worked out that the sidebar draws: each group's matches while searching, the trail's heads in view.
#[derive(Default, Clone)]
struct Drawn {
    hits: Option<Vec<(GroupId, usize)>>,
    trail: Vec<(SharedString, usize, bool)>,
}

/// A piece of Settings the app's frame lays in its own place: the page list in its sidebar column under its frame row,
/// or the crumbs and the header's act in its header row.
#[derive(Clone, Copy, PartialEq)]
pub enum Part {
    Sidebar,
    Header,
}

/// One of a framed screen's parts as a view of its own, drawn from the screen and again whenever the screen moves.
pub struct SettingsPart {
    screen: Entity<SettingsScreen>,
    part: Part,
    _moved: Subscription,
}

impl SettingsPart {
    pub fn new(screen: &Entity<SettingsScreen>, part: Part, cx: &mut Context<Self>) -> Self {
        SettingsPart { screen: screen.clone(), part, _moved: cx.observe(screen, |_, _, cx| cx.notify()) }
    }
}

impl Render for SettingsPart {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let part = self.part;
        let weak = self.screen.downgrade();
        self.screen.update(cx, |s, cx| {
            let at = s.resolved();
            let acts = Acts::new(weak, s.host.clone(), s.runtime.clone());
            let ctx = Ctx::new(&s.data, &at, s.level.as_ref(), acts);
            match part {
                Part::Sidebar => {
                    frame::sidebar(&ctx, &s.search_field, s.drawn.hits.as_deref(), &s.drawn.trail, false, window.viewport_size().height, cx)
                        .into_any_element()
                }
                Part::Header => frame::top_bar(&ctx, !s.search.is_empty(), true, window, cx).into_any_element(),
            }
        })
    }
}

/// The trail's heads, each with whether its card is on screen now; a card is the page column's child at its index.
fn in_view(trail: &[(SharedString, usize)], scroll: &ScrollHandle) -> Vec<(SharedString, usize, bool)> {
    let shown = scroll.bounds();
    trail
        .iter()
        .map(|(head, ix)| {
            let seen = scroll.bounds_for_item(*ix).is_some_and(|b| b.bottom() > shown.top() && b.top() < shown.bottom());
            (head.clone(), *ix, seen)
        })
        .collect()
}

/// Opens Settings alone in a window, as the capture flag does: `--screen settings[:<page>]`.
pub fn open_alone(opening: Opening, window: &mut Window, cx: &mut App) -> Entity<SettingsScreen> {
    cx.new(|cx| SettingsScreen::new(opening, window, cx))
}

/// The acts a page's row raises, held past the frame that drew it. Each is laid onto the screen once whatever is
/// running is over, so an act raised while the screen draws or takes an answer never re-enters it.
#[derive(Clone)]
pub struct Acts {
    screen: WeakEntity<SettingsScreen>,
    host: Option<Arc<HostClient>>,
    runtime: Arc<Runtime>,
}

impl Acts {
    pub fn new(screen: WeakEntity<SettingsScreen>, host: Option<Arc<HostClient>>, runtime: Arc<Runtime>) -> Self {
        Acts { screen, host, runtime }
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
        let screen = self.screen.clone();
        window.defer(cx, move |window, cx| {
            if let Some(screen) = screen.upgrade() {
                screen.update(cx, |s, cx| s.go(at, window, cx));
            }
        });
    }

    pub fn go_later(&self, at: At, cx: &mut App) {
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

    pub fn failed(&self, said: impl Into<String>, cx: &mut App) {
        let said = said.into();
        self.with(cx, |s, cx| s.failed(said, cx));
    }

    pub fn done(&self, line: impl Into<String>, cx: &mut App) {
        let line = line.into();
        self.with(cx, |s, cx| s.done(line, cx));
    }

    /// A plain fact about an act, neither a failure nor a success: an add whose dialog this app does not draw.
    pub fn note(&self, line: impl Into<String>, cx: &mut App) {
        let line = line.into();
        self.with(cx, |s, cx| s.note(line, cx));
    }

    pub fn reread(&self, what: Reread, cx: &mut App) {
        self.with(cx, move |s, cx| s.reread(what, cx));
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

    /// Scrolls the page to its `ix`th card, as a press on the sidebar's trail does.
    pub fn reveal_section(&self, ix: usize, cx: &mut App) {
        self.with(cx, move |s, cx| {
            // To the card's top, as far as the page goes (`scrollIntoView({ block: "start" })`).
            let scroll = &s.page_scroll;
            if let Some(card) = scroll.bounds_for_item(ix) {
                let y = (scroll.offset().y - (card.top() - scroll.bounds().top())).clamp(-scroll.max_offset().y, px(0.));
                scroll.set_offset(gpui_kit::point(scroll.offset().x, y));
            }
            cx.notify();
        });
    }

    /// Back: Settings asks whoever holds it to close it.
    pub fn close(&self, _window: &mut Window, cx: &mut App) {
        self.with(cx, |_, cx| cx.emit(Closed));
    }

    /// The runtime and the host, for a page that follows something of its own (Activity's processes).
    pub fn host(&self, _cx: &App) -> Option<(Arc<HostClient>, Arc<Runtime>)> {
        Some((self.host.clone()?, self.runtime.clone()))
    }
}
